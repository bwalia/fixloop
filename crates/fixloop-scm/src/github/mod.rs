//! GitHub App integration: app registration (manifest flow), installation
//! tokens, REST client and webhooks.

pub mod auth;
pub mod client;
pub mod manifest;
pub mod webhook;

pub use auth::AppAuth;
pub use client::GithubClient;
