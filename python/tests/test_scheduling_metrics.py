# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Unit tests for scheduling metrics exposed via the Python bindings."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PREEMPTIVE_CONFIG = ROOT / "configs/clusters/preemption_preemptive.yaml"


@unittest.skipUnless(PREEMPTIVE_CONFIG.exists(), "preemptive config missing")
class TestSchedulingMetrics(unittest.TestCase):
    def test_preemptive_run_reports_segment_metrics(self) -> None:
        try:
            import zyvor_janus
            from zyvor_janus import _zyvor_janus
        except ImportError:
            self.skipTest("zyvor_janus extension not built")

        metrics = _zyvor_janus.run_from_config(str(PREEMPTIVE_CONFIG))
        self.assertEqual(metrics.preemptions, 1)
        self.assertGreater(metrics.gpu_utilization, 0.0)
        self.assertGreaterEqual(metrics.jobs_completed, 1)

    def test_sim_result_json_includes_new_fields(self) -> None:
        try:
            from zyvor_janus import _zyvor_janus
        except ImportError:
            self.skipTest("zyvor_janus extension not built")

        metrics = _zyvor_janus.run_from_config(str(PREEMPTIVE_CONFIG))
        payload = metrics.to_json()
        self.assertIn("jobs_unschedulable", payload)
        self.assertIn("queue_max_length", payload)


if __name__ == "__main__":
    unittest.main()
