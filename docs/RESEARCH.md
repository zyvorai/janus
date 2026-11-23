# Research notes: GPU-cluster scheduling simulation and workload modelling

Method: pages marked **[R]** were fetched and read while writing this file
(2026-11). Anything marked **[U]** is from memory and was not verified
against a fetched page; treat it as a lead, not a citation.

## What the field does

| Source | What it does | Janus today |
|---|---|---|
| Microsoft Helios traces [R] <https://github.com/S-Lab-System-Group/HeliosData> | Four Slurm clusters (Earth, Saturn, Uranus, Venus); `cluster_log.csv` columns `job_id,user,vc,gpu_num,cpu_num,node_num,state,submit_time,start_time,end_time,duration,queue`; states COMPLETED, CANCELLED, FAILED, TIMEOUT, NODE_FAIL. Page does not state the timestamp format. | **Imported** (`import-trace`, see [trace_import.md](trace_import.md)). |
| Microsoft Philly trace [R] <https://github.com/msr-fiddle/philly-traces> | JSON `cluster_job_log`: `status` (Pass/Killed/Failed), `vc`, `jobid`, `submitted_time`, `user`, `attempts[]` with `start_time`, `end_time`, `detail[]` (`ip`, `gpus`). Attempts may lack times. CC-BY-4.0. | Not imported yet (idea 4). Per-attempt data would also give real retry behaviour to calibrate failure injection. |
| Alibaba PAI GPU trace 2020 [R] <https://github.com/alibaba/clusterdata/tree/master/cluster-trace-gpu-v2020> | Tables `pai_job_table`, `pai_task_table` (`plan_gpu` in percent of a GPU, `gpu_type`), `pai_instance_table`, sensor and machine tables. ~6,500 GPUs; times offset-desensitised. Fractional GPU requests (`plan_gpu` 50 = half a GPU). | Not imported (idea 5). Needs a job/task join and fractional-GPU modelling. |
| Google Borg / Omega / cluster-data traces [U] | Job/task event tables with priorities, resource requests and kill/evict/fail/finish events. | Not modelled; CPU/memory centric. |
| Themis (NSDI'20) [R abstract only] <https://arxiv.org/abs/1907.01484> | Two-level auction scheduler for finish-time fairness for gang-scheduled, placement-sensitive ML jobs. The rho formula was NOT on the fetched page [U]: from memory, rho = finish time under sharing / finish time with a 1/N cluster share. | Janus reports bounded slowdown and Jain indices, not rho. |
| Gavel (OSDI'20) [R abstract only] <https://arxiv.org/abs/2008.09213> | Heterogeneity-aware policies expressed as optimisation problems with round-based allocation; metrics: average JCT, makespan, sustainable load. | Heterogeneous GPU kinds exist in configs, but no per-model throughput matrix per GPU type. |
| Tiresias (NSDI'19) [R abstract only] <https://www.usenix.org/conference/nsdi19/presentation/gu> | Duration-agnostic scheduling (2D-LAS, 2D-Gittins) and relaxed consolidation; trace-driven simulation, up to 5.5x lower average JCT than a YARN baseline. | No LAS/Gittins policy. Janus policies are fifo, priority, preemptive, bestfit. |
| kube-scheduler-simulator [R] <https://github.com/kubernetes-sigs/kube-scheduler-simulator> | SIG tool: real kube-scheduler on a KWOK fake cluster, per-plugin filter/score results annotated on pods, snapshots and record/replay of cluster changes. | Different goal: it debugs the *real* scheduler's plugins; Janus models GPU policy at DES level. |
| Kueue / Volcano [U] | Kueue: ClusterQueue quotas, cohorts, borrowing, preemption policies. Volcano: gang (PodGroup), queues, DRF/proportion plugins. | Quotas and gang exist; no borrowing/cohorts, no DRF. |
| SimGrid / CloudSim [U] | General discrete-event simulators for distributed systems / clouds; not GPU-topology aware. | Janus is GPU- and MIG-specific. |
| Jain's fairness index [U] | J = (sum x)^2 / (n * sum x^2), range 1/n..1. | **Implemented** (`jain_index`). |
| Bounded slowdown (Feitelson) [U] | max((wait + run) / max(run, tau), 1) with tau ~ 10 s to stop tiny jobs dominating. | **Implemented**, tau = 10 s. |
| MIG partition rules [U] | A100/H100 MIG instances are placed on fixed slice boundaries; not every combination of profiles fits. | Already modelled in `configs/mig` and `zyvor-janus-model::mig`; not re-audited here. |
| NVLink/NCCL cost models [U] | Ring/tree all-reduce time depends on slowest-link bandwidth; cross-domain placement inflates step time. | Janus has a synthetic bandwidth-penalty model (`TopologyGraph`), documented as synthetic. |

## What Janus lacked before this change

1. Only means: makespan, mean wait, utilisation. No wait percentiles, slowdown, fairness, or utilisation/fragmentation over time, so two policies with equal means but very different tails looked identical.
2. No way to load a public trace; only hand-written YAML and the Zynera CRD bundle.
3. No failures at all: every job that starts finishes. Preemption existed, crash-and-restart did not.
4. Policy comparison meant two manual runs and eyeballing; no notion of run-to-run variance or significance.
5. Hostile or sloppy workloads were accepted: NaN/negative times corrupt event ordering, duplicate ids silently overwrote running jobs, and arrival handling was O(n^2) in the job count.

## Ranked ideas

| # | Idea | Rationale | Status |
|---|---|---|---|
| 1 | Scheduler-quality metrics (percentiles, bounded slowdown, Jain, utilisation + fragmentation timelines) | Cheap, post-hoc, deterministic, and it is what every paper above argues policies should be judged on (Themis fairness, Gavel/Tiresias JCT). | **Built** |
| 2 | Public-trace importer (Helios) | Real arrival/size/duration distributions instead of synthetic ones; documented columns, no GPU-type guessing needed. | **Built** |
| 3 | Seeded failure injection | Helios/Philly both record FAILED/Killed/NODE_FAIL jobs, so failures are a first-order effect on goodput; Janus had none. | **Built** (job-level crashes only, see limits) |
| 4 | Baseline-vs-candidate comparison with paired CIs | Only meaningful once a source of variance (3) exists; avoids claiming wins from one run. | **Built** |
| 5 | Philly importer using `attempts[]` as observed retries | Gives calibration data for idea 3 (per-job restart counts, lost time). | Not built: nested JSON, attempts may lack times; needs its own validation work. |
| 6 | Alibaba PAI importer with fractional GPUs | Large and recent, but needs a job/task join and sub-GPU sharing that Janus models only via MIG. | Not built |
| 7 | Finish-time-fairness rho with a proper fair-share baseline | Closest to Themis; needs a defensible 1/N share definition for gang jobs. Bounded slowdown is the honest stand-in until then. | Not built |
| 8 | Duration-agnostic policies (2D-LAS), Kueue-style borrowing, DRF | New policies, not simulator infrastructure. | Not built |
| 9 | GPU/node hardware failure (device goes out of service) | More realistic than job crashes, but needs resource-manager changes and MTTR data Janus does not have. | Rejected for now, see below |

## Why some things were rejected

- **Hardware failure model**: marking GPUs unavailable touches the resource manager, MIG slicing and topology; without real MTBF/MTTR data it would add fidelity-looking detail that is not calibrated. Job-level crashes are documented as exactly that.
- **Finish-time fairness rho**: reporting a number named "rho" without Themis's fair-share definition would overstate fidelity.
- **Random-number crate**: the workspace has no `rand`; the simulator is fully deterministic. Failure draws use a stateless splitmix64 hash of `(seed, job id, run generation)` so results never depend on event order and no external RNG stream can drift between crate versions.
