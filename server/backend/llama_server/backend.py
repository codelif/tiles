"""Unified llama-server backend (Linux + macOS experiment)."""

from __future__ import annotations

import logging
from collections.abc import AsyncGenerator
from pathlib import Path
from typing import Any

import httpx
from fastapi import HTTPException

from ...config import get_llama_config
from ...schemas import ResponsesRequest
from .gguf import find_gguf_file
from . import process
from .openresponses_adapter import (
    generate_response_chat_stream as _generate_response_chat_stream,
)

logger = logging.getLogger("app")

_current_model_path: str | None = None
_current_llama_config: dict[str, Any] | None = None


class LlamaServerRunner:
    """Placeholder runner kept for API compatibility with older backends."""

    def __init__(self, model_path: str, llama_config: dict[str, Any]):
        self.model_path = model_path
        self.llama_config = llama_config


def _resolve_model_path(model_spec: str, model_cache_path: str | None) -> Path:
    if isinstance(model_cache_path, str) and model_cache_path:
        return Path(model_cache_path)

    response = httpx.get(
        f"http://127.0.0.1:1729/model-cache-path?model_name={model_spec}",
        timeout=10,
    )
    if response.status_code != 200:
        raise HTTPException(
            status_code=404,
            detail=f"Model {model_spec} not found in cache daemon",
        )
    return Path(response.text)


def get_or_load_model(
    model_spec: str,
    model_cache_path: str | None = None,
    verbose: bool = True,
) -> LlamaServerRunner:
    global _current_model_path, _current_llama_config

    llama_config = get_llama_config()
    if (
        model_cache_path is None
        and _current_model_path is not None
        and _current_llama_config == llama_config
    ):
        logger.info("Model %s already loaded in llama-server", model_spec)
        return LlamaServerRunner(_current_model_path, llama_config)

    try:
        model_dir = _resolve_model_path(model_spec, model_cache_path)
        if not model_dir.exists():
            raise HTTPException(
                status_code=404,
                detail=f"Model {model_spec} not found at {model_dir}",
            )
        gguf_path = find_gguf_file(model_dir)
    except HTTPException:
        raise
    except Exception as exc:
        raise HTTPException(
            status_code=404,
            detail=f"Model {model_spec} not found: {exc}",
        ) from exc

    if verbose:
        print(f"Loading model via llama-server: {model_spec}")
        print(f"Using GGUF file: {gguf_path}")

    logger.info("Loading model via llama-server: %s (%s)", model_spec, gguf_path)
    process.ensure_running(gguf_path, llama_config)

    _current_model_path = str(model_dir)
    _current_llama_config = llama_config
    return LlamaServerRunner(str(model_dir), llama_config)


async def generate_response_chat_stream(
    request: ResponsesRequest,
) -> AsyncGenerator[str, None]:
    get_or_load_model(request.model)
    llama_config = get_llama_config()
    async for chunk in _generate_response_chat_stream(request, llama_config):
        yield chunk


async def generate_response_chat(request: ResponsesRequest):
    chunks: list[str] = []
    async for chunk in generate_response_chat_stream(request):
        chunks.append(chunk)
    return "".join(chunks)
