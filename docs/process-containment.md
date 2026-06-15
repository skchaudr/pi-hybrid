# Process Containment

Pi Hybrid integration work must make process state visible before it starts child
processes, enforce hard timeouts while they run, and clean up on every exit path.
This checkpoint exists because runaway agent/test processes are more dangerous
than a failing test.

## Inspect Live Process State

Use these commands before and after integration or end-to-end checks:

```sh
ps -axo pid,ppid,etime,%cpu,%mem,command \
  | egrep '[r]ust-core|[t]arget/(debug|release)/rust-core|[p]i |[H]ermes|[h]ermes|[C]odex|[c]odex|[S]kyComputerUseClient' \
  || true

pgrep -af '[r]ust-core|[t]arget/.*/rust-core|[S]kyComputerUseClient|[H]ermes|[h]ermes|[p]i ' || true
```

Expected state before a bounded test: no old `rust-core` process owned by a
previous harness run. Long-lived Pi, Hermes, Codex, or `SkyComputerUseClient`
helpers should be intentional and explainable.

Abnormal state:

- `rust-core` remains after a test exits.
- A helper has been running for hours without an active session that explains it.
- CPU stays high after the harness reports success or failure.
- A test-created temp directory/database remains without `KEEP_ARTIFACTS=1`.

## Harness Rules

Every integration or end-to-end harness must:

1. Print the child PID after `Popen`/spawn.
2. Print process snapshots before and after the test.
3. Use a wall-clock timeout for the whole harness.
4. Use per-read timeouts; never wait forever for output.
5. On failure, `terminate`, then `kill` if the child does not exit quickly.
6. Use temp config and temp SQLite database paths.
7. Default to `api_key_env = "none"` and `ts_bridge_path = "none"`.
8. Avoid network/provider calls unless the test name and docs explicitly say so.
9. Remove temp artifacts unless `KEEP_ARTIFACTS=1` is set.
10. Assert that the harness-owned child process has exited.

## Safe Manual Cleanup

Prefer targeted cleanup over broad process killing:

```sh
pgrep -af '[t]arget/.*/rust-core|[r]ust-core --headless'
kill <pid>
sleep 2
kill -9 <pid>   # only if the process ignored the normal signal
```

For terminal recovery after a crashed TUI:

```sh
reset
stty sane
```

Do not kill Pi, Hermes, Codex, or `SkyComputerUseClient` helpers unless you have
confirmed they are stale and not attached to active work.

## Current Contained Checks

```sh
python3 tests/e2e_headless.py
python3 tests/tui_smoke.py
```

Both scripts are expected to start one `rust-core` child, enforce a timeout,
clean it up, and print before/after process evidence. The headless harness uses
JSON-RPC `shutdown`; the TUI harness validates startup/render and terminates the
child through the containment cleanup path because synthetic key input is not
reliable on every macOS non-interactive PTY.
