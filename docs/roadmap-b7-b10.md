# B7-B10 Hardening Roadmap

**Goal:** Make Pi Hybrid boring — production-grade, documented, benchmarked, audited.

**Operating model:** Hermes orchestrates, Codex executes one checkbox per task, user approves scope changes, Codex reviews at phase boundaries.

---

## Current Status

| Phase | What | Status |
|-------|------|--------|
| B1 | Zero unwraps in production code | ✅ |
| B2 | Structured tracing (tracing crate) | ✅ |
| B3 | Config validation | ✅ |
| B4 | Graceful shutdown + signal handling | ✅ |
| B5 | 425-test suite (79% coverage) | ✅ |
| B6 | CI: ubuntu + macos, clippy, fmt, coverage | ✅ |

---

## B7: Documentation & Architecture

> *Control point. Future agents must not rediscover this context.*

- [ ] **Architecture map** (`docs/architecture.md`)
  - Data flow diagram (ascii or mermaid)
  - Crate responsibilities
  - Key design decisions and tradeoffs
  - Provider/plugin/tool extension points
- [ ] **Operator runbook** (`docs/operator.md`)
  - Install, build, run
  - Config file format, env vars, precedence
  - CLI flags reference
  - Troubleshooting: common failures + fixes
- [ ] **Contributor guide** (`docs/contributing.md`)
  - How to add a provider / plugin / tool / TUI pane
  - Code conventions (edition 2024, clippy, fmt)
  - Test patterns, mocking, fixtures
  - PR checklist
- [ ] **Public API docs** (rustdoc pass)
  - `cargo doc --document-private-items` — no missing docs warnings
  - Module-level docs for every crate and module
  - Examples for key public types
- [ ] **Clone-to-run verification**
  - `git clone && cargo build && cargo test` succeeds in <10 min
  - Runbook covers every step

**Dispatch order:** architecture → operator → contributing → rustdoc → verification

---

## B8: Cross-Platform Hardening

> *Runs on M5 and M1, in any terminal, at any size.*

- [ ] **M1 Mac Mini validation**
  - `cargo build --release` succeeds
  - Full test suite passes
  - TUI renders correctly (screenshot comparison)
- [ ] **Terminal compatibility matrix**
  - Ghostty, iTerm2, Terminal.app, Kitty, tmux, VS Code terminal
  - Verify: colors, keybindings, resize behavior
- [ ] **Terminal resize handling**
  - 80×24, 120×40, ultra-wide, ultra-narrow
  - No panics, no rendering artifacts
- [ ] **Feature flags**
  - `default = ["tui"]`, `headless` feature
  - Headless binary compiles and runs without TUI deps
- [ ] **Graceful degradation**
  - Missing Python → py-extensions disabled (no crash)
  - No git repo → git features disabled (no crash)
  - No network → headless mode works offline

**Dispatch:** parallel with B9 — independent workstreams

---

## B9: Performance Benchmarks

> *Criterion harness, CI regression gates, 6MB/500ms/25MB targets.*

- [ ] **Benchmark harness** (`benches/`)
  - Agent loop turn latency
  - TUI frame render time (target 60fps / 16ms)
  - SQLite session save/load
  - Bridge RPC round-trip
  - Subagent spawn overhead
  - Startup time (cold)
- [ ] **Baseline report** (`docs/benchmarks.md`)
  - M5 MacBook Air numbers
  - M1 Mac Mini numbers
  - Per-benchmark mean, p99, stddev
- [ ] **CI regression gates**
  - Fails if binary >6MB
  - Fails if cold startup >500ms
  - Fails if benchmark regression >10%
- [ ] **Benchmark tracking**
  - Historical data in CI artifacts
  - Trend dashboard or summary

**Dispatch:** parallel with B8 — independent workstreams

---

## B10: Security Audit

> *Last phase. Surface area is stable. Verify claims against evidence.*

- [ ] **Dependency audit**
  - `cargo audit` — zero critical/high vulns
  - `cargo deny` — license compliance, no duplicate crates
- [ ] **Secrets and credential scan**
  - No API keys, tokens, or passwords in source, config, or logs
  - No credentials in error messages or debug output
  - `.env` and auth files in `.gitignore`
- [ ] **Injection surface review**
  - SQL: verify sqlx parameterized queries (compile-time enforced — confirm)
  - Shell: no `Command::new(user_input)` without sanitization
  - JSON-RPC bridge: fuzz parser, validate message schemas
  - Prompt injection: validate user input paths
- [ ] **Rate limiting**
  - LLM API call rate limit (prevent cost explosions)
  - Configurable per-session token budgets
- [ ] **Threat model** (`docs/threat-model.md`)
  - Attack surface: TUI, RPC bridge, config, file operations, git
  - Trust boundaries: user ↔ agent, agent ↔ LLM, agent ↔ bridge, agent ↔ filesystem
  - Mitigations per boundary
- [ ] **Remediation PRs**
  - One PR per finding
  - Each with test evidence

**Dispatch:** last — after B7/B8/B9 surface is stable

---

## Success Criteria ("Boring")

- [ ] 0 `unwrap()` in production code ✅ (B1)
- [ ] 80%+ test coverage on core modules (B5: 79% — final push in B10)
- [ ] Structured logging on every operation ✅ (B2)
- [ ] Graceful shutdown saves session state ✅ (B4)
- [ ] Config validation catches all misconfigurations ✅ (B3)
- [ ] Green CI on macOS + Linux ✅ (B6)
- [ ] Architecture + operator + contributor docs live (`docs/`)
- [ ] Cross-platform: M5 + M1, all terminals, all sizes
- [ ] Benchmarks track regression, CI blocks regressions
- [ ] Security audit clean, threat model documented
- [ ] Binary <6MB, startup <500ms, RAM <25MB idle
