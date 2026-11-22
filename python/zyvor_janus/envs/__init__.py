# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Gymnasium environments wrapping Zyvor Janus RL sessions."""

from typing import TYPE_CHECKING, Any

__all__ = ["ZyvorJanusEnv"]

if TYPE_CHECKING:
    from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv


def __getattr__(name: str) -> Any:
    if name == "ZyvorJanusEnv":
        from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv

        return ZyvorJanusEnv
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
