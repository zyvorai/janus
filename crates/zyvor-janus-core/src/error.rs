// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimError {
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("gpu not found: {0}")]
    GpuNotFound(String),
    #[error("insufficient gpus: need {need}, have {available}")]
    InsufficientGpus { need: u32, available: u32 },
    #[error("invalid configuration: {0}")]
    Config(String),
}

pub type SimResult<T> = Result<T, SimError>;
