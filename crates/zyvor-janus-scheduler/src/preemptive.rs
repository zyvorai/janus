// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, Placement};

/// A job that's already been preempted this many times becomes exempt from
/// further preemption, so a persistently low-priority job still eventually
/// runs to completion instead of being evicted forever.
const MAX_PREEMPTIONS: u32 = 3;

/// Like `PriorityScheduler`, but a waiting job that can't currently fit may
/// evict lower-priority running jobs to make room. Evicted jobs resume
/// later with whatever runtime they had left (no restart penalty) — see
/// `docs/design/m6_scheduler_features.md` for the design tradeoffs.
///
/// Only whole-GPU jobs participate in eviction (as both the job that
/// triggers it and, for now, its victims are drawn from any running job
/// regardless of MIG-ness — see tests). A MIG job that doesn't fit is left
/// waiting rather than attempting eviction.
#[derive(Debug, Clone)]
pub struct PreemptivePriorityScheduler {
    pub restart_penalty_secs: f64,
    pub quota_aware_preemption: bool,
}

impl Default for PreemptivePriorityScheduler {
    fn default() -> Self {
        Self {
            restart_penalty_secs: 0.0,
            quota_aware_preemption: true,
        }
    }
}

impl Scheduler for PreemptivePriorityScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        cluster.sort_waiting_by_priority();
        let waiting = cluster.waiting_queue.to_vec();
        let mut placements = Vec::new();

        for job in waiting {
            if try_place(cluster, resource_manager, &job, &mut placements) {
                continue;
            }
            if job.is_mig_job() {
                continue;
            }
            if attempt_preemption(cluster, resource_manager, &job, self.quota_aware_preemption) {
                try_place(cluster, resource_manager, &job, &mut placements);
            }
        }

        // The marking above only guards against double-booking the same
        // GPU to two different placements within this one schedule() call.
        // The engine applies the real, persistent marking via
        // Cluster::start_job once it commits these placements.
        for placement in &placements {
            for resource_id in &placement.gpu_ids {
                cluster.mark_resource_free(resource_id);
            }
        }

        placements
    }
}

fn try_place(
    cluster: &mut Cluster,
    resource_manager: &ResourceManager,
    job: &Job,
    placements: &mut Vec<Placement>,
) -> bool {
    if !resource_manager.can_place(cluster, job) {
        return false;
    }
    match resource_manager.allocate(cluster, job, cluster.clock) {
        Ok(placement) => {
            for resource_id in &placement.gpu_ids {
                cluster.mark_resource_busy(resource_id, &job.id);
            }
            placements.push(placement);
            true
        }
        Err(_) => false,
    }
}

/// Evict lower-priority running jobs, lowest priority first, until `job`
/// fits or there are no more viable candidates. Commits the eviction (and
/// requeues the victims) only if it actually freed enough room; otherwise
/// restores every job it evicted and leaves the cluster unchanged.
fn attempt_preemption(
    cluster: &mut Cluster,
    resource_manager: &ResourceManager,
    job: &Job,
    quota_aware: bool,
) -> bool {
    let mut candidates: Vec<String> = cluster
        .running_jobs
        .values()
        .filter(|r| {
            r.priority < job.priority
                && r.preemption_count < MAX_PREEMPTIONS
                && (!quota_aware
                    || job.tenant.is_none()
                    || r.tenant.as_deref() == job.tenant.as_deref())
        })
        .map(|r| r.id.clone())
        .collect();
    candidates.sort_by(|a, b| {
        let priority_a = cluster.running_jobs[a].priority;
        let priority_b = cluster.running_jobs[b].priority;
        priority_a.cmp(&priority_b).then_with(|| a.cmp(b))
    });

    let mut evicted = Vec::new();
    for candidate_id in candidates {
        if let Some(victim) = cluster.evict_job(&candidate_id) {
            evicted.push(victim);
        }
        if resource_manager.can_place(cluster, job) {
            let at_time = cluster.clock;
            for mut victim in evicted {
                let freed_gpus = victim.assigned_gpus.clone();
                victim.requeue_after_preemption(at_time);
                cluster.total_preemptions += 1;
                if victim.gang_enabled && victim.gang_deadline.is_some() {
                    cluster.note_gang_timeout_rearm(&victim.id);
                }
                cluster.record_decision(
                    zyvor_janus_core::decision_log::SchedulerDecision::new(
                        at_time,
                        "job_preempted",
                        format!(
                            "Preempted '{}' (priority {}) for '{}'",
                            victim.name, victim.priority, job.name
                        ),
                    )
                    .with_job(&victim.id, &victim.name)
                    .with_gpus(freed_gpus),
                );
                cluster.waiting_queue.push(victim);
                cluster.queue_max_length =
                    cluster.queue_max_length.max(cluster.waiting_queue.len());
            }
            return true;
        }
    }

    for victim in evicted {
        cluster.resume_evicted_job(victim);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Node};

    fn one_gpu_cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }])
    }

    #[test]
    fn evicts_lower_priority_running_job_to_place_higher_priority_one() {
        let mut cluster = one_gpu_cluster();
        cluster.clock = 10.0;
        let mut low = Job::new("low", "low", 0.0, 100.0, 1);
        low.priority = 10;
        cluster.start_job(low, &["g0".into()], 0.0);

        let mut high = Job::new("high", "high", 10.0, 20.0, 1);
        high.priority = 90;
        cluster.enqueue_job(high);

        let mut sched = PreemptivePriorityScheduler::default();
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].job_id, "high");

        // The evicted job is back in the waiting queue with reduced
        // remaining runtime (ran 10s of its 100s) and a bumped preemption
        // count, not silently dropped.
        let requeued = cluster
            .waiting_queue
            .iter()
            .find(|j| j.id == "low")
            .expect("low-priority job requeued");
        assert_eq!(requeued.duration_remaining(), 90.0);
        assert_eq!(requeued.preemption_count, 1);
        // schedule() only returns placements — the engine is the one that
        // commits them into running_jobs via Cluster::start_job. What
        // schedule() must guarantee is that the evicted job's GPU is
        // actually free for the new placement to use, which the returned
        // Placement (checked above) already confirms.
        assert!(!cluster.running_jobs.contains_key("low"));
    }

    #[test]
    fn does_not_evict_equal_or_higher_priority_running_jobs() {
        let mut cluster = one_gpu_cluster();
        let mut running = Job::new("running", "running", 0.0, 100.0, 1);
        running.priority = 50;
        cluster.start_job(running, &["g0".into()], 0.0);

        let mut waiting = Job::new("waiting", "waiting", 0.0, 20.0, 1);
        waiting.priority = 50; // equal priority — must not preempt
        cluster.enqueue_job(waiting);

        let mut sched = PreemptivePriorityScheduler::default();
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert!(placements.is_empty());
        assert!(cluster.running_jobs.contains_key("running"));
        assert_eq!(cluster.waiting_queue.len(), 1);
    }

    #[test]
    fn restores_evicted_jobs_when_freed_capacity_still_is_not_enough() {
        // Two low-priority jobs, each on their own GPU. The high-priority
        // job needs both GPUs, but preempting only one still isn't enough
        // — preemption must give up cleanly and leave both running jobs
        // exactly as they were.
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![
                Gpu::new("g0", "n0", "H100", 80.0),
                Gpu::new("g1", "n0", "H100", 80.0),
            ],
        }]);
        let mut low_a = Job::new("low-a", "low-a", 0.0, 100.0, 1);
        low_a.priority = 10;
        cluster.start_job(low_a, &["g0".into()], 0.0);
        let mut low_b = Job::new("low-b", "low-b", 0.0, 100.0, 1);
        low_b.priority = 20; // still below the waiting job, but MAX_PREEMPTIONS exempts it
        low_b.preemption_count = MAX_PREEMPTIONS;
        cluster.start_job(low_b, &["g1".into()], 0.0);

        let mut high = Job::new("high", "high", 0.0, 20.0, 2);
        high.priority = 90;
        cluster.enqueue_job(high);

        let mut sched = PreemptivePriorityScheduler::default();
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert!(placements.is_empty());
        assert!(cluster.running_jobs.contains_key("low-a"));
        assert!(cluster.running_jobs.contains_key("low-b"));
        assert_eq!(cluster.free_gpu_count(), 0);
        assert_eq!(cluster.waiting_queue.len(), 1);
    }

    #[test]
    fn records_job_preempted_decision() {
        let mut cluster = one_gpu_cluster();
        cluster.clock = 10.0;
        let mut low = Job::new("low", "low", 0.0, 100.0, 1);
        low.priority = 10;
        cluster.start_job(low, &["g0".into()], 0.0);

        let mut high = Job::new("high", "high", 10.0, 20.0, 1);
        high.priority = 90;
        cluster.enqueue_job(high);

        let mut sched = PreemptivePriorityScheduler::default();
        let rm = ResourceManager::new();
        sched.schedule(&mut cluster, &rm);

        assert!(cluster
            .decision_log
            .iter()
            .any(|d| d.kind == "job_preempted"));
    }

    #[test]
    fn quota_aware_preemption_does_not_evict_other_tenant() {
        let mut cluster = one_gpu_cluster();
        cluster.clock = 10.0;
        let mut victim = Job::new("victim", "victim", 0.0, 100.0, 1);
        victim.priority = 10;
        victim.tenant = Some("team-a".into());
        cluster.start_job(victim, &["g0".into()], 0.0);

        let mut high = Job::new("high", "high", 10.0, 20.0, 1);
        high.priority = 90;
        high.tenant = Some("team-b".into());
        cluster.enqueue_job(high);

        let mut sched = PreemptivePriorityScheduler {
            quota_aware_preemption: true,
            ..Default::default()
        };
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert!(placements.is_empty());
        assert!(cluster.running_jobs.contains_key("victim"));
    }

    #[test]
    fn preemption_rearms_gang_timeout_for_gang_victim() {
        let mut cluster = one_gpu_cluster();
        cluster.clock = 10.0;
        let mut gang = Job::new("gang", "gang", 0.0, 100.0, 1);
        gang.priority = 10;
        gang.gang_enabled = true;
        gang.gang_size_nodes = Some(1);
        gang.gang_timeout_secs = Some(30.0);
        cluster.start_job(gang, &["g0".into()], 0.0);

        let mut high = Job::new("high", "high", 10.0, 20.0, 1);
        high.priority = 90;
        cluster.enqueue_job(high);

        let mut sched = PreemptivePriorityScheduler::default();
        let rm = ResourceManager::new();
        sched.schedule(&mut cluster, &rm);

        assert!(cluster.gang_timeout_rearm_ids.contains(&"gang".to_string()));
        let requeued = cluster
            .waiting_queue
            .iter()
            .find(|j| j.id == "gang")
            .expect("gang requeued");
        assert_eq!(requeued.gang_deadline, Some(40.0));
    }
}
