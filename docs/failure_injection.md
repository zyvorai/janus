# Failure injection

Add a `failure:` section to a simulation config:

```yaml
failure:
  seed: 1              # u64; override with `zyvor-janus run --seed N`
  job_mtbf_secs: 1500  # mean simulated seconds between crashes of a running job
  checkpoint_secs: 120 # progress is saved every N s of run time (0 = never)
  max_retries: 3       # restarts allowed; the next crash marks the job Failed
```

Example: `configs/clusters/failure_injection.yaml`.

## Model

Every *run* of a job (its first start and each restart) draws an independent
exponential time-to-crash with mean `job_mtbf_secs`. If that is shorter than
the run would take, a `job_failure` event kills the job at that moment: its
GPUs are freed, progress since the last checkpoint is lost and the job
re-enters the queue with the lost work added back. After `max_retries`
restarts the next crash ends the job as `Failed` (counted in `jobs_failed`).
New metrics: `failures_injected`, `lost_gpu_seconds`; the decision log has
`job_failure` / `job_failure_final` entries.

## Determinism

The draw is a pure function of `(seed, job id, run generation)` (splitmix64
over an FNV-1a hash of the job id), not of a shared random stream, so the
same seed and config always give the same result regardless of event order,
and no external RNG crate can change the stream. Different seeds give
different crash schedules. Without a `failure:` section nothing changes.

## What this is not

- It is not a GPU or node hardware-failure model. A crash kills one job; the
  GPU stays healthy and immediately reusable. Modelling devices going out of
  service needs MTBF/MTTR data this project does not have.
- The exponential (memoryless) distribution is an assumption, not a fit to
  any trace. Calibrate `job_mtbf_secs` from your own failure rates.
- Restart cost is only the lost work; there is no restore/queueing overhead
  beyond what the scheduler itself imposes.
- Progress is counted in wall-clock run time, so a topology-inflated run
  loses the inflated time.

Limits: `job_mtbf_secs` must be finite and > 0, `checkpoint_secs` finite and
>= 0, `max_retries` <= 1000 (bounds events per job). `--seed` without a
`failure:` section is an error rather than silently ignored.
