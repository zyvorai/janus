#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

# Start the Zyvor Janus Next.js frontend (port 3000 by default).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=common.sh
source "$ROOT/scripts/common.sh"

WEB="$ROOT/web"
PORT="${PORT:-3000}"
export ZYVOR_JANUS_API_URL="${ZYVOR_JANUS_API_URL:-http://127.0.0.1:8080}"
# Next's dev/start server can't proxy WebSocket upgrades through rewrites (see
# web/src/lib/api.ts's runWebSocketUrl for details), so point the browser at the
# API directly for the live run view.
export NEXT_PUBLIC_ZYVOR_JANUS_WS_URL="${NEXT_PUBLIC_ZYVOR_JANUS_WS_URL:-ws://127.0.0.1:8080}"

if [[ -z "${ZYVOR_JANUS_AUTH_CUSTOM:-}" ]]; then
  export ZYVOR_JANUS_DASHBOARD_USER="Admin"
  export ZYVOR_JANUS_DASHBOARD_PASSWORD="Admin@321"
else
  export ZYVOR_JANUS_DASHBOARD_USER="${ZYVOR_JANUS_DASHBOARD_USER:-Admin}"
  export ZYVOR_JANUS_DASHBOARD_PASSWORD="${ZYVOR_JANUS_DASHBOARD_PASSWORD:-Admin@321}"
fi

ensure_web_deps "$WEB"

echo "Zyvor Janus web UI → http://127.0.0.1:${PORT}"
echo "(proxies /api and /ws to http://127.0.0.1:8080 — start ./scripts/run_web_api.sh first)"
echo

cd "$WEB"
exec npm run dev -- -p "$PORT"
