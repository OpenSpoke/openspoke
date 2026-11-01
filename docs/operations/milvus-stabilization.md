# Milvus stabilisation

The hub Milvus stack in v3.0 is Milvus 2.6.20 with three specific
changes on top of a stock deployment: a consolidated coordinator
topology, a durable Pulsar volume with a fixed BookKeeper identity,
and a highly-available etcd. These are not optional tuning knobs;
they are the shape the deployment needs to survive node reboots and
coordinator restarts without operator intervention.

This document lists what to check when standing up a new hub, and
what to do if you inherit an older configuration.

## Upgrade path

If you are coming from Milvus 2.4.x, do not upgrade in place to 2.6.
Go through a 2.4 -> 2.5 -> 2.6.20 stepped upgrade. Each hop:

1. Update the image tag on every coordinator and node Deployment.
2. Wait for pods to become Ready.
3. Confirm etcd contains sane channel checkpoints before moving on.

Skipping 2.5 has left channel checkpoints in an inconsistent state
in the past. It is faster to do the intermediate hop than to recover
from a broken 2.4 -> 2.6 jump.

## Coordinator topology

Milvus 2.6 uses a single `mixCoord` process that fuses the previously
separate coordinators. The v3.0 hub manifest ships this consolidated
form. If you are inheriting a deployment with separate
`rootcoord` / `datacoord` / `querycoord` / `indexcoord` Deployments,
delete them and let Fleet reconcile the new `mixCoord`.

The one storage env var that matters is `COMMON_STORAGETYPE`. It must
be spelled exactly `COMMON_STORAGETYPE=remote` — no underscore between
`STORAGE` and `TYPE`, and `remote`, not `minio`, as the value. The
older `COMMON_STORAGE_TYPE=minio` spelling will silently start
`querynode` and `streamingnode` without a working segcore filesystem;
the failure mode looks like partial reads for weeks before it becomes
obvious.

## Pulsar durability

The bundled Pulsar needs three things to survive pod recreation.
Missing any one of them will produce a "stale checkpoint" that pins
reads at 505 until you intervene.

**A real PVC for `/pulsar/data`.** Not `emptyDir`. `emptyDir` seems
to work for weeks and then loses data on the first pod restart, and
the resulting corrupt checkpoints are not easy to unwind.

**A fixed BookKeeper identity via `standalone.conf`.** Inject
`bookieId=pulsar.milvus.svc.cluster.local:3181` (or the equivalent for
your namespace and Service name). Without a fixed identity, BookKeeper
picks a new one on each restart and the ledger becomes unrecoverable.

**Port `3181` on the Service.** BookKeeper listens on 3181; the
Service must expose it or the fixed identity cannot be resolved from
inside the pod.

All three are set together in the v3.0 hub Milvus bundle. If you are
copying from an older version, verify all three exist — one of them
alone is worse than none, because it makes the failure mode harder to
recognise.

## etcd HA

The v3.0 hub ships etcd as a 3-replica StatefulSet with
`auto-compaction-mode=periodic` and `auto-compaction-retention=1h`.

The 3-replica arrangement lets a single etcd pod restart without
losing quorum. Without it, a single node reboot could take Milvus
down. The auto-compaction is what keeps the etcd data volume from
growing without bound; the failure mode without it is a slow, silent
disk fill that eventually stops the whole hub.

If you are inheriting a single-replica etcd, migrate before you take
your first node reboot in anger. Backup, scale down Milvus, replace
the etcd manifest with the 3-replica StatefulSet, restore etcd data,
scale Milvus back up.

## What to check on a new hub

After Fleet applies the Milvus bundle, before you consider the hub
"ready":

1. `kubectl -n milvus get pods` shows `mixCoord`, `querynode`,
   `datanode`, `indexnode`, `proxy`, `pulsar`, three `etcd-*`, and
   `minio` as Ready.
2. The etcd pods form a quorum (`etcdctl endpoint status --cluster`).
3. `kubectl -n milvus exec statefulset/etcd -- etcdctl endpoint
   status` shows `raft term` and `raft index` consistent across
   replicas.
4. Milvus `describe collection` on a small test collection returns
   without error.
5. A round-trip vector query on the test collection returns results.

If step 4 or 5 fails, do not paper over it with a pod restart. Check
the coordinator logs for channel checkpoint messages; a stale
checkpoint at this stage is a Pulsar problem and needs the three-
point fix above.

## Related documents

- [`docs/installation/hub.md`](../installation/hub.md) — the hub
  install path that applies the Milvus bundle.
- [`docs/operations/knowledge-base-migration.md`](knowledge-base-migration.md) —
  moving `_handoff_*` rows out of Milvus into Memgraph. Does not touch
  the general RAG content; Milvus continues to serve that.
