# Fixloop — Architecture & Delivery Plan

Status: **DRAFT for review** · Date: 2026-11-14 · Owner: lead engineer

Fixloop closes the bug loop on a codebase: **Review → Find → File → Fix → Check → Close**.
This document is the plan of record. Major decisions are in [`docs/adr/`](adr/).

---

## 1. Principles

1. **One binary, many roles.** `fixloop serve --role=all|api|worker|operator`. Small teams run one process with SQLite or Postgres; large teams split API and workers and scale workers out.
2. **Traits at every vendor seam.** `LlmProvider`, `ScmProvider`, `Tracker`, `SandboxRunner`, `Notifier`, `CiSource`. Core logic never imports a vendor SDK.
3. **Untrusted by default.** Diffs, issue bodies, comments, CI logs and MCP tool output are *data* and are never treated as instructions (ADR-0005).
4. **Nothing reaches the default branch.** Every change goes through a PR. Fixloop never merges on its own.
5. **Precision over recall.** Findings carry a confidence score. Only findings above the repo threshold are posted.
6. **Works without optional parts.** No OpenShell, no frontier API, no Kubernetes: Docker plus a local model still runs the full loop.
7. **Vendor neutral and Apache-2.0 everywhere.** No closed enterprise fork.

---

## 2. Architecture

```
                 ┌──────────────── Inbound ────────────────┐
 GitHub App ─┐   │ webhooks (HMAC verified) · REST API      │
 GitLab ─────┤   │ MCP server (stdio / streamable HTTP)     │   CLI (fixloop review/fix)
 Sentry ─────┼──▶│ generic bug webhook · scheduler (cron)   │◀── runs the same engine
 Jira/Linear ┘   └───────────────┬──────────────────────────┘    in-process
                                 │ enqueue (tx-safe)
                                 ▼
               ┌───────────────────────────────────┐
               │  Durable job queue (Postgres/SQLite)│  ADR-0002
               │  jobs · steps · leases · retries   │
               └───────────────┬───────────────────┘
                               │ lease
              ┌────────────────▼──────────────────────────┐
              │            Worker (fixloop-agents)          │
              │  Reviewer · Fixer · Bug Fixer state machines│
              │  ┌─────────┐ ┌──────────┐ ┌──────────────┐ │
              │  │ Context │ │ Policy & │ │ Secret scan /│ │
              │  │ builder │ │ budgets  │ │ redaction    │ │
              │  │(gix+ts) │ │          │ │              │ │
              │  └─────────┘ └──────────┘ └──────────────┘ │
              └──┬──────────┬───────────┬───────────┬──────┘
                 │          │           │           │
      ┌──────────▼─┐ ┌──────▼─────┐ ┌───▼────────┐ ┌▼────────────┐
      │ fixloop-llm│ │fixloop-scm │ │fixloop-    │ │fixloop-     │
      │ router,    │ │GitHub,     │ │trackers    │ │sandbox      │
      │ fallback,  │ │GitLab, ... │ │GH Issues,  │ │Docker/Podman│
      │ budgets    │ │            │ │Jira, ...   │ │K8s Job      │
      └─┬────────┬─┘ └────────────┘ └────────────┘ │Local (dev)  │
        │        │                                  │OpenShell*   │
   Frontier   OpenAI-compatible                     └─────────────┘
   (Anthropic, (Ollama, vLLM,                         * feature flag
   OpenAI, ...) llama.cpp, NIM)

 Cross-cutting: OpenTelemetry (traces/metrics/logs, GenAI semconv) · Prometheus /metrics
                audit log · RBAC/SSO · Leptos dashboard (same binary) · MCP client registry
```

### Process roles
| Role | Contains | Scales |
|---|---|---|
| `api` | axum HTTP: REST, webhooks, dashboard assets, MCP server (HTTP), `/metrics`, `/healthz`, `/readyz` | horizontally, stateless |
| `worker` | job leasing, agents, sandbox orchestration, outbound SCM/tracker calls | horizontally; per-worker concurrency |
| `scheduler` | cron triggers (backlog sweeps, tracker sync, eval runs). Leader election via Postgres advisory lock | one active |
| `operator` | kube-rs controller reconciling CRDs into DB config | one active (lease) |
| `all` | all of the above in one process | single node |

---

## 3. Workspace / crates

| Crate | Responsibility | Key deps |
|---|---|---|
| `fixloop-core` | Domain types (Finding, Issue, FixAttempt, Job, Policy, Repo, Budget), error types, the traits (`ScmProvider`, `Tracker`, `SandboxRunner`, `Notifier`, `CiSource`), config schema, redaction, secret scanning | serde, thiserror, figment, schemars |
| `fixloop-store` *(added)* | sqlx repositories, migrations for **both** Postgres and SQLite, durable queue | sqlx |
| `fixloop-llm` | `LlmProvider` trait, providers, router, fallback chain, budget meter, tool-call ↔ JSON-mode shim, offline guard | reqwest, rustls |
| `fixloop-agents` | Reviewer / Fixer / Bug Fixer state machines, prompts (versioned templates), context builder, dedupe, confidence scoring, rule-file loader | tree-sitter, gix |
| `fixloop-sandbox` | `SandboxRunner` impls: Docker/Podman (bollard), K8s Job (kube), Local, OpenShell (`feature = "openshell"`) | bollard, kube |
| `fixloop-scm` | GitHub (App + Action), GitLab, Bitbucket, Gitea/Forgejo; CI log readers | octocrab, jsonwebtoken |
| `fixloop-trackers` | GitHub Issues, Jira (Cloud + DC), Linear, Bugzilla, Azure Boards, Sentry, generic webhook; two-way sync | reqwest |
| `fixloop-notify` *(added)* | Slack, Teams, Discord, generic webhook | reqwest |
| `fixloop-mcp` | MCP client registry (allow/deny/approval per tool) + MCP server exposing Fixloop tools | rmcp |
| `fixloop-server` | axum app, webhooks, REST (OpenAPI via utoipa), auth (OIDC/SAML/API tokens), RBAC, scheduler, telemetry init | axum, tower, utoipa, openidconnect |
| `fixloop-dashboard` | Leptos CSR app, compiled to WASM, embedded into server binary | leptos |
| `fixloop-cli` | `fixloop` binary (all subcommands; `serve` pulls in server) | clap |
| `fixloop-operator` | CRDs + reconciler | kube, k8s-openapi |
| `fixloop-eval` *(added)* | Eval harness: seeded-bug corpus, scoring, reports | — |
| `xtask` | dev tasks: codegen of CRD YAML, OpenAPI spec, embed dashboard, release helpers | — |

Changes from the suggested layout: `store`, `notify`, `eval` and `xtask` are split out so that core stays free of I/O and the CLI doesn't link Slack code just to run a review. These are small changes and can be rolled back if you'd prefer the original list.

Crate versions checked 2026-11-14: tokio 1.54, axum 0.8.9, sqlx 0.9.0, clap 4.6, octocrab 0.54, gix 0.90, tree-sitter 0.27, rmcp 3.10, kube 5.1, tracing-opentelemetry 0.34 / opentelemetry-otlp 0.33. Leptos stable is 0.8.x; 0.9 is still RC. We pin to stable (see ADR-0001).

---

## 4. Data model (core tables)

```
org ─< team ─< membership >─ user            (RBAC: admin | maintainer | viewer)
org ─< repo ── repo_policy (mode, thresholds, model routing, data-egress class, budgets)
repo ─< pull_request ─< review ─< finding
finding  (id, repo, fingerprint, title, severity, category, path, line_start/end,
          rationale, suggested_fix, confidence, state: open|accepted|dismissed|fixed,
          source: pr_review|scan|import, embedding_ref)
finding ─< finding_feedback (dismiss reason, user)   → noise learning
issue    (id, tracker, external_id, url, state, dedupe_group_id, last_synced_at)
finding >─< issue (link)
dedupe_group (id, canonical_issue_id, method: exact|fingerprint|semantic, score)
job      (id, kind: review|fix|bugfix|sync|scan|eval, state: queued|leased|running|
          waiting_approval|succeeded|failed|cancelled, priority, attempts, lease_until,
          worker_id, trace_id, budget_tokens, budget_cost, payload jsonb)
job_step (job, seq, name, status, started/ended, tokens_in/out, cost, log_ref)
fix_attempt (job, try_no, branch, build/test/lint results, second_review verdict, diff_ref)
sandbox_event (job, runner, kind: policy_block|oom|timeout, detail)
approval (id, kind: fix_pr|policy_change|mcp_action, subject, requested_by, decided_by, state)
llm_call (job, provider, model, task, tokens, cost, latency, outcome)   → cost reports
provider_config, mcp_server_config, api_token, audit_event (append-only, hash-chained)
```

* **Fingerprint** = hash of (rule/category, normalised path, normalised snippet AST node). This gives a stable identity across line shifts.
* **Semantic dedupe**: embeddings are stored in Postgres with **pgvector** when available. Otherwise (SQLite, or no embedding model configured) we fall back to brute-force cosine in process. The scope is per-repo and small, so this stays fast.
* Migrations live in two sqlx migration sets (pg/sqlite). CI runs the full test suite against both.

---

## 5. Main flows

### 5.1 Review flow (Reviewer)
1. `pull_request.opened|synchronize` webhook (or `/fixloop review`) → verify HMAC → dedupe delivery id → enqueue `review` job (same tx as audit event).
2. Worker leases the job, mints a short-lived GitHub App installation token and creates a **check run (in_progress)**.
3. Context builder: fetch diff, then a shallow/partial clone via gix. Tree-sitter extracts the enclosing symbols, callers/callees and related tests. Load `.fixloop.yaml` and `.fixloop/rules/*.md`.
4. Policy gate: decide which model may see this repo (data-egress class), apply the budget reservation, and run the secret scan with redaction **before** any LLM call.
5. Pass 1 (cheap model): triage the files and pick the hunks worth deep review. Pass 2 (strong model): produce structured findings (JSON schema).
6. Post-process: verify each file/line exists in the diff, then score confidence (model score × heuristics × learned dismissal penalty). Drop findings below the threshold and dedupe against existing findings and open review comments.
7. Publish: inline comments (+ `suggestion` blocks in `suggest` mode or higher), a summary comment, and the check run conclusion (pass/warn/fail per policy).
8. Optional: high-severity findings in existing code (not introduced by the PR) → **File flow**.

### 5.2 File flow (issue filing + dedupe)
1. For each candidate: exact fingerprint match → semantic match (embedding cosine ≥ τ) → LLM tie-break on the top-k candidates (cheap model).
2. Match found: link it, add a comment if new evidence exists, and reopen if it was closed but has regressed. No match: create the issue with labels and a backlink, and store the link.
3. Two-way sync: tracker webhooks (or polling) update state, labels and comments. A Fixloop-originated change carries a marker so it isn't echoed back.

### 5.3 Fix flow (Fixer on a PR / Bug Fixer on an issue)
1. Trigger: `/fixloop fix`, a labelled issue, a Sentry alert, a schedule, or a manual run. **Mode check**: `comment-only` and `suggest` stop here (suggestions only). `open-fix-PR` / `auto-push` continue. If "ask first" is set, create an `approval` and wait.
2. Prepare the sandbox (runner chosen by policy): repo workspace + tmp, network allow list, **no long-lived credentials**. Git pushes happen *outside* the sandbox from the worker, using a scoped token.
3. Agent loop (tool calls: read_file, search, edit, run_tests, run_build): reproduce first (write or locate a failing test), then patch.
4. **Gate**: build + tests + lint pass in the sandbox **and** a second-model review approves. Up to N tries (default 3). On exhaustion: write up what was tried and post it to the issue/PR.
5. Secret scan the diff → push the branch `fixloop/fix-<issue>-<short>` → open the PR (or push to the PR branch in `auto-push` mode) linked to the issue.
6. **Close**: the `pull_request.closed && merged` webhook → close the linked issue(s) via the tracker and record MTTR plus the outcome.

### 5.4 Bug-import flow
1. Connect a tracker → paged import of open bugs matching a filter → normalise them into `issue`.
2. Dedupe across trackers into `dedupe_group`s, and score fixability (cheap model: is there a repro, a stack trace, a referenced file?).
3. Queue Bug Fixer jobs within budget, ordered by severity × fixability. Results are synced back two-way.

---

## 6. Cross-cutting design

* **LLM layer** — ADR-0004. Config-defined providers and models, per-task routing, fallback chain, native tool calls with a JSON-mode fallback, and a budget meter that is checked before and reconciled after every call. **Offline guard**: when `offline: true`, the HTTP client is built with an egress allow list containing only configured local endpoints. A test asserts that no other host is ever dialled, using a wiremock-style catch-all that fails the test.
* **Sandbox** — ADR-0003.
* **Prompt-injection defence** — ADR-0005. Fixed system prompts; untrusted content goes in delimited, labelled data blocks; output is schema-validated; tools are capability-scoped per agent; there is a regression corpus of injection attempts.
* **Observability** — `tracing` → OTel (OTLP). One root span per job, propagated via `traceparent` stored on the job row so that webhook → worker stays one trace. GenAI semconv attributes go on LLM spans. Prometheus via `metrics-exporter-prometheus`. Logs are JSON, and a redaction layer runs on every field.
* **Auth** — OIDC (openidconnect crate) first. SAML via a maintained crate, or via an IdP→OIDC bridge if no Rust SAML crate meets the license and maintenance bar (to verify in Phase 8). API tokens are hashed with argon2.
* **Audit log** — append-only, each row hash-chained to the previous one. Export as JSONL/CSV. Retention is configurable.
* **Config precedence** — built-in defaults < org policy repo (git) < `FixloopPolicy` CRD/DB < repo `.fixloop.yaml` (only *narrowing* overrides allowed for security-relevant keys) < env vars.

---

## 7. Phases

Each phase is a series of small PRs and ends with green CI, tests and updated docs/CHANGELOG.

| # | Phase | Main deliverables | Exit criterion |
|---|---|---|---|
| 0 | Skeleton | Workspace, all crates stubbed, CI (fmt, clippy -D warnings, test, deny, audit, DCO), CNCF governance files, config loader, tracing init, `fixloop --help` | CI green; `fixloop --help` works |
| 1 | LLM layer | Trait, Anthropic, OpenAI, OpenAI-compatible (Ollama/vLLM/llama.cpp/LM Studio/NIM presets), router, fallback, budgets, offline guard + test, eval harness skeleton | Same test prompt passes on a frontier and a local model |
| 2 | Reviewer | Store + queue, GitHub App (manifest flow), webhooks, context builder, review pipeline, check runs, `fixloop review` local, GitHub Action | Reviews a real PR on a test repo |
| 3 | Issue filing | GitHub Issues + Jira trackers, fingerprint + semantic dedupe, two-way links/sync | Findings become issues, no duplicates |
| 4 | Sandbox + Fixer | Docker/Podman + K8s Job + Local runners, Fixer + Bug Fixer, test gate, second review, retries + write-up, Close-on-merge | Seeded bug fixed end-to-end, no OpenShell |
| 4b | OpenShell (optional) | Runner behind `feature = "openshell"`, policies in `deploy/openshell/`, inference routing, blocked-action → approval flow | Same seeded bug fixed inside OpenShell |
| 5 | MCP | Client registry with allow/deny/approval; server tools `review_pr`, `list_findings`, `file_issue`, `fix_issue`, `get_job_status`, `explain_finding` | Claude Code calls `review_pr` |
| 6 | Dashboard | All pages in spec §7, dark mode, shortcuts, setup wizard | New user sets up a repo from UI only |
| 7 | Observability | Full trace propagation, metrics, Grafana dashboards + alerts, vendor docs | One trace shows a whole job |
| 8 | Enterprise | SSO (OIDC/SAML), RBAC, teams/multi-tenant, audit export, budgets/reports (CSV/PDF), policy-as-code repo | Admin restricts a repo to local models only |
| 9 | Cloud native | Distroless multi-arch image, Helm, operator + CRDs, cosign, SBOM, SLSA provenance | `helm install` on kind works; release signed |
| 10 | CNCF prep | Scorecard, Best Practices badge, docs site, sandbox application draft | §13 checklist complete |

Remaining adapters (GitLab, Bitbucket, Gitea, Linear, Bugzilla, Azure Boards, Teams/Discord, Jenkins/Buildkite) are scheduled as parallel "adapter" PRs from Phase 3 onward. They are good-first-issue material once the traits stabilise.

### Phase 0 PR breakdown (next up after approval)
1. Workspace + crate stubs + `rust-toolchain.toml` + `deny.toml` + lints in `Cargo.toml` (`unwrap_used = "deny"` outside tests).
2. CI: fmt, clippy, test (pg + sqlite matrix), cargo-deny, cargo-audit, DCO check, Scorecard.
3. Governance files: LICENSE, README, CONTRIBUTING, CODE_OF_CONDUCT, GOVERNANCE, MAINTAINERS, SECURITY, ADOPTERS, ROADMAP, CHANGELOG, `docs/cncf/sandbox-application.md` stub, `docs/security/threat-model.md` v0.
4. `fixloop-core` config schema (figment, YAML + env) + `fixloop-cli` with `--help`, `init`, `config validate`; tracing/OTel init.

---

## 8. Testing strategy

* **Unit**: core logic (scoring, fingerprinting, routing, budget math, redaction), using property tests where useful (proptest).
* **Adapter integration**: recorded HTTP fixtures (wiremock) per SCM/tracker/LLM provider. A nightly optional live job runs against sandbox accounts.
* **E2E**: `examples/seeded-bugs/` (Rust, Python, TS mini-repos with known bugs) driven through the full loop. A mock SCM is used on every PR. A real GitHub test org is used nightly.
* **Security**: prompt-injection corpus, secret-leak tests, offline-mode egress test, sandbox escape smoke tests.
* **Eval**: `fixloop-eval` runs on a schedule. It reports found bugs, false alarms and fix pass rate per model.

---

## 9. Risks

| Risk | Mitigation |
|---|---|
| Scope is very large for v1.0 | Strict phase gates; adapters beyond GitHub/Jira are community-scalable via traits |
| SQLite + Postgres dual support doubles migration work | Keep SQL portable; both in CI from day one; pg-only features (pgvector, SKIP LOCKED) have SQLite fallbacks |
| Leptos churn (0.9 RC) | Pin 0.8.x; dashboard is API-driven so it can be replaced (ADR-0001) |
| OpenShell API changes (pre-1.0, v0.1.8 today) | Isolated behind feature flag + trait; pin a tested version; contract tests |
| False-positive noise kills adoption | Confidence threshold, dismissal learning, eval harness tracks false-alarm rate as a release gate |
| Rust SAML ecosystem thin | Decide in Phase 8; OIDC bridge fallback |

---

## 10. Open questions (blocking or near-blocking)

Decided (2026-11-14):
* Repo: `github.com/bwalia/fixloop`. The unrelated QA Allure manifests were removed (still in git history at `9450a51`).
* Copyright holder: "The Fixloop Authors".
* Container registry: GHCR under the project org (`ghcr.io/bwalia/fixloop`).
* Security reports: GitHub private vulnerability reporting (no email needed).
* MSRV: Rust 1.94 (raised from 1.89 in Phase 2; required by sqlx 0.9), edition 2024. CI checks it.
* YAML: `serde-saphyr`, because `serde_yaml` is deprecated. figment is used for layering and env overrides only.

Still open:
* Additional maintainers (ideally from other organisations; this strengthens the CNCF application).
* Test GitHub org/repo for the GitHub App (needed in Phase 2).
* Jira Cloud test site, or fixtures only (Phase 3).
* Frontier provider and local model for the Phase 1 exit test.
