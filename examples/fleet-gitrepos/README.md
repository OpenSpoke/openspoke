# Fleet GitRepo Templates

Example Fleet GitRepo manifests for deploying OpenSpoke components via
Standalone Fleet GitOps.

Copy these to a location watched by your Fleet controller (typically
`fleet/manifests/managed/GitRepos/` on the Fleet server cluster) and
customize:

- `spec.repo`: URL of your fork or the upstream `OpenSpoke/openspoke` repo
- `spec.branch`: usually `main`
- `spec.paths`: subdirectory of the repo to watch
  (see `hub/manifests/` and `spoke/kubernetes/template/` in this repo)
- `spec.targets[].clusterSelector.matchLabels`: labels of clusters where
  the manifests should be applied

## Templates

- `hub-kernel.yaml` — deploys the hub RAG kernel (Ollama variant) to a
  single hub cluster (matches clusters labeled `role: hub`)
- `hub-kernel-vllm.yaml` — deploys the hub RAG kernel (vLLM + Swallow
  variant, NVIDIA GPU) to a single hub cluster (matches clusters
  labeled `role: hub, llm: vllm`)
- `hub-tunnel-server.yaml` (v2.0) — deploys the hub-side reverse
  tunnel server (matches clusters labeled `role: hub`)
- `hub-task-worker.yaml` (v2.0) — deploys the spawn framework worker
  (matches clusters labeled `role: hub`)
- `spoke-mcp.yaml` — fans out the spoke MCP server to all downstream
  clusters (matches clusters labeled `role: spoke`)
- `spoke-tunnel-client.yaml` (v2.0) — fans out the spoke-side reverse
  tunnel client to all downstream clusters (matches clusters labeled
  `role: spoke`)

## Prerequisites

- Standalone Fleet controller running on a Fleet server cluster
  (see `fleet/manifests/bootstrap/`)
- Downstream clusters registered with Fleet and labeled appropriately
  (`role: hub` / `role: spoke` or your preferred convention)
- For private forks, a Secret referenced via `clientSecretName` containing
  GitHub credentials
