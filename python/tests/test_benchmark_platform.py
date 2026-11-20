# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import unittest

from zyvor_janus.adapters.profiles import ProfileRegistry
from zyvor_janus.adapters.serving_trace import SERVING_TRACE_VERSION, from_aiperf_rows, load_serving_trace
from zyvor_janus.workloads.generate_synthetic import generate_jobs, validate_jobs
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests" / "fixtures"


class SyntheticWorkloadTests(unittest.TestCase):
    def test_deterministic_seed(self) -> None:
        a = generate_jobs("peak_chat", seed=7)
        b = generate_jobs("peak_chat", seed=7)
        self.assertEqual([j.arrival_time for j in a], [j.arrival_time for j in b])
        self.assertGreater(len(a), 0)

    def test_rejects_invalid_tokens(self) -> None:
        jobs = generate_jobs("morning_rag", seed=1)
        jobs[0].input_tokens = 0
        jobs[0].output_tokens = 0
        with self.assertRaises(ValueError):
            validate_jobs(jobs)


class ServingTraceAdapterTests(unittest.TestCase):
    def test_load_fixture_jsonl(self) -> None:
        trace = load_serving_trace(FIXTURES / "traces" / "serving_llama.jsonl")
        self.assertEqual(trace["version"], SERVING_TRACE_VERSION)
        self.assertEqual(len(trace["records"]), 1)

    def test_aiperf_mapping_roundtrip(self) -> None:
        trace = load_serving_trace(FIXTURES / "traces" / "serving_llama.jsonl")
        rows = [{"timestamp": r["time"], "model": r["model"], "input_sequence_length": r["input_tokens"], "output_sequence_length": r["output_tokens"]} for r in trace["records"]]
        mapped = from_aiperf_rows(rows)
        self.assertEqual(mapped["records"][0]["input_tokens"], 512)


class ProfileV2Tests(unittest.TestCase):
    def test_lookup_v2(self) -> None:
        reg = ProfileRegistry(ROOT / "configs" / "profiles")
        prefill, decode = reg.lookup_v2("llama-70b", "H100")
        self.assertGreater(prefill, 0)
        self.assertGreater(decode, 0)


if __name__ == "__main__":
    unittest.main()
