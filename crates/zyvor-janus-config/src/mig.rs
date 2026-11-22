// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Load MIG profile definitions keyed by hardware profile name.

use std::fs;
use std::path::Path;

use zyvor_janus_model::{MigHardwareConfig, MigProfileRegistry};

use crate::{ConfigError, ConfigResult};

pub fn load_mig_registry(path: &Path) -> ConfigResult<MigProfileRegistry> {
    let content = fs::read_to_string(path)?;
    let config: MigHardwareConfig = serde_yaml::from_str(&content)?;
    Ok(MigProfileRegistry::from_config(config))
}

pub fn load_mig_registry_for_hardware(
    dir: &Path,
    hardware_name: &str,
) -> ConfigResult<Option<MigProfileRegistry>> {
    if !dir.exists() {
        return Ok(None);
    }
    let path = dir.join(format!("{}.yaml", hardware_name.to_lowercase()));
    if !path.exists() {
        let alt = dir.join(format!(
            "{}.yaml",
            hardware_name.replace('_', "").to_lowercase()
        ));
        if alt.exists() {
            return Ok(Some(load_mig_registry(&alt)?));
        }
        return Ok(None);
    }
    Ok(Some(load_mig_registry(&path)?))
}

pub fn resolve_mig_registry_for_cluster(
    dir: &Path,
    hardware_names: &[String],
    any_mig_capable: bool,
) -> ConfigResult<Option<MigProfileRegistry>> {
    if !any_mig_capable {
        return Ok(None);
    }
    for name in hardware_names {
        if let Some(registry) = load_mig_registry_for_hardware(dir, name)? {
            return Ok(Some(registry));
        }
    }
    Err(ConfigError::Invalid(format!(
        "cluster has MIG-capable GPUs but no MIG profiles found in {}",
        dir.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_h100_mig_profiles() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/mig/h100_80gb.yaml");
        if !path.exists() {
            return;
        }
        let registry = load_mig_registry(&path).unwrap();
        assert!(registry.profile("1g.10gb").is_ok());
    }
}
