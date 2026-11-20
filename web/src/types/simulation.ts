// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

export interface SimulationMetrics {
  makespan: number;
  mean_wait_time: number;
  mean_cumulative_wait_time?: number;
  gpu_utilization: number;
  jobs_completed: number;
  jobs_total: number;
  queue_max_length: number;
  jobs_unschedulable?: number;
  mig_reconfigs: number;
  preemptions: number;
  topology_penalties: number;
  topology_runtime_inflation: number;
  jobs_failed: number;
  inference_jobs?: number;
  ttft_p50?: number;
  ttft_p99?: number;
  itl_p50?: number;
  tps_mean?: number;
  goodput?: number;
  queue_delay_p99?: number;
}

export interface SchedulerBenchmarkReport {
  scheduler: string;
  config_hash: string;
  metrics: SimulationMetrics;
  jain_fairness: number;
  fragmentation: number;
  cost_usd: number;
  score_vector: Record<string, number>;
}

export function hasInferenceMetrics(metrics: SimulationMetrics | null | undefined): boolean {
  return Boolean(metrics && (metrics.inference_jobs ?? 0) > 0);
}

export interface JobRunSegment {
  gpu_ids: string[];
  /** Nodes for gpu_ids (same order); used for machine→machine migrate labels. */
  node_ids?: string[];
  start: number;
  end: number;
}

export interface JobTimelineRecord {
  job_id: string;
  name: string;
  arrival_time: number;
  start_time: number | null;
  finish_time: number | null;
  runtime: number;
  gpu_count: number;
  assigned_gpus: string[];
  /** Closed run segments across preemptions (empty when never started). */
  segments?: JobRunSegment[];
  priority: number;
  tenant: string | null;
  state: string;
}

export interface JobsTimeline {
  makespan: number;
  gpu_count: number;
  jobs: JobTimelineRecord[];
}

export interface SchedulerDecision {
  time: number;
  kind: string;
  job_id: string | null;
  job_name: string | null;
  gpu_ids: string[];
  message: string;
}

export interface GpuSnapshot {
  id: string;
  node_id: string;
  busy: boolean;
  utilization: number;
  job_id: string | null;
  job_name: string | null;
  nvlink_group: number | null;
}

export interface ClusterSnapshot {
  clock: number;
  free_gpus: number;
  waiting: number;
  running: number;
  finished: number;
  node_count: number;
  gpu_count: number;
  queue_jobs: Array<{ id: string; name: string; priority: number; tenant: string | null; gpu_count: number; state: string }>;
  nodes: Array<{ id: string; gpus: GpuSnapshot[] }>;
}

export interface RunSummary {
  id: string;
  config: string;
  scheduler: string | null;
  shadow_scheduler?: string | null;
  status: string;
  created_at: string;
  finished_at: string | null;
}

export interface RunDetail extends RunSummary {
  error: string | null;
  metrics: SimulationMetrics | null;
  timeline: JobsTimeline | null;
  decision_count: number;
  benchmark?: SchedulerBenchmarkReport | null;
  shadow_metrics?: SimulationMetrics | null;
  shadow_decision_count?: number;
  shadow_benchmark?: SchedulerBenchmarkReport | null;
}

/** One event off the live `/ws/runs/{id}` shadow stream (`{"type": "step", ...}`). */
export interface ShadowStepEvent {
  side: "primary" | "shadow";
  time: number;
  kind: string;
  decisions: SchedulerDecision[];
}

export interface ConfigEntry {
  id: string;
  path: string;
  /** Best-effort twin correlation hint resolved server-side; null for non-inference configs. */
  gpu_type: string | null;
  model: string | null;
}

export interface CompareResult {
  config: string;
  status: string;
  metrics: SimulationMetrics | null;
  run_id: string;
}

/** One calibrated GPU/model pair from `GET /api/twins` (`outputs/twins/twins.sqlite`). */
export interface TwinEntry {
  gpu_type: string;
  model: string;
  ttft_ms: number;
  tps: number;
  throughput: number;
  measured_at: string;
  aiperf_run_id: string | null;
}
