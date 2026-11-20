#!/usr/bin/env python3
# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Run a Zyvor Janus FIFO simulation from the sample H100 cluster config."""

from pathlib import Path

import zyvor_janus

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / "configs" / "clusters" / "small_h100.yaml"


def main() -> None:
    result = zyvor_janus.run_from_config(str(CONFIG))
    print(result)
    print(result.to_json())


if __name__ == "__main__":
    main()
