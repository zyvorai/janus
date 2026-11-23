# Setura Gateway (optional)

`setura-gateway.yaml` is a Gateway API alternative to the Ingress. The default deploy is unchanged. To use it, install the controller once, then apply the Gateway manifest instead of the Ingress:

```sh
setura gateway install --image ghcr.io/zyvorai/zyvor-setura-gateway:0.7.0 --namespace setura-system | kubectl apply -f -
kubectl apply -f deploy/kubernetes/setura-gateway.yaml
```
