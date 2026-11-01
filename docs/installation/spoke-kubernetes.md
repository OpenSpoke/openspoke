# Spoke Installation Guide (Kubernetes Template)

This guide walks through installing an OpenSpoke **kubernetes** spoke —
a downstream cluster that runs a light RAG kernel plus MCP server, and
optionally the node-shell daemon for host-level operations. Manifests
live under `spoke/kubernetes/template/`.

For the standalone Go binary variant (single executable, cross-platform)
see `spoke-native.md`.

## Prerequisites

- A Kubernetes cluster (v1.28+) with `kubectl` context configured
- A container registry your cluster can pull from
- Reachability from the spoke cluster to the Hub kernel service
  (typically via a Cloudflare Tunnel — see the cloudflared Deployment
  under `spoke/kubernetes/template/rag-spoke/Deployments/`)
- Hub already installed and reachable (see `hub.md`)

## Step 1 — Create namespaces

```sh
kubectl apply -f spoke/kubernetes/template/rag-spoke/Namespace/
kubectl create namespace mcp
kubectl create namespace node-shell
```

## Step 2 — Build and push images

Retag the shared images to your registry and update the corresponding
Deployment `image:` fields before applying:

| Short name                     | Used by                                                                     |
| ------------------------------ | --------------------------------------------------------------------------- |
| `rag-backend-gpu:v1.0.3`       | `spoke/kubernetes/template/kernel/Deployments/rag-backend-kernel.yaml` (3 places) |
| `rag-mcp-server:latest`        | `spoke/kubernetes/template/mcp/Deployments/mcp-company.yaml`                |
| `node-shell:latest` (optional) | `spoke/kubernetes/template/node-shell/DaemonSets/`                          |

## Step 3 — Create Secrets

The repository ships without real credentials. Create every Secret that
the Deployments reference before applying them.

### `rag-spoke` namespace

```sh
# Anthropic API key for the spoke kernel
kubectl create secret generic anthropic-api-key \
  -n rag-spoke \
  --from-literal=api-key='sk-ant-api03-...'

# Optional: token used to bootstrap knowledge migration from the hub.
# If you skip this the KNOWLEDGE_MIGRATE_TOKEN env stays unset (marked
# optional in the Deployment) and migration jobs are simply disabled.
kubectl create secret generic knowledge-migrate-token \
  -n rag-spoke \
  --from-literal=token="$(openssl rand -hex 32)"

# Cloudflare Tunnel credentials for the spoke -> hub reverse tunnel
# (skip if you use a different transport)
kubectl create secret generic cloudflared-bundle \
  -n rag-spoke \
  --from-file=credentials.json=./credentials.json
```

### `mcp` namespace

```sh
kubectl create secret generic mcp-api-key \
  -n mcp \
  --from-literal=api-key='<same-value-as-hub-side-mcp-api-key>'

kubectl create secret generic keycloak-client-secret \
  -n mcp \
  --from-literal=client-secret='<keycloak-client-secret>'
```

### `node-shell` namespace (only if you deploy node-shell)

```sh
# Bearer token used by the hub MCP to authenticate against the spoke's
# node-shell endpoint. Reuse the same value in your MCP config on the
# hub side.
kubectl create secret generic node-shell-api-key \
  -n node-shell \
  --from-literal=api-key="$(openssl rand -hex 32)"
```

## Step 4 — Customize environment variables

Placeholder values in the manifests that must be replaced before
applying:

### `spoke/kubernetes/template/mcp/Deployments/mcp-company.yaml`

| env var                 | Placeholder                    | Set to                                  |
| ----------------------- | ------------------------------ | --------------------------------------- |
| `KEYCLOAK_URL`          | `https://keycloak.example.com` | your Keycloak public URL                |
| `KEYCLOAK_INTERNAL_URL` | `https://keycloak.example.com` | your Keycloak internal URL              |
| `BOOTSTRAP_ADMINS`      | `admin`                        | comma-separated `preferred_username`s   |

### `spoke/kubernetes/template/kernel/Deployments/rag-backend-kernel.yaml`

Point the kernel at the hub via the `HUB_KERNEL_URL` env if your spoke
kernel needs to call back into the hub. The default assumes the spoke
runs a fully self-contained kernel; adjust only if you diverge from the
reference topology.

## Step 5 — Register the spoke with the hub

The hub-side registry lives in an OpenSearch index (`spoke_clusters`).
Register the spoke by calling `spoke_register` on the hub MCP with:

- `cluster_name`: a unique identifier for this spoke
- `client_id`: OIDC client-id assigned to the spoke (matches the
  Keycloak client used by `mcp-server` in `mcp/Deployments/`)
- `service_url`: the URL the hub uses to reach the spoke MCP
  (typically the Cloudflare Tunnel hostname pointing at
  `mcp-company1.mcp.svc.cluster.local:8000`)

Without registration the hub's `cluster_*` tools cannot fan out to this
spoke.

## Step 6 — Apply the manifests

Apply in this order:

```sh
# 1. Infrastructure in rag-spoke (Qdrant, OpenSearch, Memgraph, Valkey,
#    Cloudflared)
kubectl apply -R -f spoke/kubernetes/template/rag-spoke/PVC/
kubectl apply -R -f spoke/kubernetes/template/rag-spoke/Services/
kubectl apply -R -f spoke/kubernetes/template/rag-spoke/Deployments/

# 2. Kernel
kubectl apply -R -f spoke/kubernetes/template/kernel/

# 3. MCP server
kubectl apply -R -f spoke/kubernetes/template/mcp/

# 4. Node-shell (optional)
kubectl apply -R -f spoke/kubernetes/template/node-shell/
```

## Step 7 — Verify

```sh
kubectl get pods -n rag-spoke -w
kubectl get pods -n mcp -w
kubectl -n rag-spoke logs deploy/rag-backend-kernel -f
kubectl -n mcp logs deploy/mcp-company1 -f
```

Then from the hub side, confirm the spoke appears in the registry and
that `cluster_local_mcp_call` reaches this spoke's MCP.

## Examples

See `spoke/kubernetes/examples/README.md` for scenario-oriented
walkthroughs (single-node dev cluster, multi-node with GPUs, etc.).
