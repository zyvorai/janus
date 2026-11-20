// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

/// `GET /api/health` -- matches `{"status": "ok"}` from the Python server.
/// Routed outside the bearer-auth layer (see main.rs) so K8s liveness/
/// readiness probes need no credentials.
pub async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}
