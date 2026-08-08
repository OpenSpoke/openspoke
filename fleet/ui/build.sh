#!/bin/bash
set -euo pipefail

VERSION="${1:-0.1.0}"
REGISTRY="${REGISTRY:-ghcr.io/openspoke}"
IMAGE="${REGISTRY}/fleet-ui:${VERSION}"

echo "==> Building ${IMAGE}"
docker build -t "${IMAGE}" .

echo "==> Pushing ${IMAGE}"
docker push "${IMAGE}"

echo "==> Done: ${IMAGE}"
