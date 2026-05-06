from types import SimpleNamespace
from unittest.mock import AsyncMock, patch

import pytest

from server.backend import mlx
from server.schemas import ResponsesRequest


class FakeTokenizer:
    def encode(self, text):
        return text.split()


class FakeRunner:
    tokenizer = FakeTokenizer()

    def get_effective_max_tokens(self, max_output_tokens):
        return max_output_tokens or 128

    def generate_streaming(self, *, prompt, max_tokens, temperature, top_p):
        yield "hello"
        yield " "
        yield "world"


async def collect_stream(request):
    chunks = []

    async for chunk in mlx.generate_response_chat_stream(request):
        chunks.append(chunk)

    return chunks


@pytest.mark.asyncio
async def test_stream_emits_created_delta_completed_and_done():
    request = ResponsesRequest(
        model="test-model",
        input="Say hello",
        stream=True,
    )

    fake_response = SimpleNamespace(status_code=200, text="/tmp/model-cache")

    with (
        patch.object(mlx.client, "get", AsyncMock(return_value=fake_response)),
        patch.object(mlx, "get_or_load_model", return_value=FakeRunner()),
        patch.object(mlx, "is_harmony_family", return_value=False),
    ):
        chunks = await collect_stream(request)

    stream_text = "".join(chunks)

    assert "event: response.created\n" in stream_text
    assert "event: response.output_item.added\n" in stream_text
    assert "event: response.output_text.delta\n" in stream_text
    assert "event: response.output_text.done\n" in stream_text
    assert "event: response.completed\n" in stream_text
    assert stream_text.endswith("data: [DONE]\n\n")
