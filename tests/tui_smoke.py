#!/usr/bin/env python3
"""Smoke-test the Rust TUI key sequences through a real pseudo-terminal."""

from __future__ import annotations

import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "debug" / "rust-core"


def build_binary() -> None:
    if BIN.exists():
        return
    subprocess.run(["cargo", "build", "-p", "rust-core"], cwd=ROOT, check=True)


def read_until(fd: int, needle: str, timeout: float = 3.0) -> str:
    deadline = time.monotonic() + timeout
    output = ""
    while time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if not ready:
            continue
        chunk = os.read(fd, 4096).decode("utf-8", errors="ignore")
        output += chunk
        if needle in output:
            return output
    raise AssertionError(f"timed out waiting for {needle!r}\n\nLast output:\n{output[-2000:]}")


def send(fd: int, keys: bytes) -> None:
    os.write(fd, keys)
    time.sleep(0.1)


def set_terminal_size(fd: int, rows: int = 30, columns: int = 100) -> None:
    import fcntl

    size = struct.pack("HHHH", rows, columns, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, size)


def main() -> int:
    build_binary()

    master, slave = pty.openpty()
    set_terminal_size(slave)
    env = os.environ.copy()
    env.setdefault("TERM", "xterm-256color")

    process = subprocess.Popen(
        [str(BIN)],
        cwd=ROOT,
        env=env,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
    )
    os.close(slave)

    try:
        read_until(master, "Pi Hybrid v0.1.0")

        send(master, b"\x10")  # Ctrl+P
        read_until(master, "Command Palette")

        send(master, b"\x1b")  # Esc
        send(master, b"\x1bOP")  # F1 in xterm/SS3 form
        read_until(master, "Help")

        send(master, b"q")
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired as exc:
            raise AssertionError("q did not quit the TUI within 3 seconds") from exc

        if process.returncode != 0:
            raise AssertionError(f"TUI exited with status {process.returncode}")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=3)
        os.close(master)

    print("tui smoke passed: Ctrl+P, F1, q")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"tui smoke failed: {exc}", file=sys.stderr)
        raise
