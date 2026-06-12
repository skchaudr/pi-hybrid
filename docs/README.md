# Pi Rust-Core Hybrid Workspace

[![CI](https://github.com/<github-user>/<github-repo>/actions/workflows/ci.yml/badge.svg)](https://github.com/<github-user>/<github-repo>/actions/workflows/ci.yml)

Phase 0 establishes the workspace foundation for a hybrid Pi agent runtime.

## Workspace Layout

- `rust-core/` - Rust terminal core with the Ratatui shell UI.
- `ts-bridge/` - TypeScript bridge crate placeholder.
- `py-extensions/` - Python extension crate placeholder.
- `rust-core-temp/` - Reference checkout for Pi agent Rust patterns.
- `grain-reference/` - Intended reference checkout for Grain agent harness patterns.
- `docs/` - Project notes and build instructions.

## Build

From the workspace root:

```sh
cargo build
```

Run the Phase 0 TUI:

```sh
cargo run -p rust-core
```

The TUI opens an alternate-screen Ratatui interface with Files, Editor, Agents,
and Plan/Approval panes. Press `tab` to cycle the active pane and `q` to quit.
