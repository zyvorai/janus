# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Zyvor Janus — thin Python API over the Rust simulation core."""

from typing import TYPE_CHECKING, Any

__all__ = ["SimResult", "SimSession", "run_from_config", "ZyvorJanusEnv"]
__version__ = "0.1.0"

if TYPE_CHECKING:
    from zyvor_janus._zyvor_janus import SimResult, SimSession
    from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv


def __getattr__(name: str) -> Any:
    if name in ("SimResult", "SimSession", "run_from_config"):
        from zyvor_janus import _zyvor_janus

        return getattr(_zyvor_janus, name)
    if name == "ZyvorJanusEnv":
        from zyvor_janus.envs.zyvor_janus_gym import ZyvorJanusEnv

        return ZyvorJanusEnv
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
