#!/bin/bash

set -e

REGISTRY="${REGISTRY:-ghcr.io/openspoke}"

# build
docker build -t "${REGISTRY}"/pdf-converter:0.3.0 .
docker tag "${REGISTRY}"/pdf-converter:0.3.0 "${REGISTRY}"/pdf-converter:latest

# push
docker push "${REGISTRY}"/pdf-converter:0.3.0
docker push "${REGISTRY}"/pdf-converter:latest
