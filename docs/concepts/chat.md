# Chat backend and Cost accounting

Version-first: **added in v3.0**.

The v3.0 hub adds a first-class in-browser chat that lets an
authenticated user talk to Claude directly, with the model router,
token accounting, and knowledge-base access wired up on the server
side. It is exposed as a Streamlit view (`Chat`) and backed by a new
`/core/chat/*` endpoint group; a second dashboard view (`Cost`) plots
the resulting spend.

## Endpoints

The kernel loads `chat_endpoints.py` from
`hub/manifests/kernel/ConfigMaps/rag-backend-kernel-chat.yaml`. The
router registers under `/core/chat`.

Requests carry:

- `messages` — the conversation so far, in the usual assistant / user
  role shape
- `model` — a preferred model, or `orchestrator` to let the router
  pick
- optional `project_id` / `spawn_id` — tags used by the Cost view

Responses always include:

- `content` — the assistant's reply, streamed as Server-Sent Events
- `usage` — `{ prompt_tokens, completion_tokens, total_tokens }`
- `cost` — the estimated dollar amount using the current price table
- `topic_id` — set when a knowledge-base entry was created or updated
- `user_report` — a short human-readable summary of what happened
  during this turn

Every completed turn is written to a `chat_events` index and — for
runs that touched the router — to `claude_usage` as well, so the Cost
view has one table to sum.

## Orchestration

When `model` is `orchestrator` (or unset), the endpoint hands the
prompt to `gemma2` for triage. `gemma2` classifies the request, and
the router picks the smallest model that can plausibly answer:

- trivial follow-ups stay on `gemma2`
- routine assistance goes to a smaller Claude
- code, long-form reasoning, or tool-heavy work goes to a larger
  Claude

Triage failures fall back automatically to a Claude call, so a
`gemma2` outage never blocks the user.

## Jev integration

If the request is admin-tagged as a Jev turn — for example when the
chat is being driven from the operator dashboard — the endpoint calls
`jev_prep_turn` before dispatch and `jev_record_turn` afterwards.
Both hit the TypeSafe API using the token stored under
`org:typesafe:typesafe` in the credentials store (see
[`../operations/git-credentials.md`](../operations/git-credentials.md));
neither is required for ordinary user chat.

## The Cost view

`view_cost.py` reads `claude_usage`, `chat_events`, and the spawn /
project tables and renders daily totals broken down by project,
spawn, and chat. It is intentionally an aggregate view — nothing in
the UI reveals user prompts. The dashboard is visible to any
authenticated user; admins additionally see per-user totals.

## Related

- [`../operations/git-credentials.md`](../operations/git-credentials.md)
  — the credentials store that holds the Anthropic and TypeSafe
  tokens the chat endpoint uses.
- [`spawn.md`](spawn.md) — how longer-running conversations are
  handed off to the async task worker.
- [`knowledge-base.md`](knowledge-base.md) — the Memgraph-backed
  knowledge base that stores `topic_id` entries produced by the
  chat.
