//! Anthropic Messages API.

use async_trait::async_trait;
use fixloop_core::config::ProviderKind;
use serde_json::{Value, json};
use url::Url;

use super::endpoint;
use crate::egress::HttpClient;
use crate::error::LlmError;
use crate::provider::LlmProvider;
use crate::types::{
    CompletionRequest, CompletionResponse, Message, Part, Role, StopReason, ToolCall, Usage,
};

/// Default endpoint.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// Anthropic provider.
#[derive(Debug)]
pub struct Anthropic {
    name: String,
    url: Url,
    api_key: String,
    http: HttpClient,
}

impl Anthropic {
    /// Create a provider. `base` is the API root (default [`DEFAULT_BASE_URL`]).
    pub fn new(
        name: &str,
        base: &Url,
        api_key: String,
        http: HttpClient,
    ) -> Result<Self, LlmError> {
        Ok(Self {
            name: name.to_owned(),
            url: endpoint(base, "v1/messages")?,
            api_key,
            http,
        })
    }

    fn decode_error(&self, message: impl Into<String>) -> LlmError {
        LlmError::Decode {
            provider: self.name.clone(),
            message: message.into(),
        }
    }
}

/// Build the request body. Public for fixture tests.
pub fn request_body(model: &str, req: &CompletionRequest) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": req.max_tokens,
        "messages": req.messages.iter().map(message).collect::<Vec<_>>(),
    });
    if let Some(system) = &req.system {
        body["system"] = json!(system);
    }
    // `req.temperature` is deliberately not sent. Current Claude models
    // (Opus 4.7 and later, Sonnet 5 and later, Haiku 5.5, Fable) reject
    // sampling parameters with HTTP 400, and newer API versions reject them
    // on every model.
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema}))
            .collect();
    }
    body
}

fn message(m: &Message) -> Value {
    let role = match m.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    let content: Vec<Value> = m
        .content
        .iter()
        .map(|p| match p {
            Part::Text { text } => json!({"type": "text", "text": text}),
            Part::ToolCall(c) => {
                json!({"type": "tool_use", "id": c.id, "name": c.name, "input": c.arguments})
            }
            Part::ToolResult {
                call_id,
                content,
                is_error,
            } => json!({
                "type": "tool_result", "tool_use_id": call_id,
                "content": content, "is_error": is_error
            }),
        })
        .collect();
    json!({"role": role, "content": content})
}

/// Parse a response body. Public for fixture tests.
pub fn parse_response(body: &Value) -> Result<CompletionResponse, String> {
    let blocks = body
        .get("content")
        .and_then(Value::as_array)
        .ok_or("missing `content` array")?;
    let mut content = Vec::new();
    for b in blocks {
        match b.get("type").and_then(Value::as_str) {
            Some("text") => content.push(Part::Text {
                text: b
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }),
            Some("tool_use") => content.push(Part::ToolCall(ToolCall {
                id: str_field(b, "id")?,
                name: str_field(b, "name")?,
                arguments: b.get("input").cloned().unwrap_or_else(|| json!({})),
            })),
            // Thinking and other block types are not surfaced.
            _ => {}
        }
    }
    let usage = Usage {
        input_tokens: body
            .pointer("/usage/input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: body
            .pointer("/usage/output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    };
    let stop_reason = match body.get("stop_reason").and_then(Value::as_str) {
        Some("end_turn" | "stop_sequence") => StopReason::EndTurn,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("tool_use") => StopReason::ToolUse,
        Some(other) => StopReason::Other(other.to_owned()),
        None => StopReason::Other("unknown".into()),
    };
    Ok(CompletionResponse {
        content,
        usage,
        stop_reason,
    })
}

fn str_field(v: &Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("tool_use block missing `{key}`"))
}

#[async_trait]
impl LlmProvider for Anthropic {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    async fn complete(
        &self,
        model: &str,
        req: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError> {
        let body = request_body(model, req);
        let headers = [
            ("x-api-key", self.api_key.as_str()),
            ("anthropic-version", API_VERSION),
        ];
        let resp = self.http.post_json(&self.url, &headers, &body).await?;
        parse_response(&resp).map_err(|m| self.decode_error(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_parameters_are_never_sent() {
        let req = CompletionRequest {
            temperature: Some(0.0),
            ..CompletionRequest::default()
        };
        let body = request_body("claude-sonnet-5-5", &req);
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
    }
}
