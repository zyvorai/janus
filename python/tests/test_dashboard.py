# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import unittest

from zyvor_janus.dashboard.state import (
    format_sim_time,
    render_dashboard_text,
    snapshot_to_dashboard_state,
)


class TestDashboardState(unittest.TestCase):
    def test_format_sim_time(self) -> None:
        self.assertEqual(format_sim_time(735.0), "00:12:15")

    def test_snapshot_to_dashboard_state(self) -> None:
        snapshot = {
            "clock": 12.0,
            "running": 1,
            "waiting": 2,
            "finished": 3,
            "node_count": 1,
            "gpu_count": 2,
            "nodes": [
                {
                    "id": "node-a",
                    "gpus": [
                        {"id": "GPU0", "node_id": "node-a", "utilization": 1.0, "job_name": "train"},
                        {"id": "GPU1", "node_id": "node-a", "utilization": 0.53, "job_name": None},
                    ],
                }
            ],
            "queue_jobs": [
                {"id": "j1", "name": "llama70b"},
                {"id": "j2", "name": "stable-diffusion"},
            ],
        }
        state = snapshot_to_dashboard_state(snapshot)
        self.assertEqual(state.running, 1)
        self.assertEqual(len(state.gpus), 2)
        self.assertEqual(state.queue_names[0], "llama70b")
        text = render_dashboard_text(state)
        self.assertIn("Simulation Time", text)
        self.assertIn("GPU0", text)
        self.assertIn("llama70b", text)


if __name__ == "__main__":
    unittest.main()
