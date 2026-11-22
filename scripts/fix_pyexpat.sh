#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

# Repair Homebrew python@3.13 pyexpat + venv wrapper without recreating .venv.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=common.sh
source "$ROOT/scripts/common.sh"

VENV="$ROOT/.venv"
PY="$VENV/bin/python"

if [[ ! -x "$PY" && ! -L "$PY" ]]; then
  echo "No .venv found. Run ./scripts/setup_dev.sh first." >&2
  exit 1
fi

fix_homebrew_pyexpat

if venv_python_wrapper_broken "$VENV"; then
  echo "Detected broken python wrapper (infinite exec loop). Repairing..." >&2
  repair_venv_python_wrapper "$VENV" || {
    echo "Could not repair .venv automatically." >&2
    echo "Run: rm -rf .venv && USE_UV=1 ./scripts/setup_dev.sh" >&2
    exit 1
  }
fi

wrap_venv_python_for_pyexpat "$VENV"
ensure_pip_shims "$VENV"

if ! "$PY" -c "import pyexpat, pip; import sys; assert sys.prefix.startswith('$VENV')" >/dev/null 2>&1; then
  echo "pyexpat/pip still broken after wrapper install." >&2
  echo "Try: rm -rf .venv && USE_UV=1 ./scripts/setup_dev.sh" >&2
  exit 1
fi

echo "OK: venv python prefix=$("$PY" -c 'import sys; print(sys.prefix)')"
echo "Run: source .venv/bin/activate && maturin develop"
