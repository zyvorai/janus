// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use crate::snapshot::{obs_size, ClusterSnapshot, DEFAULT_OBS_TOP_K};
use zyvor_janus_core::events::{Event, EventKind, EventQueue};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, JobState, Placement};
use zyvor_janus_scheduler::resource::ResourceManager;

#[derive(Debug, Clone)]
pub struct StepResult {
    pub observation: ClusterSnapshot,
    pub reward: f64,
    pub done: bool,
    pub placed: bool,
    pub invalid_action: bool,
}

/// Interactive DES session for RL: pause at scheduling decisions, apply discrete actions.
pub struct RlSession {
    template_cluster: Cluster,
    pub cluster: Cluster,
    pub resource_manager: ResourceManager,
    event_queue: EventQueue,
    pending_arrivals: Vec<Job>,
    initial_jobs: Vec<Job>,
    pub jobs_total: usize,
    pub top_k: usize,
    pub time_scale: f64,
    last_mean_wait: f64,
    done: bool,
}

impl RlSession {
    pub fn new(
        cluster: Cluster,
        resource_manager: ResourceManager,
        jobs: Vec<Job>,
        top_k: usize,
    ) -> Self {
        let jobs_total = jobs.len();
        let template_cluster = cluster.clone();
        let mut session = Self {
            template_cluster,
            cluster,
            resource_manager,
            event_queue: EventQueue::new(),
            pending_arrivals: Vec::new(),
            initial_jobs: jobs,
            jobs_total,
            top_k: top_k.max(1),
            time_scale: 1.0,
            last_mean_wait: 0.0,
            done: false,
        };
        session.reset();
        session
    }

    pub fn reset(&mut self) -> ClusterSnapshot {
        self.cluster = self.template_cluster.clone();
        self.event_queue = EventQueue::new();
        self.pending_arrivals.clear();
        self.done = false;
        self.last_mean_wait = 0.0;

        let mut jobs = self.initial_jobs.clone();
        jobs.sort_by(|a, b| {
            a.arrival_time
                .partial_cmp(&b.arrival_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for job in jobs {
            self.event_queue.push(Event {
                time: job.arrival_time,
                kind: EventKind::JobArrival,
                job_id: job.id.clone(),
                run_generation: 0,
            });
            self.pending_arrivals.push(job);
        }
        self.advance_to_decision();
        self.observe()
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn obs_size(&self) -> usize {
        obs_size(self.top_k)
    }

    pub fn action_space_n(&self) -> usize {
        self.top_k + 1
    }

    pub fn observe(&self) -> ClusterSnapshot {
        let mask = self.placeable_mask();
        ClusterSnapshot::from_cluster(&self.cluster, self.top_k, &mask)
    }

    pub fn feature_vector(&self) -> Vec<f32> {
        self.observe().to_feature_vector()
    }

    pub fn decisions(&self) -> &[zyvor_janus_core::decision_log::SchedulerDecision] {
        &self.cluster.decision_log
    }

    /// Auto-pick the first placeable waiting job, or noop to advance time.
    pub fn step_fifo(&mut self) -> StepResult {
        if self.done {
            return StepResult {
                observation: self.observe(),
                reward: 0.0,
                done: true,
                placed: false,
                invalid_action: false,
            };
        }
        let mask = self.placeable_mask();
        let action = mask.iter().position(|&p| p).unwrap_or(self.top_k);
        self.step(action)
    }

    pub fn step(&mut self, action: usize) -> StepResult {
        if self.done {
            return StepResult {
                observation: self.observe(),
                reward: 0.0,
                done: true,
                placed: false,
                invalid_action: false,
            };
        }

        let noop = action >= self.top_k;
        let mut placed = false;
        let mut invalid_action = false;

        if noop {
            self.advance_after_action(false);
        } else if action < self.cluster.waiting_queue.len() {
            placed = self.try_place_waiting_index(action);
            if !placed {
                invalid_action = true;
            }
            self.advance_after_action(placed);
        } else {
            invalid_action = true;
            self.advance_after_action(false);
        }

        let observation = self.observe();
        let mean_wait = mean_cumulative_wait(&self.cluster);
        let reward = self.last_mean_wait - mean_wait;
        self.last_mean_wait = mean_wait;

        StepResult {
            observation,
            reward: reward / self.time_scale,
            done: self.done,
            placed,
            invalid_action,
        }
    }

    fn placeable_mask(&self) -> Vec<bool> {
        self.cluster
            .waiting_queue
            .iter()
            .take(self.top_k)
            .map(|job| self.resource_manager.can_place(&self.cluster, job))
            .collect()
    }

    fn has_placeable_waiting(&self) -> bool {
        self.cluster
            .waiting_queue
            .iter()
            .any(|job| self.resource_manager.can_place(&self.cluster, job))
    }

    fn try_place_waiting_index(&mut self, index: usize) -> bool {
        if index >= self.cluster.waiting_queue.len() {
            return false;
        }
        let job = self.cluster.waiting_queue[index].clone();
        if !self.resource_manager.can_place(&self.cluster, &job) {
            return false;
        }
        let clock = self.cluster.clock;
        match self
            .resource_manager
            .allocate(&mut self.cluster, &job, clock)
        {
            Ok(placement) => {
                self.apply_placement(job, placement);
                true
            }
            Err(_) => false,
        }
    }

    fn apply_placement(&mut self, mut job: Job, placement: Placement) {
        job.run_generation += 1;
        let duration = job.duration_remaining() * placement.runtime_multiplier;
        if placement.runtime_multiplier > 1.0 {
            let base = job.duration_remaining();
            self.cluster.topology_runtime_inflation += base * (placement.runtime_multiplier - 1.0);
        }
        let run_generation = job.run_generation;
        self.cluster
            .start_job(job.clone(), &placement.gpu_ids, placement.start_time);
        self.cluster.record_decision(
            zyvor_janus_core::decision_log::SchedulerDecision::new(
                placement.start_time,
                "job_scheduled",
                format!(
                    "Scheduled '{}' on {} GPU(s)",
                    job.name,
                    placement.gpu_ids.len()
                ),
            )
            .with_job(&job.id, &job.name)
            .with_gpus(placement.gpu_ids.clone()),
        );
        self.event_queue.push(Event {
            time: placement.start_time + duration,
            kind: EventKind::JobComplete,
            job_id: job.id,
            run_generation,
        });
        let idx = self
            .cluster
            .waiting_queue
            .iter()
            .position(|j| j.id == placement.job_id)
            .expect("placed job must be waiting");
        self.cluster.waiting_queue.remove(idx);
    }

    fn advance_to_decision(&mut self) {
        self.cluster.sort_waiting_by_priority();
        loop {
            if self.at_decision_point() {
                break;
            }
            if !self.advance_time_one_event_or_unblock() {
                self.done = true;
                break;
            }
        }
        self.last_mean_wait = mean_cumulative_wait(&self.cluster);
    }

    fn advance_after_action(&mut self, placed: bool) {
        if placed && self.cluster.waiting_queue.is_empty() && !self.has_pending_work() {
            self.drain_events();
            self.done = self.cluster.finished_jobs.len() >= self.jobs_total;
            return;
        }
        if (!placed || !self.at_decision_point()) && !self.advance_time_one_event_or_unblock() {
            self.drain_events();
        }
        self.advance_to_decision();
    }

    fn at_decision_point(&self) -> bool {
        !self.cluster.waiting_queue.is_empty() && self.has_placeable_waiting()
    }

    fn has_pending_work(&self) -> bool {
        !self.pending_arrivals.is_empty() || !self.event_queue.is_empty()
    }

    fn drain_events(&mut self) {
        while let Some(event) = self.event_queue.pop() {
            self.cluster.clock = event.time;
            self.handle_event(event);
        }
        self.done = self.cluster.finished_jobs.len() >= self.jobs_total;
    }

    fn advance_time_one_event_or_unblock(&mut self) -> bool {
        if let Some(event) = self.event_queue.pop() {
            self.cluster.clock = event.time;
            self.handle_event(event);
            return true;
        }
        false
    }

    fn handle_event(&mut self, event: Event) {
        match event.kind {
            EventKind::JobArrival => {
                let idx = self
                    .pending_arrivals
                    .iter()
                    .position(|j| j.id == event.job_id)
                    .expect("arrival job must exist");
                let mut job = self.pending_arrivals.remove(idx);
                job.enter_waiting(self.cluster.clock.max(job.arrival_time));
                if job.gang_enabled {
                    if let Some(timeout) = job.gang_timeout_secs.filter(|t| *t > 0.0) {
                        job.gang_deadline = Some(job.arrival_time + timeout);
                        job.gang_timeout_generation += 1;
                        self.push_gang_timeout(&job);
                    }
                }
                self.cluster.enqueue_job(job);
            }
            EventKind::JobComplete => {
                match self.cluster.running_jobs.get(&event.job_id) {
                    Some(job) if job.run_generation == event.run_generation => {}
                    _ => return,
                }
                self.cluster.finish_job(&event.job_id, self.cluster.clock);
            }
            EventKind::GangTimeout => {
                let still_waiting = self.cluster.waiting_queue.iter().any(|j| {
                    j.id == event.job_id
                        && j.state == JobState::Waiting
                        && j.gang_timeout_generation == event.run_generation
                });
                if still_waiting {
                    self.cluster
                        .fail_waiting_job(&event.job_id, self.cluster.clock);
                }
            }
        }
        self.drain_gang_timeout_rearms();
    }

    fn push_gang_timeout(&mut self, job: &Job) {
        if !job.gang_enabled {
            return;
        }
        let Some(deadline) = job.gang_deadline else {
            return;
        };
        self.event_queue.push(Event {
            time: deadline,
            kind: EventKind::GangTimeout,
            job_id: job.id.clone(),
            run_generation: job.gang_timeout_generation,
        });
    }

    fn drain_gang_timeout_rearms(&mut self) {
        let ids = std::mem::take(&mut self.cluster.gang_timeout_rearm_ids);
        for job_id in ids {
            let snapshot = self
                .cluster
                .waiting_queue
                .iter()
                .find(|j| j.id == job_id)
                .map(|j| (j.id.clone(), j.gang_deadline, j.gang_timeout_generation));
            if let Some((id, Some(deadline), gen)) = snapshot {
                self.event_queue.push(Event {
                    time: deadline,
                    kind: EventKind::GangTimeout,
                    job_id: id,
                    run_generation: gen,
                });
            }
        }
    }
}

fn mean_cumulative_wait(cluster: &Cluster) -> f64 {
    if cluster.waiting_queue.is_empty() {
        return 0.0;
    }
    cluster
        .waiting_queue
        .iter()
        .map(|job| crate::snapshot::job_current_cumulative_wait(job, cluster.clock))
        .sum::<f64>()
        / cluster.waiting_queue.len() as f64
}

pub fn default_top_k() -> usize {
    DEFAULT_OBS_TOP_K
}
