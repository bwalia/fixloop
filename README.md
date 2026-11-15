# Fixloop

**Fixloop is an open source AI agent that closes the bug loop on your codebase:**

**Review → Find → File → Fix → Check → Close**

1. **Review** pull requests and leave clear, useful comments.
2. **Find** bugs, security issues and code smells, in PRs and in existing code.
3. **File** each real problem as an issue (GitHub Issues, Jira, …) with no duplicates.
4. **Fix** the problem in a sandbox and open a fix PR.
5. **Check** the fix: build, tests, lint and a second AI review must all pass.
6. **Close** the issue when the fix PR is merged.

Fixloop works with frontier models (Anthropic, OpenAI, Gemini, Azure, Bedrock, Vertex) **and** fully offline with local models (Ollama, vLLM, llama.cpp, LM Studio, NVIDIA NIM). It runs code only in sandboxes (Docker/Podman, Kubernetes Jobs, or optionally NVIDIA OpenShell), never pushes to your default branch, and never merges on its own.

> **Status: early development (Phase 0 of 10).** Nothing here is ready for production yet. See [ROADMAP.md](ROADMAP.md) and [docs/PLAN.md](docs/PLAN.md).

## Try it

```sh
cargo install --git https://github.com/bwalia/fixloop fixloop-cli
cd your-repo
fixloop init                      # adds .fixloop.yaml and .fixloop/rules/
fixloop config validate-repo      # checks it
```

## Build from source

Requires Rust 1.94 or newer.

```sh
cargo build --workspace
cargo test --workspace
cargo run --bin fixloop -- --help
```

## Documentation

- [Architecture and plan](docs/PLAN.md)
- [Architecture decision records](docs/adr/)
- [Configuration reference](docs/config-reference.md)
- [Threat model](docs/security/threat-model.md)

## Community

- Questions and ideas: [GitHub Discussions](https://github.com/bwalia/fixloop/discussions)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md) (commits need a DCO sign-off)
- Governance: [GOVERNANCE.md](GOVERNANCE.md)
- Security issues: [SECURITY.md](SECURITY.md). Please do not open public issues for these.
- Code of conduct: [CNCF Code of Conduct](CODE_OF_CONDUCT.md)

## License

Apache License 2.0. See [LICENSE](LICENSE).
