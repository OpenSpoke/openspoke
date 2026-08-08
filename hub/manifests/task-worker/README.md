# task-worker (v2.0, spawn framework)

Hub-side background worker that executes spawns queued by the kernel.
Part of the [spawn framework](../../../docs/concepts/spawn.md) added
in v2.0.

## Layout

- `Deployments/rag-task-worker.yaml` — 3 replicas of the Python worker
  (`python:3.12-slim` + runtime pip install; `namespace: rag-company1`).
- `Services/rag-task-worker.yaml` — ClusterIP :8000 (health only).
- `ConfigMaps/rag-task-worker.yaml` — the worker's Python entrypoint
  (`main.py`), ~538 lines. Poll interval and Claude cost constants are
  environment-driven.
- `fleet.yaml` — Fleet takeOwnership.

## Configuration

- `KERNEL_URL` — Kernel base URL. Default:
  `http://rag-backend-kernel.rag-company1.svc.cluster.local:8000`.
- `WORKER_PICKUP_POLL_SEC` — how often to poll `/core/spawn/pickup`.
  Default `1.0`.
- `ANTHROPIC_API_KEY` — pulled from Secret `anthropic-api-key` (key
  `api-key`), optional at boot. Required for any Claude-mode spawn.
- Cost-per-million-token env vars per tier (Sonnet / Haiku / Opus)
  drive the `claude_cost_usd` snapshot in each spawn's `usage`.

## Secret

```sh
kubectl -n rag-company1 create secret generic anthropic-api-key \
  --from-literal=api-key=<your Anthropic API key>
```

Without this Secret the worker still boots and the health check passes,
but any picked spawn will fail on the first call to
`/core/claude/stream-with-mcp`.

## Modes

Each spawn document carries a `mode`. The worker handles both:

- `claude` (default) — direct Claude Sonnet call, tool loop up to
  `WORKER_MAX_TOOL_ITERATIONS` (default 30).
- `orchestrator` — call `/core/orchestrator/triage` first (small gemma2
  model). If the triage says the task is simple, answer with gemma2
  alone; otherwise escalate to Claude at the recommended tier.

See [`docs/concepts/spawn.md`](../../../docs/concepts/spawn.md) for the
full lifecycle.

## Supervisor spawns

`is_supervisor=true` on a spawn switches the system prompt into
"supervisor" mode: longer turn limit (20 vs 5), higher tool iteration
budget (50 vs 30), and a system prompt that instructs the spawn to
create child spawns with `parent_supervisor=<my_full_id>`. This is the
building block for long-running orchestrated multi-worker runs.
