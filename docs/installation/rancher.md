# Rancher Installation Guide (Optional)

Rancher is an **optional** control plane for managing OpenSpoke
clusters. OpenSpoke does not require Rancher — you can run Fleet
standalone (see `fleet.md`). Use Rancher if you want a UI for cluster
provisioning, RBAC, and lifecycle management on top of Fleet.

Manifests live under `rancher/manifests/`.

## Prerequisites

- A Kubernetes cluster to host Rancher (v1.28+; a dedicated 3-node
  cluster is recommended for HA)
- `kubectl` context pointed at the Rancher host cluster
- A public hostname pointed at the Rancher Ingress
  (Rancher requires TLS)
- `cert-manager` installed in the Rancher host cluster (Rancher's
  helm chart depends on it for the built-in Let's Encrypt / self-signed
  path — or supply your own certificate)

## Step 1 — Install Rancher via Helm

Follow the upstream Rancher install guide:
<https://ranchermanager.docs.rancher.com/>

Recommended chart version: latest stable at time of install.

```sh
helm repo add rancher-latest https://releases.rancher.com/server-charts/latest
helm repo update

kubectl create namespace cattle-system

helm install rancher rancher-latest/rancher \
  --namespace cattle-system \
  --set hostname=rancher.YOUR-DOMAIN.example \
  --set bootstrapPassword=<initial-admin-password> \
  --set ingress.tls.source=letsEncrypt \
  --set letsEncrypt.email=<your-email>
```

Wait for the `rancher` Deployment in the `cattle-system` namespace to
become Ready, then open `https://rancher.YOUR-DOMAIN.example` and
finish first-run setup.

## Step 2 — Register OpenSpoke clusters

Once Rancher is up:

1. In the Rancher UI, go to **Cluster Management** → **Import Existing**
2. Give the cluster a name that matches your `role=hub` /
   `role=spoke` labeling convention
3. Copy the generated `kubectl apply -f` command and run it against the
   OpenSpoke cluster you want to register
4. Rancher will label the cluster with `provider.cattle.io=k3s`
   (or `rke`, `import`, etc.) — add your own labels
   (`role=hub`, `role=spoke`) via the Rancher UI or:

```sh
kubectl label cluster.management.cattle.io/<cluster-id> role=spoke \
  -n fleet-default
```

Repeat for every hub and spoke cluster you want under Rancher's
management.

## Step 3 — (Optional) Use Rancher-provisioned Fleet

Rancher ships an embedded Fleet controller. If you install Rancher, you
can use Rancher's Fleet in place of Standalone Fleet
(see `fleet.md`). The choice is mutually exclusive per management
cluster:

- **Standalone Fleet** — no Rancher required, minimal footprint
- **Rancher-embedded Fleet** — Fleet UI comes bundled inside Rancher

Both consume the same `GitRepo` CR format, so the templates in
`examples/fleet-gitrepos/` work with either.

## Step 4 — Apply the example Rancher manifests

The `rancher/manifests/example/` directory holds example resources
(RBAC bindings, project namespaces, etc.) that you may want in a
Rancher-managed OpenSpoke deployment. Review and apply what fits your
setup:

```sh
kubectl apply -R -f rancher/manifests/example/
```

## Step 5 — Verify

```sh
kubectl -n cattle-system get pods
kubectl -n cattle-fleet-system get pods   # embedded Fleet
```

Then in the Rancher UI:

- **Cluster Management** → confirm every imported cluster is `Active`
- **Continuous Delivery** (Fleet) → confirm GitRepos and Bundles are
  reconciling
- **Apps** → optionally deploy additional charts onto managed clusters

## Related guides

- [hub.md](hub.md) — install the OpenSpoke Hub
- [spoke-kubernetes.md](spoke-kubernetes.md) — install a Kubernetes
  spoke
- [fleet.md](fleet.md) — Standalone Fleet install
  (alternative to Rancher-embedded Fleet)
- [node-labels.md](node-labels.md) — required node labels for stateful
  Hub components
