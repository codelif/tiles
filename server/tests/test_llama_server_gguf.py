from pathlib import Path

from server.backend.llama_server.gguf import find_gguf_file, find_mtp_gguf_file


def test_find_gguf_excludes_mtp(tmp_path: Path):
    (tmp_path / "gemma-4-12b-it-Q4_K_M.gguf").write_bytes(b"x")
    (tmp_path / "mtp-gemma-4-12b-it.gguf").write_bytes(b"x")
    assert find_gguf_file(tmp_path).name == "gemma-4-12b-it-Q4_K_M.gguf"


def test_find_mtp_prefers_root_mtp_file(tmp_path: Path):
    (tmp_path / "mtp-gemma-4-12b-it.gguf").write_bytes(b"x")
    assert find_mtp_gguf_file(tmp_path) == tmp_path / "mtp-gemma-4-12b-it.gguf"


def test_find_mtp_prefers_Q8_0_in_MTP_dir(tmp_path: Path):
    mtp_dir = tmp_path / "MTP"
    mtp_dir.mkdir()
    (mtp_dir / "head-F16.gguf").write_bytes(b"x")
    (mtp_dir / "head-Q8_0.gguf").write_bytes(b"x")
    assert find_mtp_gguf_file(tmp_path) == mtp_dir / "head-Q8_0.gguf"


def test_find_mtp_falls_back_through_quantizations(tmp_path: Path):
    mtp_dir = tmp_path / "MTP"
    mtp_dir.mkdir()
    (mtp_dir / "head-BF16.gguf").write_bytes(b"x")
    (mtp_dir / "head-F16.gguf").write_bytes(b"x")
    # F16 comes before BF16 in the precedence list.
    assert find_mtp_gguf_file(tmp_path) == mtp_dir / "head-F16.gguf"


def test_find_mtp_falls_back_to_sorted_first(tmp_path: Path):
    # No recognized quant tag in the name; the first sorted file is returned.
    mtp_dir = tmp_path / "MTP"
    mtp_dir.mkdir()
    (mtp_dir / "zzz.gguf").write_bytes(b"x")
    (mtp_dir / "aaa.gguf").write_bytes(b"x")
    assert find_mtp_gguf_file(tmp_path) == mtp_dir / "aaa.gguf"


def test_find_mtp_glob_skips_mmproj(tmp_path: Path):
    (tmp_path / "mtp-head.gguf").write_bytes(b"x")
    (tmp_path / "mmproj-mtp.gguf").write_bytes(b"x")
    assert find_mtp_gguf_file(tmp_path) == tmp_path / "mtp-head.gguf"


def test_find_mtp_returns_none_when_absent(tmp_path: Path):
    (tmp_path / "model.gguf").write_bytes(b"x")
    assert find_mtp_gguf_file(tmp_path) is None
