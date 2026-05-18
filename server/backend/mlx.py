import json
import logging
from ssl import SSLCertVerificationError
import time
import uuid
from collections.abc import AsyncGenerator
from pathlib import Path
from fastapi import HTTPException, requests
from openai_harmony import (
    Conversation,
    DeveloperContent,
    Message,
    ReasoningEffort,
    Role,
    SystemContent,
)
from openresponses_types import (
    AssistantMessageItemParam,
    ReasoningEffortEnum,
    ResponseCompletedStreamingEvent,
    SystemMessageItemParam,
)
from openresponses_types.types import (
    DeveloperMessageItemParam,
    Error,
    IncompleteDetails,
    InputTokensDetails,
    OutputTokensDetails,
    Usage,
    UserMessageItemParam,
)

from ..reasoning_utils import ReasoningExtractor

from ..schemas import (
    CAssistantMessageItemParam,
    CDeveloperMessageItemParam,
    CSystemMessageItemParam,
    CUserMessageItemParam,
    ChatCompletionRequest,
    ChatMessage,
    GenerationMetrics,
    ResponsesRequest,
    ResponsesResponse,
)
from .mlx_runner import MLXRunner

import httpx

client = httpx.AsyncClient()

logger = logging.getLogger("app")

from typing import Any, Dict, Iterator, List, Optional, Tuple, Union

_model_cache: Dict[str, MLXRunner] = {}
_default_max_tokens: Optional[int] = None  # Use dynamic model-aware limits by default
_current_model_path: Optional[str] = None
# Store generated responses for follow-up support (previous_response_id)
_responses: Dict[str, ResponsesResponse] = {}


def get_or_load_model(
    model_spec: str, model_cache_path: str | None = None, verbose: bool = True
) -> MLXRunner:
    """Get model from cache or load it if not cached."""
    global _model_cache, _current_model_path
    model_name = model_spec
    if isinstance(model_cache_path, str):
        model_path_str = model_cache_path
        # Check if we need to load a different model
        if _current_model_path != model_path_str:
            # Proactively clean up any previously loaded runner to release memory
            if _model_cache:
                try:
                    for _old_runner in list(_model_cache.values()):
                        try:
                            _old_runner.cleanup()
                        except Exception:
                            pass
                finally:
                    _model_cache.clear()

            runner = MLXRunner(model_path_str, verbose=verbose)
            runner.load_model()

            _model_cache[model_path_str] = runner
            _current_model_path = model_path_str
            return runner
        else:
            logger.info(f"Model {model_name} already in memory")
            return _model_cache[_current_model_path]  # pyright: ignore
    else:
        logger.info(f"Model Path {_current_model_path} already in memory")
        return _model_cache[_current_model_path]  # pyright: ignore


def format_chat_messages_for_runner(
    messages: List[ChatMessage],
) -> List[Dict[str, str]]:
    """Convert chat messages to format expected by MLXRunner.

    Returns messages in dict format for the runner to apply chat templates.
    """
    return [{"role": msg.role, "content": msg.content} for msg in messages]


# TODO: probly will remove wrto new open-responses api
def _calc_usage(
    runner: MLXRunner, input_text: str, generated_text: str
) -> Dict[str, int]:
    """Calculate token usage using the runner tokenizer; fall back to zeros on error."""
    try:
        input_tokens = len(runner.tokenizer.encode(input_text))
        output_tokens = len(runner.tokenizer.encode(generated_text))
        return {"input_tokens": input_tokens, "output_tokens": output_tokens}
    except Exception:
        return {"input_tokens": 0, "output_tokens": 0}


def _store_response(
    response_id: str,
    created: int,
    completed_at: Optional[int],
    model: str,
    status: str,
    output: List[Dict[str, Any]],
    usage: Dict[str, int],
    error: Error | None = None,
    incomplete_details: IncompleteDetails | None = None,
    metrics: Optional[Dict[str, Any]] = None,
) -> ResponsesResponse:
    """Create a ResponsesResponse, attach metrics to metadata and store it in `_responses`."""
    resp = ResponsesResponse(
        id=response_id,
        created_at=created,
        completed_at=completed_at,
        model=model,
        status=status,
        object="response",
        error=error,
        output=output,
        usage=usage,
        incomplete_details=incomplete_details,
    )
    if metrics:
        try:
            resp.metadata["metrics"] = metrics
        except Exception:
            pass
    try:
        _responses[response_id] = resp
    except Exception:
        pass
    return resp


def count_tokens(text: str) -> int:
    """Rough token count estimation."""
    return int(len(text.split()) * 1.3)  # Approximation, convert to int


def handle_response_input(request: ResponsesRequest):
    user_msg_item = None
    user_input_content = ""

    if isinstance(request.input, str):
        user_input_content = request.input
    else:
        user_msg_item = request.input[-1]
        if isinstance(user_msg_item.content, list):
            user_input_content = user_msg_item.content[0].text
        else:
            user_input_content = user_msg_item.content.root
    return user_input_content


# DOING: Please refactor for Deus sake
# TODO: Add more tests for this api
# TODO: Consider benchmark stuff
async def generate_response_chat_stream(
    request: ResponsesRequest,
) -> AsyncGenerator[str, None]:
    """Generate streaming chat responses for OpenResponses API."""

    created = int(time.time())
    runner = await _get_runner(request.model)
    user_input_content = handle_response_input(request)

    if is_harmony_family(request.model):
        reasoning_effort = get_reasoning_effort(request.reasoning.effort)
        convo = build_harmony_conversation(
            reasoning_effort, request.input  # pyright: ignore
        )

    # TODO: DO we need it here, or in response.complete event?, so
    # we can avoid using tokenizer encoding here...
    input_tokens = len(runner.tokenizer.encode(user_input_content))  # pyright: ignore

    response_id = f"resp_{uuid.uuid4()}"
    message_id = f"msg_{uuid.uuid4()}"
    sequence_number = 0

    ## response.created envelope event ##
    initial_response = _get_response_on_create(response_id, request, created)
    resp_str, sequence_number = _sse(
        "response.created", {"response": initial_response}, sequence_number
    )
    yield resp_str
    ############

    accumulated_text = ""
    answer_text = ""
    output_tokens = 0
    error = None
    incomplete_details = None
    has_answer_started: bool = False
    # TODO: This will increment if we have multiple items than only text
    output_index = 0
    content_index = 0

    try:
        iterator: Iterator
        if is_harmony_family(request.model):
            iterator = runner.generate_streaming_gpt(
                conversation=convo,
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
            )
        else:
            iterator = runner.generate_streaming(
                prompt=user_input_content,
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
            )

        for token in iterator:
            if isinstance(token, GenerationMetrics):
                continue

            if not isinstance(token, str):
                continue

            if "**[Answer]**" in token or has_answer_started:
                has_answer_started = True
                answer_text += token

            accumulated_text += token
            output_tokens += 1  # Each yield is one token

            event_name = ""
            item_chunk = {}
            # TODO: Maybe we can avoid this?, comeback pls
            if sequence_number == 1:
                event_name = "response.output_item.added"
                # TODO: Maybe we dont need content for this event
                # yeah response.content_part.added for initializing..
                item_chunk = {
                    "type": "message",
                    "id": message_id,
                    "status": "in_progress",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": token,
                            "annotations": [],
                        }
                    ],
                }
                event = {
                    "output_index": output_index,
                    "item": item_chunk,
                }
                resp_str, sequence_number = _sse(event_name, event, sequence_number)
                yield resp_str
            event_name = "response.output_text.delta"
            event = {
                "output_index": output_index,
                # TODO: item_id is not message Id, change it
                "item_id": message_id,
                "delta": token,
                "content_index": content_index,
            }

            content_index += 1

            resp_str, sequence_number = _sse(event_name, event, sequence_number)
            yield resp_str

    except Exception as e:
        error = {"message": str(e), "code": "500"}
        incomplete_details = {"reason": "internal server error"}

        # TODO: fix error response acc to the standard
        err_response = _get_response_on_error(
            response_id, request, created, incomplete_details, error
        )
        resp_str, sequence_number = _sse(
            "response.failed", {"response": err_response}, sequence_number
        )
        yield resp_str
        return

    payload = {
        "item_id": message_id,
        "output_index": output_index,
        "content_index": content_index,
        # TODO: shouldnt be only answer ig, but check once w pi rendering
        "text": answer_text,
    }
    resp_str, sequence_number = _sse(
        "response.output_text.done", payload, sequence_number
    )
    yield resp_str

    ## Envelope, response.completed
    #
    # TODO: This shld be final output array which i think contains all output items. This is `ResponseOutputMessage`. And the output_text only answer and not thinking, is this a Pi fuckup, hmm there is reasoning item for reasoning output
    #
    output = [
        {
            "type": "message",
            "id": message_id,
            "status": "completed",
            "role": "assistant",
            "content": [
                {
                    "type": "output_text",
                    "text": answer_text,
                    "annotations": [],
                }
            ],
        }
    ]
    usage = Usage(
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        total_tokens=input_tokens + output_tokens,
        input_tokens_details=InputTokensDetails(cached_tokens=0),
        # TODO: Will revisit this when we handling output_item=reasoning for putting actual value
        output_tokens_details=OutputTokensDetails(reasoning_tokens=0),
    )
    final_response = _get_response_on_completed(
        response_id, request, created, output, usage
    )

    # TODO: Add the token usage here as its completed..
    resp_str, sequence_number = _sse(
        "response.completed", {"response": final_response}, sequence_number
    )
    yield resp_str
    ###############

    yield "data: [DONE]\n\n"


async def generate_response_chat(request: ResponsesRequest):
    """Generate chat responses for Responses API"""

    model = request.model
    response_id = f"resp-{uuid.uuid4()}"
    msg_id = f"msg_{uuid.uuid4()}"
    created = int(time.time())
    runner = get_or_load_model(model, None)

    user_input_content = ""

    user_input_content = handle_response_input(request)

    reasoning_effort = get_reasoning_effort(request.reasoning.effort)
    convo: Conversation | None = None
    if is_harmony_family(model):
        convo = build_harmony_conversation(
            reasoning_effort, request.input  # pyright: ignore
        )

    metrics_obj = None
    error = None
    incomplete_details = None

    try:
        generated_text = ""
        start_time = time.time()
        if is_harmony_family(model):
            runner.generate_batch_gpt(
                conversation=convo,  # pyright: ignore
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
                use_chat_template=True,
            )
        else:
            runner.generate_batch(
                prompt=user_input_content,  # pyright: ignore
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
                use_chat_template=True,
            )
        # Metrics for batch generation (approximate)
        generation_time = time.time() - start_time

        completed_at = int(time.time())
        status = "completed"
        error = None
        incomplete_details = None
        # Calculate token usage
        usage = _calc_usage(runner, user_input_content, generated_text)
        output_tokens = usage.get("output_tokens", 0)
        metrics_obj = {
            "ttft_ms": generation_time * 1000.0,
            "total_tokens": output_tokens,
            "tokens_per_second": (
                (output_tokens / generation_time) if generation_time > 0 else 0.0
            ),
            "total_latency_s": generation_time,
        }

    except Exception as e:
        completed_at = None
        status = "failed"
        error = {"message": str(e), "code": "500"}
        incomplete_details = {"reason": "internal server error"}
        generated_text = ""
        usage = {"input_tokens": 0, "output_tokens": 0}

    output_block = (
        [
            {
                "type": "message",
                "id": msg_id,
                "status": "completed" if status == "completed" else "failed",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": generated_text, "annotations": []}
                ],
            }
        ]
        if status == "completed"
        else []
    )

    resp = _store_response(
        response_id=response_id,
        created=created,
        completed_at=completed_at,
        model=model,
        status=status,
        output=output_block,
        usage=usage,
        error=error,
        incomplete_details=incomplete_details,
        metrics=(metrics_obj if status == "completed" else None),
    )

    return resp


def get_reasoning_effort(reasoning_effort_enum: ReasoningEffortEnum | None):
    reasoning_effort: ReasoningEffort
    match reasoning_effort_enum:
        case ReasoningEffortEnum.high:
            reasoning_effort = ReasoningEffort.HIGH
        case ReasoningEffortEnum.medium:
            reasoning_effort = ReasoningEffort.MEDIUM
        case ReasoningEffortEnum.low:
            reasoning_effort = ReasoningEffort.LOW
        case ReasoningEffortEnum.xhigh:
            reasoning_effort = ReasoningEffort.HIGH
        case _:
            raise TypeError("unknow reasoing effort")
    return reasoning_effort


def build_harmony_conversation(
    reasoning_effort: ReasoningEffort,
    convos: list,
):

    convo_list = [
        Message.from_role_and_content(
            Role.SYSTEM, SystemContent.new().with_reasoning_effort(reasoning_effort)
        )
    ]
    for item in convos:
        # print(f"ITEM {item}")
        match item:
            case CUserMessageItemParam():
                content = ""
                if isinstance(item.content, list):
                    content = item.content[0].text
                else:
                    content = item.content.root
                convo_list.append(
                    Message.from_role_and_content(Role.USER, content)  # pyright: ignore
                )
            case CDeveloperMessageItemParam():
                convo_list.append(
                    Message.from_role_and_content(
                        Role.DEVELOPER,
                        DeveloperContent.new().with_instructions(
                            item.content.root
                        ),  # pyright: ignore                    )
                    )
                )
            case CAssistantMessageItemParam():
                content = ""
                if isinstance(item.content, list):
                    content = item.content[0].text
                else:
                    content = item.content.root

                convo_list.append(
                    Message.from_role_and_content(
                        Role.ASSISTANT, content
                    )  # pyright: ignore
                )
            case CSystemMessageItemParam():
                convo_list.append(
                    Message.from_role_and_content(Role.SYSTEM, item.content.root)
                )
            case _:
                raise TypeError("unknown type")

    convo = Conversation.from_messages(convo_list)
    return convo


def is_harmony_family(model_name: str):
    return ReasoningExtractor.detect_model_type(model_name) == "gpt-oss"


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
        "paralell_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        # TODO:  will revisit on tool call impl
        "tools": [{"name": "", "type": "function"}],
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
        "paralell_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        # TODO:  will revisit on tool call impl
        "tools": [{"name": "", "type": "function"}],
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
        "paralell_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        # TODO:  will revisit on tool call impl
        "tools": [{"name": "", "type": "function"}],
        "error": error,
    }
    created_response.update(_get_commons_responses(request))
    return created_response


def _get_commons_responses(request: ResponsesRequest):
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
    }


async def _get_runner(model: str):
    # comms w tiles daemon to get correct model local path
    response = await client.get(
        f"http://127.0.0.1:1729/model-cache-path?model_name={model}"
    )

    model_cache_path = None
    if response.status_code == 200:
        model_cache_path = response.text
    else:
        raise HTTPException(status_code=500, detail="Model not found")

    runner = get_or_load_model(model, model_cache_path)
    return runner
