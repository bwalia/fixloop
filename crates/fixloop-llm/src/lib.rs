//! Model access for Fixloop.
//!
//! Everything else in Fixloop talks to models through [`Router`], never to a
//! vendor API directly. The router:
//!
//! * picks models per [`Task`](fixloop_core::config::Task) from config (no
//!   model names are hard-coded),
//! * walks an ordered fallback chain with retries,
//! * enforces per-repository data-egress rules (`local-only`),
//! * meters token and cost budgets,
//! * only ever connects to the endpoints in config (see [`egress`]), and
//! * offers tools natively or through a JSON fallback for models without
//!   native tool calling.

pub mod budget;
pub mod egress;
pub mod error;
pub mod json_tools;
pub mod provider;
pub mod providers;
pub mod router;
pub mod types;

pub use budget::BudgetAccount;
pub use error::LlmError;
pub use provider::LlmProvider;
pub use router::{CallContext, RoutedResponse, Router};
pub use types::{
    CompletionRequest, CompletionResponse, Message, Part, Role, StopReason, ToolCall, ToolSpec,
    Usage,
};
