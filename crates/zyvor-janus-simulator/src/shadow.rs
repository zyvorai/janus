// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use zyvor_janus_metrics::SimulationMetrics;
use zyvor_janus_model::cluster::Cluster;

use crate::engine::{StepOutcome, SteppableSimulation};
use crate::snapshot::ClusterSnapshot;

/// Which of the two engines a [`ShadowStep`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShadowSide {
    Primary,
    Shadow,
}

/// One [`StepOutcome`] from either engine, tagged with which side produced
/// it -- what [`ShadowRun::step`] returns.
#[derive(serde::Serialize)]
pub struct ShadowStep {
    pub side: ShadowSide,
    pub outcome: StepOutcome,
}

/// Drives a production engine and a shadow-scheduler engine over the same
/// job arrivals, stepping whichever has the earlier next event time so a
/// caller observes both schedulers' decisions in a single time-ordered
/// stream instead of running one to completion before the other starts.
pub struct ShadowRun {
    primary: Box<dyn SteppableSimulation>,
    shadow: Box<dyn SteppableSimulation>,
    jobs_total: usize,
}

impl ShadowRun {
    pub fn new(
        primary: Box<dyn SteppableSimulation>,
        shadow: Box<dyn SteppableSimulation>,
        jobs_total: usize,
    ) -> Self {
        Self {
            primary,
            shadow,
            jobs_total,
        }
    }

    /// Steps whichever engine's next event happens first, ties broken in
    /// favor of `Primary` so its schedule is authoritative when timestamps
    /// coincide exactly. Returns `None` once both engines are done.
    pub fn step(&mut self) -> Option<ShadowStep> {
        let side = match (
            self.primary.peek_next_event_time(),
            self.shadow.peek_next_event_time(),
        ) {
            (None, None) => return None,
            (Some(_), None) => ShadowSide::Primary,
            (None, Some(_)) => ShadowSide::Shadow,
            (Some(p), Some(s)) if p <= s => ShadowSide::Primary,
            (Some(_), Some(_)) => ShadowSide::Shadow,
        };
        let outcome = match side {
            ShadowSide::Primary => self.primary.step_once(),
            ShadowSide::Shadow => self.shadow.step_once(),
        }?;
        Some(ShadowStep { side, outcome })
    }

    pub fn is_done(&self) -> bool {
        self.primary.is_done() && self.shadow.is_done()
    }

    /// Steps both engines to completion, collecting every step in time
    /// order across both sides.
    pub fn run_to_completion(&mut self) -> Vec<ShadowStep> {
        let mut steps = Vec::new();
        while let Some(step) = self.step() {
            steps.push(step);
        }
        steps
    }

    /// Metrics for `(primary, shadow)` computed from whatever has completed
    /// so far -- valid mid-run as well as after [`Self::is_done`].
    pub fn metrics(&self) -> (SimulationMetrics, SimulationMetrics) {
        (
            self.primary.snapshot_metrics(self.jobs_total),
            self.shadow.snapshot_metrics(self.jobs_total),
        )
    }

    pub fn primary_cluster(&self) -> &Cluster {
        self.primary.cluster()
    }

    pub fn shadow_cluster(&self) -> &Cluster {
        self.shadow.cluster()
    }

    /// Drains and returns `(primary, shadow)` replay snapshots captured so
    /// far -- only populated if both engines were built
    /// `.with_replay_capture()` before being boxed.
    pub fn take_replay_snapshots(&mut self) -> (Vec<ClusterSnapshot>, Vec<ClusterSnapshot>) {
        (
            self.primary.take_replay_snapshots(),
            self.shadow.take_replay_snapshots(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::SimulationEngine;
    use zyvor_janus_model::cluster::Cluster;
    use zyvor_janus_model::models::{Gpu, Job, Node};
    use zyvor_janus_scheduler::{FifoScheduler, PriorityScheduler};

    fn cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }])
    }

    fn jobs() -> Vec<Job> {
        vec![
            Job::new("a", "tenant", 0.0, 5.0, 1),
            Job::new("b", "tenant", 0.0, 3.0, 1),
        ]
    }

    #[test]
    fn steps_are_time_ordered_across_both_engines() {
        let mut primary = SimulationEngine::new(cluster(), FifoScheduler);
        primary.submit_jobs(jobs());
        let mut shadow = SimulationEngine::new(cluster(), PriorityScheduler);
        shadow.submit_jobs(jobs());

        let mut run = ShadowRun::new(Box::new(primary), Box::new(shadow), 2);
        let steps = run.run_to_completion();

        assert!(!steps.is_empty());
        assert!(run.is_done());
        for pair in steps.windows(2) {
            assert!(pair[0].outcome.event_time <= pair[1].outcome.event_time);
        }
        assert!(steps.iter().any(|s| s.side == ShadowSide::Primary));
        assert!(steps.iter().any(|s| s.side == ShadowSide::Shadow));
    }

    #[test]
    fn metrics_available_before_and_after_completion() {
        let mut primary = SimulationEngine::new(cluster(), FifoScheduler);
        primary.submit_jobs(jobs());
        let mut shadow = SimulationEngine::new(cluster(), PriorityScheduler);
        shadow.submit_jobs(jobs());

        let mut run = ShadowRun::new(Box::new(primary), Box::new(shadow), 2);
        let (before_primary, before_shadow) = run.metrics();
        assert_eq!(before_primary.makespan, 0.0);
        assert_eq!(before_shadow.makespan, 0.0);

        run.run_to_completion();
        let (after_primary, after_shadow) = run.metrics();
        assert!(after_primary.makespan > 0.0);
        assert!(after_shadow.makespan > 0.0);
    }

    #[test]
    fn empty_engines_step_to_none_immediately() {
        let primary = SimulationEngine::new(cluster(), FifoScheduler);
        let shadow = SimulationEngine::new(cluster(), FifoScheduler);
        let mut run = ShadowRun::new(Box::new(primary), Box::new(shadow), 0);
        assert!(run.is_done());
        assert!(run.step().is_none());
    }
}
