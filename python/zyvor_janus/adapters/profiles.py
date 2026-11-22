# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Calibrated model runtime profiles keyed by (model, gpuType)."""

from __future__ import annotations

from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover
    from zyvor_janus.adapters import simple_yaml as yaml


class ProfileLookupError(Exception):
    pass


class ProfileRegistry:
    def __init__(self, profiles_dir: Path) -> None:
        self._profiles: dict[str, dict[str, dict[str, Any]]] = {}
        if profiles_dir.exists():
            for path in sorted(profiles_dir.glob("*.yaml")):
                data = yaml.safe_load(path.read_text()) or {}
                model = data.get("model")
                if model:
                    self._profiles[str(model)] = data.get("profiles") or {}

    def lookup(self, model: str, gpu_type: str) -> tuple[float, float]:
        profiles = self._profiles.get(model)
        if not profiles:
            raise ProfileLookupError(
                f"no calibrated profile for model '{model}' in profiles-dir"
            )
        entry = profiles.get(gpu_type)
        if not entry:
            raise ProfileLookupError(
                f"no calibrated profile for model '{model}' gpuType '{gpu_type}'"
            )
        return float(entry["runtime_seconds"]), float(entry["gpu_memory_gb"])

    def lookup_v2(self, model: str, gpu_type: str) -> tuple[float, float]:
        """Return (prefill_ms_per_token, decode_tps)."""
        profiles = self._profiles.get(model)
        if not profiles:
            raise ProfileLookupError(f"no profile for model '{model}'")
        entry = profiles.get(gpu_type) or profiles.get(gpu_type.split("_")[0])
        if not entry:
            raise ProfileLookupError(f"no profile for model '{model}' gpu '{gpu_type}'")
        prefill = float(entry.get("prefill_ms_per_token", 0.08))
        decode = float(entry.get("decode_tps", 120.0))
        return prefill, decode
