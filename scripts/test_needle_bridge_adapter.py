#!/usr/bin/env python3
"""Pipe-style tests for scripts/needle_bridge_adapter.py."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
ADAPTER = REPO_ROOT / "scripts" / "needle_bridge_adapter.py"
if str(ADAPTER.parent) not in sys.path:
    sys.path.insert(0, str(ADAPTER.parent))

from needle_bridge_adapter import extract_last_user_prompt


def run_adapter_completed(
    requests: list[dict],
    *,
    env: dict[str, str],
    cwd: Path,
) -> subprocess.CompletedProcess[str]:
    stdin = "\n".join(json.dumps(req) for req in requests) + "\n"
    return subprocess.run(
        [sys.executable, str(ADAPTER)],
        input=stdin,
        capture_output=True,
        text=True,
        cwd=str(cwd),
        env=env,
        check=True,
    )


def parse_adapter_responses(stdout: str) -> list[dict]:
    lines = [line for line in stdout.splitlines() if line.strip()]
    return [json.loads(line) for line in lines]


def run_adapter(
    requests: list[dict],
    *,
    env: dict[str, str],
    cwd: Path,
) -> list[dict]:
    completed = run_adapter_completed(requests, env=env, cwd=cwd)
    return parse_adapter_responses(completed.stdout)


def make_send_prompt_request(request_id: int, prompt: str) -> dict:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "send_prompt",
        "params": {
            "model": "test-model",
            "messages": [{"role": "user", "content": prompt, "tool_calls": None}],
            "system": "test",
        },
    }


def assert_prompt_response_shape(test_case: unittest.TestCase, result: dict) -> None:
    test_case.assertIn("content", result)
    test_case.assertIsInstance(result["content"], str)
    test_case.assertIn("tool_calls", result)
    test_case.assertIsInstance(result["tool_calls"], list)
    usage = result["usage"]
    test_case.assertIsInstance(usage, dict)
    for key in ("prompt_tokens", "completion_tokens", "total_tokens"):
        test_case.assertIn(key, usage)
        test_case.assertIsInstance(usage[key], int)
    test_case.assertEqual(result.get("finish_reason"), "stop")


class NeedleBridgeAdapterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temp_dir.name)
        self.env = os.environ.copy()
        self.env["WORKSPACE"] = str(self.workspace)
        self.env["NEEDLE_USAGE_LOGGING"] = "0"

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_extract_last_user_prompt_joins_list_content(self) -> None:
        params = {
            "messages": [
                {"role": "user", "content": "older prompt"},
                {"role": "assistant", "content": "assistant reply"},
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "read "},
                        {"type": "image_url", "image_url": {"url": "ignored"}},
                        "the ",
                        {"type": "text", "text": "file"},
                    ],
                },
            ]
        }

        self.assertEqual(extract_last_user_prompt(params), "read the file")

    def test_read_file_route_executes_locally(self) -> None:
        target = self.workspace / "sample.txt"
        target.write_text("local file contents", encoding="utf-8")

        mock_route = self.workspace / "mock_pi_route.py"
        mock_route.write_text(
            textwrap.dedent(
                """
                #!/usr/bin/env python3
                import json, sys
                print(json.dumps({
                    "name": "read_file",
                    "arguments": {"path": "sample.txt"},
                    "_route_source": "test"
                }))
                """
            ).strip()
            + "\n",
            encoding="utf-8",
        )
        mock_route.chmod(0o755)

        self.env["NEEDLE_ROUTE_BIN"] = str(mock_route)

        responses = run_adapter(
            [make_send_prompt_request(7, "read sample.txt")],
            env=self.env,
            cwd=self.workspace,
        )

        self.assertEqual(len(responses), 1)
        self.assertEqual(responses[0]["id"], 7)
        self.assertIn("result", responses[0])
        result = responses[0]["result"]
        assert_prompt_response_shape(self, result)
        self.assertIn("local file contents", result["content"])
        self.assertTrue(result["content"].startswith("[needle:read_file]"))

    def test_edit_file_route_forwards_to_pi_json(self) -> None:
        mock_route = self.workspace / "mock_pi_route.py"
        mock_route.write_text(
            textwrap.dedent(
                """
                #!/usr/bin/env python3
                import json
                print(json.dumps({
                    "name": "edit_file",
                    "arguments": {"path": "sample.txt", "intent": "change it"},
                    "_route_source": "test"
                }))
                """
            ).strip()
            + "\n",
            encoding="utf-8",
        )
        mock_route.chmod(0o755)

        mock_pi = self.workspace / "mock_pi.py"
        mock_pi.write_text(
            textwrap.dedent(
                """
                #!/usr/bin/env python3
                import json, sys
                prompt = sys.argv[-1]
                events = [
                    {"type": "session"},
                    {"type": "agent_start"},
                    {"type": "turn_start"},
                    {
                        "type": "message_end",
                        "message": {
                            "role": "assistant",
                            "content": [{"type": "text", "text": f"Pi handled: {prompt}"}],
                            "usage": {"input": 12, "output": 8, "totalTokens": 20},
                        },
                    },
                ]
                for event in events:
                    print(json.dumps(event), flush=True)
                """
            ).strip()
            + "\n",
            encoding="utf-8",
        )
        mock_pi.chmod(0o755)

        self.env["NEEDLE_ROUTE_BIN"] = str(mock_route)
        self.env["PI_CMD"] = str(mock_pi)

        responses = run_adapter(
            [make_send_prompt_request(9, "edit sample.txt to say hello")],
            env=self.env,
            cwd=self.workspace,
        )

        self.assertEqual(responses[0]["id"], 9)
        result = responses[0]["result"]
        assert_prompt_response_shape(self, result)
        self.assertIn("Pi handled: edit sample.txt to say hello", result["content"])
        self.assertEqual(result["usage"]["total_tokens"], 20)

    def test_route_failure_forwards_to_pi_json(self) -> None:
        mock_route = self.workspace / "mock_pi_route_fail.py"
        mock_route.write_text(
            "#!/usr/bin/env python3\nimport sys\nsys.exit(3)\n",
            encoding="utf-8",
        )
        mock_route.chmod(0o755)

        mock_pi = self.workspace / "mock_pi.py"
        mock_pi.write_text(
            textwrap.dedent(
                """
                #!/usr/bin/env python3
                import json, sys
                prompt = sys.argv[-1]
                print(json.dumps({
                    "type": "message_end",
                    "message": {
                        "role": "assistant",
                        "content": [{"type": "text", "text": f"fallback:{prompt}"}],
                        "usage": {"input": 3, "output": 2, "totalTokens": 5},
                    },
                }))
                """
            ).strip()
            + "\n",
            encoding="utf-8",
        )
        mock_pi.chmod(0o755)

        self.env["NEEDLE_ROUTE_BIN"] = str(mock_route)
        self.env["PI_CMD"] = str(mock_pi)

        responses = run_adapter(
            [make_send_prompt_request(11, "do something fuzzy")],
            env=self.env,
            cwd=self.workspace,
        )

        result = responses[0]["result"]
        assert_prompt_response_shape(self, result)
        self.assertIn("fallback:do something fuzzy", result["content"])

    def test_empty_route_stdout_logs_error_and_forwards(self) -> None:
        mock_route = self.workspace / "mock_pi_route_empty.py"
        mock_route.write_text(
            "#!/usr/bin/env python3\n",
            encoding="utf-8",
        )
        mock_route.chmod(0o755)

        mock_pi = self.workspace / "mock_pi.py"
        mock_pi.write_text(
            textwrap.dedent(
                """
                #!/usr/bin/env python3
                import json, sys
                prompt = sys.argv[-1]
                print(json.dumps({
                    "type": "message_end",
                    "message": {
                        "role": "assistant",
                        "content": [{"type": "text", "text": f"fallback:{prompt}"}],
                        "usage": {"input": 3, "output": 2, "totalTokens": 5},
                    },
                }))
                """
            ).strip()
            + "\n",
            encoding="utf-8",
        )
        mock_pi.chmod(0o755)

        self.env["NEEDLE_ROUTE_BIN"] = str(mock_route)
        self.env["PI_CMD"] = str(mock_pi)

        completed = run_adapter_completed(
            [make_send_prompt_request(13, "empty route stdout")],
            env=self.env,
            cwd=self.workspace,
        )

        self.assertIn(
            "needle_bridge_adapter: pi-route returned empty stdout with exit 0: no stderr",
            completed.stderr,
        )
        responses = parse_adapter_responses(completed.stdout)
        result = responses[0]["result"]
        assert_prompt_response_shape(self, result)
        self.assertIn("fallback:empty route stdout", result["content"])

    def test_unknown_method_returns_json_rpc_error(self) -> None:
        responses = run_adapter(
            [{"jsonrpc": "2.0", "id": 3, "method": "list_skills", "params": None}],
            env=self.env,
            cwd=self.workspace,
        )
        self.assertEqual(responses[0]["id"], 3)
        self.assertIn("error", responses[0])
        self.assertEqual(responses[0]["error"]["code"], -32601)


if __name__ == "__main__":
    unittest.main()