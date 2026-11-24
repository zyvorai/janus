# Workload generation, SLOs, load sweeps and new policies

Added after reading the sources listed in [RESEARCH.md](RESEARCH.md) ("Round 3").

## New schedulers

| Name | Rule | Reference |
|---|---|---|
| `sjf` | Non-preemptive shortest-job-first on the declared `runtime` (an oracle; real clusters only have estimates). Ties by arrival, then id. Starves long jobs under sustained load: compare `wait_max` / `slowdown_max`. | Duration-aware baseline used in Tiresias / Gavel comparisons. |
| `fairshare` | Max-min over GPUs held per tenant: each step serves the tenant holding the fewest GPUs (running jobs plus placements already made this pass), FIFO inside a tenant. Untenanted jobs form one group. It does not preempt, so a tenant that already holds the cluster is only equalised as jobs finish. | Single-resource case of DRF (Volcano) and the max-min policy in Gavel. |

Both are accepted everywhere a scheduler name is (config `scheduler.type`, `--scheduler`, `compare`, `leaderboard`, `load-sweep`, trace replay, API `scheduler` fields, web selector).

## JCT metrics

`QualityReport.jct_mean/p50/p90/p99` = `finish - arrival` of finished jobs (includes waiting and failure restarts). JCT is the headline metric of Tiresias, Gavel and Pollux. `leaderboard --sort-by jct_mean` and `jct_p99` in the leaderboard rows.

## SLO attainment

Add an `slo:` section to a config (at least one limit; all given limits must hold):

```yaml
slo:
  max_wait_secs: 60      # cumulative queue wait
  max_slowdown: 3.0      # bounded slowdown (>= 1)
  max_jct_secs: 900      # finish - arrival
```

`SimulationReport.slo` then has `attainment` (met / all jobs; failed, unstarted and unfinished jobs count as misses), per-tenant attainment and the worst tenant. `leaderboard --sort-by slo_attainment` ranks by it. There is no universal SLO; the limits are yours.

## Workload generator

`zyvor-janus generate-workload --jobs 500 --seed 1 --rate 0.05 --median-runtime 600 --sigma 1.0 --gpu-mix 1:0.7,2:0.15,4:0.1,8:0.05 --tenants 4 --tenant-skew 1.0 -o configs/workloads/gen.yaml`

Poisson arrivals (exponential gaps), log-normal runtimes clamped to `[1, max_runtime]`, a discrete GPU-size mix, Zipf tenant popularity, optional priority levels. Every draw is a stateless hash of `(seed, stream, job index)`: same spec, same file, independent of any RNG crate. API: `POST /api/workloads/generate` (same fields as the spec, returns `{jobs, yaml}`). Limits: 100,000 jobs, 1,000 tenants. This produces *shaped* synthetic load for what-if studies, not a fitted model of any production trace.

## Load sweep

`zyvor-janus load-sweep -c configs/clusters/backfill_demo.yaml --schedulers fifo,easy,sjf --loads 0.5,1,2,4 --max-wait-p90 600`

Replays the config with every arrival time divided by the load factor (2 = arrivals twice as fast; runtimes and sizes untouched) and prints JCT mean/p99, wait p90, utilisation and SLO attainment per scheduler and load. `sustainable_load` is the highest swept load such that it and every lower swept load kept wait p90 <= `--max-wait-p90` with no unstarted job; this is a configurable proxy for the "sustainable load" idea in the Gavel evaluation, not their exact definition. Limits: 50 loads, 12 schedulers, load in (0, 1000]. API: `POST /api/load-sweep`.

## API

`GET /api/runs/{id}` and `POST /api/benchmark/run` now also return `quality`, `chargeback` and `slo` (they previously dropped them). Config names containing `/`, `\` or `..` are rejected with 400.
