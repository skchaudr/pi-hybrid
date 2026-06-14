# B8 M1 Mac Mini Validation

Validated on the M1 Mac Mini (`sab-mini`) against the stable Rust workspace path.

## Commands run

```sh
cargo build --workspace
cargo test -p rust-core -p ts-bridge
cargo fmt --check --all
cargo clippy --workspace -- -D warnings
```

## Results

- `cargo build --workspace` passed.
- `cargo test -p rust-core -p ts-bridge` passed.
  - `rust-core`: 434 tests passed.
  - `ts-bridge`: 7 tests passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace -- -D warnings` passed.

## Scope note

The validation intentionally excludes the failing `py-extensions` test suite from the pass/fail gate for this phase.
