# Deploying Zyvor Janus

Zyvor Janus can run locally via scripts or on Kubernetes with container images.

## Local (dev)

```bash
./scripts/setup_dev.sh
cd web && npm install && cd ..
./scripts/run_web_dashboard.sh
```

See [ui_dashboard.md](ui_dashboard.md).

## Docker images

From the repo root:

```bash
./deploy/build-images.sh
# optional: REGISTRY=registry.example.com/org TAG=0.1.0 PUSH=1 ./deploy/build-images.sh
```

Images:

| Dockerfile | Role |
|------------|------|
| `deploy/docker/Dockerfile.api` | FastAPI + Rust sim (`zyvor-janus-api`) |
| `web/Dockerfile` | Next.js UI (`zyvor-janus-web`) |

## Kubernetes

Full steps (secrets, ingress, PVC, kustomize): **[deploy/kubernetes/README.md](../deploy/kubernetes/README.md)**.

```bash
kubectl apply -k deploy/kubernetes
```

No cluster yet? See **[Local cluster (kind)](../deploy/kubernetes/README.md#local-cluster-kind)** for a from-scratch walkthrough (kind create cluster → build → load images → deploy → port-forward).

There is no `docker-compose.yml` in-tree; use the scripts above or Kubernetes.
