# Getting started

This guide takes you from nothing to Fixloop reviewing pull requests on your own project, with the AI model of your choice. It takes about 15 minutes.

**What works today:** the **Reviewer**. It reviews pull requests (or your local changes before you push), comments on the changed lines with fixes you can apply, and reports a pass / warn / fail check. Filing issues and fixing bugs come in later releases (see [ROADMAP](../ROADMAP.md)); `fixloop fix` is not available yet.

**The steps:**

1. [Install Fixloop](#1-install-fixloop)
2. [Choose and configure a model](#2-choose-and-configure-a-model)
3. [Review your local changes](#3-review-your-local-changes) (no GitHub needed)
4. [Add Fixloop to a project](#4-add-fixloop-to-a-project)
5. [Connect GitHub](#5-connect-github)
6. [Run the server for automatic reviews](#6-run-the-server-for-automatic-reviews)

---

## 1. Install Fixloop

Pick one.

**From source** (Rust 1.94 or newer, from [rustup.rs](https://rustup.rs)):

```sh
cargo install --locked --git https://github.com/bwalia/fixloop fixloop-cli
fixloop --version
```

This installs one program, `fixloop`. Prebuilt release binaries will come with the first release.

**With Docker** (to run the server; amd64 and arm64):

```sh
docker pull ghcr.io/bwalia/fixloop:edge
docker run --rm ghcr.io/bwalia/fixloop:edge --version
```

`edge` follows the `main` branch. For a fixed version, use a `sha-<commit>` tag; version tags will appear with releases.

---

## 2. Choose and configure a model

Fixloop works with hosted models and with models you run yourself. You set this up in a server config file, `fixloop.yaml`, which you pass with `--config` (or the `FIXLOOP_CONFIG` environment variable).

Three rules apply to every setup:

- **API keys never go in the file.** The file names an environment variable (`api_key_env`), and you export the key in your shell or secret store.
- **Model names live only in config.** Use the exact model id your provider lists.
- **`routes` decide which model does what.** `review` is the route the Reviewer uses. List more than one model to get a fallback: if the first fails or is rate limited, the next one is tried.

Create `fixloop.yaml` from one of these.

### Anthropic (Claude)

```yaml
llm:
  providers:
    anthropic: { kind: anthropic, api_key_env: ANTHROPIC_API_KEY }
  models:
    claude: { provider: anthropic, model: "claude-sonnet-5-5" }
  routes:
    review: [claude]
```

```sh
export ANTHROPIC_API_KEY=...
```

### OpenAI

```yaml
llm:
  providers:
    openai: { kind: openai, api_key_env: OPENAI_API_KEY }
  models:
    gpt: { provider: openai, model: "<model id from your OpenAI account>" }
  routes:
    review: [gpt]
```

### Google Gemini

Gemini works through its OpenAI-compatible endpoint:

```yaml
llm:
  providers:
    gemini:
      kind: openai_compatible
      base_url: "https://generativelanguage.googleapis.com/v1beta/openai"
      api_key_env: GEMINI_API_KEY
  models:
    gemini: { provider: gemini, model: "<Gemini model id>" }
  routes:
    review: [gemini]
```

### Any other OpenAI-compatible service

OpenRouter, Azure OpenAI, Together, Groq, a company gateway: anything that speaks the OpenAI chat completions API works the same way. Set its `base_url` (the part before `/chat/completions`) and the variable holding its key:

```yaml
llm:
  providers:
    gateway:
      kind: openai_compatible
      base_url: "https://openrouter.ai/api/v1"
      api_key_env: OPENROUTER_API_KEY
  models:
    main: { provider: gateway, model: "<model id on that service>" }
  routes:
    review: [main]
```

### Fully local and offline (Ollama, vLLM, llama.cpp, LM Studio, NVIDIA NIM)

No code leaves your machine. Presets fill in the usual local address (`ollama` is `http://localhost:11434/v1`); set `base_url` instead if your server runs elsewhere. `offline: true` makes Fixloop refuse to start if any provider points at a public AI service.

```yaml
llm:
  offline: true
  providers:
    ollama: { kind: openai_compatible, preset: ollama }
  models:
    local: { provider: ollama, model: "qwen3:30b-a3b-instruct-2507-q4_K_M" }
  routes:
    review: [local]
```

```sh
ollama pull qwen3:30b-a3b-instruct-2507-q4_K_M
```

Model size matters for review quality. In our tests a 30B-class model found real bugs with no false alarms, while 3–4B models are fine only for trying things out. If a model handles tool calls poorly, add `tool_mode: json` to it.

### Mix them

Use a hosted model with a local fallback, and cap spending:

```yaml
llm:
  providers:
    anthropic: { kind: anthropic, api_key_env: ANTHROPIC_API_KEY }
    ollama:    { kind: openai_compatible, preset: ollama }
  models:
    claude:
      provider: anthropic
      model: "claude-sonnet-5-5"
      # Your provider's prices, to enable cost budgets and reports:
      # price: { input_per_mtok: 0.0, output_per_mtok: 0.0 }
    local: { provider: ollama, model: "qwen3:30b-a3b-instruct-2507-q4_K_M" }
  routes:
    review: [claude, local]
  budgets:
    per_job: { max_tokens: 2000000, max_cost_usd: 5.0 }
    daily:   { max_cost_usd: 50.0 }
```

A project that must never send code to a hosted model sets `data_egress: local-only` in its `.fixloop.yaml` (step 4). For that project, Fixloop skips hosted models in every route, and fails rather than fall back to one.

### Check the setup

```sh
fixloop --config fixloop.yaml config validate   # is the file valid?
fixloop --config fixloop.yaml llm list          # models, routes, allowed endpoints
fixloop --config fixloop.yaml llm test          # send a prompt through the review route
fixloop --config fixloop.yaml llm test --tool   # check the model can report findings
```

All options are in the [configuration reference](config-reference.md).

---

## 3. Review your local changes

The quickest way to see Fixloop work, with no GitHub setup: commit or edit some code on a branch, then run inside the repository:

```sh
fixloop --config /path/to/fixloop.yaml review
```

Fixloop compares your working tree with the default branch (`--base main` to choose another) and prints its findings with suggested fixes. Nothing is posted anywhere. It exits with code 3 if it found blocking problems, so you can use it in a pre-push hook.

---

## 4. Add Fixloop to a project

In the project's repository:

```sh
fixloop init
```

This creates two things to commit:

- **`.fixloop.yaml`**, the project's settings. The ones you are most likely to change:

  | Setting | Default | Meaning |
  |---|---|---|
  | `data_egress` | `any` | `local-only`: only self-hosted models may see this code |
  | `review.confidence_threshold` | `0.7` | Hide findings the model is less sure of |
  | `review.min_severity` | `low` | Hide findings below this severity |
  | `review.ignore_paths` | `vendor/**`, generated files | Paths never reviewed |

- **`.fixloop/rules/`**, your team's rules, one Markdown file each, in plain words. For example, `.fixloop/rules/no-ignored-errors.md`:

  ```markdown
  # Don't ignore errors
  Never discard an error with `_ =` or an empty `catch`. Handle it, or return it with context.
  ```

  The Reviewer checks every pull request against these rules.

Check the file with `fixloop config validate-repo`. Settings and rules are always read from the pull request's **base** branch, so a pull request cannot loosen its own review.

---

## 5. Connect GitHub

Fixloop runs on GitHub as a **GitHub App** that you own. It asks only for what the Reviewer needs:

- **Contents:** read.
- **Pull requests:** write, to post comments.
- **Checks:** write, to post pass / warn / fail.
- **Issues:** read, which GitHub requires before an App can receive pull request comments.

**Create the App.** One command, one click:

```sh
fixloop github create-app --name <your-app-name> \
  --webhook-url https://<your-host>/webhooks/github \
  --out-dir ~/.config/fixloop
```

A browser page opens; click **Create GitHub App**. Add `--org <org>` to create it in an organisation. If you don't have a public URL yet, leave out `--webhook-url` and set it later in the App's settings. Fixloop saves into `--out-dir`:

- `<slug>.private-key.pem`, the App's private key;
- `<slug>.json`, with the App id and webhook secret.

The command prints the `github:` section for your config, which looks like this:

```yaml
github:
  app_id: 123456
  private_key_path: /home/you/.config/fixloop/<slug>.private-key.pem
```

**Install it.** Open the install link the command prints, choose **Only select repositories**, and pick your projects.

**Try it on a pull request** before turning on automatic reviews:

```sh
fixloop --config fixloop.yaml review --pr https://github.com/<owner>/<repo>/pull/<n>          # dry run: prints, posts nothing
fixloop --config fixloop.yaml review --pr https://github.com/<owner>/<repo>/pull/<n> --post   # posts the review and check
```

---

## 6. Run the server for automatic reviews

The server receives GitHub's webhooks and reviews every pull request when it is opened or updated. GitHub must be able to reach `https://<your-host>/webhooks/github`.

Every option needs the webhook secret from `<slug>.json`:

```sh
export FIXLOOP_GITHUB_WEBHOOK_SECRET=$(jq -r .webhook_secret ~/.config/fixloop/<slug>.json)
```

**On a machine:**

```sh
fixloop --config fixloop.yaml serve
```

It listens on `127.0.0.1:8080` by default (`server.bind` changes this). Put it behind your reverse proxy or a tunnel that serves HTTPS.

**With Docker:**

```sh
docker run -d --name fixloop -p 8080:8080 \
  -v $PWD/fixloop.yaml:/etc/fixloop/fixloop.yaml:ro \
  -v ~/.config/fixloop/<slug>.private-key.pem:/etc/fixloop/github.pem:ro \
  -v fixloop-data:/var/lib/fixloop \
  -e ANTHROPIC_API_KEY -e FIXLOOP_GITHUB_WEBHOOK_SECRET \
  ghcr.io/bwalia/fixloop:edge --config /etc/fixloop/fixloop.yaml serve
```

In the container, set `github.private_key_path: /etc/fixloop/github.pem`, and add a database on the volume:

```yaml
database:
  url: "sqlite:///var/lib/fixloop/fixloop.db?mode=rwc"
```

**On Kubernetes:** see [deploy/kubernetes](../deploy/kubernetes/README.md).

**Database:** SQLite (the default) is fine for one server. For more than one server, or for high volume, use PostgreSQL:

```yaml
database:
  url: "postgres://user:password@host:5432/fixloop"
```

You can also set it as the `FIXLOOP_DATABASE__URL` environment variable instead.

**Check it is up:**

- `https://<your-host>/` shows the version.
- `/healthz` reports whether the server is running.
- `/readyz` reports whether the database is reachable.

Then open a pull request. On GitHub, the App's **Settings → Advanced → Recent deliveries** page shows whether the webhook arrived.

---

## Using it day to day

- **Automatic:** every pull request is reviewed when it is opened, updated, reopened or marked ready. Drafts are skipped.
- **On demand:** comment `/fixloop review` on a pull request to review it again. Only people with write access can do this.
- **Before you push:** run `fixloop review` locally.
- **Tuning:** too noisy? Raise `review.confidence_threshold` or `review.min_severity`. Missing a team convention? Add a rule file.

## Troubleshooting

| Symptom | Check |
|---|---|
| `environment variable ... is not set` | Export the API key named by `api_key_env` |
| `llm test` fails | `fixloop llm list` shows the endpoint used; check the model id and that the server is running |
| No review on a pull request | The App's **Recent deliveries** page; the server's log; that the App is installed on that repository |
| `401` on deliveries | `FIXLOOP_GITHUB_WEBHOOK_SECRET` must match the App's webhook secret |
| `503 GitHub webhooks are not configured` | The config has no `github:` section, or the webhook secret is not set |
| Findings look wrong or noisy | Use a larger model, or raise `review.confidence_threshold` |

Run any command with `--log-level debug` for detail. More: [GitHub setup](github.md), [configuration reference](config-reference.md), [security model](security/threat-model.md).
