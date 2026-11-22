# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Zyvor Janus live terminal dashboard (Phase 1)."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class GpuState:
    id: str
    node_id: str
    utilization: float
    job_name: str | None


@dataclass
class DashboardState:
    clock: float
    running: int
    waiting: int
    finished: int
    node_count: int
    gpu_count: int
    gpus: list[GpuState]
    queue_names: list[str]


def format_sim_time(seconds: float) -> str:
    total = int(max(0.0, seconds))
    hours, rem = divmod(total, 3600)
    minutes, secs = divmod(rem, 60)
    return f"{hours:02d}:{minutes:02d}:{secs:02d}"


def snapshot_to_dashboard_state(snapshot: dict[str, Any]) -> DashboardState:
    gpus: list[GpuState] = []
    for node in snapshot.get("nodes", []):
        for gpu in node.get("gpus", []):
            gpus.append(
                GpuState(
                    id=str(gpu.get("id", "")),
                    node_id=str(gpu.get("node_id", node.get("id", ""))),
                    utilization=float(gpu.get("utilization", 0.0)),
                    job_name=gpu.get("job_name"),
                )
            )
    queue_names = [
        str(job.get("name") or job.get("id", f"job-{idx}"))
        for idx, job in enumerate(snapshot.get("queue_jobs", []))
    ]
    return DashboardState(
        clock=float(snapshot.get("clock", 0.0)),
        running=int(snapshot.get("running", 0)),
        waiting=int(snapshot.get("waiting", 0)),
        finished=int(snapshot.get("finished", 0)),
        node_count=int(snapshot.get("node_count", 0)),
        gpu_count=int(snapshot.get("gpu_count", len(gpus))),
        gpus=gpus,
        queue_names=queue_names,
    )


def render_util_bar(utilization: float, width: int = 10) -> str:
    util = max(0.0, min(1.0, utilization))
    filled = int(round(util * width))
    return "█" * filled + "░" * (width - filled)


def render_util_bar_rich(utilization: float, width: int = 10) -> Any:
    from zyvor_janus.theme import ACCENT, TEXT_DIM
    from rich.text import Text

    util = max(0.0, min(1.0, utilization))
    filled = int(round(util * width))
    bar = Text()
    bar.append("█" * filled, style=ACCENT)
    bar.append("░" * (width - filled), style=TEXT_DIM)
    return bar


def render_dashboard_text(state: DashboardState) -> str:
    lines = [
        "━" * 42,
        f"Simulation Time : {format_sim_time(state.clock)}",
        f"Running Jobs    : {state.running}",
        f"Queued Jobs     : {state.waiting}",
        "",
        "GPU Utilization",
    ]
    for gpu in state.gpus:
        pct = int(round(gpu.utilization * 100))
        bar = render_util_bar(gpu.utilization)
        lines.append(f"{gpu.id:<6} {bar} {pct:>3}%")
    lines.append("━" * 42)
    lines.append("Queue")
    if not state.queue_names:
        lines.append("  (empty)")
    else:
        for idx, name in enumerate(state.queue_names[:12], start=1):
            lines.append(f"{idx}. {name}")
    lines.append("━" * 42)
    return "\n".join(lines)


def render_dashboard_rich(state: DashboardState) -> Any:
    from rich.console import Group
    from rich.panel import Panel
    from rich.table import Table

    from zyvor_janus.theme import rich_styles

    styles = rich_styles()
    summary = Table.grid(padding=(0, 2))
    summary.add_row("Simulation Time", format_sim_time(state.clock))
    summary.add_row("Running Jobs", str(state.running))
    summary.add_row("Queued Jobs", str(state.waiting))
    summary.add_row("Finished", str(state.finished))

    gpu_table = Table(title="GPU Utilization", show_header=True, header_style="bold")
    gpu_table.add_column("GPU")
    gpu_table.add_column("Util")
    gpu_table.add_column("Bar")
    gpu_table.add_column("%", justify="right")
    for gpu in state.gpus:
        pct = int(round(gpu.utilization * 100))
        gpu_table.add_row(
            gpu.id,
            render_util_bar(gpu.utilization),
            render_util_bar_rich(gpu.utilization),
            f"{pct}%",
        )

    queue_table = Table(title="Queue", show_header=False, box=None)
    if not state.queue_names:
        queue_table.add_row("(empty)")
    else:
        for idx, name in enumerate(state.queue_names[:12], start=1):
            queue_table.add_row(f"{idx}. {name}")

    return Group(
        Panel(summary, title="Zyvor Janus · Zyvor", border_style=styles["summary"]),
        Panel(gpu_table, border_style=styles["gpu"]),
        Panel(queue_table, title="Waiting", border_style=styles["queue"]),
    )
