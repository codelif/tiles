import json
from types import SimpleNamespace
from unittest.mock import AsyncMock, patch

import pytest

from server.backend import mlx
from server.schemas import GenerationMetrics, ResponsesRequest, ToolCallStart


class FakeTokenizer:
    def encode(self, text):
        return text.split()


class FakeRunner:
    tokenizer = FakeTokenizer()

    def __init__(self, tokens=None, error=None):
        self.tokens = ["hello", " ", "world"] if tokens is None else tokens
        self.error = error
        self.generate_streaming_kwargs = None

    def get_effective_max_tokens(self, max_output_tokens):
        return max_output_tokens or 128

    def generate_streaming(self, *, prompt, max_tokens, temperature, top_p):
        self.generate_streaming_kwargs = {
            "prompt": prompt,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "top_p": top_p,
        }
        if self.error is not None:
            raise self.error
        yield from self.tokens

    def generate_streaming_gpt(self, *, conversation, max_tokens, temperature, top_p):
        self.generate_streaming_kwargs = {
            "conversation": conversation,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "top_p": top_p,
        }
        if self.error is not None:
            raise self.error
        yield from self.tokens


async def collect_stream(request):
    chunks = []

    async for chunk in mlx.generate_response_chat_stream(request):
        chunks.append(chunk)

    return chunks


def parse_sse_events(chunks):
    events = []

    for chunk in chunks:
        if chunk == "data: [DONE]\n\n":
            events.append({"event": "done", "data": "[DONE]"})
            continue

        event_name = None
        payload = None
        for line in chunk.strip().splitlines():
            if line.startswith("event: "):
                event_name = line.removeprefix("event: ")
            if line.startswith("data: "):
                payload = json.loads(line.removeprefix("data: "))

        events.append({"event": event_name, "data": payload})

    return events


def test_responses_request_accepts_replayed_reasoning_item():
    request = ResponsesRequest.model_validate(
        {
            "model": "mlx-community/gpt-oss-20b-MXFP4-Q4",
            "input": [
                {"role": "system", "content": "system prompt"},
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": "hello"}],
                },
                {
                    "type": "reasoning",
                    "id": "reasoning_123",
                    "status": "completed",
                    "role": "assistant",
                    "summary": [{"type": "summary_text", "text": "thought summary"}],
                    "content": [{"type": "output_text", "text": "thought summary"}],
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hi there!",
                            "annotations": [],
                        }
                    ],
                    "status": "completed",
                },
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": "hello again"}],
                },
            ],
            "stream": True,
            "prompt_cache_key": "cache-key",
            "store": False,
            "tools": [
                {
                    "type": "function",
                    "name": "read",
                    "description": "Read a file",
                    "parameters": {
                        "type": "object",
                        "required": ["path"],
                        "properties": {"path": {"type": "string"}},
                    },
                    "strict": False,
                }
            ],
        }
    )

    assert request.prompt_cache == "cache-key"
    assert mlx.handle_response_input(request) == "hello again"


@pytest.mark.asyncio
async def test_stream_emits_created_delta_completed_and_done():
    request = ResponsesRequest(
        model="test-model",
        input=[
            {"role": "system", "content": "system prompt"},
            {
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}],
            },
        ],
        stream=True,
        tools=[
            {
                "type": "function",
                "name": "read",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "required": ["path"],
                    "properties": {"path": {"type": "string"}},
                },
                "strict": False,
            }
        ],
    )

    fake_response = SimpleNamespace(status_code=200, text="/tmp/model-cache")
    runner = FakeRunner(
        tokens=[
            "**[Reasoning]**",
            "lets say hello world",
            "**[Answer]**",
            "hello",
            "world",
        ]
    )

    with (
        patch.object(mlx.client, "get", AsyncMock(return_value=fake_response)),
        patch.object(mlx, "get_or_load_model", return_value=runner),
        patch.object(mlx, "is_harmony_family", return_value=True),
    ):
        chunks = await collect_stream(request)

    stream_text = "".join(chunks)

    # print(f"stream_text\n{stream_text}")

    assert "event: response.created\n" in stream_text
    assert "event: response.output_item.added\n" in stream_text
    assert "event: response.output_text.delta\n" in stream_text
    assert "event: response.output_item.done\n" in stream_text
    assert "event: response.completed\n" in stream_text
    assert stream_text.endswith("data: [DONE]\n\n")


@pytest.mark.asyncio
async def test_stream_strips_harmony_function_namespace():
    request = ResponsesRequest(
        model="mlx-community/gpt-oss-20b-MXFP4-Q4",
        input=[
            {
                "role": "user",
                "content": [{"type": "input_text", "text": "read changelog.md"}],
            }
        ],
        stream=True,
        tools=[
            {
                "type": "function",
                "name": "read",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "required": ["path"],
                    "properties": {"path": {"type": "string"}},
                },
                "strict": False,
            }
        ],
    )
    fake_response = SimpleNamespace(status_code=200, text="/tmp/model-cache")
    runner = FakeRunner(
        tokens=[
            "**[Reasoning]**",
            "Need to read file.",
            ToolCallStart("functions.read"),
            '{"path":"changelog.md"}',
        ]
    )

    with (
        patch.object(mlx.client, "get", AsyncMock(return_value=fake_response)),
        patch.object(mlx, "get_or_load_model", return_value=runner),
        patch.object(mlx, "is_harmony_family", return_value=True),
    ):
        chunks = await collect_stream(request)

    done_event = next(
        chunk
        for chunk in chunks
        if chunk.startswith("event: response.function_call_arguments.done")
    )
    done_payload = json.loads(done_event.splitlines()[1].removeprefix("data: "))

    assert done_payload["name"] == "read"


@pytest.mark.asyncio
async def test_stream_passes_request_options_to_runner():
    request = ResponsesRequest(
        model="test-model",
        input="Use these options",
        stream=True,
        max_output_tokens=42,
        temperature=0.25,
        top_p=0.8,
    )
    fake_response = SimpleNamespace(status_code=200, text="/tmp/model-cache")
    runner = FakeRunner(tokens=["options ", "were ", "forwarded"])

    with (
        patch.object(mlx.client, "get", AsyncMock(return_value=fake_response)),
        patch.object(mlx, "get_or_load_model", return_value=runner),
        patch.object(mlx, "is_harmony_family", return_value=False),
    ):
        await collect_stream(request)

    assert runner.generate_streaming_kwargs == {
        "prompt": "Use these options",
        "max_tokens": 42,
        "temperature": 0.25,
        "top_p": 0.8,
    }


@pytest.mark.asyncio
async def test_stream_emits_failed_event_when_generation_raises():
    request = ResponsesRequest(
        model="test-model",
        input="Say hello",
        stream=True,
    )
    fake_response = SimpleNamespace(status_code=200, text="/tmp/model-cache")
    runner = FakeRunner(
        tokens=["this token should not be emitted"],
        error=RuntimeError("generation failed"),
    )

    with (
        patch.object(mlx.client, "get", AsyncMock(return_value=fake_response)),
        patch.object(mlx, "get_or_load_model", return_value=runner),
        patch.object(mlx, "is_harmony_family", return_value=False),
    ):
        events = parse_sse_events(await collect_stream(request))

    assert [event["event"] for event in events] == [
        "response.created",
        "response.failed",
    ]
    failed_response = events[-1]["data"]["response"]
    assert failed_response["status"] == "failed"
    assert failed_response["error"] == {
        "message": "generation failed",
        "code": "500",
    }
