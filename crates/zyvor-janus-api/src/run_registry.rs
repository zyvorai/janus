// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use zyvor_janus_config::SimulationReport;

/// In-memory run registry, matching the Python server's `RUNS: dict[str,
/// RunRecord]` -- process-local, lost on restart, no rehydration from the
/// `outputs/runs/{id}/*.json` files this module also writes. That's an
/// intentional parity choice (see plan's non-goals), not an oversight.
pub type RunRegistry = Arc<RwLock<HashMap<Uuid, RunRecord>>>;

pub fn new_registry() -> RunRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: Uuid,
    pub config: String,
    /// User-requested scheduler override, if any -- distinct from the
    /// resolved scheduler name that ends up in `report.scheduler`.
    pub requested_scheduler: Option<String>,
    /// Scheduler this run shadows the primary against, if it's a shadow
    /// run at all -- `None` means an ordinary single-scheduler run.
    pub requested_shadow_scheduler: Option<String>,
    pub status: RunStatus,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub report: Option<SimulationReport>,
    /// The shadow scheduler's finished report, populated alongside `report`
    /// once a shadow run completes.
    pub shadow_report: Option<SimulationReport>,
}

impl RunRecord {
    pub fn new(
        id: Uuid,
        config: String,
        requested_scheduler: Option<String>,
        requested_shadow_scheduler: Option<String>,
    ) -> Self {
        Self {
            id,
            config,
            requested_scheduler,
            requested_shadow_scheduler,
            status: RunStatus::Pending,
            created_at: Utc::now(),
            finished_at: None,
            error: None,
            report: None,
            shadow_report: None,
        }
    }

    /// The scheduler name to show in list/detail responses: the user's
    /// explicit override if they gave one, else the resolved scheduler from
    /// the completed report -- matches Python's `run.scheduler or
    /// run.resolved_scheduler`.
    pub fn display_scheduler(&self) -> Option<String> {
        self.requested_scheduler
            .clone()
            .or_else(|| self.report.as_ref().map(|r| r.scheduler.clone()))
    }
}
