#!/usr/bin/env bash
set -euo pipefail

# Build tunnel-server and push to the configured registry.
# Usage:
#   ./build.sh            # tag = current UTC yymmdd-HHMMSS
#   ./build.sh v0.1.0     # explicit tag
#
# Set REGISTRY to your own registry (default: ghcr.io/openspoke).

cd "$(dirname "$0")"

REGISTRY="${REGISTRY:-ghcr.io/openspoke}"
TAG="${1:-$(date -u +%y%m%d-%H%M%S)}"
IMG="${REGISTRY}/openspoke-tunnel-server:${TAG}"

echo "building ${IMG}"
docker build -t "${IMG}" .

echo "pushing ${IMG}"
docker push "${IMG}"

echo "done: ${IMG}"
