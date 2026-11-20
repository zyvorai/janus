# Run detail

## Purpose

Inspect one simulation run — cluster/queue view, metrics, and jump to Simulate.

## When to use it

- Operate **Run detail** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/runs/:id`
- Nav: **Dashboard runs · Benchmark reports → Open**

## Operate from the console (UX)

1. Open `/runs/<id>` from a run link.
2. Read status, metrics, and cluster/queue panels.
3. **Open in Simulate** to replay scheduling decisions.
4. **Back to dashboard** when done.
5. **Empty / fail:** Run not found → stale id or API offline.
6. **Success:** Run payload loads; Simulate deep-link works.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Simulate](simulate.md)
- [Dashboard](home.md)
- [Benchmark](benchmark.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
