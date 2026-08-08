# hub/manifests/

Kubernetes manifests for hub components. Each subdirectory is one component,
delivered as a Fleet bundle via a dedicated GitRepo.

**Admin**: place each subdirectory from `OpenSpoke/Core/Hub/manifests/` here.

Components (in-scope list to be confirmed after per-component sanitization review):
builder, fleet-state-publisher, kernel, kernel-admin, kernel-frontend, mcp,
nodes-aggregator, opensearch, pdf-converter, rag-hub, spoke-probe,
tunnel-server (v2.0), task-worker (v2.0).

`mcp-kintone/` is application-specific and out of scope; do not copy.
