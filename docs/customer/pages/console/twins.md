# Twins

## Purpose

AIPerf-calibrated GPU/model performance twins that ground the simulator against measured hardware.

## When to use it

- Operate **Twins** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/twins`
- Nav: **Sidebar → Twins**

## Operate from the console (UX)

1. Open `/twins`.
2. Browse calibrated twins; sort/filter by measured_at.
3. Open linked AIPerf run when `aiperf_run_id` is present.
4. **Empty / fail:** No twins calibrated yet → run AIPerf calibration pipeline per product docs.
5. **Success:** Twin cards list hardware/model pairs used by Benchmark sim-vs-measured.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Benchmark](benchmark.md)
- [Run detail](runs-id.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
