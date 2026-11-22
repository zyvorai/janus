// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::profile_registry::ProfileRegistry;
use crate::run_registry::{new_registry, RunRegistry};

/// Sliding-window request timestamps per client key, guarded by a plain
/// `std::sync::Mutex` since each critical section is a short, non-blocking
/// `Vec` scan -- no need for an async lock. Mirrors Python's
/// `_rate_buckets: dict[str, list[float]]`.
pub type RateBuckets = Arc<Mutex<HashMap<String, Vec<Instant>>>>;

/// One broadcast sender per in-progress shadow run -- present only while
/// the run is actively stepping. `/ws/runs/{id}` subscribes here to relay
/// each [`zyvor_janus_simulator::ShadowStep`] live; `execute_shadow_run`
/// removes the entry once the run finishes, at which point the WS handler
/// falls back to the stored `RunRecord.report`/`shadow_report`.
pub type ShadowStreams = Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>;

/// Shared application state, mirroring the env-driven paths the Python
/// server resolves today (`ZYVOR_JANUS_ROOT`/`CONFIG_DIR`/`RUNS_DIR`/`PROFILES_DIR`).
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub repo_root: PathBuf,
    pub config_dir: PathBuf,
    pub runs_dir: PathBuf,
    pub api_key: String,
    pub ws_ticket_secret: String,
    pub runs: RunRegistry,
    pub model_profiles: ProfileRegistry,
    pub shim_rate_limit_per_min: u32,
    pub rate_buckets: RateBuckets,
    pub shadow_streams: ShadowStreams,
}

impl AppState {
    pub fn from_env() -> Self {
        let repo_root =
            PathBuf::from(std::env::var("ZYVOR_JANUS_ROOT").unwrap_or_else(|_| ".".to_string()));
        let config_dir = std::env::var("ZYVOR_JANUS_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.join("configs").join("clusters"));
        let runs_dir = std::env::var("ZYVOR_JANUS_RUNS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.join("outputs").join("runs"));
        let profiles_dir = std::env::var("ZYVOR_JANUS_PROFILES_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.join("configs").join("profiles"));
        let api_key = std::env::var("ZYVOR_JANUS_API_KEY")
            .unwrap_or_else(|_| "dev-zyvor-janus-key".to_string());
        let ws_ticket_secret = std::env::var("ZYVOR_JANUS_WS_TICKET_SECRET")
            .unwrap_or_else(|_| "dev-zyvor-janus-ws-ticket-secret".to_string());
        let shim_rate_limit_per_min = std::env::var("ZYVOR_JANUS_SHIM_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120);
        let model_profiles = ProfileRegistry::load(&profiles_dir);

        Self {
            inner: Arc::new(AppStateInner {
                repo_root,
                config_dir,
                runs_dir,
                api_key,
                ws_ticket_secret,
                runs: new_registry(),
                model_profiles,
                shim_rate_limit_per_min,
                rate_buckets: Arc::new(Mutex::new(HashMap::new())),
                shadow_streams: Arc::new(RwLock::new(HashMap::new())),
            }),
        }
    }
}
