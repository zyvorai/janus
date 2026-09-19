# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Integration tests: invoke zyvor-janus CLI and verify end-to-end outputs."""

from __future__ import annotations

import json
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUTPUTS = ROOT / "outputs"


def _cargo_available() -> bool:
    return shutil.which("cargo") is not None


def _zyvor_janus(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["cargo", "run", "-q", "-p", "zyvor-janus-cli", "--", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


@unittest.skipUnless(_cargo_available(), "cargo not available")
class TestZyvorJanusCliIntegration(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        OUTPUTS.mkdir(exist_ok=True)

    def test_run_synthetic_config(self) -> None:
        out = OUTPUTS / "integration_metrics.json"
        result = _zyvor_janus(
            "run",
            "--config",
            "configs/clusters/small_h100.yaml",
            "--output",
            str(out),
        )
        self.assertEqual(
            result.returncode,
            0,
            msg=result.stderr,
        )
        self.assertTrue(out.exists())
        metrics = json.loads(out.read_text())
        self.assertEqual(metrics["jobs_completed"], metrics["jobs_total"])
        self.assertGreater(metrics["makespan"], 0)

    def test_run_zynera_bundle(self) -> None:
        result = _zyvor_janus(
            "run",
            "--zynera-bundle",
            "tests/fixtures/zynera",
            "--profiles-dir",
            "configs/profiles",
        )
        self.assertEqual(result.returncode, 0, msg=result.stderr)
        self.assertIn("jobs completed", result.stdout)

    def test_replay_trace_zero_diffs(self) -> None:
        out = OUTPUTS / "integration_trace_diff.json"
        result = _zyvor_janus(
            "replay",
            "--trace",
            "tests/fixtures/traces/fifo_match.jsonl",
            "--config",
            "configs/clusters/single_gpu.yaml",
            "--output",
            str(out),
        )
        self.assertEqual(result.returncode, 0, msg=result.stderr)
        report = json.loads(out.read_text())
        self.assertEqual(report["differing_placements"], 0)
        self.assertEqual(report["matching_placements"], 2)

    def test_run_mig_workload(self) -> None:
        result = _zyvor_janus("run", "--config", "configs/clusters/mig_single.yaml")
        self.assertEqual(result.returncode, 0, msg=result.stderr)
        self.assertIn("mig reconfigs:", result.stdout)

    def test_run_without_input_fails(self) -> None:
        result = _zyvor_janus("run")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
