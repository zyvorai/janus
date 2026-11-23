# Trace import (Helios)

```bash
zyvor-janus import-trace --input cluster_log.csv --output workloads/helios.yaml \
  [--states COMPLETED,FAILED | all] [--max-jobs 100000] [--max-gpus-per-job 1024] \
  [--tenant-from-user] [--time-scale 60]
```

Converts a Microsoft Helios `cluster_log.csv`
(<https://github.com/S-Lab-System-Group/HeliosData>) into the workload YAML
used by `workload.path` in a simulation config. The header is matched by
column name, so column order does not matter; `job_id`, `gpu_num`, `state`,
`submit_time` and `duration` are required.

## Field mapping

| Helios column | Janus job field | Notes |
|---|---|---|
| `job_id` | `id` = `helios-<job_id>` | Duplicate ids are skipped (counted). |
| `gpu_num` | `gpu_count` | Must be a non-negative whole number. 0 is skipped; more than `--max-gpus-per-job` is skipped. |
| `submit_time` | `arrival_time` | Normalised so the earliest imported job arrives at 0, then divided by `--time-scale`. Accepts plain seconds or `YYYY-MM-DD HH:MM:SS`. The Helios page does not state its timestamp format, so both are accepted. |
| `duration` | `runtime` | Observed seconds, divided by `--time-scale`. Must be finite and > 0. |
| `vc` (or `user` with `--tenant-from-user`) | `tenant` | Hashed ids are kept as-is. |
| `state` | filter only | Default keeps `COMPLETED`. |
| `queue`, `start_time`, `end_time`, `cpu_num`, `node_num` | ignored | The original queueing is the *original* scheduler's behaviour; Janus re-derives waiting from its own policy. |

## Limits and honesty

- `runtime` is the observed run time. For `FAILED`/`CANCELLED` jobs it is truncated, so accepting those states understates demand.
- Helios records no GPU type, memory, topology or MIG demand: imported jobs carry none, and the cluster you simulate on is your choice. Results say how a *policy* behaves on Helios-shaped demand, not what Helios would have done.
- The imported arrival process keeps the original burstiness; `--time-scale` compresses it uniformly.
- Limits: input file <= 1 GiB, `--max-jobs` <= 2,000,000 (default 100,000); the summary reports how many rows were skipped per reason and the first five problems. Rows are split on commas (Helios has no quoted fields); a row with too few fields is skipped as malformed. NaN, infinite, negative or zero durations and unparseable timestamps are skipped, never propagated.
- The generated YAML passes the same workload validation as hand-written files (finite non-negative times, unique ids, size caps).
- Import is a pure function of the file and options: the same input gives byte-identical output.
