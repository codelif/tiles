from unittest.mock import Mock, patch

from server.config import get_llama_config


def test_get_llama_config_handles_null_llama_config():
    response = Mock()
    response.json.return_value = {"llama": None}

    # When the daemon has no llama config, get_llama_config falls back to
    # config.toml. Isolate the test from any on-disk config so it stays hermetic.
    with (
        patch("server.config.httpx.get", return_value=response),
        patch("server.config._read_llama_config_from_toml", return_value={}),
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
        }
    }

    with patch("server.config.httpx.get", return_value=response):
        assert get_llama_config() == {
            "context_length": 20000,
            "gpu_layers": 12,
            "offload_kqv": False,
            "batch_size": 128,
        }
