#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

# Run the Rich live dashboard using the project venv.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PY="$ROOT/.venv/bin/python"

if [[ -d /opt/homebrew/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/opt/homebrew/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
elif [[ -d /usr/local/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/usr/local/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
fi

if [[ ! -x "$PY" ]]; then
  echo "No .venv found. Run ./scripts/setup_dev.sh first." >&2
  exit 1
fi

if ! "$PY" -c "import pip" >/dev/null 2>&1; then
  echo ".venv is incomplete (no pip). Run ./scripts/setup_dev.sh" >&2
  exit 1
fi

export PYTHONPATH="$ROOT/python${PYTHONPATH:+:$PYTHONPATH}"

if ! "$PY" -c "import zyvor_janus._zyvor_janus" 2>/dev/null; then
  echo "Zyvor Janus extension not built. Run ./scripts/setup_dev.sh" >&2
  exit 1
fi

if ! "$PY" -c "import rich" 2>/dev/null; then
  echo "rich not installed. Run ./scripts/setup_dev.sh" >&2
  exit 1
fi

exec "$PY" python/examples/live_dashboard.py "$@"
