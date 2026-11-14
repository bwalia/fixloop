# fixloop-core

Domain model, configuration schema and error types shared by every Fixloop crate. It performs no network I/O.

- `config`: server/worker configuration (`fixloop.yaml` + `FIXLOOP_*` env vars)
- `repo_config`: per-repository `.fixloop.yaml`
- `domain`: severities, categories, repo modes, data-egress classes
- `error`: errors that carry an actionable hint

License: Apache-2.0
