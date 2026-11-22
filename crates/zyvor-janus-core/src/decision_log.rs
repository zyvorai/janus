// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use serde::{Deserialize, Serialize};

/// A single schedulable moment recorded for replay and UI animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerDecision {
    pub time: f64,
    pub kind: String,
    pub job_id: Option<String>,
    pub job_name: Option<String>,
    pub gpu_ids: Vec<String>,
    pub message: String,
}

impl SchedulerDecision {
    pub fn new(time: f64, kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            time,
            kind: kind.into(),
            job_id: None,
            job_name: None,
            gpu_ids: Vec::new(),
            message: message.into(),
        }
    }

    pub fn with_job(mut self, job_id: &str, job_name: &str) -> Self {
        self.job_id = Some(job_id.to_string());
        self.job_name = Some(job_name.to_string());
        self
    }

    pub fn with_gpus(mut self, gpu_ids: Vec<String>) -> Self {
        self.gpu_ids = gpu_ids;
        self
    }
}
