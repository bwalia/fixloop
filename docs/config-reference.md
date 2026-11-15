# Configuration reference

Fixloop has two kinds of configuration:

| File | Who writes it | Purpose |
|---|---|---|
| `fixloop.yaml` | the person running Fixloop | server, database, telemetry, models, sandbox |
| `.fixloop.yaml` | each repository | what Fixloop may do in that repo |

Check them with `fixloop config validate` and `fixloop config validate-repo`. Print the effective server config with `fixloop config show`.

## Server config (`fixloop.yaml`)

Pass the file with `--config` or `FIXLOOP_CONFIG`. Every key can be overridden by an environment variable named `FIXLOOP_<SECTION>__<KEY>` (double underscore between levels), e.g. `FIXLOOP_SERVER__BIND=0.0.0.0:8080`. Other `FIXLOOP_*` variables without a double underscore (such as `FIXLOOP_CONFIG`) are not overrides.

Precedence, lowest first: built-in defaults → file → environment.

Unknown keys are rejected so that typos are caught early.

### `server`
| Key | Default | Description |
|---|---|---|
| `bind` | `127.0.0.1:8080` | Address to listen on |
| `role` | `all` | `all`, `api` or `worker` |
| `workers` | `2` | Concurrent job workers in this process |

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

| Key | Default | Description |
|---|---|---|
| `offline` | `false` | When `true`, only `openai_compatible` (self-hosted) providers are allowed, and they may not point at a public AI API host. Checked at startup. |
| `providers.<name>.kind` | | `anthropic`, `openai`, `openai_compatible`. (`gemini`, `azure_openai`, `bedrock`, `vertex` are accepted by the schema but not implemented yet; Gemini and Azure work today via `openai_compatible` and their OpenAI-compatible endpoint.) |
| `providers.<name>.base_url` | per kind | Endpoint. Defaults: Anthropic `https://api.anthropic.com`, OpenAI `https://api.openai.com/v1`. Required for `openai_compatible` unless `preset` is set. |
| `providers.<name>.preset` | | For `openai_compatible`: `ollama` (`:11434`), `vllm` (`:8000`), `llama_cpp` (`:8080`), `lm_studio` (`:1234`), `nim` (`:8000`), all on `http://localhost/v1`. |
| `providers.<name>.api_key_env` | | **Name** of the environment variable holding the key. Keys never go in the file. Required for `anthropic` and `openai`. |
| `providers.<name>.timeout_secs` | `300` | Per-request timeout. |
| `models.<name>.provider` | | A key under `providers` |
| `models.<name>.model` | | The provider's model identifier |
| `models.<name>.price` | none (free) | `{ input_per_mtok, output_per_mtok }` in US dollars per million tokens. Needed for cost budgets and cost reports. |
| `models.<name>.tool_mode` | `native` | `native` uses the provider's tool calling. `json` describes tools in the prompt and parses a JSON reply, for models without reliable native tool support. |
| `routes.<task>` | | Ordered fallback list of model names. Tasks: `triage`, `dedupe`, `embed`, `review`, `fix`, `second_review`, `explain`, `summarize` |
| `budgets.per_job` | 2M tokens, $5 | `{ max_tokens, max_cost_usd }` for one job. Unset fields mean no limit. |
| `budgets.daily` | no limit | Same shape, for all jobs in one UTC day. |
| `retry.max_retries` | `2` | Retries on the same model for rate limits (429), server errors (5xx), timeouts and connection errors, before moving to the next model. |
| `retry.initial_backoff_ms` | `500` | First retry delay; doubles each time. |

**How a request is routed:** the router walks the task's chain in order. Models a repository may not use (`data_egress: local-only` skips external providers) are left out. If nothing is left, the request fails; it is never silently sent somewhere else. Each model is tried with retries, then the next one. A budget overrun stops the whole request.

**Network:** model calls can only reach the hosts in `llm.providers`, and HTTP redirects are not followed.

See [`examples/fixloop-local.yaml`](../examples/fixloop-local.yaml) (offline, Ollama only) and [`examples/fixloop-frontier.yaml`](../examples/fixloop-frontier.yaml). Check a setup with:

```sh
fixloop --config fixloop.yaml llm list                 # models, routes, allowed endpoints
fixloop --config fixloop.yaml llm test                 # send a prompt through the review route
fixloop --config fixloop.yaml llm test --model local --tool   # check tool calling
```

### `github`
Unset means GitHub integration is off. See [GitHub setup](github.md).

| Key | Default | Description |
|---|---|---|
| `app_id` | | The GitHub App's id |
| `private_key_path` | | Path to the App's private key (PEM) |
| `webhook_secret_env` | `FIXLOOP_GITHUB_WEBHOOK_SECRET` | Name of the environment variable holding the webhook secret |
| `api_url` | `https://api.github.com` | Change for GitHub Enterprise Server |

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
