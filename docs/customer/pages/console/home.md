# Dashboard

## Purpose

Mission Control — launch simulations, compare configs, and jump into recent runs.

## When to use it

- Operate **Dashboard** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/`
- Nav: **Sidebar → Dashboard**

## Operate from the console (UX)

1. Open `/` after sign-in.
2. **Launch simulation**: pick a cluster config → optional shadow scheduler → **Run** (or **Run + shadow**).
3. **Compare configs**: choose A and B → Compare → review metric deltas in ComparePanel.
4. Open a recent run → `/runs/:id`, or jump to Benchmark / What-if shortcuts.
5. Anchor **/#runs** focuses the runs list (sidebar Runs).
6. **Empty / fail:** No configs → ensure `configs/clusters/` is visible to the API; Run fails → check FastAPI logs on `:8080`.
7. **Success:** New run appears; Open navigates to run detail.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Run detail](runs-id.md)
- [Benchmark](benchmark.md)
- [What-if](what-if.md)
- [Simulate](simulate.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
