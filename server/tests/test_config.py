from unittest.mock import Mock, patch

import httpx

from server.config import get_llama_config


def test_get_llama_config_handles_null_llama_config():
    response = Mock()
    response.json.return_value = {"llama": None}

    # Daemon reachable with null/empty llama means "no overrides" — do not
    # fall through to ~/.config/tiles/config.toml.
    with (
        patch("server.config.httpx.get", return_value=response),
        patch(
            "server.config._read_llama_config_from_toml",
            side_effect=AssertionError("must not fall back when daemon answers"),
        ),
    ):
        assert get_llama_config() == {}


def test_get_llama_config_returns_present_llama_values():
    response = Mock()
    response.json.return_value = {
        "llama": {
            "context_length": 20000,
            "gpu_layers": 12,
            "offload_kqv": False,
            "batch_size": 128,
            "n_cpu_moe": 12,
            "flash_attn": True,
            "no_mmap": True,
        }
    }

    with patch("server.config.httpx.get", return_value=response):
        assert get_llama_config() == {
            "context_length": 20000,
            "gpu_layers": 12,
            "offload_kqv": False,
            "batch_size": 128,
            "n_cpu_moe": 12,
            "flash_attn": True,
            "no_mmap": True,
        }


def test_get_llama_config_falls_back_to_toml_when_daemon_down():
    with (
        patch("server.config.httpx.get", side_effect=httpx.ConnectError("down")),
        patch(
            "server.config._read_llama_config_from_toml",
            return_value={"gpu_layers": 4},
        ),
    ):
        assert get_llama_config() == {"gpu_layers": 4}
