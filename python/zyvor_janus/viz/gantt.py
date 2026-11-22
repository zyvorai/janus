# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Gantt chart for job timelines."""

from __future__ import annotations

from typing import Any

import matplotlib.pyplot as plt
from matplotlib.figure import Figure

from zyvor_janus.theme import ACCENT_ORANGE, TEAL, TEXT_MUTED, matplotlib_rcparams


def plot_gantt(timeline: dict[str, Any], *, title: str = "Zyvor Janus job schedule") -> Figure:
    plt.rcParams.update(matplotlib_rcparams())
    jobs = timeline.get("jobs", [])
    fig, ax = plt.subplots(figsize=(10, max(3, len(jobs) * 0.35)))

    y_labels: list[str] = []
    for idx, job in enumerate(jobs):
        y = idx
        y_labels.append(job.get("name") or job.get("job_id", f"job-{idx}"))
        arrival = float(job.get("arrival_time", 0.0))
        start = job.get("start_time")
        finish = job.get("finish_time")
        if start is None:
            ax.barh(y, 0.01, left=arrival, height=0.4, color=TEXT_MUTED, label=None)
            continue
        start_f = float(start)
        end_f = float(finish) if finish is not None else start_f + float(job.get("runtime", 0.0))
        wait_width = max(0.0, start_f - arrival)
        run_width = max(0.01, end_f - start_f)
        if wait_width > 0:
            ax.barh(y, wait_width, left=arrival, height=0.4, color=ACCENT_ORANGE)
        ax.barh(y, run_width, left=start_f, height=0.4, color=TEAL)

    ax.set_yticks(range(len(y_labels)))
    ax.set_yticklabels(y_labels)
    ax.set_xlabel("simulation time (s)")
    ax.set_title(title)
    ax.grid(axis="x", alpha=0.2)
    fig.tight_layout()
    return fig
