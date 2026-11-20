# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""CleanRL-inspired PPO baseline for ZyvorJanusEnv."""

from __future__ import annotations

import argparse
import random
from dataclasses import dataclass

import numpy as np

try:
    import torch
    import torch.nn as nn
    from torch.distributions import Categorical
except ImportError as exc:  # pragma: no cover
    raise ImportError("PPO baseline requires torch; install with pip install -e '.[rl]'") from exc

from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv


@dataclass
class Rollout:
    obs: list[np.ndarray]
    actions: list[int]
    logprobs: list[float]
    rewards: list[float]
    dones: list[bool]
    values: list[float]


class Agent(nn.Module):
    def __init__(self, obs_size: int, action_size: int) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(obs_size, 64),
            nn.Tanh(),
            nn.Linear(64, 64),
            nn.Tanh(),
        )
        self.policy = nn.Linear(64, action_size)
        self.value = nn.Linear(64, 1)

    def forward(self, x: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        hidden = self.net(x)
        return self.policy(hidden), self.value(hidden)


def rollout_policy(env: ZyvorJanusEnv, agent: Agent, *, steps: int, seed: int) -> Rollout:
    rng = random.Random(seed)
    obs_buf: list[np.ndarray] = []
    actions: list[int] = []
    logprobs: list[float] = []
    rewards: list[float] = []
    dones: list[bool] = []
    values: list[float] = []

    obs, _ = env.reset(seed=seed)
    for _ in range(steps):
        obs_t = torch.as_tensor(obs, dtype=torch.float32)
        logits, value = agent(obs_t)
        dist = Categorical(logits=logits)
        action = int(dist.sample().item())
        logprob = float(dist.log_prob(torch.tensor(action)).item())

        obs, reward, terminated, truncated, _info = env.step(action)
        obs_buf.append(obs_t.numpy())
        actions.append(action)
        logprobs.append(logprob)
        rewards.append(float(reward))
        values.append(float(value.item()))
        dones.append(bool(terminated or truncated))
        if terminated or truncated:
            obs, _ = env.reset(seed=rng.randint(0, 1_000_000))
    return Rollout(obs_buf, actions, logprobs, rewards, dones, values)


def train(args: argparse.Namespace) -> None:
    env = ZyvorJanusEnv(args.config)
    agent = Agent(env.obs_size, env.action_space_n)
    optimizer = torch.optim.Adam(agent.parameters(), lr=args.lr)

    for update in range(1, args.updates + 1):
        batch = rollout_policy(env, agent, steps=args.rollout_steps, seed=update)
        returns: list[float] = []
        g = 0.0
        for reward, done in zip(reversed(batch.rewards), reversed(batch.dones)):
            if done:
                g = 0.0
            g = reward + args.gamma * g
            returns.insert(0, g)
        returns_t = torch.tensor(returns, dtype=torch.float32)
        obs_t = torch.tensor(np.array(batch.obs), dtype=torch.float32)
        actions_t = torch.tensor(batch.actions, dtype=torch.int64)
        old_logprobs = torch.tensor(batch.logprobs, dtype=torch.float32)
        old_values = torch.tensor(batch.values, dtype=torch.float32)
        advantages = returns_t - old_values
        advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)

        for _ in range(args.epochs):
            logits, values = agent(obs_t)
            dist = Categorical(logits=logits)
            logprobs = dist.log_prob(actions_t)
            entropy = dist.entropy().mean()
            ratio = torch.exp(logprobs - old_logprobs)
            surr1 = ratio * advantages
            surr2 = torch.clamp(ratio, 1.0 - args.clip, 1.0 + args.clip) * advantages
            policy_loss = -torch.min(surr1, surr2).mean()
            value_loss = ((returns_t - values.squeeze(-1)) ** 2).mean()
            loss = policy_loss + 0.5 * value_loss - 0.01 * entropy
            optimizer.zero_grad()
            loss.backward()
            optimizer.step()

        if update % max(1, args.updates // 5) == 0:
            metrics = env.metrics()
            print(
                f"update={update}/{args.updates} "
                f"loss={loss.item():.3f} makespan={metrics.makespan:.2f}"
            )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", default="configs/clusters/rl_small.yaml")
    parser.add_argument("--updates", type=int, default=20)
    parser.add_argument("--rollout-steps", type=int, default=128)
    parser.add_argument("--epochs", type=int, default=4)
    parser.add_argument("--lr", type=float, default=3e-4)
    parser.add_argument("--gamma", type=float, default=0.99)
    parser.add_argument("--clip", type=float, default=0.2)
    args = parser.parse_args()
    train(args)


if __name__ == "__main__":
    main()
