# Scheduler Benchmark Score (P4)

This document defines the Zyvor Janus scheduler benchmark report and optional composite score. **MVP implemented** in `crates/zyvor-janus-metrics` (`SchedulerBenchmarkReport`). See the [benchmark platform roadmap](benchmark_platform.md).

## Purpose

Compare scheduling policies on both **cluster efficiency** (utilization, fragmentation, fairness) and **serving quality** (TTFT, TPS, goodput) when inference jobs are present.

## Metric vector

Each run can produce a `SchedulerBenchmarkReport` with fields including:

| Category | Metric | Definition |
|----------|--------|------------|
| Scheduling | `makespan` | Simulation clock at last job completion |
| Scheduling | `mean_cumulative_wait` | Mean queue wait across completed jobs |
| Scheduling | `gpu_utilization` | GPU-seconds busy / (makespan × GPU count) |
| Scheduling | `queue_delay_p99` | 99th percentile queue delay (inference jobs) |
| Scheduling | `preemptions` | Total preemption count |
| Scheduling | `topology_penalties` | Cross-domain placement count |
| Serving | `ttft_p50`, `ttft_p99` | Time to first token percentiles (ms) — **not** `time_to_first_start` |
| Serving | `itl_p50` | Inter-token latency percentile (ms) |
| Serving | `tps_mean` | Mean decode tokens/sec |
| Serving | `goodput` | **MVP:** `inference_jobs / jobs_total` (share of jobs that are inference). SLA-fraction goodput is **not** implemented yet |
| Fairness | `jain_fairness` | Jain fairness index across tenants |
| Efficiency | `fragmentation` | Idle GPU-time / total GPU-time |
| Cost | `gpu_hour_cost` | `gpu_seconds × rate` from `configs/analytics/cost.yaml` |

Golden fixture: `tests/fixtures/benchmark/score_vector.json`.

## Composite score (optional)

`SchedulerBenchmarkReport::composite_score(weights)` accepts a weight map. A checked-in `configs/analytics/score_weights.yaml` is **not** shipped yet — callers pass weights in code/API.

Illustrative weights (planned config shape):

```yaml
# configs/analytics/score_weights.yaml (not yet in tree)
weights:
  ttft_p99: 0.25
  goodput: 0.25
  gpu_utilization: 0.20
  makespan: 0.15
  cost: 0.15
direction:
  ttft_p99: lower_is_better
  goodput: higher_is_better
  gpu_utilization: higher_is_better
  makespan: lower_is_better
  cost: lower_is_better
```

Default UI behavior: show the **full metric vector**. Composite score is opt-in.

## Cost model

```yaml
# configs/analytics/cost.yaml
gpu_hour_usd: 3.50
```

## SLA / goodput (future)

SLA-based goodput would require per-workload ceilings, for example:

```yaml
sla:
  ttft_p99_ms: 500
  e2e_latency_p99_ms: 5000
```

Until that lands, treat `goodput` as the inference-job fraction documented above — do not interpret it as SLO attainment.

## Related docs

- [Benchmark platform roadmap](benchmark_platform.md)
- [M6 scheduler features](design/m6_scheduler_features.md)
- [Manual test guide](manual_test_benchmark_platform.md)
