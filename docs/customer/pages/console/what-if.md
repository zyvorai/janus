# What-if

## Purpose

Sweep schedulers on the same inference workload and compare a results matrix.

## When to use it

- Operate **What-if** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/what-if`
- Nav: **Sidebar → What-if**

## Operate from the console (UX)

1. Open `/what-if`.
2. Pick base config (e.g. `small_h100.yaml`) and subset of schedulers.
3. **Run sweep** → inspect Results matrix bars; **Download CSV** when offered.
4. **Empty / fail:** No schedulers selected → enable at least one; sweep fails → config path.
5. **Success:** Matrix populated for each scheduler under test.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Dashboard](home.md)
- [Benchmark](benchmark.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
