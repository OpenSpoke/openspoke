# .github/workflows/

GitHub Actions CI.

`ci.yaml` runs three jobs on every push and pull request against `main`:

- **yamllint** — runs against the repo with `.yamllint.yml` (rules
  relaxed to match the current repo state; catches syntax errors and
  gross formatting mistakes).
- **shellcheck** — lints every `*.sh` in the tree.
- **kubeconform** — validates every Kubernetes manifest against the k8s
  OpenAPI schema with `-strict -summary -ignore-missing-schemas`. CRDs
  (Fleet, custom) fall through the ignore flag.
