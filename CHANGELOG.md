# Changelog

All notable changes to OpenSpoke are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v3.0] — 2026-11

Initial public release.

### Hub operator frontend

- **Nuxt 3 operator frontend (`openspoke-frontend`)** —
  [`hub/images/kernel-frontend/`](hub/images/kernel-frontend/) ships a
  Nuxt 3 application served by `openspoke-frontend`, with Keycloak
  authentication handled by a sidecar `oauth2-proxy`. The bundle in
  [`hub/manifests/kernel-frontend/`](hub/manifests/kernel-frontend/)
  (Deployments for the Nuxt app and `oauth2-proxy`, Ingress for
  `llm.example.com`) exposes the operator views: Projects, Spawns,
  Clusters, Chat, Cost, Security, kubectl, Knowledge Base, Git
  credentials. Operators replace the placeholder image references
  (`your-registry.example.com/openspoke-frontend:latest`,
  `quay.io/oauth2-proxy/oauth2-proxy:v7.15.2`) and the
  `llm.example.com` / `keycloak.example.com` hosts with their own.

### Hub kernel

- **Kernel ConfigMap split (`kernel` + `kernel2`)** — Fleet caps a
  single bundle at 1 MiB, so the hub kernel's modular ConfigMaps are
  split into two Fleet bundles. The hot path (`rag-backend-kernel`,
  `rag-backend-kernel-fleet-credentials`, Deployments, PDBs) lives in
  [`hub/manifests/kernel/`](hub/manifests/kernel/); every other
  per-endpoint ConfigMap lives in
  [`hub/manifests/kernel2/`](hub/manifests/kernel2/) and is delivered
  through a sibling `GitRepo`. The Deployment mounts both bundles.

- **Knowledge base (Memgraph)** — the hub knowledge base lives in
  Memgraph as `(:Brief {id})` and
  `(:Topic {id, title, summary, content, category, tags, created_at,
  updated_at})` nodes, and `(:Tag {name})` nodes with `HAS_TAG`
  relationships. The kernel exposes seven `/core/handoff/*` endpoints
  (brief get/upsert, topics list, topic get/upsert/delete, migrate).
  MCP tools (`system_brief`, `system_topics`, `system_lookup`,
  `upsert_knowledge`, `delete_knowledge`, `upsert_knowledge_batch`)
  dispatch by id. See
  [`docs/concepts/knowledge-base.md`](docs/concepts/knowledge-base.md).

- **Automatic topic tagging** — the `/core/handoff/topic/upsert`
  endpoint runs a background task that asks a local LLM (default
  `gemma2:9b` served by Ollama at `OLLAMA_URL`) to propose one to
  three tags per topic and stores them as a `tags` property on the
  `Topic` node and `HAS_TAG` relationships to `Tag` nodes. The
  upsert itself always succeeds; `gemma2` is optional and when
  Ollama is unreachable tags are left empty and the Knowledge Base
  view continues to work without the tag map.

- **Knowledge base body in SysML v2** — topic bodies written to
  Memgraph on or after the `SYSML_KB_CUTOFF` date must parse as
  SysML v2 textual notation. The check runs at
  `MemgraphHandoffStore.topic_upsert` in `backends.py` and calls the
  `sysml-validator` service for a two-stage check: `sysml2` 0.3.1
  for syntax, `mercurio-sysml` 0.86.0 for semantic resolution
  (types, redefinitions, names in expressions). Narrative content
  (sequence of events, commands, logs, lessons) belongs in
  `doc /* ... */` comments on the relevant `package` or `part`
  elements, not in free prose. Operators can disable the check in
  an emergency with `SYSML_KB_ENFORCE=off`; the default is `strict`.

- **SysML v2 validator service (`sysml-validator`)** — a Rust HTTP
  service in
  [`hub/images/sysml-validator/`](hub/images/sysml-validator/) that
  bundles the `sysml2` syntax checker and the `mercurio-sysml`
  semantic validator behind a single `POST /validate` endpoint.
  The service includes a mutation-based negative control
  (`src/main.rs`) so a regression in the validator is caught at the
  service level.
  [`.openspoke/build.json`](hub/images/sysml-validator/.openspoke/build.json)
  wires it into the in-cluster CI with `manifest_bump`. The
  Deployment runs two replicas with `maxUnavailable: 0` by default.
  Bundle:
  [`hub/manifests/sysml-validator/`](hub/manifests/sysml-validator/).

- **Chat endpoints (`/core/chat/*`)** — `chat_endpoints.py` drives
  the in-browser chat view: it selects the model (Claude family or
  `gemma2` orchestrator), forwards the conversation, and records
  per-message token and cost so the dashboard can render them.
  Each response exposes a `user_report` field alongside the
  streamed content. See
  [`docs/concepts/chat.md`](docs/concepts/chat.md).

- **Git credentials store (org-scoped PATs)** —
  `rag-backend-kernel-git-credentials.yaml` ships
  `git_credentials.py`, which registers `/core/git_credentials/*`.
  GitHub, GitLab, Anthropic, and TypeSafe PATs live as documents in
  a single OpenSearch `git_credentials` index keyed by
  `{scope_type}:{provider}:{scope_id}`.
  `github_account.resolve_github_token(user_id, owner)` consults
  the store for org-scoped tokens before the `GITHUB_TOKEN`
  environment fallback. See
  [`docs/operations/git-credentials.md`](docs/operations/git-credentials.md).

- **Central kubectl (per-spoke kubeconfig)** —
  `rag-backend-kernel-kubectl.yaml` ships `kubectl_endpoints.py`.
  The hub kernel stores each spoke's admin kubeconfig in an
  OpenSearch `kubeconfigs` index and exposes
  `POST /core/kubectl/execute` to run `kubectl` against any
  registered spoke over the reverse tunnel. The kubectl binary is
  downloaded lazily into an `emptyDir` cache, the kubeconfig is
  piped through stdin without touching the filesystem, and every
  command is recorded in a `kubectl_audit` index. See
  [`docs/operations/kubectl-centralization.md`](docs/operations/kubectl-centralization.md).

- **Git mirror endpoints** — `rag-backend-kernel-git-mirror.yaml`
  adds `git_mirror.py`, which drives `git clone --mirror` and
  `git push --mirror` as asynchronous jobs from
  `/core/git_mirror/*`. Job state and logs live in an OpenSearch
  `git_mirror_jobs` index.

- **GitLab account and repo endpoints** —
  `rag-backend-kernel-gitlab-account.yaml` and
  `rag-backend-kernel-gitlab-ops.yaml` add `gitlab_account.py` and
  `gitlab_ops.py`. Together they expose `/core/gitlab-account/*`
  (per-user PATs with multi-host support, resolved from the
  credentials store) and `/core/gitlab/*` (read/write on files,
  directories, and branches over the GitLab v4 REST API).

### Spokes added in v3.0

- **Application spokes** — a lighter kind of spoke: one per
  application, `SPOKE_ID` prefix `app-`, ships as a `tunnel-client`
  plus `kernel-proxy` sidecar. Cluster spokes and application
  spokes coexist on the same cluster; the `spoke_clusters` index
  carries a `kind` field (`cluster` | `application` | `native`)
  and the topology view renders each shape distinctly.
  `tunnel-server` enforces a path allowlist for `kind: application`
  callers. The reverse-tunnel wire format adds `HttpRequest` /
  `HttpResponse` frame variants at field 40 for this mode. See
  [`docs/concepts/application-spoke.md`](docs/concepts/application-spoke.md).

- **Native spoke (hub-side receiver)** —
  `rag-backend-kernel-native-spoke-mac.yaml` adds
  `native_spoke_mac.py`, the hub-side receiver for an OS-native
  (no Kubernetes) spoke variant. Spokes register with a `nsmt-`
  prefixed `SPOKE_ID` and reach the hub over the same reverse
  tunnel used by Kubernetes spokes. See
  [`docs/concepts/native-spoke.md`](docs/concepts/native-spoke.md).

### Vulnerability scanning

- **Scanner image builder (`security-builder`)** —
  [`hub/manifests/security-builder/`](hub/manifests/security-builder/)
  ships two generic ConfigMaps (`trivy-builder-config` and
  `vuln-scanner-config`) that build the Trivy base image and the
  OpenSpoke `openspoke-vuln-scanner` image.

- **Vulnerability scanning framework** —
  `rag-backend-kernel-security.yaml` exposes `/core/vuln/*` for
  OSS-dependency scans. Events land in `vuln_events` and
  per-app findings in `vuln_findings_mobile`. A Kubernetes
  CronJob drives the weekly scan schedule. Default scanner image:
  `ghcr.io/openspoke/openspoke-vuln-scanner:0.1.0` (override via
  the CronJob environment). See
  [`docs/operations/security-scanning.md`](docs/operations/security-scanning.md).

- **Mobile vulnerability scanning bundle** —
  [`hub/manifests/security-mobile/`](hub/manifests/security-mobile/)
  ships the `security-mobile` Namespace, the `mobile-scan-targets`
  ConfigMap (placeholder modules), the Python scanners
  (`mobile-scan-script`, `mobile-binary-scan-ios-script`,
  `mobile-binary-scan-android-script`), runner ConfigMaps
  (`mobile-scan-runner`, `mobile-binary-scan-runner`), and the
  weekly `mobile-scan` / `mobile-binary-scan` CronJobs. Both
  CronJobs default to `suspend: true` and `DRY_RUN=true` so a
  fresh install does not fire before the operator has swapped in
  real targets and done a dry run.

### In-cluster CI

- **CI dispatcher / runner (`builder`)** —
  [`hub/manifests/builder/`](hub/manifests/builder/) adds a
  dispatcher-and-runner pattern that pulls each target
  repository's `.openspoke/build.json` and dynamically creates a
  Kubernetes `Job` per build. The dispatcher runs on a `CronJob`
  (every 5 minutes by default) and also serves manual requests
  through a `ci-requests` ConfigMap. Insecure-registry hints are
  passed through the `CI_INSECURE_REGISTRIES` environment variable.
  `targets.json` ships empty — operators list the repositories
  they want built. See
  [`hub/manifests/builder/ConfigMaps/ci-dispatcher-config.yaml`](hub/manifests/builder/ConfigMaps/ci-dispatcher-config.yaml).

- **Reference-check CI** —
  [`hub/manifests/builder/ConfigMaps/reference-check-config.yaml`](hub/manifests/builder/ConfigMaps/reference-check-config.yaml)
  ships a generic verifier that clones a target repository, builds
  the `harness/`, runs the positive examples, and then runs the
  negative control to confirm the harness catches real defects.
  Repository URL and branch arrive through the `Job`'s environment
  variables, so the same ConfigMap is reused across targets.

### Fleet operations

- **Fleet GitRepo polling strategy** — the default
  `pollingInterval` is selected per module: 1m for hot paths, 5m
  for typical applications, 15m for stable modules, to avoid the
  gitcloner contention that occurs when dozens of GitRepos poll
  every minute at the same second. See
  [`docs/operations/fleet-polling-strategy.md`](docs/operations/fleet-polling-strategy.md).

- **Fleet GitRepo templates** —
  [`examples/fleet-gitrepos/`](examples/fleet-gitrepos/) ships
  ready-made Fleet `GitRepo` templates per deployable component
  (hub kernel, hub kernel frontend, hub tunnel server, hub task
  worker, hub `sysml-validator`, hub `security-mobile`, spoke MCP,
  spoke tunnel client).
  [`examples/application-spoke/`](examples/application-spoke/)
  holds the minimum manifests (`Secret`, `Deployment`, `Service`)
  for the application-spoke shape.

### Milvus (vector store)

- **Milvus 2.6 topology** — the hub Milvus stack is pinned to
  v2.6.20 with a mixCoord-consolidated topology; the `pulsar/data`
  volume is a PVC with a fixed `bookieId` and a bookie port 3181
  on the Service; etcd is a 3-replica HA StatefulSet with
  periodic auto-compaction. See
  [`docs/operations/milvus-stabilization.md`](docs/operations/milvus-stabilization.md).

### Documentation

- [`docs/concepts/`](docs/concepts/) — architecture notes
  (`reverse-tunnel.md`, `mcp-endpoints.md`, `spawn.md`,
  `knowledge-base.md`, `application-spoke.md`, `native-spoke.md`,
  `chat.md`).
- [`docs/installation/`](docs/installation/) — setup guides for
  the hub, Rancher, Standalone Fleet, reverse-tunnel onboarding,
  Kubernetes spokes, and node labelling.
- [`docs/operations/`](docs/operations/) — operational runbooks
  (knowledge base migration, Milvus stabilisation, Fleet polling,
  Git credentials, kubectl centralisation, security scanning,
  user ACL).

## [v2.0] — 2026-08

### Added

- **Reverse tunnel (spoke → hub)** —
  [`hub/manifests/tunnel-server/`](hub/manifests/tunnel-server/) and
  [`spoke/kubernetes/template/tunnel-client/`](spoke/kubernetes/template/tunnel-client/)
  ship the tunnel transport. Image build contexts live in
  [`hub/images/tunnel-server/`](hub/images/tunnel-server/) and
  [`spoke/images/tunnel-client/`](spoke/images/tunnel-client/),
  both Rust (tonic 0.12 + prost 0.13 + tokio 1 + rustls +
  webpki-roots), shipping in
  `gcr.io/distroless/cc-debian12:nonroot` runtime images.
  `tunnel-server.yaml` carries a `KERNEL_URL` environment variable
  pointing at the cluster-internal hub kernel. Onboarding a spoke
  is one Secret (`tunnel-client-secret`) plus one Fleet target
  update; the spoke does not need a public hostname. See
  [`docs/concepts/reverse-tunnel.md`](docs/concepts/reverse-tunnel.md),
  [`docs/installation/reverse-tunnel.md`](docs/installation/reverse-tunnel.md).

- **MCP endpoints (`/sse` and `/mcp`)** — the hub MCP server serves
  both `/sse` (SSE transport) and `/mcp` (Streamable HTTP, MCP
  protocol revision 2025-03-26) on the same port. Streamable HTTP
  session state persists across pod restarts via Valkey, with a
  30-minute idle session timeout. See
  [`docs/concepts/mcp-endpoints.md`](docs/concepts/mcp-endpoints.md).

- **User ACL and admin role** — `is_admin()` consults the
  `openspoke_admin` Keycloak realm role. Per-user scopes live in a
  durable OpenSearch `user_acl` index; the frontend and the MCP
  server share the same policy source. See
  [`docs/operations/user-acl.md`](docs/operations/user-acl.md).

- **Spawn framework (async background LLM tasks)** — a
  `task-worker` Deployment
  ([`hub/manifests/task-worker/`](hub/manifests/task-worker/))
  executes asynchronous background LLM tasks. The kernel exposes
  `/core/spawn/*` and `/core/project/*` endpoints backed by
  OpenSearch indexes (`spawns`, `projects`, `spawn_counter`).
  Interactive sessions hand off work via MCP tools (`spawn`,
  `list_spawns`, `get_spawn_result`, `cancel_spawn`, `answer_spawn`,
  `list_spawn_questions`, `force_spawn_state`, `create_project`,
  `list_projects`, `get_project`, `update_project`,
  `archive_project`, `delete_project`, `add_project_member`,
  `remove_project_member`). Supports orchestrator-mode triage
  (`gemma2` → Claude escalation) and per-spawn usage tracking.
  See [`docs/concepts/spawn.md`](docs/concepts/spawn.md).

- **`spoke/images/`** — top-level directory for spoke-side
  container image build contexts, matching the existing
  `hub/images/` convention. Ships `tunnel-client/`.
