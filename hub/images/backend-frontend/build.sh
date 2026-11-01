#!/bin/bash
# Docker image build & push script.
# Set REGISTRY to your container registry (e.g. ghcr.io/your-org).
# Run in a Docker-capable environment.

set -e

REGISTRY="${REGISTRY:-ghcr.io/openspoke}"
BACKEND_IMAGE="${REGISTRY}/rag-backend-gpu-offline:v1.0.3"
FRONTEND_IMAGE="${REGISTRY}/rag-frontend-offline:v1.0.3"

echo "========================================="
echo "RAG System - Offline Docker Images Build"
echo "========================================="
echo ""

# バックエンドイメージのビルド
echo "Step 1: Building backend image..."
echo "Image: ${BACKEND_IMAGE}"
echo ""
docker build -f Dockerfile.backend -t "${BACKEND_IMAGE}" .
echo "✅ Backend image built successfully"
echo ""

# フロントエンドイメージのビルド
echo "Step 2: Building frontend image..."
echo "Image: ${FRONTEND_IMAGE}"
echo ""
docker build -f Dockerfile.frontend -t "${FRONTEND_IMAGE}" .
echo "✅ Frontend image built successfully"
echo ""

# レジストリにプッシュ
echo "Step 3: Pushing images to registry..."
echo ""
docker push "${BACKEND_IMAGE}"
echo "✅ Backend image pushed"
echo ""
docker push "${FRONTEND_IMAGE}"
echo "✅ Frontend image pushed"
echo ""

echo "========================================="
echo "Build Complete!"
echo "========================================="
echo ""
echo "Images built:"
echo "  - ${BACKEND_IMAGE}"
echo "  - ${FRONTEND_IMAGE}"
echo ""
echo "Next steps:"
echo "1. Update Deployment manifests to use new images"
echo "2. Apply updated manifests:"
echo "   kubectl apply -f rag-backend-offline-v2.yaml"
echo "   kubectl apply -f rag-frontend-offline.yaml"
echo "3. Restart pods:"
echo "   kubectl delete pod -n rag-company1 -l app=rag-backend"
echo "   kubectl delete pod -n rag-company1 -l app=rag-frontend"
