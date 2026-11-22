// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ApiError;
use crate::state::AppState;

/// Shared-secret bearer-token check applied to the whole router except
/// `/api/health` (see main.rs — health is routed outside this layer so K8s
/// liveness/readiness probes need no credentials).
///
/// Closes the gap where `deploy/kubernetes/ingress.yaml` routes `/api` and
/// `/ws` directly to this service's K8s Service, bypassing the Next.js
/// pod's session-cookie check entirely -- every request now needs the
/// shared secret regardless of which path reaches this server.
pub async fn require_bearer_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = header.and_then(|h| h.strip_prefix("Bearer "));

    match token {
        Some(t) if constant_time_eq(t.as_bytes(), state.inner.api_key.as_bytes()) => {
            Ok(next.run(request).await)
        }
        _ => Err(ApiError::unauthorized("missing or invalid bearer token")),
    }
}

/// Constant-time byte comparison to avoid leaking the secret's length/prefix
/// via response-timing side channels. Also used by `ws_ticket` to verify
/// ticket signatures.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
