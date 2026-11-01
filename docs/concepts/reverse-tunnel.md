# Reverse tunnel (spoke -> hub)

*Introduced in v2.0.*

## Why

In v1.0, each spoke exposed its local MCP endpoint to the hub through a
per-spoke public tunnel (Cloudflare Tunnel via `cloudflared`). That
approach works but has three drawbacks:

1. Every spoke needs a public hostname (`spoke-XXXX.example.com`).
2. Adding a spoke requires an out-of-band DNS + tunnel provisioning
   step, which is easy to forget and hard to automate cleanly.
3. Tunnel outages surface as "one spoke is unreachable", making it
   hard to distinguish network problems from application problems.

The v2.0 reverse tunnel inverts the direction: each spoke opens a
persistent gRPC stream *outbound* to a single hub-side endpoint
(`hub.example.com`), and the hub uses that stream to reach the spoke.

## Shape

```
+-----------------+                 hub.example.com                  +---------------+
|                 |    gRPC (443)                                    |               |
|  tunnel-client  | --------------------------> tunnel-server        |  mcp-company1 |
|   (rag-spoke)   |    Tunnel.Connect(stream)   (namespace mcp)      |  (rag-spoke)  |
|                 | <-------------------------- listens on           |               |
+-----------------+                              per-spoke ports     +-------^-------+
       ^                                         10001, 10002, ...           |
       | dial mcp-company1:8000 on request                                   |
       +---------------------------------------------------------------------+
                                    (via tunnel)
```

- One `Tunnel.Connect(stream)` per spoke.
- Any TCP dial inside the hub to
  `tunnel-server.mcp.svc.cluster.local:<per-spoke port>` is transparently
  forwarded through the existing stream to the spoke's MCP endpoint.
- The hub does not need to know a spoke's IP address or hostname; it only
  needs to know which per-spoke listen port maps to which `spoke_id`.

## Protocol

Defined in `hub/images/tunnel-server/proto/tunnel.proto` (and the
byte-identical spoke-side copy). The wire format is intentionally small:

- `HandshakeChallenge` (hub -> spoke): 32-byte nonce + `issued_at`.
- `HandshakeResponse` (spoke -> hub): `spoke_id` + `signature`.
- `OpenStream` (hub -> spoke): open a new virtual TCP connection.
- `Data` (both directions): payload bytes keyed by `stream_id`.
- `CloseStream` (both directions): half-close or full close.
- `Ping` / `Pong`: keepalive and (future) RTT metric.

Every virtual TCP connection is identified by a hub-assigned `stream_id`.

## HTTP-proxy mode (v3.0)

v3.0 adds a second mode to the existing tunnel, sharing the same
`Tunnel.Connect(stream)`: a spoke-originated HTTP proxy that lets the
spoke call hub-kernel endpoints as if the kernel were local. This is
the transport that powers [application spokes](application-spoke.md),
where the whole point of the spoke is to borrow the hub's AI without
running any hub-side machinery on the spoke cluster.

Two new frame variants are added at field number 40 (additive, so
older `tunnel-server` and `tunnel-client` builds simply ignore them
and the pre-existing 6 variants continue to work):

- `HttpRequest` (spoke -> hub): `method`, `path`, `headers`, `body`.
- `HttpResponse` (hub -> spoke): `status`, `headers`, `body`, `error`.

Because both directions now assign stream ids, the spoke picks its
ids from the top half of the `int64` range (`>= 2^63`) so that they
can never collide with the hub-assigned ids used by the pre-v3.0 TCP
variants. Handshake is unchanged; the initial `HandshakeResponse`
still proves the spoke's identity for the whole stream.

The hub side of the proxy is enforced in `tunnel-server`: when it
receives a `HttpRequest`, it looks up the caller's `kind` in
`spoke_clusters` (`cluster`, `application`, or `native`) and applies a
path allowlist for `kind: application`. Allowed paths cover
`/core/chat/*`, `/core/orchestrate`, `/core/embedding/*`,
`/core/vectorstore/*`, `/core/graph/run`, and `/core/handoff/*`;
cluster-management paths (`/core/k8s/*`, `/core/cluster/*`,
`/core/fleet_state/*`, `/core/spoke/*`, `/core/spawn/*`) are refused
with an `HttpResponse{status=403}`. `kind: cluster` keeps full access.

Metrics are exposed at `tunnel-server:9090` as
`openspoke_tunnel_application_requests_total{spoke_id, path, status}`
and `openspoke_tunnel_application_request_duration_seconds{spoke_id, path}`.

## Authentication (Phase 1 vs Phase 3)

- **Phase 1 (v2.0)**: `signature` is the raw bytes of a pre-shared
  token. The token is compared against a static per-spoke registry in
  `TUNNEL_SPOKES`. This is deliberately minimal so the tunnel can be
  bootstrapped before a key management story is in place.
- **Phase 3 (planned)**: `signature` is
  `Ed25519(nonce || spoke_id || issued_at)`. The hub verifies the
  signature against the spoke's registered public key and rejects
  replayed nonces. Rotating a spoke's key is a Secret update on the
  spoke plus a public-key update on the hub.

## When to use it (vs. the v1 per-spoke tunnel)

Use the reverse tunnel when:

- Spokes sit behind NAT or private networks and have outbound connectivity.
- You want spoke onboarding to be a single Secret + Fleet target update.
- You want per-spoke traffic and error rates observable from one place
  (Prometheus scrape on `tunnel-server:9090`).

Stay on the v1 `cloudflared` path when:

- You already rely on the public per-spoke hostname for something else
  (browser access, CI hitting the spoke, etc.).
- Your spokes' outbound access is more restricted than their inbound.

The two paths coexist safely; running both is redundant but not harmful.

## Related files

- Hub side: `hub/manifests/tunnel-server/`, `hub/images/tunnel-server/`
- Spoke side: `spoke/kubernetes/template/tunnel-client/`,
  `spoke/images/tunnel-client/`
- Install steps: [`docs/installation/reverse-tunnel.md`](../installation/reverse-tunnel.md)
