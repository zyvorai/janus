# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""What-if parameter sweep runner."""

from __future__ import annotations

import itertools
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class SweepVariant:
    cluster: str | None = None
    scheduler: str | None = None
    workload: str | None = None


def cartesian_variants(**options: list[str]) -> list[dict[str, str]]:
    keys = list(options.keys())
    values = [options[k] for k in keys]
    return [dict(zip(keys, combo, strict=True)) for combo in itertools.product(*values)]


def run_sweep(
    base_config: Path,
    variants: list[dict[str, str]],
    *,
    run_fn: Any,
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for variant in variants:
        scheduler = variant.get("scheduler")
        report = run_fn(str(base_config), scheduler)
        metrics = json.loads(report["metrics"].to_json())
        row = {"variant": variant, "metrics": metrics}
        if "benchmark" in report:
            row["benchmark"] = report["benchmark"]
        results.append(row)
    return results
