FROM python:3.11-slim

WORKDIR /app

RUN pip install --no-cache-dir \
    "mcp[cli]>=1.0.0" \
    "fastmcp>=0.1.0" \
    httpx \
    uvicorn \
    "PyJWT[crypto]>=2.8.0" \
    kubernetes

# コードは ConfigMap から mount するので COPY は不要だが、
# ローカルビルドテスト用に残す
COPY mcp_server.py .

CMD ["python", "mcp_server.py"]
