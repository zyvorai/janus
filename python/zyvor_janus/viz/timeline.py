# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Load Zyvor Janus jobs timeline JSON."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def load_timeline(path: str | Path) -> dict[str, Any]:
    with Path(path).open(encoding="utf-8") as handle:
        return json.load(handle)
