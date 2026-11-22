// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Multi-Instance GPU (MIG) partition and reconfiguration model.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::{Gpu, MigSlice};
use zyvor_janus_core::error::{SimError, SimResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigProfileSpec {
    pub memory_gb: f64,
    pub max_per_gpu: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigHardwareConfig {
    pub hardware: String,
    #[serde(default = "default_reconfig_seconds")]
    pub reconfig_seconds: f64,
    pub profiles: HashMap<String, MigProfileSpec>,
}

fn default_reconfig_seconds() -> f64 {
    30.0
}

#[derive(Debug, Clone)]
pub struct MigProfileRegistry {
    pub reconfig_seconds: f64,
    profiles: HashMap<String, MigProfileSpec>,
}

impl MigProfileRegistry {
    pub fn from_config(config: MigHardwareConfig) -> Self {
        Self {
            reconfig_seconds: config.reconfig_seconds,
            profiles: config.profiles,
        }
    }

    pub fn profile(&self, name: &str) -> SimResult<&MigProfileSpec> {
        self.profiles
            .get(name)
            .ok_or_else(|| SimError::Config(format!("unknown MIG profile '{name}'")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuMigMode {
    WholeGpu,
    Partitioned(String),
}

impl Gpu {
    pub fn mig_mode(&self) -> GpuMigMode {
        if !self.mig_capable || self.slices.is_empty() {
            GpuMigMode::WholeGpu
        } else {
            GpuMigMode::Partitioned(self.active_mig_profile.clone().unwrap_or_default())
        }
    }

    pub fn is_whole_gpu_free(&self) -> bool {
        self.running_job_id.is_none() && self.slices.is_empty()
    }

    pub fn is_fully_idle(&self) -> bool {
        if !self.mig_capable {
            return self.is_free();
        }
        if !self.slices.is_empty() {
            return self.slices.iter().all(|s| s.running_job_id.is_none());
        }
        self.running_job_id.is_none()
    }

    pub fn free_mig_slice_count(&self, profile: &str) -> u32 {
        self.slices
            .iter()
            .filter(|s| s.profile == profile && s.running_job_id.is_none())
            .count() as u32
    }
}

pub fn apply_mig_layout(gpu: &mut Gpu, profile: &str, count: u32, spec: &MigProfileSpec) {
    gpu.running_job_id = None;
    gpu.slices.clear();
    gpu.active_mig_profile = Some(profile.to_string());
    for i in 0..count {
        gpu.slices.push(MigSlice {
            id: format!("{}-mig-{i}", gpu.id),
            profile: profile.to_string(),
            memory_gb: spec.memory_gb,
            running_job_id: None,
        });
    }
}

pub fn reset_gpu_to_whole(gpu: &mut Gpu) {
    gpu.slices.clear();
    gpu.active_mig_profile = None;
    gpu.running_job_id = None;
}

pub fn reconfigure_gpu(
    gpu: &mut Gpu,
    profile: &str,
    count: u32,
    registry: &MigProfileRegistry,
) -> SimResult<()> {
    if !gpu.mig_capable {
        return Err(SimError::Config(format!(
            "gpu '{}' is not MIG-capable",
            gpu.id
        )));
    }
    if !gpu.is_fully_idle() {
        return Err(SimError::Config(format!(
            "gpu '{}' is busy; cannot reconfigure MIG layout",
            gpu.id
        )));
    }
    let spec = registry.profile(profile)?;
    if count > spec.max_per_gpu {
        return Err(SimError::Config(format!(
            "profile '{profile}' allows at most {} slices per gpu, requested {count}",
            spec.max_per_gpu
        )));
    }
    apply_mig_layout(gpu, profile, count, spec);
    Ok(())
}

pub fn find_reconfigurable_gpu<'a>(
    cluster_gpus: impl Iterator<Item = &'a Gpu>,
    profile: &str,
    needed: u32,
    registry: &MigProfileRegistry,
) -> Option<&'a Gpu> {
    let spec = registry.profile(profile).ok()?;
    cluster_gpus
        .filter(|g| g.mig_capable && g.is_fully_idle())
        .find(|g| {
            if g.free_mig_slice_count(profile) >= needed {
                return true;
            }
            if !g.is_fully_idle() {
                return false;
            }
            if needed > spec.max_per_gpu {
                return false;
            }
            matches!(g.mig_mode(), GpuMigMode::WholeGpu) || {
                matches!(g.mig_mode(), GpuMigMode::Partitioned(ref active) if active != profile)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> MigProfileRegistry {
        MigProfileRegistry::from_config(MigHardwareConfig {
            hardware: "H100_80GB".into(),
            reconfig_seconds: 30.0,
            profiles: HashMap::from([
                (
                    "1g.10gb".into(),
                    MigProfileSpec {
                        memory_gb: 10.0,
                        max_per_gpu: 7,
                    },
                ),
                (
                    "7g.80gb".into(),
                    MigProfileSpec {
                        memory_gb: 80.0,
                        max_per_gpu: 1,
                    },
                ),
            ]),
        })
    }

    fn mig_gpu(id: &str) -> Gpu {
        Gpu {
            id: id.into(),
            node_id: "node-0".into(),
            profile: "H100_80GB".into(),
            memory_gb: 80.0,
            nvlink_group: None,
            running_job_id: None,
            mig_capable: true,
            active_mig_profile: None,
            slices: Vec::new(),
        }
    }

    #[test]
    fn reconfigure_creates_slices() {
        let reg = registry();
        let mut gpu = mig_gpu("gpu-0");
        reconfigure_gpu(&mut gpu, "1g.10gb", 3, &reg).unwrap();
        assert_eq!(gpu.slices.len(), 3);
        assert_eq!(gpu.slices[0].profile, "1g.10gb");
    }

    #[test]
    fn rejects_over_max_slices() {
        let reg = registry();
        let mut gpu = mig_gpu("gpu-0");
        assert!(reconfigure_gpu(&mut gpu, "1g.10gb", 8, &reg).is_err());
    }
}
