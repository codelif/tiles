"""Spawn and manage a llama-server subprocess."""

from __future__ import annotations

import logging
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path
from typing import Any

import httpx

from ...config import LLAMA_SERVER_HOST, LLAMA_SERVER_PORT
from .gguf import find_mtp_gguf_file

logger = logging.getLogger("app")

_process: subprocess.Popen[bytes] | None = None
_loaded_gguf: Path | None = None
_loaded_config_key: tuple[Any, ...] | None = None


def resolve_llama_server_binary() -> str:
    env_bin = os.environ.get("TILES_LLAMA_SERVER_BIN")
    if env_bin and Path(env_bin).is_file():
        return env_bin

    server_root = Path(__file__).resolve().parents[2]
    for candidate in (
        server_root / "bin" / "llama-server",
        server_root.parent / "bin" / "llama-server",
    ):
        if candidate.is_file():
            return str(candidate)

    path_bin = shutil.which("llama-server")
    if path_bin:
        return path_bin

    raise FileNotFoundError(
        "llama-server binary not found. Set TILES_LLAMA_SERVER_BIN, place a binary at "
        "server/bin/llama-server, or install llama-server on PATH. "
        "See scripts/fetch_llama_server.sh."
    )


def _config_key(llama_config: dict[str, Any]) -> tuple[Any, ...]:
    return tuple(sorted(llama_config.items()))


def _health_url() -> str:
    return f"http://{LLAMA_SERVER_HOST}:{LLAMA_SERVER_PORT}/health"


def _models_url() -> str:
    return f"http://{LLAMA_SERVER_HOST}:{LLAMA_SERVER_PORT}/v1/models"


def wait_until_ready(proc: subprocess.Popen[bytes], timeout_s: float = 120.0) -> None:
    deadline = time.time() + timeout_s
    last_error: Exception | None = None
    while time.time() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(
                f"llama-server exited during startup (code {proc.returncode}). "
                "Check logs — on 8GB GPUs, gpu_layers=99 with a ~7GB model often OOMs."
            )
        try:
            response = httpx.get(_health_url(), timeout=2.0)
            if response.status_code == 200:
                return
            response = httpx.get(_models_url(), timeout=2.0)
            if response.status_code == 200:
                return
        except Exception as exc:  # noqa: BLE001 - poll until ready
            last_error = exc
        time.sleep(0.25)

    if proc.poll() is not None:
        raise RuntimeError(
            f"llama-server exited before becoming ready (code {proc.returncode})"
        )
    raise TimeoutError(
        f"llama-server did not become ready at {_health_url()}: {last_error}"
    )


def stop() -> None:
    global _process, _loaded_gguf, _loaded_config_key
    if _process is None:
        _loaded_gguf = None
        _loaded_config_key = None
        return

    proc = _process
    _process = None
    _loaded_gguf = None
    _loaded_config_key = None

    if proc.poll() is not None:
        return

    proc.send_signal(signal.SIGTERM)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)


def ensure_running(gguf_path: Path, llama_config: dict[str, Any]) -> None:
    """Start or restart llama-server for the given GGUF and config."""
    global _process, _loaded_gguf, _loaded_config_key

    gguf_path = gguf_path.resolve()
    key = _config_key(llama_config)
    if (
        _process is not None
        and _process.poll() is None
        and _loaded_gguf == gguf_path
        and _loaded_config_key == key
    ):
        return

    stop()

    context_length = int(llama_config.get("context_length") or 8192)
    gpu_layers = llama_config.get("gpu_layers")
    if gpu_layers is None:
        env_ngl = os.environ.get("TILES_GPU_LAYERS")
        gpu_layers = int(env_ngl) if env_ngl else 99
    gpu_layers = int(gpu_layers)
    batch_size = int(llama_config.get("batch_size") or 512)
    offload_kqv = llama_config.get("offload_kqv")
    if offload_kqv is None:
        offload_kqv = True

    if gpu_layers <= 0:
        logger.warning(
            "gpu_layers=%s — running on CPU. Set [llama].gpu_layers in config.toml "
            "or TILES_GPU_LAYERS for GPU offload.",
            gpu_layers,
        )
    else:
        logger.info(
            "llama-server GPU offload: -ngl %s, context %s, batch %s, kv_offload %s",
            gpu_layers,
            context_length,
            batch_size,
            offload_kqv,
        )

    binary = resolve_llama_server_binary()
    cmd = [
        binary,
        "--host",
        LLAMA_SERVER_HOST,
        "--port",
        str(LLAMA_SERVER_PORT),
        "-m",
        str(gguf_path),
        "-c",
        str(context_length),
        "-b",
        str(batch_size),
        "-ngl",
        str(gpu_layers),
        "--jinja",
    ]
    if offload_kqv:
        cmd.append("--kv-offload")
    else:
        cmd.append("--no-kv-offload")

    mtp_enabled = llama_config.get("mtp")
    if mtp_enabled is None:
        mtp_enabled = os.environ.get("TILES_MTP", "").lower() in ("1", "true", "yes")
    else:
        mtp_enabled = bool(mtp_enabled)

    if mtp_enabled:
        mtp_path = find_mtp_gguf_file(gguf_path)
        if mtp_path is None:
            logger.warning(
                "MTP enabled but no MTP GGUF found next to %s. "
                "Re-run model download or set mtp = false in config.",
                gguf_path,
            )
        else:
            cmd.extend(
                [
                    "--spec-type",
                    "draft-mtp",
                    "--spec-draft-model",
                    str(mtp_path),
                ]
            )
            logger.info("MTP speculative decoding enabled with %s", mtp_path)

    logger.info("Starting llama-server: %s", " ".join(cmd))
    env = os.environ.copy()
    lib_dir = str(Path(binary).resolve().parent)
    prev = env.get("LD_LIBRARY_PATH", "")
    env["LD_LIBRARY_PATH"] = f"{lib_dir}:{prev}" if prev else lib_dir
    log_dir = Path.cwd() / ".tiles_dev" / "tiles" / "data" / "logs"
    if not log_dir.is_dir():
        log_dir = Path.home() / ".local" / "share" / "tiles" / "data" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    stdout_log = open(log_dir / "llama-server.out.log", "ab")  # noqa: SIM115
    stderr_log = open(log_dir / "llama-server.err.log", "ab")  # noqa: SIM115
    _process = subprocess.Popen(
        cmd,
        stdout=stdout_log,
        stderr=stderr_log,
        env=env,
    )
    _loaded_gguf = gguf_path
    _loaded_config_key = key
    wait_until_ready(_process)
