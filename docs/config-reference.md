# Configuration reference

Fixloop has two kinds of configuration:

| File | Who writes it | Purpose |
|---|---|---|
| `fixloop.yaml` | the person running Fixloop | server, database, telemetry, models, sandbox |
| `.fixloop.yaml` | each repository | what Fixloop may do in that repo |

Check them with `fixloop config validate` and `fixloop config validate-repo`. Print the effective server config with `fixloop config show`.

## Server config (`fixloop.yaml`)

Pass the file with `--config` or `FIXLOOP_CONFIG`. Every key can be overridden by an environment variable named `FIXLOOP_<SECTION>__<KEY>` (double underscore between levels), e.g. `FIXLOOP_SERVER__BIND=0.0.0.0:8080`.

Precedence, lowest first: built-in defaults → file → environment.

Unknown keys are rejected so that typos are caught early.

### `server`
| Key | Default | Description |
|---|---|---|
| `bind` | `127.0.0.1:8080` | Address to listen on |
| `role` | `all` | `all`, `api` or `worker` |

### `database`
| Key | Default | Description |
|---|---|---|
| `url` | `sqlite://fixloop.db?mode=rwc` | `postgres://…` for teams, `sqlite://…` for single-user mode |

### `telemetry`
| Key | Default | Description |
|---|---|---|
| `log_format` | `pretty` | `pretty` or `json` |
| `log_level` | `info` | `tracing` filter, e.g. `fixloop=debug,info` |
| `otlp_endpoint` | unset | OTLP gRPC endpoint, e.g. `http://otel-collector:4317`. Trace export is off when unset. |
| `service_name` | `fixloop` | OpenTelemetry `service.name` |

### `llm`
Model names live **only** in config. Nothing is hard-coded.

| Key | Description |
|---|---|
| `offline` | When `true`, only `openai_compatible` (self-hosted) providers are allowed. External APIs are rejected at startup. |
| `providers.<name>.kind` | `anthropic`, `openai`, `gemini`, `azure_openai`, `bedrock`, `vertex`, `openai_compatible` |
| `providers.<name>.base_url` | Endpoint. Required for `openai_compatible`. |
| `providers.<name>.api_key_env` | **Name** of the environment variable holding the key. Keys never go in the file. |
| `models.<name>.provider` | A key under `providers` |
| `models.<name>.model` | The provider's model identifier |
| `routes.<task>` | Ordered fallback list of model names. Tasks: `triage`, `dedupe`, `embed`, `review`, `fix`, `second_review`, `explain`, `summarize` |

```yaml
llm:
  providers:
    claude: { kind: anthropic, api_key_env: ANTHROPIC_API_KEY }
    local:  { kind: openai_compatible, base_url: "http://localhost:11434/v1" }
  models:
    strong: { provider: claude, model: "<model id>" }
    cheap:  { provider: local,  model: "<model id>" }
  routes:
    review: [strong, cheap]
    triage: [cheap]
```

### `sandbox`
| Key | Default | Description |
|---|---|---|
| `runner` | `docker` | `docker`, `podman`, `kubernetes`, `local` (dev only, no isolation), `openshell` (optional build feature) |
| `fallback` | unset | Runner to use if `runner` is unavailable. Unset means fail with an error. Cannot be `local`. |

## Repository config (`.fixloop.yaml`)

Created by `fixloop init`. All keys are optional.

| Key | Default | Description |
|---|---|---|
| `version` | `1` | Schema version |
| `mode` | `suggest` | `comment-only`, `suggest`, `open-fix-pr`, `auto-push-to-pr-branch` |
| `data_egress` | `any` | `any` or `local-only` (only self-hosted models may see this code) |
| `review.confidence_threshold` | `0.7` | Findings below this confidence (0.0–1.0) are not posted |
| `review.min_severity` | `low` | `info`, `low`, `medium`, `high`, `critical` |
| `review.ignore_paths` | `[]` | Glob patterns to skip |
| `fix.max_attempts` | `3` | Tries to make build + tests + lint pass before giving up |
| `fix.require_approval` | `true` | Ask a human before opening a fix PR |
| `rules_dir` | `.fixloop/rules` | Folder of plain-text Markdown rule files |
