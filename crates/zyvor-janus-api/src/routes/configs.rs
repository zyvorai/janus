// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize, Clone)]
pub struct ConfigEntry {
    id: String,
    path: String,
    /// Best-effort twin correlation hint -- see `resolve_config_twin_hint`.
    gpu_type: Option<String>,
    model: Option<String>,
}

/// Shared with `routes::batch::benchmark_presets`, which embeds the same
/// config list inline in its response (matching Python's
/// `benchmark_presets()`, which calls the same `_list_configs()` helper
/// `list_configs()` below also uses).
pub fn list_configs_raw(state: &AppState) -> std::io::Result<Vec<ConfigEntry>> {
    let config_dir = &state.inner.config_dir;
    if !config_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<ConfigEntry> = std::fs::read_dir(config_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("yaml"))
        .map(|path| {
            let id = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let rel = path
                .strip_prefix(&state.inner.repo_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            let (gpu_type, model) = zyvor_janus_config::resolve_config_twin_hint(&path)
                .map(|(g, m)| (Some(g), Some(m)))
                .unwrap_or((None, None));
            ConfigEntry {
                id,
                path: rel,
                gpu_type,
                model,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

/// `GET /api/configs` -- lists `configs/clusters/*.yaml`, matching the
/// Python server's `_list_configs()` (id = filename, path = relative to
/// repo root, sorted).
pub async fn list_configs(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(list_configs_raw(&state)?))
}
