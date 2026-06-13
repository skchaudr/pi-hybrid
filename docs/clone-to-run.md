# Pi Hybrid Clone-to-Run Verification

This checklist is the shortest honest path from a fresh clone to a usable local
workspace. It assumes a machine with a Rust toolchain and `git` installed.

## 1) Clone the repository

```sh
git clone https://github.com/skchaudr/pi-hybrid.git
cd pi-hybrid
```

If you already have a checkout, make sure you are on the latest `main`:

```sh
git pull --rebase origin main
```

## 2) Confirm the workspace layout

```sh
rg --files -g 'Cargo.toml' -g 'docs/*.md' -g 'rust-core/src/**/*.rs' -g 'ts-bridge/src/**/*.rs'
```

Expected top-level docs and crates:

- `rust-core/` — main binary and TUI
- `ts-bridge/` — TypeScript bridge crate
- `py-extensions/` — placeholder Python extension crate
- `docs/` — operator, architecture, contributor, and API docs

## 3) Build the workspace

```sh
cargo build --workspace
```

If you want the release binary as part of the verification:

```sh
cargo build --release -p rust-core
```

## 4) Run the Rust tests that are expected to pass in CI

```sh
cargo test -p rust-core -p ts-bridge
```

That exercises the stable Rust workspace surface without depending on the
placeholder Python extension crate.

If your machine has the Python runtime and extension environment ready, you can
also try the full workspace test command:

```sh
cargo test --workspace
```

## 5) Create and validate a local config

Generate a commented config file:

```sh
cargo run -p rust-core -- --init-config --config /tmp/pi-hybrid.toml
```

Edit `/tmp/pi-hybrid.toml` so it is valid for local testing. A minimal config
that avoids provider API keys looks like this:

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

Validate the config:

```sh
cargo run -p rust-core -- --validate-config --config /tmp/pi-hybrid.toml
```

## 6) Smoke-test the TUI entry point

```sh
cargo run -p rust-core -- --config /tmp/pi-hybrid.toml
```

You should see the TUI open. Quit with `q`.

## 7) Confirm the clone is ready for work

A clone is ready when all of these are true:

- `cargo build --workspace` succeeds.
- `cargo test -p rust-core -p ts-bridge` succeeds.
- `cargo run -p rust-core -- --validate-config --config /tmp/pi-hybrid.toml` succeeds.
- The TUI starts and exits cleanly.
- `docs/operator.md` and `docs/contributing.md` are present for operators and contributors.

## Verification checklist

- [ ] Repository cloned from GitHub.
- [ ] Workspace builds cleanly.
- [ ] Core Rust tests pass.
- [ ] Local config validates.
- [ ] TUI starts and quits cleanly.
- [ ] Operator, contributor, API, and clone-to-run docs are available.
