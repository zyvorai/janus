# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""Scheduler event trace adapter (JSONL)."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class TraceRecord:
    events: list[dict[str, Any]] = field(default_factory=list)


class TraceAdapter:
    """Load Zynera scheduler event exports for replay and oracle comparison."""

    def from_file(self, path: str | Path) -> TraceRecord:
        record = TraceRecord()
        content = Path(path).read_text()
        for line_no, line in enumerate(content.splitlines(), start=1):
            trimmed = line.strip()
            if not trimmed or trimmed.startswith("#"):
                continue
            try:
                event = json.loads(trimmed)
            except json.JSONDecodeError as exc:
                raise ValueError(
                    f"invalid trace JSON at {path}:{line_no}: {exc}"
                ) from exc
            if "event" not in event or "timestamp" not in event:
                raise ValueError(
                    f"trace line missing required fields at {path}:{line_no}"
                )
            record.events.append(event)
        if not record.events:
            raise ValueError(f"trace file {path} contains no events")
        return record

    def jobs_from_events(self, events: list[dict[str, Any]]) -> list[dict[str, Any]]:
        jobs: list[dict[str, Any]] = []
        for event in events:
            if event.get("event") != "JobSubmitted":
                continue
            job_id = str(event["job"])
            jobs.append(
                {
                    "id": job_id,
                    "name": job_id,
                    "arrival_time": float(event["timestamp"]),
                    "runtime": float(event["runtime"]),
                    "gpu_count": int(event["gpu_count"]),
                    "gpu_memory_gb": float(event.get("gpu_memory_gb", 0)),
                    "priority": int(event.get("priority", 0)),
                }
            )
        if not jobs:
            raise ValueError("trace must contain at least one JobSubmitted event")
        return jobs

    def oracle_schedules(
        self, events: list[dict[str, Any]]
    ) -> list[dict[str, Any]]:
        schedules: list[dict[str, Any]] = []
        for event in events:
            if event.get("event") != "JobScheduled":
                continue
            gpus = event.get("gpus", [])
            normalized: list[str] = []
            node = str(event["node"])
            for gpu in gpus:
                if isinstance(gpu, int):
                    normalized.append(f"{node}-gpu-{gpu}")
                else:
                    normalized.append(str(gpu))
            schedules.append(
                {
                    "job_id": str(event["job"]),
                    "timestamp": float(event["timestamp"]),
                    "node": node,
                    "gpu_ids": normalized,
                }
            )
        return schedules

    def diff_placements(
        self,
        oracle: list[dict[str, Any]],
        simulated: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        sim_by_job = {entry["job_id"]: entry for entry in simulated}
        diffs: list[dict[str, Any]] = []
        for entry in oracle:
            job_id = entry["job_id"]
            sim = sim_by_job.get(
                job_id,
                {
                    "job_id": job_id,
                    "start_time": float("nan"),
                    "gpu_ids": [],
                },
            )
            oracle_gpus = set(entry.get("gpu_ids", []))
            sim_gpus = set(sim.get("gpu_ids", []))
            placement_match = oracle_gpus == sim_gpus
            schedule_time_match = abs(
                float(entry["timestamp"]) - float(sim.get("start_time", float("nan")))
            ) < 1e-6
            diffs.append(
                {
                    "job_id": job_id,
                    "placement_match": placement_match,
                    "schedule_time_match": schedule_time_match,
                    "oracle": entry,
                    "simulated": sim,
                }
            )
        return diffs
