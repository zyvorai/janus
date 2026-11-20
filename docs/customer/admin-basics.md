# Admin basics

## Ports

| Service | Default | Notes |
|---------|---------|--------|
| Next.js UI | `3000` | `UI_PORT` |
| FastAPI | `8080` | `API_PORT`; health `GET /api/health` |
| CLI | n/a | Local `zyvor-janus-cli` process |

Start together: `./scripts/run_web_dashboard.sh` (API binds `0.0.0.0` by default in `run_web_api.sh`).

## Auth (web console)

| Env | Default | Purpose |
|-----|---------|---------|
| `ZYVOR_JANUS_DASHBOARD_USER` | `Admin` | Login username |
| `ZYVOR_JANUS_DASHBOARD_PASSWORD` | `Admin@321` | Login password |
| `ZYVOR_JANUS_AUTH_SECRET` | (password) | HMAC for session cookies |

Sessions are httpOnly cookies (≈7-day expiry). Change defaults before any shared host.

## Operate from the console (admin)

1. Confirm `http://<host>:8080/api/health` then open `http://<host>:3000`.
2. Sign in → Dashboard Run smoke test with `small_h100.yaml`.
3. Restrict config file permissions; follow upstream `SECURITY.md`.
4. Packaged systemd units when available — `journalctl -u <unit> -f`.

## Support

GitHub issues on the Janus repo · Contact Zyvor for Enterprise.

## Related

- [Getting Started](getting-started.md)
- [Using the Dashboard](using-the-dashboard.md)
- [Login](pages/auth/login.md)
