# Hub Installation Guide

This guide walks through installing the OpenSpoke Hub — the central
cluster that runs the RAG kernel, MCP server, Milvus, OpenSearch,
Memgraph, guardrails, Ollama, Valkey, Keycloak, and supporting jobs.

## Prerequisites

- A Kubernetes cluster (v1.28+) with `kubectl` context configured
- An Ingress controller (guide assumes `nginx`)
- A local-path storage class (or replace it in the PVC manifests — see
  `node-labels.md` for the alternative)
- A container registry your cluster can pull from
- Node labels applied per [node-labels.md](node-labels.md)
- Optional: NVIDIA GPU device plugin
  (see `hub/manifests.static/gpu/README.md`)

The Hub uses the namespace `rag-company1` throughout — that name is the
verified topology and is referenced from every Deployment / Service /
ConfigMap. Do not rename it unless you rename it everywhere.

## Choosing an LLM variant

The Hub ships two mutually-exclusive kernel + LLM-server pairings. Pick
exactly one before you apply the kernel and LLM manifests.

| Variant                                             | Kernel path                        | LLM server path                       | LLM stack       | Target GPU |
| --------------------------------------------------- | ---------------------------------- | ------------------------------------- | --------------- | ---------- |
| **Ollama** (default reference: `rag` cluster)       | `hub/manifests/kernel/`            | `hub/manifests/rag-hub/rag/`          | Ollama + Gemma  | Intel iGPU |
| **vLLM** (default reference: GPU-equipped clusters) | `hub/manifests/kernel-vllm/`       | `hub/manifests/rag-hub/rag-vllm/`     | vLLM + Swallow  | NVIDIA GPU |

The choice is a topology decision — the kernel manifests bake in the
LLM endpoint (Ollama vs vLLM), so the kernel and LLM-server variants
must be paired. Do not mix (`hub/manifests/kernel/` with `rag-vllm/`
or `hub/manifests/kernel-vllm/` with `rag/` will not work out of the
box).

The rest of this guide uses `hub/manifests/kernel/` and
`hub/manifests/rag-hub/rag/` in the example commands. If you pick the
vLLM variant, substitute `hub/manifests/kernel-vllm/` and
`hub/manifests/rag-hub/rag-vllm/` in every command below.

## Step 1 — Create namespaces

Apply the static namespace manifests first:

```sh
kubectl apply -f hub/manifests.static/kernel/Namespaces/
kubectl apply -f hub/manifests.static/mcp/Namespaces/
kubectl apply -f hub/manifests.static/cloudflared/Namespaces/
kubectl apply -f hub/manifests.static/gardrails/Namespaces/
kubectl apply -f hub/manifests.static/memgraph/Namespaces/
kubectl apply -f hub/manifests.static/milvus/Namespaces/
kubectl apply -f hub/manifests.static/opensearch/Namespaces/
kubectl apply -f hub/manifests.static/pdf-converter/Namespaces/
kubectl apply -f hub/manifests.static/valkey/Namespaces/
```

Keycloak also needs a namespace:

```sh
kubectl create namespace keycloak
```

## Step 2 — Build and push images

The manifests reference images by short name (no registry). Build each
one from the corresponding build context under
[`hub/images/`](../../hub/images/), push it to your registry, and update
the `image:` field in each Deployment to reference your pushed tag
before applying.

Each `hub/images/*/build.sh` reads a `REGISTRY` environment variable
(default `ghcr.io/openspoke`). Set it to your own registry first:

```sh
export REGISTRY=ghcr.io/YOUR-ORG
cd hub/images/backend-frontend && ./build.sh
```

| Short name (as referenced by manifests)  | Built by                                   |
| ---------------------------------------- | ------------------------------------------ |
| `rag-backend-gpu-offline:v1.0.3`         | `hub/images/backend-frontend/build.sh`     |
| `rag-frontend-offline:v1.0.3`            | `hub/images/backend-frontend/build.sh`     |
| `rag-mcp-server:latest`                  | `hub/images/mcp/build.sh`                  |
| `pdf-converter:0.3.0`                    | `hub/images/pdf-converter/build.sh`        |
| `fleet-ui:0.2.0`                         | `fleet/ui/` (see `fleet/ui/README.md`)     |

All other images (Ollama, vLLM, Milvus, OpenSearch, Memgraph, Valkey,
Qdrant, Keycloak, PostgreSQL, oauth2-proxy, cloudflared, etc.) are
pulled directly from their upstream public registries — no build or
retag needed unless you run in an air-gapped environment.

## Step 3 — Create Secrets

The repository ships without any real credentials. Create every Secret
listed below before applying the Deployments that reference them.

### Kernel namespace (`rag-company1`)

```sh
# Anthropic API key (Deployment: rag-backend-kernel)
kubectl create secret generic anthropic-api-key \
  -n rag-company1 \
  --from-literal=api-key='sk-ant-api03-...'

# GitHub PAT (used by MCP GitHub tools)
kubectl create secret generic github-api-token \
  -n rag-company1 \
  --from-literal=token='ghp_...'

# Google OAuth (used by kernel google integration)
kubectl create secret generic google-oauth-credentials \
  -n rag-company1 \
  --from-literal=client-id='<id>' \
  --from-literal=client-secret='<secret>'

# Kintone (optional, only if you use the kintone MCP)
kubectl create secret generic kintone-credentials \
  -n rag-company1 \
  --from-literal=api-token='<token>' \
  --from-literal=subdomain='<subdomain>'

# MCP shared API key (kernel <-> mcp trust boundary)
kubectl create secret generic mcp-api-key \
  -n rag-company1 \
  --from-literal=api-key="$(openssl rand -hex 32)"

# Slack bot token (used by Slack transport)
kubectl create secret generic slack-bot-token \
  -n rag-company1 \
  --from-literal=token='xoxb-...'

# Slack signing secret + webhook URL
kubectl create secret generic slack-signature-webhook \
  -n rag-company1 \
  --from-literal=signing-secret='<signing-secret>' \
  --from-literal=webhook-url='https://hooks.slack.com/...'
```

### MCP namespace

```sh
# Keycloak client secret for the MCP OIDC integration
kubectl create secret generic keycloak-client-secret \
  -n mcp \
  --from-literal=client-secret='<keycloak-client-secret>'

# MCP API key (same value as the one in rag-company1 above)
kubectl create secret generic mcp-api-key \
  -n mcp \
  --from-literal=api-key='<same-value-as-rag-company1>'
```

### Keycloak namespace

```sh
# Keycloak admin credentials + JDBC credentials (used by keycloak Deployment)
kubectl create secret generic keycloak-secret \
  -n keycloak \
  --from-literal=KEYCLOAK_ADMIN='<admin-user>' \
  --from-literal=KEYCLOAK_ADMIN_PASSWORD='<admin-password>' \
  --from-literal=KC_DB_USERNAME='<db-user>' \
  --from-literal=KC_DB_PASSWORD='<db-password>'

# PostgreSQL credentials (used by postgresql Deployment)
kubectl create secret generic postgresql-secret \
  -n keycloak \
  --from-literal=POSTGRES_USER='<db-user>' \
  --from-literal=POSTGRES_PASSWORD='<db-password>' \
  --from-literal=POSTGRES_DB='keycloak'
# NOTE: POSTGRES_USER / POSTGRES_PASSWORD must match
# KC_DB_USERNAME / KC_DB_PASSWORD above.

# Keycloak client secret consumed by the MCP OIDC integration
kubectl create secret generic keycloak-client-secret \
  -n keycloak \
  --from-literal=client-secret='<keycloak-client-secret>'
```

### rag-frontend Keycloak admin API secret (rag-company1 namespace)

```sh
# Client secret for the Keycloak service-account client used by
# rag-frontend to call the Keycloak admin API (user list / role updates).
kubectl create secret generic keycloak-admin-api-secret \
  -n rag-company1 \
  --from-literal=client-secret='<keycloak-admin-client-secret>'
```

### Cloudflared namespace (optional — only if you use Cloudflare Tunnel)

```sh
# Save your tunnel credentials JSON as credentials.json first
kubectl create secret generic cloudflared-bundle \
  -n cloudflared \
  --from-file=credentials.json=./credentials.json
```

## Step 4 — Customize environment variables

Several manifests contain placeholder values that must be replaced with
your environment's real values before applying.

### `hub/manifests/mcp/Deployments/mcp-company.yaml`

| env var                   | Placeholder                 | Set to                       |
| ------------------------- | --------------------------- | ---------------------------- |
| `KEYCLOAK_URL`            | `https://keycloak.example.com` | your Keycloak public URL |
| `KEYCLOAK_INTERNAL_URL`   | `https://keycloak.example.com` | your Keycloak internal URL (may equal the public one) |
| `BOOTSTRAP_ADMINS`        | `admin`                     | comma-separated Keycloak `preferred_username` values that should receive the `openspoke_admin` role at first boot |

### `hub/manifests/kernel-frontend/Deployments/rag-frontend.yaml`

| env var          | Placeholder                    | Set to                    |
| ---------------- | ------------------------------ | ------------------------- |
| `KEYCLOAK_URL`   | `https://keycloak.example.com` | your Keycloak public URL  |

### `hub/manifests/rag-hub/opensearch/CronJobs/opensearch-snapshot.yaml`

| env var    | Placeholder                    | Set to                                          |
| ---------- | ------------------------------ | ----------------------------------------------- |
| `INDICES`  | `rag_chunks,claude_usage`      | comma-separated list of indices to snapshot     |

### `fleet/manifests/bootstrap/HelmCharts/fleet-controller.yaml`

| Field           | Placeholder                          | Set to                                       |
| --------------- | ------------------------------------ | -------------------------------------------- |
| `apiServerURL`  | `https://fleet-api.example.com`      | public URL of your fleet-cluster kube-apiserver |

## Step 5 — Create Ingress resources

The repository ships without hostname-bound Ingresses. Adapt the
template below (nginx + basic host routing) for each service you want to
expose, then apply.

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: rag-frontend
  namespace: rag-company1
spec:
  ingressClassName: nginx
  rules:
    - host: llm.YOUR-DOMAIN.example
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: oauth2-proxy
                port:
                  number: 4180
```

Repeat for:

- `rag-backend-kernel` (in `rag-company1`) — internal only, usually no
  Ingress needed
- `mcp-company1` (in `mcp`) — expose if you want remote MCP clients
- `keycloak` (in `keycloak`) — expose for OAuth callback traffic

## Step 6 — Create the auth Deployments

The repository ships without `oauth2-proxy` (per-install auth wiring is
site-specific). If your topology mirrors the reference deployment, use
this Deployment as a starting point:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: oauth2-proxy
  namespace: rag-company1
spec:
  replicas: 1
  selector:
    matchLabels:
      app: oauth2-proxy
  template:
    metadata:
      labels:
        app: oauth2-proxy
    spec:
      containers:
        - name: oauth2-proxy
          image: quay.io/oauth2-proxy/oauth2-proxy:v7.6.0
          args:
            - --provider=keycloak-oidc
            - --oidc-issuer-url=https://keycloak.YOUR-DOMAIN.example/realms/rag
            - --client-id=rag-frontend1
            - --whitelist-domain=keycloak.YOUR-DOMAIN.example
            - --upstream=http://rag-frontend.rag-company1.svc.cluster.local:8501
            - --http-address=0.0.0.0:4180
            - --redirect-url=https://llm.YOUR-DOMAIN.example/oauth2/callback
            - --email-domain=*
            - --cookie-samesite=none
            - --cookie-name=_oauth2_proxy_rag_company1
            - --cookie-domain=llm.YOUR-DOMAIN.example
            - --session-store-type=cookie
          env:
            - name: OAUTH2_PROXY_COOKIE_SECRET
              valueFrom:
                secretKeyRef:
                  name: oauth2-proxy-secret
                  key: cookie-secret
            - name: OAUTH2_PROXY_CLIENT_SECRET
              valueFrom:
                secretKeyRef:
                  name: oauth2-proxy-secret
                  key: client-secret
          ports:
            - containerPort: 4180
---
apiVersion: v1
kind: Service
metadata:
  name: oauth2-proxy
  namespace: rag-company1
spec:
  selector:
    app: oauth2-proxy
  ports:
    - port: 4180
      targetPort: 4180
```

Also create the `oauth2-proxy-secret` Secret:

```sh
kubectl create secret generic oauth2-proxy-secret \
  -n rag-company1 \
  --from-literal=cookie-secret="$(openssl rand -base64 32 | head -c 32)" \
  --from-literal=client-secret='<keycloak-client-secret-for-rag-frontend1>'
```

## Step 7 — Apply the static manifests

Apply the remaining static manifests in this order:

```sh
kubectl apply -R -f hub/manifests.static/gpu/                # optional
kubectl apply -R -f hub/manifests.static/default/            # optional
kubectl apply -R -f hub/manifests.static/kernel/PersistentVolumeClaims/
kubectl apply -R -f hub/manifests.static/memgraph/PersistentVolumeClaims/
kubectl apply -R -f hub/manifests.static/milvus/PersistentVolumeClaims/
kubectl apply -R -f hub/manifests.static/opensearch/PersistentVolumeClaims/
kubectl apply -R -f hub/manifests.static/rag/PersistentVolumeClaims/
kubectl apply -R -f hub/manifests.static/keycloak/
kubectl apply -R -f hub/manifests.static/rag/
```

## Step 8 — Apply the Fleet-managed manifests

If you use Fleet, register a GitRepo pointing at `hub/manifests/` — see
`examples/fleet-gitrepos/hub-kernel.yaml` for a template.

If you apply directly without Fleet:

```sh
kubectl apply -R -f hub/manifests/rag-hub/memgraph/
kubectl apply -R -f hub/manifests/rag-hub/milvus/
kubectl apply -R -f hub/manifests/rag-hub/opensearch/
kubectl apply -R -f hub/manifests/rag-hub/valkey/
kubectl apply -R -f hub/manifests/rag-hub/rag/                # Ollama
kubectl apply -R -f hub/manifests/rag-hub/gardrails/          # optional
kubectl apply -R -f hub/manifests/rag-hub/cloudflared/        # optional
kubectl apply -R -f hub/manifests/kernel/
kubectl apply -R -f hub/manifests/mcp/
kubectl apply -R -f hub/manifests/kernel-frontend/
kubectl apply -R -f hub/manifests/pdf-converter/
kubectl apply -R -f hub/manifests/nodes-aggregator/
kubectl apply -R -f hub/manifests/spoke-probe/
kubectl apply -R -f hub/manifests/fleet-state-publisher/
```

## Step 9 — Verify

```sh
kubectl get pods -A | grep -E 'rag-company1|mcp|milvus|memgraph|opensearch|keycloak|rag-hub'
kubectl -n rag-company1 logs deploy/rag-backend-kernel -f
kubectl -n mcp logs deploy/mcp-company1 -f
```

At this point the RAG kernel should accept requests, MCP should register
its tools, and the frontend should complete the OAuth flow through
Keycloak.
