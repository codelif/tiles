"""HTTP client for llama-server OpenAI-compatible chat completions."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator
from typing import Any

import httpx

from ...config import LLAMA_SERVER_HOST, LLAMA_SERVER_PORT


def chat_completions_url() -> str:
    return f"http://{LLAMA_SERVER_HOST}:{LLAMA_SERVER_PORT}/v1/chat/completions"


async def stream_chat_completions(body: dict[str, Any]) -> AsyncIterator[dict[str, Any]]:
    async with httpx.AsyncClient(timeout=None) as client:
        async with client.stream(
            "POST",
            chat_completions_url(),
            json=body,
            headers={"Accept": "text/event-stream"},
        ) as response:
            response.raise_for_status()
            async for line in response.aiter_lines():
                if not line or not line.startswith("data: "):
                    continue
                payload = line.removeprefix("data: ").strip()
                if payload == "[DONE]":
                    return
                yield json.loads(payload)
