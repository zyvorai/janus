# M5 — Topology (scoping)

Status: **done** (domain grouping + runtime inflation). Domain-level NVLink
grouping with scatter fallback, `topology_penalties`, and
`topology_runtime_inflation` when jobs span domains. See
[`topology.rs`](../../crates/zyvor-janus-core/src/topology.rs) and
[`resource.rs`](../../crates/zyvor-janus-core/src/resource.rs).

## What exists today

Implemented in `zyvor-janus-core`:

- `Gpu.nvlink_group: Option<u32>` — set from cluster `topology_template`
  (`nvlink_pairs`, `full_mesh`, `pcie_only`) or explicit GPU spec; Zynera
  bundle still defaults to `i / 2` pairing when no template is set.
- `HardwareProfile.nvlink_bw_gbs` / `pcie_bw_gbs` — feed `TopologyGraph` for
  runtime inflation when jobs span NVLink domains or nodes.
- `Job.network_bw_gbps` — jobs with this set (or `gang_enabled`) prefer
  same-`nvlink_group` placement in `ResourceManager::can_place_whole_gpu`.
- Scatter fallback increments `topology_penalties`; cross-domain placement
  inflates job duration via `Placement.runtime_multiplier`
  (`topology_runtime_inflation` in metrics).

## Goal

Model NVLink/PCIe (intra-node) and RDMA/network (inter-node) topology as a
graph derived from `FabricGpuNode`, and make placement locality-aware so
Zyvor Janus can answer: *does this scheduler colocate gang/distributed jobs on
well-connected GPUs, and does that matter for runtime?*

## Resolved decisions

1. **Topology source**: synthetic templates via `ClusterConfig.topology_template`
   until Zynera exports per-GPU adjacency from CRDs.
2. **Graph granularity**: domain-level `nvlink_group` (implemented); per-GPU
   adjacency is a future refinement.
3. **Scoring vs. hard constraint**: prefer same domain, fall back with
   `topology_penalties` and `runtime_multiplier` inflation (implemented).
4. **Location**: `TopologyGraph` in `zyvor-janus-core`, placement in
   `ResourceManager`, gang spread uses NVLink-aware per-node GPU pick.

## Future refinements

- Real adjacency from Zynera CRDs when available.
- Per-GPU topology graph for NVSwitch mesh modeling.
- Optional hard constraint mode (reject cross-domain placement).

## Open questions (historical — see Resolved decisions above)

1. ~~**Where does real topology come from?**~~ Synthetic templates for now.

## Suggested first slice

~~Domain-level grouping + hard-constraint-with-fallback…~~ **Done** — see
“What exists today” and [milestones.md](../milestones.md) M5 criteria.
Future work is under **Future refinements** above.

## Non-goals for M5

- Real NVLink/PCIe topology *export* from a live Zynera cluster (depends on
  what Zynera's CRDs actually expose — a prerequisite question, not part of
  Zyvor Janus itself).
- Multi-rack / multi-datacenter network modeling — single-cluster only, per
  the existing `Cluster` model.
