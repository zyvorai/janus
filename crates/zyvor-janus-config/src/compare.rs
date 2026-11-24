// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Baseline-vs-candidate scheduler comparison with paired confidence
//! intervals across failure-injection seeds.
//!
//! The simulator itself is deterministic: seeds only vary the injected
//! crash schedule. A config without a `failure:` section therefore yields
//! one run per policy and NO confidence interval (a CI over identical
//! deterministic runs would be meaningless). See docs/POLICY_COMPARISON.md.

use std::path::Path;

use serde::Serialize;

use crate::{run_simulation_report_with_options, ConfigError, ConfigResult, SimulationReport};

pub const MAX_COMPARE_SEEDS: usize = 1000;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MetricComparison {
    pub metric: String,
    pub lower_is_better: bool,
    pub baseline_mean: f64,
    pub candidate_mean: f64,
    /// candidate - baseline, averaged over seeds (paired).
    pub mean_diff: f64,
    pub std_dev: Option<f64>,
    /// Half-width of the 95% Student-t interval on the mean paired difference.
    pub ci95_half_width: Option<f64>,
    /// "candidate_better", "candidate_worse" or "inconclusive".
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    pub baseline: String,
    pub candidate: String,
    pub seeds: Vec<u64>,
    /// False when the config has no failure injection: one run, no CI.
    pub stochastic: bool,
    pub metrics: Vec<MetricComparison>,
    pub notes: Vec<String>,
}

/// Two-sided 95% Student-t critical value for `df` degrees of freedom.
pub fn t_crit_95(df: usize) -> f64 {
    const T: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    match df {
        0 => f64::INFINITY,
        1..=30 => T[df - 1],
        31..=40 => 2.021,
        41..=60 => 2.000,
        61..=120 => 1.980,
        _ => 1.960,
    }
}

type Extractor = fn(&SimulationReport) -> f64;

fn q(r: &SimulationReport) -> zyvor_janus_metrics::QualityReport {
    r.quality.clone().unwrap_or_default()
}

fn metric_table() -> Vec<(&'static str, bool, Extractor)> {
    vec![
        ("makespan", true, |r| r.metrics.makespan),
        ("mean_wait_time", true, |r| r.metrics.mean_wait_time),
        ("wait_p99", true, |r| q(r).wait_p99),
        ("slowdown_mean", true, |r| q(r).slowdown_mean),
        ("slowdown_p99", true, |r| q(r).slowdown_p99),
        ("gpu_utilization", false, |r| r.metrics.gpu_utilization),
        ("jain_tenant_service", false, |r| q(r).jain_tenant_service),
        ("ftf_rho_max", true, |r| q(r).ftf_rho_max),
        ("mean_fragmentation", true, |r| q(r).mean_fragmentation),
        ("jobs_failed", true, |r| r.metrics.jobs_failed as f64),
    ]
}

pub fn compare_policies(
    config_path: &Path,
    baseline: &str,
    candidate: &str,
    seeds: &[u64],
) -> ConfigResult<ComparisonReport> {
    if seeds.len() > MAX_COMPARE_SEEDS {
        return Err(ConfigError::Invalid(format!(
            "at most {MAX_COMPARE_SEEDS} seeds are allowed (got {})",
            seeds.len()
        )));
    }
    let stochastic = crate::load_simulation_config(config_path)?
        .failure
        .is_some();
    let mut notes = Vec::new();
    let seed_list: Vec<Option<u64>> = if stochastic {
        if seeds.is_empty() {
            return Err(ConfigError::Invalid("provide at least one seed".into()));
        }
        seeds.iter().map(|s| Some(*s)).collect()
    } else {
        notes.push(
            "config has no `failure:` section: runs are deterministic, so one run per policy and no confidence interval"
                .into(),
        );
        vec![None]
    };

    let table = metric_table();
    let mut base_vals: Vec<Vec<f64>> = vec![Vec::new(); table.len()];
    let mut cand_vals: Vec<Vec<f64>> = vec![Vec::new(); table.len()];
    for seed in &seed_list {
        let b = run_simulation_report_with_options(config_path, Some(baseline), *seed)?;
        let c = run_simulation_report_with_options(config_path, Some(candidate), *seed)?;
        for (i, (name, _, f)) in table.iter().enumerate() {
            let (bv, cv) = (f(&b), f(&c));
            if !bv.is_finite() || !cv.is_finite() {
                return Err(ConfigError::Invalid(format!(
                    "metric '{name}' was not finite"
                )));
            }
            base_vals[i].push(bv);
            cand_vals[i].push(cv);
        }
    }

    let metrics = table
        .iter()
        .enumerate()
        .map(|(i, (name, lower_is_better, _))| {
            summarize(name, *lower_is_better, &base_vals[i], &cand_vals[i])
        })
        .collect();
    Ok(ComparisonReport {
        baseline: baseline.into(),
        candidate: candidate.into(),
        seeds: seed_list.iter().flatten().copied().collect(),
        stochastic,
        metrics,
        notes,
    })
}

pub(crate) fn summarize(
    name: &str,
    lower_is_better: bool,
    base: &[f64],
    cand: &[f64],
) -> MetricComparison {
    let n = base.len();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let diffs: Vec<f64> = base.iter().zip(cand).map(|(b, c)| c - b).collect();
    let mean_diff = mean(&diffs);
    let (std_dev, half) = if n >= 2 {
        let var = diffs.iter().map(|d| (d - mean_diff).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        let sd = var.sqrt();
        (Some(sd), Some(t_crit_95(n - 1) * sd / (n as f64).sqrt()))
    } else {
        (None, None)
    };
    let verdict = match half {
        Some(h) if mean_diff.abs() > h => {
            if (mean_diff < 0.0) == lower_is_better {
                "candidate_better"
            } else {
                "candidate_worse"
            }
        }
        _ => "inconclusive",
    };
    MetricComparison {
        metric: name.into(),
        lower_is_better,
        baseline_mean: mean(base),
        candidate_mean: mean(cand),
        mean_diff,
        std_dev,
        ci95_half_width: half,
        verdict: verdict.into(),
    }
}

/// One scheduler's row in a [`leaderboard`].
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LeaderboardRow {
    pub rank: usize,
    pub scheduler: String,
    pub makespan: f64,
    pub mean_wait_time: f64,
    pub wait_p99: f64,
    pub jct_mean: f64,
    pub jct_p99: f64,
    pub slowdown_mean: f64,
    pub gpu_utilization: f64,
    pub jain_tenant_service: f64,
    pub ftf_rho_max: f64,
    pub cost_usd: f64,
    pub jobs_failed: usize,
    pub preemptions: u32,
    /// SLO attainment (0..1); `None` unless the config has an `slo:` section.
    pub slo_attainment: Option<f64>,
}

/// Metrics a leaderboard can be sorted by; the bool is "lower is better".
pub const LEADERBOARD_SORT_KEYS: [(&str, bool); 10] = [
    ("jct_mean", true),
    ("slo_attainment", false),
    ("makespan", true),
    ("mean_wait_time", true),
    ("wait_p99", true),
    ("slowdown_mean", true),
    ("ftf_rho_max", true),
    ("cost_usd", true),
    ("gpu_utilization", false),
    ("jain_tenant_service", false),
];

fn sort_value(r: &LeaderboardRow, key: &str) -> f64 {
    match key {
        "makespan" => r.makespan,
        "mean_wait_time" => r.mean_wait_time,
        "wait_p99" => r.wait_p99,
        "jct_mean" => r.jct_mean,
        "slo_attainment" => r.slo_attainment.unwrap_or(0.0),
        "slowdown_mean" => r.slowdown_mean,
        "ftf_rho_max" => r.ftf_rho_max,
        "cost_usd" => r.cost_usd,
        "gpu_utilization" => r.gpu_utilization,
        _ => r.jain_tenant_service,
    }
}

/// Runs `config_path` once per scheduler (same seed) and ranks them by
/// `sort_by`. Ties break on scheduler name so output is deterministic.
pub fn leaderboard(
    config_path: &Path,
    schedulers: &[String],
    sort_by: &str,
    seed: Option<u64>,
) -> ConfigResult<Vec<LeaderboardRow>> {
    let Some(&(_, lower_is_better)) = LEADERBOARD_SORT_KEYS.iter().find(|(k, _)| *k == sort_by)
    else {
        return Err(ConfigError::Invalid(format!(
            "unknown sort key '{sort_by}'; expected one of {}",
            LEADERBOARD_SORT_KEYS
                .iter()
                .map(|(k, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    };
    let mut names: Vec<&str> = Vec::new();
    for s in schedulers {
        if !names.contains(&s.as_str()) {
            names.push(s.as_str());
        }
    }
    if names.is_empty() {
        return Err(ConfigError::Invalid(
            "provide at least one scheduler".into(),
        ));
    }
    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let r = run_simulation_report_with_options(config_path, Some(name), seed)?;
        let quality = r.quality.clone().unwrap_or_default();
        rows.push(LeaderboardRow {
            rank: 0,
            scheduler: name.to_string(),
            makespan: r.metrics.makespan,
            mean_wait_time: r.metrics.mean_wait_time,
            wait_p99: quality.wait_p99,
            jct_mean: quality.jct_mean,
            jct_p99: quality.jct_p99,
            slowdown_mean: quality.slowdown_mean,
            gpu_utilization: r.metrics.gpu_utilization,
            jain_tenant_service: quality.jain_tenant_service,
            ftf_rho_max: quality.ftf_rho_max,
            cost_usd: r.chargeback.map(|c| c.used_cost_usd).unwrap_or(0.0),
            jobs_failed: r.metrics.jobs_failed as usize,
            preemptions: r.metrics.preemptions as u32,
            slo_attainment: r.slo.as_ref().map(|s| s.attainment),
        });
    }
    rows.sort_by(|a, b| {
        let (x, y) = (sort_value(a, sort_by), sort_value(b, sort_by));
        let o = x.total_cmp(&y);
        (if lower_is_better { o } else { o.reverse() }).then_with(|| a.scheduler.cmp(&b.scheduler))
    });
    for (i, r) in rows.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_table_is_monotone_and_hits_known_values() {
        assert_eq!(t_crit_95(1), 12.706);
        assert_eq!(t_crit_95(9), 2.262);
        assert_eq!(t_crit_95(10_000), 1.960);
        for df in 1..200 {
            assert!(t_crit_95(df) >= t_crit_95(df + 1));
        }
    }

    #[test]
    fn clear_improvement_is_reported_better() {
        let base = [100.0, 102.0, 98.0, 101.0, 99.0];
        let cand = [80.0, 83.0, 78.0, 81.0, 79.0];
        let m = summarize("makespan", true, &base, &cand);
        assert!(m.mean_diff < -15.0);
        assert_eq!(m.verdict, "candidate_better");
        let m = summarize("gpu_utilization", false, &base, &cand);
        assert_eq!(m.verdict, "candidate_worse");
    }

    #[test]
    fn noise_is_inconclusive_and_single_run_has_no_ci() {
        let base = [10.0, 12.0, 9.0, 11.0];
        let cand = [11.0, 10.0, 10.0, 11.0];
        assert_eq!(summarize("x", true, &base, &cand).verdict, "inconclusive");
        let one = summarize("x", true, &[1.0], &[0.0]);
        assert!(one.ci95_half_width.is_none());
        assert_eq!(one.verdict, "inconclusive");
    }

    #[test]
    fn identical_runs_have_zero_diff() {
        let v = [5.0, 6.0, 7.0];
        let m = summarize("x", true, &v, &v);
        assert_eq!(m.mean_diff, 0.0);
        assert_eq!(m.verdict, "inconclusive");
    }
}
