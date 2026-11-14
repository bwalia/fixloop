//! Outbound connection control for model calls.
//!
//! Every model request goes through [`HttpClient`], which refuses any
//! endpoint that is not in the allow list built from `llm.providers`.
//! Redirects are disabled, so a configured endpoint cannot bounce a request
//! to another host. With `llm.offline: true`, config validation already
//! limits providers to self-hosted endpoints; this guard is the second layer.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value;
use url::Url;

use crate::error::LlmError;

/// The set of `host:port` pairs model calls may reach.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EgressGuard {
    allowed: BTreeSet<String>,
}

impl EgressGuard {
    /// A guard that allows exactly the given endpoints.
    pub fn new<'a>(endpoints: impl IntoIterator<Item = &'a Url>) -> Self {
        Self {
            allowed: endpoints.into_iter().filter_map(host_port).collect(),
        }
    }

    /// The allowed `host:port` pairs.
    pub fn allowed(&self) -> impl Iterator<Item = &str> {
        self.allowed.iter().map(String::as_str)
    }

    /// Refuse any URL whose host and port are not allowed.
    pub fn check(&self, url: &Url) -> Result<(), LlmError> {
        match host_port(url) {
            Some(hp) if self.allowed.contains(&hp) => Ok(()),
            Some(hp) => Err(LlmError::EgressBlocked { host: hp }),
            None => Err(LlmError::EgressBlocked {
                host: url.to_string(),
            }),
        }
    }
}

fn host_port(url: &Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url.port_or_known_default()?;
    Some(format!("{host}:{port}"))
}

/// A JSON-over-HTTPS client that only reaches allowed endpoints.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    guard: EgressGuard,
    provider: String,
}

impl HttpClient {
    /// Build a client for one provider.
    pub fn new(provider: &str, guard: EgressGuard, timeout: Duration) -> Result<Self, LlmError> {
        let inner = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("fixloop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| LlmError::Config {
                message: format!("could not build HTTP client for {provider}: {e}"),
                hint: "this is a bug; please report it".into(),
            })?;
        Ok(Self {
            inner,
            guard,
            provider: provider.to_owned(),
        })
    }

    /// POST a JSON body and return the JSON response. Non-2xx responses
    /// become [`LlmError::Http`] carrying the provider's error message.
    pub async fn post_json(
        &self,
        url: &Url,
        headers: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value, LlmError> {
        self.guard.check(url)?;
        let mut req = self.inner.post(url.clone()).json(body);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.send().await.map_err(|e| self.transport_error(&e))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| self.transport_error(&e))?;
        if !status.is_success() {
            return Err(LlmError::Http {
                provider: self.provider.clone(),
                status: status.as_u16(),
                message: error_message(&text),
            });
        }
        serde_json::from_str(&text).map_err(|e| LlmError::Decode {
            provider: self.provider.clone(),
            message: format!("response is not JSON: {e}"),
        })
    }

    fn transport_error(&self, e: &reqwest::Error) -> LlmError {
        if e.is_timeout() {
            LlmError::Timeout {
                provider: self.provider.clone(),
            }
        } else {
            LlmError::Transport {
                provider: self.provider.clone(),
                message: e.to_string(),
            }
        }
    }
}

/// Pull a readable message out of a provider error body. Both Anthropic and
/// OpenAI use `{"error": {"message": ...}}`; some servers use a string.
fn error_message(body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let msg = parsed.as_ref().and_then(|v| {
        v.pointer("/error/message")
            .or_else(|| v.get("error"))
            .or_else(|| v.get("message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let mut msg = msg.unwrap_or_else(|| body.to_owned());
    if msg.len() > 500 {
        let mut end = 500;
        while !msg.is_char_boundary(end) {
            end -= 1;
        }
        msg.truncate(end);
        msg.push('…');
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn guard_allows_only_configured_host_and_port() {
        let g = EgressGuard::new([&url("http://localhost:11434/v1")]);
        assert!(
            g.check(&url("http://localhost:11434/v1/chat/completions"))
                .is_ok()
        );
        assert!(g.check(&url("http://localhost:8080/v1")).is_err());
        assert!(matches!(
            g.check(&url("https://api.anthropic.com/v1/messages")),
            Err(LlmError::EgressBlocked { .. })
        ));
    }

    #[test]
    fn default_ports_are_normalised() {
        let g = EgressGuard::new([&url("https://api.openai.com/v1")]);
        assert!(
            g.check(&url("https://api.openai.com:443/v1/chat/completions"))
                .is_ok()
        );
        assert!(g.check(&url("http://api.openai.com/v1")).is_err());
    }

    #[test]
    fn error_message_extraction() {
        assert_eq!(
            error_message(r#"{"error":{"message":"bad key"}}"#),
            "bad key"
        );
        assert_eq!(
            error_message(r#"{"error":"model not found"}"#),
            "model not found"
        );
        assert_eq!(error_message("plain"), "plain");
    }
}
