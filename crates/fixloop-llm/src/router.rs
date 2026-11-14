//! Per-task model routing with fallback, retries, data-egress rules and
//! budgets.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fixloop_core::config::{
    BudgetsConfig, LlmConfig, Price, ProviderConfig, ProviderKind, RetryConfig, Task, ToolMode,
};
use fixloop_core::domain::{DataEgress, Locality};
use tracing::Instrument;
use tracing::field::Empty;
use url::Url;

use crate::budget::{self, BudgetAccount};
use crate::egress::{EgressGuard, HttpClient};
use crate::error::LlmError;
use crate::json_tools;
use crate::provider::LlmProvider;
use crate::providers::{anthropic, openai};
use crate::types::{CompletionRequest, CompletionResponse, Usage};

/// A model the router can use.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    /// Name from `llm.models`.
    pub name: String,
    /// Provider's model identifier.
    pub model_id: String,
    /// The provider serving it.
    pub provider: Arc<dyn LlmProvider>,
    /// Price, if known.
    pub price: Option<Price>,
    /// How tools are offered.
    pub tool_mode: ToolMode,
}

impl ModelEntry {
    fn cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        self.price.map_or(0.0, |p| {
            (input_tokens as f64 * p.input_per_mtok + output_tokens as f64 * p.output_per_mtok)
                / 1_000_000.0
        })
    }
}

/// Per-call context: where the code may go and which budgets apply.
#[derive(Debug, Clone, Default)]
pub struct CallContext {
    /// The repository's data-egress class.
    pub data_egress: DataEgress,
    /// Budgets to charge, in addition to the router's daily budget.
    pub budgets: Vec<Arc<BudgetAccount>>,
}

impl CallContext {
    /// A context with the given egress class and no extra budgets.
    pub fn new(data_egress: DataEgress) -> Self {
        Self {
            data_egress,
            budgets: Vec::new(),
        }
    }

    /// Also charge `account`.
    #[must_use]
    pub fn with_budget(mut self, account: Arc<BudgetAccount>) -> Self {
        self.budgets.push(account);
        self
    }
}

/// A successful completion and how it was served.
#[derive(Debug, Clone)]
pub struct RoutedResponse {
    /// The model's reply.
    pub response: CompletionResponse,
    /// Model name from config.
    pub model: String,
    /// Provider's model identifier.
    pub model_id: String,
    /// Provider name from config.
    pub provider: String,
    /// Cost in US dollars (0 when no price is configured).
    pub cost_usd: f64,
    /// Earlier models in the chain that failed, one line each.
    pub failed_attempts: Vec<String>,
}

/// Routes requests to models. Build it once per process and share it.
#[derive(Debug)]
pub struct Router {
    models: BTreeMap<String, ModelEntry>,
    routes: BTreeMap<Task, Vec<String>>,
    retry: RetryConfig,
    budgets: BudgetsConfig,
    daily: Mutex<(u64, Arc<BudgetAccount>)>,
    allowlist: Vec<String>,
}

impl Router {
    /// Build a router from config. `env` looks up environment variables
    /// (pass `|k| std::env::var(k).ok()` in production).
    pub fn from_config(
        cfg: &LlmConfig,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self, LlmError> {
        let mut providers: BTreeMap<String, Arc<dyn LlmProvider>> = BTreeMap::new();
        let mut allowlist = Vec::new();
        for (name, p) in &cfg.providers {
            if cfg.offline && p.kind.locality() == Locality::External {
                return Err(LlmError::Config {
                    message: format!(
                        "provider `{name}` is an external API but llm.offline is true"
                    ),
                    hint: "remove it or set llm.offline: false".into(),
                });
            }
            let (provider, base) = build_provider(name, p, env)?;
            allowlist.extend(EgressGuard::new([&base]).allowed().map(str::to_owned));
            providers.insert(name.clone(), provider);
        }

        let mut models = BTreeMap::new();
        for (name, m) in &cfg.models {
            let provider = providers.get(&m.provider).ok_or_else(|| LlmError::Config {
                message: format!("model `{name}` uses unknown provider `{}`", m.provider),
                hint: "fix llm.models.<name>.provider".into(),
            })?;
            models.insert(
                name.clone(),
                ModelEntry {
                    name: name.clone(),
                    model_id: m.model.clone(),
                    provider: Arc::clone(provider),
                    price: m.price,
                    tool_mode: m.tool_mode,
                },
            );
        }
        allowlist.sort();
        allowlist.dedup();
        Ok(Self::new(
            models.into_values().collect(),
            cfg.routes.clone(),
            cfg.retry,
            cfg.budgets,
        )
        .with_allowlist(allowlist))
    }

    /// Build a router from ready-made model entries (useful for tests and
    /// custom providers).
    pub fn new(
        models: Vec<ModelEntry>,
        routes: BTreeMap<Task, Vec<String>>,
        retry: RetryConfig,
        budgets: BudgetsConfig,
    ) -> Self {
        Self {
            models: models.into_iter().map(|m| (m.name.clone(), m)).collect(),
            routes,
            retry,
            budgets,
            daily: Mutex::new((today(), BudgetAccount::new("daily", budgets.daily))),
            allowlist: Vec::new(),
        }
    }

    fn with_allowlist(mut self, allowlist: Vec<String>) -> Self {
        self.allowlist = allowlist;
        self
    }

    /// The `host:port` pairs model calls may reach.
    pub fn egress_allowlist(&self) -> &[String] {
        &self.allowlist
    }

    /// Configured models.
    pub fn models(&self) -> impl Iterator<Item = &ModelEntry> {
        self.models.values()
    }

    /// The fallback chain for a task.
    pub fn route(&self, task: Task) -> &[String] {
        self.routes.get(&task).map_or(&[], Vec::as_slice)
    }

    /// A fresh budget account for one job, using `llm.budgets.per_job`.
    pub fn job_budget(&self, name: impl Into<String>) -> Arc<BudgetAccount> {
        BudgetAccount::new(name, self.budgets.per_job)
    }

    /// Complete `req` with the models routed for `task`.
    pub async fn complete(
        &self,
        task: Task,
        req: &CompletionRequest,
        ctx: &CallContext,
    ) -> Result<RoutedResponse, LlmError> {
        let chain = self.route(task).to_vec();
        self.run_chain(&format!("task `{}`", task_name(task)), &chain, req, ctx)
            .await
    }

    /// Complete `req` with one named model, bypassing routes.
    pub async fn complete_with_model(
        &self,
        model: &str,
        req: &CompletionRequest,
        ctx: &CallContext,
    ) -> Result<RoutedResponse, LlmError> {
        if !self.models.contains_key(model) {
            return Err(LlmError::NoModel {
                what: format!("model `{model}`"),
                reason: "no such model in llm.models".into(),
            });
        }
        self.run_chain(&format!("model `{model}`"), &[model.to_owned()], req, ctx)
            .await
    }

    async fn run_chain(
        &self,
        what: &str,
        chain: &[String],
        req: &CompletionRequest,
        ctx: &CallContext,
    ) -> Result<RoutedResponse, LlmError> {
        let candidates: Vec<&ModelEntry> = chain
            .iter()
            .filter_map(|n| self.models.get(n))
            .filter(|m| allowed(ctx.data_egress, m))
            .collect();
        if candidates.is_empty() {
            let reason = if chain.is_empty() {
                "no route is configured".to_owned()
            } else {
                format!(
                    "this repository is `{}` and none of [{}] is a local model",
                    egress_name(ctx.data_egress),
                    chain.join(", ")
                )
            };
            return Err(LlmError::NoModel {
                what: what.to_owned(),
                reason,
            });
        }

        let mut failures = Vec::new();
        for entry in candidates {
            match self.try_model(entry, req, ctx).await {
                Ok(mut r) => {
                    r.failed_attempts = failures;
                    return Ok(r);
                }
                Err(e) if e.is_fatal() => return Err(e),
                Err(e) => {
                    tracing::warn!(model = %entry.name, error = %e, "model failed, trying next");
                    failures.push(format!("{}: {e}", entry.name));
                }
            }
        }
        Err(LlmError::AllFailed {
            what: what.to_owned(),
            attempts: failures,
        })
    }

    async fn try_model(
        &self,
        entry: &ModelEntry,
        req: &CompletionRequest,
        ctx: &CallContext,
    ) -> Result<RoutedResponse, LlmError> {
        let json_mode = entry.tool_mode == ToolMode::Json && !req.tools.is_empty();
        let wire = if json_mode {
            json_tools::to_json_mode(req)
        } else {
            req.clone()
        };

        let est_in = wire.estimated_input_tokens();
        let est_out = u64::from(wire.max_tokens);
        let mut accounts = ctx.budgets.clone();
        accounts.push(self.daily_account());
        let reservation =
            budget::reserve(&accounts, est_in + est_out, entry.cost(est_in, est_out))?;

        let span = tracing::info_span!(
            "gen_ai.chat",
            otel.name = %format!("chat {}", entry.model_id),
            otel.kind = "client",
            gen_ai.operation.name = "chat",
            gen_ai.provider.name = %provider_system(entry.provider.kind()),
            gen_ai.request.model = %entry.model_id,
            gen_ai.request.max_tokens = wire.max_tokens,
            gen_ai.usage.input_tokens = Empty,
            gen_ai.usage.output_tokens = Empty,
            fixloop.model = %entry.name,
            fixloop.provider = %entry.provider.name(),
            fixloop.cost_usd = Empty,
            error.type = Empty,
        );

        let result: Result<(CompletionResponse, Usage), (LlmError, Usage)> = async {
            let first = self
                .call_with_retry(entry, &wire)
                .await
                .map_err(|e| (e, Usage::default()))?;
            if !json_mode {
                let usage = first.usage;
                return Ok((first, usage));
            }
            match json_tools::parse_json_mode(&first, &req.tools, 0) {
                Ok(r) => Ok((r, first.usage)),
                Err(problem) => {
                    let mut again = wire.clone();
                    again
                        .messages
                        .extend(json_tools::correction(&first.text(), &problem));
                    let second = self
                        .call_with_retry(entry, &again)
                        .await
                        .map_err(|e| (e, first.usage))?;
                    let usage = Usage {
                        input_tokens: first.usage.input_tokens + second.usage.input_tokens,
                        output_tokens: first.usage.output_tokens + second.usage.output_tokens,
                    };
                    json_tools::parse_json_mode(&second, &req.tools, 0)
                        .map(|r| (r, usage))
                        .map_err(|p| (LlmError::InvalidToolCall(p), usage))
                }
            }
        }
        .instrument(span.clone())
        .await;

        match result {
            Ok((mut response, usage)) => {
                response.usage = usage;
                let cost = entry.cost(usage.input_tokens, usage.output_tokens);
                reservation.commit(usage.total(), cost);
                span.record("gen_ai.usage.input_tokens", usage.input_tokens);
                span.record("gen_ai.usage.output_tokens", usage.output_tokens);
                span.record("fixloop.cost_usd", cost);
                Ok(RoutedResponse {
                    response,
                    model: entry.name.clone(),
                    model_id: entry.model_id.clone(),
                    provider: entry.provider.name().to_owned(),
                    cost_usd: cost,
                    failed_attempts: Vec::new(),
                })
            }
            Err((e, usage)) => {
                // Charge what was actually consumed before the failure.
                reservation.commit(
                    usage.total(),
                    entry.cost(usage.input_tokens, usage.output_tokens),
                );
                span.record("error.type", error_type(&e));
                Err(e)
            }
        }
    }

    async fn call_with_retry(
        &self,
        entry: &ModelEntry,
        req: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError> {
        let mut delay = Duration::from_millis(self.retry.initial_backoff_ms);
        let mut attempt = 0;
        loop {
            match entry.provider.complete(&entry.model_id, req).await {
                Ok(r) => return Ok(r),
                Err(e) if e.is_retryable() && attempt < self.retry.max_retries => {
                    attempt += 1;
                    tracing::debug!(model = %entry.name, attempt, error = %e, "retrying");
                    tokio::time::sleep(delay).await;
                    delay = delay.saturating_mul(2);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn daily_account(&self) -> Arc<BudgetAccount> {
        let mut g = self.daily.lock().unwrap_or_else(|e| e.into_inner());
        let day = today();
        if g.0 != day {
            *g = (day, BudgetAccount::new("daily", self.budgets.daily));
        }
        Arc::clone(&g.1)
    }
}

fn build_provider(
    name: &str,
    p: &ProviderConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(Arc<dyn LlmProvider>, Url), LlmError> {
    let default_base = match p.kind {
        ProviderKind::Anthropic => Some(anthropic::DEFAULT_BASE_URL),
        ProviderKind::Openai => Some(openai::DEFAULT_BASE_URL),
        _ => None,
    };
    let base = p
        .effective_base_url()
        .or_else(|| default_base.and_then(|u| Url::parse(u).ok()))
        .ok_or_else(|| LlmError::Config {
            message: format!("provider `{name}` has no endpoint"),
            hint: "set base_url (or a preset for openai_compatible)".into(),
        })?;

    let api_key = match &p.api_key_env {
        Some(var) => Some(
            env(var)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| LlmError::Config {
                    message: format!(
                        "environment variable `{var}` (API key for `{name}`) is not set"
                    ),
                    hint: format!("export {var}=<your key>, or remove provider `{name}`"),
                })?,
        ),
        None => None,
    };

    let http = HttpClient::new(
        name,
        EgressGuard::new([&base]),
        Duration::from_secs(p.timeout_secs),
    )?;
    let provider: Arc<dyn LlmProvider> = match p.kind {
        ProviderKind::Anthropic => {
            let key = api_key.ok_or_else(|| missing_key(name, "ANTHROPIC_API_KEY"))?;
            Arc::new(anthropic::Anthropic::new(name, &base, key, http)?)
        }
        ProviderKind::Openai => {
            let key = api_key.ok_or_else(|| missing_key(name, "OPENAI_API_KEY"))?;
            Arc::new(openai::OpenAi::new(name, p.kind, &base, Some(key), http)?)
        }
        ProviderKind::OpenaiCompatible => {
            Arc::new(openai::OpenAi::new(name, p.kind, &base, api_key, http)?)
        }
        other => {
            return Err(LlmError::Config {
                message: format!(
                    "provider kind `{other:?}` (provider `{name}`) is not implemented yet"
                ),
                hint: "it is planned for a later release; Gemini and Azure OpenAI can be used now \
                       through `kind: openai_compatible` with their OpenAI-compatible base_url"
                    .into(),
            });
        }
    };
    Ok((provider, base))
}

fn missing_key(name: &str, example: &str) -> LlmError {
    LlmError::Config {
        message: format!("provider `{name}` needs an API key"),
        hint: format!("set api_key_env to the variable holding it, e.g. api_key_env: {example}"),
    }
}

fn allowed(egress: DataEgress, m: &ModelEntry) -> bool {
    match egress {
        DataEgress::Any => true,
        DataEgress::LocalOnly => m.provider.locality() == Locality::Local,
    }
}

fn today() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400)
}

fn task_name(task: Task) -> String {
    serde_json::to_value(task)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{task:?}"))
}

fn egress_name(e: DataEgress) -> &'static str {
    match e {
        DataEgress::Any => "any",
        DataEgress::LocalOnly => "local-only",
    }
}

/// `gen_ai.provider.name` values from the OpenTelemetry GenAI conventions.
fn provider_system(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::Openai | ProviderKind::OpenaiCompatible => "openai",
        ProviderKind::Gemini => "gcp.gemini",
        ProviderKind::AzureOpenai => "azure.ai.openai",
        ProviderKind::Bedrock => "aws.bedrock",
        ProviderKind::Vertex => "gcp.vertex_ai",
    }
}

fn error_type(e: &LlmError) -> &'static str {
    match e {
        LlmError::Http { .. } => "http",
        LlmError::Timeout { .. } => "timeout",
        LlmError::Transport { .. } => "transport",
        LlmError::Decode { .. } => "decode",
        LlmError::InvalidToolCall(_) => "invalid_tool_call",
        LlmError::EgressBlocked { .. } => "egress_blocked",
        LlmError::BudgetExceeded { .. } => "budget_exceeded",
        LlmError::NoModel { .. } => "no_model",
        LlmError::AllFailed { .. } => "all_failed",
        LlmError::Config { .. } => "config",
    }
}
