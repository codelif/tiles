from pathlib import Path

from server.backend.llama_server.gguf import find_gguf_file, find_mtp_gguf_file


def test_find_gguf_excludes_mtp(tmp_path: Path):
    (tmp_path / "gemma-4-12b-it-Q4_K_M.gguf").write_bytes(b"x")
    (tmp_path / "mtp-gemma-4-12b-it.gguf").write_bytes(b"x")
    assert find_gguf_file(tmp_path).name == "gemma-4-12b-it-Q4_K_M.gguf"


def test_find_mtp_prefers_root_mtp_file(tmp_path: Path):
    (tmp_path / "mtp-gemma-4-12b-it.gguf").write_bytes(b"x")
    assert find_mtp_gguf_file(tmp_path) == tmp_path / "mtp-gemma-4-12b-it.gguf"
