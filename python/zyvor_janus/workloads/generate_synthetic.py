# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Synthetic LLM workload generation for benchmark scenarios."""

from __future__ import annotations

import argparse
import json
import random
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover
    from zyvor_janus.adapters import simple_yaml as yaml


PRESETS: dict[str, dict[str, Any]] = {
    "morning_rag": {
        "description": "Low-rate RAG lookups with short outputs",
        "arrival_rate": 0.4,
        "duration_secs": 120.0,
        "input_tokens": (128, 512),
        "output_tokens": (32, 128),
        "model_id": "llama-7b",
        "tenants": ["rag-team"],
    },
    "peak_chat": {
        "description": "High-rate chat traffic with mixed tenants",
        "arrival_rate": 2.5,
        "duration_secs": 180.0,
        "input_tokens": (256, 2048),
        "output_tokens": (64, 512),
        "model_id": "llama-70b",
        "tenants": ["team-a", "team-b", "team-c"],
    },
    "night_training": {
        "description": "Sparse large-context requests overnight",
        "arrival_rate": 0.15,
        "duration_secs": 240.0,
        "input_tokens": (1024, 4096),
        "output_tokens": (128, 256),
        "model_id": "llama-70b",
        "tenants": ["batch"],
    },
}


@dataclass
class SyntheticJob:
    id: str
    name: str
    arrival_time: float
    runtime: float
    gpu_count: int
    model_id: str
    input_tokens: int
    output_tokens: int
    batch_size: int
    concurrency: int
    tenant: str | None = None

    def to_yaml_dict(self) -> dict[str, Any]:
        row: dict[str, Any] = {
            "id": self.id,
            "name": self.name,
            "arrival_time": round(self.arrival_time, 4),
            "runtime": self.runtime,
            "gpu_count": self.gpu_count,
            "model_id": self.model_id,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "batch_size": self.batch_size,
            "concurrency": self.concurrency,
        }
        if self.tenant:
            row["tenant"] = self.tenant
        return row


def _rand_range(rng: random.Random, bounds: tuple[int, int]) -> int:
    lo, hi = bounds
    return rng.randint(lo, hi)


def generate_jobs(
    preset: str = "peak_chat",
    *,
    seed: int = 42,
    duration_secs: float | None = None,
) -> list[SyntheticJob]:
    if preset not in PRESETS:
        raise ValueError(f"unknown preset '{preset}'")
    cfg = PRESETS[preset]
    rng = random.Random(seed)
    rate = float(cfg["arrival_rate"])
    horizon = float(duration_secs if duration_secs is not None else cfg["duration_secs"])
    tenants: list[str] = list(cfg["tenants"])
    jobs: list[SyntheticJob] = []
    t = 0.0
    idx = 0
    while t < horizon:
        t += rng.expovariate(rate)
        if t >= horizon:
            break
        idx += 1
        jobs.append(
            SyntheticJob(
                id=f"syn-{preset}-{idx:04d}",
                name=f"{preset}-{idx:04d}",
                arrival_time=t,
                runtime=1.0,
                gpu_count=1,
                model_id=str(cfg["model_id"]),
                input_tokens=_rand_range(rng, tuple(cfg["input_tokens"])),
                output_tokens=_rand_range(rng, tuple(cfg["output_tokens"])),
                batch_size=rng.choice([1, 1, 1, 2, 4]),
                concurrency=rng.choice([1, 1, 2, 4, 8]),
                tenant=rng.choice(tenants),
            )
        )
    return jobs


def jobs_to_workload_yaml(jobs: list[SyntheticJob]) -> str:
    lines = ["jobs:"]
    for job in jobs:
        row = job.to_yaml_dict()
        lines.append(f"  - id: {row['id']}")
        lines.append(f"    name: {row['name']}")
        lines.append(f"    arrival_time: {row['arrival_time']}")
        lines.append(f"    runtime: {row['runtime']}")
        lines.append(f"    gpu_count: {row['gpu_count']}")
        lines.append(f"    model_id: {row['model_id']}")
        lines.append(f"    input_tokens: {row['input_tokens']}")
        lines.append(f"    output_tokens: {row['output_tokens']}")
        lines.append(f"    batch_size: {row['batch_size']}")
        lines.append(f"    concurrency: {row['concurrency']}")
        if row.get("tenant"):
            lines.append(f"    tenant: {row['tenant']}")
    return "\n".join(lines) + "\n"


def jobs_to_serving_trace(jobs: list[SyntheticJob]) -> dict[str, Any]:
    return {
        "version": "serving.trace.v1",
        "records": [
            {
                "time": job.arrival_time,
                "model": job.model_id,
                "input_tokens": job.input_tokens,
                "output_tokens": job.output_tokens,
                "batch_size": job.batch_size,
                "concurrency": job.concurrency,
                "request_id": job.id,
                "tenant": job.tenant,
            }
            for job in jobs
        ],
    }


def validate_jobs(jobs: list[SyntheticJob]) -> None:
    for job in jobs:
        if job.input_tokens <= 0 and job.output_tokens <= 0:
            raise ValueError(f"job {job.id}: token counts must be positive")
        if not job.model_id:
            raise ValueError(f"job {job.id}: missing model_id")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate synthetic LLM workloads")
    parser.add_argument("--preset", default="peak_chat", choices=sorted(PRESETS))
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--output", type=Path, help="Write Zyvor Janus workload YAML")
    parser.add_argument("--trace-output", type=Path, help="Write serving.trace.v1 JSON")
    parser.add_argument("--preview", action="store_true", help="Print JSON preview to stdout")
    args = parser.parse_args()

    jobs = generate_jobs(args.preset, seed=args.seed)
    validate_jobs(jobs)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(jobs_to_workload_yaml(jobs))
    if args.trace_output:
        args.trace_output.parent.mkdir(parents=True, exist_ok=True)
        args.trace_output.write_text(json.dumps(jobs_to_serving_trace(jobs), indent=2))
    if args.preview or (not args.output and not args.trace_output):
        print(json.dumps({"preset": args.preset, "job_count": len(jobs), "jobs": [j.to_yaml_dict() for j in jobs[:5]]}, indent=2))


if __name__ == "__main__":
    main()
