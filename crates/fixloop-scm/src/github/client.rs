//! GitHub REST client implementing [`ScmProvider`] for one App installation.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fixloop_core::scm::{
    CheckConclusion, PullRequest, RepoRef, ReviewSubmission, ScmError, ScmProvider,
};
use reqwest::Method;
use serde_json::{Value, json};
use url::Url;

use crate::github::auth::AppAuth;

/// Errors from the GitHub client.
#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    /// GitHub returned an error.
    #[error("GitHub API returned HTTP {status}: {message}")]
    Api {
        /// HTTP status.
        status: u16,
        /// Message.
        message: String,
    },
    /// Network failure.
    #[error("could not reach GitHub: {0}")]
    Transport(String),
    /// Unexpected response.
    #[error("unexpected response from GitHub: {0}")]
    Decode(String),
    /// Authentication problem.
    #[error("{0}")]
    Auth(String),
    /// Bad input.
    #[error("{0}")]
    Invalid(String),
}

impl From<GithubError> for ScmError {
    fn from(e: GithubError) -> Self {
        match e {
            GithubError::Api {
                status: 404,
                message,
            } => ScmError::NotFound(message),
            GithubError::Api { status, message } => ScmError::Api {
                host: "GitHub",
                status,
                message,
            },
            GithubError::Transport(m) => ScmError::Transport(m),
            GithubError::Decode(m) => ScmError::Decode(m),
            GithubError::Auth(m) | GithubError::Invalid(m) => ScmError::Auth(m),
        }
    }
}

const ACCEPT_JSON: &str = "application/vnd.github+json";
const API_VERSION: &str = "2022-11-28";
/// GitHub limits a check-run summary to 65535 characters.
const MAX_SUMMARY: usize = 65_000;

/// Entry point for one GitHub App: finds installations and builds clients.
#[derive(Debug, Clone)]
pub struct GithubApp {
    auth: Arc<AppAuth>,
    api: Url,
    http: reqwest::Client,
}

impl GithubApp {
    /// Create from App credentials and the API root.
    pub fn new(auth: AppAuth, api: Url) -> Result<Self, GithubError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("fixloop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| GithubError::Transport(e.to_string()))?;
        let mut api = api;
        if !api.path().ends_with('/') {
            let p = format!("{}/", api.path());
            api.set_path(&p);
        }
        Ok(Self {
            auth: Arc::new(auth),
            api,
            http,
        })
    }

    /// A client for a known installation.
    pub fn installation(&self, installation_id: u64) -> GithubClient {
        GithubClient {
            app: self.clone(),
            installation_id,
        }
    }

    /// Find the installation for a repository and return a client for it.
    pub async fn for_repo(&self, repo: &RepoRef) -> Result<GithubClient, GithubError> {
        let jwt = self.auth.jwt()?;
        let v = send(
            &self.http,
            &self.api,
            Method::GET,
            &format!("repos/{repo}/installation"),
            &jwt,
            ACCEPT_JSON,
            None,
        )
        .await
        .map_err(|e| match e {
            GithubError::Api { status: 404, .. } => GithubError::Api {
                status: 404,
                message: format!("the Fixloop GitHub App is not installed on {repo}"),
            },
            other => other,
        })?;
        let id = json_value(&v)?
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| GithubError::Decode("installation without id".into()))?;
        Ok(self.installation(id))
    }
}

/// A client acting as one App installation.
#[derive(Debug, Clone)]
pub struct GithubClient {
    app: GithubApp,
    installation_id: u64,
}

impl GithubClient {
    /// The installation this client acts as.
    pub fn installation_id(&self) -> u64 {
        self.installation_id
    }

    async fn token(&self) -> Result<String, GithubError> {
        if let Some(t) = self.app.auth.cached_token(self.installation_id) {
            return Ok(t);
        }
        let jwt = self.app.auth.jwt()?;
        let path = format!("app/installations/{}/access_tokens", self.installation_id);
        let body = send(
            &self.app.http,
            &self.app.api,
            Method::POST,
            &path,
            &jwt,
            ACCEPT_JSON,
            None,
        )
        .await?;
        let token = json_value(&body)?
            .get("token")
            .and_then(Value::as_str)
            .ok_or_else(|| GithubError::Decode("access token response without token".into()))?
            .to_owned();
        self.app
            .auth
            .store_token(self.installation_id, token.clone());
        Ok(token)
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        accept: &str,
        body: Option<Value>,
    ) -> Result<String, GithubError> {
        let token = self.token().await?;
        send(
            &self.app.http,
            &self.app.api,
            method,
            path,
            &token,
            accept,
            body,
        )
        .await
    }

    async fn call_json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, GithubError> {
        json_value(&self.call(method, path, ACCEPT_JSON, body).await?)
    }
}

async fn send(
    http: &reqwest::Client,
    api: &Url,
    method: Method,
    path: &str,
    bearer: &str,
    accept: &str,
    body: Option<Value>,
) -> Result<String, GithubError> {
    let url = api
        .join(path)
        .map_err(|e| GithubError::Invalid(format!("bad path {path}: {e}")))?;
    let mut req = http
        .request(method, url)
        .bearer_auth(bearer)
        .header("accept", accept)
        .header("x-github-api-version", API_VERSION);
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| GithubError::Transport(e.to_string()))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| GithubError::Transport(e.to_string()))?;
    if status.is_success() {
        Ok(text)
    } else {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                let msg = v.get("message")?.as_str()?.to_owned();
                Some(match v.get("errors") {
                    Some(errs) => format!("{msg} ({errs})"),
                    None => msg,
                })
            })
            .unwrap_or(text);
        Err(GithubError::Api {
            status: status.as_u16(),
            message,
        })
    }
}

fn json_value(text: &str) -> Result<Value, GithubError> {
    serde_json::from_str(text).map_err(|e| GithubError::Decode(e.to_string()))
}

fn str_at(v: &Value, ptr: &str) -> Result<String, GithubError> {
    v.pointer(ptr)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| GithubError::Decode(format!("missing {ptr}")))
}

/// Percent-encode each path segment (keeps `/`).
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            seg.bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        (b as char).to_string()
                    }
                    _ => format!("%{b:02X}"),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_ref(r: &str) -> String {
    encode_path(r).replace('/', "%2F")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n\n…(truncated)", &s[..end])
}

#[async_trait]
impl ScmProvider for GithubClient {
    async fn pull_request(&self, repo: &RepoRef, number: u64) -> Result<PullRequest, ScmError> {
        let v = self
            .call_json(Method::GET, &format!("repos/{repo}/pulls/{number}"), None)
            .await?;
        Ok(PullRequest {
            repo: repo.clone(),
            number,
            title: str_at(&v, "/title")?,
            body: v
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            author: str_at(&v, "/user/login")?,
            head_sha: str_at(&v, "/head/sha")?,
            base_sha: str_at(&v, "/base/sha")?,
            base_ref: str_at(&v, "/base/ref")?,
            draft: v.get("draft").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    async fn pull_request_diff(&self, repo: &RepoRef, number: u64) -> Result<String, ScmError> {
        Ok(self
            .call(
                Method::GET,
                &format!("repos/{repo}/pulls/{number}"),
                "application/vnd.github.diff",
                None,
            )
            .await?)
    }

    async fn file_at(
        &self,
        repo: &RepoRef,
        path: &str,
        git_ref: &str,
    ) -> Result<Option<String>, ScmError> {
        let p = format!(
            "repos/{repo}/contents/{}?ref={}",
            encode_path(path),
            encode_ref(git_ref)
        );
        match self
            .call(Method::GET, &p, "application/vnd.github.raw+json", None)
            .await
        {
            Ok(text) => Ok(Some(text)),
            Err(GithubError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_dir(
        &self,
        repo: &RepoRef,
        path: &str,
        git_ref: &str,
    ) -> Result<Vec<String>, ScmError> {
        let p = format!(
            "repos/{repo}/contents/{}?ref={}",
            encode_path(path),
            encode_ref(git_ref)
        );
        match self.call_json(Method::GET, &p, None).await {
            Ok(Value::Array(items)) => Ok(items
                .iter()
                .filter(|i| i.get("type").and_then(Value::as_str) == Some("file"))
                .filter_map(|i| i.get("name").and_then(Value::as_str).map(str::to_owned))
                .collect()),
            Ok(_) => Ok(Vec::new()),
            Err(GithubError::Api { status: 404, .. }) => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    async fn start_check(
        &self,
        repo: &RepoRef,
        head_sha: &str,
        name: &str,
    ) -> Result<u64, ScmError> {
        let v = self
            .call_json(
                Method::POST,
                &format!("repos/{repo}/check-runs"),
                Some(json!({"name": name, "head_sha": head_sha, "status": "in_progress"})),
            )
            .await?;
        v.get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| ScmError::Decode("check run without id".into()))
    }

    async fn finish_check(
        &self,
        repo: &RepoRef,
        check_id: u64,
        conclusion: CheckConclusion,
        title: &str,
        summary: &str,
    ) -> Result<(), ScmError> {
        let conclusion = match conclusion {
            CheckConclusion::Success => "success",
            CheckConclusion::Neutral => "neutral",
            CheckConclusion::Failure => "failure",
        };
        self.call_json(
            Method::PATCH,
            &format!("repos/{repo}/check-runs/{check_id}"),
            Some(json!({
                "status": "completed",
                "conclusion": conclusion,
                "output": {"title": truncate(title, 200), "summary": truncate(summary, MAX_SUMMARY)}
            })),
        )
        .await?;
        Ok(())
    }

    async fn post_review(
        &self,
        repo: &RepoRef,
        number: u64,
        review: &ReviewSubmission,
    ) -> Result<(), ScmError> {
        let comments: Vec<Value> = review
            .comments
            .iter()
            .map(|c| {
                let mut v =
                    json!({"path": c.path, "line": c.line, "side": "RIGHT", "body": c.body});
                if let Some(start) = c.start_line.filter(|s| *s < c.line) {
                    v["start_line"] = json!(start);
                    v["start_side"] = json!("RIGHT");
                }
                v
            })
            .collect();
        let path = format!("repos/{repo}/pulls/{number}/reviews");
        let body = json!({"commit_id": review.commit_sha, "event": "COMMENT", "body": review.body, "comments": comments});
        match self.call_json(Method::POST, &path, Some(body)).await {
            Ok(_) => Ok(()),
            // If GitHub rejects an inline position (the PR moved on, say), post
            // the findings as one review body rather than losing them.
            Err(GithubError::Api {
                status: 422,
                message,
            }) if !review.comments.is_empty() => {
                tracing::warn!(%message, "inline review rejected; posting findings in the review body");
                let mut text = review.body.clone();
                for c in &review.comments {
                    text.push_str(&format!(
                        "\n\n---\n**`{}` line {}**\n\n{}",
                        c.path, c.line, c.body
                    ));
                }
                self.call_json(
                    Method::POST,
                    &path,
                    Some(json!({"commit_id": review.commit_sha, "event": "COMMENT", "body": truncate(&text, MAX_SUMMARY)})),
                )
                .await?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn post_comment(&self, repo: &RepoRef, number: u64, body: &str) -> Result<(), ScmError> {
        self.call_json(
            Method::POST,
            &format!("repos/{repo}/issues/{number}/comments"),
            Some(json!({"body": truncate(body, MAX_SUMMARY)})),
        )
        .await?;
        Ok(())
    }

    async fn can_write(&self, repo: &RepoRef, user: &str) -> Result<bool, ScmError> {
        let p = format!(
            "repos/{repo}/collaborators/{}/permission",
            encode_path(user)
        );
        match self.call_json(Method::GET, &p, None).await {
            Ok(v) => Ok(matches!(
                v.get("permission").and_then(Value::as_str),
                Some("admin" | "maintain" | "write")
            )),
            Err(GithubError::Api { status: 404, .. }) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_encoding() {
        assert_eq!(encode_path("src/a b.rs"), "src/a%20b.rs");
        assert_eq!(encode_path(".fixloop/rules"), ".fixloop/rules");
        assert_eq!(encode_ref("feature/x"), "feature%2Fx");
    }
}
