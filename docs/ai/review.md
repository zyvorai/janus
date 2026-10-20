# AI review (janus)

_Generated 2026-10-20 by claude-opus-5-5 from a snapshot of tracked sources; verify before acting._

# Code Review: Zyvor Janus

## Summary

Janus is a discrete-event simulator for GPU scheduling on Kubernetes. The core is a Rust workspace covering events, the cluster/MIG/topology model, schedulers, metrics, the CLI and an axum API. On top sit PyO3 bindings, a Gymnasium environment with a PPO baseline, matplotlib and Rich visualisation, and a legacy FastAPI server. A Next.js dashboard uses session-cookie auth and proxies to the API. Kubernetes manifests and GHCR images cover deployment.

## Bugs and correctness risks

**Dashboard breaks behind the shipped Ingress.**
- `deploy/kubernetes/ingress.yaml` sends `/api` straight to `zyvor-janus-api`.
- The API requires `Authorization: Bearer …` (`crates/zyvor-janus-api/src/auth.rs`).
- The browser only holds the session cookie. The bearer token is injected only in `web/src/middleware.ts`, which this route bypasses.
- Result: every dashboard `/api/*` call through the Ingress should return 401.
- `deploy/kubernetes/README.md` calls this routing "safe", which is true for security but means the UI doesn't work. Port-forward and NodePort paths that go via the web pod are unaffected.

**Matplotlib theme likely crashes plotting.**
- `python/zyvor_janus/theme.py` sets `grid.color` and `axes.edgecolor` (via `BORDER`) to `"rgba(255,255,255,0.06)"`. This is a CSS color string, not a matplotlib color.
- `plt.rcParams.update(...)` should raise `ValueError`, breaking `plot_gantt`, `plot_gpu_heatmap` and `save_run_figures`.
- CI never installs matplotlib, so `test_viz.py` skips the affected tests and this is hidden.

**AIPerf import overwrites calibrated data.**
- `update_profile_yaml` in `benchmarks/aiperf_adapter.py` unconditionally writes `runtime_seconds=3600` and `gpu_memory_gb=80` into an existing entry. For example, it would clobber llama-70b/H100's `3480`.
- It also hand-serialises the YAML, dropping comments and doing no quoting.
- `extract_profile_curves` hard-codes a divisor of 512 input tokens to derive `prefill_ms_per_token`, regardless of the real prompt length.

**Composite score normalisation is broken for TPS.**
- `normalize_score` in `crates/zyvor-janus-metrics/src/benchmark.rs` applies `value.min(1.0)` to `tps_mean`. Any realistic TPS (e.g. 95) saturates to 1.0, so the term carries no information.
- `jain_index` measures fairness by job count per tenant, not by GPU-seconds share, which is a weak fairness signal.

**Silent hardware substitution.**
- `configs/gpu_type_registry.yaml` maps `A100`, `A100-80G`, `L40` and `L40S` to `H100_80GB`, so A100/L40 jobs run with H100 characteristics.
- `resolve_mig_registry_for_cluster` in `crates/zyvor-janus-config/src/mig.rs` returns the first MIG registry it finds and applies it cluster-wide. Mixed MIG fleets (H100, H200, A100_40GB) would share one slice table.
- Only `configs/mig/h100_80gb.yaml` is visible. I can't confirm how H200 and A100 MIG configs behave.

**Visualisation ignores preemption.**
- `viz/gantt.py` and `viz/heatmap.py` draw a single start→finish run on `assigned_gpus`.
- The timeline carries `segments` (see `web/src/types/simulation.ts`), which these plots ignore. Preempted or migrated jobs are therefore drawn incorrectly.

**Event ordering.**
- `crates/zyvor-janus-core/src/events.rs`: `Ord` compares only time and then `job_id`. `PartialEq` additionally compares `kind`, so the two are inconsistent.
- At equal times, ties are broken by *reverse* job-id order, not by kind. Whether a `JobComplete` frees GPUs before a simultaneous `JobArrival` therefore depends on job names.

**"FIFO" backfill can starve large jobs.**
- `fifo.rs` and `common.rs` skip any job that doesn't fit and have no reservation mechanism.
- Large or gang jobs can be starved indefinitely. This is documented, but it is surprising behaviour for a policy named "fifo".

**`run_sweep` drops most variant dimensions.**
- In `benchmarks/sweep.py`, `run_sweep` only uses `variant["scheduler"]`. Any `cluster` or `workload` variants passed in are silently ignored.
- `SweepVariant` is unused.

**Python OpenAI shim quirks** (`server/openai_shim.py`):
- `max_tokens` defaults to 128, so `body.max_tokens or default_output` never falls back.
- TTFT adds 5% of decode time, which diverges from `inference.rs`.
- Rate limiting is keyed by client IP, while the docs say "per client key".

**Twins page links to the wrong ID space.**
- `web/src/app/twins/page.tsx` links `aiperf_run_id` to `/runs/:id`.
- The API parses `:id` as a `Uuid` of a Janus run, so these links will 4xx.

**Version drift.**
- `pyproject.toml` and `Cargo.toml` are at 0.2.0.
- `python/zyvor_janus/__init__.py` has `__version__ = "0.1.0"`.
- K8s manifests, `job.example.yaml` and the `build-images.sh` default tag all use `0.1.0`.

**Minor issues:**
- **WebSocket stream gaps:** `ws.rs` silently skips `Lagged` broadcast messages, so clients see gaps.
- **Shadow-run race:** a shadow run connected while still `Pending` gets an immediate `complete` with null metrics.
- **Side-effecting read:** `GET /api/twins` creates directories and a DB on read (`twin_store.rs`).

## Security concerns

**Default credentials and secrets everywhere.**
- `Admin/Admin@321` (`web/src/lib/auth.ts`).
- API key `dev-zyvor-janus-key` and a matching WS ticket secret (`state.rs`, `middleware.ts`).
- The GHCR `docker run` example in `README.md` sets none of these, so a default deploy exposes a known bearer token.

**Session HMAC secret falls back to the dashboard password.**
- `getAuthSecret()` uses the password when `ZYVOR_JANUS_AUTH_SECRET` is unset. Anyone who knows the default password can forge 7-day sessions.
- Tokens aren't bound to a user and can't be revoked; logout only clears the cookie.

**Password checks are weak.**
- `matchesPassword` lowercases and trims the input, which reduces entropy.
- `verifyCredentials` accepts the default credentials whenever `NODE_ENV !== "production"`. That covers every `npm run dev` started by the scripts, even when custom credentials are configured.

**Scripts override and leak credentials.**
- `scripts/run_web_dashboard.sh` and `run_web_ui.sh` overwrite user-supplied credentials unless `ZYVOR_JANUS_AUTH_CUSTOM` is set. The README's override example is therefore silently ignored.
- `run_web_dashboard.sh` also prints the password to stdout.

**Over-exposed API.**
- `main.rs` uses `CorsLayer::permissive()` and a hard-coded `0.0.0.0:8080` bind.
- `run_web_api.sh` binds `0.0.0.0` with `--reload`.
- `nodeport.yaml` exposes the API directly on port 30818.

**WebSocket tickets are weaker than they could be.**
- Tickets are reusable within their 45 s TTL.
- They are passed in the query string, so they may appear in access or trace logs.

**Non-constant-time key comparison.** The Python shim compares the API key with `!=`.

## Test and CI gaps

**Python coverage is mostly skipped.**
- `benchmark.yml` runs the Python tests without building the extension or installing `pyyaml`, so most tests skip and the `simple_yaml` fallback is used.
- `python.yml` installs no `server`, `viz` or `rl` extras, so the FastAPI, viz and Gym tests skip too. This is what hid the matplotlib bug.

**No web CI.** There is no `npm run build`, lint or type-check, and no tests for `auth.ts`, `middleware.ts` or `navigation.ts`.

**API crate is barely tested.**
- There are no route or integration tests for `zyvor-janus-api`: bearer layer, health bypass, WS ticket flow, shim rate limit.
- No test exercises the Ingress routing assumption.

**Image publishing isn't gated.**
- `docker-publish.yml` pushes `latest` on any `v*.*.*` tag, with no dependency on the test workflows.

**Workflow hygiene.**
- `benchmark.yml` re-runs the full `cargo test` (duplicating `rust.yml`) without caching.
- It also triggers on a stray `init_dev` branch.

**No security scanning.** There is no `cargo audit`, `npm audit` or Dependabot configuration visible.

**Missing simulator invariant tests.** Nothing covers same-time event ordering, MIG registries for mixed fleets, or composite-score normalisation.

**Tests write into the repo.** `outputs/` is written to by `test_viz.py` and the CLI tests.

## Documentation gaps

**Python vs. Rust backend confusion.**
- `docs/ui_roadmap.md` says the Rust `zyvor-janus-api` replaced FastAPI in production.
- Meanwhile the dev scripts, `admin-basics.md`, `openai_shim.md` and the customer docs still describe FastAPI. `API_PORT` doesn't apply to the Rust bin
