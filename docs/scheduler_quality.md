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

Slowdown is computed on successfully finished jobs only; failed jobs show up
in `jobs_failed`.

### Finish-time fairness (Themis-style rho)

`ftf_rho_mean/p90/max`, `ftf_worst_tenant`, `ftf_worst_tenant_rho`.
Per finished job, `rho = (finish - arrival) / (max(runtime, 10 s) * N)`, where
`N` is the time-averaged number of tenants that have at least one job in the
system during the job's lifetime (minimum 1; untenanted jobs count as one
tenant). `rho <= 1` means the job finished no later than an equal 1/N tenant
share would predict; bigger is worse. This is a simplification of Themis
(Mahajan et al., NSDI 2020): it assumes runtime scales as 1/share, which gang
jobs do not, and it ignores per-job GPU demand. Use it to compare policies on
one workload, not as an absolute SLO. `ftf_worst_tenant` is the tenant with the
highest mean rho.

## Chargeback (`SimulationReport.chargeback`)

A FinOps-style showback computed from the finished cluster and the flat
`$/GPU-hour` rate in `analytics/cost.yaml` (default $3.50):

| Field | Definition |
|---|---|
| `used_gpu_hours`, `used_cost_usd` | Sum of each job's consumed GPU-seconds (includes work later lost to preemption or crashes). |
| `capacity_gpu_hours` | Total GPUs times the horizon (last finish time, or the clock if later). |
| `idle_gpu_hours`, `idle_cost_usd` | Capacity nobody used over the horizon. |
| `wasted_gpu_hours`, `wasted_cost_usd` | GPU-time lost to injected crashes (already counted inside used). |
| `tenants[]` | Per-tenant jobs, GPU-hours, cost and share, sorted by cost. Untenanted jobs are "(none)". |

One flat rate: no per-GPU-type pricing, reservations or spot discounts.
`run` prints the totals and a per-tenant table; `--report-output` writes the
full object.

## EASY backfill scheduler (`easy`)

`--scheduler easy` / `scheduler: {type: easy}` / `compare --candidate easy`.
Jobs start in arrival order. The first job that does not fit gets a
reservation at the earliest time enough GPUs free up (from the running jobs'
remaining runtimes). Later jobs may start early only if they finish before
the reservation or fit in the GPUs the head will not need, so the head is
never delayed by backfilled work. `fifo` here skips blocked jobs with no
reservation, which can starve wide jobs; `configs/clusters/backfill_demo.yaml`
shows the difference (the 4-GPU head starts at t=100 under `easy`, t=502 under
`fifo`).

Limits: the reservation counts GPUs only (not node packing, topology, quotas
or MIG) and uses true runtimes rather than noisy user estimates, so on real
systems backfill is less exact than here.

## Leaderboard

```bash
zyvor-janus leaderboard --config configs/clusters/backfill_demo.yaml \
  [--schedulers fifo,priority,preemptive,bestfit,easy,sjf,fairshare] \
  [--sort-by mean_wait_time] [--seed N] [--output lb.json]
```

Runs the config once per scheduler and ranks them. Sort keys: `makespan`,
`mean_wait_time`, `wait_p99`, `jct_mean`, `slowdown_mean`, `ftf_rho_max`, `cost_usd`
(lower is better) and `gpu_utilization`, `jain_tenant_service`, `slo_attainment` (higher is
better). Ties break on scheduler name. `compare` additionally reports
`ftf_rho_max`.

## Policy comparison

```bash
zyvor-janus compare --config configs/clusters/failure_injection.yaml \
  --baseline fifo --candidate bestfit --seeds 20 [--first-seed 1] [--output cmp.json]
```

Runs both policies on the same config for each seed (`first-seed ..
first-seed+seeds`), pairs the results per seed, and reports for ten metrics
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
