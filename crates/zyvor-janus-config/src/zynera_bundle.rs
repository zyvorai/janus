// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

//! Load normalized Zynera export bundles (FabricAIJob, FabricGpuNode, FabricQuota).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yaml::Value;
use zyvor_janus_metrics::{JobsTimeline, SimulationMetrics};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Gpu, Job, Node};
use zyvor_janus_topology::TopologyGraph;

use crate::{
    load_hardware_profiles, resolve_mig_registry_for_cluster, ConfigError, ConfigResult,
    HardwareProfile, SimulationReport,
};

const ZYNERA_API_GROUP: &str = "zynera.ai/v1";
/// Real label the ai-operator's federated-training-run controller sets on
/// every per-site `FabricAIJob` it creates (see
/// `fabricfederatedtrainingrun_controller.go`'s per-site job labels). Reused
/// here on hand-authored `FabricGpuNode` fixtures too, tagging which
/// federation site a node belongs to.
const FEDERATION_SITE_LABEL: &str = "zynera.ai/federated-training-site";

#[derive(Debug, Clone, Deserialize)]
pub struct GpuTypeRegistry {
    pub mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelProfileEntry {
    pub runtime_seconds: f64,
    pub gpu_memory_gb: f64,
    #[serde(default)]
    pub prefill_ms_per_token: Option<f64>,
    #[serde(default)]
    pub decode_tps: Option<f64>,
    #[serde(default)]
    pub max_batch: Option<u32>,
}

impl ModelProfileEntry {
    pub fn prefill_ms_per_token(&self) -> f64 {
        self.prefill_ms_per_token.unwrap_or(0.08)
    }

    pub fn decode_tps(&self) -> f64 {
        self.decode_tps.unwrap_or(120.0)
    }

    pub fn max_batch(&self) -> u32 {
        self.max_batch.unwrap_or(32)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelProfile {
    pub model: String,
    pub profiles: HashMap<String, ModelProfileEntry>,
}

/// Metadata read from a `FabricFederatedTrainingRun` in the bundle's
/// `federation/` directory, if present. Informational only — it doesn't
/// currently change scheduling, but recognizing the kind (instead of
/// silently dropping it like every non-Job/Node/Quota kind) lets a report
/// show what federation config the bundle's jobs were generated under.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FederationRunMeta {
    pub name: String,
    pub target_clusters: Vec<String>,
    pub secure_aggregation: bool,
    pub dropout_recovery: bool,
}

#[derive(Debug, Clone)]
pub struct ZyneraBundle {
    pub jobs: Vec<Job>,
    pub cluster: Cluster,
    pub federation: Option<FederationRunMeta>,
}

pub fn load_gpu_type_registry(path: &Path) -> ConfigResult<GpuTypeRegistry> {
    let content = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

pub fn load_model_profiles(dir: &Path) -> ConfigResult<HashMap<String, ModelProfile>> {
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
        let profile: ModelProfile = serde_yaml::from_str(&content)?;
        profiles.insert(profile.model.clone(), profile);
    }
    Ok(profiles)
}

fn map_gpu_type(gpu_type: &str, registry: &GpuTypeRegistry) -> ConfigResult<String> {
    registry
        .mappings
        .get(gpu_type)
        .cloned()
        .ok_or_else(|| ConfigError::Invalid(format!("unknown gpuType '{gpu_type}'")))
}

fn yaml_documents(content: &str) -> ConfigResult<Vec<Value>> {
    let mut docs = Vec::new();
    for chunk in content.split("\n---") {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        let doc: Value = serde_yaml::from_str(trimmed)?;
        if doc.is_null() {
            continue;
        }
        // `kubectl get <resource> -A -o yaml` wraps multiple resources in a
        // single `kind: List` document with an `items:` array, rather than
        // `---`-separating them — exactly the export command documented in
        // docs/zynera_input.md. Unwrap it so downstream kind/apiVersion
        // checks see the actual FabricAIJob/FabricGpuNode/FabricQuota docs.
        if kind_of(&doc) == Some("List") {
            if let Some(items) = doc.get("items").and_then(|i| i.as_sequence()) {
                docs.extend(items.iter().cloned());
            }
            continue;
        }
        docs.push(doc);
    }
    Ok(docs)
}

fn kind_of(doc: &Value) -> Option<&str> {
    doc.get("kind").and_then(|k| k.as_str())
}

fn api_version_of(doc: &Value) -> Option<&str> {
    doc.get("apiVersion").and_then(|k| k.as_str())
}

fn site_label_of(meta: &Value) -> Option<String> {
    meta.get("labels")
        .and_then(|l| l.get(FEDERATION_SITE_LABEL))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn validate_forge_doc(doc: &Value) -> ConfigResult<()> {
    match api_version_of(doc) {
        Some(v) if v == ZYNERA_API_GROUP => Ok(()),
        Some(v) => Err(ConfigError::Invalid(format!(
            "unsupported apiVersion '{v}', expected '{ZYNERA_API_GROUP}'"
        ))),
        None => Err(ConfigError::Invalid("missing apiVersion".into())),
    }
}

fn collect_yaml_files(dir: &Path) -> ConfigResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "yaml" || ext == "yml" {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn parse_fabric_quotas(dir: &Path) -> ConfigResult<Vec<Value>> {
    let mut quotas = Vec::new();
    for path in collect_yaml_files(dir)? {
        let content = fs::read_to_string(&path)?;
        for doc in yaml_documents(&content)? {
            if kind_of(&doc) == Some("FabricQuota") {
                validate_forge_doc(&doc)?;
                quotas.push(doc);
            }
        }
    }
    Ok(quotas)
}

fn parse_tenant_quotas(quotas: &[Value]) -> HashMap<String, u32> {
    let mut result = HashMap::new();
    for quota in quotas {
        let Some(team) = quota
            .get("spec")
            .and_then(|s| s.get("team"))
            .and_then(|t| t.as_str())
        else {
            continue;
        };
        let Some(max_gpus) = quota
            .get("spec")
            .and_then(|s| s.get("gpuQuota"))
            .and_then(|q| q.get("maxGPUs"))
            .and_then(|m| m.as_i64())
        else {
            continue;
        };
        result.insert(team.to_string(), max_gpus as u32);
    }
    result
}

fn parse_federated_training_runs(dir: &Path) -> ConfigResult<Vec<Value>> {
    let mut runs = Vec::new();
    for path in collect_yaml_files(dir)? {
        let content = fs::read_to_string(&path)?;
        for doc in yaml_documents(&content)? {
            if kind_of(&doc) == Some("FabricFederatedTrainingRun") {
                validate_forge_doc(&doc)?;
                runs.push(doc);
            }
        }
    }
    Ok(runs)
}

/// Only one federated-training-run's worth of jobs is expected per bundle
/// (the export command in `docs/zynera_input.md` scopes to a single run's
/// namespace); the first document wins if more than one is present.
fn federation_meta_from(runs: &[Value]) -> Option<FederationRunMeta> {
    let doc = runs.first()?;
    let name = doc
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();
    let spec = doc.get("spec")?;
    let target_clusters = spec
        .get("targetClusters")
        .and_then(|t| t.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let secure_aggregation = spec
        .get("secureAggregation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dropout_recovery = spec
        .get("dropoutRecovery")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Some(FederationRunMeta {
        name,
        target_clusters,
        secure_aggregation,
        dropout_recovery,
    })
}

fn resolve_tenant(namespace: &str, quotas: &[Value]) -> Option<String> {
    for quota in quotas {
        let spec = quota.get("spec")?;
        let team = spec.get("team").and_then(|t| t.as_str())?;
        if let Some(namespaces) = spec.get("namespaces").and_then(|n| n.as_sequence()) {
            for ns in namespaces {
                if ns.as_str() == Some(namespace) {
                    return Some(team.to_string());
                }
            }
        } else {
            return Some(team.to_string());
        }
    }
    None
}

fn gpu_count_from_spec(spec: &Value) -> u32 {
    let distributed = spec.get("distributed");
    if distributed
        .and_then(|d| d.get("enabled"))
        .and_then(|e| e.as_bool())
        .unwrap_or(false)
    {
        let nodes = distributed
            .and_then(|d| d.get("nodes"))
            .and_then(|n| n.as_i64())
            .unwrap_or(1) as u32;
        let gpn = distributed
            .and_then(|d| d.get("gpusPerNode"))
            .and_then(|n| n.as_i64())
            .unwrap_or(1) as u32;
        nodes * gpn
    } else {
        spec.get("gpus").and_then(|g| g.as_i64()).unwrap_or(1) as u32
    }
}

fn parse_arrival_time(meta: &Value) -> f64 {
    meta.get("creationTimestamp")
        .and_then(|t| t.as_str())
        .map(|_| 0.0)
        .unwrap_or(0.0)
}

fn parse_duration_secs(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if let Some(num) = s.strip_suffix('s') {
        return num.parse().ok();
    }
    if let Some(num) = s.strip_suffix('m') {
        return num.parse::<f64>().ok().map(|m| m * 60.0);
    }
    if let Some(num) = s.strip_suffix('h') {
        return num.parse::<f64>().ok().map(|h| h * 3600.0);
    }
    s.parse().ok()
}

fn lookup_runtime(
    model: &str,
    gpu_type: &str,
    profiles: &HashMap<String, ModelProfile>,
) -> ConfigResult<(f64, f64)> {
    let profile = profiles.get(model).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "no calibrated profile for model '{model}' (required in profiles-dir)"
        ))
    })?;
    let entry = profile.profiles.get(gpu_type).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "no calibrated profile for model '{model}' gpuType '{gpu_type}'"
        ))
    })?;
    Ok((entry.runtime_seconds, entry.gpu_memory_gb))
}

pub fn parse_fabric_ai_job(
    doc: &Value,
    quotas: &[Value],
    model_profiles: &HashMap<String, ModelProfile>,
    gpu_registry: &GpuTypeRegistry,
) -> ConfigResult<Job> {
    validate_forge_doc(doc)?;
    if kind_of(doc) != Some("FabricAIJob") {
        return Err(ConfigError::Invalid("expected FabricAIJob".into()));
    }

    let meta = doc
        .get("metadata")
        .ok_or_else(|| ConfigError::Invalid("missing metadata".into()))?;
    let spec = doc
        .get("spec")
        .ok_or_else(|| ConfigError::Invalid("missing spec".into()))?;

    let name = meta
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");
    let namespace = meta
        .get("namespace")
        .and_then(|n| n.as_str())
        .unwrap_or("default")
        .to_string();

    let annotations = meta.get("annotations").and_then(|a| a.as_mapping());
    let gang_enabled = annotations
        .and_then(|a| a.get(Value::from("zynera.ai/gang-schedule")))
        .and_then(|v| v.as_str())
        .map(|s| s == "true")
        .unwrap_or(false);
    let gang_size_nodes = annotations
        .and_then(|a| a.get(Value::from("zynera.ai/gang-size")))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());
    let gang_timeout_secs = annotations
        .and_then(|a| a.get(Value::from("zynera.ai/gang-timeout")))
        .and_then(|v| v.as_str())
        .and_then(parse_duration_secs);

    let gpu_type_raw = spec
        .get("gpuType")
        .and_then(|g| g.as_str())
        .unwrap_or("any")
        .to_string();
    let gpu_type = map_gpu_type(&gpu_type_raw, gpu_registry).unwrap_or(gpu_type_raw.clone());
    let model = spec
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(name)
        .to_string();

    let (runtime, gpu_memory_gb) = lookup_runtime(&model, &gpu_type_raw, model_profiles)?;

    let mig = spec.get("mig");
    let mig_profile = mig
        .and_then(|m| m.get("profile"))
        .and_then(|p| p.as_str())
        .map(String::from);
    let mig_count = mig
        .and_then(|m| m.get("count"))
        .and_then(|c| c.as_i64())
        .map(|c| c as u32);

    let network_bw = match spec.get("network").and_then(|n| n.as_str()) {
        Some("rdma") => Some(400.0),
        Some("sriov") => Some(200.0),
        _ => None,
    };

    let priority = spec.get("priority").and_then(|p| p.as_i64()).unwrap_or(0) as u32;

    let gpu_count = if mig_profile.is_some() {
        mig_count.unwrap_or(1)
    } else {
        gpu_count_from_spec(spec)
    };

    let job = Job {
        id: format!("{namespace}/{name}"),
        name: name.to_string(),
        arrival_time: parse_arrival_time(meta),
        runtime,
        gpu_count,
        gpu_memory_gb,
        priority,
        tenant: resolve_tenant(&namespace, quotas),
        network_bw_gbps: network_bw,
        gpu_type: Some(gpu_type),
        namespace: Some(namespace),
        site: site_label_of(meta),
        gang_enabled,
        gang_size_nodes,
        gang_timeout_secs,
        mig_profile,
        mig_count,
        ..Job::new("", "", 0.0, 0.0, 0)
    };
    crate::validate_job_gang_config(&job)?;
    Ok(job)
}

pub fn parse_fabric_gpu_nodes(
    dir: &Path,
    gpu_registry: &GpuTypeRegistry,
    hw_profiles: &HashMap<String, HardwareProfile>,
) -> ConfigResult<Cluster> {
    let mut nodes = Vec::new();
    let mut node_sites: HashMap<String, String> = HashMap::new();
    for path in collect_yaml_files(dir)? {
        let content = fs::read_to_string(&path)?;
        for doc in yaml_documents(&content)? {
            if kind_of(&doc) != Some("FabricGpuNode") {
                continue;
            }
            validate_forge_doc(&doc)?;
            let spec = doc
                .get("spec")
                .ok_or_else(|| ConfigError::Invalid("missing spec".into()))?;
            let site = doc.get("metadata").and_then(site_label_of);
            let node_name = spec
                .get("nodeName")
                .and_then(|n| n.as_str())
                .ok_or_else(|| ConfigError::Invalid("FabricGpuNode missing nodeName".into()))?;
            let gpu_type = spec
                .get("gpuType")
                .and_then(|g| g.as_str())
                .unwrap_or("any");
            let gpu_count = spec.get("gpuCount").and_then(|g| g.as_i64()).unwrap_or(1) as u32;
            let memory_gb = spec
                .get("memoryGB")
                .and_then(|m| m.as_i64())
                .map(|m| m as f64)
                .or_else(|| {
                    map_gpu_type(gpu_type, gpu_registry)
                        .ok()
                        .and_then(|p| hw_profiles.get(&p).map(|hp| hp.memory_gb))
                })
                .unwrap_or(80.0);

            let profile_name = map_gpu_type(gpu_type, gpu_registry)?;

            let hw_profile = hw_profiles.get(&profile_name);
            let mut gpus = Vec::new();
            for i in 0..gpu_count {
                let mut gpu = Gpu::new(
                    format!("{node_name}-gpu-{i}"),
                    node_name.to_string(),
                    profile_name.clone(),
                    memory_gb,
                );
                gpu.nvlink_group = Some(i / 2);
                gpu.mig_capable = hw_profile.map(|p| p.mig).unwrap_or(false);
                gpus.push(gpu);
            }
            if let Some(site) = site {
                node_sites.insert(node_name.to_string(), site);
            }
            nodes.push(Node {
                id: node_name.to_string(),
                gpus,
            });
        }
    }
    if nodes.is_empty() {
        return Err(ConfigError::Invalid(
            "no FabricGpuNode documents found in cluster/".into(),
        ));
    }
    let mut nvlink_bw: f64 = 900.0;
    let mut pcie_bw: f64 = 64.0;
    for node in &nodes {
        for gpu in &node.gpus {
            if let Some(profile) = hw_profiles.get(&gpu.profile) {
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
    cluster.node_sites = node_sites;
    Ok(cluster)
}

pub fn load_zynera_bundle(
    bundle_dir: &Path,
    profiles_dir: &Path,
    gpu_registry_path: &Path,
    hardware_profiles_dir: &Path,
) -> ConfigResult<ZyneraBundle> {
    let quotas_dir = bundle_dir.join("quotas");
    let jobs_dir = bundle_dir.join("jobs");
    let cluster_dir = bundle_dir.join("cluster");
    let federation_dir = bundle_dir.join("federation");

    let quotas = parse_fabric_quotas(&quotas_dir)?;
    let federation = federation_meta_from(&parse_federated_training_runs(&federation_dir)?);
    let model_profiles = load_model_profiles(profiles_dir)?;
    let gpu_registry = load_gpu_type_registry(gpu_registry_path)?;
    let hw_profiles = load_hardware_profiles(hardware_profiles_dir)?;

    let mut jobs = Vec::new();
    for path in collect_yaml_files(&jobs_dir)? {
        let content = fs::read_to_string(&path)?;
        for doc in yaml_documents(&content)? {
            if kind_of(&doc) == Some("FabricAIJob") {
                jobs.push(parse_fabric_ai_job(
                    &doc,
                    &quotas,
                    &model_profiles,
                    &gpu_registry,
                )?);
            }
        }
    }

    if jobs.is_empty() {
        return Err(ConfigError::Invalid(
            "no FabricAIJob documents in jobs/".into(),
        ));
    }

    let mut cluster = parse_fabric_gpu_nodes(&cluster_dir, &gpu_registry, &hw_profiles)?;
    cluster.tenant_quotas = parse_tenant_quotas(&quotas);

    Ok(ZyneraBundle {
        jobs,
        cluster,
        federation,
    })
}

pub fn run_zynera_bundle_report(
    bundle_dir: &Path,
    profiles_dir: &Path,
    gpu_registry_path: &Path,
    hardware_profiles_dir: &Path,
    mig_profiles_dir: &Path,
    scheduler: &str,
) -> ConfigResult<SimulationReport> {
    let bundle = load_zynera_bundle(
        bundle_dir,
        profiles_dir,
        gpu_registry_path,
        hardware_profiles_dir,
    )?;
    let jobs_total = bundle.jobs.len();
    let federation = bundle.federation.clone();
    let hardware_names: Vec<String> = bundle
        .cluster
        .all_gpus()
        .map(|g| g.profile.clone())
        .collect();
    let any_mig_capable = bundle.cluster.all_gpus().any(|g| g.mig_capable);
    let any_mig_job = bundle.jobs.iter().any(|j| j.is_mig_job());
    let mig_registry = if any_mig_job || any_mig_capable {
        resolve_mig_registry_for_cluster(mig_profiles_dir, &hardware_names, any_mig_capable)?
    } else {
        None
    };
    if any_mig_job && mig_registry.is_none() {
        return Err(ConfigError::Invalid(
            "zynera bundle contains MIG jobs but no MIG profile registry is configured".into(),
        ));
    }
    let resource_manager = crate::build_resource_manager(mig_registry, scheduler);
    let (cluster, metrics) = crate::run_to_completion_with_policy(
        bundle.cluster,
        scheduler,
        resource_manager,
        bundle.jobs,
        jobs_total,
    )?;
    let serving_trace = crate::serving_trace::export_serving_trace_from_cluster(&cluster);
    Ok(SimulationReport {
        metrics,
        timeline: JobsTimeline::from_cluster(&cluster),
        decisions: cluster.decision_log.clone(),
        snapshots: Vec::new(),
        scheduler: scheduler.to_string(),
        config_hash: String::new(),
        benchmark: None,
        federation,
        serving_trace,
    })
}

pub fn run_zynera_bundle(
    bundle_dir: &Path,
    profiles_dir: &Path,
    gpu_registry_path: &Path,
    hardware_profiles_dir: &Path,
    mig_profiles_dir: &Path,
    scheduler: &str,
) -> ConfigResult<SimulationMetrics> {
    Ok(run_zynera_bundle_report(
        bundle_dir,
        profiles_dir,
        gpu_registry_path,
        hardware_profiles_dir,
        mig_profiles_dir,
        scheduler,
    )?
    .metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn gpu_count_distributed() {
        let spec: Value = serde_yaml::from_str(
            r#"
            gpus: 8
            distributed:
              enabled: true
              nodes: 4
              gpusPerNode: 8
            "#,
        )
        .unwrap();
        assert_eq!(gpu_count_from_spec(&spec), 32);
    }

    #[test]
    fn gpu_count_non_distributed() {
        let spec: Value = serde_yaml::from_str("gpus: 4").unwrap();
        assert_eq!(gpu_count_from_spec(&spec), 4);
    }

    #[test]
    fn yaml_documents_unwraps_kubectl_list_output() {
        // `kubectl get fabricaijobs -A -o yaml` (the exact command
        // docs/zynera_input.md tells users to run) wraps every matching
        // resource in a single `kind: List` document instead of
        // `---`-separating them.
        let content = r#"
apiVersion: v1
items:
- apiVersion: zynera.ai/v1
  kind: FabricAIJob
  metadata:
    name: job-a
    namespace: default
  spec:
    model: llama-7b
    gpus: 4
    gpuType: A100
- apiVersion: zynera.ai/v1
  kind: FabricAIJob
  metadata:
    name: job-b
    namespace: default
  spec:
    model: gpt-13b
    gpus: 2
    gpuType: H100
kind: List
metadata:
  resourceVersion: ""
"#;
        let docs = yaml_documents(content).expect("parse kubectl List output");
        assert_eq!(docs.len(), 2);
        assert_eq!(kind_of(&docs[0]), Some("FabricAIJob"));
        assert_eq!(
            docs[0]
                .get("metadata")
                .and_then(|m| m.get("name"))
                .and_then(|n| n.as_str()),
            Some("job-a")
        );
        assert_eq!(api_version_of(&docs[1]), Some("zynera.ai/v1"));
    }

    #[test]
    fn yaml_documents_still_handles_dash_separated_docs() {
        let content = "apiVersion: zynera.ai/v1\nkind: FabricAIJob\nmetadata:\n  name: a\n---\napiVersion: zynera.ai/v1\nkind: FabricAIJob\nmetadata:\n  name: b\n";
        let docs = yaml_documents(content).expect("parse multi-doc yaml");
        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn yaml_documents_empty_list_yields_no_docs() {
        let content = "apiVersion: v1\nitems: []\nkind: List\n";
        let docs = yaml_documents(content).expect("parse empty list");
        assert!(docs.is_empty());
    }

    #[test]
    fn site_label_of_reads_the_federation_site_label() {
        let meta: Value =
            serde_yaml::from_str("labels:\n  zynera.ai/federated-training-site: site-a\n").unwrap();
        assert_eq!(site_label_of(&meta).as_deref(), Some("site-a"));
    }

    #[test]
    fn site_label_of_returns_none_without_the_label() {
        let meta: Value = serde_yaml::from_str("labels:\n  other: x\n").unwrap();
        assert_eq!(site_label_of(&meta), None);
    }

    #[test]
    fn parse_fabric_ai_job_carries_the_site_label_onto_the_job() {
        let doc: Value = serde_yaml::from_str(
            r#"
apiVersion: zynera.ai/v1
kind: FabricAIJob
metadata:
  name: job-a
  namespace: default
  labels:
    zynera.ai/federated-training-site: site-a
spec:
  model: llama-7b
  gpus: 1
  gpuType: H100
"#,
        )
        .unwrap();
        let registry = GpuTypeRegistry {
            mappings: HashMap::from([("H100".to_string(), "H100_80GB".to_string())]),
        };
        let mut profiles = HashMap::new();
        profiles.insert(
            "llama-7b".to_string(),
            ModelProfile {
                model: "llama-7b".to_string(),
                profiles: HashMap::from([(
                    "H100".to_string(),
                    ModelProfileEntry {
                        runtime_seconds: 10.0,
                        gpu_memory_gb: 40.0,
                        prefill_ms_per_token: None,
                        decode_tps: None,
                        max_batch: None,
                    },
                )]),
            },
        );
        let job = parse_fabric_ai_job(&doc, &[], &profiles, &registry).unwrap();
        assert_eq!(job.site.as_deref(), Some("site-a"));
    }

    #[test]
    fn parse_fabric_ai_job_without_the_label_has_no_site() {
        let doc: Value = serde_yaml::from_str(
            r#"
apiVersion: zynera.ai/v1
kind: FabricAIJob
metadata:
  name: job-a
  namespace: default
spec:
  model: llama-7b
  gpus: 1
  gpuType: H100
"#,
        )
        .unwrap();
        let registry = GpuTypeRegistry {
            mappings: HashMap::from([("H100".to_string(), "H100_80GB".to_string())]),
        };
        let mut profiles = HashMap::new();
        profiles.insert(
            "llama-7b".to_string(),
            ModelProfile {
                model: "llama-7b".to_string(),
                profiles: HashMap::from([(
                    "H100".to_string(),
                    ModelProfileEntry {
                        runtime_seconds: 10.0,
                        gpu_memory_gb: 40.0,
                        prefill_ms_per_token: None,
                        decode_tps: None,
                        max_batch: None,
                    },
                )]),
            },
        );
        let job = parse_fabric_ai_job(&doc, &[], &profiles, &registry).unwrap();
        assert_eq!(job.site, None);
    }

    #[test]
    fn federation_meta_from_parses_target_clusters_and_flags() {
        let doc: Value = serde_yaml::from_str(
            r#"
apiVersion: zynera.ai/v1
kind: FabricFederatedTrainingRun
metadata:
  name: run-a
spec:
  targetClusters: [site-a, site-b, site-c]
  secureAggregation: true
  dropoutRecovery: true
"#,
        )
        .unwrap();
        let meta = federation_meta_from(&[doc]).expect("federation meta");
        assert_eq!(meta.name, "run-a");
        assert_eq!(
            meta.target_clusters,
            vec![
                "site-a".to_string(),
                "site-b".to_string(),
                "site-c".to_string()
            ]
        );
        assert!(meta.secure_aggregation);
        assert!(meta.dropout_recovery);
    }

    #[test]
    fn federation_meta_from_empty_is_none() {
        assert!(federation_meta_from(&[]).is_none());
    }

    #[test]
    fn loads_fixture_zynera_bundle() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/zynera");
        if !root.exists() {
            return;
        }
        let profiles = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/profiles");
        let registry =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/gpu_type_registry.yaml");
        let hw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/hardware");

        let bundle = load_zynera_bundle(&root, &profiles, &registry, &hw).unwrap();
        let gang = bundle
            .jobs
            .iter()
            .find(|j| j.name == "gpt-distributed-training")
            .expect("gang job");
        assert_eq!(gang.gpu_count, 32);
        assert_eq!(gang.tenant.as_deref(), Some("ml-training"));

        let mig = bundle
            .jobs
            .iter()
            .find(|j| j.name == "mig-inference")
            .expect("mig job");
        assert_eq!(mig.gpu_count, 2);
        assert_eq!(mig.mig_profile.as_deref(), Some("1g.10gb"));
    }

    #[test]
    fn zynera_bundle_e2e_simulation() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/zynera");
        if !root.exists() {
            return;
        }
        let profiles = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/profiles");
        let registry =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/gpu_type_registry.yaml");
        let hw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/hardware");
        let mig = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/mig");

        let metrics = run_zynera_bundle(&root, &profiles, &registry, &hw, &mig, "fifo").unwrap();
        assert_eq!(metrics.jobs_completed, metrics.jobs_total);
        assert!(metrics.jobs_total >= 2);
    }
}
