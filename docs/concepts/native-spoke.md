# Native spokes

Version-first: **added in v3.0**.

A **native spoke** is a spoke that runs on the host operating system
with no Kubernetes at all. It is a single binary that opens the same
reverse tunnel a cluster spoke does, and — from the hub's point of
view — looks like just another spoke on the topology graph.

The initial and reference implementation is **`nsmt`**, the *Native
Spoke for Mac Terminal*: a small daemon a macOS operator installs on
their own laptop, or on a shared build machine, so that the hub can
reach that host's toolchain over the tunnel. `nsmt` is what makes
Xcode builds, iOS simulator runs, code signing and native
`security(1)` calls available to the hub kernel and to the MCP tools
that drive them.

## The `kind` field

Native spokes are `kind: native` in the `spoke_clusters` OpenSearch
index. See [`application-spoke.md`](application-spoke.md) for the
sibling values `cluster` and `application`.

The topology view renders native spokes as **hexagons**, so at a
glance a mixed environment shows circles for cluster spokes, hexagons
for native spokes, and diamonds for application spokes.

## The `SPOKE_ID` convention

Native spokes take the `nsmt-` prefix (for the Mac implementation) or,
more generally, a prefix that identifies the host runtime. This makes
`spoke_id` filtering in logs and usage records unambiguous.

## What the hub side ships

The hub kernel exposes `/core/nsmt/*` (from
`hub/manifests/kernel/ConfigMaps/rag-backend-kernel-native-spoke-mac.yaml`)
for the endpoints that only make sense against a native macOS host:
enumerating installed toolchains, kicking off builds, wiring up a
signed run. These endpoints follow the same reverse-tunnel pattern as
everything else — the hub calls `POST /core/nsmt/build` and the
tunnel-server routes it through to the correct `nsmt-*` spoke.

## Coexistence

A native spoke and a cluster spoke can share a host: nothing about
`nsmt` needs Kubernetes, and nothing about a cluster spoke prevents a
laptop-side daemon from also connecting. They register as separate
entries in `spoke_clusters` with distinct `kind` values.

## Related

- [`reverse-tunnel.md`](reverse-tunnel.md) — the underlying tunnel
  protocol shared by all three kinds of spoke.
- [`application-spoke.md`](application-spoke.md) — the `application`
  kind, for per-application sidecars.
