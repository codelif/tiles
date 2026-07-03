#!/usr/bin/env bash
# Download unsloth/gemma-4-12b-it-GGUF (Q4_K_M) into the Tiles HuggingFace cache.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="unsloth/gemma-4-12b-it-GGUF"
FILE="${TILES_GEMMA_QUANT:-gemma-4-12b-it-Q4_K_M.gguf}"
MTP_FILE="${TILES_GEMMA_MTP:-mtp-gemma-4-12b-it.gguf}"

if [[ "${TILES_DEV:-1}" == "1" ]]; then
  CACHE_DIR="${ROOT}/.tiles_dev/tiles/data/models/huggingface/hub"
else
  CACHE_DIR="${HOME}/.local/share/tiles/data/models/huggingface/hub"
fi

mkdir -p "${CACHE_DIR}"

if ! command -v hf >/dev/null 2>&1; then
  echo "huggingface-cli not found. Install with:"
  echo "  uv tool install huggingface-hub"
  exit 1
fi

echo "Downloading ${REPO}/${FILE}"
echo "Cache: ${CACHE_DIR}"

hf download "${REPO}" "${FILE}" --cache-dir "${CACHE_DIR}"

echo "Downloading ${REPO}/${MTP_FILE}"
hf download "${REPO}" "${MTP_FILE}" --cache-dir "${CACHE_DIR}"

echo "Done. Snapshot path:"
find "${CACHE_DIR}" -path "*models--unsloth--gemma-4-12b-it-GGUF*" -name "${FILE}" | head -1
