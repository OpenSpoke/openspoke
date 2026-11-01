# spoke/

Components running on OpenSpoke spoke nodes.

- `kubernetes/` — spoke as a full Kubernetes cluster (RKE2 / K3s + Fleet agent)
- `native/` — spoke as an OS-native daemon (v3.0+ hub receiver; no k8s)
- `images/` — container image build contexts for spoke-side components
  (currently: `tunnel-client`, v2.0)
