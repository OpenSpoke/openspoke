# docs/installation/

Per-component install guides.

Planned: spoke-native.md.

Available:

- [hub.md](hub.md) — full Hub install (namespaces, images, Secrets, env
  customization, Ingress + oauth2-proxy templates, apply order).
- [spoke-kubernetes.md](spoke-kubernetes.md) — Kubernetes-template spoke
  install (namespaces, images, Secrets, env customization,
  register-with-hub, apply order).
- [fleet.md](fleet.md) — Standalone Fleet bootstrap (Secrets,
  apiServerURL, HelmChart, cloudflared, GitRepo templates, downstream
  registration, Fleet UI).
- [rancher.md](rancher.md) — optional Rancher install (Helm, cluster
  registration, embedded Fleet, example manifests).
- [reverse-tunnel.md](reverse-tunnel.md) (v2.0) — the spoke -> hub reverse
  tunnel: build images, mint per-spoke tokens, apply hub- and spoke-side
  manifests, verify the tunnel, migrate off the v1 per-spoke tunnel.

Cross-cutting:

- [node-labels.md](node-labels.md) — required node labels for stateful
  Hub components (Milvus, OpenSearch, Keycloak PostgreSQL, GPU workloads).
