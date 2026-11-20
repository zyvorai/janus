// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::Placement;

use crate::common::place_in_order;

/// Highest `priority` first (ties broken by earliest arrival), all-or-nothing GPU placement.
#[derive(Debug, Default, Clone)]
pub struct PriorityScheduler;

impl Scheduler for PriorityScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        cluster.sort_waiting_by_priority();
        let waiting = cluster.waiting_queue.to_vec();
        place_in_order(cluster, resource_manager, waiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Job, Node};

    #[test]
    fn priority_schedules_higher_priority_job_first_despite_later_arrival() {
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }]);
        cluster.clock = 0.0;
        let mut low = Job::new("j1", "low-priority-early", 0.0, 10.0, 1);
        low.priority = 10;
        let mut high = Job::new("j2", "high-priority-late", 5.0, 10.0, 1);
        high.priority = 90;
        cluster.enqueue_job(low);
        cluster.enqueue_job(high);

        let mut sched = PriorityScheduler;
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].job_id, "j2");
    }

    #[test]
    fn priority_falls_back_to_arrival_order_on_tie() {
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }]);
        cluster.clock = 0.0;
        let mut later = Job::new("j1", "later", 5.0, 10.0, 1);
        later.priority = 50;
        let mut earlier = Job::new("j2", "earlier", 0.0, 10.0, 1);
        earlier.priority = 50;
        cluster.enqueue_job(later);
        cluster.enqueue_job(earlier);

        let mut sched = PriorityScheduler;
        let rm = ResourceManager::new();
        let placements = sched.schedule(&mut cluster, &rm);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].job_id, "j2");
    }
}
