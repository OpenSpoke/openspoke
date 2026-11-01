# docs/concepts/

Background concepts and design rationale.

Available:

- [reverse-tunnel.md](reverse-tunnel.md) — the v2.0 spoke -> hub reverse
  tunnel (`tunnel-server` + `tunnel-client`), and when to use it instead
  of the v1 per-spoke public tunnel.
- [mcp-endpoints.md](mcp-endpoints.md) — the two MCP transports (`/sse`
  and `/mcp` Streamable HTTP), when to reach for each, and how session
  state is persisted.
- [spawn.md](spawn.md) — the spawn framework: asynchronous background
  LLM tasks, projects, orchestrator mode, usage tracking.
- [knowledge-base.md](knowledge-base.md) (v3.0) — the Memgraph-backed
  knowledge base (brief, topics, tags), how it replaced the vector-
  store `_handoff_*` rows, and the six MCP tools plus seven kernel
  endpoints that read and write it.
- [application-spoke.md](application-spoke.md) (v3.0) — the second
  kind of spoke: one-app-per-spoke, sidecar-only, `SPOKE_ID` prefix
  `app-`. Covers the `kind` field, the tunnel path allowlist, and
  the wire-format `HttpRequest` / `HttpResponse` variants at field
  40.
- [native-spoke.md](native-spoke.md) (v3.0) — the third kind of
  spoke: no Kubernetes at all, an OS-native daemon. `nsmt` (Native
  Spoke for Mac Terminal) is the reference implementation for
  macOS.
- [chat.md](chat.md) (v3.0) — the in-browser chat: the
  `/core/chat/*` endpoints, the `orchestrator` model with `gemma2`
  triage, per-turn `user_report`, and how the Cost view aggregates
  the resulting usage.

Planned: `hub-and-spoke.md`, `gitops-with-fleet.md`, `ledger-schema.md`.
