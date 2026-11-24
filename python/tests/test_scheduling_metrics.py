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


BACKFILL_CONFIG = ROOT / "configs/clusters/backfill_demo.yaml"


@unittest.skipUnless(BACKFILL_CONFIG.exists(), "backfill config missing")
class TestRound3Reports(unittest.TestCase):
    def _report(self, scheduler: str) -> dict:
        try:
            from zyvor_janus import _zyvor_janus
        except ImportError:
            self.skipTest("zyvor_janus extension not built")
        return _zyvor_janus.run_report_from_config(str(BACKFILL_CONFIG), scheduler)

    def test_report_exposes_quality_chargeback_and_slo(self) -> None:
        import json

        report = self._report("easy")
        quality = json.loads(report["quality"])
        self.assertEqual(quality["jct_mean"], 234.25)
        self.assertEqual(quality["jobs_considered"], 4)
        slo = json.loads(report["slo"])
        self.assertEqual((slo["jobs_total"], slo["jobs_met"]), (4, 2))
        self.assertEqual(slo["attainment"], 0.5)
        self.assertIn("tenants", json.loads(report["chargeback"]))

    def test_new_schedulers_run_through_the_bindings(self) -> None:
        for name in ("sjf", "fairshare", "easy"):
            report = self._report(name)
            self.assertEqual(report["scheduler"], name)
            self.assertEqual(report["metrics"].jobs_completed, 4)

    def test_unknown_scheduler_is_an_error(self) -> None:
        with self.assertRaises(RuntimeError):
            self._report("no-such-scheduler")


if __name__ == "__main__":
    unittest.main()
