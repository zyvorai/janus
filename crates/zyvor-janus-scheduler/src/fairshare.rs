// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};

use crate::common::{release_markers, try_place};
use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, Placement};

/// Tenant fair-share (max-min over GPUs held, the single-resource case of
/// DRF as used by Volcanos proportion/DRF plugins and Gavels LAS-style
/// max-min policy). Each step serves the tenant currently holding the fewest
/// GPUs (running jobs plus placements already made this pass); within a
/// tenant, jobs go in arrival order. Jobs without a tenant form one group.
///
/// Unlike `fifo`, one tenant flooding the queue cannot starve the others.
#[derive(Debug, Default, Clone)]
pub struct FairShareScheduler;

const NO_TENANT: &str = "(none)";

impl Scheduler for FairShareScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        let mut waiting: Vec<Job> = cluster.waiting_queue.to_vec();
        waiting.sort_by(|a, b| {
            a.arrival_time
                .partial_cmp(&b.arrival_time)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut queues: BTreeMap<String, VecDeque<Job>> = BTreeMap::new();
        for job in waiting {
            let key = job.tenant.clone().unwrap_or_else(|| NO_TENANT.to_string());
            queues.entry(key).or_default().push_back(job);
        }
        let mut held: BTreeMap<String, u32> = queues
            .keys()
            .map(|t| {
                let usage = if t == NO_TENANT {
                    cluster
                        .running_jobs
                        .values()
                        .filter(|j| j.tenant.is_none())
                        .map(|j| j.gpu_count)
                        .sum()
                } else {
                    cluster.tenant_gpu_usage(t)
                };
                (t.clone(), usage)
            })
            .collect();

        let mut placements = Vec::new();
        loop {
            // Tenant with the fewest GPUs; ties by earliest waiting head, then name.
            let next = queues
                .iter()
                .filter(|(_, q)| !q.is_empty())
                .min_by(|(ta, qa), (tb, qb)| {
                    held[*ta]
                        .cmp(&held[*tb])
                        .then_with(|| {
                            qa[0]
                                .arrival_time
                                .partial_cmp(&qb[0].arrival_time)
                                .unwrap_or(Ordering::Equal)
                        })
                        .then_with(|| ta.cmp(tb))
                })
                .map(|(t, _)| t.clone());
            let Some(tenant) = next else { break };
            let job = queues
                .get_mut(&tenant)
                .and_then(|q| q.pop_front())
                .expect("non-empty queue");
            if let Some(p) = try_place(cluster, resource_manager, &job) {
                *held.get_mut(&tenant).expect("tenant tracked") += job.gpu_count;
                placements.push(p);
            }
        }
        release_markers(cluster, &placements);
        placements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Node};

    fn cluster(gpus: usize) -> Cluster {
        Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: (0..gpus)
                .map(|i| Gpu::new(format!("g{i}"), "n0", "H100", 80.0))
                .collect(),
        }])
    }

    fn job(id: &str, arrival: f64, tenant: &str) -> Job {
        let mut j = Job::new(id, id, arrival, 100.0, 1);
        j.tenant = Some(tenant.to_string());
        j
    }

    #[test]
    fn flooding_tenant_does_not_starve_a_late_small_tenant() {
        let mut c = cluster(2);
        for i in 0..4 {
            c.enqueue_job(job(&format!("a{i}"), i as f64, "a"));
        }
        c.enqueue_job(job("b0", 10.0, "b"));
        let p = FairShareScheduler.schedule(&mut c, &ResourceManager::new());
        let ids: Vec<_> = p.iter().map(|p| p.job_id.as_str()).collect();
        assert_eq!(ids, ["a0", "b0"], "fifo would give a0,a1");
    }

    #[test]
    fn running_usage_counts_against_a_tenant() {
        let mut c = cluster(3);
        let a = job("a-run", 0.0, "a");
        c.enqueue_job(a.clone());
        let started = c
            .waiting_queue
            .iter()
            .find(|j| j.id == "a-run")
            .cloned()
            .unwrap();
        c.waiting_queue.retain(|j| j.id != "a-run");
        c.start_job(started, &["g0".to_string()], 0.0);
        c.enqueue_job(job("a1", 1.0, "a"));
        c.enqueue_job(job("b0", 2.0, "b"));
        let p = FairShareScheduler.schedule(&mut c, &ResourceManager::new());
        assert_eq!(p[0].job_id, "b0");
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn single_tenant_degrades_to_fifo() {
        let mut c = cluster(1);
        c.enqueue_job(job("late", 5.0, "a"));
        c.enqueue_job(job("early", 1.0, "a"));
        let p = FairShareScheduler.schedule(&mut c, &ResourceManager::new());
        assert_eq!(p[0].job_id, "early");
    }
}
