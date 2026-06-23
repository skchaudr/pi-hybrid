# TUI E2E Collaboration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build enough integrated TUI evidence that Sab can run the app, feel the current workflow, and trust what is real versus scaffolded.

**Architecture:** Keep CMUX as the agent router. Codex coordinates the testing spine and owns final integration; Claude, Grok Composer 2.5, or Pi take bounded review/execution slices so no single agent burns the whole context window. Work moves in small commits after each independently verified slice.

**Tech Stack:** Rust 2024, ratatui, crossterm, Python PTY smoke scripts, graphify, CMUX relay, Cargo test/build/clippy.

## Global Constraints

- CMUX is the coordination layer; do not add tmux to the agent topology for this push.
- Repo starts clean on `force-pushed-phase-5`; verify `git status --short --branch` before edits and after every task.
- Do not touch secrets, API keys, `.env`, `~/.ssh`, global config, or provider credentials.
- Keep all test configs local and fake-key capable; integrated tests must not require paid provider calls.
- Use graphify before codebase-wide source browsing when asking architectural questions.
- Use relay messages only for `STATE`, `ASK`, `ANSWER`, and terminal `ACK`; never ACK an ACK.
- Plain `STATE` updates do not require replies.
- Any destructive action, force git operation, or deletion requires Sab's explicit approval.

---

## Current Baseline

Verified before writing this plan:

- `git status --short --branch` showed a clean `force-pushed-phase-5` branch tracking `origin/force-pushed-phase-5`.
- `cargo test -p rust-core` passed with `426 passed`.
- `cargo build -p rust-core` passed.
- `cargo fmt --check` passed.
- `cargo clippy -p rust-core -- -D warnings` passed.
- `python3 tests/e2e_headless.py` passed with process access.
- `python3 tests/tui_smoke.py` passed with process access.

Observed confidence gap:

- Existing PTY smoke validates startup/render/cleanup only.
- Headless `run` is documented as simulated and does not yet prove the same agent/session path as the TUI.
- The missing confidence layer is scripted interaction: send keys, capture screen, assert visible state transitions, then let Sab manually run the same path.

## Communication Protocol

Use these message types between agents:

```text
STATE: one-way status change; no reply expected
ASK: needs action, review, decision, or confirmation
ANSWER: direct response to an ASK
ACK: terminal confirmation only; never ACK an ACK
```

Recommended CMUX relay from Codex to Claude when Claude is live:

```bash
cmux send --surface surface:3 "[from codex] ASK: Review Task 3 only: integrated PTY interaction harness for palette, help, and quit."
cmux send-key --surface surface:3 enter
```

Before sending, verify the target surface is idle:

```bash
cmux read-screen --surface surface:3
```

## Work Split

- Codex: coordinator, baseline owner, integration reviewer, final test runner.
- Claude: independent reviewer for harness design, UX test scenarios, and risk gaps.
- Grok Composer 2.5: optional coding peer for one bounded implementation slice if Sab has it live in CMUX.
- Pi: optional execution peer for repetitive verification, repo hygiene, or command-output validation.
- Sab: only decision points, approvals, and subjective TUI feel.

### Task 1: Lock The Test Target

**Files:**
- Read: `rust-core/src/main.rs`
- Read: `rust-core/src/keybindings.rs`
- Read: `tests/tui_smoke.py`
- Read: `tests/e2e_headless.py`
- Modify: none unless the task discovers stale docs

**Interfaces:**
- Consumes: Current clean repo and existing passing baseline.
- Produces: A short target statement for the first integrated TUI scenario.

- [ ] **Step 1: Query graphify for the event path**

Run:

```bash
graphify query "rust-core TUI event loop keybindings command palette help popup PTY smoke"
```

Expected: nodes around `rust-core/src/main.rs`, `rust-core/src/keybindings.rs`, and `tests/tui_smoke.py`.

- [ ] **Step 2: Pick the first scenario**

Use this first scenario unless Sab changes it:

```text
Launch TUI in a PTY with fake local config, assert title renders, send Ctrl+P, assert command palette renders, send Esc, send F1 or ?, assert help renders, send Esc, send q, assert clean exit.
```

- [ ] **Step 3: Ask Claude for scenario review**

Send only if Claude is live and idle:

```text
[from codex] ASK: Review first TUI E2E scenario: launch PTY, assert title, Ctrl+P palette, Esc, ?/F1 help, Esc, q clean exit. Any critical gap before implementation?
```

Expected Claude response: `ANSWER` with either "good" or one specific gap.

- [ ] **Step 4: Commit only if docs changed**

If no files changed, do not commit. If a stale doc was corrected:

```bash
git add docs/operator.md
git commit -m "docs: clarify first TUI e2e scenario"
```

### Task 2: Harden Existing Smoke Scripts For Restricted Runners

**Files:**
- Modify: `tests/tui_smoke.py`
- Modify: `tests/e2e_headless.py`

**Interfaces:**
- Consumes: Existing `process_snapshot(label: str) -> None`.
- Produces: A best-effort process snapshot that does not fail the smoke before the app starts.

- [ ] **Step 1: Write the failing expectation**

In both smoke scripts, define the intended behavior:

```python
def process_snapshot(label: str) -> None:
    print(f"\n--- process snapshot: {label} ---")
    try:
        result = subprocess.run(
            ["ps", "-axo", "pid,ppid,etime,%cpu,%mem,command"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
    except PermissionError as exc:
        print(f"(process snapshot unavailable: {exc})")
        return
```

- [ ] **Step 2: Run the smoke without escalation**

Run:

```bash
python3 tests/e2e_headless.py
python3 tests/tui_smoke.py
```

Expected: both scripts pass or reach the actual app path; neither fails solely on `PermissionError: ps`.

- [ ] **Step 3: Run normal verification**

Run:

```bash
cargo fmt --check
cargo test -p rust-core
python3 tests/e2e_headless.py
python3 tests/tui_smoke.py
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e_headless.py tests/tui_smoke.py
git commit -m "test: tolerate restricted process snapshots"
```

### Task 3: Add A Scripted TUI Interaction Harness

**Files:**
- Create: `tests/tui_interaction.py`
- Modify: none in Rust unless the test reveals a real app bug

**Interfaces:**
- Consumes: `target/debug/rust-core`, PTY setup from `tests/tui_smoke.py`, fake TOML config writer.
- Produces: A Python script that verifies visible TUI state transitions from real key input.

- [ ] **Step 1: Create the script with a failing interaction check**

Initial target content:

```python
#!/usr/bin/env python3
"""Integrated PTY interaction smoke for rust-core TUI."""

from __future__ import annotations

import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "debug" / "rust-core"
GLOBAL_TIMEOUT = 18.0
READ_TIMEOUT = 5.0
STOP_TIMEOUT = 3.0


def now() -> float:
    return time.monotonic()


def remaining(deadline: float) -> float:
    return max(0.0, deadline - now())


def build_binary(deadline: float) -> None:
    if BIN.exists():
        return
    subprocess.run(["cargo", "build", "-p", "rust-core"], cwd=ROOT, check=True, timeout=remaining(deadline))


def write_config(path: Path, db_path: Path) -> None:
    path.write_text(
        f'''provider = "deepseek"

[session]
db_path = "{db_path}"

[bridge]
ts_bridge_path = "none"
ts_bridge_timeout = 30000

[logging]
level = "error"

[agent]
max_turns = 5
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
''',
        encoding="utf-8",
    )


def set_terminal_size(fd: int, rows: int = 34, columns: int = 120) -> None:
    import fcntl

    size = struct.pack("HHHH", rows, columns, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, size)


def read_available(fd: int, duration: float) -> str:
    deadline = now() + duration
    output = ""
    while now() < deadline:
        ready, _, _ = select.select([fd], [], [], min(0.05, remaining(deadline)))
        if ready:
            output += os.read(fd, 4096).decode("utf-8", errors="ignore")
    return output


def wait_for(fd: int, needle: str, deadline: float, timeout: float = READ_TIMEOUT) -> str:
    local_deadline = min(deadline, now() + timeout)
    output = ""
    while now() < local_deadline:
        output += read_available(fd, 0.1)
        if needle in output:
            return output
    raise AssertionError(f"timed out waiting for {needle!r}\n\nLast output:\n{output[-2500:]}")


def send(fd: int, data: bytes) -> None:
    os.write(fd, data)
    time.sleep(0.25)


def stop_process(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except Exception:
        process.terminate()
    try:
        process.wait(timeout=STOP_TIMEOUT)
        return
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except Exception:
        process.kill()
    process.wait(timeout=STOP_TIMEOUT)


def main() -> int:
    deadline = now() + GLOBAL_TIMEOUT
    build_binary(deadline)
    temp_root = Path(tempfile.mkdtemp(prefix="pi-hybrid-tui-interaction-"))
    master: int | None = None
    process: subprocess.Popen[bytes] | None = None
    try:
        config_path = temp_root / "config.toml"
        db_path = temp_root / "sessions.db"
        write_config(config_path, db_path)

        master, slave = pty.openpty()
        set_terminal_size(slave)
        env = os.environ.copy()
        env.setdefault("TERM", "xterm-256color")

        process = subprocess.Popen(
            [str(BIN), "--config", str(config_path)],
            cwd=ROOT,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            close_fds=True,
            start_new_session=True,
        )
        os.close(slave)

        wait_for(master, "Pi Hybrid v0.1.0", deadline)
        send(master, b"\x10")  # Ctrl+P
        wait_for(master, "Command", deadline)
        send(master, b"\x1b")  # Esc
        send(master, b"?")
        wait_for(master, "Help", deadline)
        send(master, b"\x1b")  # Esc
        send(master, b"q")
        process.wait(timeout=min(STOP_TIMEOUT, remaining(deadline)))
        if process.returncode != 0:
            raise AssertionError(f"tui exited with status {process.returncode}")
    finally:
        if process is not None:
            stop_process(process)
        if master is not None:
            os.close(master)
        shutil.rmtree(temp_root, ignore_errors=True)

    print("tui interaction passed: palette, help, quit")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Run it and inspect failure**

Run:

```bash
python3 tests/tui_interaction.py
```

Expected first result: either PASS, or a concrete timeout showing which screen transition is not observable.

- [ ] **Step 3: If key input fails, ask Claude to review before changing Rust**

Send:

```text
[from codex] ASK: PTY interaction sends Ctrl+P/?/q but the command palette transition failed. Please review whether this is harness timing, escape encoding, or Rust event handling before I edit code.
```

Expected: Claude gives one bounded `ANSWER`.

- [ ] **Step 4: Apply the smallest fix**

If the issue is harness-side, adjust `tests/tui_interaction.py` only.
If the issue is Rust-side, edit the owning file:

- `rust-core/src/keybindings.rs` for key mapping.
- `rust-core/src/main.rs` for overlay state or event dispatch.
- `rust-core/src/tui/command_palette.rs` or `rust-core/src/tui/help_popup.rs` for visible text/rendering.

- [ ] **Step 5: Verify**

Run:

```bash
python3 tests/tui_interaction.py
cargo test -p rust-core
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add tests/tui_interaction.py rust-core/src/main.rs rust-core/src/keybindings.rs rust-core/src/tui/command_palette.rs rust-core/src/tui/help_popup.rs
git commit -m "test: add integrated TUI interaction smoke"
```

If only `tests/tui_interaction.py` changed, stage only that file.

### Task 4: Manual Operator Run With Sab

**Files:**
- Modify: `docs/operator.md` only if the manual path is missing or misleading.

**Interfaces:**
- Consumes: Passing automated interaction harness.
- Produces: One manual run path that Sab can execute and judge.

- [ ] **Step 1: Build the app**

Run:

```bash
cargo build -p rust-core
```

Expected: `Finished dev profile`.

- [ ] **Step 2: Create a temporary local config**

Run:

```bash
tmpdir="$(mktemp -d)"
cat > "$tmpdir/config.toml" <<EOF
provider = "deepseek"

[session]
db_path = "$tmpdir/sessions.db"

[bridge]
ts_bridge_path = "none"
ts_bridge_timeout = 30000

[logging]
level = "error"

[agent]
max_turns = 5
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
EOF
target/debug/rust-core --config "$tmpdir/config.toml"
```

Expected: TUI opens without API keys.

- [ ] **Step 3: Sab does the feel pass**

Manual script:

```text
1. Confirm initial layout feels legible.
2. Press Tab through panes.
3. Press Ctrl+P and search/open a command.
4. Press ? or F1 for help.
5. Toggle file tree and agent pane.
6. Quit with q.
```

- [ ] **Step 4: Capture findings**

Create or update a small note only if Sab wants it recorded:

```text
docs/tui-e2e-findings.md
```

Commit if edited:

```bash
git add docs/operator.md docs/tui-e2e-findings.md
git commit -m "docs: capture TUI e2e operator findings"
```

### Task 5: Independent UX/Risk Review

**Files:**
- Read: `docs/operator.md`
- Read: `docs/terminal-matrix.md`
- Read: `tests/tui_interaction.py`
- Read: `rust-core/src/main.rs`
- Optional modify: `docs/tui-e2e-findings.md`

**Interfaces:**
- Consumes: Automated and manual E2E findings.
- Produces: A ranked list of 3-5 highest-value fixes, not a broad rewrite.

- [ ] **Step 1: Ask one peer agent for review**

Claude request:

```text
[from codex] ASK: Review the current TUI E2E evidence and propose the top 3 risk-reducing fixes only. Constraints: no provider calls, no broad refactor, focus on integrated feel and testability.
```

Grok request, if Grok Composer 2.5 is live:

```text
[from codex] ASK: Independently inspect the Rust TUI and test harness. Return top 3 concrete fixes for integrated E2E confidence, with files and verification commands.
```

- [ ] **Step 2: Codex reconciles recommendations**

Accept only fixes that meet all criteria:

```text
- User-visible or test-confidence impact.
- Can be verified locally.
- Touches 1-3 files.
- Does not require secrets or provider spend.
```

- [ ] **Step 3: Ask Sab for the next implementation target**

Send in chat:

```text
ASK: The top candidates are harden process snapshots, add interaction smoke, and document manual operator run. I recommend interaction smoke because it directly tests user-visible TUI behavior. Approve that target?
```

Do not implement until Sab picks.

### Task 6: First Real Fix Slice

**Files:**
- To be selected after Task 5.

**Interfaces:**
- Consumes: Sab-approved target.
- Produces: One verified fix and one logical commit.

- [ ] **Step 1: Write or extend a failing test**

Use the smallest relevant test surface:

```text
- Rust unit test for pure state/key/render logic.
- Python PTY test for integrated user-visible behavior.
- Headless JSON-RPC test only for headless protocol behavior.
```

- [ ] **Step 2: Verify failure**

Run the specific test command first.

- [ ] **Step 3: Implement the smallest fix**

Edit only the owning file(s).

- [ ] **Step 4: Verify complete slice**

Run:

```bash
cargo fmt --check
cargo test -p rust-core
python3 tests/e2e_headless.py
python3 tests/tui_smoke.py
python3 tests/tui_interaction.py
```

- [ ] **Step 5: Commit**

Use a conventional commit scoped to the actual fix:

```bash
git add rust-core/src/main.rs rust-core/src/keybindings.rs tests/tui_interaction.py
git commit -m "fix: make TUI interaction path deterministic"
```

## Final Gate

Before claiming serious progress:

```bash
git status --short --branch
cargo fmt --check
cargo clippy -p rust-core -- -D warnings
cargo test -p rust-core
python3 tests/e2e_headless.py
python3 tests/tui_smoke.py
python3 tests/tui_interaction.py
```

Expected:

- All commands pass.
- Working tree is clean except intentional uncommitted handoff notes approved by Sab.
- Sab has manually run the TUI once and given a subjective feel read.

## Execution Choice

Recommended execution mode: subagent-style over CMUX, but with only one peer active at a time.

1. Codex implements Task 2 or Task 3.
2. Claude reviews that single slice.
3. Codex reconciles and commits.
4. Grok Composer 2.5 is invited only for a bounded coding slice if Sab wants a third coding perspective.
5. Pi runs verification or repo hygiene checks when useful.
