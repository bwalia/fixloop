//! Per-repository configuration (`.fixloop.yaml` at the repository root).

use serde::{Deserialize, Serialize};

use crate::domain::{DataEgress, RepoMode, Severity};
use crate::error::ConfigError;

/// File name of the per-repository config.
pub const REPO_CONFIG_FILE: &str = ".fixloop.yaml";

/// Current schema version of [`RepoConfig`].
pub const REPO_CONFIG_VERSION: u32 = 1;

/// Settings a repository can declare for itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RepoConfig {
    /// Schema version.
    pub version: u32,
    /// What Fixloop may do in this repository.
    pub mode: RepoMode,
    /// Which model providers may receive this repository's code.
    pub data_egress: DataEgress,
    /// Review settings.
    pub review: ReviewSettings,
    /// Fix settings.
    pub fix: FixSettings,
    /// Directory of plain-text rule files.
    pub rules_dir: String,
}

/// Review settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReviewSettings {
    /// Findings below this confidence (0.0 to 1.0) are not posted.
    pub confidence_threshold: f32,
    /// Findings below this severity are not posted.
    pub min_severity: Severity,
    /// Glob patterns of paths to skip.
    pub ignore_paths: Vec<String>,
}

impl Default for ReviewSettings {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.7,
            min_severity: Severity::Low,
            ignore_paths: Vec::new(),
        }
    }
}

/// Fix settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FixSettings {
    /// How many times to try making build, tests and lint pass before giving up.
    pub max_attempts: u32,
    /// Require a human to approve before a fix PR is opened.
    pub require_approval: bool,
}

impl Default for FixSettings {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            require_approval: true,
        }
    }
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            version: REPO_CONFIG_VERSION,
            mode: RepoMode::default(),
            data_egress: DataEgress::default(),
            review: ReviewSettings::default(),
            fix: FixSettings::default(),
            rules_dir: ".fixloop/rules".into(),
        }
    }
}

impl RepoConfig {
    /// Parse and validate a `.fixloop.yaml` document. `origin` names the
    /// source in error messages.
    pub fn from_yaml(text: &str, origin: &str) -> Result<Self, ConfigError> {
        let config: Option<Self> =
            serde_saphyr::from_str(text).map_err(|e| ConfigError::Parse {
                path: origin.into(),
                message: e.to_string(),
            })?;
        let config = config.unwrap_or_default();
        config.validate()?;
        Ok(config)
    }

    /// Check value ranges.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != REPO_CONFIG_VERSION {
            return Err(ConfigError::invalid(
                "version",
                format!("unsupported version {}", self.version),
                format!("set `version: {REPO_CONFIG_VERSION}`"),
            ));
        }
        let t = self.review.confidence_threshold;
        if !(0.0..=1.0).contains(&t) {
            return Err(ConfigError::invalid(
                "review.confidence_threshold",
                format!("{t} is outside 0.0 to 1.0"),
                "use a value between 0.0 and 1.0; 0.7 is a good start",
            ));
        }
        if self.fix.max_attempts == 0 {
            return Err(ConfigError::invalid(
                "fix.max_attempts",
                "must be at least 1",
                "use 3 (the default) unless you have a reason to change it",
            ));
        }
        Ok(())
    }

    /// The commented template written by `fixloop init`.
    pub fn template() -> &'static str {
        TEMPLATE
    }
}

const TEMPLATE: &str = r#"# Fixloop repository settings. Reference: docs/config-reference.md
version: 1

# What Fixloop may do here:
#   comment-only | suggest | open-fix-pr | auto-push-to-pr-branch
mode: suggest

# Which models may see this code: any | local-only
data_egress: any

review:
  # Only post findings at or above this confidence (0.0 - 1.0).
  confidence_threshold: 0.7
  # info | low | medium | high | critical
  min_severity: low
  ignore_paths:
    - "vendor/**"
    - "**/*.generated.*"

fix:
  # Attempts to make build + tests + lint pass before giving up.
  max_attempts: 3
  # Ask a human before opening a fix PR.
  require_approval: true

# Plain-text team rules, one Markdown file per rule.
rules_dir: .fixloop/rules
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses_and_matches_defaults_where_expected() {
        let c = RepoConfig::from_yaml(RepoConfig::template(), "template").unwrap();
        assert_eq!(c.mode, RepoMode::Suggest);
        assert_eq!(c.fix, FixSettings::default());
        assert_eq!(c.review.ignore_paths.len(), 2);
    }

    #[test]
    fn empty_document_is_default() {
        assert_eq!(
            RepoConfig::from_yaml("", "x").unwrap(),
            RepoConfig::default()
        );
    }

    #[test]
    fn local_only_parses() {
        let c = RepoConfig::from_yaml("data_egress: local-only", "x").unwrap();
        assert_eq!(c.data_egress, DataEgress::LocalOnly);
    }

    #[test]
    fn threshold_out_of_range_is_rejected() {
        let e = RepoConfig::from_yaml("review: { confidence_threshold: 1.5 }", "x").unwrap_err();
        assert!(e.to_string().contains("confidence_threshold"), "{e}");
    }

    #[test]
    fn zero_attempts_rejected() {
        assert!(RepoConfig::from_yaml("fix: { max_attempts: 0 }", "x").is_err());
    }

    #[test]
    fn unknown_mode_rejected() {
        assert!(RepoConfig::from_yaml("mode: yolo", "x").is_err());
    }
}
