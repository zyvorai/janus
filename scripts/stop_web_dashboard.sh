#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

# Stop Zyvor Janus web dashboard servers (FastAPI + Next.js).
set -euo pipefail

API_PORT="${API_PORT:-8080}"
UI_PORT="${UI_PORT:-3000}"
STOP_API=1
STOP_UI=1

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Stop Zyvor Janus web dashboard processes started by run_web_dashboard.sh
(or run_web_api.sh / run_web_ui.sh).

Options:
  --api-only   Stop FastAPI only (default port: ${API_PORT})
  --ui-only    Stop Next.js only (default port: ${UI_PORT})
  -h, --help   Show this help

Environment:
  API_PORT     FastAPI port (default: 8080)
  UI_PORT      Next.js port (default: 3000)

Examples:
  ./scripts/stop_web_dashboard.sh
  API_PORT=9000 UI_PORT=3001 ./scripts/stop_web_dashboard.sh
  ./scripts/stop_web_dashboard.sh --api-only
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --api-only)
      STOP_UI=0
      ;;
    --ui-only)
      STOP_API=0
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

kill_listeners_on_port() {
  local port="$1"
  local label="$2"
  local pids

  pids="$(lsof -ti "tcp:${port}" -sTCP:LISTEN 2>/dev/null || true)"
  if [[ -z "$pids" ]]; then
    echo "No ${label} listening on port ${port}."
    return 0
  fi

  echo "Stopping ${label} on port ${port} (PID: ${pids//$'\n'/ })..."
  # shellcheck disable=SC2086
  kill $pids 2>/dev/null || true

  sleep 0.5
  pids="$(lsof -ti "tcp:${port}" -sTCP:LISTEN 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    echo "Force stopping ${label} on port ${port}..."
    # shellcheck disable=SC2086
    kill -9 $pids 2>/dev/null || true
  fi
}

stop_api() {
  kill_listeners_on_port "$API_PORT" "FastAPI"
  if pgrep -f "uvicorn zyvor_janus\\.server\\.app:app" >/dev/null 2>&1; then
    echo "Stopping remaining uvicorn (zyvor_janus.server.app) processes..."
    pkill -f "uvicorn zyvor_janus\\.server\\.app:app" 2>/dev/null || true
  fi
}

stop_ui() {
  kill_listeners_on_port "$UI_PORT" "Next.js"
  if pgrep -f "next dev.*-p ${UI_PORT}" >/dev/null 2>&1; then
    echo "Stopping remaining Next.js dev server on port ${UI_PORT}..."
    pkill -f "next dev.*-p ${UI_PORT}" 2>/dev/null || true
  fi
}

echo "Stopping Zyvor Janus web dashboard..."

if [[ "$STOP_API" -eq 1 ]]; then
  stop_api
fi

if [[ "$STOP_UI" -eq 1 ]]; then
  stop_ui
fi

echo "Done."
