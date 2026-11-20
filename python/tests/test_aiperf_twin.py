# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import json
import tempfile
import unittest
from pathlib import Path

from zyvor_janus.benchmarks.aiperf_adapter import extract_profile_curves, parse_aiperf_results, update_profile_yaml
from zyvor_janus.benchmarks.sweep import cartesian_variants
from zyvor_janus.benchmarks.twin_store import TwinEntry, TwinStore

ROOT = Path(__file__).resolve().parents[2]


class AIPerfAdapterTests(unittest.TestCase):
    def test_import_curves(self) -> None:
        data = parse_aiperf_results(ROOT / "tests/fixtures/aiperf/sample_result.json")
        curves = extract_profile_curves(data, model="llama-70b", gpu_type="H100")
        self.assertIn("prefill_ms_per_token", curves)
        self.assertIn("decode_tps", curves)

    def test_update_profile_yaml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = update_profile_yaml(
                Path(tmp),
                model="llama-test",
                gpu_type="H100",
                curves={"prefill_ms_per_token": 0.1, "decode_tps": 100.0, "max_batch": 32},
            )
            payload = path.read_text()
            self.assertIn("prefill_ms_per_token", payload)


class SweepTests(unittest.TestCase):
    def test_cartesian_product(self) -> None:
        combos = cartesian_variants(scheduler=["fifo", "preemptive"], cluster=["a"])
        self.assertEqual(len(combos), 2)


class TwinStoreTests(unittest.TestCase):
    def test_crud_and_drift(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = TwinStore(Path(tmp))
            store.upsert(
                TwinEntry("H100", "llama-70b", 50.0, 95.0, 10.0, TwinStore.now_iso(), "run-1")
            )
            self.assertFalse(store.detect_drift("H100", "llama-70b", 52.0))
            self.assertTrue(store.detect_drift("H100", "llama-70b", 80.0))


if __name__ == "__main__":
    unittest.main()
