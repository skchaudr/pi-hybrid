# Pi Hybrid Operator Runbook

This runbook covers the current B7 operator surface: install, build, run,
configuration, environment overrides, CLI flags, and common failure recovery.
It describes the code as it exists now. Some agent, provider, bridge, and
headless paths are scaffolded and are called out where that affects operation.

## Quick Start

Prerequisites:

- Rust toolchain with edition 2024 support.
- `python3` only if you want to run the external TUI smoke test.
- Optional provider API keys if using the built-in provider config:
  `PI_DEEPSEEK_KEY` and `PI_GLM_KEY`.

From the workspace root:

```sh
cargo build --workspace
cargo test --workspace
cargo run -p rust-core -- --init-config
```

The default config is written to `~/.pi-hybrid/config.toml`. Review it before
running the TUI. The generated config references provider API key environment
variables, so validation fails until those variables are set or the providers
are changed to `api_key_env = "none"` for local/test operation. It also writes
`~/.pi-hybrid/sessions.db` literally; replace `~` with an absolute path such as
`/Users/sab-mini/.pi-hybrid/sessions.db` because config paths are not shell
expanded.

Validate the config:

```sh
cargo run -p rust-core -- --validate-config
```

Run the TUI:

```sh
cargo run -p rust-core
```

Quit with `q`.

## Install And Build

This repository is a Cargo workspace with these active members:

- `rust-core`: main binary, TUI, config, sessions, agent orchestration, and
  headless entry point.
- `ts-bridge`: JSON-RPC bridge protocol crate.
- `py-extensions`: placeholder Python extension crate.

Useful build commands:

```sh
cargo build --workspace
cargo build --release -p rust-core
cargo test --workspace
python3 tests/tui_smoke.py
```

Optional feature builds:

```sh
cargo build -p rust-core --features typescript
cargo build -p rust-core --features python
```

Default `rust-core` builds do not enable `typescript` or `python`.

## Run Modes

### TUI Mode

```sh
cargo run -p rust-core
```

The TUI opens an alternate-screen terminal UI with Files, Editor, Agents, and
Plan/Approval panes. It uses the current working directory as the workspace
root for file tree and git status.

Common controls:

| Key | Action |
| --- | --- |
| `q` | Quit |
| `Tab` | Cycle panes |
| `F2`, `F3`, `F4` | Focus Editor, Agents, Plan |
| `j`/`k`, arrows | Move selection or scroll |
| `gg`, `G` | Top, bottom |
| `Ctrl+d`, `Ctrl+u` | Page down, page up |
| `?` or `F1` | Help popup |
| `Ctrl+P` or `Cmd+P` | Command palette |
| `F5` | Toggle file tree |
| `F6` | Toggle agent pane |
| `F7` | Toggle dark/light mode |
| `F8` | Spawn subagent prompt |
| `F9` | Show plugins |
| `Ctrl+F10` | Toggle git status display |
| `a`/`r`/`e` in Plan pane | Approve, reject, edit plan |

Command palette entries include opening files, switching panes, toggles,
spawning a subagent, running the bridge test, showing plugins, selecting the
DeepSeek or GLM provider, rendering Mermaid diagrams, and quitting.

### Headless Mode

```sh
cargo run -p rust-core -- --headless
```

Headless mode starts a JSON-RPC server over stdin/stdout. On startup it emits a
`ready` notification:

```json
{"jsonrpc":"2.0","method":"ready","params":{"version":"0.1.0","mode":"headless"}}
```

Supported methods:

| Method | Params | Notes |
| --- | --- | --- |
| `run` | `prompt`, optional `provider`, `model`, `max_turns` | Creates a simulated session. |
| `status` | optional `session_id` | Returns active or requested session status. |
| `cancel` | optional `session_id` | Marks the session cancelled. |
| `list_sessions` | none | Lists in-memory headless sessions. |
| `resume` | `session_id` | Selects an existing in-memory session. |
| `shutdown` | none | Returns success and exits the server loop. |

Example:

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"status","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}' \
  | cargo run -p rust-core -- --headless
```

Headless `run` currently simulates agent work. It does not yet route through
the same agent/session path as the TUI.

## Configuration

Default path:

```text
~/.pi-hybrid/config.toml
```

Use `--config <PATH>` to load or initialize a different file.

Generate a commented default:

```sh
cargo run -p rust-core -- --init-config
cargo run -p rust-core -- --init-config --config /tmp/pi-hybrid.toml
```

Validate:

```sh
cargo run -p rust-core -- --validate-config
cargo run -p rust-core -- --validate-config --config /tmp/pi-hybrid.toml
```

Minimal local/test config that validates without API keys:

```toml
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
```

Full config shape:

| TOML key | Meaning |
| --- | --- |
| `provider` | Default provider name. Must match a key under `[providers]`. |
| `session.db_path` | SQLite session database path. Parent must exist or be creatable. |
| `bridge.ts_bridge_path` | TypeScript bridge executable path. Empty or `none` falls back to `PI_BRIDGE_COMMAND`; with that env var unset, the derived bridge command is empty. |
| `bridge.ts_bridge_timeout` | Bridge call timeout in milliseconds. |
| `logging.level` | `trace`, `debug`, `info`, `warn`, or `error`. |
| `agent.max_turns` | Agent turn limit. Must be `1..=500`. |
| `agent.default_model` | Default model string passed into agent config. |
| `providers.<name>.name` | Human-readable provider name. |
| `providers.<name>.api_base` | Provider API base URL. |
| `providers.<name>.api_key_env` | Env var containing API key, or `none` for no key. |
| `providers.<name>.default_model` | Provider default model. |

Config precedence:

1. Built-in defaults.
2. TOML config file from `~/.pi-hybrid/config.toml` or `--config <PATH>`.
3. Environment overrides.
4. `--log-level <LEVEL>` for tracing output only.

Validation runs after the file and environment are merged.

## Environment Variables

Config loader overrides:

| Variable | Overrides |
| --- | --- |
| `PI_PROVIDER` | `provider` |
| `PI_SESSION_DB` | `session.db_path` |
| `PI_MAX_TURNS` | `agent.max_turns` when it parses as an integer |
| `PI_LOG_LEVEL` | `logging.level` |
| `PI_DEEPSEEK_KEY` | Built-in config value for `providers.deepseek` |
| `PI_GLM_KEY` | Built-in config value for `providers.glm` |
| `OLLAMA_HOST` | `providers.ollama.api_base` host (default `127.0.0.1:9000`) |

Local Ollama models (placeholder provider `ollama`, no API key):

| Model | Notes |
| --- | --- |
| `qwen2.5-coder:7b` | Default; good on Mac mini and MacBook Air |
| `qwen2.5-coder:14b` | Heavier; Mac mini primary |
| Gemma 12B (Google Eloquence) | Not Ollama; add a provider when wired |

```sh
export OLLAMA_HOST=127.0.0.1:9000
ollama pull qwen2.5-coder:7b
PI_PROVIDER=ollama cargo run -p rust-core -- --validate-config
```

Bridge command fallback:

| Variable | Used when |
| --- | --- |
| `PI_BRIDGE_COMMAND` | `bridge.ts_bridge_path` is empty or `none` when deriving agent config. |

Tracing:

| Variable | Used when |
| --- | --- |
| `RUST_LOG` | Overrides the configured tracing filter through `tracing_subscriber::EnvFilter`. |

Current UI provider registry note: the TUI provider selector uses a separate
registry whose built-in provider metadata names `DEEPSEEK_API_KEY` and
`GLM_API_KEY`. Startup validation is governed by `config.rs`, so use
`PI_DEEPSEEK_KEY` and `PI_GLM_KEY` for the current config path unless you are
working directly on the scaffolded provider registry.

## CLI Flags

Flags are parsed manually in `rust-core/src/main.rs`; there is no generated
`--help` output yet.

| Flag | Effect |
| --- | --- |
| `--config <PATH>` | Use a config file path instead of `~/.pi-hybrid/config.toml`. |
| `--init-config` | Write the default commented TOML config and exit. Creates the parent directory if needed. |
| `--validate-config` | Load, merge, validate, print a config summary, and exit. |
| `--log-level <LEVEL>` | Override tracing level for this process. Applied after config load. |
| `--headless` | Run the stdin/stdout JSON-RPC server instead of the TUI. |

Flag order does not matter for the current parser, except flags requiring a
value must be followed by that value.

## Logging

When stdout is a TTY, logs use pretty text formatting. When stdout is piped,
logs use JSON formatting. `RUST_LOG` has priority over the configured log level
because tracing uses `EnvFilter::try_from_default_env()` first.

Examples:

```sh
cargo run -p rust-core -- --log-level debug
RUST_LOG=rust_core=trace cargo run -p rust-core
cargo run -p rust-core -- --validate-config 2> /tmp/pi-hybrid.log
```

## Troubleshooting

### `provider 'deepseek': api_key_env 'PI_DEEPSEEK_KEY' is not set`

Cause: the built-in default config requires provider API key env vars.

Fix for real provider operation:

```sh
export PI_DEEPSEEK_KEY=...
export PI_GLM_KEY=...
cargo run -p rust-core -- --validate-config
```

Fix for local/test operation: set provider `api_key_env = "none"` in the config
for providers that should not require keys.

### `provider '<name>' is not defined in [providers]`

Cause: `provider` or `PI_PROVIDER` names a provider that is not present under
`[providers.<name>]`.

Fix: set `provider` to an existing provider key, or add a matching
`[providers.<name>]` table.

### `agent.max_turns must be > 0` or `must be <= 500`

Cause: `agent.max_turns` or `PI_MAX_TURNS` is outside the validated range.

Fix: use a value from 1 through 500.

### `PI_MAX_TURNS is not a valid usize, ignoring`

Cause: `PI_MAX_TURNS` was set to a non-integer. The loader ignores it and keeps
the file/default value.

Fix: unset it or set an integer:

```sh
export PI_MAX_TURNS=50
```

### `session.db_path parent directory ... does not exist and cannot be created`

Cause: the configured session DB path points under a missing or inaccessible
parent. A common version is leaving the generated literal
`~/.pi-hybrid/sessions.db` in place; the config loader does not expand `~`.

Fix: create the parent directory or set `session.db_path`/`PI_SESSION_DB` to a
writable path:

```sh
mkdir -p ~/.pi-hybrid
export PI_SESSION_DB="$HOME/.pi-hybrid/sessions.db"
```

### `session.db_path parent ... is not a directory`

Cause: part of the configured DB parent path is a file.

Fix: choose a DB path whose parent is a directory.

### `bridge.ts_bridge_path '<path>' does not exist`

Cause: bridge path validation emits a non-fatal warning when a non-empty bridge
path does not exist.

Fix: set `bridge.ts_bridge_path = "none"` to disable the bridge for now, leave
it empty to use `PI_BRIDGE_COMMAND`, or point it at a real executable bridge.

### `Bridge test failed: bridge command is empty`

Cause: the TUI command palette bridge test checks whether the derived bridge
command is empty.

Fix: set a real `bridge.ts_bridge_path`, or set `PI_BRIDGE_COMMAND` when the
config bridge path is empty or `none`.

### TUI leaves the terminal in a bad state

Cause: terminal raw mode or alternate screen did not restore after a crash.

Fix:

```sh
reset
stty sane
```

Then re-run with config validation first:

```sh
cargo run -p rust-core -- --validate-config
```

### Headless output mixes logs and JSON-RPC

Cause: headless mode writes JSON-RPC protocol messages to stdout while tracing
format depends on whether stdout is a TTY or pipe.

Fix: keep protocol stdout separate from diagnostics in calling scripts where
possible, and use `--validate-config` before starting headless mode to catch
config failures outside the protocol stream.

### `cargo run -p rust-core -- --help` does not show usage

Cause: flags are parsed manually and no help flag is implemented yet.

Fix: use the CLI flags table in this runbook until a real parser/help command
is added.

## Verification Checklist

Before handing a build to another operator:

```sh
cargo fmt --check
cargo test --workspace
cargo run -p rust-core -- --validate-config --config /path/to/test-config.toml
python3 tests/tui_smoke.py
```

For docs-only runbook updates, verify the required sections exist and smoke-test
the config example:

```sh
rg -n "Install And Build|Run Modes|Configuration|Environment Variables|CLI Flags|Troubleshooting" docs/operator.md
cargo run -p rust-core -- --validate-config --config /tmp/pi-hybrid-operator.toml
```
