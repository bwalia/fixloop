# Roadmap

The detailed plan lives in [docs/PLAN.md](docs/PLAN.md). Progress is tracked in GitHub milestones, one per phase.

| Phase | Goal | Status |
|---|---|---|
| 0 | Skeleton: workspace, CI, license, CNCF files, config, logging, tracing | Done |
| 1 | LLM layer: provider trait, Anthropic, OpenAI, OpenAI-compatible (Ollama/vLLM), routing, budgets | Done |
| 2 | Reviewer: GitHub App, webhooks, inline review comments, check runs | Done |
| 3 | Issue filing: GitHub Issues + Jira, dedupe, two-way links | Planned |
| 4 | Sandbox + Fixer: Docker and Kubernetes runners, test-gated fix PRs | Planned |
| 4b | OpenShell runner (optional) | Planned |
| 5 | MCP client and server | Planned |
| 6 | Dashboard | Planned |
| 7 | Observability: OpenTelemetry, Prometheus, Grafana | Planned |
| 8 | Enterprise: SSO, RBAC, audit log, budgets, reports | Planned |
| 9 | Cloud native: Helm, operator, CRDs, signed multi-arch images, SBOM | Planned |
| 10 | CNCF Sandbox application | Planned |

Adapters beyond GitHub and Jira (GitLab, Bitbucket, Gitea, Linear, Bugzilla, Azure Boards, Teams, Discord, Jenkins, Buildkite) are open for contribution once the traits stabilise in Phases 2–3.

## Follow-ups carried forward

- **GitHub Action** (review without hosting anything): needs published release binaries, so it ships with Phase 9.
- **Tree-sitter context** (enclosing symbols, callers, related tests): Phase 2 sends the diff plus the full changed files. Symbol-level context arrives with the Fixer (Phase 4), where it matters most.
- **Two-pass review** (cheap triage, then strong review) for very large PRs: the `triage` route exists; the second pass will be added when large-PR evals show the need.
- **sqlx upgrade** to a release that allows `libsqlite3-sys >= 0.39`, then drop the RUSTSEC-2026-0459 exception.
