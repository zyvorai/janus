# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

> The project was renamed from ZyForgeSim/ForgeSim to **Zyvor Janus**.
> Historical entries below have been relabeled to the new name; the
> underlying milestones and code they describe are unchanged.

## [Unreleased]

### Dependencies and CI

Rust (`cargo update` plus major bumps, each compile-checked):
- axum 0.7 -> 0.8 (route paths `/:id` -> `/{id}`; WebSocket `Message::Text` now takes `Utf8Bytes`).
- tower-http 0.6 -> 0.7, rusqlite 0.32 -> 0.43 (no source changes needed).
- hmac 0.12 -> 0.13 and sha2 0.10 -> 0.11 (`KeyInit` must be imported for `new_from_slice`; the ws-ticket code already hex-encodes via `hex`, not `{:x}`).
- pyo3 0.23 -> 0.30 (`PyDict::new_bound`/`PyList::empty_bound` -> `new`/`empty`). Compile-checked with `cargo check` against Python 3.14; the maturin wheel build and Python test suite were NOT run (no pip on the dev host), so CI python.yml is the first full check.
- There is no `rand` dependency, so no seeding or stream behaviour changed; simulator determinism is untouched.

Web (`web/`, lockfile kept): next 14 -> 16 (`middleware.ts` -> `proxy.ts`, async `cookies()` and `params`), react/react-dom 18 -> 19 with @types, recharts 2 -> 3 (tooltip formatters take `ValueType | undefined`), framer-motion 12 -> 14, zustand 4 -> 5, typescript 5 -> 7, @types/node 20 -> 24. `npm run lint` is now `tsc --noEmit` because `next lint` was removed in Next 16 (there was no ESLint config).

Held back:
- tailwindcss 3 -> 4: needs a PostCSS plugin and config migration that changes generated CSS; not verifiable without a browser.

Images and CI: web image `node:20` -> `node:24-bookworm-slim`; API image `rust:1-bookworm`/`debian:bookworm-slim` -> `rust:1-trixie`/`debian:trixie-slim` (tags verified on Docker Hub). Actions: checkout v7, setup-python v7, docker/login v4, metadata v6, setup-buildx v4, build-push v7; Python in CI 3.10/3.11 -> 3.14 (package still supports >=3.10). Unverified and unchanged: `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`.

### Added

- **Scheduler-quality report** (`SimulationReport.quality`, CLI output, `run --report-output`): queue-wait p50/p90/p99, bounded slowdown, Jain fairness indices (per job and per tenant), 24-point utilisation and fragmentation timelines. See `docs/scheduler_quality.md`.
- **Seeded failure injection** (`failure:` config section, `run --seed`): job crashes with checkpointed restart and bounded retries; new metrics `failures_injected` and `lost_gpu_seconds`. Job-level crashes only, not a GPU-hardware failure model. See `docs/failure_injection.md`.
- **`import-trace`**: Microsoft Helios `cluster_log.csv` to workload YAML with explicit field mapping, skip accounting and size caps. See `docs/trace_import.md`.
- **`compare`**: baseline-vs-candidate policy comparison with paired 95% confidence intervals across seeds; honest single-run mode when there is no failure injection. See `docs/scheduler_quality.md`.
- `docs/RESEARCH.md`: survey of GPU-cluster trace formats, schedulers and fairness metrics, with ranked ideas and what was rejected.

### Fixed

- Workloads are now validated: NaN/infinite/negative arrival or runtime, duplicate job ids (a second running job with the same id silently overwrote the first), GPU counts above 100,000, more than 2,000,000 jobs, and files over 256 MiB are rejected with a clear error.
- Arrival handling was O(n^2) in the job count (linear scan of pending arrivals per event); it is now a hash lookup, so 20,000-job workloads run in well under a second.

### Changed
- Renamed the project from ZyForgeSim/ForgeSim to **Zyvor Janus**: Rust
  crates (`forgesim-*` -> `zyvor-janus-*`), the CLI binary
  (`zynera-sim` -> `zyvor-janus`), the Python package/PyO3 module
  (`forgesim` -> `zyvor_janus`), Docker images, the Kubernetes
  namespace, and all `FORGESIM_*` env vars (-> `ZYVOR_JANUS_*`).

### Added

#### Milestones M5–M8
- **M5 — Topology**: NVLink-domain placement, `topology_penalties`,
  runtime inflation via `TopologyGraph` when jobs span domains.
- **M6 — Scheduler features**: tenant GPU quotas, `PriorityScheduler`,
  `PreemptivePriorityScheduler`, gang node spread + timeout,
  `ZyneraScheduler` / `BestFitScheduler` (`fifo` | `priority` |
  `preemptive` | `zynera` | `bestfit`).
- **M7 — RL**: `RlSession` / `SimSession`, Gymnasium `ZyvorJanusEnv`,
  PPO baseline (`python/baselines/ppo_cleanrl.py`).
- **M8 — Visualization**: `--jobs-output` timeline JSON, Gantt/heatmap
  via `zyvor_janus.viz`.

#### UI
- Rich live terminal dashboard (`python/zyvor_janus/dashboard/`,
  `./scripts/run_live_dashboard.sh`).
- FastAPI run registry + Next.js web dashboard (`web/`) with login,
  compare, `/benchmark`, and `/what-if` routes.
- Dev scripts: `setup_dev.sh`, `run_web_*.sh`, `stop_web_dashboard.sh`,
  `clean.sh`, `fix_pyexpat.sh`.

#### Benchmark platform (P0–P10 MVP)
- Inference performance model (`inference.rs`) and profile v2 fields
  (`prefill_ms_per_token`, `decode_tps`, `max_batch`).
- Synthetic LLM workloads (`generate_synthetic.py`) and
  `serving.trace.v1` import/export (Rust + Python adapter; API export).
- `SchedulerBenchmarkReport` + cost model (`configs/analytics/cost.yaml`).
- OpenAI-compatible shim at `POST /v1/chat/completions` (API key + rate
  limit; analytical timing).
- AIPerf calibration adapter (`python -m zyvor_janus.benchmarks.aiperf_adapter`).
- Twin store API (`GET /api/twins`) and what-if / benchmark REST endpoints.
- CI workflow `.github/workflows/benchmark.yml` +
  `benchmarks/ci/run_golden.sh`.

#### Deploy
- Dockerfiles and `./deploy/build-images.sh`.
- Kubernetes manifests under `deploy/kubernetes/` (API + web + ingress).

### Fixed
- `zyvor-janus run --output` / `replay --output` no longer fail with "No such
  file or directory" when the target directory does not already exist.
- `zyvor-janus-py` failed to compile after `mig_reconfigs` was added to
  `SimulationMetrics` (M4); the Python `SimResult` binding now exposes it too.
- A stale `JobComplete` event scheduled before a job was preempted could,
  if that job was later resumed, fire after the resumed run had already
  started — incorrectly finishing it early and freeing its GPU while the
  resumed run was still actually using it. Fixed with a
  `Job::run_generation` counter that lets the engine tell a stale
  completion apart from the one that matches the job's current run.
- `--zynera-bundle` ingest silently found zero jobs/nodes/quotas when fed
  the output of `kubectl get <resource> -A -o yaml` — exactly the export
  command `docs/zynera_input.md` documents. `kubectl` wraps multiple
  resources in a single `kind: List` document with an `items:` array
  instead of `---`-separating them; the YAML parser only ever split on
  `---` and never unwrapped `List`. Found by exporting and replaying a
  real Zynera deployment. `yaml_documents()` now unwraps `kind: List`.

### Added (earlier Unreleased slices)
- CI workflows for Rust (`fmt`, `clippy`, unit + integration tests) and
  Python (`maturin build` + `unittest`).
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `rust-toolchain.toml`.
- M6 (quotas slice): `FabricQuota.spec.gpuQuota.maxGPUs` is now enforced
  per tenant at placement time — a job that would push its tenant over
  quota stays queued until another of that tenant's jobs frees capacity.
  Internal YAML configs can set the same limits via
  `cluster.tenant_quotas`. See `docs/zynera_input.md` and
  `docs/design/m6_scheduler_features.md`.
- M6 (priority scheduler slice): `PriorityScheduler` now really schedules
  (previously a no-op stub) — orders the waiting queue by highest
  `priority` first, ties broken by earliest arrival. Select it with
  `scheduler.type: priority` in internal YAML configs or `--scheduler
  priority` on `zyvor-janus run --zynera-bundle` / `zyvor-janus replay`. Does
  not preempt already-running jobs.
- M6 (preemption slice): new `PreemptivePriorityScheduler`
  (`scheduler.type: preemptive` / `--scheduler preemptive`) — a waiting
  job may evict lower-priority running jobs to fit. Evicted jobs resume
  later with their remaining runtime (no restart penalty by default), and
  become exempt from further preemption after 3 evictions. New
  `SimulationMetrics.preemptions` field, printed by the CLI as
  `preemptions:` when nonzero. See `docs/zynera_input.md` and
  `docs/design/m6_scheduler_features.md`.

## [0.1.0] — M1–M4

- **M1 — Simulation core**: discrete-event engine, FIFO scheduler, internal
  YAML workload configs, CLI, Python bindings.
- **M2 — Zynera compatibility**: `FabricAIJob`/`FabricGpuNode`/`FabricQuota`
  ingest via `ZyneraBundleAdapter`, calibrated runtime profiles,
  `--zynera-bundle` CLI flag.
- **M3 — Trace replay**: JSONL scheduler event replay with oracle vs.
  simulated placement diff reporting.
- **M4 — MIG simulation**: fractional GPU slice allocation with simulated
  reconfiguration delay.

See [docs/milestones.md](docs/milestones.md) for full success criteria per
milestone.
