# spoke/

Components running on OpenSpoke spoke nodes.

- `kubernetes/` — spoke as a full Kubernetes cluster (RKE2 / K3s + Fleet agent)
- `native/` — spoke as a single Go binary (Windows / Linux / macOS, no k8s)
- `images/` — container image build contexts for spoke-side components
  (currently: `tunnel-client`, v2.0)
