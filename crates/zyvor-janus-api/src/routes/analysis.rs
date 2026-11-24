// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use zyvor_janus_config::{
    generate_workload, generated_workload_to_yaml, load_sweep, ConfigError, GenerateSpec,
};

use crate::error::ApiError;
use crate::state::AppState;

fn config_err(e: ConfigError) -> ApiError {
    match e {
        ConfigError::Invalid(m) => ApiError::bad_request(m),
        other => ApiError::internal(other.to_string()),
    }
}

/// `POST /api/workloads/generate` -- body is a `GenerateSpec`; returns the
/// job count and the workload YAML the simulator loads.
pub async fn generate(Json(spec): Json<GenerateSpec>) -> Result<impl IntoResponse, ApiError> {
    let out = tokio::task::spawn_blocking(move || {
        let w = generate_workload(&spec)?;
        let yaml = generated_workload_to_yaml(&w)?;
        Ok::<_, ConfigError>((w.jobs.len(), yaml))
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .map_err(config_err)?;
    Ok(Json(serde_json::json!({ "jobs": out.0, "yaml": out.1 })))
}

#[derive(Deserialize)]
pub struct LoadSweepRequest {
    config: String,
    #[serde(default = "default_schedulers")]
    schedulers: Vec<String>,
    #[serde(default = "default_loads")]
    loads: Vec<f64>,
    #[serde(default = "default_max_wait")]
    max_wait_p90_secs: f64,
    seed: Option<u64>,
}

fn default_schedulers() -> Vec<String> {
    ["fifo", "easy", "sjf"].map(String::from).to_vec()
}
fn default_loads() -> Vec<f64> {
    vec![0.5, 1.0, 2.0, 4.0]
}
fn default_max_wait() -> f64 {
    600.0
}

/// `POST /api/load-sweep` -- latency-vs-load per scheduler for one config.
pub async fn load_sweep_route(
    State(state): State<AppState>,
    Json(body): Json<LoadSweepRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let path = state.config_path(&body.config)?;
    if !path.exists() {
        return Err(ApiError::not_found(format!(
            "unknown config: {}",
            body.config
        )));
    }
    let report = tokio::task::spawn_blocking(move || {
        load_sweep(
            &path,
            &body.schedulers,
            &body.loads,
            body.max_wait_p90_secs,
            body.seed,
        )
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .map_err(config_err)?;
    Ok(Json(report))
}
