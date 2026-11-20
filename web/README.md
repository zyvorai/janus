# Zyvor Janus Web Dashboard

Next.js frontend for monitoring Zyvor Janus simulations and benchmark scenarios.

**Full documentation:** [../docs/ui_dashboard.md](../docs/ui_dashboard.md)

## Quick start

From the repo root:

```bash
./scripts/setup_dev.sh
cd web && npm install && cd ..
./scripts/run_web_dashboard.sh
```

Open http://localhost:3000

## Routes

| Path | Purpose |
|------|---------|
| `/` | Run simulations, recent runs, compare |
| `/login` | Dashboard auth |
| `/benchmark` | Scheduler benchmark hub (TTFT/TPS/score) |
| `/what-if` | Cluster × scheduler sweeps |
| `/runs/:id` | **Not shipped** — home may link here; use API artifacts under `outputs/runs/{id}/` |

## Scripts

| Script | Purpose |
|--------|---------|
| `./scripts/run_web_dashboard.sh` | API (8080) + UI (3000) together |
| `./scripts/stop_web_dashboard.sh` | Stop API + UI servers |
| `./scripts/run_web_api.sh` | FastAPI backend only |
| `./scripts/run_web_ui.sh` | Next.js frontend only |

Custom ports: `API_PORT=9000 UI_PORT=3001 ./scripts/run_web_dashboard.sh`

## Stack

- Next.js 14, React, TypeScript, Tailwind CSS
- Recharts (metrics), React Flow (topology), Zustand (replay state)
- Proxies `/api` and `/ws` to FastAPI on port 8080
- OpenAI-compatible shim is served by the API at `/v1` (see [docs/openai_shim.md](../docs/openai_shim.md))

## Brand / theming

The web UI matches the [Zyvor](https://zyvor.dev) / HyperSDK dark palette:

| Token | Value | Usage |
|-------|-------|-------|
| `--hs-bg` | `#050505` | Page background |
| `--hs-accent` | `#f0583a` | Primary buttons, links |
| `--hs-indigo` | `#6366f1` | Active / busy GPU state |
| `--hs-teal` | `#10b981` | Gantt run segments |

- CSS variables: [`src/styles/zyvor-tokens.css`](src/styles/zyvor-tokens.css)
- Tailwind mapping: [`tailwind.config.ts`](tailwind.config.ts)
- Chart/topology constants: [`src/lib/theme.ts`](src/lib/theme.ts)
- Shared Python palette (CLI + matplotlib): [`../python/zyvor_janus/theme.py`](../python/zyvor_janus/theme.py)

Fonts: **Inter** (UI), **JetBrains Mono** (metrics). Header uses the Zyvor logo from `public/zyvor-logo.png`.

## Login

Default credentials (override via env for production):

| Field | Default |
|-------|---------|
| Username | `Admin` (`ZYVOR_JANUS_DASHBOARD_USER`) |
| Password | `Admin@321` (`ZYVOR_JANUS_DASHBOARD_PASSWORD`) |

```bash
./scripts/run_web_dashboard.sh
# or override:
ZYVOR_JANUS_DASHBOARD_USER=Admin ZYVOR_JANUS_DASHBOARD_PASSWORD='your-secret' ./scripts/run_web_dashboard.sh
```

Sessions use an httpOnly cookie (7-day expiry). Unauthenticated users are redirected to `/login`.

## Deploy

Container/Kubernetes install: [../deploy/kubernetes/README.md](../deploy/kubernetes/README.md).

## Development

See [docs/ui_dashboard.md](../docs/ui_dashboard.md) for API endpoints, run artifacts, troubleshooting, and production build notes.
