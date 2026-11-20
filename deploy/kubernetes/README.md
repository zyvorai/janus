# Zyvor Janus on Kubernetes

Deploy the Zyvor Janus web dashboard (Rust API + Next.js UI) to a Kubernetes cluster.

## Architecture

```text
Ingress (nginx)
  /api/auth/*  → zyvor-janus-web:3000   (Next.js login)
  /api/*       → zyvor-janus-api:8080   (Rust API, bearer-auth required)
  /ws/*        → zyvor-janus-api:8080   (WebSocket replay, ticket-auth required)
  /*           → zyvor-janus-web:3000   (UI)
```

## Prerequisites

- Docker
- `kubectl` configured for your cluster
- For local testing without a real cluster: [kind](https://kind.sigs.k8s.io) (recommended) or [minikube](https://minikube.sigs.k8s.io) — see [Local cluster (kind)](#local-cluster-kind) below
- NGINX Ingress Controller (`ingressClassName: nginx`) — only needed if you use the Ingress; `kubectl port-forward` works without it (default for local testing)
- A default StorageClass for the API outputs PVC (kind and minikube both ship one by default)

## 1. Build images

From the repo root:

```bash
chmod +x deploy/build-images.sh
./deploy/build-images.sh
```

This also rewrites the `images:` block in [`kustomization.yaml`](kustomization.yaml) to match what was just built, so `kubectl apply -k .` always deploys exactly what you built — no manual edit needed.

Push to your registry:

```bash
REGISTRY=registry.example.com/your-org TAG=0.1.0 PUSH=1 ./deploy/build-images.sh
```

## Local cluster (kind)

No remote cluster needed — this runs entirely on your machine.

```bash
# 1. Create a local cluster
kind create cluster --name zyvor-janus

# 2. Build images (auto-syncs kustomization.yaml, see above)
./deploy/build-images.sh

# 3. Load the locally-built images into kind — kind's nodes run their own
#    containerd, separate from your host Docker daemon, so images built with
#    `docker build` aren't visible until loaded explicitly
kind load docker-image zyvor-janus-api:0.1.0 zyvor-janus-web:0.1.0 --name zyvor-janus

# 4. Configure auth (see "2. Configure auth" below) and deploy (see "3. Deploy" below)
cd deploy/kubernetes
cp secret.example.yaml secret.yaml   # edit credentials
kubectl apply -f secret.yaml
kubectl apply -k .
kubectl -n zyvor-janus rollout status deploy/zyvor-janus-api
kubectl -n zyvor-janus rollout status deploy/zyvor-janus-web

# 5. Reach the dashboard (port-forward — simplest, no ingress controller needed)
kubectl -n zyvor-janus port-forward svc/zyvor-janus-web 3000:3000 &
kubectl -n zyvor-janus port-forward svc/zyvor-janus-api 8080:8080 &
open http://localhost:3000   # log in with credentials from secret.yaml
```

If you used a different `TAG` when building, pass the same tag to `kind load docker-image` (e.g. `zyvor-janus-api:$TAG zyvor-janus-web:$TAG`).

### Optional: exercise the real Ingress on kind

Only needed if you want to test [`ingress.yaml`](ingress.yaml) itself rather than port-forward — requires installing ingress-nginx and a local hosts entry:

```bash
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
kubectl -n ingress-nginx wait --for=condition=ready pod \
  --selector=app.kubernetes.io/component=controller --timeout=120s

echo "127.0.0.1 zyvor-janus.example.com" | sudo tee -a /etc/hosts
open http://zyvor-janus.example.com
```

### Optional: minikube instead of kind

```bash
minikube start -p zyvor-janus
eval $(minikube -p zyvor-janus docker-env)   # build directly into minikube's runtime
./deploy/build-images.sh                  # skip `kind load` — no separate load step needed
kubectl apply -f secret.yaml && kubectl apply -k .
minikube -p zyvor-janus service zyvor-janus-web -n zyvor-janus --url
```

### Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `ImagePullBackOff` | Images built locally but never loaded into the cluster | Run `kind load docker-image` (or use the minikube `docker-env` approach) |
| `web` pod `CreateContainerConfigError` | Secret applied after/without the Deployment | `kubectl apply -f secret.yaml` — the pod recovers automatically, no restart needed |
| Ingress returns 404 / connection refused | No `/etc/hosts` entry, or ingress-nginx not installed | Use port-forward instead, or follow the Ingress steps above |
| `kubectl apply -k .` deploys the wrong image tag | `kustomization.yaml` out of sync with what was built | Re-run `./deploy/build-images.sh` — it rewrites the `images:` block automatically |

## 2. Configure auth

```bash
cd deploy/kubernetes
cp secret.example.yaml secret.yaml
# Edit ZYVOR_JANUS_DASHBOARD_PASSWORD, ZYVOR_JANUS_AUTH_SECRET,
# ZYVOR_JANUS_API_KEY, and ZYVOR_JANUS_WS_TICKET_SECRET
kubectl apply -f secret.yaml
```

`ZYVOR_JANUS_API_KEY` and `ZYVOR_JANUS_WS_TICKET_SECRET` are shared between
the `web` and `api` pods via the same `zyvor-janus-auth` Secret (`api.yaml`'s
Deployment now has `envFrom` on it too, not just `web.yaml`'s). The API
rejects every request without a matching bearer token except `/api/health`;
`web`'s `middleware.ts` injects it automatically so the dashboard itself
needs no separate configuration.

## 3. Deploy

```bash
kubectl apply -k deploy/kubernetes
kubectl -n zyvor-janus get pods
```

Edit [`ingress.yaml`](ingress.yaml) and set `host:` to your domain before applying, or patch after deploy.

## 4. Verify

Port-forward (no ingress):

```bash
kubectl -n zyvor-janus port-forward svc/zyvor-janus-web 3000:3000
kubectl -n zyvor-janus port-forward svc/zyvor-janus-api 8080:8080
```

Open http://localhost:3000 and log in with credentials from `secret.yaml`.

API health:

```bash
kubectl -n zyvor-janus exec deploy/zyvor-janus-api -- \
  curl -fsS http://127.0.0.1:8080/api/health
```

## Configuration

| Item | Location |
|------|----------|
| Cluster YAML configs | Baked into API image (`configs/`) |
| Run artifacts | PVC `zyvor-janus-outputs` → `/app/outputs` |
| Dashboard login + API bearer token + WS ticket secret | Secret `zyvor-janus-auth` |
| Ingress host | `ingress.yaml` |

Cluster YAML configs are baked into the API image under `configs/` at build time. There is currently no supported way to mount extra configs without rebuilding the image — a ConfigMap-based override is not wired into `api.yaml` today.

## CLI-only batch job

Apply [`job.example.yaml`](job.example.yaml) for a one-off simulation, or:

```bash
kubectl apply -f deploy/kubernetes/job.example.yaml
kubectl -n zyvor-janus logs job/zyvor-janus-run-once
```

For interactive use, prefer the web UI or `POST /api/runs` on the API service.

Note: this Job is applied directly (`kubectl apply -f`), not through `kubectl apply -k`, so it does not pick up the tag/registry `build-images.sh` syncs into `kustomization.yaml`. If you built a non-default tag, edit its `image:` field before applying.

## Notes

- **Run history** is in-memory in the API process; restarting the API pod clears the run list. Completed artifacts under `/app/outputs/runs` persist on the PVC.
- **Web rewrites** use `ZYVOR_JANUS_API_URL=http://zyvor-janus-api:8080` at image build time as a fallback when traffic goes through the Next.js pod instead of ingress path rules.
- **API auth**: every route except `/api/health` requires `Authorization: Bearer <ZYVOR_JANUS_API_KEY>`. This is why `ingress.yaml` routing `/api` and `/ws` directly to the API Service (bypassing the Next.js pod's session-cookie check) is safe — the API itself now enforces auth regardless of which path reaches it.
- **Production**: change default credentials; set strong, distinct values for `ZYVOR_JANUS_AUTH_SECRET`, `ZYVOR_JANUS_API_KEY`, and `ZYVOR_JANUS_WS_TICKET_SECRET`; use TLS on ingress.

## Teardown

```bash
kubectl delete -k deploy/kubernetes
kubectl delete -f deploy/kubernetes/secret.yaml
```
