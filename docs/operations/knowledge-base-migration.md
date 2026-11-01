# Migrating the knowledge base to Memgraph

The v3.0 knowledge base (system brief, topics, tags) lives in
Memgraph. Earlier versions stored the same information as
`_handoff_*` rows in the general RAG vector collection
(`secretary_knowledge_*` in Milvus). This document describes how to
migrate an existing OpenSpoke installation without downtime and
without losing data.

Read [`docs/concepts/knowledge-base.md`](../concepts/knowledge-base.md)
first for the design rationale — this document assumes you have
already decided to migrate.

## What the migration does and does not do

Does:

- Copy every row in the vector store whose id starts with
  `_handoff_brief` or `_handoff_topic_` or `_handoff_summary_` into
  Memgraph as `(:Brief)` or `(:Topic)` nodes.
- Merge each `_handoff_topic_X` and `_handoff_summary_X` pair into a
  single `Topic` node with both `content` and `summary` properties.
- Route all future reads and writes of `_handoff_*` ids to Memgraph
  through the six MCP tools and the seven kernel endpoints.
- Guard the old vector-store endpoints so that stray writes cannot
  desynchronise Memgraph and the frozen Milvus copy.

Does not:

- Touch general RAG content in the vector store. Documents, chunks,
  and non-`_handoff_*` rows continue to live in Milvus and are read
  by `vector_search`, `retrieve_chunks`, `search_rag`, and friends
  exactly as before.
- Delete the old `_handoff_*` rows in Milvus. They are kept as a
  frozen rollback snapshot.
- Populate topic tags. See "After migration" below.

## Prerequisites

- A hub running OpenSpoke v3.0 with Memgraph reachable from the
  kernel over Bolt.
- The kernel and MCP images updated to the v3.0 revision that
  includes the `/core/handoff/*` endpoints and the id-based routing
  in the MCP tools.
- Recent Milvus backups. The migration is safe and reversible, but
  you are moving live operational state and a backup is prudent.

## Step 1 — verify the current state

Before starting, confirm the kernel exposes the new endpoints:

```
POST /core/handoff/topics/list
```

Should return an empty list. If it returns 404, the kernel has not
picked up the v3.0 ConfigMap; fix that first.

Confirm how many `_handoff_*` rows are in the vector store:

```
POST /core/knowledge/query
{"namespace": "your_namespace", "expr": "id like '_handoff_%'"}
```

Note the count. The migration should move all of them.

## Step 2 — run the one-shot migration

Call the migration endpoint with `dry_run=true` first:

```
POST /core/handoff/migrate
{"dry_run": true}
```

The response lists what would be created: how many `Brief` nodes
(should be 1), how many `Topic` nodes, how many rows were skipped
(backups, test rows), and any warnings for rows that appear as a
`_handoff_topic_X` without a matching `_handoff_summary_X` (or vice
versa). Read the warnings and confirm they are benign for your data.

Then run it for real:

```
POST /core/handoff/migrate
{"dry_run": false}
```

The endpoint is idempotent — running it twice is safe. On a hub with
a few hundred topics it completes in a few seconds.

## Step 3 — verify the new state

Read a topic through the new path:

```
POST /core/handoff/topic/get
{"id": "some_topic_you_know"}
```

Should return the same content that the vector-store row had.

Read the topic index:

```
POST /core/handoff/topics/list
```

The count should match the vector store, minus skipped rows. Each
entry has `id`, `title`, `summary`, `category`, and `updated_at`;
`content` is intentionally omitted.

At this point Claude should be able to open a new session, run
`system_brief`, `system_topics`, and `system_lookup`, and see exactly
the same content as before.

## Step 4 — leave the old rows in place

Do not delete the `_handoff_*` rows in the vector store. They are
your rollback snapshot. The kernel already guards
`/core/knowledge/upsert` and `/core/knowledge/delete` against
`_handoff_*` ids so nothing will drift.

If you ever need to roll back, revert the MCP tool routing commits
and the kernel guard commits. The old rows are still there and the
system falls back to the pre-migration behaviour.

## After migration

The migration copies content, not tags. Tags are populated in two
ways:

1. **Ongoing** — every subsequent `upsert_knowledge` call enqueues
   the background classifier, which asks a local LLM for one to three
   tags per topic. As topics are updated, they gain tags.

2. **One-off backfill** — to tag existing topics without waiting for
   each of them to be edited, run a small script that iterates over
   `system_topics()` and calls the kernel's tagging helper directly.
   The concepts document sketches the prompt shape; the exact wiring
   is deployment-specific.

Either way, the KnowledgeBase view in the hub frontend will show
tagged and untagged topics correctly from day one.

## Rolling back

If something is wrong after Step 2:

1. Revert the MCP tool routing commits.
2. Revert the kernel guard commits.
3. Optionally clear the Memgraph nodes:

   ```cypher
   MATCH (t:Topic) DETACH DELETE t;
   MATCH (b:Brief) DETACH DELETE b;
   MATCH (tag:Tag) DETACH DELETE tag;
   ```

The vector-store rows are still there and untouched. The system
returns to its pre-v3.0 behaviour.

## Related documents

- [`docs/concepts/knowledge-base.md`](../concepts/knowledge-base.md)
