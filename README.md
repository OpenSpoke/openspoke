# OpenSpoke

[![release](https://img.shields.io/badge/release-v3.0-blue)](CHANGELOG.md)
[![license](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

Hub-and-spoke Kubernetes GitOps platform built on Standalone Fleet, for
operating fleets of edge clusters running LLM / RAG / AI workloads.

## What's new in v3.0

- **Nuxt operator frontend with Keycloak SSO** —
  [`hub/images/kernel-frontend/`](hub/images/kernel-frontend/) is a
  Nuxt 3 application that runs with an `oauth2-proxy` sidecar against
  Keycloak. The bundle in
  [`hub/manifests/kernel-frontend/`](hub/manifests/kernel-frontend/)
  exposes operator views (Projects, Spawns, Clusters, Chat, Cost,
  Security, kubectl, Knowledge Base, Git credentials).
- **Knowledge base on Memgraph** — the hub knowledge base
  (`_handoff_*` topics accessed via `system_brief` / `system_topics` /
  `system_lookup` / `upsert_knowledge` / `delete_knowledge`) lives in
  Memgraph with automatic topic tagging via a local LLM (`gemma2`).
  See [`docs/concepts/knowledge-base.md`](docs/concepts/knowledge-base.md).
- **SysML v2 knowledge base enforcement** — topic bodies parse as
  SysML v2 textual notation. A dedicated Rust validator service
  (`sysml-validator`) enforces syntax and semantic checks.
- **Application and native spokes** — in addition to Kubernetes
  spokes, a lighter sidecar shape (application spoke) and an
  OS-native daemon shape (native spoke, hub-side receiver) share
  the same reverse tunnel. See
  [`docs/concepts/application-spoke.md`](docs/concepts/application-spoke.md),
  [`docs/concepts/native-spoke.md`](docs/concepts/native-spoke.md).
- **In-cluster CI (`builder`)** — a dispatcher-and-runner CI in
  [`hub/manifests/builder/`](hub/manifests/builder/). The dispatcher
  reads a per-repository `.openspoke/build.json` and creates a
  Kubernetes `Job` per build.
- **Vulnerability scanning** — generic Trivy /
  `openspoke-vuln-scanner` builder in
  [`hub/manifests/security-builder/`](hub/manifests/security-builder/),
  with OSS-dependency and mobile scanner CronJobs under
  [`hub/manifests/security/`](hub/manifests/security/) and
  [`hub/manifests/security-mobile/`](hub/manifests/security-mobile/).
- **Milvus 2.6** — the hub Milvus stack is pinned to v2.6.20 with a
  mixCoord-consolidated topology, PVC-backed `pulsar/data`, and
  3-replica HA etcd.
- **Fleet GitRepo polling strategy** — per-module polling intervals
  (1m / 5m / 15m) chosen to avoid gitcloner contention. See
  [`docs/operations/fleet-polling-strategy.md`](docs/operations/fleet-polling-strategy.md).

Full details in [`CHANGELOG.md`](CHANGELOG.md).

## What's new in v2.0

- **Reverse tunnel (spoke → hub)** — a Rust `tunnel-server` /
  `tunnel-client` pair that lets a spoke open its tunnel to the hub,
  so a spoke does not need a public hostname. Onboarding a spoke is
  one Secret plus one Fleet target update. See
  [`docs/concepts/reverse-tunnel.md`](docs/concepts/reverse-tunnel.md).
- **MCP endpoints (`/sse` and `/mcp`)** — the hub MCP server exposes
  `/sse` (SSE) and `/mcp` (Streamable HTTP, MCP protocol revision
  2025-03-26), with session state persisted via Valkey Streams. See
  [`docs/concepts/mcp-endpoints.md`](docs/concepts/mcp-endpoints.md).
- **User ACL and admin role** — `is_admin()` consults the
  `openspoke_admin` Keycloak realm role. Per-user scopes live in a
  durable OpenSearch `user_acl` index.
- **Spawn framework** — a `task-worker` Deployment executes
  asynchronous background LLM tasks. MCP tools (`spawn`,
  `list_spawns`, `get_spawn_result`, `cancel_spawn`, …) drive the
  queue; orchestrator-mode triage escalates from `gemma2` to Claude.
  See [`docs/concepts/spawn.md`](docs/concepts/spawn.md).

## What is OpenSpoke?

OpenSpoke is a control-plane platform where a single **hub cluster**
orchestrates many geographically distributed **spoke nodes** via GitOps.
All infrastructure state — from Fleet itself down to individual application
manifests — is stored in Git and reconciled by Standalone Rancher Fleet.

Three spoke modes are supported:

- **Kubernetes spoke** — a full Kubernetes cluster running the Fleet
  agent, suitable for edge sites with sufficient compute and
  existing k8s tooling.
- **Application spoke** — a lighter sidecar shape, one per
  application, that reaches the hub kernel over the same reverse
  tunnel the Kubernetes spoke uses. See
  [`docs/concepts/application-spoke.md`](docs/concepts/application-spoke.md).
- **Native spoke** — a small daemon that opens the same reverse
  tunnel without Kubernetes, so a laptop or a shared build machine
  can act as a first-class spoke. See
  [`docs/concepts/native-spoke.md`](docs/concepts/native-spoke.md).

The hub ships with a batteries-included LLM / RAG stack (Memgraph, Milvus,
Valkey, OpenSearch, guardrails, MCP tools) so that AI workloads can be
delivered to spokes and remotely managed from day one.

## Repository layout

- [`hub/`](hub/) — hub cluster components (manifests + container image build contexts)
- [`spoke/`](spoke/) — spoke components (Kubernetes mode and a Go reference skeleton for Native spoke)
- [`fleet/`](fleet/) — Standalone Fleet bootstrap and self-managed resources
- [`rancher/`](rancher/) — Rancher server reference deployment
- [`docs/`](docs/) — architecture, installation, and operational documentation
- [`examples/`](examples/) — end-to-end sample configurations

## Getting started

Documentation is in progress. Planned entry points:

- Architecture overview: [`docs/architecture.md`](docs/architecture.md)
- Quickstart: [`docs/quickstart.md`](docs/quickstart.md)
- Installation guides: [`docs/installation/`](docs/installation/)
- Operational runbooks: [`docs/operations/`](docs/operations/)

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) (to be added).

## Security

To report a security vulnerability, see [`SECURITY.md`](SECURITY.md) (to be added).

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See
[`NOTICE`](NOTICE) for attribution and third-party acknowledgements.
