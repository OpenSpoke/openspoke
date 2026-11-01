# Central kubectl (per-spoke kubeconfig)

Version-first: **added in v3.0**.

The hub kernel adds a small routing layer that runs `kubectl` against
any registered spoke cluster from a single place. Each spoke's admin
`kubeconfig` lives as a document in the OpenSearch `kubeconfigs`
index instead of being stored per operator laptop, and the kernel
proxies the API calls over the same reverse tunnel the rest of the
hub already uses.

## Why

- Operators do not need a local kubeconfig per spoke; the hub is the
  single point of admin.
- Rotating a spoke's admin credential is a hub-side write, not a
  distributed change on every operator machine.
- Every kubectl command is captured in a `kubectl_audit` index next
  to the request, so *who did what on which spoke* is trivially
  answerable.

## Endpoints

- `POST /core/kubeconfig` — register or update a kubeconfig
- `GET /core/kubeconfig` — list registered clusters (kubeconfig
  contents stripped)
- `GET /core/kubeconfig/{cluster_name}` — read one entry (contents
  stripped)
- `DELETE /core/kubeconfig/{cluster_name}`
- `POST /core/kubectl/execute` — run a kubectl command

The read/write endpoints for kubeconfig content require
`role:openspoke_admin` in `X-Forwarded-Groups`. The execute endpoint
also requires admin.

## Storage

`kubeconfigs` is a single-shard index. Each document is keyed by
cluster name and carries:

- `cluster_name`
- `kubeconfig` — the entire YAML in cleartext (`index: false`)
- `server_url_override` — optional; the URL the kernel should
  substitute into the kubeconfig's `server:` field before running
  `kubectl`. This is how the hub swaps the spoke's original
  `https://kube-apiserver` for `http://tunnel-server:...`.
- `note`
- `created_at`, `updated_at`, `created_by`, `updated_by`
- `last_used_at`

Rotation is a `POST /core/kubeconfig` with the new YAML; the write
path re-runs `kubectl auth can-i --list` against the target cluster
and refuses to save a config that fails the sanity check.

## Execution

`POST /core/kubectl/execute` takes a cluster name and a command
line, resolves the kubeconfig, applies `server_url_override`,
downloads the right kubectl binary into an `emptyDir` cache (the
kernel image itself does not carry kubectl, so version drift is a
per-request setting), and pipes the config through stdin using
`--kubeconfig=/dev/stdin`. The kubeconfig never touches the local
filesystem.

The tunnel path is: kernel Pod → `tunnel-server` (hub) → spoke's
`tunnel-client` → spoke `kube-apiserver`. Nothing on the spoke side
needs to change; the same tunnel that carries kernel-to-spoke MCP
already carries the kubectl traffic.

## Audit

Every execute call writes a document to `kubectl_audit`: cluster
name, caller (`X-Forwarded-Preferred-Username`), the exact `argv`
that was executed, the exit code, and truncated `stdout` / `stderr`.
Failures write the same shape, so a broken command still leaves a
trail.

## UI

The **Clusters** view (v3.0) already surfaces per-spoke status. The
central-kubectl endpoints are consumed by operators from that view
and from ad-hoc scripts against the kernel API; there is no separate
kubectl-only page.

## Related

- [`git-credentials.md`](git-credentials.md) — the same store-in-
  OpenSearch approach used for GitHub / GitLab / Anthropic PATs.
- [`user-acl.md`](user-acl.md) — the `openspoke_admin` role gate.
