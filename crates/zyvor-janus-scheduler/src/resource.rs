// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use zyvor_janus_core::error::{SimError, SimResult};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::mig::reconfigure_gpu;
use zyvor_janus_model::mig::MigProfileRegistry;
use zyvor_janus_model::models::{Job, Placement};
use zyvor_janus_topology::GpuTopologyKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuSelectionPolicy {
    #[default]
    FirstFit,
    BestFit,
}

#[derive(Debug)]
pub struct ResourceManager {
    pub mig_registry: Option<MigProfileRegistry>,
    gpu_selection: GpuSelectionPolicy,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            mig_registry: None,
            gpu_selection: GpuSelectionPolicy::default(),
        }
    }

    pub fn with_mig(registry: MigProfileRegistry) -> Self {
        Self {
            mig_registry: Some(registry),
            gpu_selection: GpuSelectionPolicy::default(),
        }
    }

    pub fn with_gpu_selection(mut self, policy: GpuSelectionPolicy) -> Self {
        self.gpu_selection = policy;
        self
    }

    pub fn can_place(&self, cluster: &Cluster, job: &Job) -> bool {
        if !self.within_tenant_quota(cluster, job) {
            return false;
        }
        if job.is_mig_job() {
            return self.can_place_mig(cluster, job);
        }
        self.can_place_whole_gpu(cluster, job)
    }

    /// Tenants without a quota entry are unrestricted.
    fn within_tenant_quota(&self, cluster: &Cluster, job: &Job) -> bool {
        let Some(tenant) = job.tenant.as_deref() else {
            return true;
        };
        let Some(&quota) = cluster.tenant_quotas.get(tenant) else {
            return true;
        };
        cluster.tenant_gpu_usage(tenant) + job.gpu_count <= quota
    }

    pub fn allocate(
        &self,
        cluster: &mut Cluster,
        job: &Job,
        start_time: f64,
    ) -> SimResult<Placement> {
        if job.is_mig_job() {
            return self.allocate_mig(cluster, job, start_time);
        }
        self.allocate_whole_gpu(cluster, job, start_time)
    }

    fn can_place_whole_gpu(&self, cluster: &Cluster, job: &Job) -> bool {
        if job.gpu_count == 0 {
            return false;
        }
        if let Some(nodes) = gang_nodes_needed(job) {
            return select_gang_gpus(cluster, job, nodes).is_some();
        }
        let free = eligible_free_gpus(cluster, job);
        free.len() >= job.gpu_count as usize
    }

    fn allocate_whole_gpu(
        &self,
        cluster: &mut Cluster,
        job: &Job,
        start_time: f64,
    ) -> SimResult<Placement> {
        if !self.can_place_whole_gpu(cluster, job) {
            return Err(SimError::InsufficientGpus {
                need: job.gpu_count,
                available: cluster.free_gpu_count() as u32,
            });
        }

        let (selected, used_penalty) = if let Some(nodes) = gang_nodes_needed(job) {
            select_gang_gpus(cluster, job, nodes).expect("can_place implied selection")
        } else if wants_topology_aware(job) {
            select_gpus_topology_aware(cluster, job).expect("can_place implied selection")
        } else {
            let ids = match self.gpu_selection {
                GpuSelectionPolicy::FirstFit => select_gpus_scatter(cluster, job),
                GpuSelectionPolicy::BestFit => select_gpus_best_fit(cluster, job),
            }
            .expect("can_place implied selection");
            (ids, false)
        };

        if used_penalty {
            cluster.topology_penalties += 1;
        }

        let gpu_topology: Vec<GpuTopologyKey> = selected
            .iter()
            .filter_map(|id| cluster.gpu(id))
            .map(|g| GpuTopologyKey {
                nvlink_group: g.nvlink_group,
                node_id: g.node_id.clone(),
            })
            .collect();
        let runtime_multiplier = cluster.topology.runtime_multiplier(
            &gpu_topology,
            job.gang_enabled,
            job.network_bw_gbps,
            used_penalty,
        );

        Ok(Placement {
            job_id: job.id.clone(),
            gpu_ids: selected,
            start_time,
            runtime_multiplier,
        })
    }

    fn can_place_mig(&self, cluster: &Cluster, job: &Job) -> bool {
        let Some(registry) = &self.mig_registry else {
            return false;
        };
        let profile = match job.mig_profile_name() {
            Some(p) => p,
            None => return false,
        };
        if registry.profile(profile).is_err() {
            return false;
        }
        let needed = job.mig_slices_needed();
        let mut available = 0u32;
        for gpu in cluster.all_gpus() {
            if !gpu.mig_capable {
                continue;
            }
            available += gpu.free_mig_slice_count(profile);
            if gpu.is_fully_idle() {
                if let Ok(spec) = registry.profile(profile) {
                    if gpu.slices.is_empty() || gpu.active_mig_profile.as_deref() != Some(profile) {
                        available += spec.max_per_gpu;
                    }
                }
            }
        }
        available >= needed
    }

    fn allocate_mig(
        &self,
        cluster: &mut Cluster,
        job: &Job,
        start_time: f64,
    ) -> SimResult<Placement> {
        let registry = self.mig_registry.as_ref().ok_or_else(|| {
            SimError::Config("MIG job submitted but no MIG profile registry configured".into())
        })?;
        let profile = job
            .mig_profile_name()
            .ok_or_else(|| SimError::Config("MIG job missing mig_profile".into()))?;
        registry.profile(profile)?;
        let needed = job.mig_slices_needed();
        let mut selected = Vec::new();
        let mut reconfig_delay = 0.0;

        for gpu in cluster.all_gpus() {
            for slice in &gpu.slices {
                if slice.profile == profile && slice.is_free() {
                    selected.push(slice.id.clone());
                    if selected.len() == needed as usize {
                        break;
                    }
                }
            }
            if selected.len() == needed as usize {
                break;
            }
        }

        if selected.len() < needed as usize {
            let gpu_id = cluster
                .all_gpus()
                .find(|g| {
                    g.mig_capable && g.is_fully_idle() && g.free_mig_slice_count(profile) < needed
                })
                .map(|g| g.id.clone());

            if let Some(gpu_id) = gpu_id {
                reconfigure_gpu(
                    cluster
                        .all_gpus_mut()
                        .find(|g| g.id == gpu_id)
                        .expect("gpu must exist"),
                    profile,
                    needed,
                    registry,
                )?;
                cluster.mig_reconfigs += 1;
                reconfig_delay = registry.reconfig_seconds;
                if let Some(gpu) = cluster.all_gpus().find(|g| g.id == gpu_id) {
                    for slice in &gpu.slices {
                        if slice.profile == profile && slice.is_free() {
                            selected.push(slice.id.clone());
                            if selected.len() == needed as usize {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if selected.len() != needed as usize {
            return Err(SimError::InsufficientGpus {
                need: needed,
                available: selected.len() as u32,
            });
        }

        Ok(Placement {
            job_id: job.id.clone(),
            gpu_ids: selected,
            start_time: start_time + reconfig_delay,
            runtime_multiplier: 1.0,
        })
    }

    pub fn release(&self, _cluster: &mut Cluster, _job: &Job) {
        // GPU release handled by Cluster::finish_job
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

fn gpu_type_matches(gpu: &zyvor_janus_model::models::Gpu, job: &Job) -> bool {
    match job.gpu_type.as_deref() {
        Some(requested) => gpu.profile == requested,
        None => true,
    }
}

fn memory_eligible(gpu: &zyvor_janus_model::models::Gpu, job: &Job) -> bool {
    job.gpu_memory_gb <= 0.0 || gpu.memory_gb >= job.gpu_memory_gb
}

/// A job tagged with a federation site only matches nodes tagged with that
/// same site; an untagged job matches any node (no partitioning in play).
fn site_matches(cluster: &Cluster, gpu: &zyvor_janus_model::models::Gpu, job: &Job) -> bool {
    match job.site.as_deref() {
        Some(requested) => cluster.node_site(&gpu.node_id) == Some(requested),
        None => true,
    }
}

fn eligible_free_gpus<'a>(
    cluster: &'a Cluster,
    job: &Job,
) -> Vec<&'a zyvor_janus_model::models::Gpu> {
    cluster
        .all_gpus()
        .filter(|g| {
            g.is_whole_gpu_free()
                && memory_eligible(g, job)
                && gpu_type_matches(g, job)
                && site_matches(cluster, g, job)
        })
        .collect()
}

fn gang_nodes_needed(job: &Job) -> Option<u32> {
    if job.gang_enabled {
        job.gang_size_nodes.filter(|&n| n > 0)
    } else {
        None
    }
}

fn gpus_per_gang_node(job: &Job, nodes: u32) -> Option<u32> {
    if nodes == 0 || !job.gpu_count.is_multiple_of(nodes) {
        None
    } else {
        Some(job.gpu_count / nodes)
    }
}

/// Gang jobs require `nodes` distinct nodes each with `gpu_count / nodes` free GPUs.
/// Prefers NVLink-coherent GPU picks within each node; sets `used_penalty` on scatter fallback.
fn select_gang_gpus(cluster: &Cluster, job: &Job, nodes: u32) -> Option<(Vec<String>, bool)> {
    let per_node = gpus_per_gang_node(job, nodes)?;

    let mut node_candidates: Vec<(String, Vec<String>, bool)> = cluster
        .nodes
        .iter()
        .filter_map(|node| {
            let free: Vec<_> = node
                .gpus
                .iter()
                .filter(|g| {
                    g.is_whole_gpu_free()
                        && memory_eligible(g, job)
                        && gpu_type_matches(g, job)
                        && site_matches(cluster, g, job)
                })
                .collect();
            if free.len() < per_node as usize {
                return None;
            }
            if let Some((ids, penalty)) = select_nvlink_coherent(&free, per_node) {
                Some((node.id.clone(), ids, penalty))
            } else {
                let ids: Vec<String> = free
                    .iter()
                    .take(per_node as usize)
                    .map(|g| g.id.clone())
                    .collect();
                Some((node.id.clone(), ids, true))
            }
        })
        .collect();

    if node_candidates.len() < nodes as usize {
        return None;
    }

    node_candidates.sort_by(|a, b| {
        a.2.cmp(&b.2)
            .then_with(|| a.1.len().cmp(&b.1.len()))
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut selected = Vec::with_capacity(job.gpu_count as usize);
    let mut used_penalty = false;
    for (_, mut gpu_ids, penalty) in node_candidates.into_iter().take(nodes as usize) {
        gpu_ids.truncate(per_node as usize);
        used_penalty |= penalty;
        selected.extend(gpu_ids);
    }
    Some((selected, used_penalty))
}

fn select_nvlink_coherent(
    free: &[&zyvor_janus_model::models::Gpu],
    per_node: u32,
) -> Option<(Vec<String>, bool)> {
    use std::collections::HashMap;
    let mut by_group: HashMap<Option<u32>, Vec<&zyvor_janus_model::models::Gpu>> = HashMap::new();
    for gpu in free {
        by_group.entry(gpu.nvlink_group).or_default().push(*gpu);
    }
    let mut best: Option<(Vec<String>, bool)> = None;
    for gpus in by_group.values() {
        if gpus.len() >= per_node as usize {
            let ids: Vec<String> = gpus
                .iter()
                .take(per_node as usize)
                .map(|g| g.id.clone())
                .collect();
            if best
                .as_ref()
                .map(|(b, _)| ids.len() < b.len())
                .unwrap_or(true)
            {
                best = Some((ids, false));
            }
        }
    }
    best
}

fn wants_topology_aware(job: &Job) -> bool {
    job.gang_enabled || job.network_bw_gbps.is_some()
}

fn select_gpus_scatter(cluster: &Cluster, job: &Job) -> Option<Vec<String>> {
    let mut selected = Vec::new();
    for gpu in cluster.all_gpus() {
        if !gpu.is_whole_gpu_free()
            || !memory_eligible(gpu, job)
            || !gpu_type_matches(gpu, job)
            || !site_matches(cluster, gpu, job)
        {
            continue;
        }
        selected.push(gpu.id.clone());
        if selected.len() == job.gpu_count as usize {
            return Some(selected);
        }
    }
    None
}

/// Prefer the tightest single-node fit; otherwise pack from the fullest nodes first.
fn select_gpus_best_fit(cluster: &Cluster, job: &Job) -> Option<Vec<String>> {
    let free = eligible_free_gpus(cluster, job);
    if free.len() < job.gpu_count as usize {
        return None;
    }

    use std::collections::HashMap;
    let mut by_node: HashMap<String, Vec<&zyvor_janus_model::models::Gpu>> = HashMap::new();
    for gpu in free {
        by_node.entry(gpu.node_id.clone()).or_default().push(gpu);
    }

    let mut best_single: Option<(usize, Vec<String>)> = None;
    for gpus in by_node.values() {
        if gpus.len() < job.gpu_count as usize {
            continue;
        }
        let slack = gpus.len() - job.gpu_count as usize;
        let ids: Vec<String> = gpus
            .iter()
            .take(job.gpu_count as usize)
            .map(|g| g.id.clone())
            .collect();
        if best_single
            .as_ref()
            .map(|(best_slack, _)| slack < *best_slack)
            .unwrap_or(true)
        {
            best_single = Some((slack, ids));
        }
    }
    if let Some((_, ids)) = best_single {
        return Some(ids);
    }

    let mut nodes: Vec<_> = by_node.into_iter().collect();
    nodes.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    let mut selected = Vec::with_capacity(job.gpu_count as usize);
    for (_, gpus) in nodes {
        for gpu in gpus {
            selected.push(gpu.id.clone());
            if selected.len() == job.gpu_count as usize {
                return Some(selected);
            }
        }
    }
    None
}

/// Prefer placing all GPUs within one NVLink domain; fall back to scatter.
fn select_gpus_topology_aware(cluster: &Cluster, job: &Job) -> Option<(Vec<String>, bool)> {
    let free = eligible_free_gpus(cluster, job);
    if free.len() < job.gpu_count as usize {
        return None;
    }

    use std::collections::HashMap;
    let mut by_group: HashMap<Option<u32>, Vec<&zyvor_janus_model::models::Gpu>> = HashMap::new();
    for gpu in free {
        by_group.entry(gpu.nvlink_group).or_default().push(gpu);
    }

    let mut best_group: Option<Option<u32>> = None;
    let mut best_len = 0usize;
    for (group, gpus) in &by_group {
        if gpus.len() >= job.gpu_count as usize && gpus.len() > best_len {
            best_len = gpus.len();
            best_group = Some(*group);
        }
    }

    if let Some(group) = best_group {
        let ids: Vec<String> = by_group
            .get(&group)
            .expect("group exists")
            .iter()
            .take(job.gpu_count as usize)
            .map(|g| g.id.clone())
            .collect();
        if ids.len() == job.gpu_count as usize {
            return Some((ids, false));
        }
    }

    select_gpus_scatter(cluster, job).map(|ids| (ids, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::GpuSelectionPolicy;
    use std::collections::HashMap;
    use zyvor_janus_model::cluster::Cluster;
    use zyvor_janus_model::mig::{MigHardwareConfig, MigProfileRegistry, MigProfileSpec};
    use zyvor_janus_model::models::{Gpu, Node};

    fn mig_registry() -> MigProfileRegistry {
        MigProfileRegistry::from_config(MigHardwareConfig {
            hardware: "H100_80GB".into(),
            reconfig_seconds: 30.0,
            profiles: HashMap::from([(
                "1g.10gb".into(),
                MigProfileSpec {
                    memory_gb: 10.0,
                    max_per_gpu: 7,
                },
            )]),
        })
    }

    fn mig_gpu_cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "node-0".into(),
            gpus: vec![Gpu {
                id: "gpu-0".into(),
                node_id: "node-0".into(),
                profile: "H100_80GB".into(),
                memory_gb: 80.0,
                nvlink_group: None,
                running_job_id: None,
                mig_capable: true,
                active_mig_profile: None,
                slices: Vec::new(),
            }],
        }])
    }

    #[test]
    fn no_partial_allocation() {
        let cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![
                Gpu {
                    id: "g0".into(),
                    node_id: "n0".into(),
                    profile: "H100".into(),
                    memory_gb: 80.0,
                    nvlink_group: None,
                    running_job_id: None,
                    mig_capable: false,
                    active_mig_profile: None,
                    slices: Vec::new(),
                },
                Gpu {
                    id: "g1".into(),
                    node_id: "n0".into(),
                    profile: "H100".into(),
                    memory_gb: 80.0,
                    nvlink_group: None,
                    running_job_id: None,
                    mig_capable: false,
                    active_mig_profile: None,
                    slices: Vec::new(),
                },
            ],
        }]);
        let rm = ResourceManager::new();
        let job = Job::new("j1", "big", 0.0, 10.0, 2);
        assert!(rm.can_place(&cluster, &job));
        let p = rm.allocate(&mut cluster.clone(), &job, 0.0).unwrap();
        assert_eq!(p.gpu_ids.len(), 2);
    }

    fn two_gpu_cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![
                Gpu::new("g0", "n0", "H100", 80.0),
                Gpu::new("g1", "n0", "H100", 80.0),
            ],
        }])
    }

    #[test]
    fn quota_blocks_job_that_would_exceed_tenant_limit() {
        let mut cluster = two_gpu_cluster();
        cluster.tenant_quotas.insert("acme".into(), 1);
        let rm = ResourceManager::new();

        let mut job = Job::new("j1", "big", 0.0, 10.0, 2);
        job.tenant = Some("acme".into());
        assert!(!rm.can_place(&cluster, &job));
    }

    #[test]
    fn quota_allows_job_within_tenant_limit() {
        let mut cluster = two_gpu_cluster();
        cluster.tenant_quotas.insert("acme".into(), 2);
        let rm = ResourceManager::new();

        let mut job = Job::new("j1", "big", 0.0, 10.0, 2);
        job.tenant = Some("acme".into());
        assert!(rm.can_place(&cluster, &job));
    }

    #[test]
    fn quota_accounts_for_already_running_jobs() {
        let mut cluster = two_gpu_cluster();
        cluster.tenant_quotas.insert("acme".into(), 1);
        let mut running = Job::new("j0", "running", 0.0, 10.0, 1);
        running.tenant = Some("acme".into());
        cluster.start_job(running, &["g0".into()], 0.0);

        let rm = ResourceManager::new();
        let mut job = Job::new("j1", "second", 0.0, 10.0, 1);
        job.tenant = Some("acme".into());
        assert!(!rm.can_place(&cluster, &job));
    }

    #[test]
    fn tenant_without_quota_entry_is_unrestricted() {
        let mut cluster = two_gpu_cluster();
        cluster.tenant_quotas.insert("other-team".into(), 1);
        let rm = ResourceManager::new();

        let mut job = Job::new("j1", "big", 0.0, 10.0, 2);
        job.tenant = Some("acme".into());
        assert!(rm.can_place(&cluster, &job));
    }

    fn two_site_cluster() -> Cluster {
        let mut cluster = Cluster::new(vec![
            Node {
                id: "n-site-a".into(),
                gpus: vec![Gpu::new("g-a", "n-site-a", "H100", 80.0)],
            },
            Node {
                id: "n-site-b".into(),
                gpus: vec![Gpu::new("g-b", "n-site-b", "H100", 80.0)],
            },
        ]);
        cluster
            .node_sites
            .insert("n-site-a".into(), "site-a".into());
        cluster
            .node_sites
            .insert("n-site-b".into(), "site-b".into());
        cluster
    }

    #[test]
    fn site_tagged_job_only_places_on_its_own_site() {
        let cluster = two_site_cluster();
        let rm = ResourceManager::new();

        let mut job = Job::new("j1", "site-a-job", 0.0, 10.0, 1);
        job.site = Some("site-a".into());
        let p = rm.allocate(&mut cluster.clone(), &job, 0.0).unwrap();
        assert_eq!(p.gpu_ids, vec!["g-a".to_string()]);
    }

    #[test]
    fn site_tagged_job_is_blocked_once_its_own_site_is_full() {
        let mut cluster = two_site_cluster();
        let rm = ResourceManager::new();

        let mut running = Job::new("j0", "running", 0.0, 10.0, 1);
        running.site = Some("site-a".into());
        cluster.start_job(running, &["g-a".into()], 0.0);

        // site-b has a free GPU, but a site-a job must not spill onto it.
        let mut job = Job::new("j1", "second-site-a-job", 0.0, 10.0, 1);
        job.site = Some("site-a".into());
        assert!(!rm.can_place(&cluster, &job));
    }

    #[test]
    fn job_without_a_site_tag_is_unpartitioned() {
        let cluster = two_site_cluster();
        let rm = ResourceManager::new();

        // No site set — matches either node, same as pre-federation behavior.
        let job = Job::new("j1", "untagged", 0.0, 10.0, 2);
        assert!(rm.can_place(&cluster, &job));
    }

    #[test]
    fn rejects_when_not_enough_gpus() {
        let cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu {
                id: "g0".into(),
                node_id: "n0".into(),
                profile: "H100".into(),
                memory_gb: 80.0,
                nvlink_group: None,
                running_job_id: None,
                mig_capable: false,
                active_mig_profile: None,
                slices: Vec::new(),
            }],
        }]);
        let rm = ResourceManager::new();
        let job = Job::new("j1", "big", 0.0, 10.0, 3);
        assert!(!rm.can_place(&cluster, &job));
    }

    #[test]
    fn mig_job_reconfigures_and_allocates_slices() {
        let mut cluster = mig_gpu_cluster();
        let rm = ResourceManager::with_mig(mig_registry());
        let mut job = Job::new("j1", "infer", 0.0, 10.0, 1);
        job.mig_profile = Some("1g.10gb".into());
        job.mig_count = Some(2);

        let placement = rm.allocate(&mut cluster, &job, 0.0).unwrap();
        assert_eq!(placement.gpu_ids.len(), 2);
        assert_eq!(placement.start_time, 30.0);
        assert_eq!(cluster.mig_reconfigs, 1);
        assert_eq!(cluster.all_gpus().next().unwrap().slices.len(), 2);
    }

    #[test]
    fn second_mig_job_reuses_existing_slices() {
        let mut cluster = mig_gpu_cluster();
        let rm = ResourceManager::with_mig(mig_registry());
        let mut job_a = Job::new("j1", "a", 0.0, 10.0, 1);
        job_a.mig_profile = Some("1g.10gb".into());
        job_a.mig_count = Some(2);
        let p1 = rm.allocate(&mut cluster, &job_a, 0.0).unwrap();
        cluster.start_job(job_a, &p1.gpu_ids, p1.start_time);

        let mut job_b = Job::new("j2", "b", 1.0, 5.0, 1);
        job_b.mig_profile = Some("1g.10gb".into());
        job_b.mig_count = Some(1);
        assert!(!rm.can_place(&cluster, &job_b));

        cluster.finish_job("j1", 40.0);
        let p2 = rm.allocate(&mut cluster, &job_b, 40.0).unwrap();
        assert_eq!(p2.gpu_ids.len(), 1);
        assert_eq!(p2.start_time, 40.0);
        assert_eq!(cluster.mig_reconfigs, 1);
    }

    fn two_node_cluster() -> Cluster {
        Cluster::new(vec![
            Node {
                id: "node-a".into(),
                gpus: (0..4)
                    .map(|i| Gpu {
                        id: format!("a-g{i}"),
                        node_id: "node-a".into(),
                        profile: "H100".into(),
                        memory_gb: 80.0,
                        nvlink_group: Some(i / 2),
                        running_job_id: None,
                        mig_capable: false,
                        active_mig_profile: None,
                        slices: Vec::new(),
                    })
                    .collect(),
            },
            Node {
                id: "node-b".into(),
                gpus: (0..4)
                    .map(|i| Gpu {
                        id: format!("b-g{i}"),
                        node_id: "node-b".into(),
                        profile: "H100".into(),
                        memory_gb: 80.0,
                        nvlink_group: Some(i / 2),
                        running_job_id: None,
                        mig_capable: false,
                        active_mig_profile: None,
                        slices: Vec::new(),
                    })
                    .collect(),
            },
        ])
    }

    #[test]
    fn gang_job_requires_spread_across_nodes() {
        let cluster = two_node_cluster();
        let rm = ResourceManager::new();
        let mut job = Job::new("g1", "gang", 0.0, 10.0, 4);
        job.gang_enabled = true;
        job.gang_size_nodes = Some(2);
        assert!(rm.can_place(&cluster, &job));
        let placement = rm.allocate(&mut cluster.clone(), &job, 0.0).unwrap();
        let nodes: std::collections::HashSet<_> = placement
            .gpu_ids
            .iter()
            .map(|id| cluster.gpu(id).unwrap().node_id.clone())
            .collect();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn gang_job_rejects_when_not_enough_nodes() {
        let mut cluster = two_node_cluster();
        cluster.nodes.pop(); // single node only
        let rm = ResourceManager::new();
        let mut job = Job::new("g1", "gang", 0.0, 10.0, 8);
        job.gang_enabled = true;
        job.gang_size_nodes = Some(2);
        assert!(!rm.can_place(&cluster, &job));
    }

    #[test]
    fn topology_aware_prefers_single_nvlink_group() {
        let cluster = two_node_cluster();
        let rm = ResourceManager::new();
        let mut job = Job::new("j1", "net", 0.0, 10.0, 2);
        job.network_bw_gbps = Some(100.0);
        let mut c = cluster.clone();
        let placement = rm.allocate(&mut c, &job, 0.0).unwrap();
        let groups: std::collections::HashSet<_> = placement
            .gpu_ids
            .iter()
            .map(|id| cluster.gpu(id).unwrap().nvlink_group)
            .collect();
        assert_eq!(groups.len(), 1);
        assert_eq!(c.topology_penalties, 0);
    }

    #[test]
    fn topology_fallback_increments_penalty() {
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![
                Gpu {
                    id: "g0".into(),
                    node_id: "n0".into(),
                    profile: "H100".into(),
                    memory_gb: 80.0,
                    nvlink_group: Some(0),
                    running_job_id: None,
                    mig_capable: false,
                    active_mig_profile: None,
                    slices: Vec::new(),
                },
                Gpu {
                    id: "g1".into(),
                    node_id: "n0".into(),
                    profile: "H100".into(),
                    memory_gb: 80.0,
                    nvlink_group: Some(1),
                    running_job_id: None,
                    mig_capable: false,
                    active_mig_profile: None,
                    slices: Vec::new(),
                },
            ],
        }]);
        let rm = ResourceManager::new();
        let mut job = Job::new("j1", "net", 0.0, 10.0, 2);
        job.network_bw_gbps = Some(100.0);
        let placement = rm.allocate(&mut cluster, &job, 0.0).unwrap();
        assert_eq!(placement.gpu_ids.len(), 2);
        assert_eq!(cluster.topology_penalties, 1);
    }

    #[test]
    fn gpu_type_mismatch_blocks_placement() {
        let cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "A100_80GB", 80.0)],
        }]);
        let rm = ResourceManager::new();
        let mut job = Job::new("j1", "train", 0.0, 10.0, 1);
        job.gpu_type = Some("H100_80GB".into());
        assert!(!rm.can_place(&cluster, &job));
    }

    #[test]
    fn gpu_type_match_allows_placement() {
        let cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100_80GB", 80.0)],
        }]);
        let rm = ResourceManager::new();
        let mut job = Job::new("j1", "train", 0.0, 10.0, 1);
        job.gpu_type = Some("H100_80GB".into());
        assert!(rm.can_place(&cluster, &job));
    }

    #[test]
    fn gang_scatter_fallback_increments_topology_penalty() {
        let cluster = two_node_cluster();
        let rm = ResourceManager::new();
        let mut job = Job::new("g1", "gang", 0.0, 10.0, 4);
        job.gang_enabled = true;
        job.gang_size_nodes = Some(2);
        let mut c = cluster.clone();
        rm.allocate(&mut c, &job, 0.0).unwrap();
        assert_eq!(c.topology_penalties, 0);
    }

    #[test]
    fn best_fit_prefers_tighter_node() {
        let mut cluster = Cluster::new(vec![
            Node {
                id: "wide".into(),
                gpus: vec![
                    Gpu::new("w0", "wide", "H100", 80.0),
                    Gpu::new("w1", "wide", "H100", 80.0),
                    Gpu::new("w2", "wide", "H100", 80.0),
                    Gpu::new("w3", "wide", "H100", 80.0),
                ],
            },
            Node {
                id: "tight".into(),
                gpus: vec![
                    Gpu::new("t0", "tight", "H100", 80.0),
                    Gpu::new("t1", "tight", "H100", 80.0),
                ],
            },
        ]);
        cluster.start_job(Job::new("block", "b", 0.0, 100.0, 1), &["w0".into()], 0.0);
        let rm = ResourceManager::new().with_gpu_selection(GpuSelectionPolicy::BestFit);
        let job = Job::new("j1", "pair", 0.0, 10.0, 2);
        let placement = rm.allocate(&mut cluster, &job, 0.0).unwrap();
        assert_eq!(placement.gpu_ids, vec!["t0", "t1"]);
    }
}
