# Common workflows

## First CLI run

1. [Getting Started](getting-started.md) — build + `run --config configs/clusters/small_h100.yaml`
2. Confirm the run completes; note TTFT/TPS only for LLM-serving configs

## Console: run → simulate → inspect

1. `./scripts/run_web_dashboard.sh` → [Login](pages/auth/login.md)
2. [Dashboard](pages/console/home.md) → pick config → **Run**
3. Open [Run detail](pages/console/runs-id.md) → **Open in Simulate**
4. Watch Accept→assign decisions on [Simulate](pages/console/simulate.md)

## Compare two configs

1. Dashboard → **Compare configs** (A vs B)
2. Or CLI two runs + Benchmark / What-if for scheduler sweeps

## Scheduler what-if sweep

1. [What-if](pages/console/what-if.md) → select schedulers → **Run sweep**
2. Download CSV; open best candidate run detail

## Benchmark with twins

1. Calibrate / list [Twins](pages/console/twins.md)
2. [Benchmark](pages/console/benchmark.md) → **Run benchmark** → read sim vs measured

## Related

- [Configuration](configuration.md)
- [Using the Dashboard](using-the-dashboard.md)
