# fleet/manifests/managed/

Fleet-managed-by-Fleet resources. These manifests are reconciled onto the
Fleet server cluster by Fleet itself once the bootstrap phase is
complete.

Subdirectories:

- `ClusterRoleBindings/`, `ClusterRoles/` — Fleet UI RBAC
- `ConfigMaps/` — Fleet UI configuration
- `Deployments/` — Fleet UI (`fleet/ui/`) and any supporting services
- `GitRepos/` — user-authored GitRepos that select downstream clusters
  and paths under this repository. See
  [`examples/fleet-gitrepos/`](../../../examples/fleet-gitrepos/) for
  templates.
- `Secrets/` — Secrets referenced by the above. Create real values with
  `kubectl create secret ...` before applying — see
  [`docs/installation/fleet.md`](../../../docs/installation/fleet.md).
- `ServiceAccounts/`, `Services/` — Fleet UI supporting resources
