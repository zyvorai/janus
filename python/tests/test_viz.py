# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Tests for Zyvor Janus visualization helpers."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests/fixtures/viz/jobs_small.json"


def _hex_to_rgb01(hex_color: str) -> tuple[float, float, float]:
    value = hex_color.lstrip("#")
    return tuple(int(value[i : i + 2], 16) / 255 for i in (0, 2, 4))


class TestViz(unittest.TestCase):
    def test_load_timeline(self) -> None:
        from zyvor_janus.viz.timeline import load_timeline

        data = load_timeline(FIXTURE)
        self.assertEqual(len(data["jobs"]), 2)
        self.assertEqual(data["gpu_count"], 2)

    def test_theme_rcparams_accepted_by_matplotlib(self) -> None:
        try:
            import matplotlib
        except ImportError:
            self.skipTest("matplotlib not installed")

        from zyvor_janus.theme import matplotlib_rcparams

        with matplotlib.rc_context(matplotlib_rcparams()):
            pass

    def test_gantt_theme_colors(self) -> None:
        try:
            __import__("matplotlib")
        except ImportError:
            self.skipTest("matplotlib not installed")

        from zyvor_janus.theme import ACCENT_ORANGE, TEAL
        from zyvor_janus.viz.gantt import plot_gantt
        from zyvor_janus.viz.timeline import load_timeline

        data = load_timeline(FIXTURE)
        fig = plot_gantt(data)
        ax = fig.axes[0]
        patch_colors = {tuple(p.get_facecolor()[:3]) for p in ax.patches}
        self.assertIn(_hex_to_rgb01(ACCENT_ORANGE), patch_colors)
        self.assertIn(_hex_to_rgb01(TEAL), patch_colors)

    def test_save_run_figures(self) -> None:
        try:
            __import__("matplotlib")
        except ImportError:
            self.skipTest("matplotlib not installed")

        from zyvor_janus.viz.plots import save_run_figures

        out_dir = ROOT / "outputs/test_viz"
        gantt, heatmap = save_run_figures(FIXTURE, out_dir, prefix="test")
        self.assertTrue(gantt.exists())
        self.assertTrue(heatmap.exists())


if __name__ == "__main__":
    unittest.main()
