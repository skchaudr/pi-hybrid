# Bench auto-commits audit (read-only)

Generated: 2026-06-20  
Branch: `force-pushed-phase-5`  
Baseline before incident: `ad10f65`  
Tip (local = remote): `8949459`  
Working tree: clean

---

## TL;DR

Running the full 45-prompt Needle bench with the **Pi-direct path** invoked real `pi --mode json`. With `defaultProjectTrust: "always"`, Pi edited the repo, ran subagent reviews, and **auto-committed + pushed 9 commits** to `origin/force-pushed-phase-5` between 17:18–17:53 PDT on 2026-06-20. Changes look mostly sensible and tests reportedly still pass, but nothing was human-reviewed. A real **timeout bug** was documented in the reviews but **not fixed**.

---

## Current git state

| Item | Value |
|------|-------|
| Branch | `force-pushed-phase-5` |
| HEAD | `8949459` |
| Remote | `origin/force-pushed-phase-5` @ `8949459` (in sync) |
| Uncommitted changes | none |

---

## The 9 commits (newest first)

| Commit | Time | Message | Likely bench prompt |
|--------|------|---------|---------------------|
| `8949459` | 17:53 | log process prompt dispatch | #27 + delegate reviews |
| `562266c` | 17:37 | fix operator heading typo | #34 |
| `6cd63de` | 17:34 | cover list user prompt content | #33 |
| `9e5777c` | 17:32 | add needle benchmark stub | #32 |
| `41bace4` | 17:30 | log bridge prompt latency | #31 |
| `af202bc` | 17:28 | mention needle benchmark | #30 |
| `b05d795` | 17:24 | log empty pi-route stdout | #29 |
| `f38f8f0` | 17:20 | raise needle route timeout default | #28 |
| `1fd28f7` | 17:18 | capture needle bridge routing updates | bulk / harness |

Several commits also regenerated `graphify-out/` (large incidental diff noise).

---

## What changed (excluding graphify-out)

~1,280 lines across 13 files:

**Rust**
- `rust-core/src/agent/bridge_client.rs` — `latency_ms` debug logging in `send_prompt`
- `rust-core/src/agent/mod.rs` — doc comment + `info!` dispatch logging

**Python**
- `scripts/needle_bridge_adapter.py` — timeout 45→60, empty-stdout logging, routing stats
- `scripts/needle_bench.py` — full comparison harness (new)
- `scripts/needle_bench_prompts.json` — 45 curated prompts (new)
- `scripts/needle_routing_stats.py` — live traffic counters (new)
- `scripts/test_needle_bridge_adapter.py` — list-content + empty-stdout tests
- `scripts/test_needle_routing_stats.py` — stats tests (new)

**Docs**
- `AGENTS.md` — Needle benchmark section
- `docs/needle-bench.md` — one-line stub
- `docs/operator.md` — heading typo (`Install And Build` → `Install and Build`)

**Reviews (Pi subagent output, committed as files)**
- `subagent-reviews/bridge-client-review.md`
- `reviews/needle-adapter-code-review.md`

---

## Prompt → commit mapping

| ID | Prompt (short) | Commit |
|----|----------------|--------|
| 27 | comment above process_prompt | `8949459` / `mod.rs` |
| 28 | NEEDLE_ROUTE_TIMEOUT → 60 | `f38f8f0` |
| 29 | log empty pi-route stdout | `b05d795` |
| 30 | mention needle benchmark | `af202bc` |
| 31 | log latency on send_prompt | `41bace4` |
| 32 | create needle-bench.md stub | `9e5777c` |
| 33 | unit test list content | `6cd63de` |
| 34 | fix operator.md typos | `562266c` |
| 41–42 | delegate code reviews | `8949459` |

---

## Timeout bug (found, not fixed)

**Problem:** Nested timeouts disagree.

- `Bridge::new()` in `rust-core/src/bridge/json_rpc.rs` → **2s** read timeout on stdout
- `BridgeClient::DEFAULT_TIMEOUT` in `bridge_client.rs` → **30s** wrapper on `send_prompt`
- Adapter: `NEEDLE_ROUTE_TIMEOUT` = 60s, `PI_JSON_TIMEOUT` = 300s

Any adapter response taking **>2s and <30s** fails at the inner 2s layer. Real Needle route + Pi forward calls will hit this. The new latency logging never fires on those failures.

**Smallest fix:** Use `Bridge::with_timeout(command, BridgeClient::DEFAULT_TIMEOUT)` in `connect`, `connect_with_provider`, and `reconnect`.

Details: `subagent-reviews/bridge-client-review.md` and `reviews/needle-adapter-code-review.md`.

---

## Decision options (no action taken yet)

| Option | Effect |
|--------|--------|
| Keep all 9 | Bench + logging on origin; timeout bug remains |
| Keep `1fd28f7` only, revert rest | Keep harness; drop Pi-authored edits/reviews |
| Revert all 9 → `ad10f65` | Clean slate; lose bench script too |
| Fix timeout | Independent of keep/revert; ~3 call sites in `bridge_client.rs` |

---

## Useful commands

```sh
# View this file
open reviews/bench-auto-commits-audit.md   # macOS
less reviews/bench-auto-commits-audit.md

# See all 9 commits
git log --oneline ad10f65..8949459

# Code diff only (no graphify noise)
git diff ad10f65..8949459 -- ':!graphify-out'

# Per-commit stats
git log --stat ad10f65..8949459
```