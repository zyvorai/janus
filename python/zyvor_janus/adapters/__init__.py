# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Input adapters for Zyvor Janus."""

__all__ = [
    "ZyneraBundle",
    "ZyneraBundleAdapter",
    "ProfileLookupError",
    "ProfileRegistry",
    "TraceAdapter",
    "TraceRecord",
    "fabric_ai_job_to_job",
    "gpu_count_from_spec",
    "resolve_tenant",
]


def __getattr__(name: str):
    if name in ("ZyneraBundle", "ZyneraBundleAdapter"):
        from zyvor_janus.adapters.bundle import ZyneraBundle, ZyneraBundleAdapter

        return {"ZyneraBundle": ZyneraBundle, "ZyneraBundleAdapter": ZyneraBundleAdapter}[name]
    if name in ("TraceAdapter", "TraceRecord"):
        from zyvor_janus.adapters.trace import TraceAdapter, TraceRecord

        return {"TraceAdapter": TraceAdapter, "TraceRecord": TraceRecord}[name]
    if name in ("ProfileLookupError", "ProfileRegistry"):
        from zyvor_janus.adapters.profiles import ProfileLookupError, ProfileRegistry

        return {"ProfileLookupError": ProfileLookupError, "ProfileRegistry": ProfileRegistry}[name]
    if name in ("fabric_ai_job_to_job", "gpu_count_from_spec", "resolve_tenant"):
        from zyvor_janus.adapters.crd import fabric_ai_job_to_job, gpu_count_from_spec, resolve_tenant

        return {
            "fabric_ai_job_to_job": fabric_ai_job_to_job,
            "gpu_count_from_spec": gpu_count_from_spec,
            "resolve_tenant": resolve_tenant,
        }[name]
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
