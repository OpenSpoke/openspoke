# rag-vllm — vLLM + Swallow LLM server (NVIDIA GPU)

This directory holds the alternative LLM server Deployment
(**vLLM + Swallow** on **NVIDIA GPUs**) that pairs with the
`hub/manifests/kernel-vllm/` kernel variant.

The default `hub/manifests/rag-hub/rag/` variant runs **Ollama + Gemma**
on **Intel iGPU** and pairs with `hub/manifests/kernel/`. Pick exactly
one pairing — do not apply both.

## Pairing

| Kernel variant                | LLM server variant                  |
| ----------------------------- | ----------------------------------- |
| `hub/manifests/kernel/`       | `hub/manifests/rag-hub/rag/`        |
| `hub/manifests/kernel-vllm/`  | `hub/manifests/rag-hub/rag-vllm/`   |

## Expected contents

Mirrors the layout of `hub/manifests/rag-hub/rag/`:

```
rag-vllm/
  Deployments/
    vllm.yaml
  Services/
    vllm.yaml
  ConfigMaps/          # if the vLLM startup args / model config is
    vllm-config.yaml   # externalized (optional)
```

## Install

See [`docs/installation/hub.md`](../../../../docs/installation/hub.md)
for the full install steps.
