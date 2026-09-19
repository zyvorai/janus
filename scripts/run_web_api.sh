#!/usr/bin/env bash
# Start the Zyvor Janus FastAPI backend (port 8080 by default).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=common.sh
source "$ROOT/scripts/common.sh"

fix_homebrew_pyexpat
require_venv "$ROOT"

PY="$ROOT/.venv/bin/python"
PORT="${PORT:-8080}"
HOST="${HOST:-0.0.0.0}"

ensure_zynera_extension "$PY" "$ROOT"
ensure_server_deps "$PY"

export PYTHONPATH="$ROOT/python${PYTHONPATH:+:$PYTHONPATH}"

echo "Zyvor Janus API → http://127.0.0.1:${PORT}"
echo "  health:  http://127.0.0.1:${PORT}/api/health"
echo "  configs: http://127.0.0.1:${PORT}/api/configs"
echo

exec "$PY" -m uvicorn zyvor_janus.server.app:app --reload --host "$HOST" --port "$PORT"
