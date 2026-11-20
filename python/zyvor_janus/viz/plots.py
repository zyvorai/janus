# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Convenience helpers to save standard Zyvor Janus figures."""

from __future__ import annotations

from pathlib import Path

from zyvor_janus.viz.gantt import plot_gantt
from zyvor_janus.viz.heatmap import plot_gpu_heatmap
from zyvor_janus.viz.timeline import load_timeline


def save_run_figures(
    timeline_path: str | Path,
    output_dir: str | Path,
    *,
    prefix: str = "run",
    bucket_size: float = 1.0,
) -> tuple[Path, Path]:
    timeline = load_timeline(timeline_path)
    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)

    gantt_path = out / f"{prefix}_gantt.png"
    heatmap_path = out / f"{prefix}_heatmap.png"

    plot_gantt(timeline).savefig(gantt_path, dpi=150, bbox_inches="tight")
    plot_gpu_heatmap(timeline, bucket_size=bucket_size).savefig(
        heatmap_path, dpi=150, bbox_inches="tight"
    )
    return gantt_path, heatmap_path
