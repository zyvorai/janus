// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::path::PathBuf;

use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zyvor_janus_config::{
    build_shadow_run, run_simulation_report_with_scheduler, shadow_side_report,
};

use crate::error::ApiError;
use crate::run_registry::{RunRecord, RunStatus};
use crate::state::AppState;
use crate::ws_ticket::mint_ticket;

#[derive(Deserialize)]
pub struct StartRunRequest {
    config: String,
    scheduler: Option<String>,
    /// If set, this run steps a shadow scheduler alongside `scheduler` over
    /// the same job arrivals and streams both sides' decisions live over
    /// `/ws/runs/{id}` instead of running a single scheduler to completion.
    shadow_scheduler: Option<String>,
}

#[derive(Serialize)]
struct StartRunResponse {
    id: Uuid,
}

#[derive(Serialize)]
struct RunSummary {
    id: Uuid,
    config: String,
    scheduler: Option<String>,
    shadow_scheduler: Option<String>,
    status: RunStatus,
    created_at: chrono::DateTime<chrono::Utc>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl RunSummary {
    fn from_record(run: &RunRecord) -> Self {
        Self {
            id: run.id,
            config: run.config.clone(),
            scheduler: run.display_scheduler(),
            shadow_scheduler: run.requested_shadow_scheduler.clone(),
            status: run.status,
            created_at: run.created_at,
            finished_at: run.finished_at,
        }
    }
}

/// `GET /api/runs` -- newest first, matching Python's
/// `sorted(RUNS.values(), key=lambda r: r.created_at, reverse=True)`.
pub async fn list_runs(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.inner.runs.read().await;
    let mut runs: Vec<RunSummary> = registry.values().map(RunSummary::from_record).collect();
    runs.sort_by_key(|r| std::cmp::Reverse(r.created_at));
    Json(runs)
}

/// `POST /api/runs` -- fire-and-forget: validates the config exists, creates
/// a Pending record, spawns the simulation on a blocking task (the axum
/// equivalent of Python's `asyncio.to_thread`), returns `{id}` immediately.
pub async fn start_run(
    State(state): State<AppState>,
    Json(body): Json<StartRunRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let config_path = state.inner.config_dir.join(&body.config);
    if !config_path.exists() {
        return Err(ApiError::not_found(format!(
            "unknown config: {}",
            body.config
        )));
    }

    let id = Uuid::new_v4();
    let record = RunRecord::new(
        id,
        body.config.clone(),
        body.scheduler.clone(),
        body.shadow_scheduler.clone(),
    );
    state.inner.runs.write().await.insert(id, record);

    match body.shadow_scheduler {
        Some(shadow_scheduler) => {
            tokio::spawn(execute_shadow_run(
                state,
                id,
                config_path,
                body.scheduler,
                shadow_scheduler,
            ));
        }
        None => {
            tokio::spawn(execute_run(state, id, config_path, body.scheduler));
        }
    }

    Ok(Json(StartRunResponse { id }))
}

/// Runs the simulation off the async executor and updates the registry +
/// persists JSON artifacts under `outputs/runs/{id}/`, matching Python's
/// `_execute_run` (including its "write-only, no rehydration on restart"
/// behavior -- an intentional parity choice, not a gap to fix here).
///
/// `start_run` above calls this via `tokio::spawn` (fire-and-forget).
/// `routes::batch` calls it directly with `.await` for the "blocks until
/// done" endpoints (`/api/compare`, `/api/benchmark/run`, `/api/what-if`),
/// matching Python's `await _execute_run(...)` in those handlers.
pub(crate) async fn execute_run(
    state: AppState,
    id: Uuid,
    config_path: PathBuf,
    scheduler_override: Option<String>,
) {
    {
        let mut registry = state.inner.runs.write().await;
        if let Some(run) = registry.get_mut(&id) {
            run.status = RunStatus::Running;
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        run_simulation_report_with_scheduler(&config_path, scheduler_override.as_deref())
    })
    .await;

    let mut registry = state.inner.runs.write().await;
    let Some(run) = registry.get_mut(&id) else {
        return;
    };

    match result {
        Ok(Ok(report)) => {
            run.report = Some(report);
            run.status = RunStatus::Completed;
            run.finished_at = Some(chrono::Utc::now());
            persist_run(&state.inner.runs_dir, run);
        }
        Ok(Err(config_err)) => {
            run.status = RunStatus::Failed;
            run.error = Some(config_err.to_string());
            run.finished_at = Some(chrono::Utc::now());
        }
        Err(join_err) => {
            run.status = RunStatus::Failed;
            run.error = Some(join_err.to_string());
            run.finished_at = Some(chrono::Utc::now());
        }
    }
}

/// Shadow-run counterpart to [`execute_run`]: builds a
/// [`build_shadow_run`] driver and steps it to completion on a blocking
/// task, broadcasting each `ShadowStep` over `state.inner.shadow_streams`
/// as it happens so `/ws/runs/{id}` can relay them live, then stores both
/// sides' final reports on the `RunRecord` exactly like a normal run.
pub(crate) async fn execute_shadow_run(
    state: AppState,
    id: Uuid,
    config_path: PathBuf,
    scheduler_override: Option<String>,
    shadow_scheduler: String,
) {
    {
        let mut registry = state.inner.runs.write().await;
        if let Some(run) = registry.get_mut(&id) {
            run.status = RunStatus::Running;
        }
    }

    let (tx, _rx) = tokio::sync::broadcast::channel(256);
    state
        .inner
        .shadow_streams
        .write()
        .await
        .insert(id, tx.clone());

    let result = tokio::task::spawn_blocking(move || {
        let handle = build_shadow_run(
            &config_path,
            scheduler_override.as_deref(),
            &shadow_scheduler,
        )?;
        let mut run = handle.run;
        while let Some(step) = run.step() {
            let msg = serde_json::json!({"type": "step", "data": step}).to_string();
            let _ = tx.send(msg);
        }
        let (primary_snapshots, shadow_snapshots) = run.take_replay_snapshots();
        let primary_report = shadow_side_report(
            run.primary_cluster(),
            primary_snapshots,
            handle.primary_scheduler,
            handle.config_hash.clone(),
            handle.jobs_total,
            &handle.cost,
        );
        let shadow_report = shadow_side_report(
            run.shadow_cluster(),
            shadow_snapshots,
            handle.shadow_scheduler,
            handle.config_hash,
            handle.jobs_total,
            &handle.cost,
        );
        Ok::<_, zyvor_janus_config::ConfigError>((primary_report, shadow_report))
    })
    .await;

    state.inner.shadow_streams.write().await.remove(&id);

    let mut registry = state.inner.runs.write().await;
    let Some(run) = registry.get_mut(&id) else {
        return;
    };

    match result {
        Ok(Ok((primary_report, shadow_report))) => {
            run.report = Some(primary_report);
            run.shadow_report = Some(shadow_report);
            run.status = RunStatus::Completed;
            run.finished_at = Some(chrono::Utc::now());
            persist_run(&state.inner.runs_dir, run);
        }
        Ok(Err(config_err)) => {
            run.status = RunStatus::Failed;
            run.error = Some(config_err.to_string());
            run.finished_at = Some(chrono::Utc::now());
        }
        Err(join_err) => {
            run.status = RunStatus::Failed;
            run.error = Some(join_err.to_string());
            run.finished_at = Some(chrono::Utc::now());
        }
    }
}

fn persist_run(runs_dir: &std::path::Path, run: &RunRecord) {
    let Some(report) = &run.report else { return };
    let dir = runs_dir.join(run.id.to_string());
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    fn write_json<T: serde::Serialize>(dir: &std::path::Path, name: &str, value: &T) {
        if let Ok(text) = serde_json::to_string_pretty(value) {
            let _ = std::fs::write(dir.join(name), text);
        }
    }
    write_json(&dir, "metrics.json", &report.metrics);
    write_json(&dir, "timeline.json", &report.timeline);
    write_json(&dir, "decisions.json", &report.decisions);
    write_json(&dir, "snapshots.json", &report.snapshots);
    write_json(&dir, "benchmark.json", &report.benchmark);

    if let Some(shadow_report) = &run.shadow_report {
        write_json(&dir, "shadow_metrics.json", &shadow_report.metrics);
        write_json(&dir, "shadow_timeline.json", &shadow_report.timeline);
        write_json(&dir, "shadow_decisions.json", &shadow_report.decisions);
        write_json(&dir, "shadow_snapshots.json", &shadow_report.snapshots);
        write_json(&dir, "shadow_benchmark.json", &shadow_report.benchmark);
    }

    let metadata = serde_json::json!({
        "config": run.config,
        "scheduler": run.requested_scheduler,
        "resolved_scheduler": report.scheduler,
        "config_hash": report.config_hash,
        "benchmark": report.benchmark,
        "shadow_scheduler": run.requested_shadow_scheduler,
        "shadow_resolved_scheduler": run.shadow_report.as_ref().map(|r| &r.scheduler),
        "shadow_benchmark": run.shadow_report.as_ref().map(|r| &r.benchmark),
    });
    if let Ok(text) = serde_json::to_string_pretty(&metadata) {
        let _ = std::fs::write(dir.join("metadata.json"), text);
    }
}

/// `GET /api/runs/{id}` -- full detail, matching Python's `get_run`.
pub async fn get_run(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = state.inner.runs.read().await;
    let run = registry
        .get(&id)
        .ok_or_else(|| ApiError::not_found("run not found"))?;

    Ok(Json(serde_json::json!({
        "id": run.id,
        "config": run.config,
        "scheduler": run.display_scheduler(),
        "status": run.status,
        "created_at": run.created_at,
        "finished_at": run.finished_at,
        "error": run.error,
        "metrics": run.report.as_ref().map(|r| &r.metrics),
        "timeline": run.report.as_ref().map(|r| &r.timeline),
        "decision_count": run.report.as_ref().map(|r| r.decisions.len()).unwrap_or(0),
        "config_hash": run.report.as_ref().map(|r| &r.config_hash),
        "benchmark": run.report.as_ref().and_then(|r| r.benchmark.as_ref()),
        "shadow_scheduler": run
            .requested_shadow_scheduler
            .clone()
            .or_else(|| run.shadow_report.as_ref().map(|r| r.scheduler.clone())),
        "shadow_metrics": run.shadow_report.as_ref().map(|r| &r.metrics),
        "shadow_decision_count": run.shadow_report.as_ref().map(|r| r.decisions.len()).unwrap_or(0),
        "shadow_benchmark": run.shadow_report.as_ref().and_then(|r| r.benchmark.as_ref()),
    })))
}

/// `GET /api/runs/{id}/snapshots`
pub async fn get_snapshots(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = state.inner.runs.read().await;
    let run = registry
        .get(&id)
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    let snapshots = run
        .report
        .as_ref()
        .map(|r| r.snapshots.clone())
        .unwrap_or_default();
    Ok(Json(snapshots))
}

/// `GET /api/runs/{id}/timeline`
pub async fn get_timeline(
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
        .ok_or_else(|| ApiError::not_found("timeline not ready"))?;
    Ok(Json(report.timeline.clone()))
}

#[derive(Serialize)]
struct WsTicketResponse {
    ticket: String,
}

/// `POST /api/runs/{id}/ws-ticket` -- mints a short-lived HMAC ticket for the
/// browser to attach to the `/ws/runs/{id}` upgrade as a query param, since
/// browsers cannot set an `Authorization` header on a WebSocket handshake.
pub async fn create_ws_ticket(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = state.inner.runs.read().await;
    if !registry.contains_key(&id) {
        return Err(ApiError::not_found("run not found"));
    }
    drop(registry);
    let ticket = mint_ticket(&state.inner.ws_ticket_secret, id);
    Ok(Json(WsTicketResponse { ticket }))
}

/// `GET /api/runs/{id}/events`
pub async fn get_events(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = state.inner.runs.read().await;
    let run = registry
        .get(&id)
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    let decisions = run
        .report
        .as_ref()
        .map(|r| r.decisions.clone())
        .unwrap_or_default();
    Ok(Json(decisions))
}

/// `GET /api/runs/{id}/shadow-events` -- the shadow-side counterpart to
/// `get_events`, empty for a non-shadow run or one that hasn't finished yet.
pub async fn get_shadow_events(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = state.inner.runs.read().await;
    let run = registry
        .get(&id)
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    let decisions = run
        .shadow_report
        .as_ref()
        .map(|r| r.decisions.clone())
        .unwrap_or_default();
    Ok(Json(decisions))
}
