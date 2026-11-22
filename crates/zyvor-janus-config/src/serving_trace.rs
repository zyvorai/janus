// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! serving.trace.v1 import/export — separate from M3 scheduler traces.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, JobState};

use crate::{ConfigError, ConfigResult};

pub const SERVING_TRACE_VERSION: &str = "serving.trace.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServingTraceRecord {
    pub time: f64,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub batch_size: u32,
    #[serde(default)]
    pub concurrency: u32,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub tenant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServingTraceFile {
    pub version: String,
    pub records: Vec<ServingTraceRecord>,
}

impl ServingTraceFile {
    pub fn new(records: Vec<ServingTraceRecord>) -> Self {
        Self {
            version: SERVING_TRACE_VERSION.into(),
            records,
        }
    }
}

pub fn load_serving_trace(path: &Path) -> ConfigResult<ServingTraceFile> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "json" {
        let content = fs::read_to_string(path)?;
        let file: ServingTraceFile = serde_json::from_str(&content)?;
        validate_serving_trace(&file)?;
        return Ok(file);
    }

    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        records.push(serde_json::from_str(trimmed)?);
    }
    let file = ServingTraceFile::new(records);
    validate_serving_trace(&file)?;
    Ok(file)
}

pub fn validate_serving_trace(file: &ServingTraceFile) -> ConfigResult<()> {
    if file.version != SERVING_TRACE_VERSION {
        return Err(ConfigError::Invalid(format!(
            "unsupported serving trace version '{}'",
            file.version
        )));
    }
    for (idx, rec) in file.records.iter().enumerate() {
        if rec.time < 0.0 {
            return Err(ConfigError::Invalid(format!(
                "record {idx}: negative arrival time"
            )));
        }
        if rec.input_tokens == 0 && rec.output_tokens == 0 {
            return Err(ConfigError::Invalid(format!(
                "record {idx}: must have input or output tokens"
            )));
        }
        if rec.model.trim().is_empty() {
            return Err(ConfigError::Invalid(format!("record {idx}: missing model")));
        }
    }
    Ok(())
}

pub fn jobs_from_serving_trace(trace: &ServingTraceFile, id_prefix: &str) -> Vec<Job> {
    trace
        .records
        .iter()
        .enumerate()
        .map(|(idx, rec)| {
            let id = rec
                .request_id
                .clone()
                .unwrap_or_else(|| format!("{id_prefix}-{idx}"));
            let mut job = Job::new(id, rec.model.clone(), rec.time, 1.0, 1);
            job.model_id = Some(rec.model.clone());
            job.input_tokens = Some(rec.input_tokens);
            job.output_tokens = Some(rec.output_tokens);
            job.batch_size = Some(rec.batch_size.max(1));
            job.concurrency = Some(rec.concurrency.max(1));
            job.tenant = rec.tenant.clone();
            job
        })
        .collect()
}

pub fn export_serving_trace_from_cluster(cluster: &Cluster) -> ServingTraceFile {
    let mut records = Vec::new();
    for job in &cluster.finished_jobs {
        if job.state != JobState::Finished {
            continue;
        }
        let Some(model) = job.model_id.clone() else {
            continue;
        };
        let Some(input_tokens) = job.input_tokens else {
            continue;
        };
        let Some(output_tokens) = job.output_tokens else {
            continue;
        };
        records.push(ServingTraceRecord {
            time: job.arrival_time,
            model,
            input_tokens,
            output_tokens,
            batch_size: job.batch_size.unwrap_or(1),
            concurrency: job.concurrency.unwrap_or(1),
            request_id: Some(job.id.clone()),
            tenant: job.tenant.clone(),
        });
    }
    records.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ServingTraceFile::new(records)
}

pub fn write_serving_trace_jsonl(path: &Path, trace: &ServingTraceFile) -> ConfigResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    for rec in &trace.records {
        let line = serde_json::to_string(rec)?;
        writeln!(file, "{line}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_import_export_fields() {
        let trace = ServingTraceFile::new(vec![ServingTraceRecord {
            time: 1.5,
            model: "llama-70b".into(),
            input_tokens: 512,
            output_tokens: 128,
            batch_size: 1,
            concurrency: 2,
            request_id: Some("req-1".into()),
            tenant: Some("team-a".into()),
        }]);
        let jobs = jobs_from_serving_trace(&trace, "srv");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].model_id.as_deref(), Some("llama-70b"));
        assert_eq!(jobs[0].input_tokens, Some(512));
        assert_eq!(jobs[0].concurrency, Some(2));
    }
}
