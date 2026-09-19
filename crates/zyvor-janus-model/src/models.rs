// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    #[default]
    Pending,
    Waiting,
    Running,
    Finished,
    Failed,
}

/// One continuous run on a fixed GPU set (cleared/restarted on preemption).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobRunSegment {
    pub gpu_ids: Vec<String>,
    /// Nodes for `gpu_ids` (same order; may repeat for multi-GPU on one host).
    #[serde(default)]
    pub node_ids: Vec<String>,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub arrival_time: f64,
    pub runtime: f64,
    pub gpu_count: u32,
    pub gpu_memory_gb: f64,
    pub priority: u32,
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub network_bw_gbps: Option<f64>,
    #[serde(default)]
    pub gpu_type: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    /// Federation site tag (Zynera `zynera.ai/federated-training-site` label).
    /// `None` means unpartitioned — matches any node regardless of site.
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub gang_enabled: bool,
    #[serde(default)]
    pub gang_size_nodes: Option<u32>,
    #[serde(default)]
    pub gang_timeout_secs: Option<f64>,
    #[serde(default)]
    pub mig_profile: Option<String>,
    #[serde(default)]
    pub mig_count: Option<u32>,
    #[serde(default)]
    pub state: JobState,
    #[serde(default)]
    pub start_time: Option<f64>,
    #[serde(default)]
    pub finish_time: Option<f64>,
    #[serde(default, skip_serializing)]
    pub assigned_gpus: Vec<String>,
    /// Node ids parallel to `assigned_gpus` for the current run.
    #[serde(default, skip_serializing)]
    pub assigned_nodes: Vec<String>,
    /// Closed run segments across preemptions (GPU set + start/end).
    #[serde(default)]
    pub run_segments: Vec<JobRunSegment>,
    /// Seconds of work left, set when a job has been preempted at least
    /// once. `None` means "use the full `runtime`" (never preempted).
    #[serde(default, skip_serializing)]
    pub remaining_runtime: Option<f64>,
    #[serde(default, skip_serializing)]
    pub preemption_count: u32,
    /// Bumped each time this job (re)starts running. Lets the engine tell
    /// a stale `JobComplete` (scheduled before a preemption) apart from
    /// the one that actually matches the job's current run.
    #[serde(default, skip_serializing)]
    pub run_generation: u32,
    /// Cumulative GPU-seconds consumed across all run segments (incl. preemption).
    #[serde(default, skip_serializing)]
    pub gpu_seconds_consumed: f64,
    /// Time spent in `Waiting` state only (excludes time running before eviction).
    #[serde(default, skip_serializing)]
    pub cumulative_wait_secs: f64,
    #[serde(default, skip_serializing)]
    pub time_to_first_start: Option<f64>,
    /// Clock time when this job last entered the waiting queue.
    #[serde(default, skip_serializing)]
    pub waiting_since: Option<f64>,
    /// Absolute deadline for gang scheduling; cleared once the job starts.
    #[serde(default, skip_serializing)]
    pub gang_deadline: Option<f64>,
    /// Bumped when gang timeout is (re)scheduled or invalidated on start.
    #[serde(default, skip_serializing)]
    pub gang_timeout_generation: u32,
    /// LLM model identifier for inference performance estimation.
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
    /// Simulated time-to-first-token in seconds (not queue wait).
    #[serde(default, skip_serializing)]
    pub ttft_secs: Option<f64>,
    #[serde(default, skip_serializing)]
    pub tps: Option<f64>,
    #[serde(default, skip_serializing)]
    pub itl_secs: Option<f64>,
}

impl Job {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arrival_time: f64,
        runtime: f64,
        gpu_count: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arrival_time,
            runtime,
            gpu_count,
            gpu_memory_gb: 0.0,
            priority: 0,
            tenant: None,
            network_bw_gbps: None,
            gpu_type: None,
            namespace: None,
            site: None,
            gang_enabled: false,
            gang_size_nodes: None,
            gang_timeout_secs: None,
            mig_profile: None,
            mig_count: None,
            state: JobState::Pending,
            start_time: None,
            finish_time: None,
            assigned_gpus: Vec::new(),
            assigned_nodes: Vec::new(),
            run_segments: Vec::new(),
            remaining_runtime: None,
            preemption_count: 0,
            run_generation: 0,
            gpu_seconds_consumed: 0.0,
            cumulative_wait_secs: 0.0,
            time_to_first_start: None,
            waiting_since: None,
            gang_deadline: None,
            gang_timeout_generation: 0,
            model_id: None,
            input_tokens: None,
            output_tokens: None,
            batch_size: None,
            concurrency: None,
            ttft_secs: None,
            tps: None,
            itl_secs: None,
        }
    }

    /// Legacy wait metric: last start minus arrival (overstates wait after preemption).
    pub fn wait_time(&self) -> f64 {
        match (self.start_time, self.state) {
            (Some(start), _) => (start - self.arrival_time).max(0.0),
            _ => self.cumulative_wait_secs,
        }
    }

    pub fn cumulative_wait_time(&self) -> f64 {
        self.cumulative_wait_secs
    }

    pub(crate) fn record_gpu_segment(&mut self, segment_start: f64, segment_end: f64) {
        let elapsed = (segment_end - segment_start).max(0.0);
        self.gpu_seconds_consumed += elapsed * self.gpu_count as f64;
        if !self.assigned_gpus.is_empty() {
            self.run_segments.push(JobRunSegment {
                gpu_ids: self.assigned_gpus.clone(),
                node_ids: self.assigned_nodes.clone(),
                start: segment_start,
                end: segment_end,
            });
        }
    }

    pub fn enter_waiting(&mut self, at_time: f64) {
        self.state = JobState::Waiting;
        self.waiting_since = Some(at_time);
    }

    pub fn account_wait_until(&mut self, start_time: f64) {
        if let Some(since) = self.waiting_since.take() {
            self.cumulative_wait_secs += (start_time - since).max(0.0);
        }
        if self.time_to_first_start.is_none() {
            self.time_to_first_start = Some(start_time);
        }
    }

    /// How long this job still needs to run, accounting for any prior
    /// preemption.
    pub fn duration_remaining(&self) -> f64 {
        self.remaining_runtime.unwrap_or(self.runtime)
    }

    /// Evict this job from a running state back into the waiting queue:
    /// reduce `duration_remaining()` by however long it just ran, and
    /// reset scheduling state so it can be placed again later.
    pub fn requeue_after_preemption(&mut self, at_time: f64) {
        if let Some(start) = self.start_time {
            self.record_gpu_segment(start, at_time);
        }
        let elapsed = self
            .start_time
            .map(|start| (at_time - start).max(0.0))
            .unwrap_or(0.0);
        self.remaining_runtime = Some((self.duration_remaining() - elapsed).max(0.0));
        self.start_time = None;
        self.assigned_gpus.clear();
        self.assigned_nodes.clear();
        self.preemption_count += 1;
        self.gang_timeout_generation += 1;
        if self.gang_enabled {
            if let Some(timeout) = self.gang_timeout_secs.filter(|t| *t > 0.0) {
                self.gang_deadline = Some(at_time + timeout);
            }
        }
        self.enter_waiting(at_time);
    }

    pub fn is_mig_job(&self) -> bool {
        self.mig_profile.is_some()
    }

    pub fn mig_slices_needed(&self) -> u32 {
        if self.is_mig_job() {
            self.mig_count.unwrap_or(1).max(1)
        } else {
            self.gpu_count
        }
    }

    pub fn mig_profile_name(&self) -> Option<&str> {
        self.mig_profile.as_deref()
    }
}

#[cfg(test)]
mod job_tests {
    use super::{Job, JobState};

    #[test]
    fn mig_slices_needed_defaults_to_one() {
        let mut job = Job::new("j1", "infer", 0.0, 10.0, 8);
        job.mig_profile = Some("1g.10gb".into());
        assert_eq!(job.mig_slices_needed(), 1);
    }

    #[test]
    fn mig_slices_needed_uses_count() {
        let mut job = Job::new("j1", "infer", 0.0, 10.0, 8);
        job.mig_profile = Some("1g.10gb".into());
        job.mig_count = Some(3);
        assert_eq!(job.mig_slices_needed(), 3);
    }

    #[test]
    fn non_mig_job_uses_gpu_count() {
        let job = Job::new("j1", "train", 0.0, 100.0, 4);
        assert!(!job.is_mig_job());
        assert_eq!(job.mig_slices_needed(), 4);
    }

    #[test]
    fn wait_time_is_zero_before_start() {
        let job = Job::new("j1", "a", 5.0, 10.0, 1);
        assert_eq!(job.wait_time(), 0.0);
    }

    #[test]
    fn duration_remaining_defaults_to_full_runtime() {
        let job = Job::new("j1", "a", 0.0, 100.0, 1);
        assert_eq!(job.duration_remaining(), 100.0);
    }

    #[test]
    fn requeue_after_preemption_subtracts_elapsed_time() {
        let mut job = Job::new("j1", "a", 0.0, 100.0, 1);
        job.start_time = Some(10.0);
        job.requeue_after_preemption(30.0); // ran for 20s of its 100s

        assert_eq!(job.state, JobState::Waiting);
        assert_eq!(job.duration_remaining(), 80.0);
        assert_eq!(job.preemption_count, 1);
        assert!(job.start_time.is_none());
        assert!(job.assigned_gpus.is_empty());

        // A second preemption continues from the reduced remaining time.
        job.start_time = Some(40.0);
        job.requeue_after_preemption(55.0); // ran another 15s of the 80s left

        assert_eq!(job.duration_remaining(), 65.0);
        assert_eq!(job.preemption_count, 2);
    }

    #[test]
    fn requeue_after_preemption_never_goes_negative() {
        let mut job = Job::new("j1", "a", 0.0, 10.0, 1);
        job.start_time = Some(0.0);
        job.requeue_after_preemption(50.0); // "ran" longer than its runtime
        assert_eq!(job.duration_remaining(), 0.0);
    }

    #[test]
    fn requeue_records_gpu_seconds_consumed() {
        let mut job = Job::new("j1", "a", 0.0, 100.0, 2);
        job.start_time = Some(0.0);
        job.requeue_after_preemption(25.0);
        assert_eq!(job.gpu_seconds_consumed, 50.0);
    }

    #[test]
    fn requeue_records_run_segment_with_gpu_ids() {
        let mut job = Job::new("j1", "mover", 0.0, 100.0, 1);
        job.start_time = Some(0.0);
        job.assigned_gpus = vec!["gpu-0".into()];
        job.assigned_nodes = vec!["node-0".into()];
        job.requeue_after_preemption(10.0);
        assert_eq!(job.run_segments.len(), 1);
        assert_eq!(job.run_segments[0].gpu_ids, vec!["gpu-0".to_string()]);
        assert_eq!(job.run_segments[0].node_ids, vec!["node-0".to_string()]);
        assert_eq!(job.run_segments[0].start, 0.0);
        assert_eq!(job.run_segments[0].end, 10.0);
        assert!(job.assigned_gpus.is_empty());
        assert!(job.assigned_nodes.is_empty());
    }

    #[test]
    fn account_wait_until_accumulates_queue_time_only() {
        let mut job = Job::new("j1", "a", 5.0, 100.0, 1);
        job.enter_waiting(5.0);
        job.account_wait_until(20.0);
        assert_eq!(job.cumulative_wait_secs, 15.0);
        assert_eq!(job.time_to_first_start, Some(20.0));

        job.enter_waiting(50.0);
        job.account_wait_until(80.0);
        assert_eq!(job.cumulative_wait_secs, 45.0);
    }

    #[test]
    fn requeue_after_preemption_sets_gang_deadline() {
        let mut job = Job::new("g1", "gang", 0.0, 100.0, 1);
        job.gang_enabled = true;
        job.gang_size_nodes = Some(1);
        job.gang_timeout_secs = Some(30.0);
        job.start_time = Some(0.0);
        job.requeue_after_preemption(10.0);

        assert_eq!(job.gang_deadline, Some(40.0));
        assert_eq!(job.gang_timeout_generation, 1);
        assert_eq!(job.waiting_since, Some(10.0));
    }

    #[test]
    fn cumulative_wait_time_excludes_running_segments() {
        let mut job = Job::new("j1", "a", 0.0, 100.0, 1);
        job.enter_waiting(0.0);
        job.account_wait_until(10.0);
        job.start_time = Some(10.0);
        assert_eq!(job.cumulative_wait_time(), 10.0);
        assert_eq!(job.wait_time(), 10.0);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigSlice {
    pub id: String,
    pub profile: String,
    pub memory_gb: f64,
    #[serde(default, skip_serializing)]
    pub running_job_id: Option<String>,
}

impl MigSlice {
    pub fn is_free(&self) -> bool {
        self.running_job_id.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gpu {
    pub id: String,
    pub node_id: String,
    pub profile: String,
    pub memory_gb: f64,
    #[serde(default)]
    pub nvlink_group: Option<u32>,
    #[serde(default, skip_serializing)]
    pub running_job_id: Option<String>,
    #[serde(default)]
    pub mig_capable: bool,
    #[serde(default, skip_serializing)]
    pub active_mig_profile: Option<String>,
    #[serde(default, skip_serializing)]
    pub slices: Vec<MigSlice>,
}

impl Gpu {
    pub fn new(
        id: impl Into<String>,
        node_id: impl Into<String>,
        profile: impl Into<String>,
        memory_gb: f64,
    ) -> Self {
        Self {
            id: id.into(),
            node_id: node_id.into(),
            profile: profile.into(),
            memory_gb,
            nvlink_group: None,
            running_job_id: None,
            mig_capable: false,
            active_mig_profile: None,
            slices: Vec::new(),
        }
    }

    pub fn is_free(&self) -> bool {
        if !self.slices.is_empty() {
            return false;
        }
        self.running_job_id.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub gpus: Vec<Gpu>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    pub job_id: String,
    pub gpu_ids: Vec<String>,
    pub start_time: f64,
    #[serde(default = "default_runtime_multiplier")]
    pub runtime_multiplier: f64,
}

fn default_runtime_multiplier() -> f64 {
    1.0
}
