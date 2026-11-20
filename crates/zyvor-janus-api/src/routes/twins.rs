// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::error::ApiError;
use crate::state::AppState;
use crate::twin_store::list_twins;

/// `GET /api/twins` -- queries `outputs/twins/twins.sqlite` directly, unlike
/// Python's `list_twins` which exports to `export.json` and rereads it.
pub async fn get_twins(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let db_path = state
        .inner
        .repo_root
        .join("outputs")
        .join("twins")
        .join("twins.sqlite");
    let twins = tokio::task::spawn_blocking(move || list_twins(&db_path))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(twins))
}
