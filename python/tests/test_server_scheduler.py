# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Tests for FastAPI scheduler override and replay snapshots."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock, patch

import importlib

try:
    # `zyvor_janus.server` re-exports the FastAPI `app` object under the same
    # name as the `app` submodule, so a `from ... import app` yields the
    # object, not the module. Import the module explicitly.
    server_app = importlib.import_module("zyvor_janus.server.app")

    HAS_SERVER = True
except ImportError:
    HAS_SERVER = False


@unittest.skipUnless(HAS_SERVER, "fastapi server deps not installed")
class TestServerScheduler(unittest.TestCase):
    @patch("zyvor_janus._zyvor_janus.run_report_from_config")
    def test_run_simulation_passes_scheduler_override(self, mock_run) -> None:
        mock_metrics = MagicMock()
        mock_metrics.to_json.return_value = "{}"
        mock_run.return_value = {
            "metrics": mock_metrics,
            "timeline": "{}",
            "decisions": [],
            "snapshots": [{"clock": 0.0}],
            "config_hash": "abc",
            "scheduler": "preemptive",
        }

        result = server_app._run_simulation_sync("preemption_preemptive.yaml", "preemptive")

        mock_run.assert_called_once()
        args = mock_run.call_args[0]
        self.assertIn("preemption_preemptive.yaml", args[0])
        self.assertEqual(args[1], "preemptive")
        self.assertEqual(len(result["snapshots"]), 1)
        self.assertEqual(result["resolved_scheduler"], "preemptive")

    @patch("zyvor_janus._zyvor_janus.run_report_from_config")
    def test_quality_chargeback_and_slo_are_passed_through(self, mock_run) -> None:
        mock_metrics = MagicMock()
        mock_metrics.to_json.return_value = "{}"
        mock_run.return_value = {
            "metrics": mock_metrics,
            "timeline": "{}",
            "decisions": [],
            "snapshots": [],
            "config_hash": "abc",
            "scheduler": "sjf",
            "quality": '{"jct_mean": 12.5}',
            "slo": '{"attainment": 0.5}',
        }
        result = server_app._run_simulation_sync("preemption_preemptive.yaml", "sjf")
        self.assertEqual(result["quality"]["jct_mean"], 12.5)
        self.assertEqual(result["slo"]["attainment"], 0.5)
        self.assertIsNone(result["chargeback"])


if __name__ == "__main__":
    unittest.main()
