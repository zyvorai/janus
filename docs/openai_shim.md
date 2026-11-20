# OpenAI-Compatible Virtual Endpoint (P6)

Zyvor Janus exposes a **virtual** OpenAI-compatible HTTP API for testing clients, workload generators, and AIPerf — **without running a real LLM**.

Mounted on the FastAPI server (`python/zyvor_janus/server/`) at `/v1`. See the platform overview: [benchmark_platform.md](benchmark_platform.md).

## Status

**MVP shipped.** Timing is **analytical** (profile v2 prefill/decode estimates). Requests are **not** injected into the DES job queue yet.

| Capability | Status |
|------------|--------|
| `POST /v1/chat/completions` | Done |
| Bearer API key auth | Done (`ZYVOR_JANUS_API_KEY`, default `dev-zyvor-janus-key`) |
| Per-key rate limiting | Done (`ZYVOR_JANUS_SHIM_RATE_LIMIT`, default 120/min) |
| SSE streaming (`stream: true`) | Done |
| Analytical TTFT from profiles | Done |
| Inject into live DES queue | **Not implemented** (planned follow-up) |
| Bind `127.0.0.1` by default | Script-dependent — `run_web_dashboard.sh` uses `127.0.0.1`; `run_web_api.sh` defaults `HOST=0.0.0.0` |

## Endpoint

```http
POST /v1/chat/completions
Authorization: Bearer <api-key>
Content-Type: application/json
```

Example (API running on port 8080):

```bash
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer dev-zyvor-janus-key" \
  -H "Content-Type: application/json" \
  -d '{"model":"llama-70b","messages":[{"role":"user","content":"hello"}],"max_tokens":64,"stream":false}'
```

## Internal flow (current MVP)

```text
HTTP request
  → authenticate Bearer token + rate limit
  → parse model + messages → estimate input/output tokens
  → look up profile v2 (prefill_ms_per_token, decode_tps)
  → compute analytical TTFT / decode schedule
  → optional SSE stream of fake tokens (timing matches estimate)
  → response complete
```

No GPU execution. No DES scheduling contention in this MVP.

## Planned flow (follow-up)

```text
HTTP request → inject JobArrival into simulation queue
  → scheduler places on virtual GPUs
  → inference model computes TTFT under contention
  → SSE stream matching sim schedule
```

## Environment

| Variable | Default | Meaning |
|----------|---------|---------|
| `ZYVOR_JANUS_API_KEY` | `dev-zyvor-janus-key` | Bearer token required by the shim |
| `ZYVOR_JANUS_SHIM_RATE_LIMIT` | `120` | Requests per minute per client key |
| `ZYVOR_JANUS_PROFILES_DIR` | `configs/profiles` | Profile registry for timing |

## AIPerf integration (P7)

AIPerf can target the shim as an OpenAI-compatible endpoint for **deterministic** benchmark runs:

```text
AIPerf → Zyvor Janus OpenAI shim → simulated TTFT/TPS
```

Live AIPerf against real vLLM remains a separate **calibration** path (offline JSON import via `python -m zyvor_janus.benchmarks.aiperf_adapter`).

## Security notes

- Change `ZYVOR_JANUS_API_KEY` outside local demos.
- Prefer binding the API to `127.0.0.1` when exposing the shim on a shared host (`HOST=127.0.0.1 ./scripts/run_web_api.sh`).
- Do not log prompt bodies in production deployments.
