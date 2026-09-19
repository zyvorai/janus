// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

//! Scheduler event trace replay and oracle comparison.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use zyvor_janus_metrics::SimulationMetrics;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, JobState};
use zyvor_janus_scheduler::Scheduler;
use zyvor_janus_scheduler::{BestFitScheduler, FifoScheduler, ZyneraScheduler, PriorityScheduler};
use zyvor_janus_simulator::SimulationEngine;

use crate::{
    build_cluster, build_resource_manager, load_hardware_profiles, load_simulation_config,
    resolve_path, ConfigError, ConfigResult,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "PascalCase")]
pub enum TraceEvent {
    JobSubmitted {
        timestamp: f64,
        job: String,
        gpu_count: u32,
        runtime: f64,
        #[serde(default)]
        gpu_memory_gb: f64,
        #[serde(default)]
        priority: u32,
        #[serde(default)]
        tenant: Option<String>,
        #[serde(default)]
        gpu_type: Option<String>,
        #[serde(default)]
        network_bw_gbps: Option<f64>,
        #[serde(default)]
        gang_enabled: bool,
        #[serde(default)]
        gang_size_nodes: Option<u32>,
        #[serde(default)]
        gang_timeout_secs: Option<f64>,
        #[serde(default)]
        mig_profile: Option<String>,
        #[serde(default)]
        mig_count: Option<u32>,
    },
    JobScheduled {
        timestamp: f64,
        job: String,
        node: String,
        gpus: Vec<GpuRef>,
        #[serde(default)]
        nodes: Vec<String>,
    },
    JobCompleted {
        timestamp: f64,
        job: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GpuRef {
    Id(String),
    Index(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OraclePlacement {
    pub job_id: String,
    pub timestamp: f64,
    pub node: String,
    pub gpu_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulatedPlacement {
    pub job_id: String,
    pub start_time: f64,
    pub gpu_ids: Vec<String>,
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacementDiff {
    pub job_id: String,
    pub placement_match: bool,
    pub schedule_time_match: bool,
    pub oracle: OraclePlacement,
    pub simulated: SimulatedPlacement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceDiffReport {
    pub scheduler: String,
    pub jobs_total: usize,
    pub oracle_schedules: usize,
    pub matching_placements: usize,
    pub differing_placements: usize,
    pub diffs: Vec<PlacementDiff>,
    pub simulation_metrics: SimulationMetrics,
    /// Job IDs that failed in simulation (e.g. gang timeout) with no successful schedule.
    #[serde(default)]
    pub simulated_failures: Vec<String>,
    /// Oracle-scheduled jobs that never finished successfully in simulation.
    #[serde(default)]
    pub oracle_jobs_not_completed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TraceReplayResult {
    pub report: TraceDiffReport,
}

pub fn load_trace(path: &Path) -> ConfigResult<Vec<TraceEvent>> {
    let content = fs::read_to_string(path)?;
    let mut events = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let event: TraceEvent = serde_json::from_str(trimmed).map_err(|e| {
            ConfigError::Invalid(format!(
                "invalid trace JSON at {}:{}: {e}",
                path.display(),
                line_no + 1
            ))
        })?;
        events.push(event);
    }
    if events.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "trace file {} contains no events",
            path.display()
        )));
    }
    Ok(events)
}

pub fn load_cluster_from_config(config_path: &Path) -> ConfigResult<Cluster> {
    let config = load_simulation_config(config_path)?;
    let base = config_path.parent().unwrap_or_else(|| Path::new("."));
    let hw_dir = resolve_path(base, &config.hardware_profiles_dir);
    let profiles = load_hardware_profiles(&hw_dir)?;
    build_cluster(&config.cluster, &profiles)
}

fn normalize_gpu_refs(node: &str, gpus: &[GpuRef], cluster: &Cluster) -> ConfigResult<Vec<String>> {
    let mut ids = Vec::new();
    for gpu_ref in gpus {
        match gpu_ref {
            GpuRef::Id(id) => {
                if cluster.gpu(id).is_none() {
                    return Err(ConfigError::Invalid(format!(
                        "unknown gpu id '{id}' in trace"
                    )));
                }
                ids.push(id.clone());
            }
            GpuRef::Index(index) => {
                let id = format!("{node}-gpu-{index}");
                if cluster.gpu(&id).is_none() {
                    return Err(ConfigError::Invalid(format!(
                        "unknown gpu index {index} on node '{node}'"
                    )));
                }
                ids.push(id);
            }
        }
    }
    Ok(ids)
}

pub fn validate_job_gang_config(job: &Job) -> ConfigResult<()> {
    if !job.gang_enabled {
        return Ok(());
    }
    let nodes = job.gang_size_nodes.filter(|&n| n > 0).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "job '{}' has gang_enabled but missing or invalid gang_size_nodes",
            job.id
        ))
    })?;
    if !job.gpu_count.is_multiple_of(nodes) {
        return Err(ConfigError::Invalid(format!(
            "job '{}': gpu_count {} is not divisible by gang_size_nodes {}",
            job.id, job.gpu_count, nodes
        )));
    }
    Ok(())
}

pub fn jobs_from_trace(events: &[TraceEvent]) -> ConfigResult<Vec<Job>> {
    let mut jobs = Vec::new();
    for event in events {
        if let TraceEvent::JobSubmitted {
            timestamp,
            job,
            gpu_count,
            runtime,
            gpu_memory_gb,
            priority,
            tenant,
            gpu_type,
            network_bw_gbps,
            gang_enabled,
            gang_size_nodes,
            gang_timeout_secs,
            mig_profile,
            mig_count,
        } = event
        {
            let job_spec = Job {
                id: job.clone(),
                name: job.clone(),
                arrival_time: *timestamp,
                runtime: *runtime,
                gpu_count: *gpu_count,
                gpu_memory_gb: *gpu_memory_gb,
                priority: *priority,
                tenant: tenant.clone(),
                gpu_type: gpu_type.clone(),
                network_bw_gbps: *network_bw_gbps,
                gang_enabled: *gang_enabled,
                gang_size_nodes: *gang_size_nodes,
                gang_timeout_secs: *gang_timeout_secs,
                mig_profile: mig_profile.clone(),
                mig_count: *mig_count,
                ..Job::new(job, job, *timestamp, *runtime, *gpu_count)
            };
            validate_job_gang_config(&job_spec)?;
            jobs.push(job_spec);
        }
    }
    if jobs.is_empty() {
        return Err(ConfigError::Invalid(
            "trace must contain at least one JobSubmitted event".into(),
        ));
    }
    Ok(jobs)
}

pub fn oracle_placements_from_trace(
    events: &[TraceEvent],
    cluster: &Cluster,
) -> ConfigResult<Vec<OraclePlacement>> {
    let mut placements = Vec::new();
    for event in events {
        if let TraceEvent::JobScheduled {
            timestamp,
            job,
            node,
            gpus,
            ..
        } = event
        {
            let gpu_ids = normalize_gpu_refs(node, gpus, cluster)?;
            placements.push(OraclePlacement {
                job_id: job.clone(),
                timestamp: *timestamp,
                node: node.clone(),
                gpu_ids,
            });
        }
    }
    Ok(placements)
}

fn nodes_for_gpus(cluster: &Cluster, gpu_ids: &[String]) -> Vec<String> {
    let mut nodes: Vec<String> = gpu_ids
        .iter()
        .filter_map(|id| cluster.gpu(id).map(|g| g.node_id.clone()))
        .collect();
    nodes.sort();
    nodes.dedup();
    nodes
}

fn simulated_placements(cluster: &Cluster) -> HashMap<String, SimulatedPlacement> {
    let mut map = HashMap::new();
    for job in &cluster.finished_jobs {
        let gpu_ids = job.assigned_gpus.clone();
        map.insert(
            job.id.clone(),
            SimulatedPlacement {
                job_id: job.id.clone(),
                start_time: job.start_time.unwrap_or(0.0),
                nodes: nodes_for_gpus(cluster, &gpu_ids),
                gpu_ids,
            },
        );
    }
    map
}

fn placement_sets_match(a: &[String], b: &[String]) -> bool {
    let set_a: HashSet<_> = a.iter().collect();
    let set_b: HashSet<_> = b.iter().collect();
    set_a == set_b
}

pub fn compare_schedules(
    oracle: &[OraclePlacement],
    simulated: &HashMap<String, SimulatedPlacement>,
    scheduler: &str,
    metrics: SimulationMetrics,
    finished_jobs: &[Job],
) -> TraceDiffReport {
    let mut diffs = Vec::new();
    let mut matching = 0usize;

    let simulated_failures: Vec<String> = finished_jobs
        .iter()
        .filter(|j| j.state == JobState::Failed)
        .map(|j| j.id.clone())
        .collect();

    let completed_ids: HashSet<_> = finished_jobs
        .iter()
        .filter(|j| j.state == JobState::Finished)
        .map(|j| j.id.as_str())
        .collect();

    let oracle_jobs_not_completed: Vec<String> = oracle
        .iter()
        .map(|o| o.job_id.clone())
        .filter(|id| !completed_ids.contains(id.as_str()))
        .collect();

    for entry in oracle {
        let simulated_entry =
            simulated
                .get(&entry.job_id)
                .cloned()
                .unwrap_or_else(|| SimulatedPlacement {
                    job_id: entry.job_id.clone(),
                    start_time: f64::NAN,
                    gpu_ids: Vec::new(),
                    nodes: Vec::new(),
                });

        let placement_match = placement_sets_match(&entry.gpu_ids, &simulated_entry.gpu_ids);
        let schedule_time_match = (entry.timestamp - simulated_entry.start_time).abs() < 1e-6;

        if placement_match && schedule_time_match {
            matching += 1;
        }

        diffs.push(PlacementDiff {
            job_id: entry.job_id.clone(),
            placement_match,
            schedule_time_match,
            oracle: entry.clone(),
            simulated: simulated_entry,
        });
    }

    TraceDiffReport {
        scheduler: scheduler.into(),
        jobs_total: metrics.jobs_total,
        oracle_schedules: oracle.len(),
        matching_placements: matching,
        differing_placements: oracle.len().saturating_sub(matching),
        diffs,
        simulation_metrics: metrics,
        simulated_failures,
        oracle_jobs_not_completed,
    }
}

pub fn run_trace_replay(
    events: &[TraceEvent],
    cluster: Cluster,
    scheduler: &str,
) -> ConfigResult<TraceReplayResult> {
    let jobs = jobs_from_trace(events)?;
    let oracle = oracle_placements_from_trace(events, &cluster)?;
    let jobs_total = jobs.len();

    let (metrics, cluster) = match scheduler {
        "fifo" => run_and_finish(cluster, FifoScheduler, scheduler, jobs, jobs_total),
        "priority" => run_and_finish(cluster, PriorityScheduler, scheduler, jobs, jobs_total),
        "preemptive" | "zynera" => run_and_finish(
            cluster,
            ZyneraScheduler::default(),
            scheduler,
            jobs,
            jobs_total,
        ),
        "bestfit" => run_and_finish(cluster, BestFitScheduler, scheduler, jobs, jobs_total),
        other => {
            return Err(ConfigError::Invalid(format!(
                "unsupported scheduler type '{other}' for trace replay"
            )));
        }
    };

    let simulated = simulated_placements(&cluster);
    let report = compare_schedules(
        &oracle,
        &simulated,
        scheduler,
        metrics,
        &cluster.finished_jobs,
    );

    Ok(TraceReplayResult { report })
}

fn run_and_finish<S: Scheduler>(
    cluster: Cluster,
    scheduler: S,
    scheduler_name: &str,
    jobs: Vec<Job>,
    jobs_total: usize,
) -> (SimulationMetrics, Cluster) {
    let resource_manager = build_resource_manager(None, scheduler_name);
    let mut engine = SimulationEngine::with_resource_manager(cluster, scheduler, resource_manager);
    engine.submit_jobs(jobs);
    engine.run();
    let metrics = SimulationMetrics::from_cluster(&engine.cluster, jobs_total);
    (metrics, engine.cluster)
}

pub fn run_trace_file(
    trace_path: &Path,
    cluster: Cluster,
    scheduler: &str,
) -> ConfigResult<TraceReplayResult> {
    let events = load_trace(trace_path)?;
    run_trace_replay(&events, cluster, scheduler)
}

pub fn trace_diff_to_json(report: &TraceDiffReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into())
}

/// Parse a single trace line (used by Python adapter parity tests).
pub fn parse_trace_line(line: &str) -> ConfigResult<TraceEvent> {
    serde_json::from_str(line.trim()).map_err(|e| ConfigError::Invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use zyvor_janus_model::models::{Gpu, Node};

    fn one_gpu_cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "node-0".into(),
            gpus: vec![Gpu::new("gpu-0", "node-0", "H100_80GB", 80.0)],
        }])
    }

    fn two_gpu_cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "node-0".into(),
            gpus: vec![
                Gpu::new("gpu-0", "node-0", "H100_80GB", 80.0),
                Gpu::new("gpu-1", "node-0", "H100_80GB", 80.0),
            ],
        }])
    }

    #[test]
    fn parses_trace_events() {
        let line =
            r#"{"timestamp":0,"event":"JobSubmitted","job":"j1","gpu_count":1,"runtime":10}"#;
        let event = parse_trace_line(line).unwrap();
        assert!(matches!(
            event,
            TraceEvent::JobSubmitted {
                job,
                gpu_count: 1,
                ..
            } if job == "j1"
        ));
    }

    #[test]
    fn normalizes_gpu_indices() {
        let cluster = Cluster::new(vec![Node {
            id: "node-4".into(),
            gpus: (0..4)
                .map(|i| Gpu::new(format!("node-4-gpu-{i}"), "node-4", "H100_80GB", 80.0))
                .collect(),
        }]);
        let events = vec![TraceEvent::JobScheduled {
            timestamp: 18.0,
            job: "llama70b".into(),
            node: "node-4".into(),
            gpus: vec![
                GpuRef::Index(0),
                GpuRef::Index(1),
                GpuRef::Index(2),
                GpuRef::Index(3),
            ],
            nodes: vec!["node-4".into()],
        }];
        let oracle = oracle_placements_from_trace(&events, &cluster).unwrap();
        assert_eq!(oracle[0].gpu_ids.len(), 4);
        assert_eq!(oracle[0].gpu_ids[0], "node-4-gpu-0");
    }

    fn job_submitted(timestamp: f64, job: &str, gpu_count: u32, runtime: f64) -> TraceEvent {
        TraceEvent::JobSubmitted {
            timestamp,
            job: job.into(),
            gpu_count,
            runtime,
            gpu_memory_gb: 0.0,
            priority: 0,
            tenant: None,
            gpu_type: None,
            network_bw_gbps: None,
            gang_enabled: false,
            gang_size_nodes: None,
            gang_timeout_secs: None,
            mig_profile: None,
            mig_count: None,
        }
    }

    #[test]
    fn fifo_trace_replay_matches_oracle() {
        let events = vec![
            job_submitted(0.0, "j1", 1, 100.0),
            job_submitted(5.0, "j2", 1, 50.0),
            TraceEvent::JobScheduled {
                timestamp: 0.0,
                job: "j1".into(),
                node: "node-0".into(),
                gpus: vec![GpuRef::Id("gpu-0".into())],
                nodes: vec!["node-0".into()],
            },
            TraceEvent::JobScheduled {
                timestamp: 100.0,
                job: "j2".into(),
                node: "node-0".into(),
                gpus: vec![GpuRef::Id("gpu-0".into())],
                nodes: vec!["node-0".into()],
            },
        ];

        let result = run_trace_replay(&events, one_gpu_cluster(), "fifo").unwrap();
        assert_eq!(result.report.matching_placements, 2);
        assert_eq!(result.report.differing_placements, 0);
    }

    #[test]
    fn detects_scheduler_divergence() {
        let events = vec![
            job_submitted(0.0, "j1", 1, 100.0),
            job_submitted(0.0, "j2", 1, 50.0),
            TraceEvent::JobScheduled {
                timestamp: 0.0,
                job: "j2".into(),
                node: "node-0".into(),
                gpus: vec![GpuRef::Id("gpu-1".into())],
                nodes: vec!["node-0".into()],
            },
            TraceEvent::JobScheduled {
                timestamp: 50.0,
                job: "j1".into(),
                node: "node-0".into(),
                gpus: vec![GpuRef::Id("gpu-0".into())],
                nodes: vec!["node-0".into()],
            },
        ];

        let result = run_trace_replay(&events, two_gpu_cluster(), "fifo").unwrap();
        assert_eq!(result.report.differing_placements, 2);
        assert!(result.report.diffs.iter().all(|d| !d.placement_match));
    }

    fn bestfit_two_node_cluster() -> Cluster {
        let mut cluster = Cluster::new(vec![
            Node {
                id: "wide".into(),
                gpus: vec![
                    Gpu::new("w0", "wide", "H100_80GB", 80.0),
                    Gpu::new("w1", "wide", "H100_80GB", 80.0),
                    Gpu::new("w2", "wide", "H100_80GB", 80.0),
                    Gpu::new("w3", "wide", "H100_80GB", 80.0),
                ],
            },
            Node {
                id: "tight".into(),
                gpus: vec![
                    Gpu::new("t0", "tight", "H100_80GB", 80.0),
                    Gpu::new("t1", "tight", "H100_80GB", 80.0),
                ],
            },
        ]);
        cluster.start_job(
            Job::new("block", "block", 0.0, 100.0, 1),
            &["w0".into()],
            0.0,
        );
        cluster
    }

    #[test]
    fn bestfit_trace_replay_packs_tight_node() {
        let events = vec![
            TraceEvent::JobSubmitted {
                timestamp: 0.0,
                job: "block".into(),
                gpu_count: 1,
                runtime: 100.0,
                gpu_memory_gb: 0.0,
                priority: 0,
                tenant: None,
                gpu_type: None,
                network_bw_gbps: None,
                gang_enabled: false,
                gang_size_nodes: None,
                gang_timeout_secs: None,
                mig_profile: None,
                mig_count: None,
            },
            TraceEvent::JobSubmitted {
                timestamp: 0.0,
                job: "pair".into(),
                gpu_count: 2,
                runtime: 10.0,
                gpu_memory_gb: 0.0,
                priority: 0,
                tenant: None,
                gpu_type: None,
                network_bw_gbps: None,
                gang_enabled: false,
                gang_size_nodes: None,
                gang_timeout_secs: None,
                mig_profile: None,
                mig_count: None,
            },
            TraceEvent::JobScheduled {
                timestamp: 0.0,
                job: "pair".into(),
                node: "tight".into(),
                gpus: vec![GpuRef::Id("t0".into()), GpuRef::Id("t1".into())],
                nodes: vec!["tight".into()],
            },
        ];

        let result = run_trace_replay(&events, bestfit_two_node_cluster(), "bestfit").unwrap();
        assert_eq!(result.report.matching_placements, 1);
    }

    #[test]
    fn rejects_invalid_gang_trace_job() {
        let events = vec![TraceEvent::JobSubmitted {
            timestamp: 0.0,
            job: "g1".into(),
            gpu_count: 4,
            runtime: 10.0,
            gpu_memory_gb: 0.0,
            priority: 0,
            tenant: None,
            gpu_type: None,
            network_bw_gbps: None,
            gang_enabled: true,
            gang_size_nodes: None,
            gang_timeout_secs: Some(5.0),
            mig_profile: None,
            mig_count: None,
        }];
        assert!(jobs_from_trace(&events).is_err());
    }

    #[test]
    fn gang_trace_schema_loads_extended_fields() {
        let events = vec![TraceEvent::JobSubmitted {
            timestamp: 0.0,
            job: "g1".into(),
            gpu_count: 4,
            runtime: 10.0,
            gpu_memory_gb: 0.0,
            priority: 5,
            tenant: Some("acme".into()),
            gpu_type: Some("H100_80GB".into()),
            network_bw_gbps: None,
            gang_enabled: true,
            gang_size_nodes: Some(2),
            gang_timeout_secs: Some(30.0),
            mig_profile: None,
            mig_count: None,
        }];
        let jobs = jobs_from_trace(&events).unwrap();
        assert!(jobs[0].gang_enabled);
        assert_eq!(jobs[0].tenant.as_deref(), Some("acme"));
    }

    #[test]
    fn loads_gang_trace_fixture() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/traces/gang_m6.jsonl");
        if !path.exists() {
            return;
        }
        let events = load_trace(&path).unwrap();
        let jobs = jobs_from_trace(&events).unwrap();
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].gang_enabled);
    }

    #[test]
    fn rejects_gang_job_with_indivisible_gpu_count() {
        let job = Job {
            id: "g1".into(),
            name: "g1".into(),
            arrival_time: 0.0,
            runtime: 10.0,
            gpu_count: 5,
            gang_enabled: true,
            gang_size_nodes: Some(2),
            ..Job::new("g1", "g1", 0.0, 10.0, 5)
        };
        assert!(validate_job_gang_config(&job).is_err());
    }

    #[test]
    fn compare_schedules_reports_simulated_failures() {
        let oracle = vec![OraclePlacement {
            job_id: "j1".into(),
            timestamp: 0.0,
            node: "n0".into(),
            gpu_ids: vec!["g0".into()],
        }];
        let simulated = HashMap::new();
        let mut failed = Job::new("j1", "j1", 0.0, 10.0, 1);
        failed.state = JobState::Failed;
        let metrics = SimulationMetrics {
            makespan: 5.0,
            mean_wait_time: 0.0,
            gpu_utilization: 0.0,
            jobs_completed: 0,
            jobs_total: 1,
            queue_max_length: 0,
            mean_cumulative_wait_time: 0.0,
            jobs_unschedulable: 0,
            mig_reconfigs: 0,
            preemptions: 0,
            topology_penalties: 0,
            topology_runtime_inflation: 0.0,
            jobs_failed: 1,
            ..Default::default()
        };
        let report = compare_schedules(&oracle, &simulated, "fifo", metrics, &[failed]);
        assert_eq!(report.simulated_failures, vec!["j1".to_string()]);
        assert_eq!(report.oracle_jobs_not_completed, vec!["j1".to_string()]);
    }

    #[test]
    fn loads_fixture_trace_file() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/traces/fifo_match.jsonl");
        if !path.exists() {
            return;
        }
        let cluster = load_cluster_from_config(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../configs/clusters/single_gpu.yaml"),
        )
        .unwrap();
        let result = run_trace_file(&path, cluster, "fifo").unwrap();
        assert_eq!(result.report.differing_placements, 0);
    }
}
