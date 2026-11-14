//! Errors from model calls.

use fixloop_core::Hinted;

/// An error from the model layer.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// The provider answered with an HTTP error.
    #[error("{provider} returned HTTP {status}: {message}")]
    Http {
        /// Provider name from config.
        provider: String,
        /// HTTP status code.
        status: u16,
        /// Error message from the provider, if any.
        message: String,
    },

    /// The request timed out.
    #[error("{provider} did not answer in time")]
    Timeout {
        /// Provider name from config.
        provider: String,
    },

    /// The connection failed.
    #[error("could not reach {provider}: {message}")]
    Transport {
        /// Provider name from config.
        provider: String,
        /// Underlying error.
        message: String,
    },

    /// The provider's answer could not be understood.
    #[error("unexpected response from {provider}: {message}")]
    Decode {
        /// Provider name from config.
        provider: String,
        /// What was wrong.
        message: String,
    },

    /// The model's tool call was malformed (JSON tool mode, or bad arguments).
    #[error("model produced an invalid tool call: {0}")]
    InvalidToolCall(String),

    /// A connection to a host outside the configured endpoints was refused.
    #[error("blocked connection to `{host}`: it is not a configured model endpoint")]
    EgressBlocked {
        /// The refused host.
        host: String,
    },

    /// A token or cost budget would be exceeded.
    #[error("budget `{scope}` exceeded: {detail}")]
    BudgetExceeded {
        /// Budget name, e.g. `job` or `daily`.
        scope: String,
        /// Limit and usage.
        detail: String,
    },

    /// No model can serve the request.
    #[error("no model available for {what}: {reason}")]
    NoModel {
        /// Task or model asked for.
        what: String,
        /// Why nothing is available.
        reason: String,
    },

    /// Every model in the fallback chain failed.
    #[error("all models failed for {what}: {}", attempts.join("; "))]
    AllFailed {
        /// Task or model asked for.
        what: String,
        /// One line per failed attempt.
        attempts: Vec<String>,
    },

    /// The model configuration cannot be used.
    #[error("{message}")]
    Config {
        /// What is wrong.
        message: String,
        /// What to do.
        hint: String,
    },
}

impl LlmError {
    /// Whether retrying the same model may succeed.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { status, .. } => *status == 429 || *status >= 500,
            Self::Timeout { .. } | Self::Transport { .. } => true,
            _ => false,
        }
    }

    /// Whether the router should stop instead of trying the next model.
    pub(crate) fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::BudgetExceeded { .. } | Self::EgressBlocked { .. }
        )
    }
}

impl Hinted for LlmError {
    fn hint(&self) -> Option<String> {
        Some(match self {
            Self::Http {
                status: 401 | 403, ..
            } => "check the API key in the environment variable named by api_key_env".into(),
            Self::Http { status: 404, .. } => {
                "check the model id under llm.models and the provider base_url".into()
            }
            Self::Http { status: 429, .. } => {
                "the provider is rate limiting; add a fallback model to the route or retry later"
                    .into()
            }
            Self::Http { .. } | Self::Decode { .. } => {
                "check the provider's status page, or add a fallback model to the route".into()
            }
            Self::Timeout { .. } => {
                "raise timeout_secs for this provider (local models can be slow)".into()
            }
            Self::Transport { .. } => {
                "check that the endpoint is running and base_url is correct".into()
            }
            Self::InvalidToolCall(_) => {
                "this model may not support tools well; try `tool_mode: json` or a stronger model"
                    .into()
            }
            Self::EgressBlocked { .. } => {
                "add the provider to llm.providers if this endpoint should be allowed".into()
            }
            Self::BudgetExceeded { .. } => "raise the limit under llm.budgets, or wait".into(),
            Self::NoModel { .. } => {
                "add a route under llm.routes; for local-only repos, include a local model".into()
            }
            Self::AllFailed { .. } => "see each attempt's error above".into(),
            Self::Config { hint, .. } => hint.clone(),
        })
    }
}
