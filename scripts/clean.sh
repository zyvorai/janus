#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

# Remove build artifacts, caches, and generated outputs from the repo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DRY_RUN=0
CLEAN_DEV=0

usage() {
  cat <<'EOF'
Usage: ./scripts/clean.sh [OPTIONS]

Remove generated files (Rust target/, Python caches, web build output, runs/, etc.).

Options:
  -n, --dry-run   Print what would be removed without deleting
  --dev, --all    Also remove .venv and web/node_modules (requires setup_dev.sh)
  -h, --help      Show this help

Examples:
  ./scripts/clean.sh
  ./scripts/clean.sh --dry-run
  ./scripts/clean.sh --dev && ./scripts/setup_dev.sh
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -n | --dry-run)
      DRY_RUN=1
      shift
      ;;
    --dev | --all)
      CLEAN_DEV=1
      shift
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
done

removed=0

remove_path() {
  local path="$1"
  [[ -e "$path" || -L "$path" ]] || return 0
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "would remove: $path"
  else
    rm -rf "$path"
    echo "removed: $path"
  fi
  removed=$((removed + 1))
}

find_remove() {
  local pattern="$1"
  shift
  local -a find_args=("$@")
  local -a prune_args=()

  if [[ "$CLEAN_DEV" != 1 ]]; then
    prune_args+=(\( -path "$ROOT/.venv" -o -path "$ROOT/.venv/*" \) -prune -o)
  fi

  while IFS= read -r -d '' path; do
    remove_path "$path"
  done < <(
    find "$ROOT" \
      \( -path "$ROOT/.git" -o -path "$ROOT/.git/*" \) -prune -o \
      "${prune_args[@]}" \
      "${find_args[@]}" -print0 2>/dev/null
  )
}

echo "Cleaning Zyvor Janus workspace (${ROOT})..."

# Rust / Cargo
remove_path "$ROOT/target"
find_remove "mutants.out" -type d -name "mutants.out*"
find_remove "debug dirs" -type d -name "debug" \
  \( -path "*/target/debug" -o -path "*/crates/*/debug" \)

# Python build + caches
remove_path "$ROOT/dist"
remove_path "$ROOT/build"
remove_path "$ROOT/.eggs"
find_remove "__pycache__" -type d -name "__pycache__"
find_remove "*.py[cod]" -type f \( -name "*.pyc" -o -name "*.pyo" -o -name "*.pyd" \)
find_remove "egg-info" -type d -name "*.egg-info"
find_remove "native libs" -type f \( -name "*.so" -o -name "*.dylib" \) \
  ! -path "$ROOT/.venv/*"
find_remove "wheels" -type f -name "*.whl"
remove_path "$ROOT/.mypy_cache"
remove_path "$ROOT/.pytest_cache"
remove_path "$ROOT/.ruff_cache"

# Setup bootstrap + sim outputs
remove_path "$ROOT/scripts/get-pip.py"
remove_path "$ROOT/outputs"
remove_path "$ROOT/runs"

# Web (Next.js)
remove_path "$ROOT/web/.next"
remove_path "$ROOT/web/out"
remove_path "$ROOT/web/dist"
find_remove "web logs" -path "$ROOT/web/*" -type f -name "*.log"

# macOS / editor noise at repo root
remove_path "$ROOT/.DS_Store"

if [[ "$CLEAN_DEV" == 1 ]]; then
  remove_path "$ROOT/.venv"
  find_remove "extra venvs" -maxdepth 1 -type d -name ".venv*"
  remove_path "$ROOT/web/node_modules"
fi

if [[ "$removed" -eq 0 ]]; then
  echo "Nothing to clean."
elif [[ "$DRY_RUN" == 1 ]]; then
  echo "Dry run complete ($removed item(s))."
else
  echo "Clean complete ($removed item(s))."
  if [[ "$CLEAN_DEV" == 1 ]]; then
    echo "Dev deps removed. Re-run: ./scripts/setup_dev.sh"
  fi
fi
