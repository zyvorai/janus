# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Zyvor / HyperSDK design tokens for CLI and matplotlib surfaces."""

from __future__ import annotations

from typing import Any

ACCENT = "#f0583a"
ACCENT_DEEP = "#d94b32"
ACCENT_LIGHT = "#f47a60"
ACCENT_ORANGE = "#f97316"
SUCCESS = "#22c55e"
TEAL = "#10b981"
INDIGO = "#6366f1"
INFO = "#06b6d4"
ERROR = "#ef4444"
BG = "#050505"
SURFACE = "#0b0f14"
SURFACE_CODE = "#101722"
TEXT_HEADING = "#f1f5f9"
TEXT_BODY = "#cbd5e1"
TEXT_MUTED = "#aeb9c8"
TEXT_DIM = "#8a8a8a"
BORDER = "rgba(255,255,255,0.06)"
# matplotlib does not parse CSS rgba(); #RRGGBBAA is the equivalent (alpha 0.06 ~ 0x0f).
BORDER_MPL = "#ffffff0f"


def matplotlib_rcparams() -> dict[str, Any]:
    """Return rcParams for dark Zyvor-themed figures."""
    return {
        "figure.facecolor": BG,
        "axes.facecolor": SURFACE,
        "axes.edgecolor": BORDER_MPL,
        "axes.labelcolor": TEXT_MUTED,
        "xtick.color": TEXT_MUTED,
        "ytick.color": TEXT_MUTED,
        "text.color": TEXT_BODY,
        "grid.color": BORDER_MPL,
        "grid.alpha": 1.0,
    }


def rich_styles() -> dict[str, str]:
    """Border styles for Rich panels."""
    return {
        "title": ACCENT,
        "summary": ACCENT,
        "gpu": INDIGO,
        "queue": TEXT_DIM,
    }


def heatmap_cmap():
    """Dark-to-accent colormap for GPU heatmaps."""
    from matplotlib.colors import LinearSegmentedColormap

    return LinearSegmentedColormap.from_list(
        "zyvor",
        [SURFACE, ACCENT_DEEP, ACCENT_ORANGE],
    )
