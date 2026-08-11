import os
from unittest.mock import patch

import httpx

from server.backend.llama_server.client import _stream_timeout


def _read_seconds(timeout: httpx.Timeout) -> float:
    # httpx stores read as a float; pull it back out for assertions.
    return float(timeout.read)


def test_default_timeout_when_env_unset():
    with patch.dict(os.environ, {}, clear=False):
        os.environ.pop("TILES_LLAMA_STREAM_READ_TIMEOUT", None)
        assert _read_seconds(_stream_timeout()) == 600.0


def test_valid_env_value_is_honored():
    with patch.dict(os.environ, {"TILES_LLAMA_STREAM_READ_TIMEOUT": "120"}):
        assert _read_seconds(_stream_timeout()) == 120.0


def test_malformed_env_falls_back_to_default():
    with patch.dict(os.environ, {"TILES_LLAMA_STREAM_READ_TIMEOUT": "abc"}):
        assert _read_seconds(_stream_timeout()) == 600.0


def test_empty_env_falls_back_to_default():
    with patch.dict(os.environ, {"TILES_LLAMA_STREAM_READ_TIMEOUT": ""}):
        assert _read_seconds(_stream_timeout()) == 600.0


def test_non_positive_env_falls_back_to_default():
    for bad in ("0", "-5", "-0.1"):
        with patch.dict(os.environ, {"TILES_LLAMA_STREAM_READ_TIMEOUT": bad}):
            assert _read_seconds(_stream_timeout()) == 600.0, f"bad={bad!r}"


def test_other_timeout_components_unchanged():
    t = _stream_timeout()
    assert float(t.connect) == 5.0
    assert float(t.write) == 30.0
    assert float(t.pool) == 5.0