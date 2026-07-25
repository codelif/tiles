import logging
import os
from pathlib import Path

import httpx
from pydantic import BaseModel

logger = logging.getLogger("app")

PORT = 6969
DAEMON_PORT = 1729
LLAMA_SERVER_HOST = os.environ.get("TILES_LLAMA_SERVER_HOST", "127.0.0.1")
LLAMA_SERVER_PORT = int(os.environ.get("TILES_LLAMA_SERVER_PORT", "18080"))
MODEL_ID = "driaforall/mem-agent"

MEMORY_PATH = os.path.expanduser("~") + "/tiles_memory"


class LlamaConfig(BaseModel):
    context_length: int | None = None
    gpu_layers: int | None = None
    offload_kqv: bool | None = None
    batch_size: int | None = None
    mtp: bool | None = None
    n_cpu_moe: int | None = None
    flash_attn: bool | None = None
    no_mmap: bool | None = None


def _config_toml_candidates() -> list[Path]:
    candidates: list[Path] = []
    if env_path := os.environ.get("TILES_CONFIG"):
        candidates.append(Path(env_path).expanduser())
    cwd = Path.cwd()
    candidates.extend(
        [
            cwd / ".tiles_dev/tiles/config.toml",
            Path.home() / ".config/tiles/config.toml",
        ]
    )
    seen: set[Path] = set()
    unique: list[Path] = []
    for path in candidates:
        resolved = path.resolve()
        if resolved not in seen:
            seen.add(resolved)
            unique.append(path)
    return unique


def _read_llama_config_from_toml() -> dict:
    import tomllib

    for path in _config_toml_candidates():
        if not path.is_file():
            continue
        try:
            with path.open("rb") as handle:
                root = tomllib.load(handle)
        except OSError:
            continue
        # First existing config file wins, even when [llama] is absent/empty.
        # Otherwise a empty .tiles_dev config would leak into ~/.config/tiles.
        llama = root.get("llama") or {}
        logger.info("Loaded llama config from %s", path)
        return LlamaConfig(**llama).model_dump(exclude_none=True)
    return {}


def get_llama_config() -> dict:
    try:
        response = httpx.get(f"http://127.0.0.1:{DAEMON_PORT}/config", timeout=2)
        response.raise_for_status()
        config = response.json()
        # Daemon answered: trust it. Empty/null [llama] means "no overrides",
        # not "keep looking in ~/.config".
        llama = config.get("llama") or {}
        logger.info("Loaded llama config from Tiles daemon")
        return LlamaConfig(**llama).model_dump(exclude_none=True)
    except httpx.HTTPError:
        pass

    return _read_llama_config_from_toml()
