# Roadmap

The detailed plan lives in [docs/PLAN.md](docs/PLAN.md). Progress is tracked in GitHub milestones, one per phase.

| Phase | Goal | Status |
|---|---|---|
| 0 | Skeleton: workspace, CI, license, CNCF files, config, logging, tracing | **In progress** |
| 1 | LLM layer: provider trait, Anthropic, OpenAI, OpenAI-compatible (Ollama/vLLM), routing, budgets | Planned |
| 2 | Reviewer: GitHub App, webhooks, inline review comments, check runs | Planned |
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
