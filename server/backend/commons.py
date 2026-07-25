import json
import time

from openresponses_types.types import Usage

from ..schemas import OutputItemDeltaModel, ResponsesRequest

"""
Common utilities shared by the llama-server backend (OpenResponses streaming).
"""


def _sse(event_name: str, payload: dict, current_seq_no: int) -> tuple[str, int]:
    seq_no = current_seq_no + 1
    event = {
        "type": event_name,
        "sequence_number": seq_no,
    }
    event.update(payload)
    event_str = f"event: {event_name}\ndata: {json.dumps(event)}\n\n"

    return event_str, seq_no


def _get_response_on_create(
    response_id: str,
    request: ResponsesRequest,
    created_at: int,
) -> dict:
    created_response = {
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "completed_at": None,
        "status": "in_progress",
        "output": [],
        "incomplete_details": None,
        "text": {"format": {"type": "text"}, "verbosity": "low"},
        "parallel_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        "error": {"code": "", "message": ""},
    }
    created_response.update(_get_commons_responses(request))
    return created_response


def _get_response_on_completed(
    response_id: str,
    request: ResponsesRequest,
    created_at: int,
    output: list,
    usage: Usage,
) -> dict:
    completed_at = int(time.time())
    completed_response = {
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "completed_at": completed_at,
        "status": "completed",
        "output": output,
        "incomplete_details": None,
        "text": {"format": {"type": "text"}, "verbosity": "low"},
        "parallel_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        "error": {"code": "", "message": ""},
        "usage": {
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
            "input_token_details": {
                "cached_tokens": usage.input_tokens_details.cached_tokens
            },
            "output_token_details": {
                "reasoning_tokens": usage.output_tokens_details.reasoning_tokens
            },
        },
    }
    completed_response.update(_get_commons_responses(request))
    return completed_response


def _get_response_on_error(
    response_id: str,
    request: ResponsesRequest,
    created_at: int,
    incomplete_details: dict,
    error: dict,
) -> dict:
    created_response = {
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "completed_at": None,
        "status": "failed",
        "output": [],
        "incomplete_details": incomplete_details,
        "text": {"format": {"type": "text"}, "verbosity": "low"},
        "parallel_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        "error": error,
    }
    created_response.update(_get_commons_responses(request))
    return created_response


def _get_commons_responses(request: ResponsesRequest):
    if request.tools != None:
        tools_as_dicts = [t.model_dump() for t in request.tools]  # pyright: ignore
    else:
        tools_as_dicts = None

    return {
        "model": request.model,
        "previous_response_id": request.previous_response_id,
        "instructions": request.instructions,
        "temperature": request.temperature,
        "prompt_cache_key": request.prompt_cache,
        "safety_identifier": request.safety_identifier,
        "service_tier": request.service_tier,
        "background": request.background,
        "store": request.store,
        "max_tool_calls": request.max_tool_calls,
        "max_output_tokens": request.max_output_tokens,
        "reasoning": {"effort": request.reasoning.effort, "summary": "disabled"},
        "top_logprobs": request.top_logprobs,
        "frequency_penalty": 0,
        "presence_penalty": 0,
        "top_p": request.top_p,
        "tools": tools_as_dicts,
    }


def _process_output_item_delta(
    output_item: OutputItemDeltaModel, sequence_number: int
) -> tuple[str, int]:
    event_name = ".".join(["response", output_item.item_name, "delta"])

    event = {
        "output_index": output_item.index,
        "item_id": output_item.item_id,
        "delta": output_item.delta,
        "content_index": output_item.content_index,
    }

    return _sse(event_name, event, sequence_number)


def _process_output_item_added(
    type: str,
    id: str,
    token: str,
    output_index,
    sequence_number: int,
    tool_name: str | None = None,
) -> tuple[str, int]:
    event_name = "response.output_item.added"
    if type == "function_call":
        if not tool_name:
            raise ValueError("tool call is missing a tool name")
        item_chunk = {
            "type": type,
            "id": id,
            "name": tool_name,
            "call_id": _get_tool_call_id(id),
            "arguments": "",
            "status": "in_progress",
        }
    else:
        item_chunk = {
            "type": type,
            "id": id,
            "status": "in_progress",
            "role": "assistant",
            "content": [
                {
                    "type": "output_text",
                    "text": token,
                }
            ],
        }
    event = {
        "output_index": output_index,
        "item": item_chunk,
    }
    return _sse(event_name, event, sequence_number)


def _process_init_reasoning_events(
    id: str, token: str, output_index, sequence_number: int
) -> tuple[str, int]:
    resp_str_a, sequence_number = _process_output_item_added(
        "reasoning", id, token, output_index, sequence_number
    )

    event_name = "response.reasoning_summary_part.added"
    event = {
        "output_index": output_index,
        "item_id": id,
        "part": {"text": token, "type": "summary_text"},
        "summary_index": 0,
    }
    resp_str, sequence_number = _sse(event_name, event, sequence_number)
    return resp_str_a + resp_str, sequence_number


def _process_stop_reasoning_events(
    id: str, output_index: int, text: str, sequence_number: int
) -> tuple[str, int, int, dict]:
    payload = {
        "item_id": id,
        "output_index": output_index,
        "text": text,
    }
    resp_str_a, sequence_number = _sse(
        "response.reasoning_summary_text.done", payload, sequence_number
    )
    event_name = "response.reasoning_summary_part.done"
    event = {
        "output_index": output_index,
        "item_id": id,
        "part": {"text": text, "type": "summary_text"},
        "summary_index": 0,
    }
    resp_str_b, sequence_number = _sse(event_name, event, sequence_number)
    resp_str_c, sequence_number, output_index, item_chunk = _process_output_item_done(
        "reasoning", id, text, output_index, sequence_number
    )
    return (
        resp_str_a + resp_str_b + resp_str_c,
        sequence_number,
        output_index,
        item_chunk,
    )


def _process_output_item_done(
    type: str,
    id: str,
    final_text: str,
    output_index,
    sequence_number: int,
    tool_name: str | None = None,
) -> tuple[str, int, int, dict]:
    event_name = "response.output_item.done"
    item_chunk: dict
    if type == "function_call":
        try:
            arguments_map = json.loads(final_text)
        except json.JSONDecodeError as e:
            arguments_map = {}

        new_args = {
            ("command" if k == "cmd" else k): v for k, v in arguments_map.items()
        }
        new_args = optimize_arguments(str(tool_name), arguments_map)
        item_chunk = {
            "type": type,
            "id": id,
            "name": tool_name,
            "call_id": _get_tool_call_id(id),
            "status": "completed",
            "arguments": json.dumps(new_args),
        }
    else:
        item_chunk = {
            "type": type,
            "id": id,
            "status": "completed",
            "role": "assistant",
            "content": [
                {
                    "type": "output_text",
                    "text": final_text,
                }
            ],
        }
    if type == "reasoning":
        item_chunk.update({"summary": [{"type": "summary_text", "text": final_text}]})
    event = {
        "output_index": output_index,
        "item": item_chunk,
    }
    resp_str, sequence_number = _sse(event_name, event, sequence_number)
    output_index = output_index + 1
    return resp_str, sequence_number, output_index, item_chunk


def _process_error_event(
    err: str,
    response_id: str,
    request: ResponsesRequest,
    created_at: int,
    sequence_number: int,
) -> tuple[str, int]:
    error = {"message": err, "code": "500"}
    incomplete_details = {"reason": "internal server error"}

    err_response = _get_response_on_error(
        response_id, request, created_at, incomplete_details, error
    )
    return _sse("response.failed", {"response": err_response}, sequence_number)


def _get_tool_call_id(id: str) -> str:
    return "call_" + id.removeprefix("toolcall_")


def _process_stop_tool_call_events(
    id: str,
    output_index: int,
    text: str,
    sequence_number: int,
    request: ResponsesRequest,
    tool_name: str | None = None,
) -> tuple[str, int, int, dict]:
    event_name = "response.function_call_arguments.done"
    has_recipient = bool(tool_name)
    tool_name = tool_name or _find_tool(request.tools, text)  # pyright: ignore
    if not tool_name:
        raise ValueError("tool call is missing a tool name and could not be inferred")

    try:
        arguments_map = json.loads(text)
    except json.JSONDecodeError as e:
        arguments_map = {}

    new_args = optimize_arguments(str(tool_name), arguments_map)

    event = {
        "output_index": output_index,
        "item_id": id,
        "name": tool_name,
        "arguments": json.dumps(new_args),
    }
    resp_str = ""
    if not has_recipient:
        # MLX uses this fallback because it resolves the tool name after reading
        # the arguments. Keep that choice in the MLX implementation.
        resp_str, sequence_number = _process_output_item_added(
            "function_call", id, text, output_index, sequence_number, tool_name
        )
        output_item = OutputItemDeltaModel(
            item_name="function_call_arguments",
            item_id=id,
            index=output_index,
            delta=text,
            content_index=1,
        )
        resp_str_delta, sequence_number = _process_output_item_delta(
            output_item, sequence_number
        )
        resp_str += resp_str_delta
    resp_str_a, sequence_number = _sse(event_name, event, sequence_number)
    resp_str_b, sequence_number, output_index, item_chunk = _process_output_item_done(
        "function_call", id, text, output_index, sequence_number, tool_name
    )
    return (
        resp_str + resp_str_a + resp_str_b,
        sequence_number,
        output_index,
        item_chunk,
    )


def _find_tool(tools: list | None, arguments_str: str) -> str | None:
    try:
        arguments_map = json.loads(arguments_str)
    except json.JSONDecodeError as e:
        return None

    if not tools:
        return None

    # To increase the accuracy of the selected tool, since we
    # check the required params is a subset of model responded
    # arguments, there is chance `read` can precede write

    tool_cmd = {"cmd": "command", "rw": "read-write"}
    response_argument_keys_raw = list(arguments_map.keys())
    # map thru and change cmd to command
    response_argument_keys = [tool_cmd.get(x, x) for x in response_argument_keys_raw]

    tool_name = ""

    for tool in reversed(tools):
        name = tool.name
        params = tool.parameters

        if params is None:
            required_params = []
        else:
            required_params = params.get("required", [])

        if _is_correct_tool(required_params, response_argument_keys):
            tool_name = name
            break

    if tool_name == "":
        return None
    return tool_name


def _is_correct_tool(required_params: list, model_argument_list: list) -> bool:
    return set(required_params).issubset(model_argument_list)


# some code level determinism for tool call arguments
# - if models gives `cmd` as the key, then convert to `command`, for a bash call, as Pi expects it
# - remove `-R` from `ls` calls, to avoid sending large amt of unwanted filenames to model
# - Add a default timeout of 60s incase model doesnt run with a timeout of large timeouts
def optimize_arguments(tool_name: str, model_args: dict) -> dict:
    new_args = {("command" if k == "cmd" else k): v for k, v in model_args.items()}

    if tool_name == "bash" and new_args.get("command") is not None:
        command = str(new_args.get("command", ""))
        main_cmd = command.split(" ")[0].lower()
        if main_cmd == "ls":
            command = " ".join([word for word in command.split() if word != "-R"])
            new_args["command"] = command
        if tool_name == "bash":
            timeout = new_args.get("timeout")
            new_args["timeout"] = 60 if timeout is None else min(timeout, 60)
    return new_args
