#!/usr/bin/env python3
"""Compare Needle adapter path vs direct Pi JSON for curated routing prompts."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

_SCRIPTS_DIR = Path(__file__).resolve().parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from needle_bridge_adapter import forward_to_pi, workspace_root
from needle_routing_stats import load_stats, summarize_stats, stats_path

REPO_ROOT = _SCRIPTS_DIR.parent
DEFAULT_PROMPTS = _SCRIPTS_DIR / "needle_bench_prompts.json"
ADAPTER = _SCRIPTS_DIR / "needle_bridge_adapter.py"

LOCAL_PREFIXES = ("[needle:read_file]", "[needle:search_code]")


@dataclass
class PathResult:
    ok: bool
    latency_ms: float
    total_tokens: int
    prompt_tokens: int
    completion_tokens: int
    path: str
    routed_verb: str | None
    error: str | None
    content_preview: str


@dataclass
class PromptBenchResult:
    prompt_id: int
    category: str
    expected_verb: str
    prompt: str
    needle: PathResult
    pi: PathResult
    routing_match: bool | None


def load_prompt_set(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or "prompts" not in data:
        raise ValueError(f"invalid prompt set: {path}")
    return data


def filter_prompts(
    prompts: list[dict[str, Any]],
    *,
    limit: int | None,
    ids: set[int] | None,
    category: str | None,
) -> list[dict[str, Any]]:
    selected = prompts
    if ids is not None:
        selected = [item for item in selected if item.get("id") in ids]
    if category:
        selected = [item for item in selected if item.get("category") == category]
    if limit is not None:
        selected = selected[:limit]
    return selected


def make_send_prompt_request(request_id: int, prompt: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "send_prompt",
        "params": {
            "model": "needle-bench",
            "messages": [{"role": "user", "content": prompt}],
        },
    }


def classify_needle_content(content: str) -> tuple[str, str | None]:
    for prefix in LOCAL_PREFIXES:
        if content.startswith(prefix):
            verb = prefix.removeprefix("[needle:").removesuffix("]")
            return "local", verb
    return "forward", None


def run_needle_adapter(prompt: str, *, env: dict[str, str], cwd: Path) -> PathResult:
    request = make_send_prompt_request(1, prompt)
    stdin = json.dumps(request) + "\n"
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            [sys.executable, str(ADAPTER)],
            input=stdin,
            capture_output=True,
            text=True,
            cwd=str(cwd),
            env=env,
            check=False,
        )
    except OSError as exc:
        elapsed_ms = (time.perf_counter() - started) * 1000
        return PathResult(
            ok=False,
            latency_ms=elapsed_ms,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="error",
            routed_verb=None,
            error=str(exc),
            content_preview="",
        )

    elapsed_ms = (time.perf_counter() - started) * 1000
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or f"exit {completed.returncode}"
        return PathResult(
            ok=False,
            latency_ms=elapsed_ms,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="error",
            routed_verb=None,
            error=detail,
            content_preview="",
        )

    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        return PathResult(
            ok=False,
            latency_ms=elapsed_ms,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="error",
            routed_verb=None,
            error="adapter produced no stdout",
            content_preview="",
        )

    try:
        payload = json.loads(lines[-1])
    except json.JSONDecodeError as exc:
        return PathResult(
            ok=False,
            latency_ms=elapsed_ms,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="error",
            routed_verb=None,
            error=f"invalid adapter JSON: {exc}",
            content_preview="",
        )

    if "error" in payload:
        message = payload["error"].get("message", "adapter error")
        return PathResult(
            ok=False,
            latency_ms=elapsed_ms,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="error",
            routed_verb=None,
            error=str(message),
            content_preview="",
        )

    result = payload.get("result") or {}
    content = str(result.get("content", ""))
    usage = result.get("usage") or {}
    path_taken, routed_verb = classify_needle_content(content)
    return PathResult(
        ok=True,
        latency_ms=elapsed_ms,
        total_tokens=int(usage.get("total_tokens", 0) or 0),
        prompt_tokens=int(usage.get("prompt_tokens", 0) or 0),
        completion_tokens=int(usage.get("completion_tokens", 0) or 0),
        path=path_taken,
        routed_verb=routed_verb,
        error=None,
        content_preview=content[:80].replace("\n", " "),
    )


def run_pi_direct(prompt: str, *, workspace: Path) -> PathResult:
    started = time.perf_counter()
    try:
        result = forward_to_pi(prompt, workspace)
    except Exception as exc:  # noqa: BLE001
        elapsed_ms = (time.perf_counter() - started) * 1000
        return PathResult(
            ok=False,
            latency_ms=elapsed_ms,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="pi_direct",
            routed_verb=None,
            error=str(exc),
            content_preview="",
        )

    elapsed_ms = (time.perf_counter() - started) * 1000
    content = str(result.get("content", ""))
    usage = result.get("usage") or {}
    return PathResult(
        ok=True,
        latency_ms=elapsed_ms,
        total_tokens=int(usage.get("total_tokens", 0) or 0),
        prompt_tokens=int(usage.get("prompt_tokens", 0) or 0),
        completion_tokens=int(usage.get("completion_tokens", 0) or 0),
        path="pi_direct",
        routed_verb=None,
        error=None,
        content_preview=content[:80].replace("\n", " "),
    )


def expected_is_local(expected_verb: str) -> bool:
    return expected_verb in {"read_file", "search_code"}


def routing_match(expected_verb: str, needle: PathResult) -> bool | None:
    if not needle.ok:
        return None
    if expected_verb == "forward":
        return needle.path == "forward"
    if expected_is_local(expected_verb):
        return needle.path == "local" and needle.routed_verb == expected_verb
    return needle.path == "forward"


def bench_prompt(
    item: dict[str, Any],
    *,
    env: dict[str, str],
    workspace: Path,
    skip_needle: bool,
    skip_pi: bool,
) -> PromptBenchResult:
    prompt_id = int(item["id"])
    prompt = str(item["prompt"])
    category = str(item.get("category", ""))
    expected_verb = str(item.get("expected_verb", ""))

    if skip_needle:
        needle = PathResult(
            ok=False,
            latency_ms=0.0,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="skipped",
            routed_verb=None,
            error="skipped",
            content_preview="",
        )
    else:
        needle = run_needle_adapter(prompt, env=env, cwd=workspace)

    if skip_pi:
        pi = PathResult(
            ok=False,
            latency_ms=0.0,
            total_tokens=0,
            prompt_tokens=0,
            completion_tokens=0,
            path="skipped",
            routed_verb=None,
            error="skipped",
            content_preview="",
        )
    else:
        pi = run_pi_direct(prompt, workspace=workspace)

    return PromptBenchResult(
        prompt_id=prompt_id,
        category=category,
        expected_verb=expected_verb,
        prompt=prompt,
        needle=needle,
        pi=pi,
        routing_match=routing_match(expected_verb, needle) if not skip_needle else None,
    )


def fmt_ms(value: float) -> str:
    return f"{value:7.0f}"


def fmt_tokens(value: int) -> str:
    return f"{value:6d}"


def print_table(results: list[PromptBenchResult]) -> None:
    header = (
        f"{'id':>3}  {'cat':<12} {'exp':<12} {'needle':<8} {'n_ms':>7} {'n_tok':>6} "
        f"{'pi_ms':>7} {'p_tok':>6} {'d_ms':>7} {'d_tok':>6}  match  prompt"
    )
    print(header)
    print("-" * len(header))

    for row in results:
        delta_ms = ""
        delta_tok = ""
        if row.needle.ok and row.pi.ok:
            delta_ms = fmt_ms(row.needle.latency_ms - row.pi.latency_ms)
            delta_tok = fmt_tokens(row.needle.total_tokens - row.pi.total_tokens)
        needle_path = row.needle.routed_verb or row.needle.path
        match = "?" if row.routing_match is None else ("yes" if row.routing_match else "NO")
        prompt_preview = row.prompt if len(row.prompt) <= 48 else row.prompt[:45] + "..."
        print(
            f"{row.prompt_id:3d}  {row.category:<12} {row.expected_verb:<12} "
            f"{needle_path:<8} {fmt_ms(row.needle.latency_ms) if row.needle.ok else '     -':>7} "
            f"{fmt_tokens(row.needle.total_tokens) if row.needle.ok else '     -':>6} "
            f"{fmt_ms(row.pi.latency_ms) if row.pi.ok else '     -':>7} "
            f"{fmt_tokens(row.pi.total_tokens) if row.pi.ok else '     -':>6} "
            f"{delta_ms:>7} {delta_tok:>6}  {match:<5}  {prompt_preview}"
        )
        if row.needle.error and row.needle.error != "skipped":
            print(f"      needle error: {row.needle.error}")
        if row.pi.error and row.pi.error != "skipped":
            print(f"      pi error: {row.pi.error}")


def print_curated_summary(results: list[PromptBenchResult]) -> None:
    total = len(results)
    local_eligible = sum(1 for row in results if expected_is_local(row.expected_verb))
    needle_local = sum(1 for row in results if row.needle.ok and row.needle.path == "local")
    matches = [row for row in results if row.routing_match is True]
    mismatches = [row for row in results if row.routing_match is False]

    needle_ok = [row for row in results if row.needle.ok]
    pi_ok = [row for row in results if row.pi.ok]
    paired = [row for row in results if row.needle.ok and row.pi.ok]

    print()
    print("Curated set summary")
    print("-------------------")
    print(f"Prompts run:              {total}")
    print(
        f"Local-eligible (curated): {local_eligible}/{total} "
        f"({(local_eligible / total * 100) if total else 0:.1f}%)"
    )
    print(
        f"Needle executed local:    {needle_local}/{total} "
        f"({(needle_local / total * 100) if total else 0:.1f}%)"
    )
    print(f"Routing matches expected: {len(matches)}/{total}")
    if mismatches:
        ids = ", ".join(str(row.prompt_id) for row in mismatches)
        print(f"Routing mismatches:       {ids}")

    if paired:
        avg_needle_ms = sum(row.needle.latency_ms for row in paired) / len(paired)
        avg_pi_ms = sum(row.pi.latency_ms for row in paired) / len(paired)
        avg_needle_tok = sum(row.needle.total_tokens for row in paired) / len(paired)
        avg_pi_tok = sum(row.pi.total_tokens for row in paired) / len(paired)
        local_pairs = [row for row in paired if row.needle.path == "local"]
        forward_pairs = [row for row in paired if row.needle.path == "forward"]
        print(f"Avg latency needle/pi:    {avg_needle_ms:.0f}ms / {avg_pi_ms:.0f}ms")
        print(f"Avg tokens needle/pi:     {avg_needle_tok:.0f} / {avg_pi_tok:.0f}")
        if local_pairs:
            saved_ms = sum(row.pi.latency_ms - row.needle.latency_ms for row in local_pairs) / len(
                local_pairs
            )
            saved_tok = sum(row.pi.total_tokens - row.needle.total_tokens for row in local_pairs) / len(
                local_pairs
            )
            print(
                f"Local hits ({len(local_pairs)}): avg save {saved_ms:.0f}ms, "
                f"{saved_tok:.0f} tokens vs pi"
            )
        if forward_pairs:
            tax_ms = sum(row.needle.latency_ms - row.pi.latency_ms for row in forward_pairs) / len(
                forward_pairs
            )
            tax_tok = sum(row.needle.total_tokens - row.pi.total_tokens for row in forward_pairs) / len(
                forward_pairs
            )
            print(
                f"Forward hits ({len(forward_pairs)}): avg tax {tax_ms:+.0f}ms, "
                f"{tax_tok:+.0f} tokens vs pi"
            )

    if needle_ok and not pi_ok:
        print(f"Needle successes:         {len(needle_ok)}/{total}")
    if pi_ok and not needle_ok:
        print(f"Pi successes:             {len(pi_ok)}/{total}")


def print_live_usage_summary() -> None:
    path = stats_path()
    stats = load_stats(path)
    summary = summarize_stats(stats)
    print()
    print("Live adapter traffic (needle_bridge_adapter.py)")
    print("----------------------------------------------")
    if path is not None:
        print(f"Stats file: {path}")
    print(f"Updated at: {summary['updated_at'] or 'never'}")
    print(f"Total events: {summary['grand_total']}")
    if summary["grand_total"] == 0:
        print("No live traffic recorded yet.")
        return

    print(
        f"Local:   {summary['local_total']:4d}  "
        f"({summary['local_ratio'] * 100:5.1f}%)  "
        f"read_file={summary['local_breakdown']['read_file']}  "
        f"search_code={summary['local_breakdown']['search_code']}"
    )
    fb = summary["forward_breakdown"]
    print(
        f"Forward: {summary['forward_total']:4d}  "
        f"({summary['forward_ratio'] * 100:5.1f}%)  "
        f"edit={fb['edit_file']} run={fb['run_shell']} delegate={fb['delegate']} "
        f"uncertain={fb['uncertain']} route_fail={fb['route_failure']}"
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prompts", type=Path, default=DEFAULT_PROMPTS)
    parser.add_argument("--workspace", type=Path, default=None)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--ids", type=str, default=None, help="Comma-separated prompt ids")
    parser.add_argument("--category", type=str, default=None)
    parser.add_argument("--skip-needle", action="store_true")
    parser.add_argument("--skip-pi", action="store_true")
    parser.add_argument("--json-out", type=Path, default=None)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    prompt_set = load_prompt_set(args.prompts)
    workspace = args.workspace or workspace_root()
    workspace = workspace.resolve()

    ids = None
    if args.ids:
        ids = {int(part.strip()) for part in args.ids.split(",") if part.strip()}

    prompts = filter_prompts(
        prompt_set["prompts"],
        limit=args.limit,
        ids=ids,
        category=args.category,
    )
    if not prompts:
        print("No prompts selected.", file=sys.stderr)
        return 1

    env = os.environ.copy()
    env["WORKSPACE"] = str(workspace)
    env.setdefault("NEEDLE_USAGE_LOGGING", "1")

    print(f"Workspace: {workspace}")
    print(f"Prompts:   {len(prompts)} from {args.prompts}")
    if args.skip_pi:
        print("Mode:      needle only")
    elif args.skip_needle:
        print("Mode:      pi direct only")
    else:
        print("Mode:      needle + pi direct")

    results: list[PromptBenchResult] = []
    for index, item in enumerate(prompts, start=1):
        print(f"\rRunning {index}/{len(prompts)}: id={item['id']}...", end="", flush=True)
        results.append(
            bench_prompt(
                item,
                env=env,
                workspace=workspace,
                skip_needle=args.skip_needle,
                skip_pi=args.skip_pi,
            )
        )
    print("\r" + " " * 60 + "\r", end="")

    print_table(results)
    print_curated_summary(results)
    print_live_usage_summary()

    if args.json_out is not None:
        payload = {
            "workspace": str(workspace),
            "prompts_file": str(args.prompts),
            "results": [
                {
                    **{k: v for k, v in asdict(row).items() if k not in {"needle", "pi"}},
                    "needle": asdict(row.needle),
                    "pi": asdict(row.pi),
                }
                for row in results
            ],
            "live_usage": summarize_stats(),
        }
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        print(f"\nWrote JSON results to {args.json_out}")

    failures = sum(1 for row in results if not row.needle.ok and not args.skip_needle)
    failures += sum(1 for row in results if not row.pi.ok and not args.skip_pi)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())