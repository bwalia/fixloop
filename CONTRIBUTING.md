# Contributing to Fixloop

Thanks for helping! This guide covers how to propose changes and what we expect from a pull request.

## Ways to contribute

- Report bugs and request features in [Issues](https://github.com/bwalia/fixloop/issues).
- Ask questions in [Discussions](https://github.com/bwalia/fixloop/discussions).
- Pick up an issue labelled `good first issue` or `help wanted`.
- Add an adapter (SCM, tracker, LLM provider, sandbox runner). Each one plugs in behind a trait; see [docs/PLAN.md](docs/PLAN.md).

For larger changes, open an issue or discussion first so we can agree on the approach. Changes to architecture need an ADR in `docs/adr/`.

## Development setup

```sh
git clone https://github.com/bwalia/fixloop && cd fixloop
cargo build --workspace
cargo test --workspace
```

Before you push, run the same checks CI runs:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check          # cargo install cargo-deny
cargo audit               # cargo install cargo-audit
```

## Code rules

- No `unwrap()`, `expect()` or `panic!` in non-test code. Return errors with context. Clippy enforces this.
- User-facing errors say what went wrong **and** what to do next.
- Never log secrets. Never send secrets to a model.
- Public items have doc comments. Each crate has a README.
- New dependencies must use a license on the [CNCF allowlist](https://github.com/cncf/foundation/blob/main/policies-guidance/allowed-third-party-license-policy.md). `cargo deny` checks this.
- Add tests: unit tests for logic, recorded-fixture tests for adapters.

## Pull requests

- Keep PRs small and focused.
- Update `CHANGELOG.md` under `Unreleased`.
- Update docs when behaviour or config changes.

## Developer Certificate of Origin

Fixloop uses the [Developer Certificate of Origin](https://developercertificate.org/) (DCO), as all CNCF projects do. Every commit must be signed off, which certifies that you wrote it or have the right to submit it under the project's license:

```sh
git commit -s -m "Fix the thing"
```

This adds `Signed-off-by: Your Name <you@example.com>`. The email must match the commit author. To fix commits you already made:

```sh
git rebase --signoff main
git push --force-with-lease
```

## Code of conduct

This project follows the [CNCF Code of Conduct](CODE_OF_CONDUCT.md).
