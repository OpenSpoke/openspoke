# Application spoke example

This directory holds a minimal example of an application-spoke
deployment: a single application namespace that borrows the hub's AI
through a `tunnel-client` + `kernel-proxy` sidecar and nothing else.
See [`../../docs/concepts/application-spoke.md`](../../docs/concepts/application-spoke.md)
for the concept.

## What the example ships

- `secret.yaml` — a `Secret` holding the pre-shared `TUNNEL_TOKEN`
  and the `SPOKE_ID` (which must start with `app-`).
- `deployment.yaml` — a `Deployment` running `tunnel-client` with
  `KERNEL_PROXY_LISTEN=0.0.0.0:8080`. The container publishes an
  in-cluster HTTP endpoint that transparently forwards to the hub
  kernel over the reverse tunnel.
- `service.yaml` — a `Service` in front of the sidecar so the
  application's own workloads reach it as
  `http://kernel-proxy.<namespace>.svc.cluster.local:8080`.

## Prerequisites

1. The hub has been told about this application spoke:

   ```
   spoke_register(cluster_name="app-example", kind="application", ...)
   ```

   The registration mints the pre-shared `TUNNEL_TOKEN` that goes
   into `secret.yaml`. See
   [`../../docs/concepts/application-spoke.md`](../../docs/concepts/application-spoke.md).

2. The application cluster has outbound connectivity to
   `hub.example.com:443` (the same address a cluster spoke would
   use).

## Customising

- Change `app-example` throughout to your own `app-<name>` slug.
  The `app-` prefix is enforced hub-side.
- Change the `namespace` from `example-app` to whatever namespace
  the application already runs in.
- Change `HUB_URL` in the `Deployment` to your own hub endpoint.

## Consuming the sidecar from the application

Anywhere the application code would have talked to the hub kernel
directly (`https://hub.example.com/core/chat/...`), point it at the
in-cluster URL instead:

```
POST http://kernel-proxy.example-app.svc.cluster.local:8080/core/chat/complete
```

`tunnel-client` wraps the request in an `HttpRequest` frame and
`tunnel-server` at the hub end applies the application-spoke path
allowlist (only `/core/chat/*`, `/core/orchestrate`,
`/core/embedding/*`, `/core/vectorstore/*`, `/core/graph/run`, and
`/core/handoff/*` are permitted; everything else returns HTTP 403).

## What this example does not ship

An application spoke deliberately does not carry:

- OpenSearch, Memgraph, Qdrant, or Valkey — use the application's
  own datastore, or none.
- `node-shell` DaemonSet — Kubernetes privileged execution is a
  cluster-spoke feature.
- A spoke-local kernel or MCP server — the hub kernel is the one
  authoritative endpoint.

If any of the above is needed, install a *cluster* spoke instead
(`examples/fleet-gitrepos/spoke-mcp.yaml` +
`examples/fleet-gitrepos/spoke-tunnel-client.yaml`). Cluster spokes
and application spokes can coexist in the same cluster.
