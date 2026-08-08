# tunnel-client (v2.0)

Spoke-side endpoint of the OpenSpoke reverse tunnel. Establishes a
persistent gRPC stream to `hub.example.com` (`tunnel-server`) so the hub
can reach this spoke's local MCP endpoint without the spoke needing
a public IP or a per-spoke public tunnel.

Replaces the `cloudflared` per-spoke tunnel that shipped with v1.0.
`cloudflared` is retained under `rag-spoke/Deployments/cloudflared.yaml`
for backward compatibility; running both is safe but redundant.

## Layout

- `Deployments/tunnel-client.yaml` — the client Deployment (`rag-spoke` ns)
- `ConfigMaps/tunnel-client-config.yaml` — hub URL, target, TLS flag
  (identical across every spoke)
- `fleet.yaml` — Fleet takeOwnership, keepResources

## Secret (per spoke, out-of-band)

```sh
kubectl -n rag-spoke create secret generic tunnel-client-secret \
  --from-literal=spoke_id=spoke-1 \
  --from-literal=token=<pre-shared token issued by hub admin>
```

Rotate the token by re-creating the Secret and restarting the pod
(`restarted-at` annotation bump).

## Concept & install

See [`docs/concepts/reverse-tunnel.md`](../../../../docs/concepts/reverse-tunnel.md)
and [`docs/installation/reverse-tunnel.md`](../../../../docs/installation/reverse-tunnel.md).
