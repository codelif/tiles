#!/usr/bin/env bash
# End-to-end smoke test for the llama-server backend (Gemma 4 12B).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export TILES_LLAMA_SERVER_BIN="${TILES_LLAMA_SERVER_BIN:-${ROOT}/server/bin/llama-server}"
export LD_LIBRARY_PATH="${ROOT}/server/bin:${LD_LIBRARY_PATH:-}"

MODEL="${TILES_SPIKE_MODEL:-unsloth/gemma-4-12b-it-GGUF}"
MODEL_CACHE="${TILES_SPIKE_MODEL_CACHE:-}"

if [[ ! -x "${TILES_LLAMA_SERVER_BIN}" ]]; then
  echo "Missing llama-server at ${TILES_LLAMA_SERVER_BIN}"
  echo "Run: ${ROOT}/scripts/fetch_llama_server.sh"
  exit 1
fi

if [[ -z "${MODEL_CACHE}" ]]; then
  for HF_DIR in \
    "${HOME}/.local/share/tiles/data/models/huggingface/hub" \
    "${ROOT}/.tiles_dev/tiles/data/models/huggingface/hub"; do
    MODEL_CACHE=$(find "${HF_DIR}" -path "*$(echo "${MODEL}" | tr '/' '--')*" -name snapshots -type d 2>/dev/null | head -1)
    if [[ -n "${MODEL_CACHE}" ]]; then
      MODEL_CACHE="$(find "${MODEL_CACHE}" -mindepth 1 -maxdepth 1 -type d | head -1)"
      break
    fi
  done
fi

if [[ -z "${MODEL_CACHE}" || ! -d "${MODEL_CACHE}" ]]; then
  echo "Model cache not found for ${MODEL}."
  echo "Download with: ./scripts/download_gemma_model.sh"
  echo "Or run tiles from source (downloads on first run):"
  echo "  ./scripts/setup_dev_layout.sh"
  echo "  cargo run -p tiles -- run"
  echo "Or set TILES_SPIKE_MODEL_CACHE to the snapshot directory."
  exit 1
fi

echo "Spike model: ${MODEL}"
echo "Model cache: ${MODEL_CACHE}"
echo "Backend: llama_server (${TILES_LLAMA_SERVER_BIN})"

cd "${ROOT}/server"
uv run uvicorn server.main:app --host 127.0.0.1 --port 6969 &
SERVER_PID=$!
trap 'kill ${SERVER_PID} 2>/dev/null || true' EXIT

for _ in $(seq 1 60); do
  if curl -sf http://127.0.0.1:6969/ping >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

echo "Loading model via /start..."
curl -sf http://127.0.0.1:6969/start \
  -H 'Content-Type: application/json' \
  -d "$(jq -n \
    --arg model "${MODEL}" \
    --arg path "${MODEL_CACHE}" \
    '{model: $model, model_cache_path: $path, memory_path: "", system_prompt: "You are Tiles."}')"

echo ""
echo "POST /v1/responses (streaming)..."
curl -sN http://127.0.0.1:6969/v1/responses \
  -H 'Content-Type: application/json' \
  -d "$(jq -n \
    --arg model "${MODEL}" \
    '{model: $model, input: [{type: "message", role: "user", content: [{type: "input_text", text: "Reply with exactly: pong"}]}], stream: true}')" \
  | head -n 50

echo ""
echo "Spike finished (first 50 SSE lines)."
