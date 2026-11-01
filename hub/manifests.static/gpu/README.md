# NVIDIA GPU Device Plugin

Optional. Apply only if your hub nodes have NVIDIA GPUs and you want to
schedule GPU workloads (e.g., Ollama, guardrails, embedding models).

## Prerequisites

- NVIDIA driver installed on nodes
- NVIDIA Container Toolkit installed
- Container runtime (containerd / CRI-O) configured for the toolkit

## Customization

`nvidia-device-plugin-config.yaml` — adjust `replicas` under `timeSlicing`
to your desired GPU sharing factor:

- `1` = exclusive (one workload per GPU)
- `N` = share N ways (each pod sees the same physical GPU, arbitrated by
  time-slicing)

## Alternative

For fully automated GPU setup (driver, toolkit, plugin, monitoring),
consider the NVIDIA GPU Operator instead:
<https://github.com/NVIDIA/gpu-operator>
