# Zyvor Janus Benchmark Platform — Manual Test Document

**Version:** P0–P10 MVP (implemented in `main`)  
**Audience:** QA / developers validating simulation, benchmark, and analytics layers  
**Status:** Use this as the QA guide for the shipped MVP. Remaining product gaps are listed in §15.

See also: [benchmark_platform.md](benchmark_platform.md) · [ui_dashboard.md](ui_dashboard.md) · [openai_shim.md](openai_shim.md)

---

## 1. Prerequisites

| Requirement | Notes |
|-------------|--------|
| Rust toolchain | `cargo build -p zyvor-janus-cli` |
| Python ≥ 3.10 + venv | `./scripts/setup_dev.sh` (builds PyO3 extension) |
| Node.js ≥ 18 | For web UI (`cd web && npm install`) |
| Optional: `fastapi`, `uvicorn` | For API + OpenAI shim tests |

### One-time setup

```bash
./scripts/setup_dev.sh
source .venv/bin/activate
cd web && npm install && cd ..
cargo build -p zyvor-janus-cli
```

### Start services (web tests)

```bash
./scripts/run_web_dashboard.sh
# UI: http://localhost:3000
# API: http://localhost:8080/api/health
```

**Login (web):** `Admin` / `Admin@321` (override via `ZYVOR_JANUS_DASHBOARD_USER` / `ZYVOR_JANUS_DASHBOARD_PASSWORD`).

**OpenAI shim API key:** `dev-zyvor-janus-key` (override via `ZYVOR_JANUS_API_KEY`).

---

## 2. Test matrix overview

| ID | Phase | Feature | Primary surface |
|----|-------|---------|-----------------|
| MT-P0-01 | P0 | Scheduler override | API / CLI |
| MT-P0-02 | P0 | Replay fidelity (engine snapshots) | API artifacts |
| MT-P0-03 | P0 | Run metadata + config hash | `outputs/runs/` |
| MT-P1-01 | P1 | Inference runtime derivation | CLI metrics JSON |
| MT-P1-02 | P1 | TTFT/TPS metrics (not queue wait) | Metrics JSON + UI tiles |
| MT-P1-03 | P1 | Profile v2 curves | `configs/profiles/` |
| MT-P2-01 | P2 | Synthetic workload generator | Python CLI |
| MT-P2-02 | P2 | Golden synthetic fixture | YAML + sim run |
| MT-P3-01 | P3 | Serving trace import (Rust) | Integration / fixture |
| MT-P3-02 | P3 | Serving trace export (API) | `GET /api/runs/{id}/serving-trace` |
| MT-P3-03 | P3 | AIPerf trace adapter (Python) | Adapter module |
| MT-P4-01 | P4 | Scheduler benchmark score vector | `benchmark.json` |
| MT-P4-02 | P4 | Cost model | `configs/analytics/cost.yaml` |
| MT-P4-03 | P4 | Compare panel inference columns | Web `/` compare |
| MT-P5-01 | P5 | Benchmark hub page | Web `/benchmark` |
| MT-P5-02 | P5 | Benchmark API | `POST /api/benchmark/run` |
| MT-P6-01 | P6 | OpenAI shim auth | `POST /v1/chat/completions` |
| MT-P6-02 | P6 | OpenAI shim streaming | SSE response |
| MT-P6-03 | P6 | Rate limiting | 429 after limit |
| MT-P7-01 | P7 | AIPerf import → profile YAML | Python CLI |
| MT-P7-02 | P7 | AIPerf export from workload | Python CLI |
| MT-P8-01 | P8 | What-if sweep API | `POST /api/what-if` |
| MT-P8-02 | P8 | What-if web page | Web `/what-if` |
| MT-P9-01 | P9 | Digital twin store CRUD | Python + `GET /api/twins` |
| MT-P9-02 | P9 | Drift detection | Python twin store |
| MT-P10-01 | P10 | CI golden benchmark script | `benchmarks/ci/run_golden.sh` |
| MT-P10-02 | P10 | GitHub workflow | `.github/workflows/benchmark.yml` |

---

## 3. P0 — Simulation hardening

### MT-P0-01: Scheduler override via API

**Steps:**

1. Start API (`./scripts/run_web_api.sh`).
2. Start a run with scheduler override:

```bash
curl -s -X POST http://localhost:8080/api/runs \
  -H "Content-Type: application/json" \
  -d '{"config":"preemption_preemptive.yaml","scheduler":"preemptive"}' | jq .
```

3. Poll until `status` is `completed`:

```bash
RUN_ID="<id from step 2>"
curl -s "http://localhost:8080/api/runs/$RUN_ID" | jq '.status, .scheduler, .metrics.preemptions'
```

**Expected:**

- `status`: `completed`
- `scheduler`: `preemptive`
- `metrics.preemptions` ≥ 1 (for preemptive config)

**Fail if:** scheduler ignored, run fails, or preemptions = 0 on preemptive workload.

---

### MT-P0-02: Replay uses engine snapshots (not `step_fifo`)

**Steps:**

1. Complete a run (MT-P0-01 or any config).
2. Fetch snapshots and decisions:

```bash
curl -s "http://localhost:8080/api/runs/$RUN_ID/snapshots" | jq 'length'
curl -s "http://localhost:8080/api/runs/$RUN_ID/events" | jq '.[0].kind'
```

3. Inspect disk artifacts:

```bash
ls outputs/runs/$RUN_ID/
# metrics.json timeline.json decisions.json snapshots.json metadata.json
```

**Expected:**

- `snapshots.json` non-empty for completed runs
- `decisions.json` contains scheduler decision kinds (`job_scheduled`, `job_preempted`, etc.)
- Snapshot count aligns with decision steps (same run, same scheduler)

**Fail if:** snapshots empty while run completed, or replay data inconsistent with `resolved_scheduler`.

---

### MT-P0-03: Config hash in metadata

**Steps:**

1. After run completes, open `outputs/runs/{run_id}/metadata.json`.

**Expected fields:**

- `config`
- `scheduler` (request override, may be null)
- `resolved_scheduler`
- `config_hash` (16-char hex)
- `benchmark` (present for inference configs)

**Fail if:** `config_hash` missing or does not change when only scheduler override changes (same config file).

---

## 4. P1 — Inference performance model

### MT-P1-01: CLI inference workload produces derived runtime

**Steps:**

```bash
cargo run -p zyvor-janus-cli -- run \
  --config configs/clusters/inference_llama.yaml \
  --output /tmp/inference_metrics.json
cat /tmp/inference_metrics.json | jq '.inference_jobs, .ttft_p50, .tps_mean, .goodput'
```

**Expected:**

- `inference_jobs`: 3
- `ttft_p50` > 0 (token latency, not queue wait)
- `tps_mean` > 0
- `goodput` > 0
- `makespan` reflects inferred runtimes (not placeholder `runtime: 1.0` from workload YAML)

**Fail if:** inference fields absent or all zero.

---

### MT-P1-02: TTFT is distinct from queue wait

**Steps:**

1. Run `inference_llama.yaml` with FIFO (default).
2. Compare:
   - `ttft_p50` / `ttft_p99` — analytical prefill/decode latency
   - `queue_delay_p99` — wait before first start
   - `mean_cumulative_wait_time` — scheduling wait

**Expected:**

- All three can differ; none should be aliased to `time_to_first_start` in metrics JSON
- Under contention, `queue_delay_p99` > 0 while `ttft_p50` stays model-derived

**Optional UI check:** On benchmark page or compare results for inference runs, tiles show **TTFT p50**, **TPS mean**, **Goodput**, **Queue delay p99**.

---

### MT-P1-03: Profile v2 fields

**Steps:**

1. Open `configs/profiles/llama-70b.yaml`.
2. Confirm per-GPU entries include `prefill_ms_per_token`, `decode_tps`, `max_batch`.

**Python check:**

```bash
PYTHONPATH=python python3 -c "
from zyvor_janus.adapters.profiles import ProfileRegistry
from pathlib import Path
r = ProfileRegistry(Path('configs/profiles'))
print(r.lookup_v2('llama-70b', 'H100'))
"
```

**Expected:** tuple `(prefill_ms, decode_tps)` with sensible positive values.

---

## 5. P2 — Synthetic LLM workload generator

### MT-P2-01: Deterministic generation

**Steps:**

```bash
PYTHONPATH=python python3 python/zyvor_janus/workloads/generate_synthetic.py \
  --preset peak_chat --seed 42 --preview | jq '.job_count'

# Repeat — job_count and first arrivals must match
```

**Expected:** Identical output for same seed/preset.

---

### MT-P2-02: Generate golden fixture + validate

**Steps:**

```bash
PYTHONPATH=python python3 python/zyvor_janus/workloads/generate_synthetic.py \
  --preset peak_chat --seed 42 \
  --output tests/fixtures/workloads/synthetic_llm_peak.yaml \
  --trace-output /tmp/serving_peak.json

head -20 tests/fixtures/workloads/synthetic_llm_peak.yaml
```

**Expected:**

- YAML jobs have `model_id`, `input_tokens`, `output_tokens`
- Trace JSON has `"version": "serving.trace.v1"`

**Negative test:** Edit a job to `input_tokens: 0` and `output_tokens: 0` — generator validator should reject.

---

## 6. P3 — Serving trace import/export

### MT-P3-01: Rust serving trace import

**Steps:**

```bash
cargo test -p zyvor-janus-config integration_serving_trace_import_roundtrip -- --nocapture
cargo test -p zyvor-janus-config serving_trace::tests -- --nocapture
```

**Expected:** Jobs loaded with `model_id`, token counts, tenant preserved.

**Important:** Do not mix with M3 scheduler traces (`crates/zyvor-janus-config/src/trace.rs` fixtures).

---

### MT-P3-02: Serving trace export via API

**Steps:**

1. Complete an inference run via API; note `RUN_ID`.
2. Export:

```bash
curl -s "http://localhost:8080/api/runs/$RUN_ID/serving-trace" | jq '.version, (.records | length)'
```

**Expected:**

- `version`: `serving.trace.v1`
- `records` ≥ 1 for completed inference runs

---

### MT-P3-03: Python AIPerf ↔ serving trace mapping

**Steps:**

```bash
PYTHONPATH=python python3 -m unittest python.tests.test_benchmark_platform.ServingTraceAdapterTests -v
```

**Manual:**

```python
from pathlib import Path
from zyvor_janus.adapters.serving_trace import load_serving_trace, to_aiperf_requests
trace = load_serving_trace(Path("tests/fixtures/traces/serving_llama.jsonl"))
rows = to_aiperf_requests(trace)
assert rows[0]["input_sequence_length"] == 512
```

**Expected:** Field mapping round-trips without using M3 trace format.

**Note:** CLI flags `--serving-trace` / `--export-serving-trace` on `zyvor-janus run` are not in `zyvor-janus-cli` yet — use API/Python/Rust library paths above.

---

## 7. P4 — Scheduler benchmark score

### MT-P4-01: Benchmark report on inference run

**Steps:**

1. Run inference config via API or CLI.
2. Inspect `outputs/runs/{id}/benchmark.json` (API runs only).

**Expected fields:**

- `scheduler`, `config_hash`
- `metrics` (full `SimulationMetrics`)
- `jain_fairness`, `fragmentation`, `cost_usd`
- `score_vector` with keys: `makespan`, `gpu_utilization`, `ttft_p50`, `goodput`, `cost_usd`, etc.

---

### MT-P4-02: Cost model

**Steps:**

1. Open `configs/analytics/cost.yaml` (`gpu_hour_usd: 3.50`).
2. Compare `benchmark.cost_usd` across runs with different GPU utilization.

**Expected:** Higher GPU-seconds → higher `cost_usd`.

---

### MT-P4-03: Compare panel — inference columns

**Steps (web):**

1. Go to `/`, login.
2. Compare two configs with inference workloads.
3. Inspect compare table.

**Expected columns include:** TTFT p50/p99, TPS mean, Goodput, Queue delay p99 (in addition to scheduling metrics).

---

## 8. P5 — Benchmark dashboard UI

### MT-P5-01: Benchmark hub page

**Steps:**

1. Open http://localhost:3000/benchmark (after login).
2. Select config `inference_llama.yaml`, scheduler `fifo`.
3. Click **Run benchmark**.

**Expected:**

- Metrics tiles include TTFT, TPS, Goodput when inference jobs present
- **Score vector** JSON panel appears
- Cost + Jain fairness summary shown
- **Recent benchmark reports** lists the run

---

### MT-P5-02: Benchmark API

**Steps:**

```bash
curl -s -X POST http://localhost:8080/api/benchmark/run \
  -H "Content-Type: application/json" \
  -d '{"config":"inference_llama.yaml","scheduler":"fifo"}' | jq '.benchmark.score_vector'

curl -s http://localhost:8080/api/benchmark/presets | jq '.workload_presets'
curl -s http://localhost:8080/api/benchmark/reports | jq 'length'
```

**Expected:** Presets list `morning_rag`, `peak_chat`, `night_training`; run returns `benchmark` object.

---

## 9. P6 — OpenAI-compatible shim

**Prerequisite:** API running (`zyvor_janus.server.app` mounts shim at `/v1`).

### MT-P6-01: Auth required

```bash
# Should 401
curl -s -o /dev/null -w "%{http_code}\n" \
  -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"llama-70b","messages":[{"role":"user","content":"hello"}]}'

# Should 200
curl -s -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer dev-zyvor-janus-key" \
  -H "Content-Type: application/json" \
  -d '{"model":"llama-70b","messages":[{"role":"user","content":"hello world"}]}' | jq '.choices[0].message.content'
```

---

### MT-P6-02: Streaming

```bash
curl -N -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer dev-zyvor-janus-key" \
  -H "Content-Type: application/json" \
  -d '{"model":"llama-70b","stream":true,"messages":[{"role":"user","content":"one two three"}]}'
```

**Expected:** SSE chunks `data: {...}` then `data: [DONE]`, with delay before first token (TTFT simulation).

---

### MT-P6-03: Rate limit

**Steps:** Send >120 requests/min from same client IP (default `ZYVOR_JANUS_SHIM_RATE_LIMIT=120`).

**Expected:** HTTP 429 with `"rate limit exceeded"`.

**Note:** Shim uses analytical profile timing, not live DES job injection.

---

## 10. P7 — AIPerf calibration plugin

### MT-P7-01: Import AIPerf JSON → profile YAML

**Steps:**

```bash
PYTHONPATH=python python3 -m zyvor_janus.benchmarks.aiperf_adapter import \
  tests/fixtures/aiperf/sample_result.json \
  --profile llama-70b --gpu-type H100 \
  --profiles-dir /tmp/profiles_test

cat /tmp/profiles_test/llama-70b.yaml
```

**Expected:**

- YAML contains `prefill_ms_per_token`, `decode_tps`
- `calibrated_from_aiperf: true`

---

### MT-P7-02: Export workload to AIPerf config

```bash
PYTHONPATH=python python3 -m zyvor_janus.benchmarks.aiperf_adapter export \
  configs/workloads/inference_llama.yaml \
  --output /tmp/aiperf_config.json

cat /tmp/aiperf_config.json | jq '.requests | length'
```

**Expected:** 3 requests with sequence lengths matching workload.

---

## 11. P8 — What-if analysis

### MT-P8-01: What-if API

```bash
curl -s -X POST http://localhost:8080/api/what-if \
  -H "Content-Type: application/json" \
  -d '{"base_config":"inference_llama.yaml","schedulers":["fifo","preemptive"]}' \
  | jq '.results[] | {scheduler, makespan: .metrics.makespan, ttft: .metrics.ttft_p50, cost: .benchmark.cost_usd}'
```

**Expected:** Two result rows; metrics differ when scheduling behavior differs.

---

### MT-P8-02: What-if web page

**Steps:**

1. Open http://localhost:3000/what-if
2. Click **Run fifo vs preemptive**

**Expected:** Results matrix with Makespan, TTFT p50, GPU util, Cost USD columns.

---

## 12. P9 — Digital twin store

### MT-P9-01: Twin CRUD (Python)

```python
from pathlib import Path
from zyvor_janus.benchmarks.twin_store import TwinStore, TwinEntry

store = TwinStore(Path("outputs/twins"))
v1 = store.upsert(TwinEntry("H100", "llama-70b", 50.0, 95.0, 10.0, TwinStore.now_iso(), "run-1"))
v2 = store.upsert(TwinEntry("H100", "llama-70b", 52.0, 96.0, 10.5, TwinStore.now_iso(), "run-2"))
assert v2 == v1 + 1
print(store.latest("H100", "llama-70b"))
```

---

### MT-P9-02: Drift detection

```python
assert not store.detect_drift("H100", "llama-70b", 52.0)   # within 10%
assert store.detect_drift("H100", "llama-70b", 80.0)       # >10% drift
```

### MT-P9-03: Twins API

```bash
curl -s http://localhost:8080/api/twins | jq .
```

**Expected:** JSON array (empty until twins seeded); creates `outputs/twins/export.json`.

---

## 13. P10 — CI benchmark gates

### MT-P10-01: Local golden script

```bash
bash benchmarks/ci/run_golden.sh
```

**Expected output:** `golden inference benchmark ok`

---

### MT-P10-02: Full regression suite (pre-PR)

```bash
cargo test --workspace --exclude zyvor-janus-py
PYTHONPATH=python python3 -m unittest discover -s python/tests -v
```

**Expected:** All non-skipped tests pass.

---

### MT-P10-03: GitHub Actions

**File:** `.github/workflows/benchmark.yml`

**Verify on PR/push to `main`:**

- Job `rust-benchmark`: Rust tests + `run_golden.sh`
- Job `python-benchmark`: Python unit tests

---

## 14. Cross-feature smoke path (recommended demo)

End-to-end “Simulated vs calibrated” demo (plan milestone: P1 + P7 + P5):

1. **Import AIPerf fixture** → update profile (use temp dir in production).
2. **CLI run:** `inference_llama.yaml` → confirm TTFT/TPS in metrics JSON.
3. **Web:** `/benchmark` → run same config → verify score vector + inference tiles.
4. **What-if:** `/what-if` → compare fifo vs preemptive queue delay / TTFT.
5. **Export trace:** `GET /api/runs/{id}/serving-trace` → validate `serving.trace.v1`.
6. **OpenAI shim:** authenticated chat completion with streaming.

---

## 15. Known gaps vs plan

| Plan item | Status |
|-----------|--------|
| Run detail UI `/runs/:id` | **Missing page** — home may link here; use `outputs/runs/{id}/` artifacts |
| CLI `--serving-trace` flags | Not in `zyvor-janus` CLI — use Rust lib / Python adapter / API export |
| Serving-trace API tokens | Export may stub `input_tokens` / `output_tokens` (e.g. 128/64) |
| OpenAI shim → live DES queue | Analytical timing only ([openai_shim.md](openai_shim.md)) |
| Benchmark AIPerf upload UI | Use Python `aiperf_adapter` CLI |
| Twin library page | API + SQLite only |
| Compare PDF export | Not implemented |
| P8 Pareto chart / CSV | Table view only |
| CI Python job + maturin | Extension-dependent tests may skip if the wheel is not built in that job |

---

## 16. Sign-off checklist

| Phase | Tester | Date | Pass/Fail | Notes |
|-------|--------|------|-----------|-------|
| P0 | | | | |
| P1 | | | | |
| P2 | | | | |
| P3 | | | | |
| P4 | | | | |
| P5 | | | | |
| P6 | | | | |
| P7 | | | | |
| P8 | | | | |
| P9 | | | | |
| P10 | | | | |

---

See also: [benchmark platform roadmap](benchmark_platform.md) · [UI dashboard guide](ui_dashboard.md) · [benchmark score](benchmark_score.md)
