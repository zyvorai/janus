// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Per-tenant GPU chargeback (FinOps showback) and idle-capacity cost.
//!
//! A pure function of the finished cluster plus a flat `$/GPU-hour` rate.
//! Used GPU-seconds come from each job's accounting (`gpu_seconds_consumed`,
//! which includes work later lost to preemption). Capacity is
//! `total GPUs x horizon`, where the horizon is the last finish time (or the
//! clock if later). The rate is one flat number, so this ignores per-GPU-type
//! pricing, reservations, and spot discounts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zyvor_janus_cost::CostModel;
use zyvor_janus_model::cluster::Cluster;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TenantCharge {
    pub tenant: String,
    pub jobs: usize,
    pub gpu_hours: f64,
    pub cost_usd: f64,
    /// Fraction of all consumed GPU-hours.
    pub share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ChargebackReport {
    pub gpu_hour_usd: f64,
    pub horizon_secs: f64,
    pub capacity_gpu_hours: f64,
    pub used_gpu_hours: f64,
    pub idle_gpu_hours: f64,
    pub used_cost_usd: f64,
    /// Cost of capacity nobody used over the horizon.
    pub idle_cost_usd: f64,
    /// GPU-hours of work lost to injected crashes (already inside used).
    pub wasted_gpu_hours: f64,
    pub wasted_cost_usd: f64,
    /// Sorted by cost descending, then tenant name. Jobs without a tenant
    /// are reported as "(none)".
    pub tenants: Vec<TenantCharge>,
}

impl ChargebackReport {
    pub fn from_cluster(cluster: &Cluster, cost: &CostModel) -> Self {
        let rate = if cost.gpu_hour_usd.is_finite() && cost.gpu_hour_usd >= 0.0 {
            cost.gpu_hour_usd
        } else {
            0.0
        };
        let mut per: BTreeMap<String, (usize, f64)> = BTreeMap::new();
        let mut used_secs = 0.0;
        let all = cluster
            .finished_jobs
            .iter()
            .chain(cluster.running_jobs.values());
        for j in all {
            let s = j.gpu_seconds_consumed;
            if !s.is_finite() || s <= 0.0 {
                continue;
            }
            used_secs += s;
            let e = per
                .entry(j.tenant.clone().unwrap_or_else(|| "(none)".into()))
                .or_insert((0, 0.0));
            e.0 += 1;
            e.1 += s;
        }
        let horizon = cluster
            .finished_jobs
            .iter()
            .filter_map(|j| j.finish_time)
            .fold(cluster.clock, f64::max)
            .max(0.0);
        let capacity_secs = cluster.gpu_count() as f64 * horizon;
        let idle_secs = (capacity_secs - used_secs).max(0.0);
        let wasted_secs = cluster.lost_gpu_seconds.max(0.0);

        let mut tenants: Vec<TenantCharge> = per
            .into_iter()
            .map(|(tenant, (jobs, secs))| TenantCharge {
                tenant,
                jobs,
                gpu_hours: secs / 3600.0,
                cost_usd: secs / 3600.0 * rate,
                share: if used_secs > 0.0 {
                    secs / used_secs
                } else {
                    0.0
                },
            })
            .collect();
        tenants.sort_by(|a, b| {
            b.cost_usd
                .total_cmp(&a.cost_usd)
                .then_with(|| a.tenant.cmp(&b.tenant))
        });

        Self {
            gpu_hour_usd: rate,
            horizon_secs: horizon,
            capacity_gpu_hours: capacity_secs / 3600.0,
            used_gpu_hours: used_secs / 3600.0,
            idle_gpu_hours: idle_secs / 3600.0,
            used_cost_usd: used_secs / 3600.0 * rate,
            idle_cost_usd: idle_secs / 3600.0 * rate,
            wasted_gpu_hours: wasted_secs / 3600.0,
            wasted_cost_usd: wasted_secs / 3600.0 * rate,
            tenants,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Job, JobState, Node};

    fn cluster() -> Cluster {
        Cluster::new(vec![Node {
            id: "n".into(),
            gpus: vec![
                Gpu::new("g0", "n", "H100", 80.0),
                Gpu::new("g1", "n", "H100", 80.0),
            ],
        }])
    }

    fn done(id: &str, tenant: Option<&str>, gpus: u32, secs: f64, finish: f64) -> Job {
        let mut j = Job::new(id, id, 0.0, secs, gpus);
        j.tenant = tenant.map(String::from);
        j.state = JobState::Finished;
        j.finish_time = Some(finish);
        j.gpu_seconds_consumed = secs * gpus as f64;
        j
    }

    #[test]
    fn splits_cost_by_tenant_and_prices_idle_capacity() {
        let mut c = cluster();
        c.finished_jobs
            .push(done("a", Some("alpha"), 1, 3600.0, 3600.0));
        c.finished_jobs
            .push(done("b", Some("beta"), 1, 1800.0, 3600.0));
        c.finished_jobs.push(done("c", None, 1, 1800.0, 3600.0));
        c.clock = 3600.0;
        let r = ChargebackReport::from_cluster(&c, &CostModel { gpu_hour_usd: 10.0 });
        // 2 GPUs x 1h = 2 capacity-hours; used 1 + 0.5 + 0.5 = 2.
        assert!((r.capacity_gpu_hours - 2.0).abs() < 1e-9);
        assert!((r.used_gpu_hours - 2.0).abs() < 1e-9);
        assert!(r.idle_gpu_hours.abs() < 1e-9);
        assert_eq!(r.tenants[0].tenant, "alpha");
        assert!((r.tenants[0].cost_usd - 10.0).abs() < 1e-9);
        assert!((r.tenants[0].share - 0.5).abs() < 1e-9);
        // ties sort by name: "(none)" < "beta"
        assert_eq!(r.tenants[1].tenant, "(none)");
        assert!((r.used_cost_usd - 20.0).abs() < 1e-9);
    }

    #[test]
    fn idle_cost_and_waste_and_hostile_rate() {
        let mut c = cluster();
        c.finished_jobs
            .push(done("a", Some("alpha"), 1, 3600.0, 3600.0));
        c.clock = 3600.0;
        c.lost_gpu_seconds = 1800.0;
        let r = ChargebackReport::from_cluster(&c, &CostModel { gpu_hour_usd: 4.0 });
        assert!((r.idle_gpu_hours - 1.0).abs() < 1e-9);
        assert!((r.idle_cost_usd - 4.0).abs() < 1e-9);
        assert!((r.wasted_cost_usd - 2.0).abs() < 1e-9);
        let bad = ChargebackReport::from_cluster(
            &c,
            &CostModel {
                gpu_hour_usd: f64::NAN,
            },
        );
        assert_eq!(bad.used_cost_usd, 0.0);
    }

    #[test]
    fn empty_cluster_is_all_zero() {
        let r = ChargebackReport::from_cluster(&Cluster::new(vec![]), &CostModel::default());
        assert_eq!(r.used_gpu_hours, 0.0);
        assert!(r.tenants.is_empty());
    }
}
