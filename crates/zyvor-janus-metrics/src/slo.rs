// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! SLO attainment: the fraction of jobs that met user-set limits on queue
//! wait, bounded slowdown and/or job completion time. Gavel and Pollux
//! report JCT and fairness; production schedulers (Slurm QOS, Kueue
//! admission) are judged on whether jobs start "soon enough", which is what
//! this measures. Limits are caller-supplied: there is no universal SLO.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, JobState};

use crate::quality::bounded_slowdown;

/// Limits a job must satisfy to count as meeting its SLO. Unset limits are
/// not checked; a spec with no limits is rejected by [`SloSpec::validate`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SloSpec {
    /// Max cumulative queue wait in simulated seconds.
    #[serde(default)]
    pub max_wait_secs: Option<f64>,
    /// Max bounded slowdown (>= 1).
    #[serde(default)]
    pub max_slowdown: Option<f64>,
    /// Max job completion time (finish - arrival) in simulated seconds.
    #[serde(default)]
    pub max_jct_secs: Option<f64>,
}

impl SloSpec {
    pub fn validate(&self) -> Result<(), String> {
        let limits = [
            ("max_wait_secs", self.max_wait_secs, 0.0),
            ("max_slowdown", self.max_slowdown, 1.0),
            ("max_jct_secs", self.max_jct_secs, 0.0),
        ];
        if limits.iter().all(|(_, v, _)| v.is_none()) {
            return Err(
                "slo needs at least one of max_wait_secs, max_slowdown, max_jct_secs".into(),
            );
        }
        for (name, v, min) in limits {
            if let Some(v) = v {
                if !v.is_finite() || v < min {
                    return Err(format!("slo.{name} must be a finite number >= {min}"));
                }
            }
        }
        Ok(())
    }

    fn met_by(&self, job: &Job) -> bool {
        if job.state != JobState::Finished {
            return false;
        }
        let Some(finish) = job.finish_time else {
            return false;
        };
        self.max_wait_secs
            .is_none_or(|m| job.cumulative_wait_time() <= m)
            && self
                .max_slowdown
                .is_none_or(|m| bounded_slowdown(job.arrival_time, finish, job.runtime) <= m)
            && self
                .max_jct_secs
                .is_none_or(|m| finish - job.arrival_time <= m)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TenantSlo {
    pub jobs: usize,
    pub met: usize,
    pub attainment: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SloReport {
    pub spec: SloSpec,
    /// Every submitted job that reached the cluster, finished or not.
    pub jobs_total: usize,
    pub jobs_met: usize,
    /// jobs_met / jobs_total (1.0 for an empty run). Jobs that failed, were
    /// still waiting, or were still running at the end count as misses.
    pub attainment: f64,
    /// Worst-served tenant, by attainment (ties by name).
    pub worst_tenant: Option<String>,
    pub per_tenant: BTreeMap<String, TenantSlo>,
}

impl SloReport {
    pub fn from_cluster(cluster: &Cluster, spec: &SloSpec) -> Self {
        let mut per_tenant: BTreeMap<String, TenantSlo> = BTreeMap::new();
        let all = cluster
            .finished_jobs
            .iter()
            .chain(cluster.waiting_queue.iter())
            .chain(cluster.running_jobs.values());
        let (mut total, mut met) = (0usize, 0usize);
        for job in all {
            let ok = spec.met_by(job);
            total += 1;
            met += usize::from(ok);
            let e = per_tenant
                .entry(job.tenant.clone().unwrap_or_else(|| "(none)".into()))
                .or_default();
            e.jobs += 1;
            e.met += usize::from(ok);
        }
        for t in per_tenant.values_mut() {
            t.attainment = t.met as f64 / t.jobs as f64;
        }
        let worst_tenant = per_tenant
            .iter()
            .min_by(|a, b| a.1.attainment.total_cmp(&b.1.attainment))
            .map(|(k, _)| k.clone());
        Self {
            spec: spec.clone(),
            jobs_total: total,
            jobs_met: met,
            attainment: if total == 0 {
                1.0
            } else {
                met as f64 / total as f64
            },
            worst_tenant,
            per_tenant,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Node};

    fn run_two_jobs() -> Cluster {
        let mut c = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }]);
        let mut a = Job::new("a", "a", 0.0, 10.0, 1);
        a.tenant = Some("t1".into());
        let mut b = Job::new("b", "b", 0.0, 10.0, 1);
        b.tenant = Some("t2".into());
        c.enqueue_job(a.clone());
        c.enqueue_job(b.clone());
        // a runs 0..10 with no wait; b waits 10 s and runs 10..20.
        c.clock = 0.0;
        let ja = c.waiting_queue.remove(0);
        c.start_job(ja, &["g0".to_string()], 0.0);
        c.finish_job("a", 10.0);
        c.clock = 10.0;
        let jb = c.waiting_queue.remove(0);
        c.start_job(jb, &["g0".to_string()], 10.0);
        c.finish_job("b", 20.0);
        c
    }

    #[test]
    fn wait_limit_splits_the_two_tenants() {
        let c = run_two_jobs();
        let r = SloReport::from_cluster(
            &c,
            &SloSpec {
                max_wait_secs: Some(5.0),
                ..Default::default()
            },
        );
        assert_eq!(r.jobs_total, 2);
        assert_eq!(r.jobs_met, 1);
        assert_eq!(r.attainment, 0.5);
        assert_eq!(r.per_tenant["t1"].attainment, 1.0);
        assert_eq!(r.per_tenant["t2"].attainment, 0.0);
        assert_eq!(r.worst_tenant.as_deref(), Some("t2"));
    }

    #[test]
    fn jct_and_slowdown_limits_are_anded() {
        let c = run_two_jobs();
        let loose = SloSpec {
            max_jct_secs: Some(20.0),
            max_slowdown: Some(2.0),
            ..Default::default()
        };
        assert_eq!(SloReport::from_cluster(&c, &loose).attainment, 1.0);
        let tight = SloSpec {
            max_jct_secs: Some(20.0),
            max_slowdown: Some(1.5),
            ..Default::default()
        };
        // b: jct 20 ok, slowdown 20/10 = 2 > 1.5.
        assert_eq!(SloReport::from_cluster(&c, &tight).jobs_met, 1);
    }

    #[test]
    fn unfinished_jobs_count_as_misses() {
        let mut c = run_two_jobs();
        c.enqueue_job(Job::new("never", "never", 0.0, 5.0, 1));
        let r = SloReport::from_cluster(
            &c,
            &SloSpec {
                max_jct_secs: Some(1000.0),
                ..Default::default()
            },
        );
        assert_eq!(r.jobs_total, 3);
        assert_eq!(r.jobs_met, 2);
    }

    #[test]
    fn spec_validation() {
        assert!(SloSpec::default().validate().is_err());
        assert!(SloSpec {
            max_slowdown: Some(0.5),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(SloSpec {
            max_wait_secs: Some(f64::NAN),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(SloSpec {
            max_wait_secs: Some(0.0),
            ..Default::default()
        }
        .validate()
        .is_ok());
    }
}
