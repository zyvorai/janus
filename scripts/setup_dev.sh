#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

# Create/fix the dev venv, build the Rust extension, and install dashboard deps.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck source=common.sh
source "$ROOT/scripts/common.sh"

VENV="$ROOT/.venv"
PY="$VENV/bin/python"
GET_PIP="$ROOT/scripts/get-pip.py"

patch_activate() {
  local activate="$VENV/bin/activate"
  [[ -f "$activate" ]] || return 0
  if ! grep -q "ZYVOR_JANUS_DYLD_EXPAT" "$activate" 2>/dev/null; then
    cat >>"$activate" <<'EOF'

# Zyvor Janus: Homebrew python@3.13 pyexpat fix
if [[ -d /opt/homebrew/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/opt/homebrew/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
elif [[ -d /usr/local/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/usr/local/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
fi
EOF
  fi
}

choose_python() {
  for candidate in "${PYTHON:-}" python3.13 python3.12 python3.11 python3; do
    [[ -z "$candidate" ]] && continue
    if command -v "$candidate" >/dev/null 2>&1; then
      if "$candidate" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)'; then
        echo "$candidate"
        return 0
      fi
    fi
  done
  echo "No suitable python3 found (need >= 3.10)." >&2
  exit 1
}

pyexpat_ok() {
  local interpreter="$1"
  "$interpreter" -c "import pyexpat" >/dev/null 2>&1
}

require_pyexpat() {
  local interpreter="$1"
  if pyexpat_ok "$interpreter"; then
    return 0
  fi
  cat >&2 <<'EOF'
Python pyexpat is still broken after applying Homebrew expat library path.

Recommended fixes (pick one):

  1) Install uv and let setup use a standalone Python (easiest):
       brew install uv
       rm -rf .venv && USE_UV=1 ./scripts/setup_dev.sh

  2) Use python.org installer (3.12+) instead of Homebrew python@3.13

  3) Reinstall Homebrew Python linked against expat:
       brew reinstall expat python@3.13
       rm -rf .venv && ./scripts/setup_dev.sh
EOF
  exit 1
}

venv_has_pip() {
  [[ -x "$PY" || -L "$PY" ]] && "$PY" -c "import pip" >/dev/null 2>&1
}

prepare_existing_venv() {
  if [[ ! -e "$PY" && ! -L "$PY" ]]; then
    return 0
  fi

  echo "Checking existing .venv..."
  if venv_python_wrapper_broken "$VENV"; then
    echo "Repairing broken python wrapper (was looping on python3.13.real)..."
    repair_venv_python_wrapper "$VENV"
  fi
  wrap_venv_python_for_pyexpat "$VENV"
  ensure_pip_shims "$VENV"
}

bootstrap_pip() {
  if venv_has_pip; then
    return 0
  fi

  echo "Bootstrapping pip into .venv..."

  if command -v uv >/dev/null 2>&1; then
    echo "Using uv to install pip..."
    uv pip install --python "$PY" pip setuptools wheel
    return 0
  fi

  if [[ ! -f "$GET_PIP" ]]; then
    echo "Downloading get-pip.py..."
    curl -fsSL https://bootstrap.pypa.io/get-pip.py -o "$GET_PIP"
  fi

  "$PY" "$GET_PIP" --no-setuptools --no-wheel
  "$PY" -m pip install setuptools wheel
}

create_venv_with_uv() {
  echo "Creating .venv with uv (standalone Python, avoids Homebrew pyexpat)..."
  rm -rf "$VENV"
  uv python install 3.12
  uv venv --python 3.12 "$VENV"
  patch_activate
  uv pip install --python "$PY" pip setuptools wheel rich pyyaml maturin
  ensure_pip_shims "$VENV"
}

create_venv() {
  local base_python="$1"
  echo "Creating fresh .venv (without bundled pip)..."
  rm -rf "$VENV"
  "$base_python" -m venv --without-pip "$VENV"
  patch_activate
  wrap_venv_python_for_pyexpat "$VENV"
  bootstrap_pip
  ensure_pip_shims "$VENV"
}

ensure_venv() {
  fix_homebrew_pyexpat

  if [[ "${USE_UV:-}" == "1" ]] && command -v uv >/dev/null 2>&1; then
    create_venv_with_uv
    return 0
  fi

  local base_python
  base_python="$(choose_python)"
  echo "Using interpreter: $($base_python --version 2>&1)"

  if ! pyexpat_ok "$base_python" && command -v uv >/dev/null 2>&1; then
    echo "Homebrew pyexpat broken; falling back to uv-managed Python..." >&2
    create_venv_with_uv
    return 0
  fi

  require_pyexpat "$base_python"

  prepare_existing_venv

  if [[ ! -x "$PY" && ! -L "$PY" ]] || [[ ! -f "$VENV/bin/activate" ]] || ! venv_has_pip; then
    create_venv "$base_python"
  fi

  require_pyexpat "$PY"
  wrap_venv_python_for_pyexpat "$VENV"
  ensure_pip_shims "$VENV"
  patch_activate

  echo "Installing Python deps (rich, pyyaml, maturin)..."
  if command -v uv >/dev/null 2>&1; then
    uv pip install --python "$PY" rich pyyaml maturin
  else
    "$PY" -m pip install rich pyyaml maturin
  fi
}

find_maturin() {
  if [[ -x "$VENV/bin/maturin" ]]; then
    echo "$VENV/bin/maturin"
  elif command -v maturin >/dev/null 2>&1; then
    command -v maturin
  else
    return 1
  fi
}

install_rust_extension() {
  local maturin_bin
  maturin_bin="$(find_maturin)" || {
    echo "maturin not found after install." >&2
    exit 1
  }

  echo "Building Zyvor Janus PyO3 extension with $maturin_bin ..."
  export VIRTUAL_ENV="$VENV"
  export PATH="$VENV/bin:$PATH"
  fix_homebrew_pyexpat
  wrap_venv_python_for_pyexpat "$VENV"

  if "$maturin_bin" develop; then
    return 0
  fi

  echo "maturin develop failed; retrying with --skip-install + wheel install..." >&2
  "$maturin_bin" develop --skip-install
  WHEEL="$(ls -t "$ROOT"/target/wheels/zyvor_janus-*.whl 2>/dev/null | head -1 || true)"
  if [[ -z "$WHEEL" ]]; then
    echo "No wheel produced under target/wheels/" >&2
    exit 1
  fi
  if command -v uv >/dev/null 2>&1; then
    uv pip install --python "$PY" --force-reinstall --no-deps "$WHEEL"
  else
    "$PY" -m pip install --force-reinstall --no-deps "$WHEEL"
  fi
}

verify_install() {
  fix_homebrew_pyexpat
  export PYTHONPATH="$ROOT/python${PYTHONPATH:+:$PYTHONPATH}"
  "$PY" -c "import zyvor_janus._zyvor_janus; import rich; print('OK: zyvor_janus extension + rich')"
}

ensure_venv
install_rust_extension
verify_install

cat <<EOF

Dev environment ready.

  source .venv/bin/activate
  ./scripts/run_live_dashboard.sh --config configs/clusters/small_h100.yaml

If pyexpat breaks again in a new shell, re-run:
  ./scripts/fix_pyexpat.sh
  source .venv/bin/activate

Or recreate with uv-managed Python (avoids Homebrew pyexpat entirely):
  brew install uv && rm -rf .venv && USE_UV=1 ./scripts/setup_dev.sh

EOF
