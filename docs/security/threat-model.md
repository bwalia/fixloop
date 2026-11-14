# Fixloop threat model (v0)

Status: draft, written in Phase 0. It is revisited at the end of every phase. Method: STRIDE over the data-flow diagram in [docs/PLAN.md](../PLAN.md).

## Assets

| Asset | Why it matters |
|---|---|
| SCM credentials (GitHub App private key, installation tokens) | Write access to customer repositories |
| Tracker and chat credentials (Jira, Slack, …) | Data access, impersonation |
| LLM API keys | Cost, data egress |
| Customer source code | Confidentiality; some repos may only go to local models |
| Integrity of default branches | Supply-chain risk to everyone downstream |
| Audit log | Accountability, compliance |
| Fixloop's own release artifacts | Supply-chain risk to Fixloop users |

## Trust boundaries

1. **Internet → API**: webhooks, REST, MCP server, dashboard.
2. **Worker → LLM provider**: code leaves the deployment (unless the provider is local).
3. **Worker → sandbox**: untrusted repository code executes.
4. **Repo/issue/CI/MCP content → model context**: attacker-controllable text reaches the model.
5. **Worker → SCM/tracker**: Fixloop acts with write permissions.

## Threats and mitigations

| # | Threat (STRIDE) | Boundary | Mitigation | Phase |
|---|---|---|---|---|
| T1 | Forged webhooks trigger jobs (S) | 1 | HMAC signature verification; delivery-id dedupe; replay window | 2 |
| T2 | Prompt injection in PR/issue/code/CI logs/MCP output steers the agent (T, E) | 4 | ADR-0005: data/instruction separation, schema-only outputs, capability-scoped tools, policy gates in code, injection regression corpus | 1–5 |
| T3 | Unauthorised user triggers `/fixloop fix` (E) | 1 | Only users with write access may issue commands | 2 |
| T4 | Sandboxed code escapes or exfiltrates (I, E) | 3 | ADR-0003: non-root, read-only rootfs, dropped caps, seccomp, egress allow list via proxy, resource limits; no secrets inside the sandbox | 4 |
| T5 | Agent pushes to the default branch or merges (T) | 5 | Enforced in code, not prompt; branch protection recommended; PR-only changes | 2–4 |
| T6 | Secrets leak into LLM prompts, comments, logs (I) | 2, 5 | Secret scanning and redaction before LLM calls and before posting; log redaction layer; config holds env-var *names*, not secrets | 0–2 |
| T7 | Code sent to a disallowed model provider (I) | 2 | Per-repo `data_egress`; offline mode refuses external hosts at startup and at the HTTP-connector level; test asserts zero external calls | 0–1 |
| T8 | Cost exhaustion via large or many PRs (D) | 2 | Budgets per job, PR, repo, team and day; queue concurrency limits per repo | 1–2 |
| T9 | Token theft from the database (I) | 1 | Short-lived installation tokens minted per job; at-rest encryption for stored secrets; API tokens stored hashed (argon2) | 2, 8 |
| T10 | Tampering with the audit log (R) | — | Append-only, hash-chained rows; exportable | 8 |
| T11 | Malicious dependency or compromised release (T) | — | cargo-deny, cargo-audit, Dependabot, Scorecard, signed releases (cosign), SBOM, SLSA provenance | 0, 9 |
| T12 | Dashboard XSS/CSRF via finding text (T, E) | 1 | Model text rendered as escaped Markdown; CSP; SameSite cookies + CSRF tokens | 6 |
| T13 | Risky MCP tool actions (E) | 4, 5 | Per-tool allow/deny list and approval flag; tool output treated as data | 5 |

## Out of scope (for now)

- A compromised host running Fixloop.
- A malicious LLM provider (mitigation: choose local models via `data_egress: local-only`).
