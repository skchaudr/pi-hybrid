# Pi Rust-Core Hybrid Workspace

[![CI](https://github.com/skchaudr/pi-hybrid/actions/workflows/ci.yml/badge.svg)](https://github.com/skchaudr/pi-hybrid/actions/workflows/ci.yml)

A Rust terminal UI hybrid agent runtime.

## Workspace Layout

- `rust-core/` - Rust terminal core with the Ratatui shell UI.
- `ts-bridge/` - TypeScript bridge crate.
- `py-extensions/` - Python extension crate.
- `docs/` - Project notes and build instructions.

## Build

From the workspace root:

```sh
cargo build
```

Run the TUI:

```sh
cargo run -p rust-core
```

The TUI opens an alternate-screen Ratatui interface with Files, Editor, Agents,
and Plan/Approval panes. Press `tab` to cycle the active pane and `q` to quit.

## Performance

See [benchmarks.md](benchmarks.md) for the Criterion benchmark harness,
baseline template, and CI regression gate notes.
