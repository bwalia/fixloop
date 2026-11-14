# CNCF Sandbox application (living draft)

Draft answers for the application form in the [`cncf/sandbox`](https://github.com/cncf/sandbox) repository. Update this file as the project changes, and check the current form fields before submitting, because the form may change.

## Project name
Fixloop

## Project summary (one line)
An open source AI agent that reviews pull requests, files deduplicated issues, and fixes bugs in a sandbox with test-gated pull requests, using frontier or fully local models.

## Project description
Fixloop closes the bug loop on a codebase: Review → Find → File → Fix → Check → Close. It reviews PRs with full repository context, files findings in the team's tracker without duplicates, fixes problems inside an isolated sandbox, and opens a fix PR only when build, tests, lint and a second review all pass. It is written in Rust, runs as a single binary or scaled out on Kubernetes (Helm chart and operator with CRDs), and is observable through OpenTelemetry and Prometheus. It never pushes to the default branch and never merges on its own.

## Org repo URL
https://github.com/bwalia/fixloop *(to be moved to a dedicated project organisation before applying)*

## Website
TBD (GitHub Pages, Phase 10)

## Roadmap
[ROADMAP.md](../../ROADMAP.md)

## Code of conduct
CNCF Code of Conduct, see [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md).

## Adopters
[ADOPTERS.md](../../ADOPTERS.md) (none yet)

## Contributing guide
[CONTRIBUTING.md](../../CONTRIBUTING.md)

## Security policy
[SECURITY.md](../../SECURITY.md)

## Maintainers file
[MAINTAINERS.md](../../MAINTAINERS.md)

## License
Apache-2.0. Dependency licenses are checked against the CNCF allowlist in CI (`cargo deny`).

## Why CNCF?
- Fixloop is cloud native by design: Kubernetes Job sandboxes, an operator for GitOps management, OpenTelemetry-first observability, Prometheus metrics.
- It is vendor neutral: every model provider, code host, tracker and sandbox sits behind an open trait, and it runs fully offline with local models.
- A neutral home helps the companies that need AI code tooling, but cannot adopt a single-vendor product, contribute together.

## Benefit to the landscape
TBD. Candidate categories: "Application Definition & Development → Continuous Integration & Delivery" or a security/compliance category. Compare with other landscape projects and explain the gap (an AI bug loop that is self-hostable, vendor neutral and sandboxed).

## Cloud native fit
Kubernetes operator and CRDs (`FixloopRepository`, `FixloopPolicy`, `FixloopModelProvider`), Helm chart, durable Postgres-backed queue with horizontally scalable workers, OTLP traces and metrics, distroless multi-arch signed images.

## Cloud native integrations and leverage
Kubernetes, OpenTelemetry, Prometheus, Helm, Argo CD/Flux (via CRDs), Sigstore (cosign), and optional sandbox runtimes.

## Similar projects
TBD.

## Business product or service to project separation
There is no commercial product. All features, including SSO, RBAC and audit, are in the open source core.

## Checklist (from build plan §13)
- [x] Apache-2.0 license
- [x] DCO sign-off enforced in CI
- [x] LICENSE, README, CONTRIBUTING, CODE_OF_CONDUCT, GOVERNANCE, MAINTAINERS, SECURITY, ADOPTERS, ROADMAP, CHANGELOG
- [x] cargo-deny license allowlist check in CI
- [x] OpenSSF Scorecard workflow
- [ ] OpenSSF Best Practices badge
- [ ] SBOM per release
- [ ] Signed images and binaries (cosign)
- [ ] SLSA build provenance
- [ ] Project website
- [ ] GitHub Discussions enabled, good-first-issue labels
- [ ] Regular semver releases
- [ ] Maintainers from more than one organisation (strengthens the application)
