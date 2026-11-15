//! Persistence for Fixloop on PostgreSQL (teams) or SQLite (single user),
//! including the durable job queue described in ADR-0002.
//!
//! One code path serves both databases through sqlx's `Any` driver. SQL is
//! kept portable; the few backend differences (row locking, id columns) are
//! chosen at runtime.

mod migrate;
pub mod queue;
pub mod reviews;

use std::time::{SystemTime, UNIX_EPOCH};

use fixloop_core::Hinted;
use sqlx::AnyPool;
use sqlx::any::{AnyPoolOptions, install_default_drivers};

/// Database errors.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Could not connect.
    #[error("could not connect to the database `{url}`: {message}")]
    Connect {
        /// URL with any password removed.
        url: String,
        /// Cause.
        message: String,
    },
    /// Unsupported URL scheme.
    #[error("unsupported database URL `{0}`")]
    UnsupportedUrl(String),
    /// A query failed.
    #[error("database error: {0}")]
    Query(#[from] sqlx::Error),
    /// Stored data could not be decoded.
    #[error("corrupt data in the database: {0}")]
    Corrupt(String),
}

impl Hinted for StoreError {
    fn hint(&self) -> Option<String> {
        Some(match self {
            Self::Connect { .. } => "check database.url and that the database is running".into(),
            Self::UnsupportedUrl(_) => "use postgres://… or sqlite://…".into(),
            Self::Query(_) | Self::Corrupt(_) => {
                "check the database logs; this may be a bug".into()
            }
        })
    }
}

/// Which database is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// PostgreSQL.
    Postgres,
    /// SQLite.
    Sqlite,
}

/// A connection pool plus backend info.
#[derive(Debug, Clone)]
pub struct Store {
    pool: AnyPool,
    backend: Backend,
}

impl Store {
    /// Connect and apply migrations.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        install_default_drivers();
        let backend = if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Backend::Postgres
        } else if url.starts_with("sqlite:") {
            Backend::Sqlite
        } else {
            return Err(StoreError::UnsupportedUrl(redact_url(url)));
        };
        let mut opts = AnyPoolOptions::new();
        if backend == Backend::Sqlite {
            // SQLite allows one writer; a single connection avoids lock errors.
            opts = opts.max_connections(1);
        } else {
            opts = opts.max_connections(10);
        }
        let pool = opts.connect(url).await.map_err(|e| StoreError::Connect {
            url: redact_url(url),
            message: e.to_string(),
        })?;
        if backend == Backend::Sqlite {
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA busy_timeout = 5000")
                .execute(&pool)
                .await?;
        }
        let store = Self { pool, backend };
        migrate::run(&store).await?;
        Ok(store)
    }

    /// Backend in use.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Check the database answers (for `/readyz`).
    pub async fn ping(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub(crate) fn pool(&self) -> &AnyPool {
        &self.pool
    }
}

/// Current time in Unix seconds.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Remove the password from a database URL for messages.
pub fn redact_url(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(s), Some(at)) if at > s => {
            let creds = &url[s + 3..at];
            match creds.split_once(':') {
                Some((user, _)) => format!("{}{user}:***{}", &url[..s + 3], &url[at..]),
                None => url.to_owned(),
            }
        }
        _ => url.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn redacts_password() {
        assert_eq!(
            super::redact_url("postgres://u:pw@h/db"),
            "postgres://u:***@h/db"
        );
        assert_eq!(super::redact_url("sqlite://x.db"), "sqlite://x.db");
    }
}
