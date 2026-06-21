# Needle bridge adapter code review

Verdict: changes requested

## Findings

- High — `scripts/needle_bridge_adapter.py:122`, `scripts/needle_bridge_adapter.py:252`: adapter subprocess timeouts are longer than the bridge caller can wait. `NEEDLE_ROUTE_TIMEOUT` defaults to 60s and `PI_JSON_TIMEOUT` defaults to 300s, but the rust bridge path starts `Bridge::new()` from `BridgeClient::connect()` (`rust-core/src/agent/bridge_client.rs:138-140`), and `Bridge::new()` uses a 2s read timeout (`rust-core/src/bridge/json_rpc.rs:18`, `rust-core/src/bridge/json_rpc.rs:52-53`, `rust-core/src/bridge/json_rpc.rs:109-110`). There is also an outer 30s `send_prompt` timeout (`rust-core/src/agent/bridge_client.rs:252-257`). Evidence: the adapter's standalone tests pass because they call `scripts/needle_bridge_adapter.py` directly, so they do not exercise the rust host timeout. In real `rust-core BridgeClient` use, any pi-route call or Pi JSON forward taking more than the bridge read timeout will time out before the adapter can return its fallback/result. Smallest safe fix: either plumb/configure the bridge timeout through `BridgeClient`/`Bridge::with_timeout` so it is >= the adapter route + Pi budgets, or lower adapter defaults below the actual bridge timeout; add an integration test with a slow fake route/forward through `BridgeClient`.

- Medium — `scripts/needle_bridge_adapter.py:122`, `scripts/needle_bridge_adapter.py:126`, `scripts/needle_bridge_adapter.py:288`: invalid `NEEDLE_ROUTE_TIMEOUT` crashes the adapter process instead of returning a JSON-RPC error or falling back to Pi. Evidence: `timeout=int(os.environ.get("NEEDLE_ROUTE_TIMEOUT", "60"))` raises `ValueError`, the `except` only catches `OSError` and `subprocess.TimeoutExpired`, and `handle_send_prompt()` calls `call_needle_route()` outside its forwarding `try`. I verified with `NEEDLE_ROUTE_TIMEOUT=bad`; the adapter exited 1 with a traceback. Smallest safe fix: parse timeout env vars through a helper that catches `ValueError`/`TypeError`, logs a clear warning, and uses a default or returns route failure; add a focused invalid-env regression test.

- Low — `scripts/needle_bridge_adapter.py:241-245`: `PI_JSON_EXTRA_ARGS` is parsed with plain `.split()`, so quoted arguments cannot be represented correctly. Evidence: `cmd.extend(extra_args.split())` splits on whitespace without shell-style quoting. Smallest safe fix: use `shlex.split(extra_args)` and add a small command-construction/unit test if this env hook is intended to support values with spaces.

## Correct / covered

- Prompt extraction now handles list content and ignores non-text parts (`scripts/needle_bridge_adapter.py:62-80`), with a regression test (`scripts/test_needle_bridge_adapter.py:93-110`).
- Empty successful pi-route stdout is logged and falls back (`scripts/needle_bridge_adapter.py:137-144`), with a focused test (`scripts/test_needle_bridge_adapter.py:256-302`).
- Read-only `read_file` routes are workspace-bounded via `resolve_safe_path()` before local execution (`scripts/needle_bridge_adapter.py:103-164`).
- Unknown route names and explicit mutating verbs forward rather than executing locally (`scripts/needle_bridge_adapter.py:267-279`, `scripts/needle_bridge_adapter.py:290-301`).

## Tests run

- `PYTHONPYCACHEPREFIX=$(mktemp -d) python3 -m py_compile scripts/needle_bridge_adapter.py scripts/test_needle_bridge_adapter.py` — passed (`py_compile_exit=0`).
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_needle_bridge_adapter.py -v` — passed, 6 tests.
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_needle_routing_stats.py -v` — passed, 1 test.
- Manual invalid-env probe: one JSON-RPC `send_prompt` with `NEEDLE_ROUTE_TIMEOUT=bad` — reproduced traceback/process exit 1.

## Residual risks / not checked

- Did not run full `cargo test -p rust-core`; review was focused on the adapter path.
- Did not exercise the adapter through a live rust `BridgeClient` process; timeout finding is based on code inspection of the bridge call path.
- Did not contact live Needle/Pi services.
- Coverage gaps remain for `search_code`, path traversal/local error responses, invalid route JSON/non-dict route payloads, malformed JSON-RPC ids, stats logging failures, and slow route/forward behavior under the rust bridge timeout.
