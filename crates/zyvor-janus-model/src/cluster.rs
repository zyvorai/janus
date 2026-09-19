// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::models::{Gpu, Job, JobState, MigSlice, Node};
use zyvor_janus_topology::TopologyGraph;

#[derive(Debug, Clone)]
pub struct Cluster {
    pub nodes: Vec<Node>,
    pub waiting_queue: Vec<Job>,
    pub running_jobs: HashMap<String, Job>,
    pub finished_jobs: Vec<Job>,
    pub clock: f64,
    pub mig_reconfigs: u32,
    pub total_preemptions: u32,
    /// Jobs placed without preferred NVLink locality (M5 fallback).
    pub topology_penalties: u32,
    /// Extra simulated runtime seconds from cross-domain placement (M5 phase 2).
    pub topology_runtime_inflation: f64,
    pub topology: TopologyGraph,
    /// Max GPUs a tenant may hold across running jobs, keyed by tenant name.
    /// Tenants with no entry are unrestricted.
    pub tenant_quotas: HashMap<String, u32>,
    /// Federation site tag per node id (Zynera `zynera.ai/federated-training-site`
    /// label). Nodes with no entry are unpartitioned — eligible for jobs of
    /// any site (or no site).
    pub node_sites: HashMap<String, String>,
    /// Scheduler decisions recorded for replay / UI animation.
    pub decision_log: Vec<zyvor_janus_core::decision_log::SchedulerDecision>,
    /// Peak waiting queue length observed during the simulation.
    pub queue_max_length: usize,
    /// Gang jobs requeued after preemption that need a new timeout event.
    pub gang_timeout_rearm_ids: Vec<String>,
}

impl Cluster {
    pub fn new(nodes: Vec<Node>) -> Self {
        Self {
            nodes,
            waiting_queue: Vec::new(),
            running_jobs: HashMap::new(),
            finished_jobs: Vec::new(),
            clock: 0.0,
            mig_reconfigs: 0,
            total_preemptions: 0,
            topology_penalties: 0,
            topology_runtime_inflation: 0.0,
            topology: TopologyGraph::default(),
            tenant_quotas: HashMap::new(),
            node_sites: HashMap::new(),
            decision_log: Vec::new(),
            queue_max_length: 0,
            gang_timeout_rearm_ids: Vec::new(),
        }
    }

    pub fn note_gang_timeout_rearm(&mut self, job_id: impl Into<String>) {
        self.gang_timeout_rearm_ids.push(job_id.into());
    }

    pub fn record_decision(&mut self, decision: zyvor_janus_core::decision_log::SchedulerDecision) {
        self.decision_log.push(decision);
    }

    /// GPUs currently held by `tenant` across its running jobs.
    pub fn tenant_gpu_usage(&self, tenant: &str) -> u32 {
        self.running_jobs
            .values()
            .filter(|j| j.tenant.as_deref() == Some(tenant))
            .map(|j| j.gpu_count)
            .sum()
    }

    /// Federation site tag for `node_id`, or `None` if the node is unpartitioned.
    pub fn node_site(&self, node_id: &str) -> Option<&str> {
        self.node_sites.get(node_id).map(|s| s.as_str())
    }

    pub fn all_gpus(&self) -> impl Iterator<Item = &Gpu> {
        self.nodes.iter().flat_map(|n| n.gpus.iter())
    }

    pub fn all_gpus_mut(&mut self) -> impl Iterator<Item = &mut Gpu> {
        self.nodes.iter_mut().flat_map(|n| n.gpus.iter_mut())
    }

    pub fn gpu_count(&self) -> usize {
        self.nodes.iter().map(|n| n.gpus.len()).sum()
    }

    pub fn free_gpu_count(&self) -> usize {
        self.all_gpus().filter(|g| g.is_whole_gpu_free()).count()
    }

    pub fn enqueue_job(&mut self, mut job: Job) {
        if job.waiting_since.is_none() {
            job.waiting_since = Some(self.clock.max(job.arrival_time));
        }
        self.waiting_queue.push(job);
        self.queue_max_length = self.queue_max_length.max(self.waiting_queue.len());
    }

    pub fn start_job(&mut self, mut job: Job, placement_resource_ids: &[String], start_time: f64) {
        job.account_wait_until(start_time);
        if job.gang_enabled {
            job.gang_deadline = None;
            job.gang_timeout_generation += 1;
        }
        for resource_id in placement_resource_ids {
            self.mark_resource_busy(resource_id, &job.id);
        }

        job.state = JobState::Running;
        job.start_time = Some(start_time);
        job.assigned_gpus = placement_resource_ids.to_vec();
        job.assigned_nodes = placement_resource_ids
            .iter()
            .filter_map(|id| self.gpu(id).map(|g| g.node_id.clone()))
            .collect();
        self.running_jobs.insert(job.id.clone(), job);
    }

    pub fn finish_job(&mut self, job_id: &str, finish_time: f64) -> Option<Job> {
        let mut job = self.running_jobs.remove(job_id)?;
        if let Some(start) = job.start_time {
            job.record_gpu_segment(start, finish_time);
        }
        for resource_id in &job.assigned_gpus {
            self.mark_resource_free(resource_id);
        }
        job.state = JobState::Finished;
        job.finish_time = Some(finish_time);
        self.finished_jobs.push(job.clone());
        Some(job)
    }

    /// Remove a waiting gang job that exceeded its scheduling timeout.
    pub fn fail_waiting_job(&mut self, job_id: &str, at_time: f64) -> Option<Job> {
        let idx = self.waiting_queue.iter().position(|j| j.id == job_id)?;
        let mut job = self.waiting_queue.remove(idx);
        if let Some(since) = job.waiting_since.take() {
            job.cumulative_wait_secs += (at_time - since).max(0.0);
        }
        job.state = JobState::Failed;
        job.finish_time = Some(at_time);
        job.gang_deadline = None;
        self.finished_jobs.push(job.clone());
        Some(job)
    }

    /// Remove a running job and free its GPUs, without finishing it.
    /// Returns the job exactly as it was running (unmodified) so the
    /// caller can either restore it via `resume_evicted_job` (if freeing
    /// it didn't actually help) or requeue it via
    /// `Job::requeue_after_preemption`.
    pub fn evict_job(&mut self, job_id: &str) -> Option<Job> {
        let job = self.running_jobs.remove(job_id)?;
        for resource_id in &job.assigned_gpus {
            self.mark_resource_free(resource_id);
        }
        Some(job)
    }

    /// Undo `evict_job`: put a job back exactly as it was running.
    pub fn resume_evicted_job(&mut self, job: Job) {
        for resource_id in &job.assigned_gpus {
            self.mark_resource_busy(resource_id, &job.id);
        }
        self.running_jobs.insert(job.id.clone(), job);
    }

    pub fn mark_resource_busy(&mut self, resource_id: &str, job_id: &str) {
        if let Some(slice) = self.slice_mut(resource_id) {
            slice.running_job_id = Some(job_id.to_string());
            return;
        }
        if let Some(gpu) = self.gpu_mut(resource_id) {
            gpu.running_job_id = Some(job_id.to_string());
        }
    }

    pub fn mark_resource_free(&mut self, resource_id: &str) {
        if let Some(slice) = self.slice_mut(resource_id) {
            slice.running_job_id = None;
            return;
        }
        if let Some(gpu) = self.gpu_mut(resource_id) {
            gpu.running_job_id = None;
        }
    }

    pub fn gpu(&self, resource_id: &str) -> Option<&Gpu> {
        if self.slice(resource_id).is_some() {
            return self
                .all_gpus()
                .find(|g| g.slices.iter().any(|s| s.id == resource_id));
        }
        self.all_gpus().find(|g| g.id == resource_id)
    }

    pub fn gpu_mut(&mut self, resource_id: &str) -> Option<&mut Gpu> {
        if self.contains_slice(resource_id) {
            return None;
        }
        self.all_gpus_mut().find(|g| g.id == resource_id)
    }

    pub fn slice(&self, slice_id: &str) -> Option<&MigSlice> {
        for gpu in self.all_gpus() {
            if let Some(slice) = gpu.slices.iter().find(|s| s.id == slice_id) {
                return Some(slice);
            }
        }
        None
    }

    pub fn slice_mut(&mut self, slice_id: &str) -> Option<&mut MigSlice> {
        for gpu in self.all_gpus_mut() {
            if let Some(slice) = gpu.slices.iter_mut().find(|s| s.id == slice_id) {
                return Some(slice);
            }
        }
        None
    }

    fn contains_slice(&self, slice_id: &str) -> bool {
        self.slice(slice_id).is_some()
    }

    pub fn sort_waiting_by_arrival(&mut self) {
        self.waiting_queue.sort_by(|a, b| {
            a.arrival_time
                .partial_cmp(&b.arrival_time)
                .unwrap_or(Ordering::Equal)
        });
    }

    /// Higher `priority` first; ties broken by earlier `arrival_time`.
    pub fn sort_waiting_by_priority(&mut self) {
        self.waiting_queue.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then_with(|| {
                a.arrival_time
                    .partial_cmp(&b.arrival_time)
                    .unwrap_or(Ordering::Equal)
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Gpu;

    fn sample_node() -> Node {
        Node {
            id: "node-0".into(),
            gpus: vec![
                Gpu {
                    id: "gpu-0".into(),
                    node_id: "node-0".into(),
                    profile: "H100_80GB".into(),
                    memory_gb: 80.0,
                    nvlink_group: Some(1),
                    running_job_id: None,
                    mig_capable: false,
                    active_mig_profile: None,
                    slices: Vec::new(),
                },
                Gpu {
                    id: "gpu-1".into(),
                    node_id: "node-0".into(),
                    profile: "H100_80GB".into(),
                    memory_gb: 80.0,
                    nvlink_group: Some(1),
                    running_job_id: None,
                    mig_capable: false,
                    active_mig_profile: None,
                    slices: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn sort_waiting_by_priority_orders_high_priority_first() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let mut low = Job::new("j1", "low", 0.0, 10.0, 1);
        low.priority = 10;
        let mut high = Job::new("j2", "high", 5.0, 10.0, 1);
        high.priority = 90;
        cluster.enqueue_job(low);
        cluster.enqueue_job(high);

        cluster.sort_waiting_by_priority();

        assert_eq!(cluster.waiting_queue[0].id, "j2");
        assert_eq!(cluster.waiting_queue[1].id, "j1");
    }

    #[test]
    fn sort_waiting_by_priority_breaks_ties_by_arrival() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let mut later = Job::new("j1", "later", 5.0, 10.0, 1);
        later.priority = 50;
        let mut earlier = Job::new("j2", "earlier", 0.0, 10.0, 1);
        earlier.priority = 50;
        cluster.enqueue_job(later);
        cluster.enqueue_job(earlier);

        cluster.sort_waiting_by_priority();

        assert_eq!(cluster.waiting_queue[0].id, "j2");
        assert_eq!(cluster.waiting_queue[1].id, "j1");
    }

    #[test]
    fn tenant_gpu_usage_sums_running_jobs_for_tenant() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let mut a = Job::new("j1", "a", 0.0, 10.0, 1);
        a.tenant = Some("acme".into());
        let mut b = Job::new("j2", "b", 0.0, 10.0, 1);
        b.tenant = Some("other".into());
        cluster.start_job(a, &["gpu-0".into()], 0.0);
        cluster.start_job(b, &["gpu-1".into()], 0.0);

        assert_eq!(cluster.tenant_gpu_usage("acme"), 1);
        assert_eq!(cluster.tenant_gpu_usage("other"), 1);
        assert_eq!(cluster.tenant_gpu_usage("nobody"), 0);
    }

    #[test]
    fn evict_job_frees_gpus_without_finishing() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let job = Job::new("j1", "job-1", 0.0, 10.0, 1);
        cluster.start_job(job, &["gpu-0".into()], 0.0);
        assert_eq!(cluster.free_gpu_count(), 1);

        let evicted = cluster.evict_job("j1").expect("job was running");
        assert_eq!(evicted.id, "j1");
        assert_eq!(cluster.free_gpu_count(), 2);
        assert!(!cluster.running_jobs.contains_key("j1"));
        assert!(cluster.finished_jobs.is_empty());
    }

    #[test]
    fn resume_evicted_job_restores_running_state() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let job = Job::new("j1", "job-1", 0.0, 10.0, 1);
        cluster.start_job(job, &["gpu-0".into()], 0.0);

        let evicted = cluster.evict_job("j1").unwrap();
        cluster.resume_evicted_job(evicted);

        assert_eq!(cluster.free_gpu_count(), 1);
        assert!(cluster.running_jobs.contains_key("j1"));
        assert_eq!(
            cluster.gpu("gpu-0").unwrap().running_job_id.as_deref(),
            Some("j1")
        );
    }

    #[test]
    fn finish_job_frees_gpus() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let job = Job::new("j1", "job-1", 0.0, 10.0, 1);
        cluster.start_job(job, &["gpu-0".into()], 0.0);
        assert_eq!(cluster.free_gpu_count(), 1);
        cluster.finish_job("j1", 10.0);
        assert_eq!(cluster.free_gpu_count(), 2);
    }

    #[test]
    fn finish_job_frees_mig_slices() {
        let mut gpu = sample_node().gpus.into_iter().next().unwrap();
        gpu.mig_capable = true;
        gpu.slices.push(MigSlice {
            id: "gpu-0-mig-0".into(),
            profile: "1g.10gb".into(),
            memory_gb: 10.0,
            running_job_id: None,
        });
        let mut cluster = Cluster::new(vec![Node {
            id: "node-0".into(),
            gpus: vec![gpu],
        }]);
        let job = Job::new("j1", "job-1", 0.0, 10.0, 1);
        cluster.start_job(job, &["gpu-0-mig-0".into()], 0.0);
        assert!(cluster
            .slice("gpu-0-mig-0")
            .unwrap()
            .running_job_id
            .is_some());
        cluster.finish_job("j1", 10.0);
        assert!(cluster
            .slice("gpu-0-mig-0")
            .unwrap()
            .running_job_id
            .is_none());
    }

    #[test]
    fn enqueue_job_tracks_queue_max_length() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        assert_eq!(cluster.queue_max_length, 0);
        cluster.enqueue_job(Job::new("j1", "a", 0.0, 10.0, 1));
        cluster.enqueue_job(Job::new("j2", "b", 0.0, 10.0, 1));
        assert_eq!(cluster.queue_max_length, 2);
    }

    #[test]
    fn finish_job_records_gpu_seconds_from_final_segment() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let mut job = Job::new("j1", "a", 0.0, 100.0, 2);
        job.gpu_seconds_consumed = 40.0;
        cluster.start_job(job, &["gpu-0".into()], 10.0);
        let finished = cluster.finish_job("j1", 25.0).expect("was running");
        assert!((finished.gpu_seconds_consumed - 70.0).abs() < 1e-6);
    }

    #[test]
    fn start_job_records_cumulative_wait_and_invalidates_gang_timeout() {
        let mut cluster = Cluster::new(vec![sample_node()]);
        let mut job = Job::new("g1", "gang", 0.0, 10.0, 1);
        job.gang_enabled = true;
        job.gang_size_nodes = Some(1);
        job.gang_timeout_secs = Some(60.0);
        job.gang_deadline = Some(60.0);
        job.gang_timeout_generation = 1;
        job.enter_waiting(0.0);
        cluster.start_job(job, &["gpu-0".into()], 15.0);
        let running = cluster.running_jobs.get("g1").expect("running");
        assert_eq!(running.cumulative_wait_secs, 15.0);
        assert!(running.gang_deadline.is_none());
        assert_eq!(running.gang_timeout_generation, 2);
    }
}
