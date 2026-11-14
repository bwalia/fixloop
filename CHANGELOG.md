# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
