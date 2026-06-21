#!/usr/bin/env python3
"""Tests for needle routing stats helpers."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from needle_routing_stats import load_stats, record_routing_event, summarize_stats


class NeedleRoutingStatsTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.stats_file = Path(self.temp_dir.name) / "stats.json"
        self.events_file = Path(self.temp_dir.name) / "events.jsonl"
        self.env = os.environ.copy()
        self.env["NEEDLE_USAGE_STATS"] = str(self.stats_file)
        self.env["NEEDLE_USAGE_EVENTS"] = str(self.events_file)
        self.env["NEEDLE_USAGE_LOGGING"] = "1"

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_record_local_and_forward_counts(self) -> None:
        with unittest.mock.patch.dict(os.environ, self.env, clear=False):
            record_routing_event(
                decision="local",
                verb="read_file",
                route={"name": "read_file", "_route_source": "test"},
                prompt_preview="read AGENTS.md",
            )
            record_routing_event(
                decision="forward",
                verb="edit_file",
                route={"name": "edit_file", "_route_source": "test"},
                prompt_preview="edit something",
            )
            record_routing_event(
                decision="forward",
                verb="route_failure",
                route=None,
                prompt_preview="fuzzy ask",
            )

        stats = load_stats(self.stats_file)
        self.assertEqual(stats["local"]["read_file"], 1)
        self.assertEqual(stats["local"]["total"], 1)
        self.assertEqual(stats["forward"]["edit_file"], 1)
        self.assertEqual(stats["forward"]["route_failure"], 1)
        self.assertEqual(stats["forward"]["total"], 2)

        summary = summarize_stats(stats)
        self.assertEqual(summary["grand_total"], 3)
        self.assertAlmostEqual(summary["local_ratio"], 1 / 3)
        self.assertAlmostEqual(summary["forward_ratio"], 2 / 3)

        events = self.events_file.read_text(encoding="utf-8").strip().splitlines()
        self.assertEqual(len(events), 3)
        first = json.loads(events[0])
        self.assertEqual(first["decision"], "local")
        self.assertEqual(first["verb"], "read_file")


if __name__ == "__main__":
    unittest.main()