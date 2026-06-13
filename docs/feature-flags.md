# Feature Flags

This project keeps the feature surface intentionally small.

## Current Cargo features

From `rust-core/Cargo.toml`:

- `default = []`
- `python = ["pyo3", "py-extensions"]`
- `typescript = ["ts-bridge"]`

## What that means in practice

- The default build stays lightweight.
- Python integration is optional and only compiled when explicitly enabled.
- TypeScript bridge support is optional and only compiled when explicitly enabled.
- There is **no separate `headless` feature today**.
- Headless execution is a runtime mode, not a Cargo feature switch.

## Why we are not changing the feature contract yet

This repo is primarily a personal tool, so the goal is to keep the build behavior boring and predictable.

That means:

- no new public feature names just for the sake of symmetry,
- no default-feature reshuffle unless it solves a real problem,
- no extra split between TUI and headless compilation until there is a concrete need.

## Practical guidance

Use the existing feature flags only when you need them:

```sh
cargo build
cargo build -p rust-core --features python
cargo build -p rust-core --features typescript
```

If you do not need Python or the TS bridge, leave the default feature set alone.
