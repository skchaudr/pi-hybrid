#!/usr/bin/env bash
# Semantic graphify extraction via Vertex AI OpenAI-compatible endpoint + ADC.
# See docs/graphify-vertex-adc.md for one-time gcloud setup.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REGION="${GRAPHIFY_VERTEX_REGION:-us-central1}"
MODEL="${GRAPHIFY_VERTEX_MODEL:-google/gemini-2.5-flash}"

if ! command -v gcloud >/dev/null 2>&1; then
  echo "error: gcloud not found. Install: brew install --cask gcloud-cli" >&2
  exit 1
fi

if ! command -v graphify >/dev/null 2>&1; then
  echo "error: graphify not found. Install: uv tool install \"graphifyy[gemini]\" --force" >&2
  exit 1
fi

PROJECT="${GRAPHIFY_VERTEX_PROJECT:-}"
if [[ -z "$PROJECT" ]]; then
  PROJECT="$(gcloud config get-value project 2>/dev/null || true)"
fi
if [[ -z "$PROJECT" || "$PROJECT" == "(unset)" ]]; then
  cat >&2 <<'EOF'
error: GCP project not set.

  export GRAPHIFY_VERTEX_PROJECT=your-project-id
  # or: gcloud config set project your-project-id

Then run one-time ADC setup:
  gcloud auth login
  gcloud auth application-default login
  gcloud services enable aiplatform.googleapis.com
EOF
  exit 1
fi

if ! gcloud auth application-default print-access-token >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: Application Default Credentials are not configured.

Run:
  gcloud auth login
  gcloud auth application-default login

See docs/graphify-vertex-adc.md
EOF
  exit 1
fi

TOKEN="$(gcloud auth application-default print-access-token)"
BASE_URL="https://${REGION}-aiplatform.googleapis.com/v1beta1/projects/${PROJECT}/locations/${REGION}/endpoints/openapi"

export OPENAI_API_KEY="$TOKEN"
export OPENAI_BASE_URL="$BASE_URL"

# Consume optional "--" separator before graphify args.
GRAPHIFY_ARGS=()
if [[ "${1:-}" == "--" ]]; then
  shift
fi
GRAPHIFY_ARGS=("$@")

echo "[graphify-vertex] project=${PROJECT} region=${REGION} model=${MODEL}"
echo "[graphify-vertex] base_url=${BASE_URL}"
echo "[graphify-vertex] running: graphify extract . --backend openai --model ${MODEL} ${GRAPHIFY_ARGS[*]:-}"

exec graphify extract . --backend openai --model "$MODEL" "${GRAPHIFY_ARGS[@]}"