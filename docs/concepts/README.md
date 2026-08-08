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

Planned: `hub-and-spoke.md`, `gitops-with-fleet.md`, `ledger-schema.md`.
