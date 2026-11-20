# Benchmark

## Purpose

Scheduler benchmarks with inference TTFT/TPS metrics, score vectors, and AIPerf twin comparison.

## When to use it

- Operate **Benchmark** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/benchmark`
- Nav: **Sidebar → Benchmark**

## Operate from the console (UX)

1. Open `/benchmark`.
2. Configure the benchmark case → **Run benchmark**.
3. Read Score vector radar and Sim vs measured (AIPerf twin) panels.
4. Open recent benchmark reports → link into `/runs/:id`.
5. **Empty / fail:** Running stuck → API/engine error; no TTFT → non-LLM workload.
6. **Success:** Report row appears with colored cells vs best-in-column.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Run detail](runs-id.md)
- [Twins](twins.md)
- [Dashboard](home.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
