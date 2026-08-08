# Install: reverse tunnel (v2.0)

Set up `tunnel-server` on the hub and `tunnel-client` on each spoke so
the hub can reach every spoke's MCP endpoint over a single outbound
gRPC stream per spoke.

Read [`docs/concepts/reverse-tunnel.md`](../concepts/reverse-tunnel.md)
first for the shape of what you are deploying.

## 1. Build (or pull) the images

The default `image:` fields in the manifests reference
`ghcr.io/openspoke/openspoke-tunnel-{server,client}:v0.1.0`. Replace
these with your own registry before applying:

```sh
export REGISTRY=ghcr.io/YOUR-ORG    # or your private registry

# Hub side
cd hub/images/tunnel-server
./build.sh v0.1.0

# Spoke side
cd ../../../spoke/images/tunnel-client
./build.sh v0.1.0
```

Then update:

- `hub/manifests/tunnel-server/Deployments/tunnel-server.yaml`
- `spoke/kubernetes/template/tunnel-client/Deployments/tunnel-client.yaml`

to point at your pushed tag.

## 2. Choose per-spoke listen ports and issue tokens

Pick one internal TCP port per spoke that lives on the hub side. The
sample manifests reserve `10001`, `10002`, `10003` for `spoke-1`,
`spoke-2`, `spoke-3`. Extend the `ports:` list on both the Deployment
and the Service if you have more than three spokes.

Issue one strong random token per spoke, at least 32 bytes:

```sh
SPOKE1_TOKEN=$(head -c 32 /dev/urandom | base64)
SPOKE2_TOKEN=$(head -c 32 /dev/urandom | base64)
```

## 3. Create the hub-side Secret

```sh
kubectl -n mcp create secret generic tunnel-server-spokes \
  --from-literal=spokes="spoke-1:10001:${SPOKE1_TOKEN},spoke-2:10002:${SPOKE2_TOKEN}"
```

The format is
`<spoke_id>:<hub_listen_port>:<pre-shared token>[,<spoke_id>:<port>:<token>...]`.
Each entry must match one `containerPort` on the Deployment and one
`port` on the Service.

## 4. Apply the hub-side manifests

Either put `hub/manifests/tunnel-server/` under Fleet as its own
GitRepo, or apply directly for a first-run smoke test:

```sh
kubectl apply -f hub/manifests/tunnel-server/Deployments/tunnel-server.yaml
kubectl apply -f hub/manifests/tunnel-server/Services/tunnel-server.yaml
kubectl apply -f hub/manifests/tunnel-server/Ingress/tunnel-server-ingress.yaml
```

Confirm the pod is Ready and the Ingress is accepting gRPC by tailing
its logs (`kubectl -n mcp logs deploy/tunnel-server`) — you should see
`"tunnel-server ready" ... spokes=N`.

## 5. Create the spoke-side Secret (per spoke)

On each spoke cluster, out-of-band:

```sh
kubectl -n rag-spoke create secret generic tunnel-client-secret \
  --from-literal=spoke_id=spoke-1 \
  --from-literal=token="${SPOKE1_TOKEN}"
```

The `spoke_id` and `token` must match one entry in the hub's
`tunnel-server-spokes` Secret.

## 6. Apply the spoke-side manifests

Put `spoke/kubernetes/template/tunnel-client/` under Fleet with your
usual fan-out targeting. The ConfigMap
(`tunnel-client-config`) is identical across every spoke — only the
Secret differs.

For a first-run test on one spoke:

```sh
kubectl apply -f spoke/kubernetes/template/tunnel-client/ConfigMaps/tunnel-client-config.yaml
kubectl apply -f spoke/kubernetes/template/tunnel-client/Deployments/tunnel-client.yaml
```

## 7. Verify the tunnel

On the hub, tail tunnel-server logs — a connected spoke prints
`"spoke connected" spoke_id=spoke-1 port=10001`.

Then, from any pod on the hub, dial the spoke's MCP endpoint through
the tunnel:

```sh
kubectl -n mcp exec -it deploy/mcp-company1 -- \
  curl -sv http://tunnel-server:10001/sse
```

You should get a 200 response from the spoke's MCP server.

Prometheus metrics live on `tunnel-server:9090/metrics`:

- `openspoke_tunnel_connected_spokes`
- `openspoke_tunnel_streams_open{spoke_id}`
- `openspoke_tunnel_bytes_sent_total{spoke_id}`
- `openspoke_tunnel_bytes_recv_total{spoke_id}`
- `openspoke_tunnel_handshake_failures_total{reason}`

## 8. Migrate off cloudflared (optional)

Once the tunnel is verified end-to-end, either scale the v1
`cloudflared` Deployment to `replicas: 0` on each spoke, or drop it from
your Fleet GitRepo entirely. See the deprecation banner at the top of
`spoke/kubernetes/template/rag-spoke/Deployments/cloudflared.yaml`.

## Rotating a spoke's token

1. Regenerate the token (`head -c 32 /dev/urandom | base64`).
2. Update the hub-side Secret (`tunnel-server-spokes`) with the new token.
3. Restart `tunnel-server` (`restarted-at` annotation bump). Any live
   session with the old token stays connected until it disconnects
   naturally.
4. Update the spoke-side Secret (`tunnel-client-secret`) and let the
   pod pick it up (`restarted-at` bump).

Phase 3 will replace this with Ed25519 keys; the operational shape
stays the same — Secret in, Secret out — but the compromised-token
window shrinks to a single handshake.
