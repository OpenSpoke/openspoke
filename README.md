# OpenSpoke

[![release](https://img.shields.io/badge/release-v2.0-blue)](CHANGELOG.md)
[![license](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

Hub-and-spoke Kubernetes GitOps platform built on Standalone Fleet, for
operating fleets of edge clusters running LLM / RAG / AI workloads.

## What's new in v2.0

- **Reverse tunnel** — spoke -> hub gRPC tunnel replaces the v1 per-spoke
  public tunnel; onboarding a spoke is now one Secret + one Fleet target.
  See [`docs/concepts/reverse-tunnel.md`](docs/concepts/reverse-tunnel.md).
- **MCP Streamable HTTP** — the hub MCP server now exposes a `/mcp`
  Streamable HTTP endpoint alongside `/sse`, with Valkey-backed session
  persistence.  See [`docs/concepts/mcp-endpoints.md`](docs/concepts/mcp-endpoints.md).
- **User ACL & admin role** — Keycloak realm role (`openspoke_admin`)
  drives admin checks; per-user scopes live in a durable OpenSearch
  index. See [`docs/operations/user-acl.md`](docs/operations/user-acl.md).
- **Spawn framework** — async LLM task queue (`task-worker` + kernel
  `/core/spawn/*`), with orchestrator mode, projects, and usage
  tracking. See [`docs/concepts/spawn.md`](docs/concepts/spawn.md).

Full details in [`CHANGELOG.md`](CHANGELOG.md).

## What is OpenSpoke?

OpenSpoke is a control-plane platform where a single **hub cluster**
orchestrates many geographically distributed **spoke nodes** via GitOps.
All infrastructure state — from Fleet itself down to individual application
manifests — is stored in Git and reconciled by Standalone Rancher Fleet.

Two spoke modes are supported:

- **Kubernetes spoke** — a full Kubernetes cluster running the Fleet agent,
  suitable for edge sites with sufficient compute and existing k8s tooling
- **Native spoke** — a single Go binary (Windows / Linux / macOS), no
  Kubernetes required, ideal for lightweight edge devices

The hub ships with a batteries-included LLM / RAG stack (Memgraph, Milvus,
Valkey, OpenSearch, guardrails, MCP tools) so that AI workloads can be
delivered to spokes and remotely managed from day one.

## Repository layout

- [`hub/`](hub/) — hub cluster components (manifests + container image build contexts)
- [`spoke/`](spoke/) — spoke components (Kubernetes mode + native single-binary mode)
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
