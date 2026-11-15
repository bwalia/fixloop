# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Deployment

#### Added
- Container image `ghcr.io/bwalia/fixloop` (distroless, non-root, amd64 and arm64), built by `.github/workflows/image.yml`: `edge` from `main`, semver tags from `v*` tags, with build provenance.
- Kubernetes manifests in `deploy/kubernetes/` (kustomize): restricted Pod Security, read-only root filesystem, probes, SQLite volume or Postgres via Secret, example Ingress.

#### Fixed
- `FIXLOOP_CONFIG` and `FIXLOOP_GITHUB_WEBHOOK_SECRET` were read as configuration overrides and made loading fail. Only `FIXLOOP_<SECTION>__<KEY>` variables are overrides now.

### Phase 2: Reviewer

#### Added
- **Reviewer agent** (`fixloop-agents`):
  - Diff parsing with new-file line numbers.
  - A versioned prompt that keeps all repository content in per-request data markers.
  - Structured findings through a `report_findings` tool.
  - Validation: findings must point at changed lines; confidence and severity thresholds; dedupe; at most 25 findings.
  - Model text is sanitised (no @-mentions, no `/fixloop` commands, no secrets).
  - Suggestions keep the original indentation.
- **Secret scanning** (`fixloop-core::secrets`): diffs are redacted before any model call, committed secrets become critical findings, and everything posted is scanned.
- **GitHub App** (`fixloop-scm`):
  - App registration in one click with `fixloop github create-app` (manifest flow, least-privilege permissions).
  - Installation tokens, PR, diff and file reads, check runs, inline reviews (falling back to a single review body if GitHub rejects a position), comments, and permission checks.
  - Webhook signature verification.
- **Store** (`fixloop-store`): PostgreSQL and SQLite through one code path; embedded migrations; a durable job queue with idempotent enqueue, leases, heartbeats, retries with backoff and crash recovery; review, finding and audit records.
- **Server** (`fixloop-server`): `POST /webhooks/github`, `/healthz`, `/readyz`, and workers that run reviews and record results.
- **CLI**:
  - `fixloop serve`.
  - `fixloop review`: local changes, or `--pr <url>` as a dry run, `--post` to publish. Exits 3 on high or critical findings.
  - `fixloop github create-app`.
- `.fixloop.yaml` and rules are read from the PR's **base** commit. `/fixloop review` only works for users with write access.
- `server.workers` config; `github` config section.
- Docs: GitHub setup guide; ADR-0006 (thin GitHub client).

#### Changed
- Minimum supported Rust version is now 1.94 (required by sqlx 0.9).

#### Security
- sqlx uses rustls with the system certificate store, which keeps CDLA-licensed `webpki-roots` out of the build.
- RUSTSEC-2026-0459 (bundled SQLite) is ignored with a recorded reason. The precondition (opening a database and `-wal` written by a less-trusted party) does not apply to Fixloop, and the fix needs a newer sqlx.

### Phase 1: LLM layer

#### Added
- `fixloop-llm`: `LlmProvider` trait with Anthropic, OpenAI and OpenAI-compatible providers (Ollama, vLLM, llama.cpp, LM Studio, NVIDIA NIM presets).
- Router with per-task routes, ordered fallback, retries with backoff, `local-only` data-egress filtering (never a silent downgrade), and per-job and daily token/cost budgets with all-or-nothing reservations.
- Tool calling: native where supported, and a JSON fallback (`tool_mode: json`) that validates replies and self-corrects once.
- Egress guard: model calls reach only configured endpoints, with no redirects. Offline mode rejects external providers and public AI hosts. Tests prove that no outside API is called.
- OpenTelemetry GenAI span attributes on every model call.
- CLI: `fixloop llm list` and `fixloop llm test [--model|--task] [--tool] [--local-only]`.
- `fixloop-eval`: eval case format, loader, scoring (found / false alarms / missed, precision, recall, fix pass rate) and three seed cases in `eval/cases/`.
- Example configs: `examples/fixloop-local.yaml` (offline) and `examples/fixloop-frontier.yaml`.

#### Changed
- Config: providers gain `preset` and `timeout_secs`; models gain `price` and `tool_mode`; new `llm.budgets` and `llm.retry` sections.
- cargo-deny checks the license graph for the targets we ship.

### Fixed
- Scorecard workflow referenced a non-existent `v2` tag of `ossf/scorecard-action`; it is now pinned to v2.5.0 by commit SHA.
- DCO check now skips commits authored by Dependabot, which cannot sign off.

### Phase 0: Skeleton

#### Added
- Cargo workspace with 14 crates (`fixloop-core`, `fixloop-cli` and placeholder crates for later phases) plus `xtask`.
- `fixloop-core`: server config schema (`fixloop.yaml` + `FIXLOOP_*` env overrides) with cross-field validation, per-repo `.fixloop.yaml` schema, domain types (severity, category, repo mode, data egress), and errors that carry hints.
- `fixloop` CLI: `init`, `config validate`, `config show`, `config validate-repo`. `serve`, `review` and `fix` are stubs that name their planned phase.
- Logging to stderr (pretty or JSON) with optional OTLP trace export.
- CI: fmt, clippy (`-D warnings`), tests on Linux and macOS, MSRV check, cargo-deny (CNCF license allowlist), cargo-audit, DCO sign-off check, OpenSSF Scorecard, Dependabot.
- Governance and community files: LICENSE (Apache-2.0), README, CONTRIBUTING, CODE_OF_CONDUCT (CNCF), GOVERNANCE, MAINTAINERS, SECURITY, ADOPTERS, ROADMAP.
- Docs: plan, ADRs 0001–0005, threat model v0, config reference, CNCF Sandbox application draft.
