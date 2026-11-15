//! Core domain vocabulary shared by every agent and adapter.

use serde::{Deserialize, Serialize};

/// How severe a finding is. Ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational note, no action required.
    Info,
    /// Minor issue.
    Low,
    /// Should be fixed.
    Medium,
    /// Likely to cause incorrect behaviour or a vulnerability.
    High,
    /// Must be fixed before merging.
    Critical,
}

/// What kind of problem a finding describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Functional defect.
    Bug,
    /// Security weakness or vulnerability.
    Security,
    /// Performance problem.
    Performance,
    /// Style or maintainability issue (code smell).
    Style,
    /// Missing or inadequate tests.
    TestGap,
}

/// How much a repository allows Fixloop to change. Each mode includes the
/// abilities of the ones before it.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum RepoMode {
    /// Post review comments only.
    CommentOnly,
    /// Comments plus suggested changes the author can apply.
    #[default]
    Suggest,
    /// May open separate fix pull requests.
    OpenFixPr,
    /// May push commits to the pull request branch.
    AutoPushToPrBranch,
}

impl RepoMode {
    /// Whether this mode allows Fixloop to create branches and fix PRs.
    pub fn can_open_fix_pr(self) -> bool {
        self >= Self::OpenFixPr
    }

    /// Whether this mode allows pushing to an existing PR branch.
    pub fn can_push_to_pr_branch(self) -> bool {
        self >= Self::AutoPushToPrBranch
    }
}

/// Which model providers may receive code from a repository.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DataEgress {
    /// Any configured provider.
    #[default]
    Any,
    /// Only self-hosted, local models (OpenAI-compatible endpoints such as Ollama or vLLM).
    LocalOnly,
}

/// Where a model provider runs, relative to the Fixloop deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Locality {
    /// Run by the organisation (Ollama, vLLM, llama.cpp, NIM, ...).
    Local,
    /// A third-party API.
    External,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_low_to_high() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::Info < Severity::Low);
    }

    #[test]
    fn modes_are_cumulative() {
        assert!(!RepoMode::Suggest.can_open_fix_pr());
        assert!(RepoMode::OpenFixPr.can_open_fix_pr());
        assert!(!RepoMode::OpenFixPr.can_push_to_pr_branch());
        assert!(RepoMode::AutoPushToPrBranch.can_push_to_pr_branch());
    }

    #[test]
    fn default_mode_is_suggest() {
        assert_eq!(RepoMode::default(), RepoMode::Suggest);
    }
}

/// A problem found in code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// One-line title.
    pub title: String,
    /// Severity.
    pub severity: Severity,
    /// Kind of problem.
    pub category: Category,
    /// File path.
    pub path: String,
    /// Line (new-file numbering). For multi-line findings, the first line.
    pub line: u32,
    /// Last line, for multi-line findings.
    pub end_line: Option<u32>,
    /// Why it matters.
    pub rationale: String,
    /// How to fix it, in words.
    pub suggested_fix: Option<String>,
    /// Exact replacement for lines `line..=end_line`, if one is certain.
    pub suggestion: Option<String>,
    /// Confidence from 0.0 to 1.0.
    pub confidence: f32,
    /// Stable identity across line shifts, used for dedupe.
    pub fingerprint: String,
}

impl Category {
    /// The kebab-case name used in config and output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bug => "bug",
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Style => "style",
            Self::TestGap => "test-gap",
        }
    }
}

impl Severity {
    /// The lowercase name used in config and output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}
