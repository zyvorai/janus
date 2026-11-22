#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

# shellcheck shell=bash
# Terminal UI helpers for deploy-remote.sh (colors, progress, banners).

if [[ -t 1 ]]; then
  _FUI_RESET=$'\033[0m'
  _FUI_BOLD=$'\033[1m'
  _FUI_DIM=$'\033[2m'
  _FUI_CYAN=$'\033[0;36m'
  _FUI_BLUE=$'\033[0;34m'
  _FUI_MAGENTA=$'\033[0;35m'
  _FUI_GREEN=$'\033[0;32m'
  _FUI_YELLOW=$'\033[0;33m'
  _FUI_RED=$'\033[0;31m'
  _FUI_WHITE=$'\033[0;97m'
else
  _FUI_RESET=''
  _FUI_BOLD=''
  _FUI_DIM=''
  _FUI_CYAN=''
  _FUI_BLUE=''
  _FUI_MAGENTA=''
  _FUI_GREEN=''
  _FUI_YELLOW=''
  _FUI_RED=''
  _FUI_WHITE=''
fi

ZYVOR_JANUS_DEPLOY_START=${SECONDS:-0}
_ZYVOR_JANUS_UI_STEP=0
_ZYVOR_JANUS_UI_TOTAL=7

zyvor_janus_ui_set_total() {
  _ZYVOR_JANUS_UI_TOTAL="${1:-7}"
}

zyvor_janus_ui_elapsed() {
  local s=$((SECONDS - ZYVOR_JANUS_DEPLOY_START))
  printf '%02d:%02d' $((s / 60)) $((s % 60))
}

zyvor_janus_ui_banner() {
  local title="${1:-Zyvor Janus Remote Deploy}"
  local subtitle="${2:-}"
  echo ""
  echo "${_FUI_MAGENTA}${_FUI_BOLD}  ╔══════════════════════════════════════════════════════════════════╗${_FUI_RESET}"
  echo "${_FUI_MAGENTA}${_FUI_BOLD}  ║${_FUI_RESET}  ${_FUI_CYAN}${_FUI_BOLD}✦  ${title}${_FUI_RESET}$(printf '%*s' $((58 - ${#title})) '')${_FUI_MAGENTA}${_FUI_BOLD}║${_FUI_RESET}"
  if [[ -n "$subtitle" ]]; then
    echo "${_FUI_MAGENTA}${_FUI_BOLD}  ║${_FUI_RESET}  ${_FUI_DIM}${subtitle}${_FUI_RESET}$(printf '%*s' $((58 - ${#subtitle})) '')${_FUI_MAGENTA}${_FUI_BOLD}║${_FUI_RESET}"
  fi
  echo "${_FUI_MAGENTA}${_FUI_BOLD}  ╚══════════════════════════════════════════════════════════════════╝${_FUI_RESET}"
  echo ""
}

zyvor_janus_ui_kv() {
  local key="$1" val="$2"
  printf "  ${_FUI_DIM}%-12s${_FUI_RESET} %s\n" "${key}:" "${val}"
}

zyvor_janus_ui_progress() {
  local current="$1" total="$2"
  local width=28
  local filled=$((current * width / total))
  local empty=$((width - filled))
  local pct=$((current * 100 / total))
  printf "  ${_FUI_BLUE}["
  printf '%*s' "$filled" '' | tr ' ' '█'
  printf '%*s' "$empty" '' | tr ' ' '░'
  printf "]${_FUI_RESET} ${_FUI_DIM}%3d%%${_FUI_RESET}  ${_FUI_DIM}elapsed %s${_FUI_RESET}\n" "$pct" "$(zyvor_janus_ui_elapsed)"
}

zyvor_janus_ui_step() {
  _ZYVOR_JANUS_UI_STEP="$1"
  local total="${2:-$_ZYVOR_JANUS_UI_TOTAL}"
  local label="${3:-}"
  echo ""
  zyvor_janus_ui_progress "$_ZYVOR_JANUS_UI_STEP" "$total"
  echo "${_FUI_CYAN}${_FUI_BOLD}  ▶ Step ${_ZYVOR_JANUS_UI_STEP}/${total}${_FUI_RESET}  ${label}"
  echo "${_FUI_DIM}  ────────────────────────────────────────────────────────────────${_FUI_RESET}"
}

info()  { echo "${_FUI_GREEN}  ✓${_FUI_RESET} $*"; }
warn()  { echo "${_FUI_YELLOW}  ⚠${_FUI_RESET}  $*"; }
error() { echo "${_FUI_RED}  ✗${_FUI_RESET} $*"; exit 1; }
step()  { zyvor_janus_ui_step "$@"; }

zyvor_janus_ui_divider() {
  echo "${_FUI_DIM}  ────────────────────────────────────────────────────────────────${_FUI_RESET}"
}

zyvor_janus_ui_success() {
  local host="$1" user="$2"
  echo ""
  echo "${_FUI_GREEN}${_FUI_BOLD}  ╔══════════════════════════════════════════════════════════════════╗${_FUI_RESET}"
  echo "${_FUI_GREEN}${_FUI_BOLD}  ║${_FUI_RESET}  ${_FUI_WHITE}${_FUI_BOLD}🎉  Deployment complete${_FUI_RESET}  ${_FUI_DIM}${user}@${host}${_FUI_RESET}$(printf '%*s' $((35 - ${#user} - ${#host})) '')${_FUI_GREEN}${_FUI_BOLD}║${_FUI_RESET}"
  echo "${_FUI_GREEN}${_FUI_BOLD}  ║${_FUI_RESET}  ${_FUI_DIM}Total time: $(zyvor_janus_ui_elapsed)${_FUI_RESET}$(printf '%*s' 44 '')${_FUI_GREEN}${_FUI_BOLD}║${_FUI_RESET}"
  echo "${_FUI_GREEN}${_FUI_BOLD}  ╚══════════════════════════════════════════════════════════════════╝${_FUI_RESET}"
  echo ""
}

zyvor_janus_ui_panel() {
  local title="$1"
  shift
  echo ""
  echo "${_FUI_BOLD}  ┌─ ${title}${_FUI_RESET}"
  while [[ $# -gt 0 ]]; do
    echo "  │  $1"
    shift
  done
  echo "${_FUI_BOLD}  └──────────────────────────────────────────────────────────────${_FUI_RESET}"
}
