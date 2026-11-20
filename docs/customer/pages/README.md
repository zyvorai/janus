# Page-by-page guides

Each guide follows: Purpose → When to use it → How to get there → Operate from the console (UX) → Related pages.

Every route is also listed in the [complete page index](../PAGE_INDEX.md).

## Auth

| Page | What it covers |
|------|----------------|
| [Login](auth/login.md) | Sign in to the Zyvor Janus web console (session cookie). |

## Console

| Page | What it covers |
|------|----------------|
| [Benchmark](console/benchmark.md) | Scheduler benchmarks with inference TTFT/TPS metrics, score vectors, and AIPerf twin comparison. |
| [Dashboard](console/home.md) | Mission Control — launch simulations, compare configs, and jump into recent runs. |
| [Run detail](console/runs-id.md) | Inspect one simulation run — cluster/queue view, metrics, and jump to Simulate. |
| [Simulate](console/simulate.md) | Live scheduling replay — watch requests → queue → GPU assign → complete for a run. |
| [Twins](console/twins.md) | AIPerf-calibrated GPU/model performance twins that ground the simulator against measured hardware. |
| [What-if](console/what-if.md) | Sweep schedulers on the same inference workload and compare a results matrix. |

---

7 guides. Regenerate: `node scripts/customer-docs/generate-guide-index.mjs`.
