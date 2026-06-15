# Pi Hybrid Contributor Guide

This guide covers the current B7 contributor path for Pi Hybrid. It should be
read with `docs/architecture.md`, `docs/operator.md`, and
`docs/process-containment.md`; those files are the source of truth for runtime
shape, operator commands, and bounded integration-test behavior.

Pi Hybrid is a Rust-first workspace with three active Cargo members:

- `rust-core`: main binary, TUI, config, sessions, agent orchestration,
  providers, plugins, tools, and headless mode.
- `ts-bridge`: JSON-RPC protocol crate for TypeScript bridge work.
- `py-extensions`: placeholder Python extension crate.

There are still scaffolded or parallel modules. Prefer the active path named in
`docs/architecture.md` before expanding a parallel one.

## Clone-To-Run Workflow

Prerequisites:

- Rust toolchain with edition 2024 support.
- `python3` for the external pseudo-terminal TUI smoke test.
- Provider API keys only when validating a real provider config.

From a fresh checkout:

```sh
git clone <repo-url> pi-hybrid
cd pi-hybrid
cargo build --workspace
cargo test --workspace
```

For local operation without API keys, create a temporary config:

```sh
mkdir -p /tmp/pi-hybrid
cat > /tmp/pi-hybrid/config.toml <<'TOML'
provider = "deepseek"

[session]
db_path = "/tmp/pi-hybrid/sessions.db"

[bridge]
ts_bridge_path = "none"
ts_bridge_timeout = 30000

[logging]
level = "info"

[agent]
max_turns = 50
default_model = "deepseek-chat"

[providers.deepseek]
name = "DeepSeek"
api_base = "https://api.deepseek.com/v1"
api_key_env = "none"
default_model = "deepseek-chat"

[providers.glm]
name = "GLM (ZhipuAI)"
api_base = "https://open.bigmodel.cn/api/paas/v4"
api_key_env = "none"
default_model = "glm-4-flash"
TOML
```

Then validate and run:

```sh
cargo run -p rust-core -- --validate-config --config /tmp/pi-hybrid/config.toml
cargo run -p rust-core -- --config /tmp/pi-hybrid/config.toml
```

Optional contained smoke checks:

```sh
python3 tests/e2e_headless.py
python3 tests/tui_smoke.py
```

The generated default config writes `~/.pi-hybrid/sessions.db` literally. If
you use `--init-config`, replace `~` with an absolute path before validation.

## Code Conventions

- Use Rust edition 2024 across workspace crates.
- Run `cargo fmt` before committing code changes.
- Run `cargo clippy --workspace --all-targets -- -D warnings` before opening a
  PR when the change touches Rust code.
- Prefer `anyhow::Result` at runtime boundaries and specific assertions in
  tests.
- Avoid `unwrap()` and `expect()` in production code. Tests may use them when
  setup failure should abort the test clearly.
- Keep changes surgical. Do not refactor parallel agent, session, plan, or
  subagent modules unless the task requires choosing one canonical path.
- Keep feature-gated runtimes optional. TypeScript work belongs behind the
  `typescript` feature; Python work belongs behind the `python` feature.
- Keep TUI rendering non-blocking. Async work should stay behind channels or a
  Tokio task, not inside a draw function.
- Preserve explicit approval around file edits, git actions, and plan
  execution. The UI already treats plan approval as a first-class state.

## Add A Provider

There are two provider surfaces today:

- Runtime config validation in `rust-core/src/config.rs`.
- TUI provider selection metadata in `rust-core/src/agent/providers.rs`.

For a built-in provider, update both surfaces until they are unified.

1. Add the provider default to `builtin_providers()` in `config.rs`.
2. Add or update validation tests near the existing config tests.
3. Add a provider constructor in `agent/providers.rs`.
4. Register it in `ProviderRegistry::register_builtins()`.
5. Add command-palette support in `rust-core/src/tui/command_palette.rs` if the
   provider should be selectable from the UI.
6. Handle the new `Command::SelectProvider` option in `App::execute_command`
   only if a new command variant is needed.
7. Document env vars in `docs/operator.md` when the provider is intended for
   operators.

Use `api_key_env = "none"` in test configs when the provider should validate
without secrets. Do not add real API keys or `.env` files.

Suggested checks:

```sh
cargo test -p rust-core config::tests
cargo test -p rust-core agent::providers::tests
cargo run -p rust-core -- --validate-config --config /tmp/pi-hybrid/config.toml
```

## Add A Plugin

The active plugin surface is `rust-core/src/agent/plugins.rs`.

Current capabilities:

- Native Rust plugins through the `Plugin` trait.
- TypeScript and Python plugin wrappers with stubbed callback execution.
- Manifest discovery for directories containing `plugin.toml`.

To add a native plugin:

1. Implement `Plugin` directly, or use `NativePlugin::new()` for a small
   callback-backed plugin.
2. Register it with `PluginRegistry::register()` or `register_boxed()`.
3. Add focused unit tests for registration, metadata, call behavior, and error
   behavior.
4. If the plugin is user-visible in the TUI, update the plugin display path in
   `App` and `rust-core/src/tui/agent_pane.rs`.

To add a manifest-discovered plugin:

1. Create a plugin directory under the scan directory used by the caller.
2. Add `plugin.toml`:

   ```toml
   name = "example"
   description = "Example plugin"
   backend = "Native"
   version = "0.1.0"
   entry_point = "optional-entry"
   ```

3. Add tests around `PluginRegistry::add_scan_dir()` and `discover()`.

TypeScript and Python backend execution is currently scaffolded. If you wire a
real backend, keep the `Plugin` trait as the boundary and add tests that cover
backend failure without crashing the registry.

Suggested checks:

```sh
cargo test -p rust-core agent::plugins::tests
cargo test -p rust-core --features typescript
cargo test -p rust-core --features python
```

## Add A Tool

The active tool placeholder is `rust-core/src/agent/tool.rs`. The
`rust-core/src/tools/mod.rs` module is currently empty.

Current behavior:

- `Tool`, `ToolCall`, and `ToolResult` define serializable tool shapes.
- `parse_tool_calls()` extracts `tool_calls` from a JSON response.
- `execute_tool()` is a stub and does not perform real dispatch yet.

For a real tool:

1. Add a typed tool definition and schema near `agent/tool.rs`, unless the work
   first makes `tools/mod.rs` the canonical home.
2. Extend dispatch in `execute_tool()` with an explicit match on tool name.
3. Return `ToolResult { error: Some(...) }` for rejected or failed calls instead
   of panicking.
4. Put filesystem, shell, git, or network effects behind explicit approval
   checks before execution.
5. Add parse tests for valid, missing, and malformed `tool_calls`.
6. Add execution tests for success, rejection, and failure.
7. If the tool is part of plan execution, update `agent/plan.rs` or
   `agent/plan_exec.rs` on the active path being changed.

Suggested checks:

```sh
cargo test -p rust-core agent::tool
cargo test -p rust-core agent::plan
cargo test -p rust-core agent::plan_exec
```

## Add A TUI Pane

The active TUI pane enum is `rust-core/src/tui/mod.rs::Pane`. App state and
layout are owned by `rust-core/src/main.rs`.

To add a pane:

1. Add a focused module under `rust-core/src/tui/`, for example
   `my_pane.rs`.
2. Export it from `rust-core/src/tui/mod.rs`.
3. Add a variant to `Pane`, update `Pane::ALL`, `Pane::title()`, and pane-cycle
   tests.
4. Add state to `App` in `main.rs` only if the pane needs persistent state.
5. Update `layout_for()` and `draw()` in `main.rs`.
6. Update focus handling in `App::focus_at()` if the pane has its own region.
7. Add keybindings in `rust-core/src/keybindings.rs` only for pane-specific
   actions that cannot use existing navigation.
8. Add command-palette entries in `tui/command_palette.rs` if users need direct
   access.
9. Update `tests/tui_smoke.py` when the pane changes startup rendering,
   overlays, or quit behavior.

Keep render functions deterministic and cheap. They should transform current
state into Ratatui widgets, not perform file IO, bridge calls, or git work.

Suggested checks:

```sh
cargo test -p rust-core tui::tests
cargo test -p rust-core main::tests
python3 tests/tui_smoke.py
```

## Test Patterns

Use the smallest test that proves the behavior.

- Pure parsing, config, registry, and display logic: inline unit tests under
  `#[cfg(test)]`.
- Async session, subagent, or bridge behavior: `#[tokio::test]`.
- Filesystem behavior: `tempfile` and explicit path assertions.
- TUI widget state: assert state transitions and rendered text where possible.
- Headless JSON-RPC behavior: `tests/e2e_headless.py`, which starts one
  contained `rust-core --headless` child, sends JSON-RPC messages, and cleans up.
- Full terminal behavior: `tests/tui_smoke.py`, which builds `rust-core` if
  needed and drives a real pseudo-terminal.
- Config behavior: prefer temporary config files with `api_key_env = "none"`
  unless the test is specifically about missing env vars.
- Integration/e2e behavior: follow `docs/process-containment.md`; every child
  process needs a PID log, hard timeout, cleanup path, and before/after process
  evidence.

Before changing shared behavior, look for nearby tests in the same module and
extend that style. Avoid broad snapshot tests unless the UI output is otherwise
hard to verify.

## PR Checklist

Before opening a PR:

- [ ] The change is scoped to the requested behavior.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes for Rust
      changes.
- [ ] `cargo test --workspace` passes, or the PR explains the exact failing
      command and why it is unrelated.
- [ ] `python3 tests/e2e_headless.py` passes for headless/integration changes.
- [ ] `python3 tests/tui_smoke.py` passes for TUI changes.
- [ ] `cargo run -p rust-core -- --validate-config --config <test-config>`
      passes for config/provider changes.
- [ ] New provider/plugin/tool/TUI behavior has focused tests.
- [ ] Operator-facing behavior is documented in `docs/operator.md`.
- [ ] Architecture-level changes are reflected in `docs/architecture.md`.
- [ ] No secrets, API keys, `.env` files, or local absolute paths were added.
- [ ] File, git, shell, and network effects remain explicit and approval-aware.
