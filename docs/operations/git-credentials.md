# git_credentials store (Phase 1a)

Version-first: **added in v3.0**.

The hub kernel ships a small OpenSearch-backed *credentials store* that
holds the Personal Access Tokens (PATs) used to talk to GitHub, GitLab,
Anthropic, and TypeSafe. Instead of one Kubernetes `Secret` per token,
tokens live as documents in a single `git_credentials` OpenSearch index
scoped by owner (organisation) or, on GitLab, by `{host}:{account}`.

The store is provisioned lazily: `_ensure_index()` runs on kernel start
and again on the first write, so the index appears without any manual
bootstrap. Access is admin-only through the frontend view; other kernel
modules resolve tokens via internal Python calls.

## Endpoints

All endpoints hang off `/core/git_credentials/`. The `PUT`, `DELETE`,
`GET /list` and `GET /{provider}/{scope_type}/{scope_id}` variants
require the caller to carry `role:openspoke_admin` in the
`X-Forwarded-Groups` header injected by `oauth2-proxy`. The
`POST .../token` internal endpoint deliberately does **not** require
that header; it is reachable only from other kernel Pods over the
ClusterIP Service (Phase 2 will add an internal shared secret).

- `GET /health`
- `GET /list?provider=github|gitlab&scope_type=org` — token stripped
- `GET /{provider}/{scope_type}/{scope_id}` — token stripped
- `PUT /{provider}/{scope_type}/{scope_id}` — register or update
- `DELETE /{provider}/{scope_type}/{scope_id}`
- `POST /{provider}/{scope_type}/{scope_id}/verify` — hit the provider
- `POST /{provider}/{scope_type}/{scope_id}/token` — internal fetch

`{scope_id}` for GitHub is the organisation login
(e.g. `myorg`). For GitLab it is `{host}:{account_name}`, for example
`gitlab.example.com:myorg`. `{scope_type}` is `org` today; `user` is
reserved for a future per-user rollout and is validated but not written
by the current UI.

## OpenSearch document layout

`_id = "{scope_type}:{provider}:{scope_id}"`, single-shard index with
`replicas=0`. The fields are:

- `provider` — `github` | `gitlab` | `anthropic` | `typesafe`
- `host` — `github.com`, `gitlab.example.com`, etc.
- `scope_type` — `org` | `user`
- `scope_id` — the scope key (see above)
- `account_type` — GitLab only, `user` | `group`
- `account_name` — display name
- `token` — the PAT in cleartext (`index: false`)
- `note` — free text
- `api_base_url` — `https://api.github.com`, `https://{host}/api/v4`
- `created_at`, `updated_at`, `created_by`, `updated_by`
- `last_used_at` — updated by the internal `token` endpoint
- `verification` — `{ok, checked_at, login, scopes, error}` from the
  most recent `verify` call

Tokens are stored in cleartext. Encryption at the record level is not
attempted — the operational model relies on disk-level encryption for
the underlying OpenSearch volume and on admin-only access at the API
boundary. Rotate tokens through the UI or `PUT` endpoint; the write
path always re-runs `verify` and refuses to save a token the provider
rejects.

## Kernel modules that consume the store

`github_account.resolve_github_token(user_id, owner)` in
`hub/manifests/kernel/ConfigMaps/rag-backend-kernel-github-account.yaml`
resolves a GitHub PAT in this order:

1. per-user token from the pre-existing `secretary_github_accounts`
   store (unchanged from v2.x)
2. per-organisation token from `git_credentials` via
   `resolve_github_token_for_org(owner)` (new in v3.0)
3. the `GITHUB_TOKEN` environment variable, with a warning log
4. otherwise, `RuntimeError`

The `owner` argument reaches this function because every kernel GitHub
endpoint in `rag-backend-kernel-github-ops.yaml` now forwards it:

```python
github_token = resolve_github_token(
    request.get("user_id"),
    owner=request.get("owner"),
)
```

The MCP tool schemas already carry `owner` as an explicit argument, so
the layer above the kernel needed no changes.

Anthropic and TypeSafe entries follow the same shape, and the kernel
modules that talk to those providers import
`resolve_anthropic_token_for_org` and
`resolve_typesafe_token_for_org` from `git_credentials` directly.
Doing an in-process import instead of an HTTP round-trip to
`/core/git_credentials/.../token` avoids a class of deadlocks in
Uvicorn workers that also handle the incoming request.

## Bootstrap mode

The kernel reads `CREDENTIALS_BOOTSTRAP_MODE` from the environment.

- `strict` (default) — the store is authoritative. The env fallback
  path in `resolve_github_token` still returns a token when the store
  is empty, but it emits a warning so operators can see the missing
  entries.
- `bootstrap` — same behaviour today. This mode is reserved for a
  future revision that will refuse the env fallback entirely once
  Anthropic and GitLab paths have been in production long enough to
  drop `GITHUB_TOKEN` from the kernel `Deployment`.

## Fleet-level Git credentials are separate

`fleet/manifests/managed/Secrets/fleet-git-credentials*.yaml` are used
by Fleet itself when it clones a private `GitRepo` and are unrelated
to this store. Rotating a Fleet credential is a Fleet configuration
change; rotating a kernel-side PAT is a UI action here.

## UI

The admin dashboard adds a **Git 認証情報** view (URL: `/?view=git_credentials`).
The tab strip covers GitHub, GitLab, Anthropic, and TypeSafe. For each
provider it shows the registered entries in a dataframe, a
verify/delete row action, and a registration form. Tokens are
write-only: after `PUT` the raw value is never rendered again — the
verify button proves the token is still live without displaying it.

## Related

- [`user-acl.md`](user-acl.md) — the same `openspoke_admin` role gate
  covers this view.
