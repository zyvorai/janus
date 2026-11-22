# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""Digital twin store for calibrated cluster fingerprints."""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


@dataclass
class TwinEntry:
    gpu_type: str
    model: str
    ttft_ms: float
    tps: float
    throughput: float
    measured_at: str
    aiperf_run_id: str | None = None


class TwinStore:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self.db_path = self.root / "twins.sqlite"
        self._init_db()

    def _init_db(self) -> None:
        with sqlite3.connect(self.db_path) as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS twins (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    gpu_type TEXT NOT NULL,
                    model TEXT NOT NULL,
                    ttft_ms REAL NOT NULL,
                    tps REAL NOT NULL,
                    throughput REAL NOT NULL,
                    measured_at TEXT NOT NULL,
                    aiperf_run_id TEXT,
                    version INTEGER NOT NULL DEFAULT 1
                )
                """
            )

    def upsert(self, entry: TwinEntry) -> int:
        with sqlite3.connect(self.db_path) as conn:
            cur = conn.execute(
                "SELECT MAX(version) FROM twins WHERE gpu_type=? AND model=?",
                (entry.gpu_type, entry.model),
            )
            version = (cur.fetchone()[0] or 0) + 1
            conn.execute(
                """
                INSERT INTO twins (gpu_type, model, ttft_ms, tps, throughput, measured_at, aiperf_run_id, version)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    entry.gpu_type,
                    entry.model,
                    entry.ttft_ms,
                    entry.tps,
                    entry.throughput,
                    entry.measured_at,
                    entry.aiperf_run_id,
                    version,
                ),
            )
            return version

    def latest(self, gpu_type: str, model: str) -> TwinEntry | None:
        with sqlite3.connect(self.db_path) as conn:
            row = conn.execute(
                """
                SELECT gpu_type, model, ttft_ms, tps, throughput, measured_at, aiperf_run_id
                FROM twins
                WHERE gpu_type=? AND model=?
                ORDER BY version DESC LIMIT 1
                """,
                (gpu_type, model),
            ).fetchone()
        if not row:
            return None
        return TwinEntry(*row)

    def detect_drift(self, gpu_type: str, model: str, measured_ttft_ms: float, threshold: float = 0.10) -> bool:
        twin = self.latest(gpu_type, model)
        if twin is None:
            return False
        delta = abs(measured_ttft_ms - twin.ttft_ms) / max(twin.ttft_ms, 1e-6)
        return delta > threshold

    def export_json(self, path: Path) -> None:
        with sqlite3.connect(self.db_path) as conn:
            rows = conn.execute(
                "SELECT gpu_type, model, ttft_ms, tps, throughput, measured_at, aiperf_run_id FROM twins ORDER BY id"
            ).fetchall()
        payload = [
            {
                "gpu_type": r[0],
                "model": r[1],
                "ttft_ms": r[2],
                "tps": r[3],
                "throughput": r[4],
                "measured_at": r[5],
                "aiperf_run_id": r[6],
            }
            for r in rows
        ]
        path.write_text(json.dumps(payload, indent=2))

    @staticmethod
    def now_iso() -> str:
        return datetime.now(timezone.utc).isoformat()
