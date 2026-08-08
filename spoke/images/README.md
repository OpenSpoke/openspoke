# spoke/images/

Container image build contexts for spoke-side components.

## Contents

| Directory        | Produces                                              | Consumers                              |
|------------------|-------------------------------------------------------|----------------------------------------|
| `tunnel-client/` | `openspoke-tunnel-client:v0.1.0`                      | `spoke/kubernetes/template/tunnel-client/` |

## Build

Every `build.sh` reads the `REGISTRY` environment variable and defaults
to `ghcr.io/openspoke`.

```sh
export REGISTRY=ghcr.io/YOUR-ORG    # or your private registry
cd spoke/images/tunnel-client
./build.sh
```

Then update the `image:` field in the Deployment manifest under
`spoke/kubernetes/template/tunnel-client/` to reference your pushed tag
before applying.

## Notes

- The tunnel-client `proto/tunnel.proto` is kept byte-identical (modulo
  `go_package`) with the hub-side copy at
  `hub/images/tunnel-server/proto/tunnel.proto`. Regenerate both when the
  wire format changes.
