#!/usr/bin/env python3
"""Contained smoke-test for Rust TUI startup/render through a pseudo-terminal."""

from __future__ import annotations

import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "debug" / "rust-core"
GLOBAL_TIMEOUT = 15.0
READ_TIMEOUT = 4.0
STOP_TIMEOUT = 3.0


def now() -> float:
    return time.monotonic()


def remaining(deadline: float) -> float:
    return max(0.0, deadline - now())


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
    terms = [
        "rust-core",
        "target/debug/rust-core",
        "target/release/rust-core",
        " pi ",
        "hermes",
        "codex",
        "skycomputeruseclient",
    ]
    lines = result.stdout.splitlines()
    matches = [
        line
        for line in lines[1:]
        if any(term in line.lower() for term in terms)
    ]
    if matches:
        print(lines[0])
        print("\n".join(matches))
    else:
        print("(none)")


def build_binary(deadline: float) -> None:
    if BIN.exists():
        return
    subprocess.run(
        ["cargo", "build", "-p", "rust-core"],
        cwd=ROOT,
        check=True,
        timeout=remaining(deadline),
    )


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


def read_until(fd: int, needle: str, deadline: float, timeout: float = READ_TIMEOUT) -> str:
    local_deadline = min(deadline, now() + timeout)
    output = ""
    while now() < local_deadline:
        ready, _, _ = select.select([fd], [], [], min(0.05, remaining(local_deadline)))
        if not ready:
            continue
        chunk = os.read(fd, 4096).decode("utf-8", errors="ignore")
        output += chunk
        if needle in output:
            return output
    raise AssertionError(f"timed out waiting for {needle!r}\n\nLast output:\n{output[-2000:]}")


def set_terminal_size(fd: int, rows: int = 30, columns: int = 100) -> None:
    import fcntl

    size = struct.pack("HHHH", rows, columns, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, size)


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
    process_snapshot("before")
    build_binary(deadline)

    temp_root = Path(tempfile.mkdtemp(prefix="pi-hybrid-tui-smoke-"))
    keep_artifacts = os.environ.get("KEEP_ARTIFACTS") == "1"
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
        )
        os.close(slave)
        print(f"started tui pid={process.pid}")

        read_until(master, "Pi Hybrid v0.1.0", deadline)

        # This PTY harness validates bounded startup/render and cleanup. On some
        # macOS non-interactive PTYs, crossterm does not observe synthetic key
        # input reliably, so shutdown is performed by the containment cleanup.
    finally:
        if process is not None:
            stop_process(process)
        if master is not None:
            os.close(master)
        if keep_artifacts:
            print(f"keeping artifacts: {temp_root}")
        else:
            shutil.rmtree(temp_root, ignore_errors=True)
        process_snapshot("after")

    print("tui smoke passed: render, cleanup")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"tui smoke failed: {exc}", file=sys.stderr)
        raise
