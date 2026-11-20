# Simulate

## Purpose

Live scheduling replay — watch requests → queue → GPU assign → complete for a run.

## When to use it

- Operate **Simulate** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/simulate`
- Nav: **Run detail → Open in Simulate · `/simulate?run=<id>`**

## Operate from the console (UX)

1. Open `/simulate` or `/simulate?run=<id>` from a completed/in-progress run.
2. Select a run if needed → **Watch** / load to stream the SimulateStage.
3. Follow legend for scheduler decisions; respect reduced-motion preferences.
4. Jump **Run detail** or **Dashboard** from actions.
5. **Empty / fail:** No runs to watch → start one from Dashboard; busy spinner → wait for snapshot.
6. **Success:** Stage animation advances with decisions for the selected run.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Run detail](runs-id.md)
- [Dashboard](home.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
