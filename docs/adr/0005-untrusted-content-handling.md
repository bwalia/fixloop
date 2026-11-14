# ADR-0005: Untrusted content handling and prompt-injection defence

* Status: Proposed
* Date: 2026-11-14

## Context
PR text, diffs, code comments, issue bodies, CI logs, tracker comments and MCP tool output are attacker-controllable. An injected agent could exfiltrate secrets, post spam, push malicious code or approve its own changes.

## Decision (defence in depth)
1. **Instruction/data separation.** System prompts are static, versioned templates in `fixloop-agents/prompts/`. Untrusted content is only ever inserted inside labelled data blocks with per-request random delimiters, and the prompt states that the contents are data. Untrusted text is never placed in the system role.
2. **Structured output only.** Agents return JSON validated against a schema. Free text is confined to specific fields (`rationale`, `suggested_fix`), which are rendered as escaped Markdown and **never** interpreted as commands. Output that mentions `/fixloop` commands is neutralised.
3. **Capability scoping.** Each agent gets a fixed tool allow list. The Reviewer has no write tools. The Fixer can only edit inside the sandbox workspace. Outbound actions (post comment, open PR, file issue, MCP writes) are performed by deterministic worker code after validation, not by the model directly.
4. **Policy gates outside the model.** Mode (`comment-only` … `auto-push`), approvals, budgets, the protected default branch and secret scanning are enforced in code. The model cannot change them.
5. **Command authorisation.** `/fixloop` PR comments are honoured only from users with write access (checked via the SCM API), never from text inside the diff or issue body.
6. **Egress control.** The sandbox allow list and the offline guard limit exfiltration channels even if the model is fooled.
7. **Secret hygiene.** Diffs and contexts are scanned and redacted before LLM calls. All outbound text (comments, issues, PR bodies) is scanned before posting.
8. **Regression corpus.** `tests/injection/` holds known injection payloads in diffs, issue bodies, filenames, CI logs and MCP responses. Every payload must result in no policy-violating action. It runs in CI on every PR.

## Consequences
There is some quality cost from strict schemas. It is offset by the reduced noise and the large gain in trust. The threat model (`docs/security/threat-model.md`, Phase 0) references this ADR.
