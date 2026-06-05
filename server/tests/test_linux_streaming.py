import json
from unittest.mock import patch

import pytest

from server.backend import linux
from server.schemas import ResponsesRequest, ToolCallStart


class FakeModel:
    def tokenize(self, text, add_bos=False):
        return text.split()


class FakeRunner:
    model = FakeModel()
    tool_name = "read"

    def get_effective_max_tokens(self, max_output_tokens):
        return max_output_tokens or 128

    def generate_streaming_gpt(self, **kwargs):
        yield "**[Reasoning]**\n\n"
        yield "Need to read file."
        yield ToolCallStart(self.tool_name)
        yield '{"path":"changelog.md"}'


def make_request():
    return ResponsesRequest.model_validate(
        {
            "model": "unsloth/gpt-oss-20b-GGUF",
            "input": [
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read changelog.md"}],
                }
            ],
            "stream": True,
            "tools": [
                {
                    "type": "function",
                    "name": "read",
                    "parameters": {
                        "type": "object",
                        "required": ["path"],
                        "properties": {"path": {"type": "string"}},
                    },
                }
            ],
        }
    )


@pytest.mark.asyncio
async def test_gpt_streaming_emits_completed_tool_call_and_done():
    request = make_request()

    with patch.object(linux, "get_or_load_model", return_value=FakeRunner()):
        chunks = [
            chunk async for chunk in linux.generate_response_chat_stream(request)
        ]

    stream = "".join(chunks)
    events = [
        json.loads(block.splitlines()[1].removeprefix("data: "))
        for block in stream.split("\n\n")
        if block.startswith("event: ")
    ]
    done_event = next(
        chunk
        for chunk in chunks
        if chunk.startswith("event: response.function_call_arguments.done")
    )
    done_payload = json.loads(done_event.splitlines()[1].removeprefix("data: "))

    assert done_payload["name"] == "read"
    assert json.loads(done_payload["arguments"]) == {"path": "changelog.md"}
    assert "event: response.output_item.done\n" in stream
    assert chunks[-1] == "data: [DONE]\n\n"

    function_call_items = [
        event["item"]
        for event in events
        if event["type"] in {"response.output_item.added", "response.output_item.done"}
        and event["item"]["type"] == "function_call"
    ]
    assert len(function_call_items) == 2
    assert function_call_items[0]["name"] == "read"
    assert function_call_items[0]["arguments"] == ""
    assert function_call_items[0]["call_id"] == function_call_items[1]["call_id"]


@pytest.mark.asyncio
async def test_gpt_streaming_strips_harmony_function_namespace():
    request = make_request()
    runner = FakeRunner()
    runner.tool_name = "functions.read"

    with patch.object(linux, "get_or_load_model", return_value=runner):
        chunks = [
            chunk async for chunk in linux.generate_response_chat_stream(request)
        ]

    done_event = next(
        chunk
        for chunk in chunks
        if chunk.startswith("event: response.function_call_arguments.done")
    )
    done_payload = json.loads(done_event.splitlines()[1].removeprefix("data: "))

    assert done_payload["name"] == "read"


@pytest.mark.asyncio
async def test_gpt_streaming_infers_tool_when_commentary_has_no_recipient():
    request = make_request()
    runner = FakeRunner()
    runner.tool_name = ""

    with patch.object(linux, "get_or_load_model", return_value=runner):
        chunks = [
            chunk async for chunk in linux.generate_response_chat_stream(request)
        ]

    stream = "".join(chunks)
    events = [
        json.loads(block.splitlines()[1].removeprefix("data: "))
        for block in stream.split("\n\n")
        if block.startswith("event: ")
    ]
    function_call_added = next(
        event
        for event in events
        if event["type"] == "response.output_item.added"
        and event["item"]["type"] == "function_call"
    )

    assert function_call_added["item"]["name"] == "read"
    assert "event: response.function_call_arguments.delta\n" in stream
    assert "event: response.failed\n" not in stream
