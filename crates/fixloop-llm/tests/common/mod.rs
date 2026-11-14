//! Shared helpers for integration tests: recorded provider responses and
//! config builders pointing at a local mock server.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use fixloop_core::config::LlmConfig;
use fixloop_llm::Router;
use serde_json::{Value, json};

/// Parse YAML into an `LlmConfig`.
pub fn llm_config(yaml: &str) -> LlmConfig {
    serde_saphyr::from_str(yaml).expect("test config parses")
}

/// Build a router with a fake environment holding `TEST_KEY`.
pub fn router(yaml: &str) -> Router {
    Router::from_config(&llm_config(yaml), &|k| {
        (k == "TEST_KEY").then(|| "sk-test".to_owned())
    })
    .expect("router builds")
}

/// A recorded Anthropic Messages API text reply.
pub fn anthropic_text(text: &str) -> Value {
    json!({
        "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
        "type": "message",
        "role": "assistant",
        "model": "test-model",
        "content": [{"type": "text", "text": text}],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 12, "output_tokens": 6}
    })
}

/// A recorded Anthropic tool-use reply.
pub fn anthropic_tool_use() -> Value {
    json!({
        "id": "msg_01Aq9w938a90dw8q",
        "type": "message",
        "role": "assistant",
        "model": "test-model",
        "content": [
            {"type": "text", "text": "I found a problem."},
            {"type": "tool_use", "id": "toolu_01A09q90qw90lq917835lq9",
             "name": "report_finding", "input": {"title": "Off-by-one", "line": 42}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 300, "output_tokens": 40}
    })
}

/// A recorded OpenAI Chat Completions text reply.
pub fn openai_text(text: &str) -> Value {
    json!({
        "id": "chatcmpl-abc123",
        "object": "chat.completion",
        "created": 1_731_600_000,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
}

/// A recorded OpenAI tool-call reply (arguments are a JSON string).
pub fn openai_tool_call() -> Value {
    json!({
        "id": "chatcmpl-tool",
        "object": "chat.completion",
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_abc",
                    "type": "function",
                    "function": {"name": "report_finding", "arguments": "{\"title\":\"Off-by-one\",\"line\":42}"}
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120}
    })
}

/// The `report_finding` tool used in tests.
pub fn report_tool() -> fixloop_llm::ToolSpec {
    fixloop_llm::ToolSpec {
        name: "report_finding".into(),
        description: "Report one bug found in the code".into(),
        input_schema: json!({
            "type": "object",
            "required": ["title", "line"],
            "properties": {"title": {"type": "string"}, "line": {"type": "integer"}}
        }),
    }
}
