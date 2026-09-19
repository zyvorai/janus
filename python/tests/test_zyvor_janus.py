# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Tests for Zyvor Janus Python bindings, adapters, and Zynera bundle ingest."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / "configs" / "clusters" / "small_h100.yaml"
FIXTURES = ROOT / "tests" / "fixtures" / "zynera"
TRACE_FIXTURES = ROOT / "tests" / "fixtures" / "traces"
PROFILES = ROOT / "configs" / "profiles"


class TestZyneraCRDAdapter(unittest.TestCase):
    def test_gang_gpu_count_is_32(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job, gpu_count_from_spec

        spec = {
            "gpus": 8,
            "distributed": {"enabled": True, "nodes": 4, "gpusPerNode": 8},
        }
        self.assertEqual(gpu_count_from_spec(spec), 32)

        manifest = {
            "metadata": {
                "name": "gpt-distributed-training",
                "namespace": "ml-training",
                "annotations": {
                    "zynera.ai/gang-schedule": "true",
                    "zynera.ai/gang-size": "4",
                },
            },
            "spec": {
                **spec,
                "model": "gpt-13b",
                "gpuType": "H100",
                "priority": 80,
                "network": "rdma",
            },
        }
        quotas = [
            {
                "spec": {
                    "team": "ml-training",
                    "namespaces": ["ml-training"],
                }
            }
        ]
        job = fabric_ai_job_to_job(
            manifest,
            quotas=quotas,
            runtime_seconds=100.0,
            gpu_memory_gb=80.0,
        )
        self.assertEqual(job["gpu_count"], 32)
        self.assertEqual(job["tenant"], "ml-training")
        self.assertEqual(job["priority"], 80)
        self.assertTrue(job["gang_enabled"])
        self.assertEqual(job["gang_size_nodes"], 4)

    def test_gang_timeout_parsed_from_annotation(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job

        manifest = {
            "metadata": {
                "name": "gang-job",
                "namespace": "default",
                "annotations": {
                    "zynera.ai/gang-schedule": "true",
                    "zynera.ai/gang-size": "2",
                    "zynera.ai/gang-timeout": "10m",
                },
            },
            "spec": {"gpus": 4, "model": "gpt-13b", "gpuType": "H100"},
        }
        job = fabric_ai_job_to_job(
            manifest, runtime_seconds=100.0, gpu_memory_gb=80.0
        )
        self.assertEqual(job["gang_timeout_secs"], 600.0)

    def test_tenant_from_fabric_quota_not_job_spec(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job, resolve_tenant

        quotas = [
            {"spec": {"team": "ml-infra", "namespaces": ["ml-infra"]}},
        ]
        self.assertEqual(resolve_tenant("ml-infra", quotas), "ml-infra")
        self.assertIsNone(resolve_tenant("other-ns", quotas))

        manifest = {
            "metadata": {"name": "job-a", "namespace": "ml-infra"},
            "spec": {"gpus": 1, "model": "llama-70b", "gpuType": "H100"},
        }
        job = fabric_ai_job_to_job(
            manifest, quotas=quotas, runtime_seconds=10.0, gpu_memory_gb=80.0
        )
        self.assertEqual(job["tenant"], "ml-infra")


class TestProfileRegistry(unittest.TestCase):
    def test_fail_on_missing_model(self) -> None:
        from zyvor_janus.adapters.profiles import ProfileLookupError, ProfileRegistry

        registry = ProfileRegistry(PROFILES)
        with self.assertRaises(ProfileLookupError):
            registry.lookup("unknown-model", "H100")


class TestZyneraBundleAdapter(unittest.TestCase):
    def test_load_fixture_bundle(self) -> None:
        from zyvor_janus.adapters.bundle import ZyneraBundleAdapter

        adapter = ZyneraBundleAdapter(PROFILES)
        bundle = adapter.from_directory(FIXTURES)
        self.assertEqual(len(bundle.jobs), 3)
        gang = next(j for j in bundle.jobs if j["name"] == "gpt-distributed-training")
        self.assertEqual(gang["gpu_count"], 32)
        self.assertEqual(gang["tenant"], "ml-training")

    def test_missing_profile_raises(self) -> None:
        from zyvor_janus.adapters.bundle import ZyneraBundleAdapter
        from zyvor_janus.adapters.profiles import ProfileLookupError

        adapter = ZyneraBundleAdapter(Path("/nonexistent/profiles"))
        with self.assertRaises(ProfileLookupError):
            adapter.from_directory(FIXTURES)


class TestTraceAdapter(unittest.TestCase):
    def test_load_fixture_trace(self) -> None:
        from zyvor_janus.adapters.trace import TraceAdapter

        adapter = TraceAdapter()
        record = adapter.from_file(TRACE_FIXTURES / "fifo_match.jsonl")
        self.assertEqual(len(record.events), 4)

    def test_jobs_and_oracle_from_trace(self) -> None:
        from zyvor_janus.adapters.trace import TraceAdapter

        adapter = TraceAdapter()
        record = adapter.from_file(TRACE_FIXTURES / "fifo_match.jsonl")
        jobs = adapter.jobs_from_events(record.events)
        oracle = adapter.oracle_schedules(record.events)
        self.assertEqual(len(jobs), 2)
        self.assertEqual(len(oracle), 2)
        self.assertEqual(oracle[0]["gpu_ids"], ["gpu-0"])

    def test_normalizes_indexed_gpus(self) -> None:
        from zyvor_janus.adapters.trace import TraceAdapter

        adapter = TraceAdapter()
        record = adapter.from_file(TRACE_FIXTURES / "indexed_gpus.jsonl")
        oracle = adapter.oracle_schedules(record.events)
        self.assertEqual(
            oracle[0]["gpu_ids"],
            ["node-0-gpu-0", "node-0-gpu-1", "node-0-gpu-2", "node-0-gpu-3"],
        )

    def test_diff_detects_mismatch(self) -> None:
        from zyvor_janus.adapters.trace import TraceAdapter

        adapter = TraceAdapter()
        oracle = [{"job_id": "j1", "timestamp": 0.0, "gpu_ids": ["gpu-1"]}]
        simulated = [{"job_id": "j1", "start_time": 0.0, "gpu_ids": ["gpu-0"]}]
        diffs = adapter.diff_placements(oracle, simulated)
        self.assertFalse(diffs[0]["placement_match"])


@unittest.skipUnless(CONFIG.exists(), "sample config not present")
class TestRunFromConfig(unittest.TestCase):
    def test_run_from_config(self) -> None:
        try:
            import zyvor_janus._zyvor_janus  # noqa: F401
        except ImportError:
            self.skipTest("zyvor_janus Rust extension not built (maturin develop)")

        import zyvor_janus

        result = zyvor_janus.run_from_config(str(CONFIG))
        self.assertEqual(result.jobs_completed, result.jobs_total)
        self.assertGreater(result.makespan, 0)


if __name__ == "__main__":
    unittest.main()
