// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use std::collections::HashSet;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::configs::list_configs_raw;
use super::runs::execute_run;
use crate::error::ApiError;
use crate::presets::PRESETS;
use crate::run_registry::{RunRecord, RunStatus};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CompareRequest {
    configs: Vec<String>,
}

#[derive(Serialize)]
struct CompareResultRow {
    config: String,
    status: RunStatus,
    metrics: Option<zyvor_janus_metrics::SimulationMetrics>,
    run_id: Uuid,
}

/// `POST /api/compare` -- runs each config sequentially and blocks until all
/// are done, matching Python's `compare_schedulers` (including that it
/// never spawns them concurrently).
pub async fn compare(
    State(state): State<AppState>,
    Json(body): Json<CompareRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let unique: HashSet<&String> = body.configs.iter().collect();
    if unique.len() != body.configs.len() {
        return Err(ApiError::bad_request("configs must be distinct"));
    }

    let mut results = Vec::with_capacity(body.configs.len());
    for config in &body.configs {
        let config_path = state.inner.config_dir.join(config);
        if !config_path.exists() {
            return Err(ApiError::not_found(format!("unknown config: {config}")));
        }
        let id = Uuid::new_v4();
        state
            .inner
            .runs
            .write()
            .await
            .insert(id, RunRecord::new(id, config.clone(), None, None));
        execute_run(state.clone(), id, config_path, None).await;

        let registry = state.inner.runs.read().await;
        let run = registry.get(&id).expect("just inserted");
        results.push(CompareResultRow {
            config: config.clone(),
            status: run.status,
            metrics: run.report.as_ref().map(|r| r.metrics.clone()),
            run_id: id,
        });
    }

    Ok(Json(serde_json::json!({ "results": results })))
}

/// `GET /api/benchmark/presets`
pub async fn benchmark_presets(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "configs": list_configs_raw(&state).unwrap_or_default(),
        "workload_presets": PRESETS,
    }))
}

/// `GET /api/benchmark/reports` -- completed runs that produced a benchmark.
pub async fn benchmark_reports(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.inner.runs.read().await;
    let reports: Vec<_> = registry
        .values()
        .filter(|run| run.status == RunStatus::Completed)
        .filter_map(|run| {
            let report = run.report.as_ref()?;
            let benchmark = report.benchmark.as_ref()?;
            Some(serde_json::json!({
                "run_id": run.id,
                "config": run.config,
                "scheduler": run.display_scheduler(),
                "metrics": report.metrics,
                "benchmark": benchmark,
            }))
        })
        .collect();
    Json(reports)
}

#[derive(Deserialize)]
pub struct BenchmarkRunRequest {
    config: String,
    scheduler: Option<String>,
}

/// `POST /api/benchmark/run` -- runs one config, blocks until complete,
/// matching Python's `benchmark_run`.
pub async fn benchmark_run(
    State(state): State<AppState>,
    Json(body): Json<BenchmarkRunRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let config_path = state.inner.config_dir.join(&body.config);
    if !config_path.exists() {
        return Err(ApiError::not_found(format!(
            "unknown config: {}",
            body.config
        )));
    }
    let id = Uuid::new_v4();
    state.inner.runs.write().await.insert(
        id,
        RunRecord::new(id, body.config.clone(), body.scheduler.clone(), None),
    );
    execute_run(state.clone(), id, config_path, body.scheduler).await;

    let registry = state.inner.runs.read().await;
    let run = registry.get(&id).expect("just inserted");
    if run.status != RunStatus::Completed {
        return Err(ApiError::internal(
            run.error.clone().unwrap_or_else(|| "run failed".into()),
        ));
    }
    let report = run.report.as_ref().expect("completed run has a report");
    Ok(Json(serde_json::json!({
        "run_id": id,
        "metrics": report.metrics,
        "benchmark": report.benchmark,
    })))
}

#[derive(Deserialize)]
pub struct WhatIfRequest {
    base_config: String,
    #[serde(default = "default_schedulers")]
    schedulers: Vec<String>,
    configs: Option<Vec<String>>,
}

fn default_schedulers() -> Vec<String> {
    vec!["fifo".to_string(), "preemptive".to_string()]
}

#[derive(Serialize)]
struct WhatIfRow {
    config: String,
    scheduler: String,
    run_id: Uuid,
    status: RunStatus,
    metrics: Option<zyvor_janus_metrics::SimulationMetrics>,
    benchmark: Option<zyvor_janus_metrics::SchedulerBenchmarkReport>,
}

/// `POST /api/what-if` -- cartesian product of configs x schedulers, blocks
/// until every run is done, matching Python's `what_if`.
pub async fn what_if(
    State(state): State<AppState>,
    Json(body): Json<WhatIfRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let configs = body
        .configs
        .unwrap_or_else(|| vec![body.base_config.clone()]);
    let mut rows = Vec::new();

    for config in &configs {
        for scheduler in &body.schedulers {
            let config_path = state.inner.config_dir.join(config);
            if !config_path.exists() {
                return Err(ApiError::not_found(format!("unknown config: {config}")));
            }
            let id = Uuid::new_v4();
            state.inner.runs.write().await.insert(
                id,
                RunRecord::new(id, config.clone(), Some(scheduler.clone()), None),
            );
            execute_run(state.clone(), id, config_path, Some(scheduler.clone())).await;

            let registry = state.inner.runs.read().await;
            let run = registry.get(&id).expect("just inserted");
            rows.push(WhatIfRow {
                config: config.clone(),
                scheduler: scheduler.clone(),
                run_id: id,
                status: run.status,
                metrics: run.report.as_ref().map(|r| r.metrics.clone()),
                benchmark: run.report.as_ref().and_then(|r| r.benchmark.clone()),
            });
        }
    }

    Ok(Json(serde_json::json!({ "results": rows })))
}
