# Getting Started with Zyvor Janus

Zyvor Janus replays GPU schedulers against a cluster config — no physical GPUs required. This page gets you to a first CLI run and an optional web console session.

## Prerequisites

| Requirement | Notes |
|-------------|--------|
| Rust toolchain | `cargo` to build `zyvor-janus-cli` |
| Cluster config YAML | `configs/clusters/` or a Zynera CRD export bundle |
| Node.js (optional) | Only for the Next.js web dashboard |

## 1. Clone and build

```bash
git clone https://github.com/zyvorai/janus.git
cd zyvor-janus   # or your checkout path
cargo build -p zyvor-janus-cli
```

## 2. First simulation (CLI)

```bash
cargo run -p zyvor-janus-cli -- run --config configs/clusters/small_h100.yaml
```

Scheduler choice (FIFO, priority, preemptive, best-fit) comes from the config. LLM-serving workloads also report TTFT/TPS.

## 3. Optional — start the web console

```bash
./scripts/run_web_dashboard.sh
```

| URL | Service |
|-----|---------|
| `http://<host>:3000` | Next.js UI |
| `http://<host>:8080/api/health` | FastAPI backend |

Sign in (**Admin** / **Admin@321** by default) → [Dashboard](pages/console/home.md) → **Run**.

Override ports: `API_PORT=9000 UI_PORT=3001 ./scripts/run_web_dashboard.sh`.

## 4. Orient yourself (UX)

1. Sidebar: Dashboard · Runs · Benchmark · What-if · Twins.
2. Launch a run from Dashboard; open **Run detail** then **Simulate** to replay decisions.
3. Use **What-if** to sweep schedulers; **Benchmark** for TTFT/TPS score vectors.

## Troubleshooting

- **`cargo run` can't find the config** — `--config` is relative to your cwd under `configs/clusters/`.
- **No TTFT/TPS** — only LLM-serving workloads emit those metrics.
- **Dashboard empty** — confirm FastAPI health and that CLI/UI share the same outputs path.

## Next steps

- [Using the Dashboard](using-the-dashboard.md)
- [Configuration](configuration.md)
- [Page guides](pages/README.md)
- [Workflows](workflows.md)
