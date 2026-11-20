// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use std::collections::HashSet;

/// Minimal per-GPU topology facts needed to compute a runtime multiplier,
/// decoupled from the `Cluster`/`Job` domain model so this module has no
/// dependency on it.
#[derive(Debug, Clone)]
pub struct GpuTopologyKey {
    pub nvlink_group: Option<u32>,
    pub node_id: String,
}

/// Synthetic cluster topology derived from hardware profile bandwidth constants.
#[derive(Debug, Clone)]
pub struct TopologyGraph {
    pub nvlink_bw_gbs: f64,
    pub pcie_bw_gbs: f64,
}

impl Default for TopologyGraph {
    fn default() -> Self {
        Self {
            nvlink_bw_gbs: 900.0,
            pcie_bw_gbs: 64.0,
        }
    }
}

impl TopologyGraph {
    pub fn from_profile_bandwidths(nvlink_bw_gbs: f64, pcie_bw_gbs: f64) -> Self {
        Self {
            nvlink_bw_gbs: nvlink_bw_gbs.max(1.0),
            pcie_bw_gbs: pcie_bw_gbs.max(1.0),
        }
    }

    /// Inflate runtime when a topology-aware job spans NVLink domains or nodes.
    pub fn runtime_multiplier(
        &self,
        gpu_topology: &[GpuTopologyKey],
        job_gang_enabled: bool,
        job_network_bw_gbps: Option<f64>,
        used_topology_fallback: bool,
    ) -> f64 {
        if !used_topology_fallback && !spans_multiple_domains(gpu_topology) {
            return 1.0;
        }
        if !job_gang_enabled && job_network_bw_gbps.is_none() && !used_topology_fallback {
            return 1.0;
        }

        let groups = distinct_nvlink_groups(gpu_topology);
        let nodes = distinct_nodes(gpu_topology);
        if groups <= 1 && nodes <= 1 {
            return 1.0;
        }

        let mut multiplier = 1.0;
        if groups > 1 {
            let pcie_ratio = self.pcie_bw_gbs / self.nvlink_bw_gbs;
            multiplier += (groups - 1) as f64 * pcie_ratio * 0.25;
        }
        if nodes > 1 {
            let inter_node_penalty = if let Some(req) = job_network_bw_gbps {
                (400.0 / req.max(1.0)).max(1.0) * 0.1
            } else {
                0.15
            };
            multiplier += (nodes - 1) as f64 * inter_node_penalty;
        }
        multiplier
    }
}

fn distinct_nvlink_groups(gpu_topology: &[GpuTopologyKey]) -> usize {
    gpu_topology
        .iter()
        .map(|k| k.nvlink_group)
        .collect::<HashSet<_>>()
        .len()
}

fn distinct_nodes(gpu_topology: &[GpuTopologyKey]) -> usize {
    gpu_topology
        .iter()
        .map(|k| k.node_id.as_str())
        .collect::<HashSet<_>>()
        .len()
}

fn spans_multiple_domains(gpu_topology: &[GpuTopologyKey]) -> bool {
    distinct_nvlink_groups(gpu_topology) > 1 || distinct_nodes(gpu_topology) > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_domain_has_unit_multiplier() {
        let gpu_topology = vec![
            GpuTopologyKey {
                nvlink_group: Some(0),
                node_id: "n0".into(),
            },
            GpuTopologyKey {
                nvlink_group: Some(0),
                node_id: "n0".into(),
            },
        ];
        let topo = TopologyGraph::default();
        let mult = topo.runtime_multiplier(&gpu_topology, false, Some(200.0), false);
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn cross_domain_inflates_runtime() {
        let gpu_topology = vec![
            GpuTopologyKey {
                nvlink_group: Some(0),
                node_id: "n0".into(),
            },
            GpuTopologyKey {
                nvlink_group: Some(1),
                node_id: "n0".into(),
            },
        ];
        let topo = TopologyGraph::from_profile_bandwidths(900.0, 64.0);
        let mult = topo.runtime_multiplier(&gpu_topology, false, Some(200.0), true);
        assert!(mult > 1.0);
    }
}
