# Changelog

All notable changes to OpenSpoke are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v3.0] — 2026-08

### Added

- **Knowledge base backed by a graph database** — the hub knowledge base
  (previously the `secretary_knowledge_*` Milvus collection with the
  reserved `_handoff_*` id prefix) now lives in Memgraph as
  `(:Brief {id})` and `(:Topic {id, title, summary, content, category,
  tags, created_at, updated_at})` nodes, and `(:Tag {name})` nodes with
  `HAS_TAG` relationships. The kernel gains seven
  [`/core/handoff/*`](docs/concepts/knowledge-base.md) endpoints (brief
  get/upsert, topics list, topic get/upsert/delete, one-shot migrate).
  MCP tools (`system_brief`, `system_topics`, `system_lookup`,
  `upsert_knowledge`, `delete_knowledge`, `upsert_knowledge_batch`)
  dispatch by id to the new endpoints; ids without the `_handoff_`
  prefix continue to go to the existing vector store. See
  [`docs/concepts/knowledge-base.md`](docs/concepts/knowledge-base.md),
  [`docs/operations/knowledge-base-migration.md`](docs/operations/knowledge-base-migration.md).

- **Knowledge base view in the hub dashboard** — a new admin-only
  KnowledgeBase view in `rag-frontend` renders the Memgraph knowledge
  base directly: a graph-schema overview, a tag-cooccurrence map, the
  `Brief` node content, and a filterable topic browser with per-tag
  drill-down. The view talks to Memgraph exclusively through the kernel
  (`/core/graph/run`); the frontend never opens a Cypher session on its
  own.

- **Automatic topic tagging** — the kernel's `/core/handoff/topic/upsert`
  endpoint runs a background task that asks a local LLM (default
  `gemma2:9b` served by Ollama at `OLLAMA_URL`) to propose one to three
  tags per topic and stores them as both a `tags` property on the
  `Topic` node and `HAS_TAG` relationships to `Tag` nodes. Failures log
  and skip; the upsert itself always succeeds.

- **Ladybug backend (evaluation)** — a fourth database backend based on
  [LadybugDB](https://github.com/ladybugdb) (a Cypher-compatible embedded
  graph engine with a vector extension). The new
  `hub/manifests/ladybug/` bundle ships a Namespace, a 100Gi PVC, a
  Deployment pinned via nodeSelector, and a ClusterIP Service. The
  kernel gains a `backends.py` abstraction layer (`VectorStore` +
  `GraphStore` + `HandoffStore`) with three adapters (Milvus, Memgraph,
  Ladybug); the active backend is selected by `BACKEND_VECTOR`,
  `BACKEND_GRAPH`, and `BACKEND_HANDOFF` env vars and defaults to
  `milvus` + `memgraph` + `memgraph`. A new
  `/core/ladybug/{health,schema,cypher}` endpoint set exposes the raw
  Ladybug API for evaluation. See
  [`docs/concepts/ladybug.md`](docs/concepts/ladybug.md),
  [`docs/installation/ladybug.md`](docs/installation/ladybug.md).

- **Spawn and project dashboards** — new admin views in `rag-frontend`
  visualise the v2.0 spawn/project framework: a Projects tab with
  members and status, and a Spawns tab that lists queued/running/
  waiting/failed/done spawns with per-spawn Claude token+cost and gemma2
  usage columns.

- **Cluster history view** — the Clusters tab now shows per-spoke
  restart events, K8s Warning event time-series, and recent Pod state
  transitions collected by a new `spoke_recorder` component on each
  spoke and proxied through the hub kernel
  (`/core/spoke_history/{cluster}/{tool_name}`).

- **Module management UI refresh** — the modules view has been rebuilt
  with concise Japanese labels and a per-module tab layout, and a new
  `module.ci` field records the CI/build pattern (P1..P4) that keeps
  each module's image in sync.

- **Git credentials store (org-scoped PATs)** — a new
  `hub/manifests/kernel/ConfigMaps/rag-backend-kernel-git-credentials.yaml`
  ConfigMap ships `git_credentials.py`, which registers
  `/core/git_credentials/*` on the hub kernel. GitHub, GitLab,
  Anthropic and TypeSafe PATs live as documents in a single
  OpenSearch `git_credentials` index keyed by
  `{scope_type}:{provider}:{scope_id}`, instead of one Kubernetes
  `Secret` per token. `github_account.resolve_github_token(user_id,
  owner)` now consults the store for org-scoped tokens before the
  `GITHUB_TOKEN` environment fallback, and every kernel GitHub
  endpoint in `github_ops.py` forwards the `owner` it receives from
  the MCP tool layer, so the migration is transparent to callers.
  A new admin-only Git 認証情報 view in `rag-frontend` renders the
  registered entries and drives register/verify/delete. See
  [`docs/operations/git-credentials.md`](docs/operations/git-credentials.md).

- **Central kubectl (per-spoke kubeconfig)** — a new
  `hub/manifests/kernel/ConfigMaps/rag-backend-kernel-kubectl.yaml`
  ConfigMap ships `kubectl_endpoints.py`. The hub kernel now stores
  each spoke's admin kubeconfig in an OpenSearch `kubeconfigs` index
  and exposes `POST /core/kubectl/execute` to run `kubectl` against
  any registered spoke over the reverse tunnel. The kubectl binary
  is downloaded lazily into an `emptyDir` cache (the kernel image
  does not carry it), the kubeconfig is piped through stdin without
  touching the filesystem, and every command is recorded in a
  `kubectl_audit` index. See
  [`docs/operations/kubectl-centralization.md`](docs/operations/kubectl-centralization.md).

- **Git mirror endpoints** —
  `hub/manifests/kernel/ConfigMaps/rag-backend-kernel-git-mirror.yaml`
  adds `git_mirror.py`, which drives `git clone --mirror` and
  `git push --mirror` as asynchronous jobs from `/core/git_mirror/*`.
  Job state and logs live in an OpenSearch `git_mirror_jobs` index.

- **GitLab account and repo endpoints** —
  `rag-backend-kernel-gitlab-account.yaml` and
  `rag-backend-kernel-gitlab-ops.yaml` add `gitlab_account.py` and
  `gitlab_ops.py`. Together they expose `/core/gitlab-account/*`
  (per-user PATs with multi-host support, resolved from the
  credentials store) and `/core/gitlab/*` (read/write on files,
  directories, and branches over the GitLab v4 REST API).

- **Native spoke on macOS (nsmt), hub side** —
  `rag-backend-kernel-native-spoke-mac.yaml` adds `native_spoke_mac.py`,
  the hub-side receiver for `nsmt`, the OS-native (no Kubernetes)
  spoke variant. Spokes register with a `nsmt-` prefixed `SPOKE_ID`
  and reach the hub over the same reverse tunnel used by Kubernetes
  spokes. The `nsmt` daemon itself is a **C# / .NET 10** executable
  and is scheduled to ship in the next release (**v3.1**); v3.0 puts
  the hub-side receiver in place so v3.1 becomes a drop-in
  addition. See
  [`docs/concepts/native-spoke.md`](docs/concepts/native-spoke.md).

- **Application spokes** — a second, lighter kind of spoke: one per
  application, `SPOKE_ID` prefix `app-`, ships as a `tunnel-client`
  plus `kernel-proxy` sidecar and nothing else. Cluster spokes and
  application spokes coexist on the same cluster; the `spoke_clusters`
  index now carries a `kind` field (`cluster` | `application` |
  `native`) and the topology view renders each shape distinctly.
  The reverse-tunnel wire format adds `HttpRequest` /
  `HttpResponse` variants at field 40, and `tunnel-server` enforces
  a path allowlist for `kind: application` callers. See
  [`docs/concepts/application-spoke.md`](docs/concepts/application-spoke.md).

- **Vulnerability scanning framework** —
  `rag-backend-kernel-security.yaml` adds `security_endpoints.py`,
  which exposes `/core/vuln/*` for mobile-app and OSS-dependency
  scans. Events land in `vuln_events` and per-app findings in
  `vuln_findings_mobile`; a Kubernetes CronJob drives the weekly
  scan schedule. The default scanner image is
  `ghcr.io/openspoke/openspoke-vuln-scanner:0.1.0` — override the
  registry via the CronJob environment before applying. See
  [`docs/operations/security-scanning.md`](docs/operations/security-scanning.md).

- **Chat frontend backend** —
  `rag-backend-kernel-chat.yaml` adds `chat_endpoints.py`, which
  registers `/core/chat/*`. The endpoint drives the in-browser
  chat view: it selects the model (Claude family or `gemma2`
  orchestrator), forwards the conversation, and records per-message
  token and cost so the dashboard can render them. Optional Jev
  (TypeSafe) integration provides prep/record helpers; each
  response exposes a `user_report` field alongside the streamed
  content. See
  [`docs/concepts/chat.md`](docs/concepts/chat.md).

- **Deployment templates for v3.0 additions** — a new
  [`examples/fleet-gitrepos/hub-kernel-frontend.yaml`](examples/fleet-gitrepos/hub-kernel-frontend.yaml)
  Fleet `GitRepo` template ships the operator dashboard (which had
  no ready-made template before) and can be applied alongside
  `hub-kernel.yaml`. A new
  [`examples/application-spoke/`](examples/application-spoke/)
  directory holds the minimum manifests — `Secret`, `Deployment`,
  and `Service` — for the application-spoke shape described in
  [`docs/concepts/application-spoke.md`](docs/concepts/application-spoke.md).
  [`docs/concepts/reverse-tunnel.md`](docs/concepts/reverse-tunnel.md)
  gains an "HTTP-proxy mode (v3.0)" section documenting the new
  `HttpRequest` / `HttpResponse` frame variants at field 40 and the
  `tunnel-server`-side path allowlist for `kind: application`
  callers.

- **Tunnel protocol v3.0 wire format** —
  `hub/images/tunnel-server/proto/tunnel.proto` and
  `spoke/images/tunnel-client/proto/tunnel.proto` (kept
  byte-identical) add the `HttpRequest` and `HttpResponse` frame
  variants at field 40. The change is additive; pre-v3.0
  `tunnel-server` and `tunnel-client` builds ignore the new
  variants, so existing cluster spokes keep working during a
  rollout. `hub/manifests/tunnel-server/Deployments/tunnel-server.yaml`
  gains a `KERNEL_URL` environment variable pointing at the
  cluster-internal hub kernel (default
  `http://rag-backend-kernel.rag-company1.svc.cluster.local:8000`).

- **Fleet template for mobile vulnerability scanning** —
  [`examples/fleet-gitrepos/hub-security-mobile.yaml`](examples/fleet-gitrepos/hub-security-mobile.yaml)
  ships a GitRepo template targeting
  `hub/manifests/security-mobile`. The bundle itself is shipped in
  the same release; see the next entry.

- **Mobile vulnerability scanning bundle** —
  `hub/manifests/security-mobile/` ships the complete bundle: the
  `security-mobile` Namespace, a `fleet.yaml`
  (`takeOwnership: true`, `atomic: false`), the
  `mobile-scan-targets` ConfigMap (generic placeholder example
  modules under `data.targets.yaml`), the Python scanners
  (`mobile-scan-script` for source, `mobile-binary-scan-ios-script`
  and `mobile-binary-scan-android-script` for built artefacts), the
  two runner ConfigMaps (`mobile-scan-runner` for the source path
  and `mobile-binary-scan-runner` for the binary path via a native
  spoke over the tunnel-server), and the weekly `mobile-scan` /
  `mobile-binary-scan` CronJobs. Both CronJobs default to
  `suspend: true` and `DRY_RUN=true` so a fresh install does not
  fire before the operator has swapped in real targets and done a
  dry run. Replace the example modules in `mobile-scan-targets`
  and, for the binary path, set `NSMT_MCP_URL` to the port that
  `spoke_register` allocated to your NSMT spoke before flipping
  the CronJobs on.

### Notes

- The v3.0 wire format and the matching `tunnel-server` and
  `tunnel-client` Rust implementations that carry the
  application-spoke HTTP proxy end-to-end ship together in this
  release. Application spokes are therefore usable end-to-end as
  soon as the two images are re-deployed. The manifests, `docs/`,
  and `examples/application-spoke/` are already in place.

- **Chat, Cost, Security and kubectl dashboard views** —
  four new `rag-frontend-code-dashboard-*.yaml` bundles ship the
  matching Streamlit views: **Chat** for the in-browser assistant,
  **Cost** for token and dollar accounting per project / spawn /
  chat, **Security** for vulnerability findings (with per-app
  current-status and diff-versus-previous drilldowns), and
  **kubectl** for the admin-only kubeconfig registration and audit
  log. Only Users, Git 認証情報 and kubectl remain admin-only;
  Chat, Cost and Security are visible to every authenticated user.

### Changed

- **Fleet GitRepo polling strategy** — the default pollingInterval is
  now selected per module: 1m for hot paths, 5m for typical
  applications, 15m for stable modules, to avoid the "gitcloner hang"
  that occurs when dozens of GitRepos poll every minute at the same
  second. See
  [`docs/operations/fleet-polling-strategy.md`](docs/operations/fleet-polling-strategy.md).

- **Milvus stabilised** — the hub Milvus stack has been upgraded to
  v2.6.20 with a mixCoord-consolidated topology; the `pulsar/data`
  volume is now a real PVC with a fixed `bookieId` and a bookie port
  3181 on the Service; and etcd is a 3-replica HA StatefulSet with
  periodic auto-compaction. See
  [`docs/operations/milvus-stabilization.md`](docs/operations/milvus-stabilization.md).

- **Tunnel implementations rewritten in Rust** —
  `hub/images/tunnel-server/` and `spoke/images/tunnel-client/`
  are now Rust programs (tonic 0.12 + prost 0.13 + tokio 1 +
  rustls + webpki-roots), shipping in
  `gcr.io/distroless/cc-debian12:nonroot` runtime images. The
  wire protocol, environment variables, and JSON log format are
  identical to the v2.0 Go builds, so onboarding, Fleet
  manifests, and reconnect behaviour are unchanged; upgrading is
  a re-deploy of `tunnel-server` and `tunnel-client`. The Go
  sources (`main.go`, `crand.go`, `go.mod`, `build.sh`) have
  been removed. Application spokes require this build to reach
  the hub kernel over the HTTP-proxy mode described in
  [`docs/concepts/reverse-tunnel.md`](docs/concepts/reverse-tunnel.md).

### Notes

- The `secretary_knowledge_*` collection in Milvus is preserved as a
  frozen rollback of the pre-migration knowledge base. Reading or
  writing `_handoff_*` ids through the vector store is now guarded at
  the kernel entrypoint and returns an error.
- Ladybug adapters are experimental and default to off. The kernel
  falls back to Milvus + Memgraph unless `BACKEND_*` env vars are set
  explicitly.
- The `gemma2` model used for topic tagging is optional; when the
  Ollama service is unreachable, tags are simply not populated and the
  KnowledgeBase view continues to work without the tag map.

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
