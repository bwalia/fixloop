//! Error types. Every user-facing error explains what went wrong and,
//! through [`Hinted`], what to do next.

use std::path::PathBuf;

/// An error that can suggest a next step to the user.
pub trait Hinted {
    /// A short, actionable suggestion, if one is available.
    fn hint(&self) -> Option<String>;
}

/// Errors raised while loading or validating configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config file could not be read.
    #[error("could not read config file `{path}`: {source}")]
    Read {
        /// File that failed to load.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The config file is not valid YAML or does not match the schema.
    #[error("could not parse `{path}`: {message}")]
    Parse {
        /// File that failed to parse.
        path: PathBuf,
        /// Parser message, including location where available.
        message: String,
    },

    /// Merging file and environment layers failed (wrong type, unknown key, ...).
    #[error("invalid configuration: {0}")]
    Extract(String),

    /// A value parsed correctly but is not allowed.
    #[error("invalid value for `{key}`: {problem}")]
    Invalid {
        /// Dotted path of the offending key, e.g. `llm.routes.review`.
        key: String,
        /// What is wrong with it.
        problem: String,
        /// What to do about it.
        hint: String,
    },
}

impl ConfigError {
    pub(crate) fn invalid(
        key: impl Into<String>,
        problem: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self::Invalid {
            key: key.into(),
            problem: problem.into(),
            hint: hint.into(),
        }
    }
}

impl Hinted for ConfigError {
    fn hint(&self) -> Option<String> {
        match self {
            Self::Read { path, .. } => Some(format!(
                "check that `{}` exists and is readable, or pass a different file with --config",
                path.display()
            )),
            Self::Parse { .. } => Some(
                "fix the YAML syntax at the reported line; see docs/config-reference.md".into(),
            ),
            Self::Extract(_) => Some(
                "check key names and value types against docs/config-reference.md; \
                 environment overrides use FIXLOOP_<SECTION>__<KEY>"
                    .into(),
            ),
            Self::Invalid { hint, .. } => Some(hint.clone()),
        }
    }
}
