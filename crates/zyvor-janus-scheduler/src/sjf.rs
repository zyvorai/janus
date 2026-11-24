// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::cmp::Ordering;

use crate::common::place_in_order;
use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::Placement;

/// Shortest-job-first (non-preemptive, oracle runtimes): the waiting job
/// with the smallest `runtime` goes first, ties by arrival then id. This is
/// the usual lower-bound-ish baseline for average JCT in the DL-scheduler
/// literature (Tiresias, Gavel); it starves long jobs under sustained load,
/// which the quality report (`wait_max`, `slowdown_max`) makes visible.
#[derive(Debug, Default, Clone)]
pub struct SjfScheduler;

impl Scheduler for SjfScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        let mut waiting = cluster.waiting_queue.to_vec();
        waiting.sort_by(|a, b| {
            a.runtime
                .partial_cmp(&b.runtime)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    a.arrival_time
                        .partial_cmp(&b.arrival_time)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| a.id.cmp(&b.id))
        });
        place_in_order(cluster, resource_manager, waiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Job, Node};

    fn one_gpu() -> Cluster {
        Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }])
    }

    #[test]
    fn shortest_runtime_wins_over_earlier_arrival() {
        let mut cluster = one_gpu();
        cluster.enqueue_job(Job::new("long", "long", 0.0, 500.0, 1));
        cluster.enqueue_job(Job::new("short", "short", 5.0, 10.0, 1));
        let p = SjfScheduler.schedule(&mut cluster, &ResourceManager::new());
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].job_id, "short");
    }

    #[test]
    fn equal_runtime_falls_back_to_arrival() {
        let mut cluster = one_gpu();
        cluster.enqueue_job(Job::new("b", "b", 5.0, 10.0, 1));
        cluster.enqueue_job(Job::new("a", "a", 1.0, 10.0, 1));
        let p = SjfScheduler.schedule(&mut cluster, &ResourceManager::new());
        assert_eq!(p[0].job_id, "a");
    }
}
