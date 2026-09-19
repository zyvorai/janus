# Zyvor Janus Architecture

## Overview

Zyvor Janus is a discrete-event GPU cluster scheduler simulator inspired by Zynera. It separates a high-performance **Rust simulation core** from a thin **Python research API**, with optional FastAPI + Next.js dashboards and a benchmark/analytics layer for LLM serving metrics.

## Layers

```
Python (Gymnasium, notebooks, viz, FastAPI, AIPerf adapters)
        │
   PyO3 / maturin
        │
Rust workspace
  ├── zyvor-janus-core      Foundational primitives: error, events, decision log, stats
  ├── zyvor-janus-topology  Synthetic NVLink/PCIe bandwidth-penalty topology model
  ├── zyvor-janus-model     Cluster state, node/GPU/job types, MIG partitioning
  ├── zyvor-janus-scheduler Scheduling policies (fifo, priority, preemptive, zynera,
  │                         bestfit) + ResourceManager + the Scheduler trait
  ├── zyvor-janus-simulator Discrete-event engine, RL stepping session, inference model
  ├── zyvor-janus-cost      GPU cost model (seed for future multi-provider pricing)
  ├── zyvor-janus-metrics   Makespan, wait, utilization, timeline, benchmark score
  ├── zyvor-janus-config    YAML / Zynera bundle / scheduler + serving trace loaders
  ├── zyvor-janus-cli       zyvor-janus binary
  └── zyvor-janus-py        Python bindings (SimResult, SimSession)
```

Dependency graph: `core` and `topology` have no path-dependencies; `model`
depends on `{core, topology}`; `scheduler` depends on `{core, model,
topology}`; `simulator` depends on `{core, model, scheduler}`; `cost` has
no path-dependencies; `metrics` depends on `{core, model, cost}`; `config`
sits above all of the above; `cli` depends on `{config, metrics}`; `py`
depends on `{core, config, metrics, simulator}`.

**Planned, not yet built:** a `zyvor-janus-api` crate for a Rust-native
HTTP API server (today the API is Python/FastAPI, see `python/zyvor_janus/server/`).
When it's built, its name will need to be reconciled with the *already
published* `zyvor-janus-api` Docker image (`deploy/docker/Dockerfile.api`,
`.github/workflows/docker-publish.yml`), which is the existing Python
server, not this planned Rust crate.

## Simulation loop

1. Jobs arrive via `JobArrival` events
2. Scheduler selects waiting jobs and allocates GPUs (all-or-nothing); a
   preemptive scheduler may also evict lower-priority running jobs back
   into the waiting queue to make room
3. `ResourceManager` enforces tenant quotas, gang node spread, and NVLink-domain
   locality (with scatter fallback tracked as `topology_penalties`). Cross-domain
   placement inflates job runtime via `TopologyGraph` (`topology_runtime_inflation`).
4. Gang jobs with `gang_timeout_secs` schedule a `GangTimeout` event; jobs still
   waiting when it fires move to `JobState::Failed` (`jobs_failed` metric).
5. `JobComplete` events free resources and trigger re-scheduling — each
   carries the `Job::run_generation` it completes, so a stale event from a
   run that was preempted before finishing is ignored rather than
   corrupting the job's later, actual completion
6. Clock advances only to the next event (no polling)

## RL session (M7)

`RlSession` pauses the DES at scheduling decision points. An agent picks a
waiting job index (or noop); the session places it, advances time to the
next event, and returns a feature-vector observation plus wait-reduction
reward. Exposed to Python as `SimSession` and wrapped by `ZyvorJanusEnv`.

## Visualization (M8)

`SimulationReport` bundles aggregate metrics with a `JobsTimeline` JSON
(finished, running, and waiting jobs) and a `decisions` log for replay.
The CLI writes timeline via `--jobs-output`; Python `zyvor_janus.viz` renders
Gantt charts and GPU utilization heatmaps.

## UI stack

The Rust core never knows about the UI — it exposes APIs and events only.

```
Zyvor Janus Core (Rust)
        │
Python Bindings (PyO3: SimSession, SimResult, run_report_from_config)
        │
   ┌────┴────┐
   ▼         ▼
Rich CLI   FastAPI + WebSockets
dashboard      │
           Next.js dashboard
           (/ , /benchmark, /what-if)
```

| Phase | Deliverable | Location | Status |
|-------|-------------|----------|--------|
| 1 | Rich live terminal dashboard | `python/zyvor_janus/dashboard/` | Done |
| 2 | FastAPI run registry + replay API | `python/zyvor_janus/server/` | Done |
| 2 | Next.js monitor (home, compare, login) | `web/` | Done (MVP) |
| 4 | Benchmark + what-if pages | `web/src/app/benchmark`, `what-if` | Done (MVP) |

See [docs/ui_roadmap.md](ui_roadmap.md). **User guide:** [docs/ui_dashboard.md](ui_dashboard.md).

**Note:** Home links to `/runs/:id`, but a dedicated run-detail page is not shipped yet; use API artifacts under `outputs/runs/{uuid}/` for Gantt/replay data.

## Benchmark platform (MVP)

Zyvor Janus connects scheduling decisions to LLM serving metrics (TTFT, TPS, goodput), calibrated via AIPerf.

```text
Simulation Layer (Rust DES + inference model)
        → Benchmark Layer (serving traces, AIPerf adapter, OpenAI shim)
        → Analytics Layer (dashboard, twin store API, CI golden)
```

| Layer | Responsibility | Status |
|-------|----------------|--------|
| **Simulation** | DES, schedulers, cluster, MIG, topology, RL, inference model | M1–M8 + P1 done |
| **Benchmark** | Synthetic workloads, serving traces, AIPerf, OpenAI shim | MVP done; see gaps in [benchmark_platform.md](benchmark_platform.md) |
| **Analytics** | Benchmark/what-if UI, score reports, twin API, CI | MVP done; twin UI and full sim-vs-measured overlay still thin |

**Roadmap / gaps:** [docs/benchmark_platform.md](benchmark_platform.md) · **QA:** [manual_test_benchmark_platform.md](manual_test_benchmark_platform.md)

## Deploy

Docker images and Kubernetes manifests live under `deploy/`. See [deploy/kubernetes/README.md](../deploy/kubernetes/README.md) (`./deploy/build-images.sh`, `kubectl apply -k deploy/kubernetes`).

## Design invariants

- The Rust core never depends on Python or Gymnasium
- Schedulers share a common `Scheduler` trait (defined in `zyvor-janus-scheduler`) for benchmarking
- Zynera CRDs and traces convert to internal models via adapters before entering the engine
- Hardware is described by capability profiles (H100, H200, B200), not hardcoded logic

## Milestone scope (M1–M8)

| Milestone | Scope |
|-----------|-------|
| M1 | Whole-GPU placement, FIFO scheduler, YAML configs, metrics JSON |
| M2 | Zynera CRD bundle ingest |
| M3 | Scheduler trace replay + diff |
| M4 | MIG slice partition/reconfig delay |
| M5 | NVLink-domain placement, `topology_penalties`, runtime inflation |
| M6 | Quotas, priority, preemption, gang spread + timeout, `ZyneraScheduler`, `BestFitScheduler` |
| M7 | Stepped RL session + Gymnasium env + PPO baseline |
| M8 | Jobs timeline export + Gantt/heatmap viz |
| **P0–P10** | **Benchmark platform MVP** — inference model, AIPerf, shim, twin API, CI ([benchmark_platform.md](benchmark_platform.md)) |
