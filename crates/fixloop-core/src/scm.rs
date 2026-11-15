//! The source-code-host interface the agents use. GitHub implements it in
//! `fixloop-scm`; tests use an in-memory fake.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Hinted;

/// `owner/name` of a repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoRef {
    /// Owner (user or organisation).
    pub owner: String,
    /// Repository name.
    pub name: String,
}

impl RepoRef {
    /// Parse `owner/name`.
    pub fn parse(s: &str) -> Option<Self> {
        let (owner, name) = s.split_once('/')?;
        let ok = |p: &str| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        (ok(owner) && ok(name) && name != "." && name != "..").then(|| Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl std::fmt::Display for RepoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

/// A pull request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullRequest {
    /// Repository.
    pub repo: RepoRef,
    /// PR number.
    pub number: u64,
    /// Title (untrusted).
    pub title: String,
    /// Description (untrusted).
    pub body: String,
    /// Author login.
    pub author: String,
    /// Head commit.
    pub head_sha: String,
    /// Base commit.
    pub base_sha: String,
    /// Base branch name.
    pub base_ref: String,
    /// Whether the PR is a draft.
    pub draft: bool,
}

/// Outcome of a check run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckConclusion {
    /// Pass.
    Success,
    /// Warn.
    Neutral,
    /// Fail.
    Failure,
}

/// An inline review comment on the new version of a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewComment {
    /// File path.
    pub path: String,
    /// Last line the comment covers (new-file numbering).
    pub line: u32,
    /// First line, for multi-line comments.
    pub start_line: Option<u32>,
    /// Markdown body.
    pub body: String,
}

/// A review to post on a pull request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewSubmission {
    /// Commit the comments refer to.
    pub commit_sha: String,
    /// Summary body.
    pub body: String,
    /// Inline comments.
    pub comments: Vec<ReviewComment>,
}

/// Errors from a code host.
#[derive(Debug, thiserror::Error)]
pub enum ScmError {
    /// The host returned an error.
    #[error("{host} API returned HTTP {status}: {message}")]
    Api {
        /// Host name, e.g. `GitHub`.
        host: &'static str,
        /// HTTP status.
        status: u16,
        /// Message from the host.
        message: String,
    },
    /// Not found, or no access.
    #[error("{0} was not found, or the app has no access to it")]
    NotFound(String),
    /// Network failure.
    #[error("could not reach the code host: {0}")]
    Transport(String),
    /// Unexpected response.
    #[error("unexpected response from the code host: {0}")]
    Decode(String),
    /// Authentication failed.
    #[error("authentication with the code host failed: {0}")]
    Auth(String),
}

impl Hinted for ScmError {
    fn hint(&self) -> Option<String> {
        Some(match self {
            Self::Api { status: 401, .. } | Self::Auth(_) => {
                "check github.app_id and github.private_key_path".into()
            }
            Self::Api { status: 403, .. } => {
                "the app may lack a permission; check its permissions and accept any pending permission requests on the installation".into()
            }
            Self::Api { status: 406 | 422, .. } => {
                "the request was rejected; for very large pull requests GitHub refuses to produce a diff".into()
            }
            Self::NotFound(_) => "install the GitHub App on this repository".into(),
            Self::Api { .. } | Self::Transport(_) | Self::Decode(_) => {
                "retry; check https://www.githubstatus.com if it persists".into()
            }
        })
    }
}

/// What the agents need from a code host.
#[async_trait]
pub trait ScmProvider: Send + Sync {
    /// Fetch a pull request.
    async fn pull_request(&self, repo: &RepoRef, number: u64) -> Result<PullRequest, ScmError>;
    /// Fetch a pull request's unified diff.
    async fn pull_request_diff(&self, repo: &RepoRef, number: u64) -> Result<String, ScmError>;
    /// Read a file at a commit or branch. `Ok(None)` if it does not exist.
    async fn file_at(
        &self,
        repo: &RepoRef,
        path: &str,
        git_ref: &str,
    ) -> Result<Option<String>, ScmError>;
    /// List file names in a directory. Empty if it does not exist.
    async fn list_dir(
        &self,
        repo: &RepoRef,
        path: &str,
        git_ref: &str,
    ) -> Result<Vec<String>, ScmError>;
    /// Start a check run on a commit; returns its id.
    async fn start_check(
        &self,
        repo: &RepoRef,
        head_sha: &str,
        name: &str,
    ) -> Result<u64, ScmError>;
    /// Complete a check run.
    async fn finish_check(
        &self,
        repo: &RepoRef,
        check_id: u64,
        conclusion: CheckConclusion,
        title: &str,
        summary: &str,
    ) -> Result<(), ScmError>;
    /// Post a review with inline comments.
    async fn post_review(
        &self,
        repo: &RepoRef,
        number: u64,
        review: &ReviewSubmission,
    ) -> Result<(), ScmError>;
    /// Post a plain comment on a pull request.
    async fn post_comment(&self, repo: &RepoRef, number: u64, body: &str) -> Result<(), ScmError>;
    /// Whether `user` has write access (used to authorise `/fixloop` commands).
    async fn can_write(&self, repo: &RepoRef, user: &str) -> Result<bool, ScmError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_ref_parse() {
        assert_eq!(RepoRef::parse("a/b").unwrap().to_string(), "a/b");
        assert!(RepoRef::parse("a").is_none());
        assert!(RepoRef::parse("a/../b").is_none());
        assert!(RepoRef::parse("a/..").is_none());
        assert!(RepoRef::parse("a/b c").is_none());
    }
}
