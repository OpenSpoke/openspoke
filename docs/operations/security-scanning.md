# Vulnerability scanning

Version-first: **added in v3.0**.

The v3.0 hub bundles a small vulnerability-scanning framework that
covers two axes:

- **OSS dependency scanning** for the container images the OpenSpoke
  workloads themselves run
- **Mobile-app scanning** for iOS and Android bundles that operators
  build and ship

The framework is split into an on-hub endpoint set plus a set of
scanner Jobs. The hub side ships as
`hub/manifests/kernel/ConfigMaps/rag-backend-kernel-security.yaml`
and exposes `/core/vuln/*`; the scanners are Kubernetes Jobs (and one
CronJob) that call those endpoints.

## OpenSearch indexes

- `vuln_events` — one document per scan event (which target, which
  scanner, which run, exit code, timestamps, links back to the
  scanner logs).
- `vuln_findings_mobile` — the per-app finding rows. Every finding
  carries the target it belongs to and the run it was found in, so
  the UI can compute a diff against the previous run.

Findings are never overwritten in place; a re-scan writes new rows
tagged with the new run id.

## The scanner image

The default scanner image is
`ghcr.io/openspoke/openspoke-vuln-scanner:0.1.0`. Override the
registry through the CronJob environment before applying:

```yaml
env:
  - name: REGISTRY
    value: registry.example.com
```

or pass `--set image.registry=registry.example.com` if you deploy
through Helm.

## Rulesets

The mobile scanner combines three input sources:

- **OSV** — for OSS dependencies declared in `Podfile.lock`,
  `Package.resolved`, and Gradle manifests.
- **OWASP MASVS** — for platform-specific code and manifest checks
  (permissions, backup, deep links, WebView configuration, and so
  on).
- **iOS hardening** — a small set of App Store-oriented checks for
  App Transport Security, URL schemes, and code-signing entitlements.

Custom rules can be added under `rulesets/` in the scanner image;
each ruleset is a JSON file that the scanner discovers at startup.

## Scheduling

The default schedule is a weekly CronJob on Monday at 12:30 JST.
Individual runs can be kicked off manually with
`kubectl create job --from=cronjob/mobile-scan`; a `DRY_RUN=true`
environment variable makes the scanner walk every target and print
findings without writing to the `POST` endpoint, which is the
recommended way to test rule changes.

## Reading the UI

The **Security** dashboard view opens on **Current findings** — the
latest row per app per scan, with a badge marking whether each
finding is new, unchanged, or resolved since the previous run.
Details for a given finding expand inline, showing the file / line
(source scans), the affected package (dependency scans), or the
manifest key (mobile checks), along with the rule id and a link to
its documentation. A second tab shows every past `vuln_events` row
in reverse chronological order.

Only admins can trigger scans from the UI; every authenticated user
can read findings.

## Related

- [`../concepts/reverse-tunnel.md`](../concepts/reverse-tunnel.md) —
  the transport the scanner uses for binaries that live on a native
  spoke (macOS build hosts).
- [`git-credentials.md`](git-credentials.md) — the credentials store
  the scanner uses to clone private repositories without a
  Kubernetes `Secret`.
