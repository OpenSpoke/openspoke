# hub/images/

Container image build contexts. Each subdirectory holds the Dockerfile,
build.sh, and any accompanying source needed to produce one image used
by the Hub manifests.

## Contents

| Directory          | Produces                                              | Consumers                                                    |
|--------------------|-------------------------------------------------------|--------------------------------------------------------------|
| `backend-frontend/`| `rag-backend-gpu-offline:v1.0.3`, `rag-frontend-offline:v1.0.3` | `hub/manifests/kernel/`, `hub/manifests/kernel-vllm/`, `hub/manifests/kernel-frontend/` |
| `mcp/`             | `rag-mcp-server:latest`                               | `hub/manifests/mcp/`, `spoke/kubernetes/template/mcp/`       |
| `pdf-converter/`   | `pdf-converter:0.3.0`                                 | `hub/manifests/pdf-converter/`                               |
| `tunnel-server/`   | `openspoke-tunnel-server:v0.1.0`                      | `hub/manifests/tunnel-server/` (v2.0)                        |

## Build

Every `build.sh` reads the `REGISTRY` environment variable and defaults
to `ghcr.io/openspoke`. Set it to your own container registry before
running:

```sh
export REGISTRY=ghcr.io/YOUR-ORG    # or your private registry
cd hub/images/backend-frontend
./build.sh
```

Then update the corresponding `image:` field in the Deployment manifest
(under `hub/manifests/`) to reference your pushed tag before applying.

## Notes

- Kernel and MCP images ship as base + dependencies only. The actual
  Python code (`rag_backend_kernel.py`, `mcp_server.py`, etc.) is
  mounted from ConfigMaps at Pod start time, so rebuilding an image is
  only needed when Python or system dependencies change.
- The backend image expects PyTorch + IPEX to be pre-populated in a
  shared model-cache PV under `/model-cache/packages`. See
  [`docs/installation/hub.md`](../../docs/installation/hub.md) for the
  model-cache setup.
