# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
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
