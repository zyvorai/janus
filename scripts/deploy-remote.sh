#!/usr/bin/env bash
# ============================================================================
# deploy-remote.sh — Deploy Zyvor Janus (API + Web) to a remote k3s host
# ============================================================================
# Pattern copied from ../forge/scripts/deploy-remote.sh and slimmed for Zyvor Janus:
#   1. Rsync repo to remote ~/.deployment/zyvor-janus
#   2. Build zyvor-janus-api + zyvor-janus-web with podman
#   3. Import images into k3s
#   4. Apply Kubernetes manifests + NodePort services
#   5. Verify health
#
# Usage:
#   ./scripts/deploy-remote.sh <host> [user] [password]
#   ./scripts/deploy-remote.sh 212.8.248.187 sus
#   ./scripts/deploy-remote.sh 212.8.248.187 sus --quick        # skip image rebuild
#   ./scripts/deploy-remote.sh 212.8.248.187 sus --skip-images  # manifests only
#   ./scripts/deploy-remote.sh 212.8.248.187 sus --uninstall
#
# Environment:
#   DEPLOY_HOST / DEPLOY_USER / DEPLOY_PASS
#   DEPLOY_DIR                 absolute remote checkout (overrides layout)
#   DEPLOYMENTS_SUBDIR         default: .deployment  (matches lab hosts)
#   ZYVOR_JANUS_CHECKOUT          default: zyvor-janus
#   ZYVOR_JANUS_IMAGE_TAG         default: 0.1.0
#   ZYVOR_JANUS_UI_NODE_PORT      default: 30300
#   ZYVOR_JANUS_API_NODE_PORT     default: 30818  (30808 is Zyvor Relay on lab)
#   ZYVOR_JANUS_DASHBOARD_USER    default: Admin
#   ZYVOR_JANUS_DASHBOARD_PASSWORD default: Admin@321
#   ZYVOR_JANUS_AUTH_SECRET       default: zyvor-janus-dev-secret
#   ZYVOR_JANUS_API_KEY           default: randomly generated each run
#   ZYVOR_JANUS_WS_TICKET_SECRET  default: randomly generated each run
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/deploy-remote-ui.sh
source "${SCRIPT_DIR}/lib/deploy-remote-ui.sh"
ZYVOR_JANUS_DEPLOY_START=${SECONDS}

QUICK_MODE=false
UNINSTALL_MODE=false
SKIP_IMAGES=false
SKIP_SYNC=false
POSITIONAL=()
for arg in "$@"; do
  case "$arg" in
    --quick)       QUICK_MODE=true; SKIP_IMAGES=true ;;
    --skip-images) SKIP_IMAGES=true ;;
    --skip-sync)   SKIP_SYNC=true ;;
    --uninstall)   UNINSTALL_MODE=true ;;
    --help|-h)
      sed -n '2,35p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) POSITIONAL+=("$arg") ;;
  esac
done

HOST="${POSITIONAL[0]:-${DEPLOY_HOST:-}}"
USER="${POSITIONAL[1]:-${DEPLOY_USER:-root}}"
PASS="${POSITIONAL[2]:-${DEPLOY_PASS:-}}"

[ -n "$HOST" ] || error "Usage: $0 <host> [user] [password] [--quick|--skip-images|--uninstall]"

DEPLOYMENTS_SUBDIR="${DEPLOYMENTS_SUBDIR:-.deployment}"
ZYVOR_JANUS_CHECKOUT="${ZYVOR_JANUS_CHECKOUT:-zyvor-janus}"
ZYVOR_JANUS_IMAGE_TAG="${ZYVOR_JANUS_IMAGE_TAG:-0.1.0}"
UI_NODE_PORT="${ZYVOR_JANUS_UI_NODE_PORT:-30300}"
API_NODE_PORT="${ZYVOR_JANUS_API_NODE_PORT:-30818}"
DASH_USER="${ZYVOR_JANUS_DASHBOARD_USER:-Admin}"
DASH_PASS="${ZYVOR_JANUS_DASHBOARD_PASSWORD:-Admin@321}"
AUTH_SECRET="${ZYVOR_JANUS_AUTH_SECRET:-zyvor-janus-dev-secret}"
# No fixed dev fallback for these two -- unlike AUTH_SECRET, a well-known
# default here is directly usable as a bearer token against the live API,
# so each deploy gets a fresh random one unless the caller pins one.
API_KEY="${ZYVOR_JANUS_API_KEY:-$(openssl rand -hex 32)}"
WS_TICKET_SECRET="${ZYVOR_JANUS_WS_TICKET_SECRET:-$(openssl rand -hex 32)}"

REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
[ -f "$REPO_DIR/deploy/docker/Dockerfile.api" ] || error "Not in zyvor-janus repo: $REPO_DIR"
[ -f "$REPO_DIR/web/Dockerfile" ] || error "Missing web/Dockerfile in $REPO_DIR"

if [ -n "${DEPLOY_DIR:-}" ]; then
  REMOTE_DIR="$DEPLOY_DIR"
  RSYNC_DEST="${USER}@${HOST}:${DEPLOY_DIR}/"
  REMOTE_RM_TARGET="$(printf '%q' "$DEPLOY_DIR")"
else
  REMOTE_DIR="\$HOME/${DEPLOYMENTS_SUBDIR}/${ZYVOR_JANUS_CHECKOUT}"
  RSYNC_DEST="${USER}@${HOST}:${DEPLOYMENTS_SUBDIR}/${ZYVOR_JANUS_CHECKOUT}/"
  REMOTE_RM_TARGET="\"\$HOME/${DEPLOYMENTS_SUBDIR}/${ZYVOR_JANUS_CHECKOUT}\""
fi

REMOTE_SH_PREFIX="cd \"${REMOTE_DIR}\""

_SSH_KEEPALIVE_OPTS="-o ServerAliveInterval=15 -o ServerAliveCountMax=6"
_KUBECONFIG_PREFIX='if [ -r "$HOME/.kube/config" ]; then export KUBECONFIG="$HOME/.kube/config"; fi;'

_ssh() {
  if [ -n "$PASS" ]; then
    SSHPASS="$PASS" sshpass -e ssh -o StrictHostKeyChecking=accept-new ${_SSH_KEEPALIVE_OPTS} "${USER}@${HOST}" "$_KUBECONFIG_PREFIX" "$@"
  else
    ssh -o StrictHostKeyChecking=accept-new ${_SSH_KEEPALIVE_OPTS} "${USER}@${HOST}" "$_KUBECONFIG_PREFIX" "$@"
  fi
}

_rsync() {
  local ssh_cmd="ssh -o StrictHostKeyChecking=accept-new ${_SSH_KEEPALIVE_OPTS}"
  if [ -n "$PASS" ]; then
    ssh_cmd="sshpass -e $ssh_cmd"
    SSHPASS="$PASS" rsync -az --delete \
      -e "$ssh_cmd" \
      --exclude '.git/' \
      --exclude 'node_modules/' \
      --exclude 'web/node_modules/' \
      --exclude 'web/.next/' \
      --exclude '.venv/' \
      --exclude 'target/' \
      --exclude 'outputs/' \
      --exclude '*.pyc' \
      --exclude '__pycache__/' \
      "$@"
  else
    rsync -az --delete \
      -e "$ssh_cmd" \
      --exclude '.git/' \
      --exclude 'node_modules/' \
      --exclude 'web/node_modules/' \
      --exclude 'web/.next/' \
      --exclude '.venv/' \
      --exclude 'target/' \
      --exclude 'outputs/' \
      --exclude '*.pyc' \
      --exclude '__pycache__/' \
      "$@"
  fi
}

zyvor_janus_ui_set_total 6
zyvor_janus_ui_banner "Zyvor Janus Remote Deploy" "${USER}@${HOST}"
zyvor_janus_ui_kv "Checkout" "${DEPLOYMENTS_SUBDIR}/${ZYVOR_JANUS_CHECKOUT}"
zyvor_janus_ui_kv "Image tag" "${ZYVOR_JANUS_IMAGE_TAG}"
zyvor_janus_ui_kv "UI port" "NodePort ${UI_NODE_PORT}"
zyvor_janus_ui_kv "API port" "NodePort ${API_NODE_PORT}"
zyvor_janus_ui_kv "Mode" "$($UNINSTALL_MODE && echo uninstall || ($SKIP_IMAGES && echo skip-images || echo full))"

# ── Uninstall ──
if $UNINSTALL_MODE; then
  step 1 2 "Remove Zyvor Janus from cluster"
  _ssh "${REMOTE_SH_PREFIX}; kubectl delete -k deploy/kubernetes --ignore-not-found; kubectl delete -f deploy/kubernetes/secret.yaml --ignore-not-found; kubectl delete ns zyvor-janus --ignore-not-found" || true
  info "Namespace/resources deleted"
  step 2 2 "Optional: remove remote checkout"
  warn "Leaving ${REMOTE_DIR} in place (re-run with: ssh ${USER}@${HOST} rm -rf ${REMOTE_RM_TARGET})"
  zyvor_janus_ui_success "$HOST" "$USER"
  exit 0
fi

# ── Sync ──
step 1 6 "Rsync source → remote"
if $SKIP_SYNC; then
  warn "Skipping rsync (--skip-sync)"
else
  _ssh "mkdir -p \"\$HOME/${DEPLOYMENTS_SUBDIR}/${ZYVOR_JANUS_CHECKOUT}\""
  _rsync "${REPO_DIR}/" "${RSYNC_DEST}"
  info "Synced to ${REMOTE_DIR}"
fi

# ── Auth secret ──
step 2 6 "Configure dashboard auth secret"
_ssh "${REMOTE_SH_PREFIX}; cat > deploy/kubernetes/secret.yaml <<EOF
apiVersion: v1
kind: Secret
metadata:
  name: zyvor-janus-auth
  namespace: zyvor-janus
  labels:
    app.kubernetes.io/name: zyvor-janus
type: Opaque
stringData:
  ZYVOR_JANUS_DASHBOARD_USER: ${DASH_USER}
  ZYVOR_JANUS_DASHBOARD_PASSWORD: ${DASH_PASS}
  ZYVOR_JANUS_AUTH_SECRET: ${AUTH_SECRET}
  ZYVOR_JANUS_API_KEY: ${API_KEY}
  ZYVOR_JANUS_WS_TICKET_SECRET: ${WS_TICKET_SECRET}
EOF
kubectl apply -f deploy/kubernetes/namespace.yaml
kubectl apply -f deploy/kubernetes/secret.yaml"
info "Secret zyvor-janus-auth applied (user=${DASH_USER})"

# ── Images ──
step 3 6 "Build + import container images"
if $SKIP_IMAGES; then
  warn "Skipping image builds (--quick / --skip-images)"
else
  WS_URL="ws://${HOST}:${API_NODE_PORT}"
  _ssh "export ZYVOR_JANUS_IMAGE_TAG='${ZYVOR_JANUS_IMAGE_TAG}' ZYVOR_JANUS_API_URL='http://zyvor-janus-api:8080' NEXT_PUBLIC_ZYVOR_JANUS_WS_URL='${WS_URL}'; ${REMOTE_SH_PREFIX}; . scripts/lib/deploy-remote-images.sh; zyvor_janus_build_all_images"
  info "Images imported into k3s"
fi

# ── Apply manifests ──
step 4 6 "Apply Kubernetes manifests"
_ssh "export ZYVOR_JANUS_IMAGE_TAG='${ZYVOR_JANUS_IMAGE_TAG}' ZYVOR_JANUS_UI_NODE_PORT='${UI_NODE_PORT}' ZYVOR_JANUS_API_NODE_PORT='${API_NODE_PORT}'; ${REMOTE_SH_PREFIX}; \
cat > deploy/kubernetes/nodeport.yaml <<EOF
apiVersion: v1
kind: Service
metadata:
  name: zyvor-janus-web
  namespace: zyvor-janus
  labels:
    app.kubernetes.io/name: zyvor-janus
    app.kubernetes.io/component: web
spec:
  type: NodePort
  selector:
    app.kubernetes.io/name: zyvor-janus
    app.kubernetes.io/component: web
  ports:
    - name: http
      port: 3000
      targetPort: http
      nodePort: ${UI_NODE_PORT}
---
apiVersion: v1
kind: Service
metadata:
  name: zyvor-janus-api
  namespace: zyvor-janus
  labels:
    app.kubernetes.io/name: zyvor-janus
    app.kubernetes.io/component: api
spec:
  type: NodePort
  selector:
    app.kubernetes.io/name: zyvor-janus
    app.kubernetes.io/component: api
  ports:
    - name: http
      port: 8080
      targetPort: http
      nodePort: ${API_NODE_PORT}
EOF
python3 - <<'PY'
from pathlib import Path
import os, re
path = Path('deploy/kubernetes/kustomization.yaml')
tag = os.environ['ZYVOR_JANUS_IMAGE_TAG']
content = path.read_text()
block = (
    '  # BEGIN zyvor-janus-images (managed by deploy/build-images.sh — do not hand-edit)\n'
    f'  - name: zyvor-janus-api\n    newName: zyvor-janus-api\n    newTag: \"{tag}\"\n'
    f'  - name: zyvor-janus-web\n    newName: zyvor-janus-web\n    newTag: \"{tag}\"\n'
    '  # END zyvor-janus-images\n'
)
new, n = re.subn(r'  # BEGIN zyvor-janus-images.*?  # END zyvor-janus-images\n', block, content, flags=re.S)
assert n == 1, n
path.write_text(new)
print('kustomization tag ->', tag)
PY
kubectl apply -k deploy/kubernetes
kubectl apply -f deploy/kubernetes/nodeport.yaml
kubectl -n zyvor-janus rollout restart deploy/zyvor-janus-api deploy/zyvor-janus-web || true
kubectl -n zyvor-janus rollout status deploy/zyvor-janus-api --timeout=300s
kubectl -n zyvor-janus rollout status deploy/zyvor-janus-web --timeout=300s"
info "Workloads rolled out"

# ── Firewall (best effort) ──
step 5 6 "Open NodePorts (best-effort ufw)"
_ssh "sudo ufw allow ${UI_NODE_PORT}/tcp comment zyvor-janus-ui 2>/dev/null || true; sudo ufw allow ${API_NODE_PORT}/tcp comment zyvor-janus-api 2>/dev/null || true; true" || true
info "Firewall rules attempted for ${UI_NODE_PORT}/${API_NODE_PORT}"

# ── Verify ──
step 6 6 "Verify health endpoints"
sleep 3
UI_URL="http://${HOST}:${UI_NODE_PORT}"
API_URL="http://${HOST}:${API_NODE_PORT}"
if curl -fsS --connect-timeout 8 "${UI_URL}/login" >/dev/null; then
  info "UI reachable: ${UI_URL}/login"
else
  warn "UI not reachable yet at ${UI_URL}/login — check: kubectl -n zyvor-janus get pods,svc"
fi
if body="$(curl -fsS --connect-timeout 8 "${API_URL}/api/health" 2>/dev/null)" \
  && printf '%s' "$body" | grep -q '"status"[[:space:]]*:[[:space:]]*"ok"'; then
  info "API healthy: ${API_URL}/api/health"
else
  warn "API health check failed at ${API_URL}/api/health (need JSON status=ok; is the NodePort free?)"
fi

zyvor_janus_ui_success "$HOST" "$USER"
zyvor_janus_ui_panel "Access" \
  "Web UI:  ${UI_URL}" \
  "API:     ${API_URL}/api/health" \
  "Login:   ${DASH_USER} / ${DASH_PASS}" \
  "WS URL:  ws://${HOST}:${API_NODE_PORT}" \
  "" \
  "Redeploy fast: ./scripts/deploy-remote.sh ${HOST} ${USER} --quick"
