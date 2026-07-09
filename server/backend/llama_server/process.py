"""Spawn and manage a llama-server subprocess."""

from __future__ import annotations

import logging
import os
import shutil
import signal
import subprocess
import sys
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


def build_llama_server_command(gguf_path: Path, llama_config: dict[str, Any]) -> list[str]:
    """Build the llama-server argv from Tiles config."""
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

    n_cpu_moe = llama_config.get("n_cpu_moe")
    if n_cpu_moe is not None:
        cmd.extend(["--n-cpu-moe", str(int(n_cpu_moe))])

    flash_attn = llama_config.get("flash_attn")
    if flash_attn:
        cmd.extend(["--flash-attn", "on"])

    no_mmap = llama_config.get("no_mmap")
    if no_mmap:
        cmd.append("--no-mmap")

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

    return cmd


def _health_url() -> str:
    return f"http://{LLAMA_SERVER_HOST}:{LLAMA_SERVER_PORT}/health"


def _llama_server_log_hint() -> str:
    log_dir = Path.cwd() / ".tiles_dev" / "tiles" / "data" / "logs"
    if not log_dir.is_dir():
        log_dir = Path.home() / ".local" / "share" / "tiles" / "data" / "logs"
    return str(log_dir / "llama-server.err.log")


def is_server_ready() -> bool:
    """True when llama-server /health reports the model is loaded."""
    try:
        response = httpx.get(_health_url(), timeout=2.0)
    except httpx.HTTPError:
        return False
    if response.status_code != 200:
        return False
    try:
        payload = response.json()
    except ValueError:
        return True
    status = payload.get("status")
    if status is None:
        return True
    return status == "ok"


def _tail_llama_server_log(max_lines: int = 8) -> str:
    log_path = Path(_llama_server_log_hint())
    if not log_path.is_file():
        return ""
    try:
        lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return ""
    if not lines:
        return ""
    return "\n".join(lines[-max_lines:])


def wait_until_ready(proc: subprocess.Popen[bytes], timeout_s: float = 600.0) -> None:
    """Poll /health until the model finishes loading. 503 while loading is normal."""
    deadline = time.time() + timeout_s
    started = time.time()
    last_progress_log = 0.0
    httpx_logger = logging.getLogger("httpx")
    previous_httpx_level = httpx_logger.level

    logger.info("Waiting for llama-server to finish loading the model...")
    httpx_logger.setLevel(logging.WARNING)
    try:
        while time.time() < deadline:
            if proc.poll() is not None:
                detail = _tail_llama_server_log()
                hint = _llama_server_log_hint()
                message = (
                    f"llama-server exited during startup (code {proc.returncode}). "
                    f"Check {hint}."
                )
                if detail:
                    message = f"{message}\n{detail}"
                raise RuntimeError(message)
            if is_server_ready():
                elapsed = time.time() - started
                logger.info("llama-server ready (%.0fs)", elapsed)
                return

            now = time.time()
            elapsed = now - started
            if elapsed >= 5 and now - last_progress_log >= 30:
                logger.info("Still loading model (%.0fs elapsed)...", elapsed)
                last_progress_log = now
            time.sleep(1.0)
    finally:
        httpx_logger.setLevel(previous_httpx_level)

    if proc.poll() is not None:
        raise RuntimeError(
            f"llama-server exited before becoming ready (code {proc.returncode}). "
            f"Check {_llama_server_log_hint()}."
        )
    raise TimeoutError(
        f"llama-server did not finish loading within {timeout_s:.0f}s. "
        f"Check {_llama_server_log_hint()}."
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
        if is_server_ready():
            return
        wait_until_ready(_process)
        return

    stop()

    gpu_layers = llama_config.get("gpu_layers")
    if gpu_layers is None:
        env_ngl = os.environ.get("TILES_GPU_LAYERS")
        gpu_layers = int(env_ngl) if env_ngl else 99
    gpu_layers = int(gpu_layers)
    context_length = int(llama_config.get("context_length") or 8192)
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

    cmd = build_llama_server_command(gguf_path, llama_config)

    logger.info("Starting llama-server: %s", " ".join(cmd))
    env = os.environ.copy()
    binary = cmd[0]
    lib_dir = str(Path(binary).resolve().parent)
    if sys.platform == "darwin":
        for var in ("DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"):
            prev = env.get(var, "")
            env[var] = f"{lib_dir}:{prev}" if prev else lib_dir
    else:
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
