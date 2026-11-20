# Zyvor Janus — Complete page index

Every primary navigable dashboard route.

_Generated: 2026-08-29 · 7 routes_

Regenerate: `node scripts/customer-docs/generate-page-index.mjs`

## AUTH

| Page | Route | Purpose | Guide |
|------|-------|---------|-------|
| Login | `/login` | Sign in to the Zyvor Janus web console (session cookie). | [Open](pages/auth/login.md) |

## CONSOLE

| Page | Route | Purpose | Guide |
|------|-------|---------|-------|
| Dashboard | `/` | Mission Control — launch simulations, compare configs, and jump into recent runs. | [Open](pages/console/home.md) |
| Simulate | `/simulate` | Live scheduling replay — watch requests → queue → GPU assign → complete for a run. | [Open](pages/console/simulate.md) |
| Benchmark | `/benchmark` | Scheduler benchmarks with inference TTFT/TPS metrics, score vectors, and AIPerf twin comparison. | [Open](pages/console/benchmark.md) |
| What-if | `/what-if` | Sweep schedulers on the same inference workload and compare a results matrix. | [Open](pages/console/what-if.md) |
| Twins | `/twins` | AIPerf-calibrated GPU/model performance twins that ground the simulator against measured hardware. | [Open](pages/console/twins.md) |
| Run detail | `/runs/:id` | Inspect one simulation run — cluster/queue view, metrics, and jump to Simulate. | [Open](pages/console/runs-id.md) |

## Related

- [Customer docs home](README.md)
- [Page-by-page guides](pages/README.md)
