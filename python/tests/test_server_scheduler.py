# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Tests for FastAPI scheduler override and replay snapshots."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock, patch

try:
    from zyvor_janus.server import app as server_app

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


if __name__ == "__main__":
    unittest.main()
