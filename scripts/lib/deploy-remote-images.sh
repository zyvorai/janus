#!/usr/bin/env bash
# shellcheck shell=bash
# Remote image build + k3s import helpers (sourced by deploy-remote.sh on the build host).
# Pattern copied from ../zynera/scripts/lib/deploy-remote-images.sh (slimmed for API + Web).

set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${PATH:-}"

ZYVOR_JANUS_IMAGE_TAG="${ZYVOR_JANUS_IMAGE_TAG:-0.1.0}"

zyvor_janus_k3s_ctr() {
  if [ -n "${ZYVOR_JANUS_K3S_CTR_CMD:-}" ]; then
    "${ZYVOR_JANUS_K3S_CTR_CMD[@]}" "$@"
    return
  fi
  if [ -x /usr/local/bin/k3s ]; then
    sudo /usr/local/bin/k3s ctr "$@"
  else
    sudo k3s ctr "$@"
  fi
}

zyvor_janus_detect_k3s_ctr() {
  if [ -x /usr/local/bin/k3s ] && sudo /usr/local/bin/k3s ctr images ls &>/dev/null; then
    ZYVOR_JANUS_K3S_CTR_CMD=(sudo /usr/local/bin/k3s ctr)
  elif command -v k3s &>/dev/null && sudo k3s ctr images ls &>/dev/null; then
    ZYVOR_JANUS_K3S_CTR_CMD=(sudo k3s ctr)
  elif command -v k3s &>/dev/null && k3s ctr images ls &>/dev/null; then
    ZYVOR_JANUS_K3S_CTR_CMD=(k3s ctr)
  else
    return 1
  fi
}

zyvor_janus_container_cmd() {
  if command -v podman &>/dev/null; then
    echo podman
  elif command -v docker &>/dev/null; then
    echo docker
  else
    return 1
  fi
}

zyvor_janus_import_image() {
  local tag="$1"
  local ctr="$2"
  echo "  Importing ${tag} into k3s..."
  # Short names like zyvor-janus-web:0.1.0 resolve to docker.io/library/<name> in k3s.
  for ref in "docker.io/library/${tag}" "docker.io/${tag}" "${tag}" "localhost/${tag}"; do
    zyvor_janus_k3s_ctr images rm "${ref}" 2>/dev/null || true
  done
  "$ctr" save "$tag" | zyvor_janus_k3s_ctr images import -
  zyvor_janus_k3s_ctr images tag "localhost/${tag}" "docker.io/library/${tag}" 2>/dev/null || true
  zyvor_janus_k3s_ctr images tag "localhost/${tag}" "docker.io/${tag}" 2>/dev/null || true
  zyvor_janus_k3s_ctr images tag "localhost/${tag}" "${tag}" 2>/dev/null || true
}

zyvor_janus_build_api() {
  local ctr="$1"
  local tag="zyvor-janus-api:${ZYVOR_JANUS_IMAGE_TAG}"
  echo "  Building ${tag}..."
  "$ctr" build -t "$tag" -f deploy/docker/Dockerfile.api .
  zyvor_janus_import_image "$tag" "$ctr"
}

zyvor_janus_build_web() {
  local ctr="$1"
  local tag="zyvor-janus-web:${ZYVOR_JANUS_IMAGE_TAG}"
  local api_url="${ZYVOR_JANUS_API_URL:-http://zyvor-janus-api:8080}"
  local ws_url="${NEXT_PUBLIC_ZYVOR_JANUS_WS_URL:-}"
  echo "  Building ${tag} (ZYVOR_JANUS_API_URL=${api_url} NEXT_PUBLIC_ZYVOR_JANUS_WS_URL=${ws_url:-unset})..."
  local args=(
    -t "$tag"
    -f web/Dockerfile
    --build-arg "ZYVOR_JANUS_API_URL=${api_url}"
  )
  if [ -n "$ws_url" ]; then
    args+=(--build-arg "NEXT_PUBLIC_ZYVOR_JANUS_WS_URL=${ws_url}")
  fi
  "$ctr" build "${args[@]}" web
  zyvor_janus_import_image "$tag" "$ctr"
}

zyvor_janus_build_all_images() {
  local ctr
  ctr="$(zyvor_janus_container_cmd)" || {
    echo "MISSING: podman or docker required to build images"
    return 1
  }
  zyvor_janus_detect_k3s_ctr || {
    echo "MISSING: k3s ctr required to import images (tried sudo /usr/local/bin/k3s ctr)"
    return 1
  }
  echo "  Container runtime: ${ctr}"
  zyvor_janus_build_api "$ctr"
  zyvor_janus_build_web "$ctr"
  echo "  Images ready: zyvor-janus-api:${ZYVOR_JANUS_IMAGE_TAG} zyvor-janus-web:${ZYVOR_JANUS_IMAGE_TAG}"
}
