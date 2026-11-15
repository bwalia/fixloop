//! Agent errors.

use fixloop_core::scm::ScmError;
use fixloop_core::{ConfigError, Hinted};
use fixloop_llm::LlmError;

/// Why a review could not be completed.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// Model call failed.
    #[error(transparent)]
    Llm(#[from] LlmError),
    /// Code host call failed.
    #[error(transparent)]
    Scm(#[from] ScmError),
    /// The repository's .fixloop.yaml is invalid.
    #[error("the repository's .fixloop.yaml is invalid: {0}")]
    RepoConfig(#[from] ConfigError),
    /// The model did not return findings in the required format.
    #[error("the model's answer could not be used: {0}")]
    NoStructuredOutput(String),
}

impl Hinted for ReviewError {
    fn hint(&self) -> Option<String> {
        match self {
            Self::Llm(e) => e.hint(),
            Self::Scm(e) => e.hint(),
            Self::RepoConfig(e) => e.hint(),
            Self::NoStructuredOutput(_) => Some(
                "use a model with reliable tool calling for the review route, or set `tool_mode: json` for this model".into(),
            ),
        }
    }
}
