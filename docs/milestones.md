# Zyvor Janus Milestones

| Milestone | Status | Deliverable |
|-----------|--------|-------------|
| **M1 — Simulation core** | Done | DES, FIFO, internal YAML, CLI, Python bindings |
| **M2 — Zynera Compatibility** | Done | Multi-CRD ingest, corrected mappings, profiles, `--zynera-bundle` CLI |
| **M3 — Trace replay** | Done | Scheduler event JSONL replay + oracle vs FIFO diff report |
| **M4 — MIG simulation** | Done | MIG slice partition/reconfig with simulated delay |
| **M5 — Topology** | Done | NVLink-domain placement, `topology_penalties`, runtime inflation via `TopologyGraph` |
| **M6 — Zynera scheduler features** | Done | Quotas, priority, preemption, gang spread + timeout, `zynera`/`bestfit` schedulers |
| **M7 — RL** | Done | Gymnasium wrapper, PPO baseline, stepped `RlSession` |
| **M8 — Visualization** | Done | Gantt, heatmaps, `--jobs-output` timeline JSON |

## M2 success criteria

- [x] `ZyneraBundleAdapter` loads `FabricAIJob`, `FabricGpuNode`, `FabricQuota`
- [x] Correct GPU count for distributed/gang jobs (32 not 8)
- [x] Tenant resolved from `FabricQuota`, not job spec
- [x] Calibrated profiles with fail-on-missing runtime
- [x] `zyvor-janus run --zynera-bundle` CLI
- [x] Golden fixtures in `tests/fixtures/zynera/`
- [x] Export workflow documented in `docs/zynera_input.md`

## M3 success criteria

- [x] JSONL trace format with `JobSubmitted` / `JobScheduled` events
- [x] `TraceAdapter` (Python) + `zyvor-janus-config::trace` (Rust)
- [x] `zyvor-janus replay --trace` CLI with cluster from config or zynera bundle
- [x] Oracle vs simulated placement diff report JSON
- [x] Golden fixture `tests/fixtures/traces/fifo_match.jsonl`

## Quick commands

```bash
# Internal synthetic workload (M1)
cargo run -p zyvor-janus-cli -- run --config configs/clusters/small_h100.yaml

# Zynera export bundle (M2)
cargo run -p zyvor-janus-cli -- run --zynera-bundle tests/fixtures/zynera --profiles-dir configs/profiles

# Trace replay + decision diff (M3)
cargo run -p zyvor-janus-cli -- replay \
  --trace tests/fixtures/traces/fifo_match.jsonl \
  --config configs/clusters/single_gpu.yaml

# Priority / preemption (M6)
cargo run -p zyvor-janus-cli -- run --config configs/clusters/preemption_preemptive.yaml

# Topology + gang (M5 / M6)
cargo run -p zyvor-janus-cli -- run --config configs/clusters/topology_penalty.yaml
cargo run -p zyvor-janus-cli -- run --config configs/clusters/gang_m6.yaml
cargo run -p zyvor-janus-cli -- run --config configs/clusters/gang_timeout_m6.yaml

# Inference metrics (P1)
cargo run -p zyvor-janus-cli -- run --config configs/clusters/inference_llama.yaml

# Timeline export + viz (M8)
cargo run -p zyvor-janus-cli -- run \
  --config configs/clusters/small_h100.yaml \
  --jobs-output outputs/jobs.json

# Web / benchmark UI
./scripts/setup_dev.sh && cd web && npm install && cd ..
./scripts/run_web_dashboard.sh   # /, /benchmark, /what-if
```

## Running tests

```bash
# Rust unit tests (all crates)
cargo test --workspace --exclude zyvor-janus-py

# Rust integration tests
cargo test -p zyvor-janus-config --test integration
cargo test -p zyvor-janus-cli --test cli_integration

# Python unit + integration tests
PYTHONPATH=python python3 -m unittest discover -s python/tests -v

# Benchmark golden (CI)
bash benchmarks/ci/run_golden.sh
```

### MIG simulation (M4)

```bash
cargo run -p zyvor-janus-cli -- run --config configs/clusters/mig_single.yaml
```

## M4 success criteria

- [x] MIG profiles in `configs/mig/` (H100 1g/2g/3g/7g)
- [x] Jobs with `mig_profile` + `mig_count` allocate slices, not whole GPUs
- [x] Reconfiguration delay simulated (`reconfig_seconds: 30`)
- [x] `mig_reconfigs` tracked in metrics output
- [x] Zynera `spec.mig.profile/count` mapped at ingest (M2) and simulated (M4)

## M5 success criteria

- [x] Jobs with `network_bw_gbps` or `gang_enabled` prefer same `nvlink_group`
- [x] Fallback scatter placement increments `topology_penalties` in metrics
- [x] Cross-domain placement inflates job runtime via `topology_runtime_inflation`
- [x] `TopologyGraph` built from hardware profile NVLink/PCIe bandwidths
- [x] Integration test `integration_topology_workload_completes`
- [x] Example config `configs/clusters/topology_h100.yaml`

## M6 success criteria

- [x] Quotas: `FabricQuota.spec.gpuQuota.maxGPUs` enforced per tenant at placement time
- [x] Priority scheduler: `scheduler.type: priority` / `--scheduler priority`
- [x] Preemption: `scheduler.type: preemptive` / `--scheduler preemptive`
- [x] Zynera scheduler: `scheduler.type: zynera` / `--scheduler zynera` (alias for preemptive priority)
- [x] Best-fit: `scheduler.type: bestfit` / `--scheduler bestfit` (tightest-node GPU packing)
- [x] Gang: `gang_enabled` + `gang_size_nodes` require GPUs across N distinct nodes (all-or-nothing)
- [x] Gang timeout: `gang_timeout_secs` / `zynera.ai/gang-timeout` fails waiting gang jobs (`jobs_failed` metric)
- [x] Integration tests for priority, preemption, gang

## M7 success criteria

- [x] `RlSession` stepped DES interface in Rust
- [x] `SimSession` PyO3 bindings (`reset`, `observe`, `step`, `metrics`)
- [x] `ZyvorJanusEnv` Gymnasium wrapper
- [x] PPO baseline in `python/baselines/ppo_cleanrl.py`
- [x] Integration test `integration_rl_session_fifo_completes`

### RL (M7)

```bash
./scripts/setup_dev.sh
source .venv/bin/activate
pip install -e '.[rl]'
python python/examples/run_rl_env.py
python python/baselines/ppo_cleanrl.py --config configs/clusters/rl_small.yaml
PYTHONPATH=python python3 -m unittest python.tests.test_rl_env -v
```

## M8 success criteria

- [x] `JobsTimeline` JSON export via `--jobs-output`
- [x] Python `zyvor_janus.viz` module (Gantt + GPU heatmap)
- [x] Example script `python/examples/plot_run.py`
- [x] Integration test `integration_simulation_writes_jobs_timeline`

### Visualization (M8)

```bash
cargo run -p zyvor-janus-cli -- run \
  --config configs/clusters/small_h100.yaml \
  --jobs-output outputs/jobs.json
pip install -e '.[viz]'
python python/examples/plot_run.py outputs/jobs.json
```

## Benchmark platform (P0–P10)

Extends Zyvor Janus from GPU scheduler simulation into a platform connecting **scheduling decisions** to **LLM serving metrics** (TTFT, TPS, goodput), with AIPerf calibration and optional digital twin.

| Phase | Status | Deliverable |
|-------|--------|-------------|
| **P0** | MVP done | Scheduler override on API runs, snapshots, run metadata / config hash |
| **P1** | MVP done | Inference performance model — TTFT/TPS from profile v2 |
| **P2** | MVP done | Synthetic LLM workload generator + golden fixture |
| **P3** | Partial | `serving.trace.v1` Rust/Python I/O + API export; **no CLI `--serving-trace` yet** |
| **P4** | MVP done | `SchedulerBenchmarkReport` + cost model; optional composite score |
| **P5** | MVP done | `/benchmark` page + benchmark APIs (sim-vs-measured overlay still thin) |
| **P6** | MVP done | OpenAI-compatible shim (auth, rate limit, analytical timing) |
| **P7** | MVP done | AIPerf adapter import/export + fixtures |
| **P8** | Partial | `/what-if` + sweep API; no cluster templates / Pareto chart yet |
| **P9** | Partial | TwinStore SQLite + `GET /api/twins`; no twin library UI |
| **P10** | MVP done | `benchmark.yml` + `benchmarks/ci/run_golden.sh` |

**Full roadmap + remaining gaps:** [docs/benchmark_platform.md](benchmark_platform.md)

**QA guide:** [docs/manual_test_benchmark_platform.md](manual_test_benchmark_platform.md)
