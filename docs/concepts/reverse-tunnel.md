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
