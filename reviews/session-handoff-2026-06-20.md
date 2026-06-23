# Session handoff — Pi Hybrid + Needle bridge

**Date:** 2026-06-20  
**Repo:** `my-pi-hybrid` (you can leave this directory now)  
**Branch:** `force-pushed-phase-5` @ `8949459` (synced with origin)  
**Machine:** Mac mini (`sab-mini`) — Needle daemon on **M5 Air** @ `100.93.242.91:9090`

---

## Architecture (settled)

| Layer | Role | Location |
|-------|------|----------|
| **Pi** | Engine — models, tools, sessions, extensions | `pi`, `~/.pi/agent/` |
| **Needle** | Router — NL → bounded verb packet | `~/.pi/needle/`, daemon `http://100.93.242.91:9090` |
| **rust-core TUI** | Cockpit — panes, keys, display | `rust-core/src/tui/` |
| **Bridge adapter** | Glue — JSON-RPC stdio between cockpit and Needle/Pi | `scripts/needle_bridge_adapter.py` |

**Not building:** a second Pi engine in Rust.  
**Legitimate middle layer:** Needle routing only (classify; don't execute mutating verbs locally).

---

## This session — what landed

| Work | Status |
|------|--------|
| `scripts/needle_bench_prompts.json` | 45 curated prompts |
| `scripts/needle_bench.py` | Needle vs Pi-direct comparison harness |
| `scripts/needle_routing_stats.py` | Live local/forward counters |
| Adapter stats logging | `~/.pi-hybrid/needle_route_stats.json` + events JSONL |
| Read-only incident audit | `reviews/bench-auto-commits-audit.md` |

**Bench incident (important):** Full bench with Pi-direct path invoked real `pi` with `defaultProjectTrust: "always"`. Pi auto-committed and pushed **9 commits** (`1fd28f7`..`8949459`) without human review. Details in audit file. **No revert done yet.**

---

## Wiring status

| Piece | Status |
|-------|--------|
| `PI_BRIDGE_COMMAND` → `AgentConfig.bridge_command` | ✅ |
| `process_prompt` → `send_prompt` | ✅ (`acc4304`) |
| Adapter + pipe tests | ✅ |
| `cargo test -p rust-core` | ✅ 437 pass (last verified Jun 20) |
| Read path (Needle → local `read_file`) | ✅ live verified |
| Forward path (mutating → `pi --mode json`) | ⚠️ works but **dangerous** with trust=always |
| Interactive TUI E2E | ⏳ needs human terminal |
| **Timeout bug** | 🔴 documented, **not fixed** — see below |

---

## Timeout bug (blocker for real TUI use)

`BridgeClient` wraps `send_prompt` in 30s, but `Bridge::new()` uses a **2s** stdout read timeout (`json_rpc.rs:18`). Adapter allows 60s route / 300s Pi. Slow real responses fail at 2s.

**Fix:** `Bridge::with_timeout(command, BridgeClient::DEFAULT_TIMEOUT)` in `connect` / `connect_with_provider` / `reconnect`.

Reviews: `subagent-reviews/bridge-client-review.md`, `reviews/needle-adapter-code-review.md`.

---

## Open decisions (my-pi-hybrid — deferred)

1. **Keep, partial revert, or full revert** the 9 auto-commits (`ad10f65`..`8949459`) — audit: `reviews/bench-auto-commits-audit.md`
2. **Fix timeout** in rust-core before trusting TUI + adapter E2E
3. **Bench safety:** never run `needle_bench.py` without `--skip-pi` until Pi runs in a sandbox / mock / read-only mode
4. Session unification (SQLite vs Pi JSONL) — still deferred

---

## Pi / Needle work (next directory — not this repo)

When you leave `my-pi-hybrid`, the interesting surface moves to **`~/.pi/`**:

| Path | What |
|------|------|
| `~/.pi/needle/pi-route` | Needle shim (calls M5 Air daemon) |
| `~/.pi/needle/` | Router config, serve.py, model assets |
| `~/.pi/agent/sessions/` | Real Pi JSONL sessions |
| `~/.pi/agent/` | Pi extensions, skills, settings |
| M5 Air `100.93.242.91:9090` | Needle daemon (launchd) |

**Real test question (not "does Needle work"):** net ROI — does routing read/search locally save more than the Needle round-trip costs on forwarded prompts?

**Bench commands (safe):**
```bash
cd /Users/sab-mini/my-pi-hybrid
export WORKSPACE="$(pwd)"
python3 scripts/needle_bench.py --limit 5 --skip-pi   # safe: no real pi
python3 scripts/needle_bench.py --help
```

**Unsafe (do not run without sandbox):**
```bash
python3 scripts/needle_bench.py              # invokes real pi on forward prompts
```

**Adapter only (no Rust):**
```bash
export PI_BRIDGE_COMMAND="python3 $(pwd)/scripts/needle_bridge_adapter.py"
export WORKSPACE="$(pwd)"
# pipe JSON-RPC lines into the script
```

**Full TUI (after timeout fix):**
```bash
export PI_BRIDGE_COMMAND="python3 $(pwd)/scripts/needle_bridge_adapter.py"
export WORKSPACE="$(pwd)"
cargo run -p rust-core
```

**Live traffic ratio:**
```bash
cat ~/.pi-hybrid/needle_route_stats.json
```

---

## Key files (my-pi-hybrid)

```
scripts/needle_bridge_adapter.py
scripts/needle_bench.py
scripts/needle_bench_prompts.json
scripts/needle_routing_stats.py
rust-core/src/agent/mod.rs
rust-core/src/agent/bridge_client.rs
rust-core/src/bridge/json_rpc.rs
reviews/bench-auto-commits-audit.md
reviews/session-handoff-2026-06-20.md   # this file
```

---

## Verify loop (when back in repo)

```sh
cargo build -p rust-core
cargo test -p rust-core
python3 scripts/test_needle_bridge_adapter.py -v
python3 scripts/test_needle_routing_stats.py -v
```

---

## One-liner for next agent

Pi = engine (`~/.pi/`), rust-core = cockpit, Needle = router on M5 Air (`100.93.242.91:9090`). Adapter at `scripts/needle_bridge_adapter.py` speaks JSON-RPC; read-only verbs local, else `pi --mode json`. Bench harness exists but **full bench auto-pushed 9 commits** — read `reviews/bench-auto-commits-audit.md` before touching git. **Timeout bug (2s vs 30s)** blocks real TUI E2E. Next work is likely **Pi/Needle** under `~/.pi/needle/`, not rust-core unless fixing timeout or reverting commits.