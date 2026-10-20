// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

//! GPU cost model -- seed crate for future multi-provider cloud pricing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    #[serde(default = "default_gpu_hour_usd")]
    pub gpu_hour_usd: f64,
}

fn default_gpu_hour_usd() -> f64 {
    3.50
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            gpu_hour_usd: default_gpu_hour_usd(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_serde_default() {
        assert_eq!(CostModel::default().gpu_hour_usd, 3.50);
        let parsed: CostModel = serde_yaml::from_str("{}").unwrap();
        assert_eq!(parsed.gpu_hour_usd, CostModel::default().gpu_hour_usd);
    }
}
