// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

/// Port of Python's `ProfileRegistry.lookup_v2`
/// (`python/zyvor_janus/adapters/profiles.py`), scoped to what the OpenAI
/// shim's TTFT estimate needs. Loaded once at startup (matching Python's
/// module-level `_profile_registry`), not re-read per request.
pub struct ProfileRegistry {
    profiles: HashMap<String, HashMap<String, GpuProfileEntry>>,
}

#[derive(Debug, Default, Deserialize)]
struct GpuProfileEntry {
    prefill_ms_per_token: Option<f64>,
    decode_tps: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProfileFile {
    model: Option<String>,
    #[serde(default)]
    profiles: HashMap<String, GpuProfileEntry>,
}

const DEFAULT_PREFILL_MS_PER_TOKEN: f64 = 0.08;
const DEFAULT_DECODE_TPS: f64 = 120.0;

impl ProfileRegistry {
    pub fn load(dir: &Path) -> Self {
        let mut profiles = HashMap::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Self { profiles };
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
            .collect();
        paths.sort();

        for path in paths {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = serde_yaml::from_str::<ProfileFile>(&content) else {
                continue;
            };
            if let Some(model) = file.model {
                profiles.insert(model, file.profiles);
            }
        }
        Self { profiles }
    }

    /// Returns `(prefill_ms_per_token, decode_tps)`, falling back to the
    /// same defaults Python uses whenever the model, the GPU type (or its
    /// `_`-prefix, matching Python's `gpu_type.split("_")[0]` fallback), or
    /// the specific field is missing.
    pub fn lookup_v2(&self, model: &str, gpu_type: &str) -> (f64, f64) {
        let entry = self.profiles.get(model).and_then(|by_gpu| {
            by_gpu.get(gpu_type).or_else(|| {
                let base = gpu_type.split('_').next().unwrap_or(gpu_type);
                by_gpu.get(base)
            })
        });
        match entry {
            Some(e) => (
                e.prefill_ms_per_token
                    .unwrap_or(DEFAULT_PREFILL_MS_PER_TOKEN),
                e.decode_tps.unwrap_or(DEFAULT_DECODE_TPS),
            ),
            None => (DEFAULT_PREFILL_MS_PER_TOKEN, DEFAULT_DECODE_TPS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_defaults_for_unknown_model() {
        let registry = ProfileRegistry {
            profiles: HashMap::new(),
        };
        assert_eq!(
            registry.lookup_v2("nope", "H100"),
            (DEFAULT_PREFILL_MS_PER_TOKEN, DEFAULT_DECODE_TPS)
        );
    }

    #[test]
    fn uses_calibrated_values_when_present() {
        let mut by_gpu = HashMap::new();
        by_gpu.insert(
            "H100".to_string(),
            GpuProfileEntry {
                prefill_ms_per_token: Some(0.12),
                decode_tps: Some(95.0),
            },
        );
        let mut profiles = HashMap::new();
        profiles.insert("llama-70b".to_string(), by_gpu);
        let registry = ProfileRegistry { profiles };
        assert_eq!(registry.lookup_v2("llama-70b", "H100"), (0.12, 95.0));
    }

    #[test]
    fn falls_back_to_gpu_family_prefix() {
        let mut by_gpu = HashMap::new();
        by_gpu.insert(
            "H100".to_string(),
            GpuProfileEntry {
                prefill_ms_per_token: Some(0.12),
                decode_tps: Some(95.0),
            },
        );
        let mut profiles = HashMap::new();
        profiles.insert("llama-70b".to_string(), by_gpu);
        let registry = ProfileRegistry { profiles };
        assert_eq!(registry.lookup_v2("llama-70b", "H100_MIG_1g"), (0.12, 95.0));
    }

    #[test]
    fn missing_fields_in_a_present_entry_fall_back_to_defaults() {
        let mut by_gpu = HashMap::new();
        by_gpu.insert("H100".to_string(), GpuProfileEntry::default());
        let mut profiles = HashMap::new();
        profiles.insert("gpt-13b".to_string(), by_gpu);
        let registry = ProfileRegistry { profiles };
        assert_eq!(
            registry.lookup_v2("gpt-13b", "H100"),
            (DEFAULT_PREFILL_MS_PER_TOKEN, DEFAULT_DECODE_TPS)
        );
    }
}
