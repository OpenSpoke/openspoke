# Knowledge base (Memgraph)

The hub's *knowledge base* is a small, curated store of reusable
context — a system brief, a set of topic-length notes, and the tags
that tie them together — that both the operator and the LLM read from
during a session. From v3.0 this store lives in Memgraph and is
accessed exclusively through six MCP tools and seven kernel endpoints.

## What lives there

Three node labels, plus one relationship type:

- `(:Brief {id: "brief", content, updated_at})` — a single node
  holding the system brief that every new session reads first. There
  is exactly one `Brief`; its `id` is the literal string `brief`.

- `(:Topic {id, title, summary, content, category, tags,
  created_at, updated_at})` — one node per topic. `id` is a short
  snake_case slug (for example `reverse_tunnel_20260807` or
  `knative_scale_to_zero_activator_path`) that appears in operator
  documents and in Claude prompts. `summary` is a one-line description
  used in the topic index; `content` is the full body. `tags` mirrors
  the outgoing `HAS_TAG` edges as a list property for cheap read paths.

- `(:Tag {name})` — one node per tag. Tags are lower snake_case
  (`memgraph`, `fleet`, `gitops`, `streamlit`, ...). A topic may have
  zero to three tags.

- `(:Topic)-[:HAS_TAG]->(:Tag)` — the actual assignment. The tag list
  on `Topic.tags` and the set of outgoing `HAS_TAG` edges are kept in
  sync by the `topic_set_tags` write path in the kernel.

Two unique indexes are created at startup: `:Brief(id)` and
`:Topic(id)`.

## Why Memgraph rather than a vector store

Earlier versions of OpenSpoke stored the same information as
`_handoff_*`-prefixed rows inside the general RAG vector collection
(`secretary_knowledge_*` in Milvus). That worked, but three things
did not:

1. **Reading the brief was slow.** Every session started by fetching
   a 20+ KB document by id from a vector store built for approximate
   similarity search.
2. **The topic index was expensive to keep small.** Listing 300
   topics as `{id, title, summary}` required 300 point lookups.
3. **Updates required an embedding round-trip** even when the update
   was purely textual.

Memgraph solves all three: `Brief` is a single point lookup by id,
`Topic` list queries are a single label scan projecting three
properties, and updates are plain Cypher `SET`s. Tags, which were
awkward as an ad-hoc field on top of vectors, become a first-class
part of the schema.

Semantic search over general RAG content (documents, chunks) is
untouched — that keeps using the vector store.

## Six MCP tools

The interface exposed to Claude has not changed:

- `system_brief()` — read the brief. Returns `content` + `updated_at`.
- `system_topics()` — read the topic index as
  `[{id, title, summary, category, updated_at, tags}]`. `content` is
  intentionally omitted; the caller is expected to follow up with
  `system_lookup` for the topics it actually needs.
- `system_lookup(topic)` — read one topic's full `content`.
- `upsert_knowledge(id, title, content, ...)` — create or update.
  The id is what routes the call: `_handoff_brief` writes the brief,
  `_handoff_topic_X` writes topic `X`'s `content`, and
  `_handoff_summary_X` writes topic `X`'s `summary` only.
  Any id that does not start with `_handoff_` still goes to the
  vector store — no behaviour change for general RAG content.
- `upsert_knowledge_batch(entries)` — same, in batches. Rows are
  routed one-by-one; a batch may freely mix Memgraph and vector-store
  entries.
- `delete_knowledge(id)` — delete a topic. Deleting `_handoff_brief`
  is refused.

## Seven kernel endpoints

The MCP tools dispatch to these endpoints on the kernel:

- `POST /core/handoff/brief/get`
- `POST /core/handoff/brief/upsert`
- `POST /core/handoff/topics/list`
- `POST /core/handoff/topic/get`
- `POST /core/handoff/topic/upsert`
- `POST /core/handoff/topic/delete`
- `POST /core/handoff/migrate` — one-shot import from the legacy
  vector store, used only during migration (see
  [`docs/operations/knowledge-base-migration.md`](../operations/knowledge-base-migration.md)).

All seven talk to Memgraph through the Bolt driver that the kernel
already keeps open. `topic/upsert` also enqueues a background task
that classifies the topic with a local LLM (see below).

## Automatic topic tagging

When a topic is upserted, the kernel enqueues a background task that
asks a local LLM — `gemma2:9b` served by Ollama at `OLLAMA_URL` by
default — to propose one to three tags. The prompt includes the
existing tag vocabulary (tags currently used on three or more topics)
as a hint, so the LLM tends to reuse rather than invent labels.

The task then:

1. Filters the returned tags to lowercase alphanumeric plus `_`/`-`,
   at most 40 characters each, and deduplicates.
2. Writes the resulting list to `Topic.tags`.
3. Deletes existing `HAS_TAG` edges for the topic and re-creates them,
   `MERGE`ing new `Tag` nodes as needed.

Any failure is logged and dropped — the `upsert_knowledge` call has
already succeeded by the time tagging runs, and the next upsert of
the same topic will retry.

## Guarding the old vector-store path

The pre-v3.0 flow — writing `_handoff_*` rows through the vector
store — is now refused at the kernel entrypoint
(`/core/knowledge/upsert` and `/core/knowledge/delete`). The old rows
are kept in place, frozen, and can be read back if a rollback is
needed, but new writes fail with an explicit error steering the
caller to the `/core/handoff/*` endpoints.

The MCP tool `vector_query` on `secretary_knowledge_*` collections
carries a docstring warning that `_handoff_*` rows there are frozen
snapshots — the Memgraph store is the authoritative copy.

## Related documents

- [`docs/operations/knowledge-base-migration.md`](../operations/knowledge-base-migration.md) —
  step-by-step guide for migrating an existing vector-store knowledge
  base into Memgraph.

