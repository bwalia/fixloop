//! OpenAI Chat Completions API. Also used for every OpenAI-compatible server
//! (Ollama, vLLM, llama.cpp, LM Studio, NVIDIA NIM).

use async_trait::async_trait;
use fixloop_core::config::ProviderKind;
use serde_json::{Value, json};
use url::Url;

use super::endpoint;
use crate::egress::HttpClient;
use crate::error::LlmError;
use crate::provider::LlmProvider;
use crate::types::{
    CompletionRequest, CompletionResponse, Part, Role, StopReason, ToolCall, Usage,
};

/// Default endpoint for `kind: openai`.
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI or OpenAI-compatible provider.
#[derive(Debug)]
pub struct OpenAi {
    name: String,
    kind: ProviderKind,
    url: Url,
    auth: Option<String>,
    http: HttpClient,
}

impl OpenAi {
    /// Create a provider. `kind` is `Openai` or `OpenaiCompatible`; `base`
    /// is the API root including `/v1`.
    pub fn new(
        name: &str,
        kind: ProviderKind,
        base: &Url,
        api_key: Option<String>,
        http: HttpClient,
    ) -> Result<Self, LlmError> {
        Ok(Self {
            name: name.to_owned(),
            kind,
            url: endpoint(base, "chat/completions")?,
            auth: api_key.map(|k| format!("Bearer {k}")),
            http,
        })
    }
}

/// Build the request body. Public for fixture tests.
pub fn request_body(kind: ProviderKind, model: &str, req: &CompletionRequest) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = &req.system {
        messages.push(json!({"role": "system", "content": system}));
    }
    for m in &req.messages {
        match m.role {
            Role::Assistant => {
                let text: String = m
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        Part::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                let calls: Vec<Value> = m
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        Part::ToolCall(c) => Some(json!({
                            "id": c.id, "type": "function",
                            "function": {"name": c.name, "arguments": c.arguments.to_string()}
                        })),
                        _ => None,
                    })
                    .collect();
                let mut msg = json!({"role": "assistant", "content": text});
                if !calls.is_empty() {
                    msg["tool_calls"] = json!(calls);
                }
                messages.push(msg);
            }
            Role::User => {
                // Tool results become separate `tool` messages; text stays a user message.
                let mut text = String::new();
                for p in &m.content {
                    match p {
                        Part::Text { text: t } => text.push_str(t),
                        Part::ToolResult {
                            call_id, content, ..
                        } => messages.push(
                            json!({"role": "tool", "tool_call_id": call_id, "content": content}),
                        ),
                        Part::ToolCall(_) => {}
                    }
                }
                if !text.is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                }
            }
        }
    }

    let mut body = json!({"model": model, "messages": messages});
    // OpenAI's current models use `max_completion_tokens`; compatible
    // servers generally only understand `max_tokens`.
    let max_key = if kind == ProviderKind::Openai {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };
    body[max_key] = json!(req.max_tokens);
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| {
                json!({"type": "function", "function": {
                    "name": t.name, "description": t.description, "parameters": t.input_schema
                }})
            })
            .collect();
    }
    body
}

/// Parse a response body. Public for fixture tests.
pub fn parse_response(body: &Value) -> Result<CompletionResponse, LlmError> {
    let decode = |m: &str| LlmError::Decode {
        provider: "openai".into(),
        message: m.to_owned(),
    };
    let choice = body
        .pointer("/choices/0")
        .ok_or_else(|| decode("missing `choices[0]`"))?;
    let msg = choice
        .get("message")
        .ok_or_else(|| decode("missing `message`"))?;

    let mut content = Vec::new();
    if let Some(text) = msg.get("content").and_then(Value::as_str)
        && !text.is_empty()
    {
        content.push(Part::Text {
            text: text.to_owned(),
        });
    }
    for (i, call) in msg
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let name = call
            .pointer("/function/name")
            .and_then(Value::as_str)
            .ok_or_else(|| LlmError::InvalidToolCall("tool call without a name".into()))?;
        let args = match call.pointer("/function/arguments") {
            Some(Value::String(s)) if s.trim().is_empty() => json!({}),
            Some(Value::String(s)) => serde_json::from_str(s).map_err(|e| {
                LlmError::InvalidToolCall(format!("arguments for `{name}` are not JSON: {e}"))
            })?,
            // Some compatible servers send an object instead of a string.
            Some(v @ Value::Object(_)) => v.clone(),
            _ => json!({}),
        };
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .map_or_else(|| format!("call_{i}"), str::to_owned);
        content.push(Part::ToolCall(ToolCall {
            id,
            name: name.to_owned(),
            arguments: args,
        }));
    }

    let usage = Usage {
        input_tokens: body
            .pointer("/usage/prompt_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: body
            .pointer("/usage/completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    };
    let has_calls = content.iter().any(|p| matches!(p, Part::ToolCall(_)));
    let stop_reason = match choice.get("finish_reason").and_then(Value::as_str) {
        Some("tool_calls") => StopReason::ToolUse,
        _ if has_calls => StopReason::ToolUse,
        Some("stop") => StopReason::EndTurn,
        Some("length") => StopReason::MaxTokens,
        Some(other) => StopReason::Other(other.to_owned()),
        None => StopReason::Other("unknown".into()),
    };
    Ok(CompletionResponse {
        content,
        usage,
        stop_reason,
    })
}

#[async_trait]
impl LlmProvider for OpenAi {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> ProviderKind {
        self.kind
    }

    async fn complete(
        &self,
        model: &str,
        req: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError> {
        let body = request_body(self.kind, model, req);
        let headers: Vec<(&str, &str)> = self
            .auth
            .as_deref()
            .map(|a| vec![("authorization", a)])
            .unwrap_or_default();
        let resp = self.http.post_json(&self.url, &headers, &body).await?;
        parse_response(&resp).map_err(|e| match e {
            LlmError::Decode { message, .. } => LlmError::Decode {
                provider: self.name.clone(),
                message,
            },
            other => other,
        })
    }
}
