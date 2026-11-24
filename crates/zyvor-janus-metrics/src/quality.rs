// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Scheduler-quality metrics computed after a run from the finished
//! cluster: queue-wait percentiles, bounded slowdown, Jain fairness
//! indices, and utilisation / fragmentation timelines.
//!
//! Definitions are deliberately plain and documented in
//! docs/QUALITY_METRICS.md. Everything here is a pure function of the
//! finished `Cluster`, so it is exactly as deterministic as the run.
//!
//! Honesty note: the "isolated" baseline for slowdown is the job's
//! requested runtime on an empty cluster. That is NOT the Themis
//! finish-time-fairness rho (which uses a 1/N fair share of the cluster);
//! it is the simpler bounded slowdown of Feitelson et al.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use zyvor_janus_core::stats::percentile;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::JobState;

/// Bounded-slowdown threshold (seconds): short jobs are treated as if they
/// ran this long so a 0.1 s job waiting 10 s is not a 100x outlier.
pub const SLOWDOWN_TAU_SECS: f64 = 10.0;
pub const DEFAULT_TIMELINE_BUCKETS: usize = 24;
pub const MAX_TIMELINE_BUCKETS: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TimelinePoint {
    /// Sample time (bucket midpoint, simulated seconds).
    pub t: f64,
    pub busy_gpus: u32,
    /// busy_gpus / total GPUs.
    pub utilization: f64,
    /// 1 - (largest free block on one node / total free GPUs); 0 when
    /// nothing is free. High = free GPUs are scattered across nodes.
    pub fragmentation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct QualityReport {
    /// Successfully finished jobs the statistics are computed over.
    pub jobs_considered: usize,
    /// Jobs that never started (still waiting when the run ended).
    pub jobs_never_started: usize,
    pub wait_p50: f64,
    pub wait_p90: f64,
    pub wait_p99: f64,
    pub wait_max: f64,
    /// Job completion time = finish - arrival (waiting + running + any
    /// restart losses), the headline metric of Tiresias, Gavel and Pollux.
    #[serde(default)]
    pub jct_mean: f64,
    #[serde(default)]
    pub jct_p50: f64,
    #[serde(default)]
    pub jct_p90: f64,
    #[serde(default)]
    pub jct_p99: f64,
    pub slowdown_mean: f64,
    pub slowdown_p99: f64,
    pub slowdown_max: f64,
    /// Jain index of per-job service rate (1/bounded slowdown); 1.0 = all
    /// jobs were slowed equally, 1/n = one job got everything.
    pub jain_job_service: f64,
    /// Jain index of per-tenant mean service rate (jobs without a tenant
    /// are grouped under "(none)"). 1.0 when fewer than two tenants.
    pub jain_tenant_service: f64,
    pub mean_fragmentation: f64,
    /// Finish-time fairness, simplified from Themis (NSDI 2020): per job,
    /// rho = (finish - arrival) / (runtime * N) where N is the time-averaged
    /// number of tenants with at least one job in the system during the
    /// job's lifetime (min 1). rho <= 1 means the job finished no later than
    /// an equal 1/N tenant share would predict; larger is worse. This
    /// assumes runtime scales as 1/share, which gang jobs do not, so treat it
    /// as a relative fairness signal between policies, not an absolute SLO.
    #[serde(default)]
    pub ftf_rho_mean: f64,
    #[serde(default)]
    pub ftf_rho_p90: f64,
    #[serde(default)]
    pub ftf_rho_max: f64,
    /// Worst per-tenant mean rho and the tenant that suffered it.
    #[serde(default)]
    pub ftf_worst_tenant: Option<String>,
    #[serde(default)]
    pub ftf_worst_tenant_rho: f64,
    pub utilization_timeline: Vec<TimelinePoint>,
}

/// Jain's fairness index: (sum x)^2 / (n * sum x^2). 1.0 for empty or
/// all-zero input (nothing to be unfair about).
pub fn jain_index(values: &[f64]) -> f64 {
    let xs: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .collect();
    if xs.is_empty() {
        return 1.0;
    }
    let sum: f64 = xs.iter().sum();
    let sq: f64 = xs.iter().map(|v| v * v).sum();
    if sq <= 0.0 {
        return 1.0;
    }
    (sum * sum / (xs.len() as f64 * sq)).clamp(0.0, 1.0)
}

/// Bounded slowdown: max((finish - arrival) / max(runtime, tau), 1).
pub fn bounded_slowdown(arrival: f64, finish: f64, runtime: f64) -> f64 {
    let denom = runtime.max(SLOWDOWN_TAU_SECS);
    ((finish - arrival) / denom).max(1.0)
}

impl QualityReport {
    pub fn from_cluster(cluster: &Cluster, buckets: usize) -> Self {
        let finished: Vec<_> = cluster
            .finished_jobs
            .iter()
            .filter(|j| j.state == JobState::Finished && j.finish_time.is_some())
            .collect();

        let mut waits: Vec<f64> = finished.iter().map(|j| j.cumulative_wait_time()).collect();
        let wait_max = waits.iter().copied().fold(0.0_f64, f64::max);
        let wait_p50 = percentile(&mut waits, 0.50);
        let wait_p90 = percentile(&mut waits, 0.90);
        let wait_p99 = percentile(&mut waits, 0.99);

        let mut jcts: Vec<f64> = finished
            .iter()
            .map(|j| (j.finish_time.unwrap_or(0.0) - j.arrival_time).max(0.0))
            .collect();
        let jct_mean = if jcts.is_empty() {
            0.0
        } else {
            jcts.iter().sum::<f64>() / jcts.len() as f64
        };
        let jct_p50 = percentile(&mut jcts, 0.50);
        let jct_p90 = percentile(&mut jcts, 0.90);
        let jct_p99 = percentile(&mut jcts, 0.99);

        let slowdowns: Vec<f64> = finished
            .iter()
            .map(|j| bounded_slowdown(j.arrival_time, j.finish_time.unwrap_or(0.0), j.runtime))
            .collect();
        let slowdown_mean = if slowdowns.is_empty() {
            0.0
        } else {
            slowdowns.iter().sum::<f64>() / slowdowns.len() as f64
        };
        let slowdown_max = slowdowns.iter().copied().fold(0.0_f64, f64::max);
        let slowdown_p99 = percentile(&mut slowdowns.clone(), 0.99);

        let service: Vec<f64> = slowdowns.iter().map(|s| 1.0 / s).collect();
        let mut per_tenant: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
        for (j, s) in finished.iter().zip(&service) {
            let e = per_tenant
                .entry(j.tenant.as_deref().unwrap_or("(none)"))
                .or_insert((0.0, 0));
            e.0 += s;
            e.1 += 1;
        }
        let tenant_means: Vec<f64> = per_tenant.values().map(|(s, n)| s / *n as f64).collect();

        let ftf = finish_time_fairness(&finished);

        let utilization_timeline = timeline(cluster, buckets.clamp(1, MAX_TIMELINE_BUCKETS));
        let mean_fragmentation = if utilization_timeline.is_empty() {
            0.0
        } else {
            utilization_timeline
                .iter()
                .map(|p| p.fragmentation)
                .sum::<f64>()
                / utilization_timeline.len() as f64
        };

        Self {
            jobs_considered: finished.len(),
            jobs_never_started: cluster.waiting_queue.len(),
            wait_p50,
            wait_p90,
            wait_p99,
            wait_max,
            jct_mean,
            jct_p50,
            jct_p90,
            jct_p99,
            slowdown_mean,
            slowdown_p99,
            slowdown_max,
            jain_job_service: jain_index(&service),
            jain_tenant_service: jain_index(&tenant_means),
            mean_fragmentation,
            ftf_rho_mean: ftf.mean,
            ftf_rho_p90: ftf.p90,
            ftf_rho_max: ftf.max,
            ftf_worst_tenant: ftf.worst_tenant,
            ftf_worst_tenant_rho: ftf.worst_tenant_rho,
            utilization_timeline,
        }
    }
}

#[derive(Default)]
struct Ftf {
    mean: f64,
    p90: f64,
    max: f64,
    worst_tenant: Option<String>,
    worst_tenant_rho: f64,
}

/// Piecewise-constant count of active tenants over time, as breakpoints
/// `(time, count_from_here, integral_up_to_here)`.
fn active_tenant_profile(jobs: &[&zyvor_janus_model::models::Job]) -> Vec<(f64, f64, f64)> {
    // Union of each tenant's [arrival, finish] intervals.
    let mut by_tenant: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
    for j in jobs {
        let f = j.finish_time.unwrap_or(j.arrival_time);
        by_tenant
            .entry(j.tenant.as_deref().unwrap_or("(none)"))
            .or_default()
            .push((j.arrival_time, f.max(j.arrival_time)));
    }
    let mut events: Vec<(f64, i32)> = Vec::new();
    for ivs in by_tenant.values_mut() {
        ivs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cur: Option<(f64, f64)> = None;
        for (s, e) in ivs.iter().copied() {
            match cur {
                Some((cs, ce)) if s <= ce => cur = Some((cs, ce.max(e))),
                Some((cs, ce)) => {
                    events.push((cs, 1));
                    events.push((ce, -1));
                    cur = Some((s, e));
                }
                None => cur = Some((s, e)),
            }
        }
        if let Some((cs, ce)) = cur {
            events.push((cs, 1));
            events.push((ce, -1));
        }
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut profile: Vec<(f64, f64, f64)> = Vec::new();
    let (mut count, mut integral, mut last_t) = (0i32, 0.0f64, 0.0f64);
    for (t, d) in events {
        if let Some(prev) = profile.last() {
            integral = prev.2 + prev.1 * (t - last_t);
        }
        count += d;
        last_t = t;
        profile.push((t, count as f64, integral));
    }
    profile
}

fn integral_at(profile: &[(f64, f64, f64)], t: f64) -> f64 {
    let idx = profile.partition_point(|p| p.0 <= t);
    if idx == 0 {
        return 0.0;
    }
    let (pt, c, i) = profile[idx - 1];
    i + c * (t - pt)
}

fn finish_time_fairness(finished: &[&zyvor_janus_model::models::Job]) -> Ftf {
    if finished.is_empty() {
        return Ftf::default();
    }
    let profile = active_tenant_profile(finished);
    let mut rhos: Vec<f64> = Vec::with_capacity(finished.len());
    let mut per_tenant: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
    for j in finished {
        let finish = j.finish_time.unwrap_or(j.arrival_time);
        let span = finish - j.arrival_time;
        let n = if span > 0.0 {
            ((integral_at(&profile, finish) - integral_at(&profile, j.arrival_time)) / span)
                .max(1.0)
        } else {
            1.0
        };
        let ideal = j.runtime.max(SLOWDOWN_TAU_SECS) * n;
        let rho = (span.max(0.0) / ideal).max(0.0);
        rhos.push(rho);
        let e = per_tenant
            .entry(j.tenant.as_deref().unwrap_or("(none)"))
            .or_insert((0.0, 0));
        e.0 += rho;
        e.1 += 1;
    }
    let mean = rhos.iter().sum::<f64>() / rhos.len() as f64;
    let max = rhos.iter().copied().fold(0.0_f64, f64::max);
    let p90 = percentile(&mut rhos, 0.90);
    let mut worst: Option<(&str, f64)> = None;
    for (t, (sum, n)) in &per_tenant {
        let m = sum / *n as f64;
        if worst.is_none_or(|(_, w)| m > w) {
            worst = Some((t, m));
        }
    }
    Ftf {
        mean,
        p90,
        max,
        worst_tenant: worst.map(|(t, _)| t.to_string()),
        worst_tenant_rho: worst.map(|(_, w)| w).unwrap_or(0.0),
    }
}

fn timeline(cluster: &Cluster, buckets: usize) -> Vec<TimelinePoint> {
    // resource id (GPU or MIG slice) -> (node index, flat gpu index)
    let mut lookup: HashMap<&str, (usize, usize)> = HashMap::new();
    let mut node_sizes = Vec::new();
    let mut flat = 0usize;
    for (ni, node) in cluster.nodes.iter().enumerate() {
        node_sizes.push(node.gpus.len());
        for gpu in &node.gpus {
            lookup.insert(gpu.id.as_str(), (ni, flat));
            for s in &gpu.slices {
                lookup.insert(s.id.as_str(), (ni, flat));
            }
            flat += 1;
        }
    }
    let total_gpus = flat;
    if total_gpus == 0 {
        return Vec::new();
    }

    // (start, end, resource ids) for finished, failed and still-open runs.
    let mut segs: Vec<(f64, f64, &[String])> = Vec::new();
    for j in cluster.finished_jobs.iter() {
        for s in &j.run_segments {
            segs.push((s.start, s.end, s.gpu_ids.as_slice()));
        }
    }
    for j in cluster.running_jobs.values() {
        for s in &j.run_segments {
            segs.push((s.start, s.end, s.gpu_ids.as_slice()));
        }
        if let Some(start) = j.start_time {
            segs.push((start, f64::INFINITY, j.assigned_gpus.as_slice()));
        }
    }
    segs.retain(|(s, e, _)| s.is_finite() && !e.is_nan() && e > s);

    let horizon = cluster
        .finished_jobs
        .iter()
        .filter_map(|j| j.finish_time)
        .fold(cluster.clock, f64::max);
    if !horizon.is_finite() || horizon <= 0.0 {
        return Vec::new();
    }
    let width = horizon / buckets as f64;

    // node index of each flat gpu index
    let mut node_of = vec![0usize; total_gpus];
    let mut idx = 0;
    for (ni, n) in node_sizes.iter().enumerate() {
        for _ in 0..*n {
            node_of[idx] = ni;
            idx += 1;
        }
    }

    let mut points = Vec::with_capacity(buckets);
    let mut busy = vec![false; total_gpus];
    for b in 0..buckets {
        let t = (b as f64 + 0.5) * width;
        busy.iter_mut().for_each(|x| *x = false);
        for (s, e, ids) in &segs {
            if *s <= t && t < *e {
                for id in ids.iter() {
                    if let Some((_, fi)) = lookup.get(id.as_str()) {
                        busy[*fi] = true;
                    }
                }
            }
        }
        let busy_n = busy.iter().filter(|x| **x).count();
        let mut free_per_node = vec![0usize; node_sizes.len()];
        for (fi, is_busy) in busy.iter().enumerate() {
            if !*is_busy {
                free_per_node[node_of[fi]] += 1;
            }
        }
        let free_total: usize = free_per_node.iter().sum();
        let largest = free_per_node.iter().copied().max().unwrap_or(0);
        let fragmentation = if free_total == 0 {
            0.0
        } else {
            1.0 - largest as f64 / free_total as f64
        };
        points.push(TimelinePoint {
            t,
            busy_gpus: busy_n as u32,
            utilization: busy_n as f64 / total_gpus as f64,
            fragmentation,
        });
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Job, JobRunSegment, Node};

    fn two_node_cluster() -> Cluster {
        Cluster::new(vec![
            Node {
                id: "n0".into(),
                gpus: vec![
                    Gpu::new("g0", "n0", "H100", 80.0),
                    Gpu::new("g1", "n0", "H100", 80.0),
                ],
            },
            Node {
                id: "n1".into(),
                gpus: vec![
                    Gpu::new("g2", "n1", "H100", 80.0),
                    Gpu::new("g3", "n1", "H100", 80.0),
                ],
            },
        ])
    }

    fn finished(
        id: &str,
        tenant: &str,
        arrival: f64,
        runtime: f64,
        finish: f64,
        wait: f64,
        gpus: &[&str],
    ) -> Job {
        let mut j = Job::new(id, id, arrival, runtime, gpus.len() as u32);
        j.tenant = Some(tenant.into());
        j.state = JobState::Finished;
        j.finish_time = Some(finish);
        j.cumulative_wait_secs = wait;
        j.run_segments.push(JobRunSegment {
            gpu_ids: gpus.iter().map(|s| s.to_string()).collect(),
            node_ids: vec![],
            start: finish - runtime,
            end: finish,
        });
        j
    }

    #[test]
    fn ftf_alone_is_one_and_contention_raises_it() {
        // One tenant: N=1, job ran immediately -> rho == 1.
        let mut c = two_node_cluster();
        c.finished_jobs
            .push(finished("a", "t1", 0.0, 100.0, 100.0, 0.0, &["g0"]));
        c.clock = 100.0;
        let q = QualityReport::from_cluster(&c, 4);
        assert!((q.ftf_rho_max - 1.0).abs() < 1e-9);

        // Two tenants overlapping the whole time: N=2 for both. Job a ran
        // immediately (rho = 100/(100*2) = 0.5); job b waited 200s
        // (rho = 300/(100*2) = 1.5).
        let mut c = two_node_cluster();
        c.finished_jobs
            .push(finished("a", "t1", 0.0, 100.0, 100.0, 0.0, &["g0"]));
        c.finished_jobs
            .push(finished("b", "t2", 0.0, 100.0, 300.0, 200.0, &["g1"]));
        c.clock = 300.0;
        let q = QualityReport::from_cluster(&c, 4);
        // t1 is active for [0,100], t2 for [0,300]: N is 2 over a's life but
        // (100*2 + 200*1)/300 = 1.333 over b's.
        assert!((q.ftf_rho_max - 300.0 / (100.0 * (400.0 / 300.0))).abs() < 1e-9);
        assert_eq!(q.ftf_worst_tenant.as_deref(), Some("t2"));
        assert!(q.ftf_rho_mean < q.ftf_rho_max);
    }

    #[test]
    fn ftf_is_zero_for_empty_and_finite_for_degenerate_jobs() {
        let q = QualityReport::from_cluster(&Cluster::new(vec![]), 4);
        assert_eq!(q.ftf_rho_max, 0.0);
        assert!(q.ftf_worst_tenant.is_none());
        let mut c = two_node_cluster();
        // zero-length job (finish == arrival)
        let mut z = finished("z", "t", 5.0, 0.0, 5.0, 0.0, &["g0"]);
        z.run_segments.clear();
        c.finished_jobs.push(z);
        c.clock = 5.0;
        let q = QualityReport::from_cluster(&c, 4);
        assert!(q.ftf_rho_max.is_finite());
    }

    #[test]
    fn jain_known_values() {
        assert!((jain_index(&[1.0, 1.0, 1.0, 1.0]) - 1.0).abs() < 1e-12);
        assert!((jain_index(&[1.0, 0.0, 0.0, 0.0]) - 0.25).abs() < 1e-12);
        assert_eq!(jain_index(&[]), 1.0);
        assert_eq!(jain_index(&[0.0, 0.0]), 1.0);
        // hostile values are ignored, not propagated
        assert!(jain_index(&[f64::NAN, 1.0, -3.0, f64::INFINITY]).is_finite());
    }

    #[test]
    fn bounded_slowdown_is_bounded_and_at_least_one() {
        assert_eq!(bounded_slowdown(0.0, 5.0, 0.1), 1.0);
        assert!((bounded_slowdown(0.0, 100.0, 50.0) - 2.0).abs() < 1e-12);
        assert_eq!(bounded_slowdown(10.0, 5.0, 1.0), 1.0);
        assert!(bounded_slowdown(0.0, f64::NAN, 1.0) >= 1.0);
    }

    #[test]
    fn report_on_empty_cluster_is_all_zero_and_finite() {
        let q = QualityReport::from_cluster(&Cluster::new(vec![]), 8);
        assert_eq!(q.jobs_considered, 0);
        assert!(q.utilization_timeline.is_empty());
        assert_eq!(q.jain_job_service, 1.0);
    }

    #[test]
    fn percentiles_slowdown_and_tenant_fairness() {
        let mut c = two_node_cluster();
        c.finished_jobs
            .push(finished("a", "t1", 0.0, 100.0, 100.0, 0.0, &["g0"]));
        c.finished_jobs
            .push(finished("b", "t2", 0.0, 100.0, 300.0, 200.0, &["g1"]));
        c.clock = 300.0;
        let q = QualityReport::from_cluster(&c, 6);
        assert_eq!(q.jobs_considered, 2);
        assert_eq!(q.wait_max, 200.0);
        assert!((q.slowdown_max - 3.0).abs() < 1e-9);
        // service rates 1.0 and 1/3 -> Jain = (4/3)^2 / (2*(1+1/9)) = 0.8
        assert!((q.jain_tenant_service - 0.8).abs() < 1e-9);
        assert_eq!(q.utilization_timeline.len(), 6);
    }

    #[test]
    fn fragmentation_detects_scattered_free_gpus() {
        let mut c = two_node_cluster();
        // g0 and g2 busy for the whole run: one free GPU on each node.
        c.finished_jobs
            .push(finished("a", "t", 0.0, 100.0, 100.0, 0.0, &["g0", "g2"]));
        c.clock = 100.0;
        let q = QualityReport::from_cluster(&c, 4);
        for p in &q.utilization_timeline {
            assert_eq!(p.busy_gpus, 2);
            assert!((p.fragmentation - 0.5).abs() < 1e-12);
        }
        assert!((q.mean_fragmentation - 0.5).abs() < 1e-12);
    }

    #[test]
    fn bucket_count_is_clamped() {
        let mut c = two_node_cluster();
        c.finished_jobs
            .push(finished("a", "t", 0.0, 10.0, 10.0, 0.0, &["g0"]));
        c.clock = 10.0;
        assert_eq!(
            QualityReport::from_cluster(&c, 0)
                .utilization_timeline
                .len(),
            1
        );
        assert_eq!(
            QualityReport::from_cluster(&c, usize::MAX)
                .utilization_timeline
                .len(),
            MAX_TIMELINE_BUCKETS
        );
    }

    #[test]
    fn deterministic_for_same_cluster() {
        let mut c = two_node_cluster();
        c.finished_jobs
            .push(finished("a", "x", 0.0, 10.0, 12.0, 2.0, &["g0"]));
        c.finished_jobs
            .push(finished("b", "y", 1.0, 10.0, 30.0, 9.0, &["g1"]));
        c.clock = 30.0;
        assert_eq!(
            QualityReport::from_cluster(&c, 12),
            QualityReport::from_cluster(&c, 12)
        );
    }
}
