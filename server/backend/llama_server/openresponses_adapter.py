"""Translate OpenResponses (Pi) requests to llama-server chat completions."""

from __future__ import annotations

import json
import time
import traceback
import uuid
from collections.abc import AsyncGenerator
from typing import Any

from openresponses_types.types import (
    InputTokensDetails,
    OutputTokensDetails,
    Usage,
)

from ..commons import (
    _get_response_on_completed,
    _get_response_on_create,
    _process_error_event,
    _process_init_reasoning_events,
    _process_output_item_added,
    _process_output_item_delta,
    _process_output_item_done,
    _process_stop_reasoning_events,
    _process_stop_tool_call_events,
    _sse,
)
from ...schemas import OutputItemDeltaModel, ResponsesRequest
from .client import stream_chat_completions

# Map Pi/OpenResponses reasoning effort levels onto the `reasoning_effort`
# values Harmony-style templates (gpt-oss) accept. gpt-oss only supports
# low/medium/high, so `xhigh` clamps to `high`.
_REASONING_EFFORT_MAP = {
    "low": "low",
    "medium": "medium",
    "high": "high",
    "xhigh": "high",
}


def _reasoning_template_kwargs(effort_value: str | None) -> dict[str, Any]:
    """Build model-agnostic chat_template_kwargs for reasoning control.

    Different chat templates read different keys, so we pass all of them and let
    each template use what it understands (Jinja ignores unknown kwargs):
      - Qwen3-style templates read `enable_thinking` (bool).
      - gpt-oss (Harmony) reads `reasoning_effort` ("low"/"medium"/"high").

    `none` turns thinking off where the template supports it (Qwen); gpt-oss has
    no off switch, so it falls back to its own template default in that case.
    """
    enable_thinking = effort_value not in (None, "none")
    kwargs: dict[str, Any] = {"enable_thinking": enable_thinking}
    reasoning_effort = _REASONING_EFFORT_MAP.get(effort_value or "")
    if reasoning_effort is not None:
        kwargs["reasoning_effort"] = reasoning_effort
    return kwargs


def _text_from_content(content: Any) -> str:
    if content is None:
        return ""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts: list[str] = []
        for part in content:
            if isinstance(part, dict):
                if part.get("type") in ("input_text", "output_text", "summary_text"):
                    parts.append(str(part.get("text") or ""))
                elif "text" in part:
                    parts.append(str(part["text"]))
            elif hasattr(part, "text"):
                parts.append(str(part.text))
            elif hasattr(part, "model_dump"):
                dumped = part.model_dump()
                parts.append(str(dumped.get("text") or ""))
        return "".join(parts)
    if hasattr(content, "root"):
        return str(content.root)
    return str(content)


def _item_to_dict(item: Any) -> dict[str, Any]:
    if isinstance(item, dict):
        return item
    if hasattr(item, "model_dump"):
        return item.model_dump()
    raise TypeError(f"Unsupported input item type: {type(item)!r}")


def openresponses_input_to_messages(
    input_items: str | list[Any],
) -> list[dict[str, Any]]:
    if isinstance(input_items, str):
        return [{"role": "user", "content": input_items}]

    messages: list[dict[str, Any]] = []
    for raw in input_items:
        item = _item_to_dict(raw)
        item_type = item.get("type")
        role = item.get("role")

        if item_type == "reasoning":
            continue

        if item_type == "function_call":
            call_id = item.get("call_id") or item.get("id") or f"call_{uuid.uuid4().hex[:8]}"
            messages.append(
                {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": call_id,
                            "type": "function",
                            "function": {
                                "name": item.get("name") or "",
                                "arguments": item.get("arguments") or "",
                            },
                        }
                    ],
                }
            )
            continue

        if item_type == "function_call_output":
            call_id = item.get("call_id") or ""
            messages.append(
                {
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": _text_from_content(item.get("output")),
                }
            )
            continue

        if role == "developer":
            messages.append({"role": "system", "content": _text_from_content(item.get("content"))})
            continue

        if role in ("system", "user", "assistant"):
            messages.append({"role": role, "content": _text_from_content(item.get("content"))})
            continue

    return messages


def openresponses_tools_to_chat_tools(
    tools: list[Any] | None,
) -> list[dict[str, Any]] | None:
    if not tools:
        return None

    chat_tools: list[dict[str, Any]] = []
    for raw in tools:
        item = _item_to_dict(raw)
        chat_tools.append(
            {
                "type": "function",
                "function": {
                    "name": item["name"],
                    "description": item.get("description") or "",
                    "parameters": item.get("parameters")
                    or {"type": "object", "properties": {}},
                },
            }
        )
    return chat_tools


def _effective_max_tokens(
    request: ResponsesRequest, llama_config: dict[str, Any]
) -> int | None:
    """Return max_tokens for the upstream request, or None to defer to llama-server."""
    context_length = llama_config.get("context_length")
    if request.max_output_tokens is not None:
        if context_length is None:
            return request.max_output_tokens
        return min(request.max_output_tokens, max(int(context_length) // 2, 256))
    if context_length is None:
        return None
    return max(int(context_length) // 2, 256)


async def generate_response_chat_stream(
    request: ResponsesRequest,
    llama_config: dict[str, Any],
) -> AsyncGenerator[str, None]:
    created = int(time.time())
    response_id = f"resp_{uuid.uuid4()}"
    message_id = f"msg_{uuid.uuid4()}"
    sequence_number = 0
    output_index = 0
    output_items: list[dict[str, Any]] = []

    initial_response = _get_response_on_create(response_id, request, created)
    resp_str, sequence_number = _sse(
        "response.created", {"response": initial_response}, sequence_number
    )
    yield resp_str

    messages = openresponses_input_to_messages(request.input)
    tools = openresponses_tools_to_chat_tools(request.tools)
    body: dict[str, Any] = {
        "model": request.model,
        "messages": messages,
        "stream": True,
        "temperature": request.temperature if request.temperature is not None else 0.7,
        "top_p": request.top_p if request.top_p is not None else 1.0,
    }
    max_tokens = _effective_max_tokens(request, llama_config)
    if max_tokens is not None:
        body["max_tokens"] = max_tokens
    if tools:
        body["tools"] = tools
        body["tool_choice"] = "auto"

    # Reasoning is opt-in per request and model-agnostic: Pi (or any client)
    # drives it through reasoning.effort, and we translate that into the kwargs
    # each chat template understands (see _reasoning_template_kwargs).
    effort = request.reasoning.effort if request.reasoning else None
    effort_value = getattr(effort, "value", effort)
    body["chat_template_kwargs"] = _reasoning_template_kwargs(effort_value)

    answer_text = ""
    content_index = 0
    in_message = False
    in_tool_call = False
    tool_id = ""
    tool_name: str | None = None
    tool_call_text = ""
    tool_calls_state: dict[int, dict[str, str]] = {}
    in_reasoning = False
    reasoning_text = ""
    reasoning_id = f"reasoning_{uuid.uuid4()}"
    reasoning_content_index = 0
    prompt_tokens = 0
    completion_tokens = 0

    try:
        async for chunk in stream_chat_completions(body):
            usage = chunk.get("usage")
            if usage:
                prompt_tokens = int(usage.get("prompt_tokens") or prompt_tokens)
                completion_tokens = int(usage.get("completion_tokens") or completion_tokens)

            choices = chunk.get("choices") or []
            if not choices:
                continue

            choice = choices[0]
            delta = choice.get("delta") or {}

            reasoning_piece = delta.get("reasoning_content")
            if reasoning_piece:
                if not in_reasoning:
                    in_reasoning = True
                    # Open the reasoning item with empty text; the first chunk is
                    # streamed as a delta below so it isn't dropped by the client.
                    resp_str, sequence_number = _process_init_reasoning_events(
                        reasoning_id, "", output_index, sequence_number
                    )
                    yield resp_str
                    reasoning_content_index = 0
                reasoning_text += reasoning_piece
                output_item = OutputItemDeltaModel(
                    item_name="reasoning_summary_text",
                    item_id=reasoning_id,
                    index=output_index,
                    delta=reasoning_piece,
                    content_index=reasoning_content_index,
                )
                resp_str, sequence_number = _process_output_item_delta(
                    output_item, sequence_number
                )
                yield resp_str
                reasoning_content_index += 1

            if delta.get("tool_calls"):
                if in_reasoning:
                    in_reasoning = False
                    resp_str, sequence_number, output_index, item = (
                        _process_stop_reasoning_events(
                            reasoning_id, output_index, reasoning_text, sequence_number
                        )
                    )
                    output_items.append(item)
                    yield resp_str
                    reasoning_text = ""
                for tool_delta in delta["tool_calls"]:
                    index = int(tool_delta.get("index") or 0)
                    state = tool_calls_state.setdefault(
                        index, {"id": "", "name": "", "arguments": ""}
                    )
                    if tool_delta.get("id"):
                        state["id"] = tool_delta["id"]
                    function = tool_delta.get("function") or {}
                    if function.get("name"):
                        state["name"] = function["name"]
                    if function.get("arguments"):
                        state["arguments"] += function["arguments"]

                    if not in_tool_call:
                        in_tool_call = True
                        in_message = False
                        tool_id = f"toolcall_{uuid.uuid4()}"
                        tool_name = state["name"] or None
                        tool_call_text = ""
                        content_index = 0
                        if tool_name:
                            resp_str, sequence_number = _process_output_item_added(
                                "function_call",
                                tool_id,
                                "",
                                output_index,
                                sequence_number,
                                tool_name,
                            )
                            yield resp_str
                            content_index = 1

                    arg_piece = function.get("arguments") or ""
                    if arg_piece and tool_name:
                        tool_call_text += arg_piece
                        output_item = OutputItemDeltaModel(
                            item_name="function_call_arguments",
                            item_id=tool_id,
                            index=output_index,
                            delta=arg_piece,
                            content_index=content_index,
                        )
                        resp_str, sequence_number = _process_output_item_delta(
                            output_item, sequence_number
                        )
                        yield resp_str
                        content_index += 1

            content_piece = delta.get("content")
            if content_piece:
                if in_reasoning:
                    in_reasoning = False
                    resp_str, sequence_number, output_index, item = (
                        _process_stop_reasoning_events(
                            reasoning_id, output_index, reasoning_text, sequence_number
                        )
                    )
                    output_items.append(item)
                    yield resp_str
                    reasoning_text = ""
                if in_tool_call:
                    in_tool_call = False
                    resp_str, sequence_number, output_index, item = (
                        _process_stop_tool_call_events(
                            tool_id,
                            output_index,
                            tool_call_text,
                            sequence_number,
                            request,
                            tool_name,
                        )
                    )
                    output_items.append(item)
                    yield resp_str
                    tool_call_text = ""
                    tool_name = None
                    content_index = 0

                if not in_message:
                    in_message = True
                    # Open the message item with empty text; the first chunk is
                    # streamed as a delta below so it isn't dropped by the client.
                    resp_str, sequence_number = _process_output_item_added(
                        "message", message_id, "", output_index, sequence_number
                    )
                    yield resp_str
                    content_index = 0

                answer_text += content_piece
                output_item = OutputItemDeltaModel(
                    item_name="output_text",
                    item_id=message_id,
                    index=output_index,
                    delta=content_piece,
                    content_index=content_index,
                )
                resp_str, sequence_number = _process_output_item_delta(
                    output_item, sequence_number
                )
                yield resp_str
                content_index += 1

            if choice.get("finish_reason") == "tool_calls":
                for index in sorted(tool_calls_state):
                    state = tool_calls_state[index]
                    if not state.get("name"):
                        continue
                    tool_id = f"toolcall_{uuid.uuid4()}"
                    tool_name = state["name"]
                    tool_call_text = state.get("arguments") or ""
                    resp_str, sequence_number, output_index, item = (
                        _process_stop_tool_call_events(
                            tool_id,
                            output_index,
                            tool_call_text,
                            sequence_number,
                            request,
                            tool_name,
                        )
                    )
                    output_items.append(item)
                    yield resp_str

    except Exception as exc:  # noqa: BLE001
        traceback.print_exc()
        resp_str, sequence_number = _process_error_event(
            str(exc), response_id, request, created, sequence_number
        )
        yield resp_str
        return

    if in_reasoning:
        in_reasoning = False
        resp_str, sequence_number, output_index, item = _process_stop_reasoning_events(
            reasoning_id, output_index, reasoning_text, sequence_number
        )
        output_items.append(item)
        yield resp_str
        reasoning_text = ""

    if in_tool_call and tool_name:
        resp_str, sequence_number, output_index, item = _process_stop_tool_call_events(
            tool_id,
            output_index,
            tool_call_text,
            sequence_number,
            request,
            tool_name,
        )
        output_items.append(item)
        yield resp_str
    elif in_message or answer_text:
        resp_str, sequence_number, output_index, item = _process_output_item_done(
            "message", message_id, answer_text, output_index, sequence_number
        )
        output_items.append(item)
        yield resp_str

    if prompt_tokens == 0:
        prompt_tokens = max(len(json.dumps(messages)) // 4, 1)
    if completion_tokens == 0:
        completion_tokens = max(len(answer_text) // 4, 0)

    usage = Usage(
        input_tokens=prompt_tokens,
        output_tokens=completion_tokens,
        total_tokens=prompt_tokens + completion_tokens,
        input_tokens_details=InputTokensDetails(cached_tokens=0),
        output_tokens_details=OutputTokensDetails(reasoning_tokens=0),
    )
    final_response = _get_response_on_completed(
        response_id, request, created, output_items, usage
    )
    resp_str, sequence_number = _sse(
        "response.completed", {"response": final_response}, sequence_number
    )
    yield resp_str
    yield "data: [DONE]\n\n"
