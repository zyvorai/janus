// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

mod zynera_bundle;
mod mig;
mod serving_trace;
mod trace;

pub use zynera_bundle::{
    load_zynera_bundle, load_gpu_type_registry, load_model_profiles, parse_fabric_ai_job,
    run_zynera_bundle, run_zynera_bundle_report, FederationRunMeta, ZyneraBundle, GpuTypeRegistry,
    ModelProfile,
};
pub use mig::{
    load_mig_registry, load_mig_registry_for_hardware, resolve_mig_registry_for_cluster,
};
pub use serving_trace::{
    export_serving_trace_from_cluster, jobs_from_serving_trace, load_serving_trace,
    validate_serving_trace, write_serving_trace_jsonl, ServingTraceFile, ServingTraceRecord,
    SERVING_TRACE_VERSION,
};
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
pub use trace::{
    compare_schedules, jobs_from_trace, load_cluster_from_config, load_trace,
    oracle_placements_from_trace, parse_trace_line, run_trace_file, run_trace_replay,
    trace_diff_to_json, validate_job_gang_config, GpuRef, OraclePlacement, PlacementDiff,
    SimulatedPlacement, TraceDiffReport, TraceEvent, TraceReplayResult,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zyvor_janus_metrics::{CostModel, JobsTimeline, SchedulerBenchmarkReport, SimulationMetrics};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Gpu, Job, Node};
use zyvor_janus_scheduler::resource::{GpuSelectionPolicy, ResourceManager};
use zyvor_janus_scheduler::Scheduler;
use zyvor_janus_scheduler::{BestFitScheduler, FifoScheduler, ZyneraScheduler, PriorityScheduler};
use zyvor_janus_simulator::inference::{estimate_inference, InferenceProfile, InferenceRequest};
use zyvor_janus_simulator::rl::RlSession;
use zyvor_janus_simulator::snapshot::ClusterSnapshot;
use zyvor_janus_simulator::SimulationEngine;
use zyvor_janus_topology::TopologyGraph;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationReport {
    pub metrics: SimulationMetrics,
    pub timeline: JobsTimeline,
    #[serde(default)]
    pub decisions: Vec<zyvor_janus_core::SchedulerDecision>,
    #[serde(default)]
    pub snapshots: Vec<zyvor_janus_simulator::ClusterSnapshot>,
    #[serde(default)]
    pub scheduler: String,
    #[serde(default)]
    pub config_hash: String,
    #[serde(default)]
    pub benchmark: Option<SchedulerBenchmarkReport>,
    /// Federation metadata from the bundle's `federation/` dir, if a
    /// `FabricFederatedTrainingRun` was present (Zynera-bundle path only).
    #[serde(default)]
    pub federation: Option<zynera_bundle::FederationRunMeta>,
    /// Per-job serving trace derived from the finished `Cluster`, captured
    /// here because `Cluster` does not otherwise survive past this function.
    #[serde(default = "default_serving_trace")]
    pub serving_trace: ServingTraceFile,
}

fn default_serving_trace() -> ServingTraceFile {
    ServingTraceFile::new(Vec::new())
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("config error: {0}")]
    Invalid(String),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Clone, Deserialize)]
pub struct HardwareProfile {
    pub name: String,
    pub memory_gb: f64,
    #[serde(default)]
    pub sm: Option<u32>,
    #[serde(default)]
    pub mig_profiles: Vec<String>,
    #[serde(default)]
    pub nvlink_bw_gbs: Option<f64>,
    #[serde(default)]
    pub pcie_bw_gbs: Option<f64>,
    #[serde(default)]
    pub mig: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GpuSpec {
    pub id: String,
    pub profile: String,
    #[serde(default)]
    pub nvlink_group: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub gpus: Vec<GpuSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterConfig {
    pub nodes: Vec<NodeSpec>,
    #[serde(default)]
    pub tenant_quotas: HashMap<String, u32>,
    /// Synthetic NVLink layout: `nvlink_pairs` (default), `full_mesh`, or `pcie_only`.
    #[serde(default = "default_topology_template")]
    pub topology_template: String,
}

fn default_topology_template() -> String {
    "nvlink_pairs".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_scheduler")]
    pub r#type: String,
}

fn default_scheduler() -> String {
    "fifo".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkloadRef {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    pub cluster: ClusterConfig,
    pub scheduler: SchedulerConfig,
    pub workload: WorkloadRef,
    #[serde(default = "default_hardware_dir")]
    pub hardware_profiles_dir: String,
    #[serde(default = "default_mig_profiles_dir")]
    pub mig_profiles_dir: String,
    #[serde(default = "default_profiles_dir")]
    pub profiles_dir: String,
}

fn default_profiles_dir() -> String {
    "../profiles".into()
}

fn default_hardware_dir() -> String {
    "configs/hardware".into()
}

fn default_mig_profiles_dir() -> String {
    "../mig".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkloadJobSpec {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub arrival_time: f64,
    pub runtime: f64,
    pub gpu_count: u32,
    #[serde(default)]
    pub gpu_memory_gb: f64,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub network_bw_gbps: Option<f64>,
    #[serde(default)]
    pub gpu_type: Option<String>,
    #[serde(default)]
    pub mig_profile: Option<String>,
    #[serde(default)]
    pub mig_count: Option<u32>,
    #[serde(default)]
    pub gang_enabled: bool,
    #[serde(default)]
    pub gang_size_nodes: Option<u32>,
    #[serde(default)]
    pub gang_timeout_secs: Option<f64>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens: Option<u32>,
    #[serde(default)]
    pub batch_size: Option<u32>,
    #[serde(default)]
    pub concurrency: Option<u32>,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkloadConfig {
    pub jobs: Vec<WorkloadJobSpec>,
}

pub fn load_hardware_profiles(dir: &Path) -> ConfigResult<HashMap<String, HardwareProfile>> {
    let mut profiles = HashMap::new();
    if !dir.exists() {
        return Ok(profiles);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let profile: HardwareProfile = serde_yaml::from_str(&content)?;
        profiles.insert(profile.name.clone(), profile);
    }
    Ok(profiles)
}

pub fn load_simulation_config(path: &Path) -> ConfigResult<SimulationConfig> {
    let content = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

pub fn load_workload(path: &Path) -> ConfigResult<Vec<Job>> {
    load_workload_with_profiles(path, &HashMap::new(), &[])
}

pub fn load_workload_with_profiles(
    path: &Path,
    model_profiles: &HashMap<String, zynera_bundle::ModelProfile>,
    default_gpu_types: &[String],
) -> ConfigResult<Vec<Job>> {
    let content = fs::read_to_string(path)?;
    let workload: WorkloadConfig = serde_yaml::from_str(&content)?;
    let default_gpu = default_gpu_types
        .first()
        .cloned()
        .unwrap_or_else(|| "H100".into());
    let mut jobs = Vec::new();
    for j in workload.jobs {
        if j.input_tokens == Some(0) && j.output_tokens == Some(0) {
            return Err(ConfigError::Invalid(format!(
                "job '{}': inference jobs need positive token counts",
                j.id
            )));
        }
        let name = j.name.unwrap_or_else(|| "job".into());
        let id = j.request_id.clone().unwrap_or_else(|| j.id.clone());
        let mut job = Job {
            id: id.clone(),
            name: name.clone(),
            arrival_time: j.arrival_time,
            runtime: j.runtime,
            gpu_count: j.gpu_count,
            gpu_memory_gb: j.gpu_memory_gb,
            priority: j.priority,
            tenant: j.tenant,
            gpu_type: j.gpu_type.clone(),
            network_bw_gbps: j.network_bw_gbps,
            mig_profile: j.mig_profile,
            mig_count: j.mig_count,
            gang_enabled: j.gang_enabled,
            gang_size_nodes: j.gang_size_nodes,
            gang_timeout_secs: j.gang_timeout_secs,
            model_id: j.model_id.clone(),
            input_tokens: j.input_tokens,
            output_tokens: j.output_tokens,
            batch_size: j.batch_size,
            concurrency: j.concurrency,
            ..Job::new(id, name, j.arrival_time, j.runtime, j.gpu_count)
        };
        apply_inference_runtime(&mut job, model_profiles, &default_gpu)?;
        validate_job_gang_config(&job)?;
        jobs.push(job);
    }
    Ok(jobs)
}

fn apply_inference_runtime(
    job: &mut Job,
    model_profiles: &HashMap<String, zynera_bundle::ModelProfile>,
    default_gpu: &str,
) -> ConfigResult<()> {
    let Some(model_id) = job.model_id.clone() else {
        return Ok(());
    };
    let Some(input_tokens) = job.input_tokens else {
        return Ok(());
    };
    let Some(output_tokens) = job.output_tokens else {
        return Ok(());
    };
    let gpu_type = job
        .gpu_type
        .clone()
        .unwrap_or_else(|| default_gpu.to_string());
    let profile = resolve_inference_profile(model_profiles, &model_id, &gpu_type)?;
    let req = InferenceRequest {
        input_tokens,
        output_tokens,
        batch_size: job.batch_size.unwrap_or(1),
        concurrency: job.concurrency.unwrap_or(1),
    };
    let estimate = estimate_inference(&profile, req);
    job.runtime = estimate.runtime_secs.max(0.001);
    job.ttft_secs = Some(estimate.ttft_secs);
    job.tps = Some(estimate.tps);
    job.itl_secs = Some(estimate.itl_secs);
    Ok(())
}

fn resolve_inference_profile(
    model_profiles: &HashMap<String, zynera_bundle::ModelProfile>,
    model_id: &str,
    gpu_type: &str,
) -> ConfigResult<InferenceProfile> {
    let profile = model_profiles.get(model_id).ok_or_else(|| {
        ConfigError::Invalid(format!("unknown inference model profile '{model_id}'"))
    })?;
    let entry = profile
        .profiles
        .get(gpu_type)
        .or_else(|| {
            gpu_type
                .split('_')
                .next()
                .and_then(|prefix| profile.profiles.get(prefix))
        })
        .ok_or_else(|| {
            ConfigError::Invalid(format!(
                "model '{model_id}' has no calibrated profile for gpu '{gpu_type}'"
            ))
        })?;
    Ok(InferenceProfile {
        model: model_id.to_string(),
        gpu_type: gpu_type.to_string(),
        prefill_ms_per_token: entry.prefill_ms_per_token(),
        decode_tps: entry.decode_tps(),
        max_batch: entry.max_batch(),
    })
}

pub fn build_cluster(
    cluster_cfg: &ClusterConfig,
    profiles: &HashMap<String, HardwareProfile>,
) -> ConfigResult<Cluster> {
    let mut nodes = Vec::new();
    for node_spec in &cluster_cfg.nodes {
        let mut gpus = Vec::new();
        for gpu_spec in &node_spec.gpus {
            let profile = profiles.get(&gpu_spec.profile).ok_or_else(|| {
                ConfigError::Invalid(format!("unknown hardware profile '{}'", gpu_spec.profile))
            })?;
            let mut gpu = Gpu::new(
                gpu_spec.id.clone(),
                node_spec.id.clone(),
                gpu_spec.profile.clone(),
                profile.memory_gb,
            );
            gpu.nvlink_group = gpu_spec.nvlink_group.or_else(|| {
                apply_topology_template(&cluster_cfg.topology_template, gpu_spec, node_spec)
            });
            gpu.mig_capable = profile.mig;
            gpus.push(gpu);
        }
        nodes.push(Node {
            id: node_spec.id.clone(),
            gpus,
        });
    }
    let mut nvlink_bw: f64 = 900.0;
    let mut pcie_bw: f64 = 64.0;
    for node_spec in &cluster_cfg.nodes {
        for gpu_spec in &node_spec.gpus {
            if let Some(profile) = profiles.get(&gpu_spec.profile) {
                if let Some(v) = profile.nvlink_bw_gbs {
                    nvlink_bw = nvlink_bw.max(v);
                }
                if let Some(v) = profile.pcie_bw_gbs {
                    pcie_bw = pcie_bw.max(v);
                }
            }
        }
    }
    let mut cluster = Cluster::new(nodes);
    cluster.topology = TopologyGraph::from_profile_bandwidths(nvlink_bw, pcie_bw);
    cluster.tenant_quotas = cluster_cfg.tenant_quotas.clone();
    Ok(cluster)
}

fn apply_topology_template(
    template: &str,
    gpu_spec: &GpuSpec,
    node_spec: &NodeSpec,
) -> Option<u32> {
    let gpu_index = node_spec.gpus.iter().position(|g| g.id == gpu_spec.id)?;
    match template {
        "full_mesh" => Some(0),
        "pcie_only" => Some(gpu_index as u32),
        _ => Some((gpu_index / 2) as u32),
    }
}

/// Best-effort `(gpu_type, model)` hint for a simulation config, used to
/// correlate benchmark runs against calibrated twins in
/// `outputs/twins/twins.sqlite` (see `zyvor-janus-api`'s `/api/twins` and
/// `/api/configs`). Looks at the first workload job with a `model_id`;
/// `gpu_type` comes from that job if set, else the cluster's first GPU
/// hardware profile with any `_<memory>GB` suffix stripped (`H100_80GB` ->
/// `H100`), matching the prefix fallback `resolve_inference_profile` already
/// does. Returns `None` for non-inference configs or on any read/parse error
/// -- callers treat this as decoration, not a hard requirement.
pub fn resolve_config_twin_hint(config_path: &Path) -> Option<(String, String)> {
    let sim_cfg = load_simulation_config(config_path).ok()?;
    let base = config_path.parent()?;
    let workload_path = resolve_path(base, &sim_cfg.workload.path);
    let content = fs::read_to_string(&workload_path).ok()?;
    let workload: WorkloadConfig = serde_yaml::from_str(&content).ok()?;
    let job = workload.jobs.iter().find(|j| j.model_id.is_some())?;
    let model = job.model_id.clone()?;
    let gpu_type = job.gpu_type.clone().or_else(|| {
        sim_cfg
            .cluster
            .nodes
            .first()
            .and_then(|n| n.gpus.first())
            .map(|g| {
                g.profile
                    .split('_')
                    .next()
                    .unwrap_or(&g.profile)
                    .to_string()
            })
    })?;
    Some((gpu_type, model))
}

pub fn resolve_path(base: &Path, relative: &str) -> PathBuf {
    let p = PathBuf::from(relative);
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

pub fn run_simulation(config_path: &Path) -> ConfigResult<SimulationMetrics> {
    Ok(run_simulation_report(config_path)?.metrics)
}

pub fn run_simulation_report(config_path: &Path) -> ConfigResult<SimulationReport> {
    run_simulation_report_with_scheduler(config_path, None)
}

pub fn run_simulation_report_with_scheduler(
    config_path: &Path,
    scheduler_override: Option<&str>,
) -> ConfigResult<SimulationReport> {
    let mut config = load_simulation_config(config_path)?;
    if let Some(sched) = scheduler_override {
        config.scheduler.r#type = sched.to_string();
    }
    let scheduler_name = config.scheduler.r#type.clone();
    let config_hash = hash_config_file(config_path, scheduler_override);

    let base = config_path.parent().unwrap_or_else(|| Path::new("."));

    let hw_dir = resolve_path(base, &config.hardware_profiles_dir);
    let profiles = load_hardware_profiles(&hw_dir)?;

    let cluster = build_cluster(&config.cluster, &profiles)?;
    let hardware_names: Vec<String> = config
        .cluster
        .nodes
        .iter()
        .flat_map(|n| n.gpus.iter().map(|g| g.profile.clone()))
        .collect();

    let profiles_dir = resolve_path(base, &config.profiles_dir);
    let model_profiles = load_model_profiles(&profiles_dir)?;

    let workload_path = resolve_path(base, &config.workload.path);
    let jobs = load_workload_with_profiles(&workload_path, &model_profiles, &hardware_names)?;
    let jobs_total = jobs.len();
    let any_mig_capable = cluster.all_gpus().any(|g| g.mig_capable);
    let any_mig_job = jobs.iter().any(|j| j.is_mig_job());
    let mig_dir = resolve_path(base, &config.mig_profiles_dir);
    let mig_registry = if any_mig_job || any_mig_capable {
        resolve_mig_registry_for_cluster(&mig_dir, &hardware_names, any_mig_capable)?
    } else {
        None
    };
    if any_mig_job && mig_registry.is_none() {
        return Err(ConfigError::Invalid(
            "workload contains MIG jobs but no MIG profile registry is configured".into(),
        ));
    }

    let resource_manager = build_resource_manager(mig_registry, &config.scheduler.r#type);

    let (cluster, _metrics, snapshots) = run_to_completion_with_policy_snapshots(
        cluster,
        &config.scheduler.r#type,
        resource_manager,
        jobs,
        jobs_total,
    )?;

    let cost_path = resolve_path(base, "../analytics/cost.yaml");
    let cost = load_cost_model(&cost_path).unwrap_or_default();

    Ok(shadow_side_report(
        &cluster,
        snapshots,
        scheduler_name,
        config_hash,
        jobs_total,
        &cost,
    ))
}

/// Loads `config_path` once and builds a [`zyvor_janus_simulator::ShadowRun`]
/// stepping a `primary_scheduler_override` (or the config's own scheduler)
/// engine alongside a `shadow_scheduler` engine over the same cluster and
/// job arrivals -- the config-loading counterpart to
/// [`run_simulation_report_with_scheduler`], but returning an un-run driver
/// instead of a completed report so a caller can step and stream it live.
pub fn build_shadow_run(
    config_path: &Path,
    primary_scheduler_override: Option<&str>,
    shadow_scheduler: &str,
) -> ConfigResult<ShadowRunHandle> {
    let mut config = load_simulation_config(config_path)?;
    if let Some(sched) = primary_scheduler_override {
        config.scheduler.r#type = sched.to_string();
    }
    let primary_scheduler = config.scheduler.r#type.clone();
    let config_hash = hash_config_file(config_path, primary_scheduler_override);

    let base = config_path.parent().unwrap_or_else(|| Path::new("."));

    let hw_dir = resolve_path(base, &config.hardware_profiles_dir);
    let profiles = load_hardware_profiles(&hw_dir)?;

    let cluster = build_cluster(&config.cluster, &profiles)?;
    let hardware_names: Vec<String> = config
        .cluster
        .nodes
        .iter()
        .flat_map(|n| n.gpus.iter().map(|g| g.profile.clone()))
        .collect();

    let profiles_dir = resolve_path(base, &config.profiles_dir);
    let model_profiles = load_model_profiles(&profiles_dir)?;

    let workload_path = resolve_path(base, &config.workload.path);
    let jobs = load_workload_with_profiles(&workload_path, &model_profiles, &hardware_names)?;
    let jobs_total = jobs.len();
    let any_mig_capable = cluster.all_gpus().any(|g| g.mig_capable);
    let any_mig_job = jobs.iter().any(|j| j.is_mig_job());
    let mig_dir = resolve_path(base, &config.mig_profiles_dir);
    let mig_registry = if any_mig_job || any_mig_capable {
        resolve_mig_registry_for_cluster(&mig_dir, &hardware_names, any_mig_capable)?
    } else {
        None
    };
    if any_mig_job && mig_registry.is_none() {
        return Err(ConfigError::Invalid(
            "workload contains MIG jobs but no MIG profile registry is configured".into(),
        ));
    }

    let primary_rm = build_resource_manager(mig_registry.clone(), &primary_scheduler);
    let shadow_rm = build_resource_manager(mig_registry, shadow_scheduler);

    let primary_engine = build_steppable_engine(
        cluster.clone(),
        &primary_scheduler,
        primary_rm,
        jobs.clone(),
    )?;
    let shadow_engine = build_steppable_engine(cluster, shadow_scheduler, shadow_rm, jobs)?;

    let cost_path = resolve_path(base, "../analytics/cost.yaml");
    let cost = load_cost_model(&cost_path).unwrap_or_default();

    Ok(ShadowRunHandle {
        run: zyvor_janus_simulator::ShadowRun::new(primary_engine, shadow_engine, jobs_total),
        primary_scheduler,
        shadow_scheduler: shadow_scheduler.to_string(),
        config_hash,
        jobs_total,
        cost,
    })
}

/// Everything [`build_shadow_run`] hands back: the driver itself plus the
/// metadata needed to turn each side's finished [`Cluster`] into a full
/// [`SimulationReport`] via [`shadow_side_report`].
pub struct ShadowRunHandle {
    pub run: zyvor_janus_simulator::ShadowRun,
    pub primary_scheduler: String,
    pub shadow_scheduler: String,
    pub config_hash: String,
    pub jobs_total: usize,
    pub cost: CostModel,
}

fn build_steppable_engine(
    cluster: Cluster,
    scheduler: &str,
    resource_manager: ResourceManager,
    jobs: Vec<Job>,
) -> ConfigResult<Box<dyn zyvor_janus_simulator::SteppableSimulation>> {
    fn seed<S: Scheduler + 'static>(
        cluster: Cluster,
        scheduler: S,
        resource_manager: ResourceManager,
        jobs: Vec<Job>,
    ) -> Box<dyn zyvor_janus_simulator::SteppableSimulation> {
        let mut engine =
            SimulationEngine::with_resource_manager(cluster, scheduler, resource_manager)
                .with_replay_capture();
        engine.submit_jobs(jobs);
        Box::new(engine)
    }
    Ok(match scheduler {
        "fifo" => seed(cluster, FifoScheduler, resource_manager, jobs),
        "priority" => seed(cluster, PriorityScheduler, resource_manager, jobs),
        "preemptive" | "zynera" => seed(cluster, ZyneraScheduler::default(), resource_manager, jobs),
        "bestfit" => seed(cluster, BestFitScheduler, resource_manager, jobs),
        other => {
            return Err(ConfigError::Invalid(format!(
                "unsupported scheduler type '{other}'"
            )));
        }
    })
}

/// Builds a full [`SimulationReport`] for one side of a (possibly
/// in-progress) [`zyvor_janus_simulator::ShadowRun`], reusing the same
/// metrics/timeline/decisions/benchmark/serving-trace construction as
/// [`run_simulation_report_with_scheduler`].
pub fn shadow_side_report(
    cluster: &Cluster,
    snapshots: Vec<ClusterSnapshot>,
    scheduler_name: String,
    config_hash: String,
    jobs_total: usize,
    cost: &CostModel,
) -> SimulationReport {
    let metrics = SimulationMetrics::from_cluster(cluster, jobs_total);
    let benchmark = Some(SchedulerBenchmarkReport::from_simulation(
        &scheduler_name,
        &config_hash,
        cluster,
        metrics.clone(),
        cost,
    ));
    let serving_trace = export_serving_trace_from_cluster(cluster);
    SimulationReport {
        metrics,
        timeline: JobsTimeline::from_cluster(cluster),
        decisions: cluster.decision_log.clone(),
        snapshots,
        scheduler: scheduler_name,
        config_hash,
        benchmark,
        federation: None,
        serving_trace,
    }
}

fn hash_config_file(config_path: &Path, scheduler_override: Option<&str>) -> String {
    use std::collections::hash_map::DefaultHasher;
    let content = fs::read_to_string(config_path).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    scheduler_override.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn load_cost_model(path: &Path) -> ConfigResult<CostModel> {
    if !path.exists() {
        return Ok(CostModel::default());
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

pub fn load_rl_session(config_path: &Path) -> ConfigResult<RlSession> {
    let config = load_simulation_config(config_path)?;
    let base = config_path.parent().unwrap_or_else(|| Path::new("."));

    let hw_dir = resolve_path(base, &config.hardware_profiles_dir);
    let profiles = load_hardware_profiles(&hw_dir)?;

    let cluster = build_cluster(&config.cluster, &profiles)?;
    let hardware_names: Vec<String> = config
        .cluster
        .nodes
        .iter()
        .flat_map(|n| n.gpus.iter().map(|g| g.profile.clone()))
        .collect();

    let profiles_dir = resolve_path(base, &config.profiles_dir);
    let model_profiles = load_model_profiles(&profiles_dir)?;

    let workload_path = resolve_path(base, &config.workload.path);
    let jobs = load_workload_with_profiles(&workload_path, &model_profiles, &hardware_names)?;

    let any_mig_capable = cluster.all_gpus().any(|g| g.mig_capable);
    let any_mig_job = jobs.iter().any(|j| j.is_mig_job());
    let mig_dir = resolve_path(base, &config.mig_profiles_dir);
    let mig_registry = if any_mig_job || any_mig_capable {
        resolve_mig_registry_for_cluster(&mig_dir, &hardware_names, any_mig_capable)?
    } else {
        None
    };
    if any_mig_job && mig_registry.is_none() {
        return Err(ConfigError::Invalid(
            "workload contains MIG jobs but no MIG profile registry is configured".into(),
        ));
    }

    let resource_manager = build_resource_manager(mig_registry, &config.scheduler.r#type);

    Ok(RlSession::new(
        cluster,
        resource_manager,
        jobs,
        zyvor_janus_simulator::DEFAULT_OBS_TOP_K,
    ))
}

pub fn build_resource_manager(
    mig_registry: Option<zyvor_janus_model::mig::MigProfileRegistry>,
    scheduler: &str,
) -> ResourceManager {
    let rm = match mig_registry {
        Some(registry) => ResourceManager::with_mig(registry),
        None => ResourceManager::new(),
    };
    if scheduler == "bestfit" {
        rm.with_gpu_selection(GpuSelectionPolicy::BestFit)
    } else {
        rm
    }
}

fn run_to_completion_with_policy_snapshots(
    cluster: Cluster,
    scheduler: &str,
    resource_manager: ResourceManager,
    jobs: Vec<Job>,
    jobs_total: usize,
) -> ConfigResult<(Cluster, SimulationMetrics, Vec<ClusterSnapshot>)> {
    Ok(match scheduler {
        "fifo" => {
            run_to_completion_snapshots(cluster, FifoScheduler, resource_manager, jobs, jobs_total)
        }
        "priority" => run_to_completion_snapshots(
            cluster,
            PriorityScheduler,
            resource_manager,
            jobs,
            jobs_total,
        ),
        "preemptive" | "zynera" => run_to_completion_snapshots(
            cluster,
            ZyneraScheduler::default(),
            resource_manager,
            jobs,
            jobs_total,
        ),
        "bestfit" => run_to_completion_snapshots(
            cluster,
            BestFitScheduler,
            resource_manager,
            jobs,
            jobs_total,
        ),
        other => {
            return Err(ConfigError::Invalid(format!(
                "unsupported scheduler type '{other}'"
            )));
        }
    })
}

pub fn run_to_completion_with_policy(
    cluster: Cluster,
    scheduler: &str,
    resource_manager: ResourceManager,
    jobs: Vec<Job>,
    jobs_total: usize,
) -> ConfigResult<(Cluster, SimulationMetrics)> {
    let (cluster, metrics, _snapshots) = run_to_completion_with_policy_snapshots(
        cluster,
        scheduler,
        resource_manager,
        jobs,
        jobs_total,
    )?;
    Ok((cluster, metrics))
}

fn run_to_completion_snapshots<S: Scheduler>(
    cluster: Cluster,
    scheduler: S,
    resource_manager: ResourceManager,
    jobs: Vec<Job>,
    jobs_total: usize,
) -> (Cluster, SimulationMetrics, Vec<ClusterSnapshot>) {
    let mut engine = SimulationEngine::with_resource_manager(cluster, scheduler, resource_manager)
        .with_replay_capture();
    engine.submit_jobs(jobs);
    engine.run();
    let snapshots = engine.take_replay_snapshots();
    (
        engine.cluster.clone(),
        SimulationMetrics::from_cluster(&engine.cluster, jobs_total),
        snapshots,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_sample_workload() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../configs/workloads/synthetic_m1.yaml");
        if root.exists() {
            let jobs = load_workload(&root).unwrap();
            assert!(!jobs.is_empty());
        }
    }

    #[test]
    fn runs_mig_workload() {
        let config = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../configs/clusters/mig_single.yaml");
        if !config.exists() {
            return;
        }
        let metrics = run_simulation(&config).unwrap();
        assert_eq!(metrics.jobs_completed, metrics.jobs_total);
        assert!(metrics.mig_reconfigs >= 1);
    }

    #[test]
    fn load_workload_rejects_invalid_gang_config() {
        let dir = std::env::temp_dir().join("zyvor_janus_test_invalid_gang");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("invalid_gang.yaml");
        std::fs::write(
            &path,
            r#"jobs:
  - id: bad-gang
    arrival_time: 0
    runtime: 10
    gpu_count: 5
    gang_enabled: true
    gang_size_nodes: 2
"#,
        )
        .unwrap();
        assert!(load_workload(&path).is_err());
    }

    #[test]
    fn topology_template_pcie_only_assigns_unique_groups() {
        let cfg = ClusterConfig {
            nodes: vec![NodeSpec {
                id: "n0".into(),
                gpus: vec![
                    GpuSpec {
                        id: "g0".into(),
                        profile: "H100_80GB".into(),
                        nvlink_group: None,
                    },
                    GpuSpec {
                        id: "g1".into(),
                        profile: "H100_80GB".into(),
                        nvlink_group: None,
                    },
                ],
            }],
            tenant_quotas: HashMap::new(),
            topology_template: "pcie_only".into(),
        };
        let mut profiles = HashMap::new();
        profiles.insert(
            "H100_80GB".into(),
            HardwareProfile {
                name: "H100_80GB".into(),
                memory_gb: 80.0,
                sm: None,
                mig_profiles: vec![],
                nvlink_bw_gbs: None,
                pcie_bw_gbs: None,
                mig: false,
            },
        );
        let cluster = build_cluster(&cfg, &profiles).unwrap();
        let g0 = cluster.gpu("g0").unwrap();
        let g1 = cluster.gpu("g1").unwrap();
        assert_ne!(g0.nvlink_group, g1.nvlink_group);
    }

    #[test]
    fn load_workload_with_mig_fields() {
        let workload =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/workloads/mig_m4.yaml");
        if !workload.exists() {
            return;
        }
        let jobs = load_workload(&workload).unwrap();
        let mig = jobs
            .iter()
            .find(|j| j.id == "mig-infer-a")
            .expect("mig job");
        assert_eq!(mig.mig_profile.as_deref(), Some("1g.10gb"));
        assert_eq!(mig.mig_count, Some(2));
        assert!(mig.is_mig_job());
    }

    #[test]
    fn resolve_config_twin_hint_reads_model_and_cluster_gpu_profile() {
        let config = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../configs/clusters/inference_llama.yaml");
        if !config.exists() {
            return;
        }
        let (gpu_type, model) = resolve_config_twin_hint(&config).expect("hint");
        assert_eq!(gpu_type, "H100");
        assert_eq!(model, "llama-70b");
    }

    #[test]
    fn resolve_config_twin_hint_none_for_non_inference_config() {
        let config = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../configs/clusters/small_h100.yaml");
        if !config.exists() {
            return;
        }
        assert_eq!(resolve_config_twin_hint(&config), None);
    }
}
