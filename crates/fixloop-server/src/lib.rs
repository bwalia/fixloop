//! The Fixloop server: webhooks in, durable jobs, workers that run agents.
//!
//! * `POST /webhooks/github`: verified with the App's webhook secret, turned
//!   into a durable job (deduplicated by delivery id), answered at once.
//! * `GET /healthz`: the process is up.
//! * `GET /readyz`: the database answers.
//! * Workers lease jobs, heartbeat while working, and record results.

pub mod http;
pub mod worker;

use std::sync::Arc;

use fixloop_llm::Router;
use fixloop_scm::github::client::GithubApp;
use fixloop_store::Store;

/// Shared state for handlers and workers.
#[derive(Clone)]
pub struct AppState {
    /// Database and queue.
    pub store: Store,
    /// GitHub App, if configured.
    pub github: Option<GithubApp>,
    /// Webhook secret.
    pub webhook_secret: Option<Arc<Vec<u8>>>,
    /// Model router.
    pub router: Arc<Router>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("github", &self.github.is_some())
            .finish_non_exhaustive()
    }
}
