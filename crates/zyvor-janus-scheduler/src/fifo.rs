// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::Placement;

use crate::common::place_in_order;

/// First-in-first-out scheduler with **backfill**: jobs are sorted by arrival
/// time, but jobs that fail `can_place` are skipped so later, smaller jobs may
/// run first. This is not strict head-of-line blocking.
#[derive(Debug, Default, Clone)]
pub struct FifoScheduler;

impl Scheduler for FifoScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        cluster.sort_waiting_by_arrival();
        let waiting = cluster.waiting_queue.to_vec();
        place_in_order(cluster, resource_manager, waiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Job, Node};

    #[test]
    fn fifo_schedules_earlier_job_first() {
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }]);
        cluster.clock = 0.0;
        cluster.enqueue_job(Job::new("j2", "late", 5.0, 10.0, 1));
        cluster.enqueue_job(Job::new("j1", "early", 0.0, 10.0, 1));

        let mut sched = FifoScheduler;
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].job_id, "j1");
    }
}
