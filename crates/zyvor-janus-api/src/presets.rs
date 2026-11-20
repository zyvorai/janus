// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use serde::Serialize;

/// Static workload-preset descriptions, ported from
/// `python/zyvor_janus/workloads/generate_synthetic.py`'s `PRESETS` dict.
/// Only `id`/`description` are exposed via the API (matching
/// `benchmark_presets()` in the Python server) -- the full preset
/// definitions (arrival rate, token ranges, tenants) stay Python-side,
/// used only by the synthetic-workload generator CLI, not this API.
#[derive(Serialize)]
pub struct WorkloadPreset {
    pub id: &'static str,
    pub description: &'static str,
}

pub const PRESETS: &[WorkloadPreset] = &[
    WorkloadPreset {
        id: "morning_rag",
        description: "Low-rate RAG lookups with short outputs",
    },
    WorkloadPreset {
        id: "peak_chat",
        description: "High-rate chat traffic with mixed tenants",
    },
    WorkloadPreset {
        id: "night_training",
        description: "Sparse large-context requests overnight",
    },
];
