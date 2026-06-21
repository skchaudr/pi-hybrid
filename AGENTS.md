## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

When the user types `/graphify`, invoke the `skill` tool with `skill: "graphify"` before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- Dirty graphify-out/ files are expected after hooks or incremental updates; dirty graph files are not a reason to skip graphify. Only skip graphify if the task is about stale or incorrect graph output, or the user explicitly says not to use it.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
- For semantic doc extraction via GCP/Vertex ADC (no AI Studio API key), see `docs/graphify-vertex-adc.md` and run `./scripts/graphify-vertex-extract.sh`.

## Build & verify (rust-core)

This repo's main crate is `rust-core/` (Cargo workspace member, Rust 2024 edition). Standard verification loop:

```
cargo build -p rust-core
cargo test -p rust-core
cargo clippy -p rust-core -- -D warnings
cargo fmt --check
```

Optional integration smoke tests (Python 3, no extra deps): `python3 tests/e2e_headless.py` (JSON-RPC over stdin/stdout via `--headless`, no terminal needed) and `python3 tests/tui_smoke.py` (PTY launch/teardown only — synthetic keystroke injection into the PTY is unreliable on macOS, so it does not send real keys).

Use `cargo test -p rust-core` as the source of truth for behavior; the TUI's `agent::spawn_agent` path is the real agent execution path. `headless.rs`'s `"run"` JSON-RPC method is currently a simulated stub (does not call `agent::spawn_agent`) — don't assume headless mode proves real agent behavior.

## Needle benchmark

The Needle routing benchmark lives at `scripts/needle_bench.py` with curated prompts in `scripts/needle_bench_prompts.json`. Use `python3 scripts/needle_bench.py --help` to inspect options; common bounded runs are `python3 scripts/needle_bench.py --limit 5` or `python3 scripts/needle_bench.py --skip-pi` when direct Pi comparison is not needed.
