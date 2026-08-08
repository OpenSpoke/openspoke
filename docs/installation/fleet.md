# Fleet Installation Guide

This guide walks through installing **Standalone Fleet** — the GitOps
controller that drives OpenSpoke deployments across the Hub and any
number of downstream (spoke) clusters.

Manifests live under:

- `fleet/manifests/bootstrap/` — the initial Fleet controller and its
  supporting resources (HelmChart, Namespaces, Deployments, Secrets,
  GitRepos)
- `fleet/manifests/managed/` — resources that Fleet manages on itself
  once bootstrap is complete (Fleet UI, ClusterRoles, Services, etc.)
- `fleet/ui/` — source for the optional Fleet UI web app

## Prerequisites

- A Kubernetes cluster that will act as the **fleet server** — this can
  be the same cluster as the Hub or a dedicated one
- `kubectl` context pointed at the fleet server cluster
- A container registry your cluster can pull from (for the Fleet UI
  image built from `fleet/ui/`)
- A public URL that downstream fleet-agents can reach (typically a
  Cloudflare Tunnel; the reference topology uses one)

## Step 1 — Bootstrap namespaces

```sh
kubectl apply -f fleet/manifests/bootstrap/Namespaces/
```

This creates `fleet-default` (where GitRepos live) and `fleet-ui`.

## Step 2 — Create bootstrap Secrets

The repository ships without real credentials. Create these before
applying the bootstrap Deployments:

```sh
# Cloudflare Tunnel credentials for the fleet-api ingress (the URL
# downstream fleet-agents connect back to). Save your tunnel JSON as
# credentials.json first.
kubectl create secret generic cloudflared-fleet-api-bundle \
  -n fleet-default \
  --from-file=credentials.json=./credentials.json

# If you also use a Cloudflare Tunnel to expose the Fleet UI
# (fleet/ui/), create its credentials Secret too.
kubectl create secret generic cloudflared-bundle \
  -n fleet-ui \
  --from-file=credentials.json=./credentials.json

# GitHub credentials used by Fleet GitRepos to clone your fork(s).
# username + password (personal access token) fields, basic-auth type.
kubectl create secret generic fleet-git-credentials \
  -n fleet-default \
  --type=kubernetes.io/basic-auth \
  --from-literal=username='<github-username>' \
  --from-literal=password='<github-personal-access-token>'

# Optional: OAuth2 proxy cookie/client secret if you gate the Fleet UI
# behind Keycloak.
kubectl create secret generic oauth2-proxy-secret \
  -n fleet-ui \
  --from-literal=cookie-secret="$(openssl rand -base64 32 | head -c 32)" \
  --from-literal=client-secret='<keycloak-client-secret>'
```

## Step 3 — Customize the Fleet controller URL

Edit `fleet/manifests/bootstrap/HelmCharts/fleet-controller.yaml` and
replace the placeholder `apiServerURL` with the public URL that
downstream fleet-agents will use to reach this fleet server's
kube-apiserver:

```yaml
apiServerURL: https://fleet-api.YOUR-DOMAIN.example
apiServerCA: ""   # empty is correct when the URL is fronted by a
                  # public-CA certificate (e.g. Cloudflare Tunnel)
```

## Step 4 — Install the Fleet controller

Fleet is deployed via Helm; the HelmChart CR in
`fleet/manifests/bootstrap/HelmCharts/` triggers the install.

```sh
kubectl apply -f fleet/manifests/bootstrap/HelmCharts/
```

Wait for the `fleet-controller` Deployment in the `cattle-fleet-system`
namespace (installed by the HelmChart) to become Ready.

## Step 5 — Expose the fleet server kube-apiserver

Apply the cloudflared Deployment that fronts the fleet-server
kube-apiserver:

```sh
kubectl apply -f fleet/manifests/bootstrap/Deployments/cloudflared-fleet-api.yaml
```

If you are not using Cloudflare Tunnel, replace this with your own
LB / Ingress / NodePort configuration that publishes the
kube-apiserver on the `apiServerURL` you set in Step 3.

## Step 6 — Register downstream clusters

Once the fleet controller is running, register each downstream cluster
with a `ClusterRegistrationToken` and label them so the GitRepo
selectors from `examples/fleet-gitrepos/` can find them:

```sh
kubectl label cluster.fleet.cattle.io/<cluster-id> role=hub    -n fleet-default
kubectl label cluster.fleet.cattle.io/<cluster-id> role=spoke  -n fleet-default
```

## Step 7 — Add GitRepos

Copy the templates from `examples/fleet-gitrepos/` into
`fleet/manifests/managed/GitRepos/` (or apply directly), adjusting:

- `spec.repo` to your fork of `github.com/OpenSpoke/openspoke.git`
- `spec.paths` to the subdirectory you want distributed
- `spec.targets[].clusterSelector.matchLabels` to your cluster labels

Then apply:

```sh
kubectl apply -R -f fleet/manifests/managed/GitRepos/
```

Fleet will start reconciling the manifests onto every matching
downstream cluster.

## Step 8 — (Optional) Deploy the Fleet UI

The Fleet UI is built from `fleet/ui/`. Build the image, push it to
your registry, update the `image:` reference in
`fleet/manifests/managed/Deployments/fleet-ui.yaml` to point at your
pushed tag, then apply the managed manifests:

```sh
cd fleet/ui/
./build.sh                            # builds and (optionally) pushes
# edit fleet/manifests/managed/Deployments/fleet-ui.yaml image: tag
kubectl apply -R -f fleet/manifests/managed/
```

The UI is a lightweight Go backend + static HTML dashboard for browsing
Fleet Bundles and GitRepo state. It is optional — everything works
without it via `kubectl` directly.

## Step 9 — Verify

```sh
kubectl -n cattle-fleet-system get pods
kubectl -n fleet-default get gitrepos
kubectl -n fleet-default get clusters
```

Downstream cluster manifests should start appearing as Bundles under
`kubectl get bundles -A` on the fleet server, and as reconciled
resources on each downstream cluster.
