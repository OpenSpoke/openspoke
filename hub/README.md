# hub/

Components running on the OpenSpoke hub cluster.

- `manifests/` — Kubernetes manifests (Deployments, ConfigMaps, Services, etc.)
- `manifests.static/` — one-shot resources applied directly with kubectl
  (Namespaces, Secrets, PVCs, and other manifests not managed by Fleet)
- `images/` — container image build contexts (Dockerfiles + build.sh)

See [`docs/installation/hub.md`](../docs/installation/hub.md) for the full
install walkthrough.
