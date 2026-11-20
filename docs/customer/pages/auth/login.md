# Login

## Purpose

Sign in to the Zyvor Janus web console (session cookie).

## When to use it

- Operate **Login** when your job matches this screen
- Prefer **Dashboard** after login if you are unsure where to start
- Confirm FastAPI health on `:8080` if the UI looks empty

## How to get there

- Route: `/login`
- Nav: **Unauthenticated → `/login`**

## Operate from the console (UX)

1. Open `http://<host>:3000/login` (or you are redirected here).
2. Enter dashboard credentials (default **Admin** / **Admin@321** unless `ZYVOR_JANUS_DASHBOARD_USER` / `PASSWORD` override).
3. Submit — session is an httpOnly cookie (≈7 days).
4. **Empty / fail:** Login failed → check API on `:8080` and env credentials; middleware blocks `/api` without auth.
5. **Success:** Land on Dashboard; sidebar shows Dashboard · Runs · Benchmark · What-if · Twins.

Web UI: `http://<host>:3000` · API: `http://<host>:8080/api/health` (`./scripts/run_web_dashboard.sh`). CLI: `cargo run -p zyvor-janus-cli -- run --config …`. Never publish lab IPs — use `<host>`.

## Related pages

- [Dashboard](../console/home.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
