// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

/// `GET /api/runs/{id}/serving-trace` -- the real per-job export from the
/// finished `Cluster` (`report.serving_trace`), populated by
/// `zyvor_janus_config::export_serving_trace_from_cluster`. This
/// intentionally differs from Python's hand-rolled version, which hardcodes
/// `input_tokens: 128, output_tokens: 64` for every record -- not a parity
/// break, an improvement the plan calls out explicitly.
pub async fn get_serving_trace(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = state.inner.runs.read().await;
    let run = registry
        .get(&id)
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    let report = run
        .report
        .as_ref()
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    Ok(Json(report.serving_trace.clone()))
}
