// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::run_registry::RunStatus;
use crate::state::AppState;
use crate::ws_ticket::verify_ticket;

#[derive(Deserialize)]
pub struct WsQuery {
    ticket: String,
}

/// `GET /ws/runs/{id}?ticket=...` -- deliberately NOT behind the bearer-auth
/// middleware (browsers cannot set an `Authorization` header on a WebSocket
/// handshake, and Next's rewrite can't proxy WS upgrades at all -- see
/// web/src/lib/api.ts). Verifies the short-lived ticket minted by
/// `POST /api/runs/{id}/ws-ticket` instead, before ever upgrading.
pub async fn ws_run(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Query(query): Query<WsQuery>,
) -> Response {
    if !verify_ticket(&state.inner.ws_ticket_secret, id, &query.ticket) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let exists = state.inner.runs.read().await.contains_key(&id);
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }
    ws.on_upgrade(move |socket| relay(socket, state, id))
}

/// For an ordinary run: replays collected snapshots (50ms apart), then
/// polls every 200ms while the run is `Running` -- a near-transliteration
/// of Python's `ws_run` (`python/zyvor_janus/server/app.py`), including its
/// "final message uses whatever metrics/decisions exist even if the run
/// never actually ran" behavior for a run that's still `Pending`.
///
/// For a shadow run that's still executing, skips the snapshot replay
/// entirely and instead forwards each live `{"type": "step", ...}` message
/// off `state.inner.shadow_streams` as the driver in `execute_shadow_run`
/// produces it, until that channel closes (the run finished). A shadow run
/// the client connects to *after* it already finished has no channel left,
/// so it falls back to the ordinary snapshot-replay path like any other
/// completed run.
///
/// Either way, ends with one final `complete` message carrying both sides'
/// metrics/decisions (the shadow fields are simply absent for a
/// non-shadow run).
async fn relay(mut socket: WebSocket, state: AppState, id: Uuid) {
    let live_rx = state
        .inner
        .shadow_streams
        .read()
        .await
        .get(&id)
        .map(|tx| tx.subscribe());

    match live_rx {
        Some(mut rx) => {
            use tokio::sync::broadcast::error::RecvError;
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if socket.send(Message::Text(msg)).await.is_err() {
                            return;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        }
        None => {
            let snapshots = {
                let registry = state.inner.runs.read().await;
                registry
                    .get(&id)
                    .and_then(|run| run.report.as_ref())
                    .map(|report| report.snapshots.clone())
                    .unwrap_or_default()
            };
            for snap in snapshots {
                let msg = json!({"type": "snapshot", "data": snap});
                if socket.send(Message::Text(msg.to_string())).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            loop {
                let status = state.inner.runs.read().await.get(&id).map(|run| run.status);
                match status {
                    Some(RunStatus::Running) => {
                        tokio::time::sleep(Duration::from_millis(200)).await
                    }
                    _ => break,
                }
            }
        }
    }

    let (metrics, decisions, shadow_metrics, shadow_decisions) = {
        let registry = state.inner.runs.read().await;
        let run = registry.get(&id);
        let report = run.and_then(|r| r.report.as_ref());
        let shadow_report = run.and_then(|r| r.shadow_report.as_ref());
        (
            report.map(|r| r.metrics.clone()),
            report.map(|r| r.decisions.clone()).unwrap_or_default(),
            shadow_report.map(|r| r.metrics.clone()),
            shadow_report
                .map(|r| r.decisions.clone())
                .unwrap_or_default(),
        )
    };
    let msg = json!({
        "type": "complete",
        "metrics": metrics,
        "decisions": decisions,
        "shadow_metrics": shadow_metrics,
        "shadow_decisions": shadow_decisions,
    });
    let _ = socket.send(Message::Text(msg.to_string())).await;
}
