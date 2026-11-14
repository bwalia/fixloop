//! Provider-neutral request and response types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Who sent a message. System instructions are separate (see
/// [`CompletionRequest::system`]) so that untrusted content can never be
/// placed in the system role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The caller (Fixloop), including tool results.
    User,
    /// The model.
    Assistant,
}

/// A piece of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// Plain text.
    Text {
        /// The text.
        text: String,
    },
    /// The model asks to call a tool.
    ToolCall(ToolCall),
    /// The result of a tool call, sent back to the model.
    ToolResult {
        /// Id of the [`ToolCall`] this answers.
        call_id: String,
        /// Result content.
        content: String,
        /// Whether the tool failed.
        is_error: bool,
    },
}

/// A tool call requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider-assigned id, echoed back in the result.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Arguments, as a JSON value.
    pub arguments: Value,
}

/// A conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Sender.
    pub role: Role,
    /// Content parts.
    pub content: Vec<Part>,
}

impl Message {
    /// A user message with one text part.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![Part::Text { text: text.into() }],
        }
    }

    /// An assistant message with one text part.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![Part::Text { text: text.into() }],
        }
    }
}

/// A tool the model may call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Tool name.
    pub name: String,
    /// What the tool does and when to use it.
    pub description: String,
    /// JSON Schema of the arguments object.
    pub input_schema: Value,
}

/// A request to a model. The model identifier is chosen by the router.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompletionRequest {
    /// Trusted system instructions.
    pub system: Option<String>,
    /// Conversation so far.
    pub messages: Vec<Message>,
    /// Tools the model may call.
    pub tools: Vec<ToolSpec>,
    /// Maximum output tokens.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: Option<f32>,
}

impl CompletionRequest {
    /// A one-turn request with a user prompt.
    pub fn from_prompt(prompt: impl Into<String>, max_tokens: u32) -> Self {
        Self {
            messages: vec![Message::user(prompt)],
            max_tokens,
            ..Self::default()
        }
    }

    /// A rough input-token estimate (about 4 characters per token), used to
    /// reserve budget before a call.
    pub fn estimated_input_tokens(&self) -> u64 {
        let mut chars = self.system.as_deref().map_or(0, str::len);
        for m in &self.messages {
            for p in &m.content {
                chars += match p {
                    Part::Text { text } => text.len(),
                    Part::ToolCall(c) => c.name.len() + c.arguments.to_string().len(),
                    Part::ToolResult { content, .. } => content.len(),
                };
            }
        }
        for t in &self.tools {
            chars += t.name.len() + t.description.len() + t.input_schema.to_string().len();
        }
        (chars as u64).div_ceil(4)
    }
}

/// Token usage reported by the provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Input (prompt) tokens.
    pub input_tokens: u64,
    /// Output (completion) tokens.
    pub output_tokens: u64,
}

impl Usage {
    /// Input plus output tokens.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Finished normally.
    EndTurn,
    /// Hit the output token limit.
    MaxTokens,
    /// Wants to call one or more tools.
    ToolUse,
    /// Anything else, as reported by the provider.
    Other(String),
}

/// A model's reply.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionResponse {
    /// Text and tool-call parts, in order.
    pub content: Vec<Part>,
    /// Token usage.
    pub usage: Usage,
    /// Why the model stopped.
    pub stop_reason: StopReason,
}

impl CompletionResponse {
    /// All text parts joined together.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// The tool calls the model made.
    pub fn tool_calls(&self) -> Vec<&ToolCall> {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::ToolCall(c) => Some(c),
                _ => None,
            })
            .collect()
    }
}
