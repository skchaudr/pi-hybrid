# Bridge client review

## Overall verdict

**Blocker** — `BridgeClient::send_prompt` advertises/enforces a 30s wrapper timeout, but the underlying `Bridge` created by `BridgeClient` has its own 2s read timeout. Real LLM `send_prompt` calls that take longer than 2s can fail before the new latency logging or the `BridgeClient` 30s timeout can take effect.

## Findings

### Blocker: `send_prompt` effectively times out after 2s, not `BridgeClient::DEFAULT_TIMEOUT` 30s

- **Evidence:** `BridgeClient::DEFAULT_TIMEOUT` is 30s at `rust-core/src/agent/bridge_client.rs:22-23`, and `send_prompt` wraps `self.bridge.call("send_prompt", params_value)` in that 30s timeout at `rust-core/src/agent/bridge_client.rs:251-257`.
- **Evidence:** However, `BridgeClient::connect`, `connect_with_provider`, and `reconnect` instantiate the lower-level bridge with `Bridge::new(command)` at `rust-core/src/agent/bridge_client.rs:140-142`, `rust-core/src/agent/bridge_client.rs:163-165`, and `rust-core/src/agent/bridge_client.rs:426-428`.
- **Evidence:** `Bridge::new` delegates to `Bridge::with_timeout(command, DEFAULT_TIMEOUT)` at `rust-core/src/bridge/json_rpc.rs:52-53`, where the lower-level bridge default is 2s at `rust-core/src/bridge/json_rpc.rs:18`; `Bridge::call` applies that timeout directly to `stdout.read_line` at `rust-core/src/bridge/json_rpc.rs:109-110`.
- **Impact:** A valid bridge/LLM response that arrives between 2s and 30s fails with the lower-level `deadline has elapsed` error. This makes `BridgeClient::DEFAULT_TIMEOUT` misleading for `send_prompt`, and the recent latency logging will not capture successful real prompt latencies beyond 2s because those calls fail first.
- **Smallest safe fix:** Have `BridgeClient` construct its bridge with the intended timeout, e.g. use `Bridge::with_timeout(command, DEFAULT_TIMEOUT)` in `connect`, `connect_with_provider`, and `reconnect`, or otherwise remove/align the nested timeout so there is a single source of truth.
- **Test gap:** Add a focused async test with a fake bridge that sleeps >2s and <30s before returning a valid `send_prompt` response; it should succeed through `BridgeClient::send_prompt` once the timeout layering is fixed.

## Correct / no regression found in the latency logging code itself

- `Instant` is monotonic and the added measurement starts immediately before the bridge call at `rust-core/src/agent/bridge_client.rs:251-257`.
- `latency_ms` is computed only after the successful call and is included in both success logging paths: with usage at `rust-core/src/agent/bridge_client.rs:263-270`, and without usage at `rust-core/src/agent/bridge_client.rs:271-273`.
- The `as_millis() as u64` cast is not practically risky under the intended 30s wrapper timeout.

## Test / validation gaps

- Existing `bridge_client` unit tests cover serde/default structs only; they do not exercise `BridgeClient::send_prompt` behavior, timeout behavior, or the emitted latency field (`rust-core/src/agent/bridge_client.rs:461-636`).
- The nearby integration-style test `process_prompt_uses_bridge_response_not_hardcoded_echo` verifies the agent uses bridge response content, but its fake bridge responds immediately (`rust-core/src/agent/mod.rs:517-525`), so it does not cover prompt latency or timeout layering.
- There is no focused regression test proving `send_prompt` succeeds for responses slower than the lower-level bridge default but faster than `BridgeClient::DEFAULT_TIMEOUT`.

## Maintainability notes

- `run_with_provider` accepts `provider_name` at `rust-core/src/agent/bridge_client.rs:201-205` but does not use it when choosing the model (`rust-core/src/agent/bridge_client.rs:206-224`). This predates the latency change, but it makes provider-specific behavior harder to reason about.
- `rust-core/src/lib.rs:1-7` and `rust-core/src/main.rs:1-7` globally allow `unused_variables`/`unused_imports`, which can hide issues like the unused `provider_name` and stale imports in `bridge_client.rs`.

## Commands run and results

- `git status --short && git branch --show-current && git diff -- rust-core/src/agent/bridge_client.rs rust-core/src/agent/mod.rs` — branch `force-pushed-phase-5`; no status/diff output.
- `git show 41bace4 -- rust-core/src/agent/bridge_client.rs --format=short` — confirmed the recent latency logging change adds `Instant`, `started`, `latency_ms`, and debug fields.
- `cargo test -p rust-core bridge_client` — passed: 15 `bridge_client` tests in lib and 15 in main.
- `cargo test -p rust-core process_prompt_uses_bridge_response_not_hardcoded_echo` — passed in lib and main.
- `cargo fmt --check` — passed with no output.
- `cargo check -p rust-core` — passed.
- One attempted combined test-filter command (`cargo test -p rust-core bridge_client process_prompt_uses_bridge_response_not_hardcoded_echo`) failed because Cargo accepts only one test name filter; reran the filters separately as above.

## Residual risk / confidence

Confidence is **high** for the timeout finding because both timeout values and call sites are visible in code. I did not run the full `cargo test -p rust-core` suite or `cargo clippy -p rust-core -- -D warnings`; validation was focused on the reviewed file and nearby caller test per scope.
