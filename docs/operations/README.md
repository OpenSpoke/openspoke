# docs/operations/

Day-two operations.

Available:

- [user-acl.md](user-acl.md) (v2.0) — admin role via the
  `openspoke_admin` Keycloak realm role, per-user ACL in OpenSearch,
  emergency fallback, and the v1 -> v2 migration checklist.
- [knowledge-base-migration.md](knowledge-base-migration.md) (v3.0) —
  migrate an existing vector-store `_handoff_*` knowledge base to
  Memgraph without downtime; verify, rollback, and post-migration
  tag backfill.
- [fleet-polling-strategy.md](fleet-polling-strategy.md) (v3.0) —
  1m / 5m / 15m polling tiers for Fleet `GitRepo` resources, the
  gitcloner-hang failure mode they avoid, and when to promote or
  demote a GitRepo.
- [milvus-stabilization.md](milvus-stabilization.md) (v3.0) — the
  three changes on top of a stock Milvus 2.6 that keep the hub
  Milvus stack alive across node reboots: consolidated coordinator,
  Pulsar three-point fix, and etcd HA.
- [git-credentials.md](git-credentials.md) (v3.0) — the hub credentials
  store: one OpenSearch index holding org-scoped GitHub, GitLab,
  Anthropic and TypeSafe PATs, endpoints, kernel resolution order,
  and the admin-only Git 認証情報 view.
- [kubectl-centralization.md](kubectl-centralization.md) (v3.0) —
  per-spoke admin kubeconfigs stored in OpenSearch, `kubectl` executed
  from the hub kernel over the reverse tunnel with a lazily downloaded
  binary and a `kubectl_audit` trail.
- [security-scanning.md](security-scanning.md) (v3.0) — the
  vulnerability-scanning framework: OSS dependency and mobile-app
  checks, the `vuln_events` / `vuln_findings_mobile` indexes, the
  `openspoke-vuln-scanner` image, and the Security dashboard view.

Planned: adding-a-spoke.md, ledger.md, mcp-tools.md, troubleshooting.md.
