#!/usr/bin/env python3
"""Needle-aware JSON-RPC bridge adapter for rust-core BridgeClient.

Reads line-delimited JSON-RPC 2.0 requests on stdin and writes one response
line per request on stdout. Classifies prompts via Needle (pi-route); executes
read-only verbs locally; forwards mutating/uncertain routes to Pi JSON mode.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

DEFAULT_NEEDLE_ROUTE = Path.home() / ".pi" / "needle" / "pi-route"
DEFAULT_NEEDLE_URL = "http://100.93.242.91:9090"
DEFAULT_PI_CMD = "pi"

LOCAL_VERBS = frozenset({"read_file", "search_code"})
FORWARD_VERBS = frozenset({"edit_file", "run_shell", "delegate"})


def workspace_root() -> Path:
    return Path(os.environ.get("WORKSPACE", os.getcwd())).resolve()


def needle_route_bin() -> str:
    return os.environ.get("NEEDLE_ROUTE_BIN", str(DEFAULT_NEEDLE_ROUTE))


def pi_cmd() -> str:
    return os.environ.get("PI_CMD", DEFAULT_PI_CMD)


def rpc_success(request_id: int, result: dict[str, Any]) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def rpc_error(request_id: int, code: int, message: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {"code": code, "message": message},
    }


def emit_response(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def extract_last_user_prompt(params: dict[str, Any]) -> str:
    messages = params.get("messages") or []
    for message in reversed(messages):
        if not isinstance(message, dict):
            continue
        if message.get("role") != "user":
            continue
        content = message.get("content", "")
        if isinstance(content, str):
            return content
        if isinstance(content, list):
            parts: list[str] = []
            for part in content:
                if isinstance(part, str):
                    parts.append(part)
                elif isinstance(part, dict) and part.get("type") == "text":
                    parts.append(str(part.get("text", "")))
            return "".join(parts)
    return ""


def estimate_tokens(text: str) -> int:
    return max(1, len(text) // 4)


def build_prompt_response(content: str, usage: dict[str, int] | None = None) -> dict[str, Any]:
    prompt_tokens = usage.get("prompt_tokens", estimate_tokens(content)) if usage else estimate_tokens(content)
    completion_tokens = usage.get("completion_tokens", estimate_tokens(content)) if usage else estimate_tokens(content)
    total_tokens = usage.get("total_tokens", prompt_tokens + completion_tokens) if usage else prompt_tokens + completion_tokens
    return {
        "content": content,
        "tool_calls": [],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
        },
        "finish_reason": "stop",
    }


def resolve_safe_path(path: str, workspace: Path) -> Path:
    candidate = Path(path)
    if not candidate.is_absolute():
        candidate = workspace / candidate
    resolved = candidate.resolve()
    workspace_resolved = workspace.resolve()
    if os.path.commonpath([str(resolved), str(workspace_resolved)]) != str(workspace_resolved):
        raise ValueError(f"path outside workspace: {path}")
    return resolved


def call_needle_route(prompt: str) -> dict[str, Any] | None:
    env = os.environ.copy()
    env.setdefault("NEEDLE_URL", DEFAULT_NEEDLE_URL)
    try:
        completed = subprocess.run(
            [needle_route_bin(), prompt],
            capture_output=True,
            text=True,
            timeout=int(os.environ.get("NEEDLE_ROUTE_TIMEOUT", "45")),
            env=env,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        sys.stderr.write(f"needle_bridge_adapter: pi-route failed: {exc}\n")
        return None

    if completed.returncode != 0:
        sys.stderr.write(
            "needle_bridge_adapter: pi-route exited "
            f"{completed.returncode}: {completed.stderr.strip()}\n"
        )
        return None

    stdout = completed.stdout.strip()
    if not stdout:
        return None

    try:
        route = json.loads(stdout)
    except json.JSONDecodeError as exc:
        sys.stderr.write(f"needle_bridge_adapter: invalid pi-route JSON: {exc}\n")
        return None

    if not isinstance(route, dict):
        return None
    return route


def execute_read_file(arguments: dict[str, Any], workspace: Path) -> str:
    path = str(arguments.get("path", "")).strip()
    if not path:
        raise ValueError("read_file requires path")
    resolved = resolve_safe_path(path, workspace)
    if not resolved.is_file():
        raise FileNotFoundError(f"file not found: {path}")
    return resolved.read_text(encoding="utf-8", errors="replace")


def execute_search_code(arguments: dict[str, Any], workspace: Path) -> str:
    pattern = str(arguments.get("pattern", "")).strip()
    if not pattern:
        raise ValueError("search_code requires pattern")
    raw_path = str(arguments.get("path", ".")).strip() or "."
    resolved = resolve_safe_path(raw_path, workspace)

    if shutil.which("rg"):
        cmd = ["rg", "-n", "--no-heading", "--color", "never", pattern, str(resolved)]
    else:
        cmd = ["grep", "-R", "-n", "-E", pattern, str(resolved)]

    completed = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if completed.returncode not in (0, 1):
        raise RuntimeError(completed.stderr.strip() or "search failed")
    output = completed.stdout.strip()
    return output or "(no matches)"


def extract_text_from_message(message: dict[str, Any]) -> str:
    content = message.get("content", [])
    if isinstance(content, str):
        return content
    if not isinstance(content, list):
        return ""

    parts: list[str] = []
    for part in content:
        if isinstance(part, str):
            parts.append(part)
        elif isinstance(part, dict) and part.get("type") == "text":
            parts.append(str(part.get("text", "")))
    return "".join(parts)


def map_pi_usage(raw_usage: dict[str, Any]) -> dict[str, int]:
    prompt_tokens = int(raw_usage.get("input", 0) or 0)
    completion_tokens = int(raw_usage.get("output", 0) or 0)
    total_tokens = int(raw_usage.get("totalTokens", prompt_tokens + completion_tokens) or 0)
    return {
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": total_tokens,
    }


def parse_pi_json_stream(stdout: str) -> tuple[str, dict[str, int]]:
    final_text = ""
    final_usage = {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}

    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") != "message_end":
            continue
        message = event.get("message") or {}
        if message.get("role") != "assistant":
            continue
        final_text = extract_text_from_message(message)
        usage = message.get("usage")
        if isinstance(usage, dict):
            final_usage = map_pi_usage(usage)

    if not final_text:
        raise RuntimeError("pi json stream did not contain a final assistant message")
    return final_text, final_usage


def forward_to_pi(prompt: str, workspace: Path) -> dict[str, Any]:
    extra_args = os.environ.get("PI_JSON_EXTRA_ARGS", "")
    cmd = [pi_cmd(), "--mode", "json", "-p", "--no-session"]
    if extra_args:
        cmd.extend(extra_args.split())
    cmd.append(prompt)

    try:
        completed = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=int(os.environ.get("PI_JSON_TIMEOUT", "300")),
            cwd=str(workspace),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise RuntimeError(f"pi subprocess failed: {exc}") from exc

    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or f"exit {completed.returncode}"
        raise RuntimeError(f"pi failed: {detail}")

    content, usage = parse_pi_json_stream(completed.stdout)
    return build_prompt_response(content, usage)


def should_forward(route: dict[str, Any] | None) -> bool:
    if route is None:
        return True
    name = route.get("name")
    if name is None:
        return True
    if not isinstance(name, str):
        return True
    if name in FORWARD_VERBS:
        return True
    if name in LOCAL_VERBS:
        return False
    return True


def handle_send_prompt(request_id: int, params: dict[str, Any]) -> dict[str, Any]:
    prompt = extract_last_user_prompt(params).strip()
    if not prompt:
        return rpc_error(request_id, -32602, "send_prompt requires a user message")

    workspace = workspace_root()
    route = call_needle_route(prompt)

    if should_forward(route):
        try:
            result = forward_to_pi(prompt, workspace)
        except Exception as exc:  # noqa: BLE001 - surface adapter failures to client
            return rpc_error(request_id, -32000, str(exc))
        return rpc_success(request_id, result)

    verb = str(route.get("name"))
    arguments = route.get("arguments") or {}
    if not isinstance(arguments, dict):
        arguments = {}

    try:
        if verb == "read_file":
            content = execute_read_file(arguments, workspace)
        elif verb == "search_code":
            content = execute_search_code(arguments, workspace)
        else:
            return rpc_error(request_id, -32000, f"unsupported local verb: {verb}")
    except Exception as exc:  # noqa: BLE001
        return rpc_error(request_id, -32000, str(exc))

    header = f"[needle:{verb}] "
    return rpc_success(request_id, build_prompt_response(header + content))


def handle_request(raw_line: str) -> dict[str, Any] | None:
    line = raw_line.strip()
    if not line:
        return None

    try:
        request = json.loads(line)
    except json.JSONDecodeError as exc:
        return rpc_error(0, -32700, f"parse error: {exc}")

    if request.get("jsonrpc") != "2.0":
        return rpc_error(int(request.get("id", 0) or 0), -32600, "invalid Request object")

    request_id = request.get("id")
    if not isinstance(request_id, int):
        return rpc_error(0, -32600, "request id must be an integer")

    method = request.get("method")
    if method != "send_prompt":
        return rpc_error(request_id, -32601, f"method not found: {method}")

    params = request.get("params") or {}
    if not isinstance(params, dict):
        return rpc_error(request_id, -32602, "params must be an object")

    return handle_send_prompt(request_id, params)


def main() -> int:
    for raw_line in sys.stdin:
        response = handle_request(raw_line)
        if response is not None:
            emit_response(response)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())