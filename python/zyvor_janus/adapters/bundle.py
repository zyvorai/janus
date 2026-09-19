# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Load Zynera export bundles (jobs/, cluster/, quotas/)."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover - fallback when PyYAML is unavailable
    from zyvor_janus.adapters import simple_yaml as yaml

from zyvor_janus.adapters.crd import fabric_ai_job_to_job, site_of
from zyvor_janus.adapters.profiles import ProfileLookupError, ProfileRegistry

ZYNERA_API_VERSION = "zynera.ai/v1"


def _yaml_documents(content: str) -> list[dict[str, Any]]:
    if hasattr(yaml, "safe_load_all"):
        docs = list(yaml.safe_load_all(content))
    else:
        docs = yaml.safe_load_all(content)
    return [d for d in docs if isinstance(d, dict)]


def _collect_yaml_files(directory: Path) -> list[Path]:
    if not directory.exists():
        return []
    files = sorted(directory.glob("*.yaml")) + sorted(directory.glob("*.yml"))
    return files


def _validate_api_version(doc: dict[str, Any]) -> None:
    api = doc.get("apiVersion")
    if api != ZYNERA_API_VERSION:
        raise ValueError(f"unsupported apiVersion '{api}', expected '{ZYNERA_API_VERSION}'")


@dataclass
class ZyneraBundle:
    jobs: list[dict[str, Any]] = field(default_factory=list)
    quotas: list[dict[str, Any]] = field(default_factory=list)
    gpu_nodes: list[dict[str, Any]] = field(default_factory=list)
    # Federation site tag per node name (from FEDERATION_SITE_LABEL), mirrors
    # zyvor-janus-config's Cluster.node_sites. Nodes with no entry are unpartitioned.
    node_sites: dict[str, str] = field(default_factory=dict)
    # FabricFederatedTrainingRun docs found under federation/, if any —
    # informational only (mirrors zyvor-janus-config's ZyneraBundle.federation),
    # recognized rather than silently dropped like any other unknown kind.
    federation: list[dict[str, Any]] = field(default_factory=list)


class ZyneraBundleAdapter:
    def __init__(self, profiles_dir: Path | None = None) -> None:
        self.profiles = ProfileRegistry(profiles_dir) if profiles_dir else None

    def from_directory(self, path: str | Path) -> ZyneraBundle:
        root = Path(path)
        bundle = ZyneraBundle()

        for file in _collect_yaml_files(root / "quotas"):
            for doc in _yaml_documents(file.read_text()):
                if doc.get("kind") == "FabricQuota":
                    _validate_api_version(doc)
                    bundle.quotas.append(doc)

        for file in _collect_yaml_files(root / "cluster"):
            for doc in _yaml_documents(file.read_text()):
                if doc.get("kind") == "FabricGpuNode":
                    _validate_api_version(doc)
                    bundle.gpu_nodes.append(doc)
                    site = site_of(doc)
                    node_name = (doc.get("spec") or {}).get("nodeName")
                    if site and node_name:
                        bundle.node_sites[node_name] = site

        for file in _collect_yaml_files(root / "federation"):
            for doc in _yaml_documents(file.read_text()):
                if doc.get("kind") == "FabricFederatedTrainingRun":
                    _validate_api_version(doc)
                    bundle.federation.append(doc)

        for file in _collect_yaml_files(root / "jobs"):
            for doc in _yaml_documents(file.read_text()):
                if doc.get("kind") != "FabricAIJob":
                    continue
                _validate_api_version(doc)
                spec = doc.get("spec") or {}
                model = spec.get("model") or doc.get("metadata", {}).get("name", "unknown")
                gpu_type = spec.get("gpuType", "any")

                runtime: float | None = None
                memory: float | None = None
                if self.profiles is not None:
                    runtime, memory = self.profiles.lookup(str(model), str(gpu_type))

                job = fabric_ai_job_to_job(
                    doc,
                    quotas=bundle.quotas,
                    runtime_seconds=runtime,
                    gpu_memory_gb=memory,
                )
                bundle.jobs.append(job)

        if not bundle.jobs:
            raise ValueError(f"no FabricAIJob documents found in {root / 'jobs'}")

        return bundle
