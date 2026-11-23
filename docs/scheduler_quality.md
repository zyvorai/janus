# Scheduler-quality metrics and policy comparison

## Quality report

Every `run` now computes a `QualityReport` (in `SimulationReport.quality`,
printed by the CLI, and written with `--report-output report.json`). It is a
pure function of the finished cluster, so it is exactly as deterministic as
the run.

| Field | Definition |
|---|---|
| `wait_p50/p90/p99/max` | Cumulative queue wait of finished jobs (percentile = nearest rank, same helper as the existing metrics). |
| `slowdown_mean/p99/max` | Bounded slowdown `max((finish - arrival) / max(runtime, 10 s), 1)`. `runtime` is the requested runtime, i.e. the isolated run on an empty cluster. |
| `jain_job_service` | Jain index of per-job service rate `1 / slowdown`. 1.0 = every job slowed equally. |
| `jain_tenant_service` | Jain index of per-tenant mean service rate (untenanted jobs form one group). |
| `jobs_never_started` | Jobs still waiting when the event queue emptied. |
| `utilization_timeline` | 24 equally spaced samples (bucket midpoints) of busy GPUs and utilisation, from run segments. A MIG GPU counts busy if any slice is busy. |
| `fragmentation` (per point) and `mean_fragmentation` | `1 - largest free block on one node / total free GPUs` (0 when none free). High = free GPUs are scattered, so a multi-GPU single-node job could not use them. |

Not the Themis finish-time-fairness rho: the isolated baseline here is the
job alone on an empty cluster, not a 1/N cluster share. Slowdown is
computed on successfully finished jobs only; failed jobs show up in
`jobs_failed`.

## Policy comparison

```bash
zyvor-janus compare --config configs/clusters/failure_injection.yaml \
  --baseline fifo --candidate bestfit --seeds 20 [--first-seed 1] [--output cmp.json]
```

Runs both policies on the same config for each seed (`first-seed ..
first-seed+seeds`), pairs the results per seed, and reports for nine metrics
the mean paired difference (candidate - baseline) with a 95% Student-t
confidence interval and a verdict (`candidate_better`, `candidate_worse`,
`inconclusive`) taking each metric's direction into account.

Honesty notes:

- The simulator is deterministic, so seeds only change the injected crash
  schedule. **If the config has no `failure:` section the comparison is a
  single run per policy with no interval** (a CI over identical runs would be
  meaningless); the report says so.
- Seeds are paired, but the workload is identical for every seed: the
  interval reflects crash-schedule variance, not workload sampling. Compare on
  several workloads before generalising.
- Nine metrics are tested at 95% each; expect about one spurious
  non-zero verdict in 20 comparisons across metrics. No multiple-comparison
  correction is applied.
- At most 1000 seeds; with 1 seed there is no interval.
