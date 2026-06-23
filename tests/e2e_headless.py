#!/usr/bin/env python3
"""Contained headless JSON-RPC smoke test for rust-core."""

from __future__ import annotations

import json
import os
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "debug" / "rust-core"
GLOBAL_TIMEOUT = 12.0
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


def run_bounded(args: list[str], deadline: float) -> None:
    subprocess.run(args, cwd=ROOT, check=True, timeout=remaining(deadline))


def build_binary(deadline: float) -> None:
    if BIN.exists():
        return
    run_bounded(["cargo", "build", "-p", "rust-core"], deadline)


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


def stop_process(process: subprocess.Popen[str]) -> None:
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


def read_json_line(process: subprocess.Popen[str], deadline: float) -> dict:
    assert process.stdout is not None
    output_tail: list[str] = []
    while now() < deadline:
        if process.poll() is not None:
            raise AssertionError(
                f"process exited early with {process.returncode}; tail={output_tail[-8:]}"
            )
        ready, _, _ = select.select([process.stdout], [], [], min(0.1, remaining(deadline)))
        if not ready:
            continue
        line = process.stdout.readline()
        if not line:
            continue
        line = line.strip()
        output_tail.append(line)
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if parsed.get("jsonrpc") == "2.0":
            return parsed
    raise AssertionError(f"timed out waiting for JSON-RPC line; tail={output_tail[-8:]}")


def wait_for(process: subprocess.Popen[str], predicate, timeout: float) -> dict:
    deadline = now() + timeout
    while now() < deadline:
        msg = read_json_line(process, deadline)
        if predicate(msg):
            return msg
    raise AssertionError("timed out waiting for expected JSON-RPC message")


def send(process: subprocess.Popen[str], payload: dict) -> None:
    assert process.stdin is not None
    process.stdin.write(json.dumps(payload) + "\n")
    process.stdin.flush()


def main() -> int:
    deadline = now() + GLOBAL_TIMEOUT
    process_snapshot("before")
    build_binary(deadline)

    temp_root = Path(tempfile.mkdtemp(prefix="pi-hybrid-e2e-headless-"))
    keep_artifacts = os.environ.get("KEEP_ARTIFACTS") == "1"
    process: subprocess.Popen[str] | None = None
    try:
        config_path = temp_root / "config.toml"
        db_path = temp_root / "sessions.db"
        write_config(config_path, db_path)

        process = subprocess.Popen(
            [str(BIN), "--config", str(config_path), "--headless"],
            cwd=ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        print(f"started headless pid={process.pid}")

        ready = wait_for(process, lambda msg: msg.get("method") == "ready", READ_TIMEOUT)
        assert ready["params"]["mode"] == "headless"

        send(process, {"jsonrpc": "2.0", "id": 1, "method": "status", "params": {}})
        status = wait_for(process, lambda msg: msg.get("id") == 1, READ_TIMEOUT)
        assert "result" in status or "error" in status

        send(process, {"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}})
        shutdown = wait_for(process, lambda msg: msg.get("id") == 2, READ_TIMEOUT)
        assert shutdown.get("result", {}).get("shutdown") is True
        process.wait(timeout=min(STOP_TIMEOUT, remaining(deadline)))
        if process.returncode != 0:
            raise AssertionError(f"headless exited with status {process.returncode}")
    finally:
        if process is not None:
            stop_process(process)
        if keep_artifacts:
            print(f"keeping artifacts: {temp_root}")
        else:
            shutil.rmtree(temp_root, ignore_errors=True)
        process_snapshot("after")

    print("headless e2e passed: ready, status, shutdown")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"headless e2e failed: {exc}", file=sys.stderr)
        raise
