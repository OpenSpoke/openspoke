# Changelog

All notable changes to OpenSpoke are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v2.0] — 2026-08

### Added

- **Reverse tunnel (spoke -> hub)** — new `hub/manifests/tunnel-server/`
  and `spoke/kubernetes/template/tunnel-client/` modules plus their Go
  source in `hub/images/tunnel-server/` and `spoke/images/tunnel-client/`.
  Onboarding a spoke is now one Secret (`tunnel-client-secret`) plus one
  Fleet target update; the spoke no longer needs a public hostname.
  Backward compatible with the v1 per-spoke `cloudflared` tunnel — the
  two coexist safely. See
  [`docs/concepts/reverse-tunnel.md`](docs/concepts/reverse-tunnel.md)
  and [`docs/installation/reverse-tunnel.md`](docs/installation/reverse-tunnel.md).

- **MCP Streamable HTTP endpoint** — the hub MCP server now serves
  both `/sse` (legacy SSE transport) and `/mcp` (Streamable HTTP,
  MCP protocol revision 2025-03-26) on the same port. Streamable HTTP
  session state persists across pod restarts via Valkey. See
  [`docs/concepts/mcp-endpoints.md`](docs/concepts/mcp-endpoints.md).

- **User ACL & admin role** — hub `is_admin()` checks now consult the
  `openspoke_admin` Keycloak realm role rather than an embedded email
  list. Per-user scopes live in a durable OpenSearch `user_acl` index,
  and both the frontend and MCP server share the same policy source. See
  [`docs/operations/user-acl.md`](docs/operations/user-acl.md).

- **Spawn framework** — asynchronous background LLM tasks, executed by
  a new `task-worker` Deployment (`hub/manifests/task-worker/`). The
  kernel gains 10 `/core/spawn/*` and 8 `/core/project/*` endpoints
  backed by three OpenSearch indexes (`spawns`, `projects`,
  `spawn_counter`). Interactive Claude sessions hand off work via MCP
  tools (`spawn`, `list_spawns`, `get_spawn_result`, `cancel_spawn`,
  `answer_spawn`, `list_spawn_questions`, `force_spawn_state`,
  `create_project`, `list_projects`, `get_project`, `update_project`,
  `archive_project`, `delete_project`, `add_project_member`,
  `remove_project_member`). Supports orchestrator-mode triage
  (gemma2 → Claude escalation) and per-spawn usage tracking. See
  [`docs/concepts/spawn.md`](docs/concepts/spawn.md).

- **New docs sections** — `docs/concepts/reverse-tunnel.md`,
  `docs/concepts/mcp-endpoints.md`, `docs/concepts/spawn.md`,
  `docs/installation/reverse-tunnel.md`,
  `docs/operations/user-acl.md`.

- **`spoke/images/`** — new top-level directory for spoke-side
  container image build contexts, matching the existing `hub/images/`
  convention. Currently ships `tunnel-client/`.

### Deprecated

- **Per-spoke `cloudflared` tunnel** — retained in
  `spoke/kubernetes/template/rag-spoke/Deployments/cloudflared.yaml`
  with a deprecation banner. New deployments should use
  `tunnel-client` instead. Removal is not scheduled for v2.x.

### Notes

- All new Go source ships with a `build.sh` that reads `REGISTRY`
  from the environment; the default `ghcr.io/openspoke/*:v0.1.0`
  image tags in the manifests are placeholders — override to point
  at your own registry before applying.
- The tunnel Phase 1 authenticates spokes with a pre-shared token.
  Phase 3 (Ed25519 signature over the handshake nonce) is on the
  roadmap but not part of v2.0.
- The `orchestrator` spawn mode requires a gemma2 endpoint at
  `/core/orchestrator/triage` and `/core/orchestrator/answer`. These
  routes are not shipped in v2.0; supply your own or let the
  task-worker fall back to Claude direct on any triage failure (the
  fallback is automatic).

## [v1.0] — 2026-07-02

Initial public release. Hub + Kubernetes spoke + Standalone Fleet
skeleton, docs skeleton. Documentation still in progress.
