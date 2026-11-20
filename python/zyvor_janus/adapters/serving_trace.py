# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""serving.trace.v1 adapters — separate from M3 scheduler traces."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

SERVING_TRACE_VERSION = "serving.trace.v1"


def load_serving_trace(path: Path) -> dict[str, Any]:
    text = path.read_text()
    if path.suffix == ".json":
        data = json.loads(text)
    else:
        records = [json.loads(line) for line in text.splitlines() if line.strip()]
        data = {"version": SERVING_TRACE_VERSION, "records": records}
    if data.get("version") != SERVING_TRACE_VERSION:
        raise ValueError(f"unsupported trace version: {data.get('version')}")
    return data


def to_aiperf_requests(trace: dict[str, Any]) -> list[dict[str, Any]]:
    """Map serving.trace.v1 records to AIPerf-style request rows."""
    rows: list[dict[str, Any]] = []
    for rec in trace.get("records", []):
        rows.append(
            {
                "timestamp": rec["time"],
                "model": rec["model"],
                "input_sequence_length": rec["input_tokens"],
                "output_sequence_length": rec["output_tokens"],
                "request_id": rec.get("request_id"),
            }
        )
    return rows


def from_aiperf_rows(rows: list[dict[str, Any]]) -> dict[str, Any]:
    records = []
    for row in rows:
        records.append(
            {
                "time": float(row.get("timestamp", row.get("time", 0.0))),
                "model": row["model"],
                "input_tokens": int(row.get("input_sequence_length", row.get("input_tokens", 0))),
                "output_tokens": int(row.get("output_sequence_length", row.get("output_tokens", 0))),
                "batch_size": int(row.get("batch_size", 1)),
                "concurrency": int(row.get("concurrency", 1)),
                "request_id": row.get("request_id"),
            }
        )
    return {"version": SERVING_TRACE_VERSION, "records": records}


def write_serving_trace_jsonl(path: Path, trace: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w") as fh:
        for rec in trace.get("records", []):
            fh.write(json.dumps(rec) + "\n")
