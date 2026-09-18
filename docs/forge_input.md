# Forge Input Adapters

Zyvor Janus stays independent from Zyvor Forge (https://zyvor.dev/forge) via an adapter layer. Adapters convert external representations into internal `Job` objects before simulation.

## Primary goal

Test Zyvor Forge scheduling **without physical H100/H200 GPUs** by exporting Forge CRDs, attaching calibrated runtime profiles, and running simulations locally.

## Adapter levels

| Level | Source | Adapter | Status |
|-------|--------|---------|--------|
| 1 | Internal YAML workloads | `YamlAdapter` | Done (M1) |
| 1b | Forge export bundle (CRDs) | `ForgeBundleAdapter` | Done (M2) |
| 2 | Scheduler event export | `TraceAdapter` | Done (M3) |
| 3 | REST API stream | `RESTAdapter` | Planned |
| 4 | K8s / Slurm / Ray traces | `TraceReplayAdapter` | Planned |

## Export from Zyvor Forge

```bash
mkdir -p forge-export/{jobs,cluster,quotas}

kubectl get fabricaijobs -A -o yaml > forge-export/jobs/all.yaml
kubectl get fabricgpunodes -o yaml > forge-export/cluster/nodes.yaml
kubectl get fabricquotas -A -o yaml > forge-export/quotas/all.yaml
```

Run simulation:

```bash
cargo run -p zyvor-janus-cli -- run \
  --forge-bundle tests/fixtures/forge \
  --profiles-dir configs/profiles
```

## FabricAIJob → internal Job

Forge uses `apiVersion: forge.ai/v1`, `kind: FabricAIJob` (not `FabricTrainingJob`).

**Job manifest:**

```yaml
apiVersion: forge.ai/v1
kind: FabricAIJob
metadata:
  name: gpt-distributed-training
  namespace: ml-training
  annotations:
    forge.ai/gang-schedule: "true"
    forge.ai/gang-size: "4"
spec:
  type: training
  model: gpt-13b
  gpus: 8
  gpuType: H100
  network: rdma
  priority: 80
  distributed:
    enabled: true
    nodes: 4
    gpusPerNode: 8
```

**Tenant via FabricQuota (separate file):**

```yaml
apiVersion: forge.ai/v1
kind: FabricQuota
metadata:
  name: ml-training-quota
  namespace: ml-training
spec:
  team: ml-training
  namespaces:
    - ml-training
  gpuQuota:
    maxGPUs: 64
```

## Field mapping (authoritative)

| Forge field | Internal Job field |
|-------------|-------------------|
| `metadata.name` + `metadata.namespace` | `id`, `name`, `namespace` |
| `spec.distributed.enabled` → `nodes × gpusPerNode`, else `spec.gpus` | `gpu_count` |
| `spec.priority` | `priority` (0–100) |
| `spec.gpuType` | `gpu_type` → hardware profile via registry |
| `spec.network` | `network_bw_gbps` hint |
| `spec.mig.profile/count` | `mig_profile`, `mig_count` (simulated M4) |
| `metadata.annotations[forge.ai/gang-schedule]` | `gang_enabled` |
| `metadata.annotations[forge.ai/gang-size]` | `gang_size_nodes` |
| `metadata.annotations[forge.ai/gang-timeout]` | `gang_timeout_secs` (e.g. `"10m"` → 600) |
| `FabricQuota.spec.team` + job namespace | `tenant` |
| Calibrated profile `(model, gpuType)` | `runtime`, `gpu_memory_gb` |

**GPU count rule:** When `spec.distributed.enabled` is true, total GPUs = `nodes × gpusPerNode` (gang example: 4×8 = **32**, not `spec.gpus` alone).

## Tenant GPU quotas (M6)

`FabricQuota.spec.gpuQuota.maxGPUs` is enforced at placement time: a job
whose tenant already holds `maxGPUs` GPUs across running jobs stays queued
until one of that tenant's jobs finishes and frees capacity. Tenants with
no matching `FabricQuota` (or no `gpuQuota.maxGPUs`) are unrestricted. This
only serializes jobs *within* a tenant — it never blocks or reorders other
tenants' jobs. Internal YAML configs can set the same limits directly via
`cluster.tenant_quotas: { <tenant>: <maxGPUs> }`.

**Known divergence from real Forge (as of this writing):** live-cluster
testing found that Forge's actual quota enforcement is materially weaker
than what Zyvor Janus simulates. `FabricQuota` is reconciled by a separate
`quota-operator`, asynchronously (event-driven plus a 1-minute poll)
against jobs that already exist — it is never consulted by the
ai-operator's scheduler or an admission webhook before a job is placed.
Even when it does detect a violation and marks a job `Rejected`, that
phase isn't recognized by the ai-operator's own reconcile loop, and the
phases that create the job's PVC/Service/StatefulSet are unconditional —
not gated on quota status at all. In a same-namespace test (two jobs
requesting 1 GPU each against a `maxGPUs: 1` quota), **both jobs reached
`Running` simultaneously** on a real Forge deployment, while Zyvor Janus
correctly serialized them (doubling the makespan, as designed). If
you're using Zyvor Janus to predict what a live Forge cluster will
actually do, don't assume quota caps hold — as of this writing, Forge
will let a tenant exceed its quota if the raw GPU capacity is there. See
`configs/profiles/quota-test-a.yaml`/`quota-test-b.yaml` for the test
scenario's calibrated profiles.

## Priority scheduling (M6)

`spec.priority` (0–100, mapped to `Job.priority`) can drive placement order
instead of arrival time: pass `--scheduler priority` to `zyvor-janus run
--forge-bundle` or `zyvor-janus replay`, or set `scheduler.type: priority` in
an internal YAML config. The priority scheduler orders the waiting queue by
highest priority first, breaking ties by earliest arrival — but it does not
preempt jobs already running, so a low-priority job that started before a
high-priority one arrived keeps running to completion.

## Preemption (M6)

`--scheduler preemptive` / `scheduler.type: preemptive` extends priority
scheduling with eviction: a waiting job that doesn't currently fit may
preempt running jobs with strictly lower priority (lowest priority evicted
first), stopping as soon as enough capacity is freed — or leaving the
cluster untouched if evicting every eligible candidate still isn't enough.

Evicted jobs resume later with whatever runtime they had left — by default
with no restart penalty, as if perfectly checkpointed — and re-enter the
queue at their *original* priority and arrival time. (Optional
`preemption_restart_penalty_secs` on the engine can delay resume when
non-zero.) A job that's been preempted 3 times becomes exempt from further
eviction, so a persistently low-priority job still eventually finishes
rather than being evicted forever. Only whole-GPU jobs trigger eviction; a
MIG job that doesn't fit is left waiting. `SimulationMetrics.preemptions`
counts how many evictions happened (`zyvor-janus run` prints a
`preemptions:` line when nonzero).

## Other schedulers (M6)

| Flag / `scheduler.type` | Behavior |
|-------------------------|----------|
| `fifo` | Arrival order |
| `priority` | Highest priority first; no preemption |
| `preemptive` | Priority + eviction of lower-priority runners |
| `forge` | Alias for preemptive priority (Forge-like default) |
| `bestfit` | Tightest-node GPU packing among feasible placements |

```bash
cargo run -p zyvor-janus-cli -- run \
  --forge-bundle tests/fixtures/forge \
  --scheduler bestfit
```

## Gang scheduling (M6)

Gang jobs (`forge.ai/gang-schedule: "true"`, `forge.ai/gang-size: N`) require
`gpu_count / N` GPUs on each of `N` distinct nodes before starting
(all-or-nothing). If the gang cannot be placed within
`forge.ai/gang-timeout` (e.g. `"10m"`, `"600s"`), the job fails with
`JobState::Failed` and appears in `SimulationMetrics.jobs_failed`.

Internal YAML workloads set the same fields via `gang_enabled`,
`gang_size_nodes`, and `gang_timeout_secs`:

```bash
cargo run -p zyvor-janus-cli -- run --config configs/clusters/gang_timeout_m6.yaml
```

## Calibrated profiles

Runtime and memory are **not** in Forge CRDs. They come from [`configs/profiles/`](../configs/profiles/):

```yaml
# configs/profiles/gpt-13b.yaml  (v1 — training-style scalar runtime)
model: gpt-13b
profiles:
  H100:
    runtime_seconds: 604800
    gpu_memory_gb: 80
```

### Profile v2 (inference)

Inference jobs can use analytical TTFT/TPS fields (P1). Example
[`configs/profiles/llama-70b.yaml`](../configs/profiles/llama-70b.yaml):

```yaml
model: llama-70b
profiles:
  H100:
    runtime_seconds: 3480
    gpu_memory_gb: 80
    prefill_ms_per_token: 0.12
    decode_tps: 95.0
    max_batch: 32
```

Internal YAML jobs may set `model_id`, `input_tokens`, `output_tokens`,
`batch_size`, and related fields; see `configs/workloads/inference_llama.yaml`
and `configs/clusters/inference_llama.yaml`.

Missing profiles cause an explicit error (no silent default runtime).

## GPU type registry

[`configs/gpu_type_registry.yaml`](../configs/gpu_type_registry.yaml) maps Forge `gpuType` to Zyvor Janus hardware profiles:

```yaml
mappings:
  H100: H100_80GB
  H200: H200_141GB
  B200: B200_192GB
  MI300X: MI300X_192GB
  Gaudi3: GAUDI3_128GB
  M4Max: M4_MAX_128GB
```

The full map is the YAML file. `configs/clusters/gpu_kinds.yaml` places one job on every shipped profile (NVIDIA, AMD, Intel Gaudi, Apple Silicon) and leaves an unknown `gpu_type` queued.

## FabricGpuNode → cluster

Export `FabricGpuNode` CRDs into `forge-export/cluster/`. Zyvor Janus builds nodes and GPUs from `spec.nodeName`, `spec.gpuCount`, `spec.gpuType`, `spec.memoryGB`.

## MIG simulation (M4)

Forge `FabricAIJob.spec.mig` requests fractional GPU slices instead of whole devices:

```yaml
spec:
  model: gpt-13b
  gpuType: H100
  mig:
    profile: 1g.10gb
    count: 2
```

MIG profiles live in [`configs/mig/`](../configs/mig/) keyed by hardware profile (`h100_80gb.yaml`). When a job needs slices that are not yet partitioned, Zyvor Janus simulates a **reconfiguration delay** (`reconfig_seconds`, default 30s) before the job starts.

Whole-GPU jobs cannot be placed on GPUs that are actively partitioned into MIG slices.

Run the MIG example:

```bash
cargo run -p zyvor-janus-cli -- run --config configs/clusters/mig_single.yaml
```

## Scheduler event trace (M3)

Export production scheduling decisions as JSONL and replay them against a simulated policy (FIFO today) to answer: *would my scheduler choose differently?*

### Trace format (JSONL)

One JSON object per line. Supported events:

| Event | Required fields | Purpose |
|-------|-----------------|--------|
| `JobSubmitted` | `timestamp`, `job`, `gpu_count`, `runtime` | Job arrival + resource needs |
| `JobScheduled` | `timestamp`, `job`, `node`, `gpus` | Oracle placement from production Forge |
| `JobCompleted` | `timestamp`, `job` | Optional completion marker (reserved) |

Example:

```json
{"timestamp": 15, "event": "JobSubmitted", "job": "llama70b", "gpu_count": 8, "runtime": 3600}
{"timestamp": 18, "event": "JobScheduled", "job": "llama70b", "node": "node-4", "gpus": [0, 1, 2, 3]}
```

`gpus` accepts full GPU ids (`"gpu-0"`) or numeric indices relative to `node` (`0` → `node-4-gpu-0` when using ForgeGpuNode naming).

### Export sources (Forge)

Derive traces from:

- `FabricAIJob.status.nodesAllocated` and allocated GPU ids
- Forge scheduler plugin events (`ForgeGang`, queue/bind decisions)
- Intelligence-engine scheduling logs (future exporter)

### Replay CLI

```bash
cargo run -p zyvor-janus-cli -- replay \
  --trace tests/fixtures/traces/fifo_match.jsonl \
  --config configs/clusters/single_gpu.yaml
```

Or with a Forge cluster export:

```bash
cargo run -p zyvor-janus-cli -- replay \
  --trace traces/production.jsonl \
  --forge-bundle forge-export
```

Output: `outputs/trace_diff.json` with per-job oracle vs simulated placement diffs and simulation metrics.

## M2 success criteria

1. Ingest Forge export directory without Kubernetes or NVIDIA hardware
2. Run FIFO simulation to completion
3. Emit makespan, wait time, utilization, jobs completed
4. Golden tests: gang job `gpu_count == 32`, tenant from FabricQuota
5. Unknown model without profile → explicit error

## Validation without hardware

1. **Pure simulation** — compare schedulers on Forge export bundles
2. **Trace replay (M3)** — replay exported scheduler events and diff placements
3. **Cloud calibration** — replace estimated runtimes with measured data
4. **Production compare** — match metrics against live Forge cluster

## M3 success criteria

1. Load JSONL scheduler traces with `JobSubmitted` + `JobScheduled` events
2. Replay jobs through FIFO simulation on a configured cluster
3. Compare oracle placements vs simulated placements (GPUs + schedule time)
4. Emit diff report JSON via `zyvor-janus replay`
5. Golden fixture in `tests/fixtures/traces/`

## Python API

```python
from pathlib import Path
from zyvor_janus.adapters import ForgeBundleAdapter, TraceAdapter

bundle = ForgeBundleAdapter(Path("configs/profiles")).from_directory("tests/fixtures/forge")
assert bundle.jobs[0]["gpu_count"] == 32  # gang job

trace = TraceAdapter().from_file("tests/fixtures/traces/fifo_match.jsonl")
oracle = TraceAdapter().oracle_schedules(trace.events)
```

## Security notes (M2)

- Pin `apiVersion: forge.ai/v1`
- Validate document kind before parsing
- Do not silently default missing runtime profiles
