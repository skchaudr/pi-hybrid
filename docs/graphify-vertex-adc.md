# Graphify semantic extraction via Vertex AI + ADC

Use Google Cloud **Application Default Credentials (ADC)** instead of a
`GEMINI_API_KEY` when your org routes Gemini access through Vertex AI.

Graphify 0.8.42 does not ship a native `--backend vertex` yet
([graphify#974](https://github.com/safishamsi/graphify/issues/974)). This repo
bridges that gap by pointing graphify's OpenAI-compatible client at Vertex's
OpenAI-compatible endpoint and feeding it a short-lived ADC bearer token.

AST-only extraction (`graphify update .`) never needs Google auth. Use this
path only when you want **semantic** edges from docs/markdown.

## Prerequisites

- `graphify` with Gemini extras: `uv tool install "graphifyy[gemini]" --force`
- `gcloud` CLI (macOS: `brew install --cask gcloud-cli`)
- A GCP project with **Vertex AI API** enabled
- IAM on your user or service account, e.g. `roles/aiplatform.user`

## One-time setup (local)

```sh
# 1) Authenticate as the account with general GCP credits (not sbkchaudry GenAI-only)
gcloud auth login sb.info.you@gmail.com
gcloud auth application-default login

# 2) Pin account + project (needle-pi = display name "needle")
gcloud config set account sb.info.you@gmail.com
gcloud config set project needle-pi
gcloud auth application-default set-quota-project needle-pi
export GRAPHIFY_VERTEX_PROJECT=needle-pi   # optional if gcloud config is set
export GRAPHIFY_VERTEX_REGION=us-central1  # optional; default us-central1

# 3) Enable Vertex AI (once per project — already on for needle-pi)
gcloud services enable aiplatform.googleapis.com --project="$GRAPHIFY_VERTEX_PROJECT"
```

**Account note:** `sbkchaudry@gmail.com` has isolated GenAI promo credits that
most tooling never bills. Use `sb.info.you@gmail.com` + `needle-pi` so Vertex
charges your general GCP credits.

Other projects on this account (if you need them later):

| Project ID | Display name |
|------------|--------------|
| `needle-pi` | needle |
| `conversations-needle-project` | Conversations-Needle-Project |
| `gen-lang-client-0561215370` | Default Gemini Project |

Verify ADC works:

```sh
gcloud auth application-default print-access-token >/dev/null \
  && echo "ADC OK"
```

Tokens expire after ~1 hour. Re-run the extract script; it refreshes the token
each time.

## Run semantic extraction (this repo)

From the workspace root:

```sh
# Full pipeline: AST + semantic docs + clustering
./scripts/graphify-vertex-extract.sh

# AST-only refresh after code edits (no Google auth, no cost)
graphify update .
graphify cluster-only . --no-label --no-viz
```

### What the script does

1. Checks `gcloud` and ADC are available
2. Requires `GRAPHIFY_VERTEX_PROJECT` (or `gcloud config get-value project`)
3. Exports a fresh bearer token:
   `gcloud auth application-default print-access-token`
4. Sets Vertex OpenAI-compatible env vars (per [Google's auth guide](https://cloud.google.com/vertex-ai/generative-ai/docs/migrate/openai/auth-and-credentials)):
   - `OPENAI_API_KEY` = ADC token
   - `OPENAI_BASE_URL` = `https://{region}-aiplatform.googleapis.com/v1beta1/projects/{project}/locations/{region}/endpoints/openapi`
5. Runs `graphify extract . --backend openai --model google/gemini-2.5-flash`

Override model or region:

```sh
GRAPHIFY_VERTEX_MODEL=google/gemini-2.0-flash-001 \
GRAPHIFY_VERTEX_REGION=europe-west1 \
./scripts/graphify-vertex-extract.sh
```

Pass extra graphify flags after `--`:

```sh
./scripts/graphify-vertex-extract.sh -- --no-cluster
```

## CI / keyless (Workload Identity Federation)

On GCP runners (Cloud Build, GKE Workload Identity, GitHub Actions → GCP OIDC),
ADC is injected by the platform — no JSON key file.

```sh
export GRAPHIFY_VERTEX_PROJECT=YOUR_PROJECT_ID
export GRAPHIFY_VERTEX_REGION=us-central1
./scripts/graphify-vertex-extract.sh
```

Ensure the job's service account has `roles/aiplatform.user` (or tighter custom
role) on the project.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `Your default credentials were not found` | Run `gcloud auth application-default login` |
| `No credentialed accounts` | Run `gcloud auth login` |
| HTTP 403 / permission denied | Enable `aiplatform.googleapis.com`; grant `aiplatform.user` |
| HTTP 401 after ~1h | Re-run script (token refresh) |
| `Please pass a valid API key` on `--backend gemini` | Wrong backend — use this Vertex/ADC path, not AI Studio keys |
| `requires the openai package` | `uv tool install "graphifyy[gemini]" --force` |

## Agent handoff (save tokens)

After a successful extract (or AST-only `graphify update` + `cluster-only`):

- `graphify-out/graph.json` — queryable graph
- `graphify-out/GRAPH_REPORT.md` — navigation summary

Agents should prefer:

```sh
graphify query "how does the agent loop work?"
graphify path "App" "BridgeClient"
graphify explain "PluginRegistry"
```

See `AGENTS.md` and `.cursor/rules/graphify.mdc` for always-on agent rules.

Commit handoff artifacts (cache stays gitignored):

```sh
git add graphify-out/graph.json graphify-out/GRAPH_REPORT.md graphify-out/manifest.json
```