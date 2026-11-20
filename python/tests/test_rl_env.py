# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Tests for Zyvor Janus RL session and Gym env."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RL_CONFIG = ROOT / "configs/clusters/rl_small.yaml"


@unittest.skipUnless(RL_CONFIG.exists(), "rl config fixture missing")
class TestSimSession(unittest.TestCase):
    def test_fifo_rollout_completes(self) -> None:
        try:
            from zyvor_janus._zyvor_janus import SimSession
        except ImportError:
            self.skipTest("zyvor_janus extension not built")

        session = SimSession(str(RL_CONFIG))
        session.reset()
        top_k = session.top_k
        while not session.is_done:
            obs = session.observe()
            action = top_k
            if obs["waiting"] > 0:
                action = 0
            session.step(action)
        metrics = session.metrics()
        self.assertEqual(metrics.jobs_completed, metrics.jobs_total)
        self.assertGreater(metrics.makespan, 0.0)


@unittest.skipUnless(RL_CONFIG.exists(), "rl config fixture missing")
class TestZyvorJanusEnv(unittest.TestCase):
    def test_gym_api(self) -> None:
        try:
            from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv
        except ImportError as exc:
            self.skipTest(str(exc))

        env = ZyvorJanusEnv(str(RL_CONFIG))
        obs, info = env.reset(seed=0)
        self.assertEqual(obs.shape[0], env.obs_size)
        self.assertEqual(env.obs_size, 5 + env.top_k * 7)
        self.assertIn("clock", info)
        total_reward = 0.0
        for _ in range(500):
            action = 0 if obs[2] > 0 else env.top_k
            obs, reward, terminated, truncated, _info = env.step(action)
            total_reward += reward
            if terminated or truncated:
                break
        metrics = env.metrics()
        self.assertEqual(metrics.jobs_completed, metrics.jobs_total)


if __name__ == "__main__":
    unittest.main()
