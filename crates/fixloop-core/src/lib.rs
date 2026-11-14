//! Core domain model, configuration schema and error types for Fixloop.
//!
//! This crate performs no network I/O. Other crates build on the types and
//! traits defined here; vendor-specific code lives in adapter crates.

pub mod config;
pub mod domain;
pub mod error;
pub mod repo_config;

pub use config::FixloopConfig;
pub use error::{ConfigError, Hinted};
pub use repo_config::RepoConfig;
