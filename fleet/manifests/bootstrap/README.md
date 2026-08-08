# fleet/manifests/bootstrap/

Day-zero Fleet installation. Applied once directly with kubectl to bring
the controller online, after which everything is self-managed via
`../managed/`.

Subdirectories:

- `Deployments/` — cloudflared (or your ingress equivalent) that exposes
  the fleet-server kube-apiserver to downstream fleet-agents
- `HelmCharts/` — the Fleet controller Helm install
- `Namespaces/` — `fleet-default` and `fleet-ui`
- `Secrets/` — Secrets referenced above. Create real values with
  `kubectl create secret ...` before applying — see
  [`docs/installation/fleet.md`](../../../docs/installation/fleet.md).
