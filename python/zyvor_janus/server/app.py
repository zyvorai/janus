# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""FastAPI backend for Zyvor Janus web dashboard (Phase 2).

Superseded by crates/zyvor-janus-api (see deploy/docker/Dockerfile.api,
which now builds and ships the Rust binary instead of this package).
Kept in-tree, unused by any deployed image, as a revert target during
the Rust API's burn-in period -- not scheduled for deletion until that
period ends.
"""

from __future__ import annotations

import asyncio
import json
import os
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field

# Source checkout: python/zyvor_janus/server/app.py → parents[3] = repo root.
# Installed wheel: prefer ZYVOR_JANUS_ROOT (Docker sets this to /app).
# All three are resolved so `path.relative_to(REPO_ROOT)` below can't fail from a
# symlink mismatch (e.g. macOS /tmp -> /private/tmp) between an unresolved
# CONFIG_DIR/RUNS_DIR and the resolved REPO_ROOT.
_DEFAULT_ROOT = Path(__file__).resolve().parents[3]
REPO_ROOT = Path(os.environ.get("ZYVOR_JANUS_ROOT", str(_DEFAULT_ROOT))).resolve()
CONFIG_DIR = Path(os.environ.get("ZYVOR_JANUS_CONFIG_DIR", str(REPO_ROOT / "configs" / "clusters"))).resolve()
RUNS_DIR = Path(os.environ.get("ZYVOR_JANUS_RUNS_DIR", str(REPO_ROOT / "outputs" / "runs"))).resolve()


class RunStatus(str, Enum):
    pending = "pending"
    running = "running"
    completed = "completed"
    failed = "failed"


class StartRunRequest(BaseModel):
    config: str = Field(..., description="Cluster config filename under configs/clusters/")
    scheduler: str | None = Field(None, description="Override scheduler type")


@dataclass
class RunRecord:
    id: str
    config: str
    scheduler: str | None
    status: RunStatus = RunStatus.pending
    created_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())
    finished_at: str | None = None
    error: str | None = None
    metrics: dict[str, Any] | None = None
    timeline: dict[str, Any] | None = None
    decisions: list[dict[str, Any]] = field(default_factory=list)
    snapshots: list[dict[str, Any]] = field(default_factory=list)
    config_hash: str | None = None
    resolved_scheduler: str | None = None
    benchmark: dict[str, Any] | None = None
    quality: dict[str, Any] | None = None
    chargeback: dict[str, Any] | None = None
    slo: dict[str, Any] | None = None


RUNS: dict[str, RunRecord] = {}


def _list_configs() -> list[dict[str, str]]:
    if not CONFIG_DIR.exists():
        return []
    return [
        {"id": path.name, "path": str(path.relative_to(REPO_ROOT))}
        for path in sorted(CONFIG_DIR.glob("*.yaml"))
    ]


def _run_simulation_sync(config_name: str, scheduler: str | None = None) -> dict[str, Any]:
    from zyvor_janus import _zyvor_janus

    config_path = CONFIG_DIR / config_name
    if not config_path.exists():
        raise FileNotFoundError(f"config not found: {config_name}")

    report = _zyvor_janus.run_report_from_config(str(config_path), scheduler)
    metrics = json.loads(report["metrics"].to_json())
    timeline = json.loads(report["timeline"])
    decisions = list(report["decisions"])
    snapshots = list(report["snapshots"])
    benchmark = report.get("benchmark")
    if isinstance(benchmark, str):
        benchmark = json.loads(benchmark)

    extras = {
        key: json.loads(report[key]) if isinstance(report.get(key), str) else None
        for key in ("quality", "chargeback", "slo")
    }

    return {
        **extras,
        "metrics": metrics,
        "timeline": timeline,
        "decisions": decisions,
        "snapshots": snapshots,
        "config_hash": report["config_hash"],
        "resolved_scheduler": report["scheduler"],
        "benchmark": benchmark,
    }


app = FastAPI(title="Zyvor Janus API", version="0.1.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/api/health")
def health() -> dict[str, str]:
    return {"status": "ok"}


@app.get("/api/configs")
def list_configs() -> list[dict[str, str]]:
    return _list_configs()


@app.get("/api/runs")
def list_runs() -> list[dict[str, Any]]:
    return [
        {
            "id": run.id,
            "config": run.config,
            "scheduler": run.scheduler or run.resolved_scheduler,
            "status": run.status.value,
            "created_at": run.created_at,
            "finished_at": run.finished_at,
        }
        for run in sorted(RUNS.values(), key=lambda r: r.created_at, reverse=True)
    ]


@app.post("/api/runs")
async def start_run(body: StartRunRequest) -> dict[str, str]:
    if not (CONFIG_DIR / body.config).exists():
        raise HTTPException(status_code=404, detail=f"unknown config: {body.config}")
    run_id = str(uuid.uuid4())
    RUNS[run_id] = RunRecord(id=run_id, config=body.config, scheduler=body.scheduler)
    asyncio.create_task(_execute_run(run_id))
    return {"id": run_id}


async def _execute_run(run_id: str) -> None:
    run = RUNS[run_id]
    run.status = RunStatus.running
    run_dir = RUNS_DIR / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    try:
        result = await asyncio.to_thread(_run_simulation_sync, run.config, run.scheduler)
        run.metrics = result["metrics"]
        run.snapshots = result["snapshots"]
        run.decisions = result["decisions"]
        run.timeline = result["timeline"]
        run.config_hash = result["config_hash"]
        run.resolved_scheduler = result["resolved_scheduler"]
        run.benchmark = result.get("benchmark")
        run.quality = result.get("quality")
        run.chargeback = result.get("chargeback")
        run.slo = result.get("slo")
        run.status = RunStatus.completed
        run.finished_at = datetime.now(timezone.utc).isoformat()
        (run_dir / "metrics.json").write_text(json.dumps(run.metrics, indent=2))
        (run_dir / "timeline.json").write_text(json.dumps(run.timeline, indent=2))
        (run_dir / "decisions.json").write_text(json.dumps(run.decisions, indent=2))
        (run_dir / "snapshots.json").write_text(json.dumps(run.snapshots, indent=2))
        (run_dir / "metadata.json").write_text(
            json.dumps(
                {
                    "config": run.config,
                    "scheduler": run.scheduler,
                    "resolved_scheduler": run.resolved_scheduler,
                    "config_hash": run.config_hash,
                    "benchmark": run.benchmark,
                },
                indent=2,
            )
        )
        if run.benchmark:
            (run_dir / "benchmark.json").write_text(json.dumps(run.benchmark, indent=2))
    except Exception as exc:  # noqa: BLE001 — surface run failures to API clients
        run.status = RunStatus.failed
        run.error = str(exc)
        run.finished_at = datetime.now(timezone.utc).isoformat()


@app.get("/api/runs/{run_id}")
def get_run(run_id: str) -> dict[str, Any]:
    run = RUNS.get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail="run not found")
    return {
        "id": run.id,
        "config": run.config,
        "scheduler": run.scheduler or run.resolved_scheduler,
        "status": run.status.value,
        "created_at": run.created_at,
        "finished_at": run.finished_at,
        "error": run.error,
        "metrics": run.metrics,
        "timeline": run.timeline,
        "decision_count": len(run.decisions),
        "config_hash": run.config_hash,
        "benchmark": run.benchmark,
        "quality": run.quality,
        "chargeback": run.chargeback,
        "slo": run.slo,
    }


@app.get("/api/runs/{run_id}/snapshots")
def get_snapshots(run_id: str) -> list[dict[str, Any]]:
    run = RUNS.get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail="run not found")
    return run.snapshots


@app.get("/api/runs/{run_id}/timeline")
def get_timeline(run_id: str) -> dict[str, Any]:
    run = RUNS.get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail="run not found")
    if run.timeline is None:
        raise HTTPException(status_code=404, detail="timeline not ready")
    return run.timeline


@app.get("/api/runs/{run_id}/events")
def get_events(run_id: str) -> list[dict[str, Any]]:
    run = RUNS.get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail="run not found")
    return run.decisions


class CompareRequest(BaseModel):
    configs: list[str] = Field(..., min_length=2, description="Config filenames to compare")


@app.post("/api/compare")
async def compare_schedulers(body: CompareRequest) -> dict[str, Any]:
    if len(set(body.configs)) != len(body.configs):
        raise HTTPException(status_code=400, detail="configs must be distinct")
    results = []
    for config in body.configs:
        if not (CONFIG_DIR / config).exists():
            raise HTTPException(status_code=404, detail=f"unknown config: {config}")
        run_id = str(uuid.uuid4())
        RUNS[run_id] = RunRecord(id=run_id, config=config, scheduler=None)
        await _execute_run(run_id)
        run = RUNS[run_id]
        results.append(
            {
                "config": config,
                "status": run.status.value,
                "metrics": run.metrics,
                "run_id": run_id,
            }
        )
    return {"results": results}


class BenchmarkRunRequest(BaseModel):
    config: str
    scheduler: str | None = None


class WhatIfRequest(BaseModel):
    base_config: str
    schedulers: list[str] = Field(default_factory=lambda: ["fifo", "preemptive"])
    configs: list[str] | None = None


@app.get("/api/benchmark/presets")
def benchmark_presets() -> dict[str, Any]:
    from zyvor_janus.workloads.generate_synthetic import PRESETS

    return {
        "configs": _list_configs(),
        "workload_presets": [
            {"id": key, "description": val["description"]} for key, val in PRESETS.items()
        ],
    }


@app.get("/api/benchmark/reports")
def benchmark_reports() -> list[dict[str, Any]]:
    reports = []
    for run in RUNS.values():
        if run.status != RunStatus.completed or run.benchmark is None:
            continue
        reports.append(
            {
                "run_id": run.id,
                "config": run.config,
                "scheduler": run.scheduler or run.resolved_scheduler,
                "metrics": run.metrics,
                "benchmark": run.benchmark,
            }
        )
    return reports


@app.post("/api/benchmark/run")
async def benchmark_run(body: BenchmarkRunRequest) -> dict[str, Any]:
    if not (CONFIG_DIR / body.config).exists():
        raise HTTPException(status_code=404, detail=f"unknown config: {body.config}")
    run_id = str(uuid.uuid4())
    RUNS[run_id] = RunRecord(id=run_id, config=body.config, scheduler=body.scheduler)
    await _execute_run(run_id)
    run = RUNS[run_id]
    if run.status != RunStatus.completed:
        raise HTTPException(status_code=500, detail=run.error or "run failed")
    return {
        "run_id": run_id,
        "metrics": run.metrics,
        "benchmark": run.benchmark,
    }


@app.post("/api/what-if")
async def what_if(body: WhatIfRequest) -> dict[str, Any]:
    configs = body.configs or [body.base_config]
    rows = []
    for config in configs:
        for scheduler in body.schedulers:
            run_id = str(uuid.uuid4())
            RUNS[run_id] = RunRecord(id=run_id, config=config, scheduler=scheduler)
            await _execute_run(run_id)
            run = RUNS[run_id]
            rows.append(
                {
                    "config": config,
                    "scheduler": scheduler,
                    "run_id": run_id,
                    "status": run.status.value,
                    "metrics": run.metrics,
                    "benchmark": run.benchmark,
                }
            )
    return {"results": rows}


@app.get("/api/runs/{run_id}/serving-trace")
def export_serving_trace(run_id: str) -> dict[str, Any]:
    run = RUNS.get(run_id)
    if run is None or run.timeline is None:
        raise HTTPException(status_code=404, detail="run not found")
    from zyvor_janus.adapters.serving_trace import SERVING_TRACE_VERSION

    records = []
    for job in run.timeline.get("jobs", []):
        if job.get("state") != "finished":
            continue
        records.append(
            {
                "time": job["arrival_time"],
                "model": job.get("name", "unknown"),
                "input_tokens": 128,
                "output_tokens": 64,
                "request_id": job["job_id"],
                "tenant": job.get("tenant"),
            }
        )
    return {"version": SERVING_TRACE_VERSION, "records": records}


@app.get("/api/twins")
def list_twins() -> list[dict[str, Any]]:
    from zyvor_janus.benchmarks.twin_store import TwinStore

    store = TwinStore(REPO_ROOT / "outputs" / "twins")
    export_path = REPO_ROOT / "outputs" / "twins" / "export.json"
    store.export_json(export_path)
    return json.loads(export_path.read_text()) if export_path.exists() else []


from zyvor_janus.server.openai_shim import router as openai_router

app.include_router(openai_router)


@app.websocket("/ws/runs/{run_id}")
async def ws_run(websocket: WebSocket, run_id: str) -> None:
    run = RUNS.get(run_id)
    if run is None:
        await websocket.close(code=4404)
        return
    await websocket.accept()
    try:
        if run.snapshots:
            for snap in run.snapshots:
                await websocket.send_json({"type": "snapshot", "data": snap})
                await asyncio.sleep(0.05)
        while run.status == RunStatus.running:
            await asyncio.sleep(0.2)
        await websocket.send_json(
            {
                "type": "complete",
                "metrics": run.metrics,
                "decisions": run.decisions,
            }
        )
    except WebSocketDisconnect:
        return
