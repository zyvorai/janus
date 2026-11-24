// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use crate::common::{release_markers, try_place};
use crate::resource::ResourceManager;
use crate::Scheduler;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, Placement};

/// EASY backfill (Lifka 1995; the default in Slurm and Maui): jobs start in
/// arrival order, but the first job that does not fit gets a *reservation*
/// at the earliest time enough GPUs free up. Later jobs may jump ahead only
/// if they cannot delay that reservation: they either finish before it, or
/// fit in the GPUs the head will not need.
///
/// Contrast with `fifo`, which skips any blocked job with no reservation, so
/// a stream of small jobs can starve a wide one.
///
/// Approximation: reservations use total free-GPU counts and the jobs' true
/// remaining runtimes (the simulator knows them; a real cluster only has user
/// estimates). Topology, node packing and quotas can still make the head
/// start later than reserved; they are not modelled in the reservation.
#[derive(Debug, Default, Clone)]
pub struct EasyBackfillScheduler;

struct Reservation {
    /// Time the head job is expected to be able to start.
    shadow: f64,
    /// GPUs free at `shadow` beyond what the head needs.
    extra: i64,
}

fn gpus_of(job: &Job) -> i64 {
    job.gpu_count.max(1) as i64
}

fn reserve(cluster: &Cluster, head: &Job, free: i64, started: &[(f64, i64)]) -> Reservation {
    let need = gpus_of(head);
    let mut releases: Vec<(f64, i64)> = cluster
        .running_jobs
        .values()
        .map(|j| {
            (
                j.start_time.unwrap_or(cluster.clock) + j.duration_remaining(),
                (j.assigned_gpus.len() as i64).max(gpus_of(j)),
            )
        })
        .chain(started.iter().copied())
        .collect();
    releases.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut avail = free;
    if avail >= need {
        // Blocked for a reason other than GPU count (quota, type, topology):
        // reserve "now" and only let jobs through that use spare GPUs.
        return Reservation {
            shadow: cluster.clock,
            extra: avail - need,
        };
    }
    for (end, g) in releases {
        avail += g;
        if avail >= need {
            return Reservation {
                shadow: end,
                extra: avail - need,
            };
        }
    }
    Reservation {
        shadow: f64::INFINITY,
        extra: 0,
    }
}

impl Scheduler for EasyBackfillScheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement> {
        cluster.sort_waiting_by_arrival();
        let waiting = cluster.waiting_queue.to_vec();
        let mut placements = Vec::new();
        let mut started: Vec<(f64, i64)> = Vec::new();
        let mut free = cluster.free_gpu_count() as i64;
        let mut reservation: Option<Reservation> = None;

        for job in waiting {
            match reservation.as_mut() {
                None => {
                    if let Some(p) = try_place(cluster, resource_manager, &job) {
                        free -= gpus_of(&job);
                        started.push((cluster.clock + job.duration_remaining(), gpus_of(&job)));
                        placements.push(p);
                    } else {
                        reservation = Some(reserve(cluster, &job, free, &started));
                    }
                }
                Some(res) => {
                    let ends_in_time = cluster.clock + job.duration_remaining() <= res.shadow;
                    let fits_extra = gpus_of(&job) <= res.extra;
                    if !(ends_in_time || fits_extra) {
                        continue;
                    }
                    if let Some(p) = try_place(cluster, resource_manager, &job) {
                        free -= gpus_of(&job);
                        if !ends_in_time {
                            res.extra -= gpus_of(&job);
                        }
                        started.push((cluster.clock + job.duration_remaining(), gpus_of(&job)));
                        placements.push(p);
                    }
                }
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

    fn cluster4() -> Cluster {
        Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: (0..4)
                .map(|i| Gpu::new(format!("g{i}"), "n0", "H100", 80.0))
                .collect(),
        }])
    }

    fn with_running_two_gpu_job(c: &mut Cluster) {
        let mut a = Job::new("a", "a", 0.0, 100.0, 2);
        a.assigned_gpus = vec!["g0".into(), "g1".into()];
        c.start_job(a, &["g0".to_string(), "g1".to_string()], 0.0);
        c.clock = 10.0;
    }

    fn ids(c: &mut Cluster) -> Vec<String> {
        EasyBackfillScheduler
            .schedule(c, &ResourceManager::new())
            .into_iter()
            .map(|p| p.job_id)
            .collect()
    }

    #[test]
    fn backfills_short_job_but_protects_head_reservation() {
        let mut c = cluster4();
        with_running_two_gpu_job(&mut c);
        // Head needs all 4 GPUs, so it must wait for `a` (done at t=100).
        c.enqueue_job(Job::new("head", "head", 1.0, 50.0, 4));
        c.enqueue_job(Job::new("short", "short", 2.0, 50.0, 1)); // ends 60 <= 100
        c.enqueue_job(Job::new("long", "long", 3.0, 500.0, 1)); // would delay head
        assert_eq!(ids(&mut c), vec!["short"]);
    }

    #[test]
    fn long_job_may_use_gpus_the_head_will_not_need() {
        let mut c = cluster4();
        with_running_two_gpu_job(&mut c);
        // Head needs 3: at t=100 there are 4 free, so 1 spare GPU.
        c.enqueue_job(Job::new("head", "head", 1.0, 50.0, 3));
        c.enqueue_job(Job::new("long1", "long1", 2.0, 500.0, 1));
        c.enqueue_job(Job::new("long2", "long2", 3.0, 500.0, 1));
        assert_eq!(ids(&mut c), vec!["long1"]);
    }

    #[test]
    fn starts_everything_when_all_fit() {
        let mut c = cluster4();
        c.enqueue_job(Job::new("x", "x", 0.0, 10.0, 2));
        c.enqueue_job(Job::new("y", "y", 1.0, 10.0, 2));
        assert_eq!(ids(&mut c).len(), 2);
    }

    #[test]
    fn unsatisfiable_head_does_not_panic_or_block_forever() {
        let mut c = cluster4();
        c.enqueue_job(Job::new("huge", "huge", 0.0, 10.0, 99));
        c.enqueue_job(Job::new("small", "small", 1.0, 10.0, 1));
        assert_eq!(ids(&mut c), vec!["small"]);
    }
}
