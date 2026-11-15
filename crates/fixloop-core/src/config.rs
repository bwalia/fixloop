//! Server/worker configuration (`fixloop.yaml`).
//!
//! Layers, lowest precedence first:
//! 1. built-in defaults,
//! 2. the YAML file (if any),
//! 3. environment variables `FIXLOOP_<SECTION>__<KEY>` (e.g. `FIXLOOP_SERVER__BIND`).
//!
//! The config never holds secret values. Credentials are referenced by the
//! *name* of an environment variable (`api_key_env`).

use std::collections::BTreeMap;
use std::path::Path;

use figment::Figment;
use figment::providers::{Env, Serialized};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::Locality;
use crate::error::ConfigError;

/// Prefix for environment variable overrides.
pub const ENV_PREFIX: &str = "FIXLOOP_";

/// Top-level configuration for a Fixloop process.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FixloopConfig {
    /// HTTP server settings.
    pub server: ServerConfig,
    /// Database settings.
    pub database: DatabaseConfig,
    /// Logging, tracing and metrics settings.
    pub telemetry: TelemetryConfig,
    /// Model providers, models and per-task routes.
    pub llm: LlmConfig,
    /// Sandbox runner selection.
    pub sandbox: SandboxConfig,
    /// GitHub App settings. Unset means GitHub integration is off.
    pub github: Option<GithubConfig>,
}

/// GitHub App settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubConfig {
    /// The App's numeric id.
    pub app_id: u64,
    /// Path to the App's private key (PEM).
    pub private_key_path: std::path::PathBuf,
    /// Name of the environment variable holding the webhook secret.
    #[serde(default = "default_webhook_secret_env")]
    pub webhook_secret_env: String,
    /// REST API root. Change for GitHub Enterprise Server.
    #[serde(default = "default_github_api")]
    pub api_url: Url,
}

fn default_webhook_secret_env() -> String {
    "FIXLOOP_GITHUB_WEBHOOK_SECRET".into()
}

fn default_github_api() -> Url {
    // A constant, known-valid URL.
    #[allow(clippy::expect_used)]
    Url::parse("https://api.github.com").expect("valid constant URL")
}

/// Which parts of Fixloop this process runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// API, worker, scheduler and operator in one process.
    #[default]
    All,
    /// HTTP API, webhooks and dashboard only.
    Api,
    /// Job worker only.
    Worker,
}

/// HTTP server settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Socket address to listen on.
    pub bind: String,
    /// Process role.
    pub role: Role,
    /// Concurrent job workers in this process (roles `all` and `worker`).
    pub workers: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".into(),
            role: Role::All,
            workers: 2,
        }
    }
}

/// Database settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseConfig {
    /// `postgres://...` or `sqlite://...` connection URL.
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://fixloop.db?mode=rwc".into(),
        }
    }
}

/// Log output format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogFormat {
    /// Human-readable, for terminals.
    #[default]
    Pretty,
    /// Structured JSON lines, for log pipelines.
    Json,
}

/// Logging, tracing and metrics settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TelemetryConfig {
    /// Log format.
    pub log_format: LogFormat,
    /// `tracing` filter directive, e.g. `info` or `fixloop=debug,info`.
    pub log_level: String,
    /// OTLP (gRPC) endpoint for traces. Tracing export is off when unset.
    pub otlp_endpoint: Option<Url>,
    /// `service.name` resource attribute.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_format: LogFormat::Pretty,
            log_level: "info".into(),
            otlp_endpoint: None,
            service_name: "fixloop".into(),
        }
    }
}

/// Model provider API family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Anthropic Messages API.
    Anthropic,
    /// OpenAI API.
    Openai,
    /// Google Gemini API.
    Gemini,
    /// Azure OpenAI.
    AzureOpenai,
    /// AWS Bedrock.
    Bedrock,
    /// Google Vertex AI.
    Vertex,
    /// Any OpenAI-compatible endpoint (Ollama, vLLM, llama.cpp, LM Studio, NIM).
    OpenaiCompatible,
}

impl ProviderKind {
    /// Whether this provider is run by the organisation or by a third party.
    pub fn locality(self) -> Locality {
        match self {
            Self::OpenaiCompatible => Locality::Local,
            _ => Locality::External,
        }
    }
}

/// Ready-made defaults for popular OpenAI-compatible servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Ollama (`http://localhost:11434/v1`).
    Ollama,
    /// vLLM (`http://localhost:8000/v1`).
    Vllm,
    /// llama.cpp server (`http://localhost:8080/v1`).
    LlamaCpp,
    /// LM Studio (`http://localhost:1234/v1`).
    LmStudio,
    /// NVIDIA NIM (`http://localhost:8000/v1`).
    Nim,
}

impl Preset {
    /// The endpoint used when `base_url` is not set.
    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Ollama => "http://localhost:11434/v1",
            Self::Vllm | Self::Nim => "http://localhost:8000/v1",
            Self::LlamaCpp => "http://localhost:8080/v1",
            Self::LmStudio => "http://localhost:1234/v1",
        }
    }
}

/// A configured model provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    /// API family.
    pub kind: ProviderKind,
    /// Endpoint. Required for `openai_compatible` unless `preset` is set.
    #[serde(default)]
    pub base_url: Option<Url>,
    /// Defaults for a known OpenAI-compatible server.
    #[serde(default)]
    pub preset: Option<Preset>,
    /// Name of the environment variable holding the API key (never the key itself).
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Request timeout in seconds. Local models can be slow, so the default is generous.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_timeout_secs() -> u64 {
    300
}

/// Hosts of public, third-party AI APIs. In offline mode an
/// `openai_compatible` provider may not point at any of these.
pub const PUBLIC_AI_HOSTS: &[&str] = &[
    "api.anthropic.com",
    "api.openai.com",
    "generativelanguage.googleapis.com",
    "aiplatform.googleapis.com",
    "openai.azure.com",
    "bedrock-runtime.amazonaws.com",
    "api.mistral.ai",
    "api.groq.com",
    "api.together.xyz",
    "api.together.ai",
    "api.fireworks.ai",
    "api.deepseek.com",
    "openrouter.ai",
    "api.cohere.com",
    "api.x.ai",
    "integrate.api.nvidia.com",
];

/// Whether `host` is, or is a subdomain of, a public AI API host.
pub fn is_public_ai_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    PUBLIC_AI_HOSTS.iter().any(|h| {
        host == *h
            || host.ends_with(&format!(".{h}"))
            || (h.starts_with("bedrock-runtime") && host.starts_with("bedrock-runtime."))
    })
}

impl ProviderConfig {
    /// The endpoint to call: `base_url`, else the preset's default.
    pub fn effective_base_url(&self) -> Option<Url> {
        self.base_url.clone().or_else(|| {
            self.preset
                .and_then(|p| Url::parse(p.default_base_url()).ok())
        })
    }
}

/// How a model is asked to call tools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    /// The provider's native tool-calling API.
    #[default]
    Native,
    /// Tools described in the prompt; the model answers with a JSON envelope.
    /// For models without native tool support.
    Json,
}

/// Price of a model in US dollars per million tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    /// Dollars per million input tokens.
    pub input_per_mtok: f64,
    /// Dollars per million output tokens.
    pub output_per_mtok: f64,
}

/// A named model on a provider. Model identifiers only ever come from config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    /// Key into [`LlmConfig::providers`].
    pub provider: String,
    /// Provider-specific model identifier.
    pub model: String,
    /// Price, for cost reporting and cost budgets. Unset means free (e.g. local models).
    #[serde(default)]
    pub price: Option<Price>,
    /// How tools are offered to this model.
    #[serde(default)]
    pub tool_mode: ToolMode,
}

/// Token and cost limits. Unset fields mean no limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BudgetLimit {
    /// Maximum input + output tokens.
    pub max_tokens: Option<u64>,
    /// Maximum cost in US dollars.
    pub max_cost_usd: Option<f64>,
}

/// Global budgets. Per-repository and per-team budgets come from policies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BudgetsConfig {
    /// Limit for a single job (one review, one fix).
    pub per_job: BudgetLimit,
    /// Limit for all jobs in one UTC day.
    pub daily: BudgetLimit,
}

impl Default for BudgetsConfig {
    fn default() -> Self {
        Self {
            per_job: BudgetLimit {
                max_tokens: Some(2_000_000),
                max_cost_usd: Some(5.0),
            },
            daily: BudgetLimit::default(),
        }
    }
}

/// Retry policy for a single model before falling back to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RetryConfig {
    /// Retries after the first attempt, for rate limits, server errors and timeouts.
    pub max_retries: u32,
    /// Delay before the first retry; doubles each time.
    pub initial_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_backoff_ms: 500,
        }
    }
}

/// The kinds of work Fixloop routes to models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    /// Decide which files or hunks deserve a deep look.
    Triage,
    /// Decide whether two issues are the same.
    Dedupe,
    /// Produce embeddings.
    Embed,
    /// Review a diff and produce findings.
    Review,
    /// Write a fix.
    Fix,
    /// Independently review a proposed fix.
    SecondReview,
    /// Explain a finding to a user.
    Explain,
    /// Summarise a review or job.
    Summarize,
}

/// Model providers, models and routes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LlmConfig {
    /// When true, only self-hosted providers may be configured or called.
    pub offline: bool,
    /// Providers by name.
    pub providers: BTreeMap<String, ProviderConfig>,
    /// Models by name.
    pub models: BTreeMap<String, ModelConfig>,
    /// Ordered fallback chain of model names, per task.
    pub routes: BTreeMap<Task, Vec<String>>,
    /// Token and cost budgets.
    pub budgets: BudgetsConfig,
    /// Retry policy.
    pub retry: RetryConfig,
}

/// Sandbox runner implementations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunnerKind {
    /// Docker (default).
    #[default]
    Docker,
    /// Podman (rootless supported).
    Podman,
    /// Kubernetes Job.
    Kubernetes,
    /// Runs directly on the host. Development only; offers no isolation.
    Local,
    /// NVIDIA OpenShell (optional, requires the `openshell` build feature).
    Openshell,
}

/// Sandbox runner selection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxConfig {
    /// Runner to use.
    pub runner: RunnerKind,
    /// Runner to use if `runner` is unavailable. Unset means fail instead.
    pub fallback: Option<RunnerKind>,
}

impl FixloopConfig {
    /// Load configuration from defaults, an optional YAML file and the
    /// process environment, then validate it.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        Self::load_with(path, Env::prefixed(ENV_PREFIX).split("__"))
    }

    fn load_with(path: Option<&Path>, env: Env) -> Result<Self, ConfigError> {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));
        if let Some(path) = path {
            let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })?;
            let value: serde_json::Value =
                serde_saphyr::from_str(&text).map_err(|e| ConfigError::Parse {
                    path: path.to_path_buf(),
                    message: e.to_string(),
                })?;
            if !value.is_null() {
                figment = figment.merge(Serialized::defaults(value));
            }
        }
        let config: Self = figment
            .merge(env)
            .extract()
            .map_err(|e| ConfigError::Extract(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Check cross-field rules that the schema alone cannot express.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let llm = &self.llm;
        for (name, p) in &llm.providers {
            if p.kind == ProviderKind::OpenaiCompatible && p.effective_base_url().is_none() {
                return Err(ConfigError::invalid(
                    format!("llm.providers.{name}.base_url"),
                    "openai_compatible providers need an endpoint",
                    "set base_url, or a preset: ollama, vllm, llama_cpp, lm_studio, nim",
                ));
            }
            if p.preset.is_some() && p.kind != ProviderKind::OpenaiCompatible {
                return Err(ConfigError::invalid(
                    format!("llm.providers.{name}.preset"),
                    "presets only apply to openai_compatible providers",
                    "remove `preset` or set `kind: openai_compatible`",
                ));
            }
            if llm.offline
                && let Some(host) = p
                    .effective_base_url()
                    .as_ref()
                    .and_then(|u| u.host_str().map(str::to_owned))
                && is_public_ai_host(&host)
            {
                return Err(ConfigError::invalid(
                    format!("llm.providers.{name}.base_url"),
                    format!("`{host}` is a public AI API but llm.offline is true"),
                    "point base_url at your own server, or set llm.offline: false",
                ));
            }
            if llm.offline && p.kind.locality() == Locality::External {
                return Err(ConfigError::invalid(
                    format!("llm.providers.{name}"),
                    format!("{:?} is an external API but llm.offline is true", p.kind),
                    "remove this provider or set llm.offline: false",
                ));
            }
        }
        for (name, m) in &llm.models {
            if let Some(price) = m.price
                && (price.input_per_mtok < 0.0 || price.output_per_mtok < 0.0)
            {
                return Err(ConfigError::invalid(
                    format!("llm.models.{name}.price"),
                    "prices cannot be negative",
                    "use dollars per million tokens, e.g. input_per_mtok: 3.0",
                ));
            }
            if !llm.providers.contains_key(&m.provider) {
                return Err(ConfigError::invalid(
                    format!("llm.models.{name}.provider"),
                    format!("unknown provider `{}`", m.provider),
                    format!("add `{}` under llm.providers or fix the name", m.provider),
                ));
            }
        }
        for (task, chain) in &llm.routes {
            let key = format!("llm.routes.{}", task_key(*task));
            if chain.is_empty() {
                return Err(ConfigError::invalid(
                    key,
                    "route has no models",
                    "list at least one model name, or remove the route",
                ));
            }
            if let Some(missing) = chain.iter().find(|m| !llm.models.contains_key(*m)) {
                return Err(ConfigError::invalid(
                    key,
                    format!("unknown model `{missing}`"),
                    format!("add `{missing}` under llm.models or fix the name"),
                ));
            }
        }
        match self.sandbox.fallback {
            Some(RunnerKind::Local) => Err(ConfigError::invalid(
                "sandbox.fallback",
                "the local runner offers no isolation and cannot be a fallback",
                "use docker, podman or kubernetes as the fallback, or remove it",
            )),
            Some(f) if f == self.sandbox.runner => Err(ConfigError::invalid(
                "sandbox.fallback",
                "fallback is the same as the primary runner",
                "pick a different runner or remove sandbox.fallback",
            )),
            _ => Ok(()),
        }
    }
}

fn task_key(task: Task) -> String {
    serde_json::to_value(task)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{task:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn no_env() -> Env {
        Env::prefixed("FIXLOOP_TEST_UNUSED_PREFIX_")
    }

    fn load_str(yaml: &str) -> Result<FixloopConfig, ConfigError> {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(yaml.as_bytes()).unwrap();
        FixloopConfig::load_with(Some(f.path()), no_env())
    }

    #[test]
    fn defaults_are_valid() {
        let c = FixloopConfig::load_with(None, no_env()).unwrap();
        assert_eq!(c, FixloopConfig::default());
    }

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(load_str("").unwrap(), FixloopConfig::default());
    }

    #[test]
    fn full_example_parses() {
        let c = load_str(
            r#"
server: { bind: "0.0.0.0:9000", role: worker }
telemetry: { log_format: json, otlp_endpoint: "http://otel:4317" }
llm:
  providers:
    claude: { kind: anthropic, api_key_env: ANTHROPIC_API_KEY }
    local:  { kind: openai_compatible, base_url: "http://localhost:11434/v1" }
  models:
    strong: { provider: claude, model: some-model }
    cheap:  { provider: local, model: some-local-model }
  routes:
    review: [strong, cheap]
    triage: [cheap]
sandbox: { runner: kubernetes, fallback: docker }
"#,
        )
        .unwrap();
        assert_eq!(c.server.role, Role::Worker);
        assert_eq!(c.telemetry.log_format, LogFormat::Json);
        assert_eq!(c.llm.routes[&Task::Review], vec!["strong", "cheap"]);
        assert_eq!(c.sandbox.fallback, Some(RunnerKind::Docker));
    }

    #[test]
    fn unknown_key_is_rejected() {
        let e = load_str("server: { bnid: x }").unwrap_err();
        assert!(matches!(e, ConfigError::Extract(_)), "{e}");
    }

    #[test]
    fn bad_yaml_reports_parse_error() {
        let e = load_str("server: [unclosed").unwrap_err();
        assert!(matches!(e, ConfigError::Parse { .. }), "{e}");
    }

    #[test]
    fn missing_file_reports_read_error() {
        let e = FixloopConfig::load_with(Some(Path::new("/nonexistent/fixloop.yaml")), no_env())
            .unwrap_err();
        assert!(matches!(e, ConfigError::Read { .. }));
    }

    #[test]
    fn offline_rejects_external_provider() {
        let e = load_str(
            "llm: { offline: true, providers: { claude: { kind: anthropic, api_key_env: K } } }",
        )
        .unwrap_err();
        assert!(e.to_string().contains("llm.providers.claude"), "{e}");
    }

    #[test]
    fn offline_accepts_local_provider() {
        load_str(
            "llm: { offline: true, providers: { o: { kind: openai_compatible, base_url: 'http://localhost:11434/v1' } } }",
        )
        .unwrap();
    }

    #[test]
    fn openai_compatible_requires_base_url() {
        let e = load_str("llm: { providers: { o: { kind: openai_compatible } } }").unwrap_err();
        assert!(e.to_string().contains("base_url"), "{e}");
    }

    #[test]
    fn preset_supplies_base_url() {
        let c = load_str("llm: { providers: { o: { kind: openai_compatible, preset: ollama } } }")
            .unwrap();
        assert_eq!(
            c.llm.providers["o"].effective_base_url().unwrap().as_str(),
            "http://localhost:11434/v1"
        );
    }

    #[test]
    fn offline_rejects_compatible_provider_pointing_at_public_api() {
        let e = load_str(
            "llm: { offline: true, providers: { o: { kind: openai_compatible, base_url: 'https://api.openai.com/v1' } } }",
        )
        .unwrap_err();
        assert!(e.to_string().contains("public AI API"), "{e}");
        assert!(is_public_ai_host("my-resource.openai.azure.com"));
        assert!(is_public_ai_host("bedrock-runtime.us-east-1.amazonaws.com"));
        assert!(!is_public_ai_host("vllm.internal.svc"));
    }

    #[test]
    fn negative_price_rejected() {
        let e = load_str(
            "llm: { providers: { p: { kind: openai, api_key_env: K } }, models: { m: { provider: p, model: x, price: { input_per_mtok: -1, output_per_mtok: 1 } } } }",
        )
        .unwrap_err();
        assert!(e.to_string().contains("price"), "{e}");
    }

    #[test]
    fn route_must_reference_known_model() {
        let e = load_str("llm: { routes: { second_review: [nope] } }").unwrap_err();
        assert!(e.to_string().contains("llm.routes.second_review"), "{e}");
        assert!(e.to_string().contains("nope"), "{e}");
    }

    #[test]
    fn local_runner_cannot_be_fallback() {
        let e = load_str("sandbox: { fallback: local }").unwrap_err();
        assert!(e.to_string().contains("sandbox.fallback"), "{e}");
    }

    #[test]
    #[allow(clippy::result_large_err)] // figment::Error is large; test-only
    fn env_overrides_file() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("f.yaml", "server: { bind: '127.0.0.1:1' }")?;
            jail.set_env("FIXLOOP_SERVER__BIND", "127.0.0.1:2");
            let c = FixloopConfig::load(Some(Path::new("f.yaml"))).map_err(|e| e.to_string())?;
            assert_eq!(c.server.bind, "127.0.0.1:2");
            Ok(())
        });
    }
}
