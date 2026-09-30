# OpenSpoke

[![release](https://img.shields.io/badge/release-v3.0-blue)](CHANGELOG.md)
[![license](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

Hub-and-spoke Kubernetes GitOps platform built on Standalone Fleet, for
operating fleets of edge clusters running LLM / RAG / AI workloads.

## What's new in v3.0

- **Central knowledge base on Memgraph** — the hub knowledge base
  (`_handoff_*` topics accessed via `system_brief` / `system_topics` /
  `system_lookup` / `upsert_knowledge` / `delete_knowledge`) now lives in
  Memgraph. The legacy Milvus `secretary_knowledge_*` collection is kept as
  frozen rollback data. See
  [`docs/concepts/knowledge-base.md`](docs/concepts/knowledge-base.md).
- **Knowledge Base dashboard** — a new admin-only Streamlit tab renders the
  schema graph, tag co-occurrence map, topic list, and system_brief in one
  place, backed by Memgraph and the hub kernel `/core/handoff/*` API.
- **Module policy management** — the module view is rebuilt around a
  three-layer model: module-wide policy (`restart_safety` / `edit_safety` /
  `desired_state`), CI pattern (`P1`–`P4`) with builder / image / manifest
  refs, and per-bundle policy overrides with orphan detection. Bundle lists
  are derived dynamically from Fleet GitRepo, so the ledger no longer
  duplicates truth.
- **Cluster history drill-down** — the cluster view now surfaces recorder
  health, pod restart timelines, Kubernetes warning event timelines, and
  Fleet Bundle / Deployment change history, all served through a
  hub-kernel proxy (`/core/spoke_history/*`) to spoke-local OpenSearch.
- **Spawn / Project dashboards** — async LLM task queue and orchestrator
  runs are now browsable with local-timezone rendering (`TZ` env,
  Asia/Tokyo by default).
- **Milvus 2.6 rebase** — full migration to Milvus v2.6.20 with three-node
  etcd HA, Pulsar bookie stability fixes (PVC for `/pulsar/data`, fixed
  `bookieId` via `standalone.conf`, bookie port 3181 in the Service), and
  a mixcoord + streamingnode topology. The legacy per-coordinator Services
  are preserved as DNS-compatible aliases pointing at mixcoord.
- **MCP `/mcp` Streamable HTTP with Valkey session persistence** — the hub
  MCP server now exposes a working `/mcp` endpoint alongside `/sse`, with a
  lifespan-managed `StreamableHTTPSessionManager`, a resumable session
  layer backed by Valkey Streams (no external Python deps), and a 30-minute
  idle session timeout. See
  [`docs/concepts/mcp-endpoints.md`](docs/concepts/mcp-endpoints.md).
- **Ladybug single-DB evaluation** — a kùzu-based combined vector + graph
  backend is integrated behind a pluggable backend abstraction, so hybrid
  search can be evaluated against Milvus + Memgraph as an alternative
  single-DB path. See
  [`docs/concepts/ladybug.md`](docs/concepts/ladybug.md).
- **Keycloak SSO logout chain** — the sidebar logout link now chains
  through `oauth2-proxy` to the Keycloak RP-Initiated Logout endpoint, so
  the identity provider session is cleared instead of being restored on
  the next request.

Full details in [`CHANGELOG.md`](CHANGELOG.md).

## What is OpenSpoke?

OpenSpoke is a control-plane platform where a single **hub cluster**
orchestrates many geographically distributed **spoke nodes** via GitOps.
All infrastructure state — from Fleet itself down to individual application
manifests — is stored in Git and reconciled by Standalone Rancher Fleet.

Two spoke modes are supported:

- **Kubernetes spoke** — a full Kubernetes cluster running the Fleet agent,
  suitable for edge sites with sufficient compute and existing k8s tooling
- **Native spoke** — a small daemon (currently the macOS-targeted
  `nsmt`, written in **C# / .NET 10**) that opens the same reverse
  tunnel without Kubernetes, so a laptop or a shared build machine can
  act as a first-class spoke. **The spoke daemon itself ships in
  v3.1**; v3.0 already includes the hub-side receiver, so v3.1 becomes
  a drop-in addition. See
  [`docs/concepts/native-spoke.md`](docs/concepts/native-spoke.md).

The hub ships with a batteries-included LLM / RAG stack (Memgraph, Milvus,
Valkey, OpenSearch, guardrails, MCP tools) so that AI workloads can be
delivered to spokes and remotely managed from day one.

## Repository layout

- [`hub/`](hub/) — hub cluster components (manifests + container image build contexts)
- [`spoke/`](spoke/) — spoke components (Kubernetes mode; native-daemon mode arriving in v3.1)
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
