"""Persistent routing counters for needle_bridge_adapter live traffic."""

from __future__ import annotations

import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

DEFAULT_STATS_PATH = Path.home() / ".pi-hybrid" / "needle_route_stats.json"
DEFAULT_EVENTS_PATH = Path.home() / ".pi-hybrid" / "needle_route_events.jsonl"


def stats_enabled() -> bool:
    return os.environ.get("NEEDLE_USAGE_LOGGING", "1").strip().lower() not in {
        "0",
        "false",
        "no",
        "off",
    }


def stats_path() -> Path | None:
    raw = os.environ.get("NEEDLE_USAGE_STATS", str(DEFAULT_STATS_PATH)).strip()
    if not raw:
        return None
    return Path(raw).expanduser()


def events_path() -> Path | None:
    raw = os.environ.get("NEEDLE_USAGE_EVENTS", str(DEFAULT_EVENTS_PATH)).strip()
    if not raw:
        return None
    return Path(raw).expanduser()


def empty_stats() -> dict[str, Any]:
    return {
        "local": {"read_file": 0, "search_code": 0, "total": 0},
        "forward": {
            "edit_file": 0,
            "run_shell": 0,
            "delegate": 0,
            "uncertain": 0,
            "route_failure": 0,
            "total": 0,
        },
        "updated_at": None,
    }


def load_stats(path: Path | None = None) -> dict[str, Any]:
    target = path or stats_path()
    if target is None or not target.is_file():
        return empty_stats()
    try:
        data = json.loads(target.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return empty_stats()
    if not isinstance(data, dict):
        return empty_stats()
    base = empty_stats()
    for section in ("local", "forward"):
        section_data = data.get(section)
        if isinstance(section_data, dict):
            for key, value in section_data.items():
                if key in base[section] and isinstance(value, int):
                    base[section][key] = value
    base["updated_at"] = data.get("updated_at")
    return base


def classify_forward_verb(route: dict[str, Any] | None) -> str:
    if route is None:
        return "route_failure"
    name = route.get("name")
    if not isinstance(name, str) or not name:
        return "uncertain"
    if name in {"edit_file", "run_shell", "delegate"}:
        return name
    return "uncertain"


def record_routing_event(
    *,
    decision: str,
    verb: str,
    route: dict[str, Any] | None = None,
    prompt_preview: str = "",
) -> None:
    if not stats_enabled():
        return

    target = stats_path()
    if target is None:
        return

    stats = load_stats(target)
    if decision == "local":
        if verb in stats["local"]:
            stats["local"][verb] += 1
        stats["local"]["total"] += 1
    else:
        forward_verb = classify_forward_verb(route) if route is not None or verb == "forward" else verb
        if forward_verb not in stats["forward"]:
            forward_verb = "uncertain"
        stats["forward"][forward_verb] += 1
        stats["forward"]["total"] += 1

    stats["updated_at"] = datetime.now(timezone.utc).isoformat()
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")

    events_target = events_path()
    if events_target is not None:
        event = {
            "ts": stats["updated_at"],
            "decision": decision,
            "verb": verb,
            "forward_verb": classify_forward_verb(route) if decision == "forward" else None,
            "route_source": route.get("_route_source") if isinstance(route, dict) else None,
            "prompt_preview": prompt_preview[:120],
        }
        events_target.parent.mkdir(parents=True, exist_ok=True)
        with events_target.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(event, separators=(",", ":")) + "\n")


def summarize_stats(stats: dict[str, Any] | None = None) -> dict[str, Any]:
    data = stats if stats is not None else load_stats()
    local_total = int(data["local"]["total"])
    forward_total = int(data["forward"]["total"])
    grand_total = local_total + forward_total
    local_ratio = (local_total / grand_total) if grand_total else 0.0
    forward_ratio = (forward_total / grand_total) if grand_total else 0.0
    return {
        "local_total": local_total,
        "forward_total": forward_total,
        "grand_total": grand_total,
        "local_ratio": local_ratio,
        "forward_ratio": forward_ratio,
        "local_breakdown": dict(data["local"]),
        "forward_breakdown": dict(data["forward"]),
        "updated_at": data.get("updated_at"),
    }