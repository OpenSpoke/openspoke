# kernel-vllm — RAG kernel variant for vLLM + Swallow (NVIDIA GPU)

This directory holds the alternative RAG kernel Deployment and
ConfigMaps for clusters that run **vLLM + Swallow** on **NVIDIA GPUs**
(e.g., the reference GPU-equipped clusters).

The default `hub/manifests/kernel/` variant targets **Ollama + Gemma**
on **Intel iGPU** (the reference `rag` cluster). Pick exactly one of
the two — do not apply both.

## Choosing a variant

| Variant                                       | LLM server | Model    | GPU vendor |
| --------------------------------------------- | ---------- | -------- | ---------- |
| `hub/manifests/kernel/` (default)             | Ollama     | Gemma    | Intel iGPU |
| `hub/manifests/kernel-vllm/` (this directory) | vLLM       | Swallow  | NVIDIA     |

Pair the kernel variant with the matching LLM server manifests under
`hub/manifests/rag-hub/`:

- Ollama variant  → `hub/manifests/rag-hub/rag/` (Ollama Deployment)
- vLLM variant    → `hub/manifests/rag-hub/rag-vllm/` (vLLM Deployment)

## Expected contents

Mirrors the layout of `hub/manifests/kernel/`:

```
kernel-vllm/
  fleet.yaml
  ConfigMaps/
    rag-backend-kernel.yaml
    rag-backend-kernel-client.yaml
    ...
  Deployments/
    rag-backend-kernel.yaml
  Services/
    rag-backend-kernel.yaml
  RBAC/
    kernel-observer.yaml
```

## Install

See [`docs/installation/hub.md`](../../../docs/installation/hub.md) for
the full install steps. When applying the manifests, substitute this
directory for `hub/manifests/kernel/`, and use `rag-hub/rag-vllm/`
instead of `rag-hub/rag/`.
