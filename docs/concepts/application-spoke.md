# Application spokes

Version-first: **added in v3.0**.

Before v3.0 a *spoke* meant a Kubernetes cluster registered with the
hub and running the full cluster-spoke bundle (per-spoke datastore,
kernel, MCP server, `node-shell` DaemonSet, tunnel client). That model
is well suited to running whole environments — a customer install, a
staging cluster, a lab — but it is heavy for the far more common case
where an application wants nothing more than to borrow the hub's AI:
its embeddings, its knowledge base, its LLM routing. Standing up a
cluster spoke just to reach `POST /core/chat` is more machinery than
the workload needs.

An **application spoke** is a second, lighter kind of spoke. One
application is one application spoke. It ships as two containers —
a `tunnel-client` and a `kernel-proxy` sidecar — deployed into the
application's own namespace. It does not include the cluster-spoke
datastore, the spoke kernel, the spoke MCP, `node-shell`, or any
Kubernetes-privileged component. Instead, the application code talks
to `http://kernel-proxy:8080/core/...` as if it were the hub kernel,
and the sidecar tunnels the call, over the same reverse tunnel that
carries cluster-spoke traffic, to the hub.

Cluster spokes and application spokes coexist happily in the same
cluster. A cluster spoke and one or more application spokes can share
a single Kubernetes cluster; two different application spokes for two
different products can sit side by side.

## The `kind` field

Every spoke document in the OpenSearch `spoke_clusters` index now
carries a `kind`:

- `kind: cluster` — a Kubernetes cluster running the full cluster
  spoke bundle. This is the pre-v3.0 default and existing entries are
  backfilled to it.
- `kind: application` — an application-scoped, sidecar-only spoke.
  The `SPOKE_ID` is required to start with `app-` (for example
  `app-example`).
- `kind: native` — a spoke running on the host operating system with
  no Kubernetes, such as `nsmt`. See
  [`native-spoke.md`](native-spoke.md).

The topology view distinguishes the three by **shape**, not colour:
cluster spokes are circles, native spokes are hexagons, and
application spokes are diamonds (a square rotated 45°).

The MCP `spoke_register` and `spoke_update` tools take a `kind`
parameter for this field. `spoke_register` defaults to `cluster`
when the parameter is omitted, and pre-v3.0 documents without an
explicit `kind` are read as `cluster` by every downstream consumer
(the topology view, ACL checks, and the `tunnel-server` path
allowlist described below).

## Authorisation

The hub restricts what an application spoke may call. `tunnel-server`
looks up the caller's `kind` on every request and applies a path
allowlist when the kind is `application`.

Allowed for `kind: application`:

- `/core/chat/*`
- `/core/orchestrate`
- `/core/embedding/*`
- `/core/vectorstore/*`
- `/core/graph/run`
- `/core/handoff/*` (knowledge base read/write)

Refused (403) for `kind: application`:

- `/core/k8s/*`
- `/core/cluster/*`
- `/core/fleet_state/*`
- `/core/spoke/*`
- `/core/spawn/*`
- other cluster-management endpoints

A cluster spoke retains full access.

## Wire format

The tunnel protocol carries application-spoke traffic in the same
gRPC stream as the pre-existing TCP-tunnelling variants. Two new
`ClientFrame` / `ServerFrame` variants are added at field number 40:

- `HttpRequest { method, path, headers, body }`
- `HttpResponse { status, headers, body, error }`

The spoke picks its own stream ids for these frames in the top half
of the `int64` range (`>= 2^63`) so they can never collide with the
hub's own TCP stream ids. Handshake and authentication are unchanged;
the initial `Connect` call is still what proves the spoke's identity.

Because the new variants are additive and use proto3 default
behaviour, older builds of `tunnel-server` and `tunnel-client` simply
ignore them, so cluster spokes running the v2.x tunnel continue to
work while application spokes roll out.

## What a deployment looks like

An application-spoke deployment is small enough to fit in one
manifest:

- one `Deployment` running `tunnel-client` (with
  `KERNEL_PROXY_LISTEN=0.0.0.0:8080` set) and no `TARGET`
- one `Secret` containing the pre-shared `TUNNEL_TOKEN` and the
  `SPOKE_ID` (which must start with `app-`)
- one `Service` in front of the sidecar so the application code can
  reach `http://kernel-proxy:8080`

No CRDs, no RBAC, no DaemonSet. Uninstall is the reverse. There is
no separate MCP or database to migrate.

## Cost accounting

Every hub-kernel call proxied through a `tunnel-server` remembers the
originating `spoke_id`. The Cost view aggregates those records so a
site running many application spokes can see per-application AI
spend without extra instrumentation.

## Related

- [`reverse-tunnel.md`](reverse-tunnel.md) — the underlying tunnel
  protocol.
- [`native-spoke.md`](native-spoke.md) — the third `kind`, for
  running a spoke directly on the host operating system.
