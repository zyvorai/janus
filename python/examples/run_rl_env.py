# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Roll out a random or FIFO policy against ZyvorJanusEnv."""

from __future__ import annotations

import argparse
import random

from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv


def rollout(env: ZyvorJanusEnv, policy: str, *, seed: int = 0) -> float:
    rng = random.Random(seed)
    obs, _info = env.reset(seed=seed)
    total_reward = 0.0
    steps = 0
    while True:
        if policy == "fifo":
            action = 0 if obs[2] > 0 else env.top_k
        else:
            action = rng.randrange(env.action_space_n)
        obs, reward, terminated, truncated, _info = env.step(action)
        total_reward += reward
        steps += 1
        if terminated or truncated:
            break
    metrics = env.metrics()
    print(
        f"policy={policy} steps={steps} reward={total_reward:.2f} "
        f"makespan={metrics.makespan:.2f} jobs={metrics.jobs_completed}/{metrics.jobs_total}"
    )
    return total_reward


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--config",
        default="configs/clusters/rl_small.yaml",
        help="Zyvor Janus cluster/workload config",
    )
    parser.add_argument("--seed", type=int, default=0)
    args = parser.parse_args()

    env = ZyvorJanusEnv(args.config)
    rollout(env, "fifo", seed=args.seed)
    rollout(env, "random", seed=args.seed)


if __name__ == "__main__":
    main()
