# spoke/kubernetes/template/

Fan-out templates. A single copy is stored here; Fleet targets each spoke
cluster with per-cluster values.

Components:

- `kernel/` — spoke RAG kernel Deployment + ConfigMaps + Services + RBAC
- `mcp/` — spoke MCP server that exposes tools to the hub
- `node-shell/` — optional DaemonSet exposing a host-level shell
  endpoint for hub-driven node operations
- `rag-spoke/` — supporting infrastructure on the spoke
  (Qdrant, OpenSearch, Memgraph, Valkey; `cloudflared` retained for v1
  backward compatibility)
- `tunnel-client/` (v2.0) — spoke-side reverse tunnel to the hub; the
  recommended replacement for the v1 `cloudflared` per-spoke tunnel

See [`docs/installation/spoke-kubernetes.md`](../../../docs/installation/spoke-kubernetes.md)
for install steps.
