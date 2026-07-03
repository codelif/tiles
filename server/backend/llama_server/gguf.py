"""GGUF path helpers for llama-server."""

from __future__ import annotations

from pathlib import Path


def find_gguf_file(model_path: Path) -> Path:
    """Return the main text GGUF (exclude mmproj vision encoder files)."""
    if model_path.is_file() and model_path.suffix == ".gguf":
        return model_path

    candidates = [
        p
        for p in model_path.glob("*.gguf")
        if "mmproj" not in p.name.lower() and "mtp" not in p.name.lower()
    ]
    if not candidates:
        raise FileNotFoundError(f"No .gguf file found in {model_path}")
    if len(candidates) == 1:
        return candidates[0]

    names = ", ".join(sorted(p.name for p in candidates))
    raise ValueError(
        f"Multiple .gguf files found in {model_path}: {names}. "
        "Pass an explicit .gguf path."
    )


def find_mtp_gguf_file(model_path: Path) -> Path | None:
    """Return the MTP head GGUF for speculative decoding, if present."""
    model_dir = model_path.parent if model_path.is_file() else model_path

    preferred = model_dir / "mtp-gemma-4-12b-it.gguf"
    if preferred.is_file():
        return preferred

    mtp_dir = model_dir / "MTP"
    if mtp_dir.is_dir():
        candidates = [p for p in mtp_dir.glob("*.gguf") if p.name.lower() != "readme.md"]
        for name_part in ("Q8_0", "F16", "BF16"):
            for candidate in candidates:
                if name_part in candidate.name:
                    return candidate
        if candidates:
            return sorted(candidates)[0]

    for candidate in sorted(model_dir.glob("*mtp*.gguf")):
        if "mmproj" not in candidate.name.lower():
            return candidate

    return None
