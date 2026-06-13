# Pi Hybrid Public API Docs

This document defines the B7 rustdoc target for the current workspace. It is a
guide for future agents deciding what to document, what to treat as stable, and
where public Rust surfaces live today.

Read this with `docs/architecture.md` and `docs/contributing.md`. Those files
remain the source of truth for runtime shape and contributor workflow.

## Workspace API Shape

The active Cargo workspace has three members:

| Crate | Current public surface | Stability |
| --- | --- | --- |
| `rust-core` | Main binary crate. It has many `pub` items inside internal modules, but no `lib.rs` and no supported external Rust library API. | Internal unless listed below as stable operator or protocol surface. |
| `ts-bridge` | Reusable library crate for blocking JSON-RPC-over-stdio calls into a TypeScript bridge process. | Stable enough for workspace callers and examples. |
| `py-extensions` | Placeholder library crate with the default sample `add` function. | Not stable; do not build new API expectations around it yet. |

`rust-core-temp/` is a reference checkout and must not be documented as part of
the active public API.

## Stable Surfaces

Stability here means "future B7-B10 work should not rename or reshape this
without updating docs and tests." It does not imply a published semver contract.

### Operator-facing stable surfaces

- CLI flags in `rust-core/src/main.rs`: `--headless`, `--config <PATH>`,
  `--init-config`, `--validate-config`, and `--log-level <LEVEL>`.
- Config file shape in `rust-core/src/config.rs`: `PiConfig`,
  `SessionConfig`, `BridgeConfig`, `LoggingConfig`, `AgentBlock`, and
  `ProviderConfig`.
- Config helpers: `builtin_providers()`, `default_config_path()`, and
  `generate_default_toml()`.
- Environment override names documented in `docs/operator.md` and
  `rust-core/src/config.rs`.
- Headless JSON-RPC request/response envelope types in `rust-core/src/headless.rs`
  while headless mode exists as an operator entry point.

### Workspace-internal stable surfaces

These are public for module boundaries inside the binary crate and should stay
documented while they are on the active path:

- Agent channel contract in `rust-core/src/agent/mod.rs`: `AgentConfig`,
  `AgentInput`, `AgentOutput`, `SubagentInfo`, `Agent`, `AgentStatus`, and
  `agent_channels()`.
- Plan lifecycle in `rust-core/src/agent/plan.rs`: `StepStatus`, `Step`,
  `PlanStatus`, `Plan`, and `PlanManager`.
- Active SQLite persistence in `rust-core/src/session/store.rs`: `SessionInfo`
  and `SessionStore`.
- Bridge client protocol models in `rust-core/src/agent/bridge_client.rs`:
  `PromptParams`, `PromptMessage`, `PromptResponse`, `ToolCallResponse`,
  `TokenUsage`, `CompactContextParams`, `CompactContextResponse`, `TokenChunk`,
  `BridgeClient`, and `default_system_prompt()`.
- Plugin boundary in `rust-core/src/agent/plugins.rs`: `PluginBackend`,
  `PluginInfo`, `Plugin`, `NativePlugin`, `TsPlugin`, `PyPlugin`,
  `PluginRegistry`, and `PluginManifest`.
- Provider registry in `rust-core/src/agent/providers.rs`: `ProviderConfig`,
  `ProviderRegistry`, `deepseek_config()`, and `glm_config()`.
- TUI navigation and command model in `rust-core/src/tui/mod.rs`,
  `rust-core/src/keybindings.rs`, and `rust-core/src/tui/command_palette.rs`:
  `Pane`, `active_border_style()`, `Action`, `KeyBindings`, `Command`,
  `PaletteCommand`, `CommandPalette`, and `fuzzy_match()`.
- Shutdown primitives in `rust-core/src/shutdown.rs`: `CancelToken`,
  `TerminalGuard`, and `ShutdownHandler`.

### `ts-bridge` stable library surface

`ts-bridge` is the clearest public Rust API in the workspace. Treat these as
stable unless the JSON-RPC protocol changes:

- Crate root module docs in `ts-bridge/src/lib.rs`.
- `rpc` convenience module and its re-exports.
- Protocol structs: `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`,
  `SkillInfo`, `CallSkillArgs`, `SendPromptArgs`, `TokenChunk`,
  `RegisterToolArgs`, and `CallSkillResult`.
- Process client: `TsBridge`.
- `TsBridge` methods: `spawn()`, `call_method()`, `call_skill()`,
  `list_skills()`, `send_prompt()`, `register_tool()`, `is_alive()`,
  `shutdown()`, and `set_timeout()`.

## Unstable Or Scaffolded Surfaces

Document these enough for navigation, but do not describe them as stable until
the architecture chooses them as the active path:

- Parallel agent/session/plan/subagent modules:
  `agent_core.rs`, `loop.rs`, `agent/session.rs`, `plan_exec.rs`, and
  `subagent.rs`.
- `rust-core/src/tools/mod.rs`, which is currently empty while
  `rust-core/src/agent/tool.rs` holds the active placeholder.
- TypeScript and Python plugin callback execution, which is still stubbed.
- `py-extensions/src/lib.rs::add()`, which is only the default sample function.
- Headless run behavior beyond the JSON-RPC envelope, because it currently
  simulates agent work instead of sharing the TUI agent path.

## Rustdoc Organization

Use this structure for the B7 rustdoc pass:

1. Crate roots use `//!` docs that explain the crate role, active entry points,
   and which surfaces are stable.
2. Every module file uses `//!` docs before imports. The first paragraph should
   answer "why does this module exist?"
3. Public structs and enums have one-sentence summaries plus field or variant
   docs when the meaning is not obvious from the name.
4. Public functions and methods document side effects, blocking behavior, async
   behavior, filesystem/process effects, and error cases.
5. Examples belong on stable public types only. Prefer short `no_run` examples
   for process-spawning code such as `TsBridge::spawn()`.
6. Scaffolded modules should say they are scaffolded or inactive instead of
   implying they are production paths.

Avoid broad examples for private TUI rendering helpers. Unit tests remain the
better executable documentation for pane rendering and small state transitions.

## Module-level Docs Expectations

Add or keep module-level docs for:

- `rust-core/src/config.rs`
- `rust-core/src/headless.rs`
- `rust-core/src/keybindings.rs`
- `rust-core/src/shutdown.rs`
- `rust-core/src/agent/mod.rs` and every file under `rust-core/src/agent/`
- `rust-core/src/bridge/mod.rs` and `rust-core/src/bridge/json_rpc.rs`
- `rust-core/src/session/mod.rs` and `rust-core/src/session/store.rs`
- `rust-core/src/tui/mod.rs` and every file under `rust-core/src/tui/`
- `rust-core/src/tools/mod.rs`
- `ts-bridge/src/lib.rs` and `ts-bridge/src/rpc.rs`
- `py-extensions/src/lib.rs`

For a module that is intentionally empty or placeholder-only, the module docs
should say so directly and point to the active implementation, if one exists.

## Public Item Documentation Expectations

During the rustdoc pass, each stable item listed above should have:

- A direct summary sentence.
- Field or variant docs for data crossing module, process, or JSON boundaries.
- At least one example when it is an entry point users or workspace callers are
  expected to instantiate directly.
- Error documentation for functions returning `anyhow::Result`.
- Stability notes when a public item exists only for internal module boundaries.

Do not add examples that require secrets, network access, a real TypeScript
bridge, or a user-local config path.

## Verification Checklist

For a docs-only API pass:

- [ ] `docs/api.md` matches the active workspace members in root `Cargo.toml`.
- [ ] `rust-core-temp/` is excluded from active API claims.
- [ ] Stable items listed here still exist in source.
- [ ] Scaffolded and parallel modules are marked unstable or inactive.
- [ ] No secrets, `.env` files, or user-local absolute paths were added.
- [ ] `cargo doc --workspace --document-private-items --no-deps` succeeds, or
      the exact rustdoc failure is recorded for follow-up.

