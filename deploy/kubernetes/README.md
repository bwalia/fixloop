# Fixloop on Kubernetes

These manifests run `fixloop serve` (webhooks, API and workers in one pod)
with the Anthropic API for reviews and SQLite on a persistent volume.

## Before you start

1. **GitHub App.** Run `fixloop github create-app --name <name> --webhook-url
   https://<your-host>/webhooks/github` on your workstation and install the
   App on the repositories to review (see [docs/github.md](../../docs/github.md)).
   You get an App id, a private key file and a webhook secret.
2. **Public HTTPS URL.** GitHub must reach `https://<host>/webhooks/github`.
   `ingress.example.yaml` shows an nginx + cert-manager Ingress.
3. **Anthropic API key.**

## Deploy

```sh
kubectl create namespace fixloop
kubectl -n fixloop create secret generic fixloop \
  --from-literal=anthropic-api-key="$ANTHROPIC_API_KEY" \
  --from-literal=github-webhook-secret="$WEBHOOK_SECRET" \
  --from-file=github-private-key=./<app-slug>.private-key.pem
# Optional, for Postgres instead of SQLite:
#   --from-literal=database-url='postgres://user:pass@host:5432/fixloop'

# Set github.app_id in configmap.yaml, then:
kubectl apply -k deploy/kubernetes
kubectl -n fixloop rollout status deploy/fixloop
kubectl -n fixloop port-forward svc/fixloop 8080:80 &
curl -s localhost:8080/readyz
```

Then apply your Ingress (from `ingress.example.yaml`) and check the App's
**Advanced → Recent deliveries** page on GitHub after opening a pull request.

## Notes

- **Image:** `ghcr.io/bwalia/fixloop:edge` follows `main`; release tags
  (`v0.2.0`, `0.2`) and `sha-<commit>` tags pin a version. Change `newTag` in
  `kustomization.yaml`. Images are built for amd64 and arm64.
- **Security:** non-root, read-only root filesystem, no capabilities, no
  service-account token, `restricted` Pod Security on the namespace. Only the
  webhook path needs to be public.
- **Scaling:** with SQLite keep one replica. With Postgres you can raise
  `replicas`, or split roles with `FIXLOOP_SERVER__ROLE=api` / `worker`.
- **Shutdown:** on SIGTERM the server stops taking work and finishes current
  jobs; `terminationGracePeriodSeconds` allows for that. Jobs left behind are
  re-leased after their lease expires.
