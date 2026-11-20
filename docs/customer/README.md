# Zyvor Janus — Customer Documentation

**Zyvor Janus** is a discrete-event Kubernetes GPU cluster simulator — schedulers, MIG, topology, quotas, gang scheduling, and LLM serving metrics with no physical GPUs required. Operators use the **CLI** and an optional **web console**.

| You want to… | Open |
|--------------|------|
| Install and first run | [Getting Started](getting-started.md) |
| Learn the console shell | [Using the Dashboard](using-the-dashboard.md) |
| Screen-by-screen UX | [Page-by-page guides](pages/README.md) |
| Look up any route | [Complete page index](PAGE_INDEX.md) |
| Cluster YAML / Zynera bundles | [Configuration](configuration.md) |
| Ports, auth, systemd | [Admin basics](admin-basics.md) |
| Multi-step jobs | [Common workflows](workflows.md) |

## Printable PDFs

```bash
set -a; source scripts/customer-docs/product.env; set +a
node scripts/customer-docs/build-customer-pdfs.mjs
```

Output lands in [`pdf/`](pdf/).

## Product at a glance

```text
  CLI        →  cargo run -p zyvor-janus-cli -- run --config configs/clusters/….yaml
  Web UI     →  http://<host>:3000
  FastAPI    →  http://<host>:8080/api/health
  Login      →  Admin / Admin@321  (override with ZYVOR_JANUS_DASHBOARD_*)
```

Never publish lab IPs — use `<host>`.
