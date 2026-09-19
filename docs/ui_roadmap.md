# Zyvor Janus UI Roadmap

Zyvor Janus's UI grows in stages on top of the Rust core and PyO3 bindings. The engine never depends on UI code.

**Full user guide:** [ui_dashboard.md](ui_dashboard.md)

## Architecture

```
Zyvor Janus Core (Rust)
        │
Python Bindings (PyO3)
        │
   ┌────┴────────────────┐
   ▼                     ▼
Rich CLI dashboard    FastAPI (REST + WebSocket)
                           │
                      Next.js web dashboard
```

## Scripts (quick reference)

| Script | Purpose |
|--------|---------|
| `./scripts/setup_dev.sh` | One-time `.venv` + Rust extension setup |
| `./scripts/run_live_dashboard.sh` | Rich terminal dashboard |
| `./scripts/run_web_dashboard.sh` | Web API + UI together |
| `./scripts/run_web_api.sh` | FastAPI only (:8080) |
| `./scripts/run_web_ui.sh` | Next.js only (:3000) |
| `./scripts/stop_web_dashboard.sh` | Stop API + UI |

## Phase 1 — Rich CLI dashboard (done)

- **Module:** `python/zyvor_janus/dashboard/`
- **Run:** `./scripts/run_live_dashboard.sh --config configs/clusters/small_h100.yaml`
- **Data:** `SimSession.step_fifo()` + extended `ClusterSnapshot`

## Phase 2 — Web dashboard (done)

- **Backend:** `crates/zyvor-janus-api` (Rust) — replaced `python/zyvor_janus/server/app.py` in prod; see Cargo cutover commits
- **Frontend:** `web/` — `./scripts/run_web_ui.sh` or `./scripts/run_web_dashboard.sh`
- **Views:** home (run + compare), login, `/runs/:id` (overview, cluster, queue, MIG, replay, shadow-race tabs; live WS streaming with polling fallback)

## Phase 3 — Zynera integration (future)

| Mode | Source |
|------|--------|
| Simulation | YAML / zynera bundle → `zyvor-janus-api` |
| Replay | M3 trace JSONL → event stream |
| Live | Zynera export → `ClusterSnapshot` mapping |

Long-term vision: **Grafana meets Kubernetes Dashboard meets DCGM — focused on AI scheduling**.

## Phase 4 — Benchmark dashboard (done)

Extends Phase 2 web UI — not a separate app. See [benchmark_platform.md](benchmark_platform.md).

| Route | Purpose | Status |
|-------|---------|--------|
| `/benchmark` | TTFT, TPS, goodput, benchmark runs, sim-vs-measured AIPerf twin overlay | Done |
| `/what-if` | Cluster/scheduler sweep matrix | Done (table; Pareto TBD) |
| `/twins` | Calibrated GPU/model twin library | Done |

Backend: inference metrics (P1), AIPerf adapter (P7), score reports (P4). `GET /api/configs` resolves a best-effort `(gpu_type, model)` hint per config (`zyvor_janus_config::resolve_config_twin_hint`) so the frontend can correlate a benchmark run against its calibrated twin from `GET /api/twins`.
