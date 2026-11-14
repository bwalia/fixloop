//! Wire-format tests for each provider against recorded responses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use fixloop_core::config::Task;
use fixloop_llm::{CallContext, CompletionRequest, Message, Part, Role, StopReason, ToolCall};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cfg(kind: &str, base: &str, extra: &str) -> String {
    format!(
        r#"
providers:
  p: {{ kind: {kind}, base_url: "{base}", api_key_env: TEST_KEY {extra} }}
models:
  m: {{ provider: p, model: test-model, price: {{ input_per_mtok: 3.0, output_per_mtok: 15.0 }} }}
routes:
  review: [m]
retry: {{ max_retries: 0 }}
"#
    )
}

#[tokio::test]
async fn anthropic_text_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "sk-test"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(body_partial_json(json!({
            "model": "test-model",
            "max_tokens": 64,
            "system": "Be brief.",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "Say hi"}]}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text("Hi!")))
        .expect(1)
        .mount(&server)
        .await;

    let router = router(&cfg("anthropic", &server.uri(), ""));
    let mut req = CompletionRequest::from_prompt("Say hi", 64);
    req.system = Some("Be brief.".into());
    let r = router
        .complete(Task::Review, &req, &CallContext::default())
        .await
        .unwrap();

    assert_eq!(r.response.text(), "Hi!");
    assert_eq!(r.response.stop_reason, StopReason::EndTurn);
    assert_eq!(r.response.usage.total(), 18);
    // 12 * $3/M + 6 * $15/M
    assert!((r.cost_usd - 0.000_126).abs() < 1e-12, "{}", r.cost_usd);
}

#[tokio::test]
async fn anthropic_tool_use_and_tool_result_mapping() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(body_partial_json(json!({
            "tools": [{"name": "report_finding"}],
            "messages": [
                {"role": "user"},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_1", "name": "report_finding"}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "filed"}]}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_tool_use()))
        .expect(1)
        .mount(&server)
        .await;

    let router = router(&cfg("anthropic", &server.uri(), ""));
    let mut req = CompletionRequest::from_prompt("Review this", 512);
    req.tools = vec![report_tool()];
    req.messages.push(Message {
        role: Role::Assistant,
        content: vec![Part::ToolCall(ToolCall {
            id: "toolu_1".into(),
            name: "report_finding".into(),
            arguments: json!({"title": "a", "line": 1}),
        })],
    });
    req.messages.push(Message {
        role: Role::User,
        content: vec![Part::ToolResult {
            call_id: "toolu_1".into(),
            content: "filed".into(),
            is_error: false,
        }],
    });

    let r = router
        .complete(Task::Review, &req, &CallContext::default())
        .await
        .unwrap();
    assert_eq!(r.response.stop_reason, StopReason::ToolUse);
    let calls = r.response.tool_calls();
    assert_eq!(calls[0].name, "report_finding");
    assert_eq!(calls[0].arguments["line"], 42);
}

#[tokio::test]
async fn openai_uses_bearer_auth_and_max_completion_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .and(body_partial_json(json!({
            "model": "test-model",
            "max_completion_tokens": 64,
            "messages": [{"role": "system", "content": "Be brief."}, {"role": "user", "content": "Say hi"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text("Hi!")))
        .expect(1)
        .mount(&server)
        .await;

    let router = router(&cfg("openai", &format!("{}/v1", server.uri()), ""));
    let mut req = CompletionRequest::from_prompt("Say hi", 64);
    req.system = Some("Be brief.".into());
    let r = router
        .complete(Task::Review, &req, &CallContext::default())
        .await
        .unwrap();
    assert_eq!(r.response.text(), "Hi!");
    assert_eq!(r.response.usage.input_tokens, 10);
}

#[tokio::test]
async fn openai_compatible_uses_max_tokens_and_parses_tool_calls() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({
            "max_tokens": 256,
            "tools": [{"type": "function", "function": {"name": "report_finding"}}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_tool_call()))
        .expect(1)
        .mount(&server)
        .await;

    let yaml = format!(
        r#"
providers:
  local: {{ kind: openai_compatible, base_url: "{}/v1" }}
models:
  m: {{ provider: local, model: qwen-test }}
routes:
  review: [m]
"#,
        server.uri()
    );
    let router = router(&yaml);
    let mut req = CompletionRequest::from_prompt("Review", 256);
    req.tools = vec![report_tool()];
    let r = router
        .complete(Task::Review, &req, &CallContext::default())
        .await
        .unwrap();
    assert_eq!(r.response.stop_reason, StopReason::ToolUse);
    assert_eq!(r.response.tool_calls()[0].arguments["title"], "Off-by-one");
    assert_eq!(r.cost_usd, 0.0, "no price configured means free");
}

#[tokio::test]
async fn http_error_message_and_hint_are_surfaced() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
        ))
        .mount(&server)
        .await;
    let router = router(&cfg("anthropic", &server.uri(), ""));
    let err = router
        .complete_with_model(
            "m",
            &CompletionRequest::from_prompt("x", 8),
            &CallContext::default(),
        )
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("HTTP 401") && msg.contains("invalid x-api-key"),
        "{msg}"
    );
}
