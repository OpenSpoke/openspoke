# Fleet GitRepo polling strategy

Standalone Fleet reconciles Bundles by cloning each `GitRepo`'s repo
and rebuilding the Bundle graph. When many `GitRepo` resources share
the same short polling interval, the clones stack up on the Fleet
controller and the reconciler can wedge into a state where new
Bundles are queued behind slow `git clone` jobs that never quite
finish — the "gitcloner hang" that used to bite this project every
few weeks.

The v3.0 answer is not to run a background daemon or add a queue: it
is to pick per-`GitRepo` polling intervals so that only the paths
that need fast feedback poll fast, and everything else polls at 5 or
15 minute cadence.

## The three tiers

Pick one of three intervals for every `GitRepo` you add.

**1m — hot paths, up to a handful.**

Use for GitRepos that gate the developer feedback loop: the hub RAG
kernel while you are iterating on `/core/*` endpoints, the frontend
while you are iterating on a Streamlit view, the MCP server while you
are adjusting tool routing. Anything you want to see reflected in
under a minute.

Keep this list small. Two or three GitRepos on 1m is fine. Fifteen is
not.

**5m — typical applications.**

The default for most app-level GitRepos. Anything that gets committed
to a few times a day and where reconciling within five minutes is
plenty fast. Almost every GitRepo starts here.

**15m — stable modules.**

Use for GitRepos that rarely change and where you would rather have
the Fleet controller idle than polling: Milvus's static topology,
OpenSearch, Keycloak, Loki. The knowledge base's own Memgraph
manifests. Anything that would have moved once in the last six months.

## How to write it

```yaml
apiVersion: fleet.cattle.io/v1alpha1
kind: GitRepo
metadata:
  name: hub-kernel
  namespace: fleet-default
spec:
  repo: ...
  paths:
    - hub/manifests/kernel
  pollingInterval: 1m0s      # hot
```

```yaml
  pollingInterval: 5m0s      # typical
```

```yaml
  pollingInterval: 15m0s     # stable
```

The templates under `examples/fleet-gitrepos/` in this repo already
follow this convention: the hub kernel example is 1m, most others are
5m, stable modules are 15m.

## When to promote a GitRepo up a tier

Promote a `5m` GitRepo to `1m` if you notice yourself sitting on the
edit-commit-wait cycle and 5 minutes is meaningfully in the way.
Demote back to `5m` when the work is done. Promotions and demotions
are cheap — they are just a value change in a Fleet CR.

The important discipline is the demotion. It is easy to forget that a
GitRepo has been at `1m` since a rushed diagnosis session six months
ago.

## What not to do

- Do not run everything at `1m`. Two dozen 1m GitRepos was the
  configuration that produced the original gitcloner hang.
- Do not run everything at `15m`. Fleet's feedback loop stops being
  useful.
- Do not add a background job to bump polling. The tiers exist so
  you do not need one.

## Related documents

- [`docs/installation/fleet.md`](../installation/fleet.md) — the
  Standalone Fleet bootstrap itself.
- [`examples/fleet-gitrepos/README.md`](../../examples/fleet-gitrepos/README.md) —
  copy-and-paste GitRepo templates with the tiers already picked.
