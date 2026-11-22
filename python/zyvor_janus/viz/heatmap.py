# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""GPU utilization heatmap from job timelines."""

from __future__ import annotations

from typing import Any

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.figure import Figure

from zyvor_janus.theme import heatmap_cmap, matplotlib_rcparams


def _busy_matrix(timeline: dict[str, Any], *, bucket_size: float = 1.0) -> tuple[np.ndarray, list[str]]:
    jobs = timeline.get("jobs", [])
    gpu_ids: list[str] = sorted(
        {gpu for job in jobs for gpu in job.get("assigned_gpus", [])}
    )
    if not gpu_ids:
        gpu_ids = [f"gpu-{i}" for i in range(int(timeline.get("gpu_count", 1)))]
    makespan = float(timeline.get("makespan", 0.0))
    n_buckets = max(1, int(np.ceil(makespan / bucket_size)))
    matrix = np.zeros((len(gpu_ids), n_buckets), dtype=float)
    gpu_index = {gpu_id: idx for idx, gpu_id in enumerate(gpu_ids)}

    for job in jobs:
        start = job.get("start_time")
        finish = job.get("finish_time")
        if start is None:
            continue
        start_f = float(start)
        end_f = float(finish) if finish is not None else start_f + float(job.get("runtime", 0.0))
        b0 = int(start_f // bucket_size)
        b1 = min(n_buckets, int(np.ceil(end_f / bucket_size)))
        for gpu_id in job.get("assigned_gpus", []):
            row = gpu_index.get(gpu_id)
            if row is None:
                continue
            matrix[row, b0:b1] = 1.0
    return matrix, gpu_ids


def plot_gpu_heatmap(
    timeline: dict[str, Any],
    *,
    bucket_size: float = 1.0,
    title: str = "GPU utilization",
) -> Figure:
    plt.rcParams.update(matplotlib_rcparams())
    matrix, gpu_ids = _busy_matrix(timeline, bucket_size=bucket_size)
    fig, ax = plt.subplots(figsize=(10, max(3, len(gpu_ids) * 0.4)))
    im = ax.imshow(matrix, aspect="auto", interpolation="nearest", cmap=heatmap_cmap())
    ax.set_yticks(range(len(gpu_ids)))
    ax.set_yticklabels(gpu_ids)
    ax.set_xlabel(f"time bucket ({bucket_size:g}s)")
    ax.set_title(title)
    fig.colorbar(im, ax=ax, fraction=0.02)
    fig.tight_layout()
    return fig
