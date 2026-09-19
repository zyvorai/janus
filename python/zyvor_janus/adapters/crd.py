# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Zynera CRD → internal job mapping (FabricAIJob, FabricQuota)."""

from __future__ import annotations

from typing import Any

# Real label the ai-operator's federated-training-run controller sets on
# every per-site FabricAIJob it creates (fabricfederatedtrainingrun_controller.go).
# Reused here on FabricGpuNode fixtures too, to tag which federation site a
# node belongs to. Mirrors zyvor-janus-config/src/zynera_bundle.rs.
FEDERATION_SITE_LABEL = "zynera.ai/federated-training-site"


def gpu_count_from_spec(spec: dict[str, Any]) -> int:
    mig = spec.get("mig") or {}
    if mig.get("profile"):
        count = mig.get("count")
        return int(count) if count is not None else 1
    distributed = spec.get("distributed") or {}
    if distributed.get("enabled"):
        nodes = int(distributed.get("nodes", 1))
        gpn = int(distributed.get("gpusPerNode", 1))
        return nodes * gpn
    return int(spec.get("gpus", 1))


def site_of(manifest: dict[str, Any]) -> str | None:
    """Federation site label off a manifest's metadata, or None if unset."""
    meta = manifest.get("metadata") or {}
    labels = meta.get("labels") or {}
    return labels.get(FEDERATION_SITE_LABEL)


def resolve_tenant(namespace: str, quotas: list[dict[str, Any]]) -> str | None:
    for quota in quotas:
        spec = quota.get("spec") or {}
        team = spec.get("team")
        if not team:
            continue
        namespaces = spec.get("namespaces")
        if namespaces is None:
            return str(team)
        if namespace in namespaces:
            return str(team)
    return None


def fabric_ai_job_to_job(
    manifest: dict[str, Any],
    *,
    quotas: list[dict[str, Any]] | None = None,
    runtime_seconds: float | None = None,
    gpu_memory_gb: float | None = None,
) -> dict[str, Any]:
    """Convert a FabricAIJob manifest to an internal workload job spec."""
    meta = manifest.get("metadata") or {}
    spec = manifest.get("spec") or {}
    annotations = meta.get("annotations") or {}
    labels = meta.get("labels") or {}
    namespace = meta.get("namespace", "default")
    name = meta.get("name", "unknown")
    site = labels.get(FEDERATION_SITE_LABEL)

    gang_enabled = annotations.get("zynera.ai/gang-schedule") == "true"
    gang_size_raw = annotations.get("zynera.ai/gang-size")
    gang_size_nodes = int(gang_size_raw) if gang_size_raw is not None else None
    gang_timeout_raw = annotations.get("zynera.ai/gang-timeout")
    gang_timeout_secs = _parse_duration_secs(gang_timeout_raw) if gang_timeout_raw else None

    mig = spec.get("mig") or {}
    network = spec.get("network")
    network_bw = 400.0 if network == "rdma" else (200.0 if network == "sriov" else None)

    job: dict[str, Any] = {
        "id": f"{namespace}/{name}",
        "name": name,
        "namespace": namespace,
        "arrival_time": 0.0,
        "gpu_count": gpu_count_from_spec(spec),
        "gpu_type": spec.get("gpuType", "any"),
        "priority": int(spec.get("priority", 0)),
        "tenant": resolve_tenant(namespace, quotas or []),
        "site": site,
        "network_bw_gbps": network_bw,
        "gang_enabled": gang_enabled,
        "gang_size_nodes": gang_size_nodes,
        "gang_timeout_secs": gang_timeout_secs,
        "mig_profile": mig.get("profile"),
        "mig_count": mig.get("count"),
    }

    if runtime_seconds is not None:
        job["runtime"] = runtime_seconds
    if gpu_memory_gb is not None:
        job["gpu_memory_gb"] = gpu_memory_gb

    return job


def _parse_duration_secs(raw: str) -> float | None:
    s = raw.strip()
    if s.endswith("s"):
        try:
            return float(s[:-1])
        except ValueError:
            return None
    if s.endswith("m"):
        try:
            return float(s[:-1]) * 60.0
        except ValueError:
            return None
    if s.endswith("h"):
        try:
            return float(s[:-1]) * 3600.0
        except ValueError:
            return None
    try:
        return float(s)
    except ValueError:
        return None
