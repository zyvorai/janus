// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use std::cmp::Ordering;

use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::Placement;

use crate::common::place_in_order;

/// Best-fit scheduler: larger jobs first, then earliest arrival. Pair with
/// `ResourceManager::with_gpu_selection(BestFit)` for tightest-node placement.
#[derive(Debug, Default, Clone)]
pub struct BestFitScheduler;

impl Scheduler for BestFitScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        let mut waiting = cluster.waiting_queue.to_vec();
        waiting.sort_by(|a, b| {
            b.gpu_count.cmp(&a.gpu_count).then_with(|| {
                a.arrival_time
                    .partial_cmp(&b.arrival_time)
                    .unwrap_or(Ordering::Equal)
            })
        });
        place_in_order(cluster, resource_manager, waiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::GpuSelectionPolicy;
    use zyvor_janus_model::models::{Gpu, Job, Node};

    #[test]
    fn best_fit_prefers_tighter_node_when_policy_enabled() {
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
        cluster.clock = 0.0;
        cluster.enqueue_job(Job::new("j1", "pair", 0.0, 10.0, 2));

        let mut sched = BestFitScheduler;
        let rm = ResourceManager::new().with_gpu_selection(GpuSelectionPolicy::BestFit);
        let placements = sched.schedule(&mut cluster, &rm);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].gpu_ids, vec!["t0", "t1"]);
    }
}
