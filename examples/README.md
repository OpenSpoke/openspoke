# examples/

End-to-end example configurations demonstrating a full OpenSpoke deployment.

Subdirectories:

- [`fleet-gitrepos/`](fleet-gitrepos/) — one Fleet `GitRepo` per
  deployable component (hub kernel, hub kernel frontend (v3.0), hub
  tunnel server, hub task worker, spoke MCP, spoke tunnel client),
  ready to copy into a Fleet server cluster and
  customise.
- [`application-spoke/`](application-spoke/) (v3.0) — the minimum
  manifests for an application spoke: a `Secret`, a `Deployment`
  (`tunnel-client` in HTTP-proxy mode), and a `Service`. Pair it
  with a `spoke_register(kind="application", ...)` on the hub.
- [`quickstart/`](quickstart/) — a walkthrough that stitches the
  above together into a single hub + single spoke lab.
