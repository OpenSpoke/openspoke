# fleet-ui

A minimal web UI for operating **Standalone Fleet** without pulling in
the full Rancher UI. Designed for cases where Fleet is deployed to a
dedicated cluster and you want a lightweight dashboard for GitRepos,
Bundles, and Clusters.

## Design

- **Minimal dependencies** — Go single-file backend + single HTML page
  (Vue 3 + Tailwind loaded from CDN)
- **Fleet-version agnostic** — uses `dynamic.Interface` against the
  `fleet.cattle.io` API group, so no compile-time coupling to Fleet
  types
- **In-cluster only** — runs with the `fleet-ui` ServiceAccount to
  reach the kube-apiserver
- **Basic auth** — `BASIC_AUTH_USERNAME` / `BASIC_AUTH_PASSWORD`
  environment variables

## Features

| Tab       | Purpose                                                        |
|-----------|----------------------------------------------------------------|
| Dashboard | GitRepo / Bundle / Cluster totals, Ready %, recent errors      |
| GitRepos  | Per-namespace list, force-sync                                 |
| Bundles   | Per-namespace list, Ready status                               |
| Clusters  | Per-namespace list, agent last-seen, Bundle Ready ratio        |

`namespace` = Fleet workspace. Switch via the header dropdown.

## Source layout

```
fleet/ui/
├── backend/
│   ├── go.mod
│   └── main.go
├── web/
│   └── index.html
├── Dockerfile
└── README.md
```

## Build

```bash
cd fleet/ui
docker build -t <your-registry>/fleet-ui:0.2.0 .
docker push <your-registry>/fleet-ui:0.2.0
```

Then update the `image:` field in
`fleet/manifests/managed/Deployments/fleet-ui.yaml` to point at your
pushed tag before applying that manifest via Fleet.

## Deploy

The Kubernetes manifests live in `fleet/manifests/managed/`. Once
applied by Fleet (see [`docs/installation/fleet.md`](../../docs/installation/fleet.md))
the UI runs in the `fleet-ui` namespace.

Default credentials are `admin` / `changeme`. **Change them in
production** by editing the `fleet-ui-basicauth` Secret:

```bash
kubectl -n fleet-ui edit secret fleet-ui-basicauth
```

## Public exposure

Any Ingress controller works. If you use Cloudflare Tunnel, add a route
in the Cloudflare dashboard pointing at
`http://fleet-ui.fleet-ui.svc.cluster.local:80` and expose it on a
hostname of your choice.

## API

Basic auth required on all endpoints except `/healthz`.

| Method | Path                                     | Description                          |
|--------|------------------------------------------|--------------------------------------|
| GET    | `/api/config`                            | UI configuration                     |
| GET    | `/api/workspaces`                        | Workspace (namespace) list           |
| GET    | `/api/gitrepos`                          | All GitRepos across namespaces       |
| GET    | `/api/gitrepos/{ns}`                     | GitRepos in one namespace            |
| GET    | `/api/gitrepos/{ns}/{name}`              | Single GitRepo                       |
| POST   | `/api/gitrepos/{ns}`                     | Create GitRepo                       |
| PATCH  | `/api/gitrepos/{ns}/{name}`              | Partial update                       |
| DELETE | `/api/gitrepos/{ns}/{name}`              | Delete                               |
| POST   | `/api/gitrepos/{ns}/{name}/force-sync`   | Force resync (annotation patch)      |
| ...    | (same pattern for bundles, clusters, bundledeployments) |                       |
| GET    | `/healthz`                               | Health check (no auth)               |
