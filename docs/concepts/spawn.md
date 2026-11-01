# Spawn framework

*Introduced in v2.0.*

## What it is

A **spawn** is an asynchronous, background LLM task. The interactive
Claude session hands work off to a spawn, keeps talking to the user, and
picks up the result later. Spawns are the building block for anything
that would otherwise pause the conversation for minutes — long analyses,
batch rewrites of large ConfigMaps, migrations, or fan-out reads across
many spokes.

## Shape

- **Kernel** — `rag-backend-kernel` owns three OpenSearch indexes:
  `spawns`, `projects`, and `spawn_counter`. Each spawn is one document
  in `spawns` with a state field (`queued`, `running`,
  `waiting_user`, `done`, `failed`, `cancelled`).
- **Worker** — a separate Pod (`task-worker`) polls the kernel for
  queued spawns, executes them, and writes results back through the
  kernel's HTTP API. Two-pickup races are prevented via OpenSearch
  optimistic concurrency (`if_seq_no` + `if_primary_term`).
- **Interactive Claude** — MCP tools `spawn`, `list_spawns`,
  `get_spawn_result`, `cancel_spawn`, and `answer_spawn` let a user (or
  a live Claude session) submit and manage spawns.

## Modes

Each spawn is submitted with a `mode` that decides how it will be run:

- `mode="claude"` (default) — the worker runs the task with the same
  Claude tier the interactive session is on. Best when the task needs
  strong reasoning or multi-step tool use.
- `mode="orchestrator"` — the worker first asks a small triage model
  (a locally hosted `gemma2:9b` is the reference implementation) to
  classify the task, then picks between (a) answering with `gemma2`
  alone if the task is simple, (b) escalating to Claude if it is
  complex, or (c) escalating to a higher Claude tier if the triage says
  it is very complex.

## Usage tracking

Every spawn's document carries a `usage` field:

```
usage:
  claude_input_tokens: ...
  claude_output_tokens: ...
  claude_cost_usd: ...
  claude_tool_calls: ...
  gemma2_calls: ...
  gemma2_input_tokens: ...
  gemma2_output_tokens: ...
  gemma2_elapsed_ms: ...
```

The frontend spawn list surfaces these as columns so an admin can see
the cost of a run at a glance and spot regressions where Claude usage
creeps up unexpectedly.

## Persistence

Spawn state lives in OpenSearch, not in Valkey. This is a deliberate
choice: OpenSearch has durable disk-backed storage in every OpenSpoke
deployment, whereas Valkey is used for ephemeral things (session
cookies, rate limiter buckets) and losing those on Pod restart is
acceptable. Losing a spawn queue would not be acceptable, so it lives
where the durability is.

## Projects

A **project** is a lightweight grouping of related spawns. It has
owner/member roles, and every spawn belongs to exactly one project.
Projects give admins one dashboard per initiative and make usage
attribution meaningful when many people share a hub.

## See also

- `hub/manifests/kernel/` — kernel Deployment and index setup.
- `hub/manifests/task-worker/` — the worker that executes spawns.
- MCP tool docs for `spawn` (available through any connected MCP client).
