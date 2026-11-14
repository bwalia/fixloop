# Fixloop Governance

Fixloop is a vendor-neutral open source project. No single company controls it. This document describes how decisions are made. It will evolve as the community grows.

## Values

- **Open**: discussions, decisions and the roadmap are public.
- **Vendor neutral**: no feature is reserved for any company, and there is no closed "enterprise" edition. Integrations with commercial products are optional plugins behind open traits.
- **Merit and inclusion**: anyone can earn a maintainer role through sustained, quality contributions.

## Roles

| Role | Who | Can |
|---|---|---|
| **Contributor** | Anyone who opens an issue, PR, review or discussion | Propose changes |
| **Reviewer** | Contributors with a record of quality reviews in an area | Approve PRs in that area (non-binding) |
| **Maintainer** | Listed in [MAINTAINERS.md](MAINTAINERS.md) | Merge PRs, triage, cut releases, vote |

### Becoming a maintainer

A contributor is nominated by an existing maintainer after sustained contributions (code, reviews, docs or community work). The nomination passes with a two-thirds majority of maintainers and no objection within 7 days.

### Stepping down and removal

Maintainers may step down at any time and become emeritus. A maintainer inactive for 6 months may be moved to emeritus after a check-in. Removal for cause needs a two-thirds majority of the other maintainers.

## Decision making

1. **Lazy consensus** is the default: a proposal (issue or PR) proceeds if no maintainer objects within a reasonable time (normally 72 hours for significant changes).
2. **Architecture changes** need an ADR in `docs/adr/` approved by two maintainers.
3. If consensus fails, maintainers **vote**: a simple majority of all maintainers decides. Governance changes need a two-thirds majority.

## Company diversity

When the project has maintainers from more than one organisation, no single organisation may hold more than half of maintainer votes. Votes beyond that limit are counted together as a single bloc.

## Code of conduct and security

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).

## Changes to this document

Changes go through a PR and need a two-thirds majority of maintainers.
