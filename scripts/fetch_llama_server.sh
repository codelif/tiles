#!/usr/bin/env bash
# Build llama-server for the Tiles experiment branch (packaging TBD).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${ROOT}/server/bin"
BUILD_DIR="${ROOT}/.cache/llama.cpp-build"
REPO_DIR="${ROOT}/.cache/llama.cpp"

mkdir -p "${OUT_DIR}"

if [[ ! -d "${REPO_DIR}/.git" ]]; then
  git clone --depth 1 https://github.com/ggml-org/llama.cpp "${REPO_DIR}"
fi

CMAKE_ARGS=(-B "${BUILD_DIR}")
if [[ "$(uname -s)" == "Linux" ]] && command -v nvcc >/dev/null 2>&1; then
  CMAKE_ARGS+=(-DGGML_CUDA=ON)
elif [[ "$(uname -s)" == "Darwin" ]]; then
  CMAKE_ARGS+=(-DGGML_METAL=ON)
fi

cmake -S "${REPO_DIR}" "${CMAKE_ARGS[@]}"
cmake --build "${BUILD_DIR}" --target llama-server -j"$(nproc 2>/dev/null || sysctl -n hw.ncpu)"

cp "${BUILD_DIR}/bin/llama-server" "${OUT_DIR}/llama-server"
cp "${BUILD_DIR}/bin/"lib*.so* "${OUT_DIR}/" 2>/dev/null || true
chmod +x "${OUT_DIR}/llama-server"
echo "Installed ${OUT_DIR}/llama-server (with shared libraries)"
