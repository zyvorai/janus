# Configuration

Cluster simulations are driven by YAML under `configs/clusters/` (or a Zynera export bundle).

## Typical layout

- **Cluster** — nodes, GPU SKUs, MIG / topology hints
- **Scheduler** — FIFO, priority, preemptive, or best-fit (and related policy knobs)
- **Workloads** — batch jobs and/or LLM serving traces
- **Quotas / gang** — when the scenario needs them

Start from a sample:

```bash
ls configs/clusters/
cargo run -p zyvor-janus-cli -- run --config configs/clusters/small_h100.yaml
```

In the web UI, the same config names appear in Dashboard **Launch simulation** and What-if / Benchmark pickers — the API must see the configs directory.

## Zynera bundles

Import/export paths follow the product Zynera integration docs. After import, treat the result like any other `--config` file for CLI and UI runs.

## Operate tip

Change one variable at a time, then use [What-if](pages/console/what-if.md) or Dashboard **Compare configs** instead of eyeballing two full CLI logs.

## Related

- [Getting Started](getting-started.md)
- [Workflows](workflows.md)
- [Dashboard](pages/console/home.md)
