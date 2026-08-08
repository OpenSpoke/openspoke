#!/bin/bash

set -e

REGISTRY="${REGISTRY:-ghcr.io/openspoke}"
MCP_SERVER_IMAGE="${REGISTRY}/rag-mcp-server:latest"

docker build -f Dockerfile -t "${MCP_SERVER_IMAGE}" .
docker push "${MCP_SERVER_IMAGE}"
