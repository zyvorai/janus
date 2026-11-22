# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"""AIPerf calibration import/export for Zyvor Janus profiles."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover
    from zyvor_janus.adapters import simple_yaml as yaml


def parse_aiperf_results(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text())
    if isinstance(data, list):
        return {"results": data}
    return data


def extract_profile_curves(
    results: dict[str, Any],
    *,
    model: str,
    gpu_type: str,
) -> dict[str, float]:
    rows = results.get("results") or results.get("measurements") or []
    ttft_ms: list[float] = []
    tps: list[float] = []
    for row in rows:
        if row.get("model") not in (None, model):
            continue
        if row.get("gpu_type") not in (None, gpu_type):
            continue
        if "time_to_first_token_ms" in row:
            ttft_ms.append(float(row["time_to_first_token_ms"]))
        if "output_token_throughput_per_user" in row:
            tps.append(float(row["output_token_throughput_per_user"]))
        elif "tokens_per_second" in row:
            tps.append(float(row["tokens_per_second"]))
    if not ttft_ms and not tps:
        raise ValueError(f"no AIPerf rows matched model={model} gpu={gpu_type}")
    prefill_ms = (sum(ttft_ms) / len(ttft_ms) / 512.0) if ttft_ms else 0.08
    decode_tps = (sum(tps) / len(tps)) if tps else 120.0
    return {
        "prefill_ms_per_token": round(prefill_ms, 4),
        "decode_tps": round(decode_tps, 2),
        "max_batch": 32,
    }


def update_profile_yaml(
    profiles_dir: Path,
    *,
    model: str,
    gpu_type: str,
    curves: dict[str, float],
    runtime_seconds: float = 3600.0,
    gpu_memory_gb: float = 80.0,
) -> Path:
    path = profiles_dir / f"{model}.yaml"
    payload: dict[str, Any]
    if path.exists():
        payload = yaml.safe_load(path.read_text()) or {}
    else:
        payload = {"model": model, "profiles": {}}
    payload.setdefault("profiles", {})
    entry = payload["profiles"].setdefault(gpu_type, {})
    entry.update(
        {
            "runtime_seconds": runtime_seconds,
            "gpu_memory_gb": gpu_memory_gb,
            **curves,
            "calibrated_from_aiperf": True,
        }
    )
    lines = [f"model: {payload['model']}", "profiles:"]
    for gpu, gpu_entry in payload["profiles"].items():
        lines.append(f"  {gpu}:")
        for key, value in gpu_entry.items():
            if isinstance(value, bool):
                lines.append(f"    {key}: {'true' if value else 'false'}")
            elif isinstance(value, str):
                lines.append(f"    {key}: {value}")
            else:
                lines.append(f"    {key}: {value}")
    path.write_text("\n".join(lines) + "\n")
    return path


def export_aiperf_config(workload_path: Path, output_path: Path) -> None:
    workload = yaml.safe_load(workload_path.read_text()) or {}
    jobs = workload.get("jobs") or []
    requests = []
    for job in jobs:
        if not job.get("model_id"):
            continue
        requests.append(
            {
                "timestamp": job.get("arrival_time", 0.0),
                "model": job["model_id"],
                "input_sequence_length": job.get("input_tokens", 128),
                "output_sequence_length": job.get("output_tokens", 64),
            }
        )
    output_path.write_text(json.dumps({"requests": requests}, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description="AIPerf calibration adapter")
    sub = parser.add_subparsers(dest="command", required=True)
    imp = sub.add_parser("import", help="Import AIPerf JSON into profile YAML")
    imp.add_argument("results", type=Path)
    imp.add_argument("--profile", required=True, help="model name, e.g. llama-70b")
    imp.add_argument("--gpu-type", default="H100")
    imp.add_argument("--profiles-dir", type=Path, default=Path("configs/profiles"))
    exp = sub.add_parser("export", help="Export workload to AIPerf config JSON")
    exp.add_argument("workload", type=Path)
    exp.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "import":
        curves = extract_profile_curves(
            parse_aiperf_results(args.results),
            model=args.profile,
            gpu_type=args.gpu_type,
        )
        path = update_profile_yaml(
            args.profiles_dir,
            model=args.profile,
            gpu_type=args.gpu_type,
            curves=curves,
        )
        print(f"updated {path}")
    elif args.command == "export":
        export_aiperf_config(args.workload, args.output)
        print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
