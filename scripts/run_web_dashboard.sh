#!/usr/bin/env bash
# Start FastAPI backend + Next.js frontend for the Zyvor Janus web dashboard.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=common.sh
source "$ROOT/scripts/common.sh"

API_PORT="${API_PORT:-8080}"
UI_PORT="${UI_PORT:-3000}"
export ZYVOR_JANUS_API_URL="http://127.0.0.1:${API_PORT}"
# Next's dev/start server can't proxy WebSocket upgrades through rewrites (see
# web/src/lib/api.ts's runWebSocketUrl for details), so point the browser at the
# API directly for the live run view.
export NEXT_PUBLIC_ZYVOR_JANUS_WS_URL="${NEXT_PUBLIC_ZYVOR_JANUS_WS_URL:-ws://127.0.0.1:${API_PORT}}"

fix_homebrew_pyexpat
require_venv "$ROOT"

PY="$ROOT/.venv/bin/python"
ensure_zynera_extension "$PY" "$ROOT"
ensure_server_deps "$PY"
ensure_web_deps "$ROOT/web"

export PYTHONPATH="$ROOT/python${PYTHONPATH:+:$PYTHONPATH}"

# Dashboard login — force local defaults unless ZYVOR_JANUS_AUTH_CUSTOM=1 is set.
if [[ -z "${ZYVOR_JANUS_AUTH_CUSTOM:-}" ]]; then
  export ZYVOR_JANUS_DASHBOARD_USER="Admin"
  export ZYVOR_JANUS_DASHBOARD_PASSWORD="Admin@321"
else
  export ZYVOR_JANUS_DASHBOARD_USER="${ZYVOR_JANUS_DASHBOARD_USER:-Admin}"
  export ZYVOR_JANUS_DASHBOARD_PASSWORD="${ZYVOR_JANUS_DASHBOARD_PASSWORD:-Admin@321}"
fi

cleanup() {
  if [[ -n "${API_PID:-}" ]] && kill -0 "$API_PID" 2>/dev/null; then
    kill "$API_PID" 2>/dev/null || true
    wait "$API_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

echo "Starting Zyvor Janus web dashboard..."
echo "  API: http://127.0.0.1:${API_PORT}"
echo "  UI:  http://127.0.0.1:${UI_PORT}"
echo "  Login: ${ZYVOR_JANUS_DASHBOARD_USER} / ${ZYVOR_JANUS_DASHBOARD_PASSWORD}"
echo
echo "Press Ctrl+C to stop both servers."
echo

"$PY" -m uvicorn zyvor_janus.server.app:app --reload --host 127.0.0.1 --port "$API_PORT" &
API_PID=$!

# Give the API a moment to bind.
sleep 1

cd "$ROOT/web"
exec npm run dev -- -p "$UI_PORT"
