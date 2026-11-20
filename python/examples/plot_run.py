# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Plot Gantt and GPU heatmap from a Zyvor Janus jobs timeline JSON."""

from __future__ import annotations

import argparse
from pathlib import Path

from zyvor_janus.viz import save_run_figures


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("timeline", type=Path, help="jobs timeline JSON from zyvor-janus run")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("outputs/figures"),
        help="directory for PNG outputs",
    )
    parser.add_argument("--prefix", default="run")
    args = parser.parse_args()

    gantt, heatmap = save_run_figures(args.timeline, args.output_dir, prefix=args.prefix)
    print(f"wrote {gantt}")
    print(f"wrote {heatmap}")


if __name__ == "__main__":
    main()
