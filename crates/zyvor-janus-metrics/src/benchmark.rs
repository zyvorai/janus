// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Scheduler benchmark score vector and optional composite weighting.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zyvor_janus_cost::CostModel;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::JobState;

use crate::SimulationMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerBenchmarkReport {
    pub scheduler: String,
    pub config_hash: String,
    pub metrics: SimulationMetrics,
    #[serde(default)]
    pub jain_fairness: f64,
    #[serde(default)]
    pub fragmentation: f64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub score_vector: HashMap<String, f64>,
}

impl SchedulerBenchmarkReport {
    pub fn from_simulation(
        scheduler: &str,
        config_hash: &str,
        cluster: &Cluster,
        metrics: SimulationMetrics,
        cost: &CostModel,
    ) -> Self {
        let jain_fairness = jain_index(cluster);
        let fragmentation = gpu_fragmentation(cluster, metrics.makespan);
        let gpu_seconds: f64 = cluster
            .finished_jobs
            .iter()
            .filter(|j| j.state == JobState::Finished)
            .map(|j| j.gpu_seconds_consumed)
            .sum();
        let cost_usd = (gpu_seconds / 3600.0) * cost.gpu_hour_usd;
        let mut score_vector = HashMap::new();
        score_vector.insert("makespan".into(), metrics.makespan);
        score_vector.insert("gpu_utilization".into(), metrics.gpu_utilization);
        score_vector.insert(
            "mean_cumulative_wait".into(),
            metrics.mean_cumulative_wait_time,
        );
        score_vector.insert("queue_delay_p99".into(), metrics.queue_delay_p99);
        score_vector.insert("ttft_p50".into(), metrics.ttft_p50);
        score_vector.insert("ttft_p99".into(), metrics.ttft_p99);
        score_vector.insert("tps_mean".into(), metrics.tps_mean);
        score_vector.insert("goodput".into(), metrics.goodput);
        score_vector.insert("jain_fairness".into(), jain_fairness);
        score_vector.insert("fragmentation".into(), fragmentation);
        score_vector.insert("cost_usd".into(), cost_usd);
        score_vector.insert("preemptions".into(), metrics.preemptions as f64);

        Self {
            scheduler: scheduler.into(),
            config_hash: config_hash.into(),
            metrics,
            jain_fairness,
            fragmentation,
            cost_usd,
            score_vector,
        }
    }

    pub fn composite_score(&self, weights: &HashMap<String, f64>) -> f64 {
        let mut total = 0.0;
        let mut weight_sum = 0.0;
        for (key, weight) in weights {
            if let Some(value) = self.score_vector.get(key) {
                total += normalize_score(key, *value) * weight;
                weight_sum += weight;
            }
        }
        if weight_sum > 0.0 {
            total / weight_sum
        } else {
            0.0
        }
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

fn normalize_score(key: &str, value: f64) -> f64 {
    match key {
        "gpu_utilization" | "goodput" | "jain_fairness" | "tps_mean" => value.min(1.0),
        "makespan"
        | "mean_cumulative_wait"
        | "queue_delay_p99"
        | "ttft_p50"
        | "ttft_p99"
        | "fragmentation"
        | "cost_usd"
        | "preemptions" => 1.0 / (1.0 + value.max(0.0)),
        _ => value,
    }
}

fn jain_index(cluster: &Cluster) -> f64 {
    let mut tenant_jobs: HashMap<String, usize> = HashMap::new();
    for job in cluster
        .finished_jobs
        .iter()
        .filter(|j| j.state == JobState::Finished)
    {
        let tenant = job.tenant.clone().unwrap_or_else(|| "default".into());
        *tenant_jobs.entry(tenant).or_default() += 1;
    }
    if tenant_jobs.is_empty() {
        return 1.0;
    }
    let values: Vec<f64> = tenant_jobs.values().map(|v| *v as f64).collect();
    let sum: f64 = values.iter().sum();
    let sum_sq: f64 = values.iter().map(|v| v * v).sum();
    if sum_sq == 0.0 {
        1.0
    } else {
        (sum * sum) / (tenant_jobs.len() as f64 * sum_sq)
    }
}

fn gpu_fragmentation(cluster: &Cluster, makespan: f64) -> f64 {
    if makespan <= 0.0 {
        return 0.0;
    }
    let gpu_count = cluster.gpu_count().max(1) as f64;
    let gpu_seconds_busy: f64 = cluster
        .finished_jobs
        .iter()
        .filter(|j| j.state == JobState::Finished)
        .map(|j| j.gpu_seconds_consumed)
        .sum();
    let total_gpu_seconds = makespan * gpu_count;
    if total_gpu_seconds <= 0.0 {
        0.0
    } else {
        ((total_gpu_seconds - gpu_seconds_busy) / total_gpu_seconds).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zyvor_janus_model::models::{Gpu, Job, Node};

    #[test]
    fn composite_score_changes_with_weights() {
        let mut cluster = Cluster::new(vec![Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
        }]);
        let mut job = Job::new("j1", "a", 0.0, 10.0, 1);
        job.state = JobState::Finished;
        job.finish_time = Some(10.0);
        job.gpu_seconds_consumed = 10.0;
        cluster.finished_jobs.push(job);

        let metrics = SimulationMetrics::from_cluster(&cluster, 1);
        let report = SchedulerBenchmarkReport::from_simulation(
            "fifo",
            "abc",
            &cluster,
            metrics,
            &CostModel::default(),
        );
        let mut util_weights = HashMap::new();
        util_weights.insert("gpu_utilization".into(), 1.0);
        let mut makespan_weights = HashMap::new();
        makespan_weights.insert("makespan".into(), 1.0);
        assert_ne!(
            report.composite_score(&util_weights),
            report.composite_score(&makespan_weights)
        );
    }
}
