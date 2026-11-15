//! GitHub App manifest flow: register an App with one click.
//!
//! See <https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest>.

use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use crate::github::client::GithubError;

/// The manifest for a Fixloop App with the minimum permissions the
/// Reviewer needs. Issue filing (Phase 3) will add `issues: write`.
pub fn manifest(name: &str, redirect_url: &Url, webhook_url: Option<&Url>) -> Value {
    json!({
        "name": name,
        "url": "https://github.com/bwalia/fixloop",
        "description": "Fixloop reviews pull requests, files issues and opens test-gated fix PRs.",
        "public": false,
        "redirect_url": redirect_url.as_str(),
        "hook_attributes": {
            // Without a public URL yet, webhooks start disabled. Set the URL
            // and activate them in the App settings once Fixloop is reachable.
            "url": webhook_url.map_or("https://example.com/fixloop/webhooks/github", Url::as_str),
            "active": webhook_url.is_some(),
        },
        "default_permissions": {
            "metadata": "read",
            "contents": "read",
            "pull_requests": "write",
            "checks": "write",
        },
        "default_events": ["pull_request", "issue_comment"],
    })
}

/// Where the user goes to create the App: their account, or an organisation.
pub fn creation_url(org: Option<&str>, state: &str) -> String {
    match org {
        Some(o) => format!("https://github.com/organizations/{o}/settings/apps/new?state={state}"),
        None => format!("https://github.com/settings/apps/new?state={state}"),
    }
}

/// What GitHub returns after the manifest flow. Contains secrets.
#[derive(Deserialize)]
pub struct CreatedApp {
    /// App id.
    pub id: u64,
    /// URL slug.
    pub slug: String,
    /// App page.
    pub html_url: String,
    /// Private key (PEM).
    pub pem: String,
    /// Webhook secret, if GitHub generated one.
    pub webhook_secret: Option<String>,
}

impl std::fmt::Debug for CreatedApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedApp")
            .field("id", &self.id)
            .field("slug", &self.slug)
            .field("pem", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Exchange the one-time code from the redirect for the App's credentials.
pub async fn convert_code(api_url: &Url, code: &str) -> Result<CreatedApp, GithubError> {
    if !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(GithubError::Invalid(
            "manifest code has unexpected characters".into(),
        ));
    }
    let url = api_url
        .join(&format!("app-manifests/{code}/conversions"))
        .map_err(|e| GithubError::Invalid(e.to_string()))?;
    let resp = reqwest::Client::new()
        .post(url)
        .header("accept", "application/vnd.github+json")
        .header("user-agent", concat!("fixloop/", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .map_err(|e| GithubError::Transport(e.to_string()))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| GithubError::Transport(e.to_string()))?;
    if !status.is_success() {
        return Err(GithubError::Api {
            status: status.as_u16(),
            message: text,
        });
    }
    serde_json::from_str(&text).map_err(|e| GithubError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_least_privilege() {
        let m = manifest("fx", &Url::parse("http://127.0.0.1:1/cb").unwrap(), None);
        let perms = m["default_permissions"].as_object().unwrap();
        assert_eq!(perms.len(), 4);
        assert_eq!(perms["contents"], "read");
        assert!(perms.get("administration").is_none());
        assert_eq!(m["hook_attributes"]["active"], false);
        assert_eq!(m["public"], false);
    }

    #[test]
    fn debug_never_prints_key() {
        let a = CreatedApp {
            id: 1,
            slug: "s".into(),
            html_url: String::new(),
            pem: "-----BEGIN RSA PRIVATE KEY-----secret".into(),
            webhook_secret: Some("w".into()),
        };
        let d = format!("{a:?}");
        assert!(!d.contains("secret") || d.contains("<redacted>"));
        assert!(!d.contains("BEGIN"));
    }
}
