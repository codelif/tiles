from pathlib import Path
from unittest.mock import Mock, patch

from server.backend.llama_server.process import (
    build_llama_server_command,
    is_server_ready,
)


def test_is_server_ready_requires_health_ok():
    loading = Mock(status_code=503)
    ready = Mock(status_code=200, json=Mock(return_value={"status": "ok"}))
    not_ready = Mock(status_code=200, json=Mock(return_value={"status": "loading model"}))

    with patch("server.backend.llama_server.process.httpx.get", return_value=loading):
        assert is_server_ready() is False
    with patch("server.backend.llama_server.process.httpx.get", return_value=not_ready):
        assert is_server_ready() is False
    with patch("server.backend.llama_server.process.httpx.get", return_value=ready):
        assert is_server_ready() is True


def test_build_llama_server_command_includes_optional_flags():
    gguf = Path("/tmp/model.gguf")
    config = {
        "context_length": 32768,
        "gpu_layers": 12,
        "offload_kqv": False,
        "batch_size": 128,
        "n_cpu_moe": 12,
        "flash_attn": True,
        "no_mmap": True,
        "mtp": False,
    }

    with patch(
        "server.backend.llama_server.process.resolve_llama_server_binary",
        return_value="/usr/bin/llama-server",
    ):
        cmd = build_llama_server_command(gguf, config)

    assert cmd[0] == "/usr/bin/llama-server"
    assert "-m" in cmd and str(gguf) in cmd
    assert "-c" in cmd and "32768" in cmd
    assert "-ngl" in cmd and "12" in cmd
    assert "--no-kv-offload" in cmd
    assert "--n-cpu-moe" in cmd and "12" in cmd
    assert "--flash-attn" in cmd and "on" in cmd
    assert "--no-mmap" in cmd
    assert "--jinja" in cmd
