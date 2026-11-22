// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Analytical inference performance model for LLM serving jobs.
//!
//! Estimates TTFT, decode duration, and end-to-end runtime from token counts,
//! batch size, concurrency, and per-(model, gpu) calibration curves.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceProfile {
    pub model: String,
    pub gpu_type: String,
    /// Milliseconds per input token during prefill.
    #[serde(default = "default_prefill_ms")]
    pub prefill_ms_per_token: f64,
    /// Decode tokens per second per GPU replica.
    #[serde(default = "default_decode_tps")]
    pub decode_tps: f64,
    #[serde(default = "default_max_batch")]
    pub max_batch: u32,
}

fn default_prefill_ms() -> f64 {
    0.08
}

fn default_decode_tps() -> f64 {
    120.0
}

fn default_max_batch() -> u32 {
    32
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InferenceEstimate {
    /// Time to first token in seconds (prefill only).
    pub ttft_secs: f64,
    /// Decode phase duration in seconds.
    pub decode_secs: f64,
    /// Total GPU occupancy duration in seconds.
    pub runtime_secs: f64,
    /// Effective decode tokens/sec for this request.
    pub tps: f64,
    /// Mean inter-token latency in seconds during decode.
    pub itl_secs: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct InferenceRequest {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub batch_size: u32,
    pub concurrency: u32,
}

impl InferenceRequest {
    pub fn from_job(job: &zyvor_janus_model::models::Job) -> Option<Self> {
        let input_tokens = job.input_tokens?;
        let output_tokens = job.output_tokens?;
        Some(Self {
            input_tokens,
            output_tokens,
            batch_size: job.batch_size.unwrap_or(1).max(1),
            concurrency: job.concurrency.unwrap_or(1).max(1),
        })
    }
}

pub fn estimate_inference(profile: &InferenceProfile, req: InferenceRequest) -> InferenceEstimate {
    let batch = req.batch_size.max(1) as f64;
    let concurrency = req.concurrency.max(1) as f64;
    let input = req.input_tokens.max(1) as f64;
    let output = req.output_tokens as f64;

    let batch_penalty = 1.0 + (batch - 1.0).max(0.0) * 0.015;
    let concurrency_penalty = 1.0 + (concurrency - 1.0).max(0.0) * 0.05;

    let ttft_secs =
        (input * profile.prefill_ms_per_token / 1000.0) * batch_penalty * concurrency_penalty;

    let effective_tps = (profile.decode_tps / batch_penalty / concurrency_penalty).max(1.0);
    let decode_secs = if output > 0.0 {
        output / effective_tps
    } else {
        0.0
    };

    let runtime_secs = ttft_secs + decode_secs;
    let itl_secs = if output > 1.0 {
        decode_secs / (output - 1.0)
    } else {
        0.0
    };

    InferenceEstimate {
        ttft_secs,
        decode_secs,
        runtime_secs,
        tps: effective_tps,
        itl_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> InferenceProfile {
        InferenceProfile {
            model: "llama-70b".into(),
            gpu_type: "H100".into(),
            prefill_ms_per_token: 0.1,
            decode_tps: 100.0,
            max_batch: 32,
        }
    }

    #[test]
    fn more_tokens_increase_runtime() {
        let small = estimate_inference(
            &profile(),
            InferenceRequest {
                input_tokens: 100,
                output_tokens: 50,
                batch_size: 1,
                concurrency: 1,
            },
        );
        let large = estimate_inference(
            &profile(),
            InferenceRequest {
                input_tokens: 1000,
                output_tokens: 500,
                batch_size: 1,
                concurrency: 1,
            },
        );
        assert!(large.runtime_secs > small.runtime_secs);
        assert!(large.ttft_secs > small.ttft_secs);
    }

    #[test]
    fn higher_concurrency_increases_ttft() {
        let low = estimate_inference(
            &profile(),
            InferenceRequest {
                input_tokens: 500,
                output_tokens: 100,
                batch_size: 1,
                concurrency: 1,
            },
        );
        let high = estimate_inference(
            &profile(),
            InferenceRequest {
                input_tokens: 500,
                output_tokens: 100,
                batch_size: 1,
                concurrency: 8,
            },
        );
        assert!(high.ttft_secs > low.ttft_secs);
    }
}
