# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for Zyvor Janus Python adapters (isolated, no CLI/Rust extension)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests" / "fixtures" / "zynera"
PROFILES = ROOT / "configs" / "profiles"


class TestGpuCountFromSpec(unittest.TestCase):
    def test_non_distributed_uses_spec_gpus(self) -> None:
        from zyvor_janus.adapters.crd import gpu_count_from_spec

        self.assertEqual(gpu_count_from_spec({"gpus": 4}), 4)

    def test_distributed_uses_nodes_times_gpus_per_node(self) -> None:
        from zyvor_janus.adapters.crd import gpu_count_from_spec

        spec = {
            "gpus": 8,
            "distributed": {"enabled": True, "nodes": 4, "gpusPerNode": 8},
        }
        self.assertEqual(gpu_count_from_spec(spec), 32)

    def test_mig_uses_count_not_spec_gpus(self) -> None:
        from zyvor_janus.adapters.crd import gpu_count_from_spec

        spec = {"gpus": 8, "mig": {"profile": "1g.10gb", "count": 2}}
        self.assertEqual(gpu_count_from_spec(spec), 2)


class TestResolveTenant(unittest.TestCase):
    def test_matches_namespace_list(self) -> None:
        from zyvor_janus.adapters.crd import resolve_tenant

        quotas = [{"spec": {"team": "team-a", "namespaces": ["ns-a", "ns-b"]}}]
        self.assertEqual(resolve_tenant("ns-b", quotas), "team-a")
        self.assertIsNone(resolve_tenant("other", quotas))


class TestFabricAIJobMapping(unittest.TestCase):
    def test_mig_fields_mapped(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job

        manifest = {
            "metadata": {"name": "mig-inference", "namespace": "ml-infra"},
            "spec": {
                "gpus": 8,
                "mig": {"profile": "1g.10gb", "count": 2},
                "priority": 50,
            },
        }
        job = fabric_ai_job_to_job(
            manifest, runtime_seconds=10.0, gpu_memory_gb=10.0
        )
        self.assertEqual(job["gpu_count"], 2)
        self.assertEqual(job["mig_profile"], "1g.10gb")
        self.assertEqual(job["mig_count"], 2)

    def test_network_rdma_hint(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job

        manifest = {
            "metadata": {"name": "j", "namespace": "default"},
            "spec": {"gpus": 1, "network": "rdma"},
        }
        job = fabric_ai_job_to_job(manifest, runtime_seconds=1.0, gpu_memory_gb=1.0)
        self.assertEqual(job["network_bw_gbps"], 400.0)

    def test_no_runtime_when_not_provided(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job

        manifest = {
            "metadata": {"name": "j", "namespace": "default"},
            "spec": {"gpus": 1},
        }
        job = fabric_ai_job_to_job(manifest)
        self.assertNotIn("runtime", job)

    def test_site_label_carried_onto_the_job(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job

        manifest = {
            "metadata": {
                "name": "j",
                "namespace": "default",
                "labels": {"zynera.ai/federated-training-site": "site-a"},
            },
            "spec": {"gpus": 1},
        }
        job = fabric_ai_job_to_job(manifest)
        self.assertEqual(job["site"], "site-a")

    def test_no_site_label_means_no_site(self) -> None:
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job

        manifest = {
            "metadata": {"name": "j", "namespace": "default"},
            "spec": {"gpus": 1},
        }
        job = fabric_ai_job_to_job(manifest)
        self.assertIsNone(job["site"])


class TestProfileRegistry(unittest.TestCase):
    def test_lookup_known_model(self) -> None:
        from zyvor_janus.adapters.profiles import ProfileRegistry

        registry = ProfileRegistry(PROFILES)
        runtime, memory = registry.lookup("gpt-13b", "H100")
        self.assertGreater(runtime, 0)
        self.assertEqual(memory, 80.0)


class TestSimpleYaml(unittest.TestCase):
    def test_loads_profile_fixture(self) -> None:
        from zyvor_janus.adapters.simple_yaml import safe_load

        data = safe_load((PROFILES / "gpt-13b.yaml").read_text())
        assert data is not None
        self.assertEqual(data["model"], "gpt-13b")
        self.assertIn("H100", data["profiles"])


class TestZyneraBundleAdapterUnit(unittest.TestCase):
    def test_rejects_empty_jobs_dir(self) -> None:
        from zyvor_janus.adapters.bundle import ZyneraBundleAdapter

        adapter = ZyneraBundleAdapter(PROFILES)
        empty = ROOT / "tests" / "fixtures" / "traces"
        with self.assertRaises(ValueError):
            adapter.from_directory(empty)

    def test_mig_job_in_fixture(self) -> None:
        from zyvor_janus.adapters.bundle import ZyneraBundleAdapter

        adapter = ZyneraBundleAdapter(PROFILES)
        bundle = adapter.from_directory(FIXTURES)
        mig = next(j for j in bundle.jobs if j["name"] == "mig-inference")
        self.assertEqual(mig["gpu_count"], 2)

    def test_node_sites_and_federation_run_are_recognized(self) -> None:
        from zyvor_janus.adapters.bundle import ZyneraBundleAdapter

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "jobs").mkdir()
            (root / "cluster").mkdir()
            (root / "federation").mkdir()

            (root / "jobs" / "job.yaml").write_text(
                "apiVersion: zynera.ai/v1\n"
                "kind: FabricAIJob\n"
                "metadata:\n"
                "  name: j\n"
                "  namespace: default\n"
                "  labels:\n"
                "    zynera.ai/federated-training-site: site-a\n"
                "spec:\n"
                "  gpus: 1\n"
            )
            (root / "cluster" / "nodes.yaml").write_text(
                "apiVersion: zynera.ai/v1\n"
                "kind: FabricGpuNode\n"
                "metadata:\n"
                "  name: n0\n"
                "  labels:\n"
                "    zynera.ai/federated-training-site: site-a\n"
                "spec:\n"
                "  nodeName: n0\n"
                "  gpuType: any\n"
                "  gpuCount: 1\n"
            )
            (root / "federation" / "run.yaml").write_text(
                "apiVersion: zynera.ai/v1\n"
                "kind: FabricFederatedTrainingRun\n"
                "metadata:\n"
                "  name: run-a\n"
                "spec:\n"
                "  targetClusters: [site-a, site-b]\n"
                "  secureAggregation: true\n"
                "  dropoutRecovery: true\n"
            )

            adapter = ZyneraBundleAdapter()
            bundle = adapter.from_directory(root)

            self.assertEqual(bundle.jobs[0]["site"], "site-a")
            self.assertEqual(bundle.node_sites, {"n0": "site-a"})
            self.assertEqual(len(bundle.federation), 1)
            self.assertEqual(bundle.federation[0]["metadata"]["name"], "run-a")


if __name__ == "__main__":
    unittest.main()
