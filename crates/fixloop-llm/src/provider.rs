//! The provider trait.

use async_trait::async_trait;
use fixloop_core::config::ProviderKind;
use fixloop_core::domain::Locality;

use crate::error::LlmError;
use crate::types::{CompletionRequest, CompletionResponse};

/// A model API. Implementations translate the neutral request types to one
/// vendor's wire format. They do not retry or fall back; the router does.
#[async_trait]
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    /// Provider name from config.
    fn name(&self) -> &str;

    /// API family.
    fn kind(&self) -> ProviderKind;

    /// Whether the provider is self-hosted or a third-party API.
    fn locality(&self) -> Locality {
        self.kind().locality()
    }

    /// Run one completion against `model`.
    async fn complete(
        &self,
        model: &str,
        req: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError>;
}
