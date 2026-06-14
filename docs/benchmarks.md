# Benchmarks

## Executive Summary

Baseline numbers pending.

## Methodology

The benchmark harness lives in `rust-core/benches/bench_main.rs` and runs with
Criterion from the workspace root:

```sh
cargo bench
```

The current suite measures only paths that are implemented and meaningful:

- `tui_frame_render_80x24` renders the full Ratatui application layout on a
  fixed `TestBackend` terminal size of 80x24. It includes the Files, Editor,
  Agents, Plan/Approval, and Status areas.
- `sqlite_session_save_load` creates an in-memory SQLite session store, saves a
  session with messages, and loads it back through the real `SessionStore` API.
- `config_load_default_no_file` loads and validates the default configuration
  without reading a config file.

## Hardware Matrix

| Benchmark | M5 Air | M1 Mini |
| --- | ---: | ---: |
| `tui_frame_render_80x24` | pending | pending |
| `sqlite_session_save_load` | pending | pending |
| `config_load_default_no_file` | pending | pending |

## Known Limitations

Bridge RPC round-trip, subagent spawn, and agent-loop turn benchmarks are
intentionally skipped. Those paths are currently scaffolding or placeholders, so
benchmarking them would produce misleading baselines instead of regression
signals.

## Raw Data

### M5 Air

| Benchmark | Mean | Median | Notes |
| --- | ---: | ---: | --- |
| `tui_frame_render_80x24` | pending | pending | Baseline numbers pending |
| `sqlite_session_save_load` | pending | pending | Baseline numbers pending |
| `config_load_default_no_file` | pending | pending | Baseline numbers pending |

### M1 Mini

| Benchmark | Mean | Median | Notes |
| --- | ---: | ---: | --- |
| `tui_frame_render_80x24` | pending | pending | Baseline numbers pending |
| `sqlite_session_save_load` | pending | pending | Baseline numbers pending |
| `config_load_default_no_file` | pending | pending | Baseline numbers pending |
