# tunnel-server (v2.0)

Hub-side endpoint of the OpenSpoke reverse tunnel. Each spoke's
`tunnel-client` opens a persistent gRPC stream to this Deployment;
anything inside the hub that dials
`tunnel-server.mcp.svc.cluster.local:<per-spoke port>` reaches the
spoke's local MCP endpoint (default `mcp-company1:8000`).

## Layout

- `Deployments/tunnel-server.yaml` — the tunnel server, `mcp` namespace
- `Services/tunnel-server.yaml` — per-spoke reverse-listen ports
- `Ingress/tunnel-server-ingress.yaml` — public gRPC entry point
  (`hub.example.com`, TLS terminated at your ingress)
- `fleet.yaml` — Fleet takeOwnership

## Configuration

`TUNNEL_SPOKES` (from Secret `tunnel-server-spokes`) is a comma-separated
list of `<spoke_id>:<hub_listen_port>:<pre-shared token>`. Add one entry
per spoke you have onboarded. See
[`docs/installation/reverse-tunnel.md`](../../../docs/installation/reverse-tunnel.md).

Rotate the pre-shared tokens by editing the Secret and restarting the
tunnel-server (`restarted-at` annotation bump). Phase 3 will replace
static tokens with Ed25519 keys.

## Concept

See [`docs/concepts/reverse-tunnel.md`](../../../docs/concepts/reverse-tunnel.md)
for how the reverse tunnel works and when to use it instead of a public
per-spoke tunnel.
