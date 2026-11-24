// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Load sweep: replay one config at several offered loads and report each
//! scheduler's latency-vs-load curve, in the spirit of the Gavel evaluation
//! (average JCT against input job rate, and the "sustainable load" past
//! which queues blow up).
//!
//! Offered load is applied by dividing every arrival time by the load
//! factor: 2.0 means jobs arrive twice as fast, 0.5 half as fast. Runtimes
//! and job sizes are untouched, so the factor scales arrival intensity only.

use std::path::Path;

use serde::Serialize;

use crate::{run_report_scaled, ConfigError, ConfigResult};

pub const MAX_SWEEP_LOADS: usize = 50;
pub const MAX_SWEEP_SCHEDULERS: usize = 12;
pub const MAX_LOAD_FACTOR: f64 = 1000.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SweepRow {
    pub scheduler: String,
    pub load: f64,
    pub jobs_completed: usize,
    pub jobs_never_started: usize,
    pub makespan: f64,
    pub jct_mean: f64,
    pub jct_p99: f64,
    pub wait_p90: f64,
    pub gpu_utilization: f64,
    /// Set when the config has an `slo:` section.
    pub slo_attainment: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SustainableLoad {
    pub scheduler: String,
    /// Highest swept load L such that every swept load <= L kept wait p90
    /// within the limit and left no job unstarted; `None` if even the
    /// smallest swept load broke the limit.
    pub sustainable_load: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SweepReport {
    pub max_wait_p90_secs: f64,
    pub loads: Vec<f64>,
    pub rows: Vec<SweepRow>,
    pub sustainable: Vec<SustainableLoad>,
}

/// Runs `config_path` for each `(scheduler, load)` pair. Loads are sorted
/// ascending and de-duplicated; scheduler order is preserved.
pub fn load_sweep(
    config_path: &Path,
    schedulers: &[String],
    loads: &[f64],
    max_wait_p90_secs: f64,
    seed: Option<u64>,
) -> ConfigResult<SweepReport> {
    let bad = |m: String| Err(ConfigError::Invalid(m));
    if !max_wait_p90_secs.is_finite() || max_wait_p90_secs < 0.0 {
        return bad("max_wait_p90_secs must be a finite number >= 0".into());
    }
    if loads.is_empty() || loads.len() > MAX_SWEEP_LOADS {
        return bad(format!("provide 1..={MAX_SWEEP_LOADS} load factors"));
    }
    if let Some(l) = loads
        .iter()
        .find(|l| !l.is_finite() || **l <= 0.0 || **l > MAX_LOAD_FACTOR)
    {
        return bad(format!(
            "load factor {l} must be a finite number in (0, {MAX_LOAD_FACTOR}]"
        ));
    }
    let mut names: Vec<&str> = Vec::new();
    for s in schedulers {
        if !names.contains(&s.as_str()) {
            names.push(s);
        }
    }
    if names.is_empty() || names.len() > MAX_SWEEP_SCHEDULERS {
        return bad(format!("provide 1..={MAX_SWEEP_SCHEDULERS} schedulers"));
    }
    let mut sorted: Vec<f64> = loads.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted.dedup();

    let mut rows = Vec::new();
    let mut sustainable = Vec::new();
    for name in &names {
        let mut best: Option<f64> = None;
        let mut broken = false;
        for &load in &sorted {
            let r = run_report_scaled(config_path, Some(name), seed, Some(1.0 / load))?;
            let q = r.quality.clone().unwrap_or_default();
            if !broken && q.wait_p90 <= max_wait_p90_secs && q.jobs_never_started == 0 {
                best = Some(load);
            } else {
                broken = true;
            }
            rows.push(SweepRow {
                scheduler: name.to_string(),
                load,
                jobs_completed: r.metrics.jobs_completed,
                jobs_never_started: q.jobs_never_started,
                makespan: r.metrics.makespan,
                jct_mean: q.jct_mean,
                jct_p99: q.jct_p99,
                wait_p90: q.wait_p90,
                gpu_utilization: r.metrics.gpu_utilization,
                slo_attainment: r.slo.as_ref().map(|s| s.attainment),
            });
        }
        sustainable.push(SustainableLoad {
            scheduler: name.to_string(),
            sustainable_load: best,
        });
    }
    Ok(SweepReport {
        max_wait_p90_secs,
        loads: sorted,
        rows,
        sustainable,
    })
}
