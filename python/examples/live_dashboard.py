#!/usr/bin/env python3
# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Run the Zyvor Janus Rich live dashboard."""

from __future__ import annotations

import sys
from pathlib import Path


def _bootstrap_python_path() -> None:
    """Allow running this script without `pip install -e .`."""
    repo_root = Path(__file__).resolve().parents[2]
    python_src = repo_root / "python"
    if python_src.is_dir():
        path = str(python_src)
        if path not in sys.path:
            sys.path.insert(0, path)


def _check_prerequisites() -> None:
    try:
        import rich  # noqa: F401
    except ImportError as exc:
        raise SystemExit(
            "Missing dependency: rich\n"
            "Run: ./scripts/setup_dev.sh\n"
            "Or: source .venv/bin/activate && pip install rich"
        ) from exc

    try:
        from zyvor_janus import _zyvor_janus  # noqa: F401
    except ImportError as exc:
        raise SystemExit(
            "Zyvor Janus Rust extension is not built.\n"
            "Run from repo root:\n"
            "  ./scripts/setup_dev.sh\n"
            "  source .venv/bin/activate\n"
            "  ./scripts/run_live_dashboard.sh --config configs/clusters/small_h100.yaml\n"
            "\n"
            "Do not use system python3 (3.9) — use the .venv (Python 3.10+)."
        ) from exc


def main() -> None:
    _bootstrap_python_path()
    _check_prerequisites()
    from zyvor_janus.dashboard.__main__ import main as run_dashboard

    run_dashboard()


if __name__ == "__main__":
    main()
