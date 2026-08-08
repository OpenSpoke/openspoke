# MCP endpoints: `/sse` and `/mcp`

*Streamable HTTP `/mcp` endpoint added in v2.0.*

## Two endpoints, one server

`mcp-company1` exposes MCP tools over two HTTP-transport variants:

- **`/sse`** — the original Server-Sent Events transport used by every
  MCP-capable Claude client since 2025. Long-lived one-way stream from
  server to client, plus a companion POST endpoint for client-to-server
  messages. Fine for interactive sessions but sensitive to intermediate
  proxies that buffer.
- **`/mcp`** — Streamable HTTP (MCP protocol revision 2025-03-26).
  A single HTTP endpoint that multiplexes requests and streamed
  responses, closer to a normal REST/streaming API. Traverses more
  intermediate proxies cleanly and is easier to load-balance.

Both endpoints share the same tool registry, the same authentication,
and the same permission model. Which one a client picks is a client
configuration choice.

## Which one should I use?

For most current clients, the answer is `/sse` — it is the transport
that shipped first, and most third-party MCP clients still default to
it.

Reach for `/mcp` when:

- You need to sit behind an ingress that mangles long-lived SSE
  connections (many WAFs, some CDNs, some corporate proxies).
- You want per-request tracing or replayability, where the single-request
  shape of Streamable HTTP is a better fit than a persistent SSE stream.
- Your client already speaks Streamable HTTP.

## Session persistence

Streamable HTTP sessions carry state that persists across Pod restarts.
`mcp-company1` stores that session state in Valkey using a small custom
RESP layer (no separate cache image, no extra Deployment). If Valkey
is unavailable, `/mcp` degrades gracefully to per-request stateless
behaviour — the server keeps answering, but a client that expects
mid-session state will need to re-establish it.

## Ingress path

Both endpoints are served by the same Pod on port `8000`. The default
Ingress rule for `mcp-company1` uses a `/` prefix, so both `/sse` and
`/mcp` fall out of the same rule with no further configuration.

## See also

- `hub/manifests/mcp/Deployments/mcp-company.yaml` — the Deployment
  that serves both endpoints.
- `hub/manifests/mcp/services/mcp-company1.yaml` — the ClusterIP that
  in-cluster clients dial.
