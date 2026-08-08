# Node Labeling for Stateful Workloads

Several Hub components use `storageClassName: local-path` PVCs, which
bind a PV to a specific node's local filesystem. Their Pods therefore
carry a `nodeAffinity` that pins them to nodes with a specific label —
without that pin, a rescheduled Pod on another node would lose access
to its PV.

Before applying the Hub manifests, label the nodes that will host these
workloads.

## Required labels

| Label                | Applies to                                          |
| -------------------- | --------------------------------------------------- |
| `rag-type=milvus`    | Milvus stack (etcd, minio, pulsar, milvus-*),       |
|                      | RAG kernel (`rag-backend-kernel`), MCP server       |
| `nodetype=database`  | OpenSearch, Keycloak PostgreSQL                     |

The two labels can coexist on the same node — for example, a single
"storage" node can carry both `rag-type=milvus` and `nodetype=database`
if you want to co-locate Milvus and OpenSearch.

## Applying the labels

Label the node(s) that own the local-path PVs before applying the
manifests:

```sh
# Storage node for the Milvus stack + kernel + MCP
kubectl label node <node-name> rag-type=milvus

# Storage node for OpenSearch and the Keycloak PostgreSQL
kubectl label node <node-name> nodetype=database
```

Verify:

```sh
kubectl get nodes -L rag-type,nodetype
```

## GPU nodes

Workloads that need GPUs (Ollama, guardrails) additionally require:

```sh
kubectl label node <gpu-node-name> gpu=enabled
```

See `hub/manifests.static/gpu/README.md` for the NVIDIA device plugin
setup that backs the GPU label.

## Alternatives

If you use a different storage class (e.g., a networked CSI provider
like Longhorn, Ceph RBD, or a cloud-managed disk that can attach
anywhere in the zone), you can remove the `nodeAffinity` blocks from
the Deployments — the PV will follow the Pod. In that case the labels
above are not needed. Keep the labels if you stay on `local-path` or
any other node-local storage.
