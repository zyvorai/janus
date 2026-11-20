# Contributing to Zyvor Janus

## Setup

Preferred (handles macOS Homebrew `pyexpat` quirks and builds the PyO3 extension):

```bash
./scripts/setup_dev.sh
source .venv/bin/activate
```

If `pyexpat` still fails: `USE_UV=1 ./scripts/setup_dev.sh`.

Optional extras:

```bash
pip install -e '.[viz]'        # Gantt / heatmaps
pip install -e '.[rl]'         # Gymnasium env + PPO baseline
pip install -e '.[server]'     # FastAPI / uvicorn (web API)
pip install -e '.[dashboard]'  # Rich CLI dashboard
```

Web UI:

```bash
cd web && npm install && cd ..
```

Manual Rust-only build (no Python extension):

```bash
cargo build --workspace --exclude zyvor-janus-py
```

## Before opening a PR

```bash
cargo fmt --all
cargo clippy --workspace --exclude zyvor-janus-py -- -D warnings
cargo test --workspace --exclude zyvor-janus-py
cargo test -p zyvor-janus-config --test integration
cargo test -p zyvor-janus-cli --test cli_integration
PYTHONPATH=python python3 -m unittest discover -s python/tests -v
```

When touching benchmark or CI fixtures:

```bash
bash benchmarks/ci/run_golden.sh
```

When changing the web UI, run `cd web && npm run build` locally if practical.

CI (`.github/workflows/rust.yml`, `python.yml`, `benchmark.yml`) runs checks on every push and PR to `main`.

## Project layout

See [docs/architecture.md](docs/architecture.md) for the crate layout and simulation loop, [docs/zynera_input.md](docs/zynera_input.md) for Zynera CRD mapping, [docs/ui_dashboard.md](docs/ui_dashboard.md) for dashboards, and [docs/benchmark_platform.md](docs/benchmark_platform.md) for the benchmark MVP.

Kubernetes deploy notes live in [deploy/kubernetes/README.md](deploy/kubernetes/README.md).

## Style

- Rust: standard `rustfmt` formatting, no `clippy` warnings.
- Prefer small, focused PRs tied to one milestone, phase, or fix — see [docs/milestones.md](docs/milestones.md).
- New Zynera field mappings must be documented in `docs/zynera_input.md`'s field mapping table.
- New HTTP APIs should be listed in `docs/ui_dashboard.md`.

## Reporting issues

Open a GitHub issue with a minimal repro (config/bundle/trace fixture if possible).
