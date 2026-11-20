// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use serde::{Deserialize, Serialize};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, JobState};

pub const DEFAULT_OBS_TOP_K: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub id: String,
    pub name: String,
    pub arrival_time: f64,
    pub runtime: f64,
    pub gpu_count: u32,
    pub priority: u32,
    pub tenant: Option<String>,
    pub state: String,
    pub wait_proxy: f64,
    pub cumulative_wait: f64,
    pub gang_enabled: bool,
    pub placeable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningJobSnapshot {
    pub id: String,
    pub name: String,
    pub gpu_count: u32,
    pub assigned_gpus: Vec<String>,
    pub priority: u32,
    pub tenant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSnapshot {
    pub id: String,
    pub node_id: String,
    pub busy: bool,
    pub utilization: f64,
    pub job_id: Option<String>,
    pub job_name: Option<String>,
    pub nvlink_group: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSnapshot {
    pub id: String,
    pub gpus: Vec<GpuSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSnapshot {
    pub clock: f64,
    pub free_gpus: u32,
    pub waiting: u32,
    pub running: u32,
    pub finished: u32,
    pub node_count: u32,
    pub gpu_count: u32,
    pub top_jobs: Vec<JobSnapshot>,
    pub queue_jobs: Vec<JobSnapshot>,
    pub running_jobs: Vec<RunningJobSnapshot>,
    pub nodes: Vec<NodeSnapshot>,
}

pub fn job_current_cumulative_wait(job: &Job, clock: f64) -> f64 {
    let mut wait = job.cumulative_wait_secs;
    if let Some(since) = job.waiting_since {
        wait += (clock - since).max(0.0);
    }
    wait
}

pub fn job_wait_proxy(job: &Job, clock: f64) -> f64 {
    job_current_cumulative_wait(job, clock)
}

fn job_state_str(state: JobState) -> String {
    match state {
        JobState::Pending => "pending",
        JobState::Waiting => "waiting",
        JobState::Running => "running",
        JobState::Finished => "finished",
        JobState::Failed => "failed",
    }
    .to_string()
}

fn job_to_snapshot(job: &Job, clock: f64, placeable: bool) -> JobSnapshot {
    JobSnapshot {
        id: job.id.clone(),
        name: job.name.clone(),
        arrival_time: job.arrival_time,
        runtime: job.runtime,
        gpu_count: job.gpu_count,
        priority: job.priority,
        tenant: job.tenant.clone(),
        state: job_state_str(job.state),
        wait_proxy: job_wait_proxy(job, clock),
        cumulative_wait: job_current_cumulative_wait(job, clock),
        gang_enabled: job.gang_enabled,
        placeable,
    }
}

impl ClusterSnapshot {
    pub fn from_cluster(cluster: &Cluster, top_k: usize, placeable_mask: &[bool]) -> Self {
        let queue_jobs: Vec<_> = cluster
            .waiting_queue
            .iter()
            .enumerate()
            .map(|(idx, job)| {
                let placeable = placeable_mask.get(idx).copied().unwrap_or(false);
                job_to_snapshot(job, cluster.clock, placeable)
            })
            .collect();

        let top_jobs = queue_jobs.iter().take(top_k).cloned().collect();

        let running_jobs: Vec<_> = cluster
            .running_jobs
            .values()
            .map(|job| RunningJobSnapshot {
                id: job.id.clone(),
                name: job.name.clone(),
                gpu_count: job.gpu_count,
                assigned_gpus: job.assigned_gpus.clone(),
                priority: job.priority,
                tenant: job.tenant.clone(),
            })
            .collect();

        let mut job_by_gpu: std::collections::HashMap<String, (&str, &str)> =
            std::collections::HashMap::new();
        for job in cluster.running_jobs.values() {
            for gpu_id in &job.assigned_gpus {
                job_by_gpu.insert(gpu_id.clone(), (job.id.as_str(), job.name.as_str()));
            }
        }

        let nodes: Vec<_> = cluster
            .nodes
            .iter()
            .map(|node| {
                let gpus = node
                    .gpus
                    .iter()
                    .map(|gpu| {
                        let busy = !gpu.is_whole_gpu_free();
                        let (job_id, job_name) = job_by_gpu
                            .get(&gpu.id)
                            .map(|(id, name)| (Some(id.to_string()), Some(name.to_string())))
                            .unwrap_or((None, None));
                        GpuSnapshot {
                            id: gpu.id.clone(),
                            node_id: node.id.clone(),
                            busy,
                            utilization: if busy { 1.0 } else { 0.0 },
                            job_id,
                            job_name,
                            nvlink_group: gpu.nvlink_group,
                        }
                    })
                    .collect();
                NodeSnapshot {
                    id: node.id.clone(),
                    gpus,
                }
            })
            .collect();

        Self {
            clock: cluster.clock,
            free_gpus: cluster.free_gpu_count() as u32,
            waiting: cluster.waiting_queue.len() as u32,
            running: cluster.running_jobs.len() as u32,
            finished: cluster.finished_jobs.len() as u32,
            node_count: cluster.nodes.len() as u32,
            gpu_count: cluster.gpu_count() as u32,
            top_jobs,
            queue_jobs,
            running_jobs,
            nodes,
        }
    }

    pub fn to_feature_vector(&self) -> Vec<f32> {
        let mut features = vec![
            self.clock as f32,
            self.free_gpus as f32,
            self.waiting as f32,
            self.running as f32,
            self.finished as f32,
        ];
        for job in &self.top_jobs {
            features.push(job.arrival_time as f32);
            features.push(job.runtime as f32);
            features.push(job.gpu_count as f32);
            features.push(job.priority as f32);
            features.push(job.cumulative_wait as f32);
            features.push(if job.gang_enabled { 1.0 } else { 0.0 });
            features.push(if job.placeable { 1.0 } else { 0.0 });
        }
        features
    }
}

pub fn obs_size(top_k: usize) -> usize {
    5 + top_k * 7
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::cluster::Cluster;
    use zyvor_janus_model::models::{Gpu, Job, Node};

    #[test]
    fn obs_size_matches_feature_vector_length() {
        let top_k = 4;
        assert_eq!(obs_size(top_k), 5 + top_k * 7);
    }

    #[test]
    fn job_current_cumulative_wait_includes_active_waiting_time() {
        let mut job = Job::new("j1", "a", 0.0, 10.0, 1);
        job.cumulative_wait_secs = 5.0;
        job.waiting_since = Some(10.0);
        assert!((job_current_cumulative_wait(&job, 25.0) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn snapshot_feature_vector_includes_gang_and_cumulative_wait() {
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }]);
        cluster.clock = 10.0;
        let mut job = Job::new("g1", "gang", 0.0, 100.0, 1);
        job.gang_enabled = true;
        job.enter_waiting(0.0);
        cluster.enqueue_job(job);

        let snap = ClusterSnapshot::from_cluster(&cluster, 1, &[true]);
        let features = snap.to_feature_vector();
        assert_eq!(features.len(), obs_size(1));
        assert_eq!(features[5 + 5], 1.0);
    }
}
