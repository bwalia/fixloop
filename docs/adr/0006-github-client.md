# ADR-0006: Thin GitHub REST client instead of octocrab

* Status: Accepted
* Date: 2026-11-15

## Context
The Reviewer needs about ten GitHub endpoints: App JWT and installation tokens, pull request and diff, file contents, check runs, reviews, comments and collaborator permission. The plan allowed either `octocrab` or a thin client.

## Decision
Use a thin client in `fixloop-scm` on top of the `reqwest`/rustls stack the model layer already uses.

* There is one HTTP and TLS stack in the binary, and one place to apply timeouts and retries.
* We control exactly which calls exist, which keeps least-privilege reviews easy. Every endpoint is visible in one file.
* App JWTs are signed with `jsonwebtoken` on `aws-lc-rs`. That avoids the `rsa` crate (RUSTSEC-2023-0071, a timing side channel) and shares the crypto provider rustls already uses.
* Installation tokens are cached per installation and refreshed after 50 minutes (they last 60).

## Consequences
* New endpoints (issues in Phase 3, branches and PRs in Phase 4) are added by hand, with fixture tests.
* GitHub Enterprise Server works through `github.api_url`.
* If the endpoint list grows large, revisit octocrab behind the same `ScmProvider` trait.
