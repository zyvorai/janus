#!/usr/bin/env bash
# Shared helpers for Zyvor Janus shell scripts.
fix_homebrew_pyexpat() {
  if [[ -d /opt/homebrew/opt/expat/lib ]]; then
    export DYLD_LIBRARY_PATH="/opt/homebrew/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
  elif [[ -d /usr/local/opt/expat/lib ]]; then
    export DYLD_LIBRARY_PATH="/usr/local/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
  fi
}

# Homebrew python@3.13 links pyexpat against Homebrew expat but the loader often
# picks /usr/lib/libexpat.1.dylib. Wrap venv interpreters so pip/maturin inherit
# the correct library path even when DYLD_* is stripped from the parent shell.
venv_python_wrapper_broken() {
  local venv="${1:-}"
  local py313="$venv/bin/python3.13"
  local target="$venv/bin/python3.13.bin"
  [[ -f "$py313" && ! -L "$py313" ]] || return 1
  grep -q "Zyvor Janus: Homebrew pyexpat wrapper" "$py313" 2>/dev/null || return 1
  [[ -e "$target" ]] || return 0
  if [[ -f "$target" && ! -L "$target" ]] && grep -q "Zyvor Janus: Homebrew pyexpat wrapper" "$target" 2>/dev/null; then
    return 0
  fi
  return 1
}

repair_venv_python_wrapper() {
  local venv="${1:-}"
  local py313="$venv/bin/python3.13"
  local target="$venv/bin/python3.13.bin"
  local base_python=""

  for candidate in python3.13 python3.12 python3; do
    if command -v "$candidate" >/dev/null 2>&1; then
      base_python="$(command -v "$candidate")"
      break
    fi
  done

  if [[ -z "$base_python" ]]; then
    return 1
  fi

  rm -f "$py313" "${py313}.real" "$target"
  ln -sf "$base_python" "$target"
  return 0
}

wrap_venv_python_for_pyexpat() {
  local venv="${1:-}"
  [[ -n "$venv" ]] || return 0
  local py313="$venv/bin/python3.13"
  local target="$venv/bin/python3.13.bin"
  [[ -e "$py313" || -e "$target" ]] || return 0

  if venv_python_wrapper_broken "$venv"; then
    repair_venv_python_wrapper "$venv" || return 1
  fi

  if [[ -f "$py313" && ! -L "$py313" ]] && grep -q "Zyvor Janus: Homebrew pyexpat wrapper" "$py313" 2>/dev/null; then
    [[ -e "$target" ]] || repair_venv_python_wrapper "$venv"
    return 0
  fi

  if [[ ! -e "$target" ]]; then
    if [[ -L "$py313" ]]; then
      mv "$py313" "$target"
    elif [[ -L "${py313}.real" ]]; then
      mv "${py313}.real" "$target"
    elif [[ -x "$py313" ]]; then
      mv "$py313" "$target"
    else
      return 0
    fi
  fi

  rm -f "$py313" "${py313}.real"
  cat >"$py313" <<'EOF'
#!/usr/bin/env bash
# Zyvor Janus: Homebrew pyexpat wrapper
if [[ -d /opt/homebrew/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/opt/homebrew/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
elif [[ -d /usr/local/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/usr/local/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
fi
exec "$(dirname "$0")/python3.13.bin" "$@"
EOF
  chmod +x "$py313"
}

ensure_pip_shims() {
  local venv="${1:-}"
  [[ -n "$venv" ]] || return 0
  cat >"$venv/bin/pip" <<'EOF'
#!/usr/bin/env bash
if [[ -d /opt/homebrew/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/opt/homebrew/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
elif [[ -d /usr/local/opt/expat/lib ]]; then
  export DYLD_LIBRARY_PATH="/usr/local/opt/expat/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
fi
exec "$(dirname "$0")/python" -m pip "$@"
EOF
  chmod +x "$venv/bin/pip"
  ln -sf pip "$venv/bin/pip3"
}

zyvor_janus_root() {
  cd "$(dirname "${BASH_SOURCE[1]}")/.." && pwd
}

require_venv() {
  local root="$1"
  local py="$root/.venv/bin/python"
  if [[ ! -x "$py" ]]; then
    echo "No .venv found. Run ./scripts/setup_dev.sh first." >&2
    exit 1
  fi
  if ! "$py" -c "import pip" >/dev/null 2>&1; then
    echo ".venv is incomplete (no pip). Run ./scripts/setup_dev.sh" >&2
    exit 1
  fi
}

ensure_server_deps() {
  local py="$1"
  if ! "$py" -c "import fastapi, uvicorn" >/dev/null 2>&1; then
    echo "Installing server deps (fastapi, uvicorn, websockets, pydantic)..."
    if command -v uv >/dev/null 2>&1; then
      uv pip install --python "$py" fastapi "uvicorn[standard]" websockets pydantic rich pyyaml
    else
      "$py" -m pip install fastapi "uvicorn[standard]" websockets pydantic rich pyyaml
    fi
  fi
}

ensure_zynera_extension() {
  local py="$1"
  export PYTHONPATH="${2}/python${PYTHONPATH:+:$PYTHONPATH}"
  if ! "$py" -c "import zyvor_janus._zyvor_janus" >/dev/null 2>&1; then
    echo "Zyvor Janus Rust extension not built. Run ./scripts/setup_dev.sh" >&2
    exit 1
  fi
}

ensure_web_deps() {
  local web_dir="$1"
  if [[ ! -d "$web_dir/node_modules" ]]; then
    echo "Installing web deps (npm install)..."
    (cd "$web_dir" && npm install)
  fi
}
