# Using the Dashboard

Zyvor Janus ships a CLI and a Next.js web console. Both consume the same simulation engine output.

## Shell chrome

| Element | Job |
|---------|-----|
| Sidebar / glass icons | Dashboard · Runs · Benchmark · What-if · Twins |
| Top bar brand | Jump home |
| Collapse control | Icon-only sidebar |
| Mobile bottom nav | Same primary destinations |

## Surfaces

| Need | Where |
|------|--------|
| Launch / compare configs | [Dashboard](pages/console/home.md) |
| Live scheduling replay | [Simulate](pages/console/simulate.md) |
| TTFT/TPS benchmarks | [Benchmark](pages/console/benchmark.md) |
| Scheduler sweeps | [What-if](pages/console/what-if.md) |
| AIPerf twins | [Twins](pages/console/twins.md) |
| One run | [Run detail](pages/console/runs-id.md) |

## Operate tips

1. Start the stack with `./scripts/run_web_dashboard.sh`, then sign in.
2. Prefer Dashboard **Run** for interactive work; use CLI for CI and scripts.
3. After a run, **Open in Simulate** before changing schedulers in What-if.
4. Never paste lab IPs into runbooks — use `<host>`.

## Related

- [Getting Started](getting-started.md)
- [Page index](PAGE_INDEX.md)
- [Admin basics](admin-basics.md)
