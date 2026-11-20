# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Rich live dashboard runner."""

from __future__ import annotations

import argparse
import time
from pathlib import Path

from zyvor_janus.dashboard.state import (
    render_dashboard_rich,
    render_dashboard_text,
    snapshot_to_dashboard_state,
)


def run_live_dashboard(
    config_path: str | Path,
    *,
    refresh_hz: float = 4.0,
    use_rich: bool = True,
    max_steps: int = 100_000,
) -> None:
    from zyvor_janus import SimSession

    session = SimSession(str(config_path))
    snapshot = session.reset()
    delay = 1.0 / max(refresh_hz, 0.1)
    steps = 0

    if use_rich:
        from rich.live import Live

        with Live(render_dashboard_rich(snapshot_to_dashboard_state(snapshot)), refresh_per_second=refresh_hz) as live:
            while not session.is_done and steps < max_steps:
                result = session.step_fifo()
                snapshot = result["observation"]
                live.update(render_dashboard_rich(snapshot_to_dashboard_state(snapshot)))
                steps += 1
                time.sleep(delay)
    else:
        while not session.is_done and steps < max_steps:
            result = session.step_fifo()
            snapshot = result["observation"]
            print(render_dashboard_text(snapshot_to_dashboard_state(snapshot)))
            print()
            steps += 1
            time.sleep(delay)

    metrics = session.metrics()
    print(
        f"\nDone — makespan={metrics.makespan:.2f}s "
        f"util={metrics.gpu_utilization * 100:.1f}% "
        f"jobs={metrics.jobs_completed}/{metrics.jobs_total}"
    )


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Zyvor Janus Rich live dashboard")
    parser.add_argument(
        "--config",
        default="configs/clusters/small_h100.yaml",
        help="Simulation config YAML",
    )
    parser.add_argument("--refresh-hz", type=float, default=4.0)
    parser.add_argument("--plain", action="store_true", help="Plain text instead of Rich")
    args = parser.parse_args(argv)
    run_live_dashboard(args.config, refresh_hz=args.refresh_hz, use_rich=not args.plain)


if __name__ == "__main__":
    main()
