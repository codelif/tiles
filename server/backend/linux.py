import json
import logging
import time
import uuid
from collections.abc import AsyncGenerator

from fastapi import HTTPException
from openai_harmony import (
    Conversation,
    DeveloperContent,
    Message,
    ReasoningEffort,
    Role,
    SystemContent,
)
from openresponses_types import AssistantMessageItemParam, ReasoningEffortEnum, SystemMessageItemParam
from openresponses_types.types import (
    DeveloperMessageItemParam,
    UserMessageItemParam,
    Error,
    IncompleteDetails,
)

from ..reasoning_utils import ReasoningExtractor

from ..cache_utils import get_model_path

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
    downloadRequest,
)
from .llama_cpp_runner import LlamaRunner

logger = logging.getLogger("app")

from typing import Any, Dict, Iterator, List, Optional, Union

_model_cache: Dict[str, LlamaRunner] = {}
_default_max_tokens: Optional[int] = None  # Use dynamic model-aware limits by default
_current_model_path: Optional[str] = None
# Store generated responses for follow-up support (previous_response_id)
_responses: Dict[str, ResponsesResponse] = {}




def get_or_load_model(model_spec: str, verbose: bool = True) -> LlamaRunner:
    """Get model from cache or load it if not cached."""
    global _model_cache, _current_model_path

    try:
        model_path, model_name, commit_hash = get_model_path(model_spec)
        if not model_path.exists():
            logger.info(f"Model {model_spec} not found in cache")
            raise HTTPException(
                status_code=404, detail=f"Model {model_spec} not found in cache"
            )
    except Exception as e:
        logger.info(f"Model {model_spec} not found in: {str(e)}")
        raise HTTPException(
            status_code=404, detail=f"Model {model_spec} not found: {str(e)}"
        )

    model_path_str = str(model_path)

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

        # Load new model
        if verbose:
            print(f"Loading model: {model_name}")

        logger.info(f"Loading model: {model_name}")
        runner = LlamaRunner(model_path_str, verbose=verbose)
        runner.load_model()

        _model_cache[model_path_str] = runner
        _current_model_path = model_path_str
    else:
        logger.info(f"Model {model_name} already in memory")

    return _model_cache[model_path_str]


async def generate_chat_stream(
    messages: List[ChatMessage], request: ChatCompletionRequest
) -> AsyncGenerator[str, None]:
    """Generate streaming chat completion response."""
    raise HTTPException(
        status_code=501,
        detail="Memory mode (chat completions API) is deprecated and not supported on Linux backends. Please use /v1/responses."
    )
    yield ""


def format_chat_messages_for_runner(
    messages: List[ChatMessage],
) -> List[Dict[str, str]]:
    """Convert chat messages to format expected by LlamaRunner.

    Returns messages in dict format for the runner to apply chat templates.
    """
    return [{"role": msg.role, "content": msg.content} for msg in messages]


def _calc_usage(
    runner: LlamaRunner, input_text: Union[str, list], generated_text: str
) -> Dict[str, int]:
    """Calculate token usage using llama-cpp-python tokenizer; fall back to estimate."""
    # Convert list of dicts to string representation for rough token counting
    if isinstance(input_text, list):
        input_text_str = json.dumps(input_text)
    else:
        input_text_str = input_text
        
    try:
        if runner.model is not None:
            input_tokens = len(
                runner.model.tokenize(input_text_str.encode("utf-8"), add_bos=False)
            )
            output_tokens = len(
                runner.model.tokenize(generated_text.encode("utf-8"), add_bos=False)
            )
            return {"input_tokens": input_tokens, "output_tokens": output_tokens}
    except Exception:
        pass
    # Rough fallback
    return {
        "input_tokens": int(len(input_text_str.split()) * 1.3),
        "output_tokens": int(len(generated_text.split()) * 1.3),
    }


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
    
    # Handle single string query fallback gracefully
    if isinstance(convos, str):
        convo_list.append(Message.from_role_and_content(Role.USER, convos))
        return Conversation.from_messages(convo_list)

    for item in convos:
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
                        ),  # pyright: ignore
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


def count_tokens(text: str) -> int:
    """Rough token count estimation."""
    return int(len(text.split()) * 1.3)


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


async def generate_response_chat_stream(
    request: ResponsesRequest,
) -> AsyncGenerator[str, None]:
    """Generate streaming chat responses for OpenResponses API.

    Uses SSE event format matching the MLX backend so Pi can parse the stream.
    """
    model = request.model
    created = int(time.time())
    runner = get_or_load_model(model)
    metrics = None

    user_input_content = handle_response_input(request)

    convo = None
    if is_harmony_family(model):
        try:
            reasoning_effort = get_reasoning_effort(request.reasoning.effort)
            convo = build_harmony_conversation(
                reasoning_effort, request.input  # pyright: ignore
            )
            logger.info(f"[Harmony] Built conversation with {len(convo.messages)} messages, effort={request.reasoning.effort}")
        except Exception as e:
            logger.warning(f"[Harmony] build_harmony_conversation failed: {e}, falling back to standard path")
            convo = None

    input_tokens = _calc_usage(runner, user_input_content, "").get("input_tokens", 0)

    response_id = f"resp_{uuid.uuid4()}"
    message_id = f"msg_{uuid.uuid4()}"
    sequence_number = 0

    # response.created event
    initial_chunk = {
        "id": response_id,
        "object": "response",
        "created_at": created,
        "model": model,
        "status": "in_progress",
        "output": [
            {
                "type": "message",
                "id": message_id,
                "status": "in_progress",
                "role": "assistant",
                "content": [],
            }
        ],
        "incomplete_details": {"reason": ""},
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
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": 0,
            "total_tokens": input_tokens,
            "input_tokens_details": 0,
            "output_tokens_details": 0,
        },
        "reasoning": {"effort": "medium", "summary": "auto"},
        "top_logprobs": request.top_logprobs,
        "frequency_penalty": 0,
        "presence_penalty": 0,
        "top_p": request.top_p,
        "text": {"format": {"type": "text"}, "verbosity": "low"},
        "paralell_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        "tools": [{"name": "", "type": "function"}],
        "error": {"code": "", "message": ""},
    }
    event = {
        "type": "response.created",
        "sequence_number": sequence_number,
        "response": initial_chunk,
    }
    sequence_number += 1
    yield "event: response.created\n"
    yield f"data: {json.dumps(event)}\n\n"

    accumulated_text = ""
    answer_text = ""
    output_tokens = 0
    error = None
    incomplete_details = None
    has_answer_started: bool = False
    output_index = 0
    content_index = 0
    try:

        # Route: Harmony path (gpt-oss) or standard path
        if convo is not None:
            logger.info("[Harmony] Using generate_streaming_gpt")
            iterator = runner.generate_streaming_gpt(
                conversation=convo,
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
            )
        else:
            # Standard path — create_chat_completion with GGUF chat template
            iterator = runner.generate_streaming(
                prompt=user_input_content,
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
                use_chat_template=True,
            )
        for token in iterator:  # pyright: ignore
            if isinstance(token, GenerationMetrics):
                metrics = token
                continue

            if not isinstance(token, str):
                continue

            if "**[Answer]**" in token or has_answer_started:
                has_answer_started = True
                answer_text += token

            accumulated_text += token
            output_tokens += 1  # Each yield is one token

            # First token: emit response.output_item.added
            if sequence_number == 1:
                event_name = "response.output_item.added"
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
                    "type": event_name,
                    "sequence_number": sequence_number,
                    "output_index": output_index,
                    "item": item_chunk,
                }
                yield f"event: {event_name}\n"
                yield f"data: {json.dumps(event)}\n\n"

            # Every token: emit response.output_text.delta
            event_name = "response.output_text.delta"
            event = {
                "type": event_name,
                "sequence_number": sequence_number,
                "output_index": output_index,
                "item_id": message_id,
                "delta": token,
                "content_index": content_index,
            }

            sequence_number += 1
            content_index += 1
            yield f"event: {event_name}\n"
            yield f"data: {json.dumps(event)}\n\n"

    except Exception as e:
        import traceback
        logger.error(f"[Stream Error] {e}\n{traceback.format_exc()}")
        error = {"message": str(e), "code": "500"}
        incomplete_details = {"reason": "internal server error"}

        error_event = {"type": "error", "sequence_number": sequence_number, "error": error}
        sequence_number += 1
        yield "event: error\n"
        yield f"data: {json.dumps(error_event)}\n\n"
        return

    # Final events
    completed_at = int(time.time())

    final_chunk = {
        "id": response_id,
        "object": "response",
        "created_at": created,
        "completed_at": completed_at,
        "model": model,
        "status": "completed",
        "output": [
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
        ],
        "incomplete_details": {"reason": ""},
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
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens,
            "input_tokens_details": 0,
            "output_tokens_details": 0,
        },
        "reasoning": {"effort": "medium", "summary": "auto"},
        "top_logprobs": request.top_logprobs,
        "frequency_penalty": 0,
        "presence_penalty": 0,
        "top_p": request.top_p,
        "text": {"format": {"type": "text"}, "verbosity": "low"},
        "paralell_tool_calls": 0,
        "truncation": "disabled",
        "tool_choice": "auto",
        "tools": [{"name": "", "type": "function"}],
        "error": {"code": "", "message": ""},
    }

    # Store response for follow-ups
    metrics_obj = None
    if metrics:
        metrics_obj = {
            "ttft_ms": metrics.ttft_ms,
            "total_tokens": metrics.total_tokens,
            "tokens_per_second": metrics.tokens_per_second,
            "total_latency_s": metrics.total_latency_s,
        }
        final_chunk["metrics"] = metrics_obj

    _store_response(
        response_id=response_id,
        created=created,
        completed_at=completed_at,
        model=model,
        status="completed",
        output=final_chunk["output"],
        usage={"input_tokens": input_tokens, "output_tokens": output_tokens},
        metrics=metrics_obj,
    )

    # response.output_text.done event
    output_done_event = {
        "type": "response.output_text.done",
        "sequence_number": sequence_number,
        "item_id": message_id,
        "output_index": output_index,
        "content_index": content_index,
        "text": answer_text,
    }
    yield "event: response.output_text.done\n"
    yield f"data: {json.dumps(output_done_event)}\n\n"

    # response.completed event
    event = {
        "type": "response.completed",
        "sequence_number": sequence_number,
        "response": final_chunk,
    }
    sequence_number += 1
    yield "event: response.completed\n"
    yield f"data: {json.dumps(event)}\n\n"
    yield "data: [DONE]\n\n"


async def generate_response_chat(request: ResponsesRequest):
    """Generate chat responses for Responses API"""

    model = request.model
    response_id = f"resp-{uuid.uuid4()}"
    msg_id = f"msg_{uuid.uuid4()}"
    created = int(time.time())
    runner = get_or_load_model(model)

    user_input_content = handle_response_input(request)
    
    convo: Conversation | None = None
    if is_harmony_family(model):
        reasoning_effort = get_reasoning_effort(request.reasoning.effort)
        convo = build_harmony_conversation(
            reasoning_effort, request.input  # pyright: ignore
        )

    metrics_obj = None
    error = None
    incomplete_details = None

    try:
        generated_text = ""
        start_time = time.time()
        
        # Apply explicit chat template formatting for consistent context
        prompt_string = runner._format_conversation(
            user_input_content, use_chat_template=True
        ) if isinstance(user_input_content, list) else user_input_content
        
        if is_harmony_family(model):
            generated_text = runner.generate_batch_gpt(
                conversation=convo,
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
            )
        else:
            generated_text = runner.generate_batch(
                prompt=prompt_string,  # pyright: ignore
                max_tokens=runner.get_effective_max_tokens(request.max_output_tokens),
                temperature=request.temperature or 1,
                top_p=request.top_p or 1,
                use_chat_template=False, # already applied above
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

