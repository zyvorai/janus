# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Gymnasium wrapper for Zyvor Janus stepped scheduling."""

from __future__ import annotations

from typing import Any

import numpy as np

try:
    import gymnasium as gym
    from gymnasium import spaces
except ImportError as exc:  # pragma: no cover - optional dependency
    raise ImportError("ZyvorJanusEnv requires gymnasium; install with pip install -e '.[rl]'") from exc

from zyvor_janus._zyvor_janus import SimSession


class ZyvorJanusEnv(gym.Env):
    """Discrete scheduling env: pick a waiting job index or noop."""

    metadata = {"render_modes": []}

    def __init__(self, config_path: str) -> None:
        super().__init__()
        self.config_path = config_path
        self.session = SimSession(config_path)
        self.obs_size = self.session.obs_size
        self.action_space_n = self.session.action_space_n
        self.top_k = self.session.top_k

        self.observation_space = spaces.Box(
            low=-np.inf,
            high=np.inf,
            shape=(self.obs_size,),
            dtype=np.float32,
        )
        self.action_space = spaces.Discrete(self.action_space_n)

    def reset(self, *, seed: int | None = None, options: dict[str, Any] | None = None):
        super().reset(seed=seed)
        obs_dict = self.session.reset()
        return self._features(obs_dict), self._info(obs_dict)

    def step(self, action: int):
        result = self.session.step(int(action))
        obs = self._features(result["observation"])
        terminated = bool(result["done"])
        truncated = False
        reward = float(result["reward"])
        info = {
            "placed": bool(result["placed"]),
            "invalid_action": bool(result["invalid_action"]),
        }
        return obs, reward, terminated, truncated, info

    def metrics(self):
        return self.session.metrics()

    @staticmethod
    def _features(obs_dict: dict[str, Any]) -> np.ndarray:
        return np.asarray(obs_dict["features"], dtype=np.float32)

    @staticmethod
    def _info(obs_dict: dict[str, Any]) -> dict[str, Any]:
        return {
            "clock": obs_dict["clock"],
            "waiting": obs_dict["waiting"],
            "free_gpus": obs_dict["free_gpus"],
        }
