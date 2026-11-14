//! Routing behaviour: fallback, retries, data egress, budgets, JSON tool mode.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use fixloop_core::config::{BudgetLimit, Task};
use fixloop_core::domain::DataEgress;
use fixloop_llm::{BudgetAccount, CallContext, CompletionRequest, LlmError, StopReason};
use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Two servers: `primary` is an external (Anthropic-kind) provider,
/// `local` is a self-hosted OpenAI-compatible one.
async fn two_providers(extra: &str) -> (MockServer, MockServer, String) {
    let primary = MockServer::start().await;
    let local = MockServer::start().await;
    let yaml = format!(
        r#"
providers:
  cloud: {{ kind: anthropic, base_url: "{}", api_key_env: TEST_KEY }}
  local: {{ kind: openai_compatible, base_url: "{}/v1" }}
models:
  strong: {{ provider: cloud, model: cloud-model, price: {{ input_per_mtok: 3.0, output_per_mtok: 15.0 }} }}
  cheap:  {{ provider: local, model: local-model }}
routes:
  review: [strong, cheap]
retry: {{ max_retries: 2, initial_backoff_ms: 1 }}
{extra}
"#,
        primary.uri(),
        local.uri()
    );
    (primary, local, yaml)
}

#[tokio::test]
async fn falls_back_to_next_model_when_first_fails() {
    let (cloud, local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3) // first try + 2 retries
        .mount(&cloud)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text("from local")))
        .expect(1)
        .mount(&local)
        .await;

    let r = router(&yaml)
        .complete(
            Task::Review,
            &CompletionRequest::from_prompt("hi", 32),
            &CallContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(r.model, "cheap");
    assert_eq!(r.response.text(), "from local");
    assert_eq!(r.failed_attempts.len(), 1);
    assert!(
        r.failed_attempts[0].contains("HTTP 503"),
        "{:?}",
        r.failed_attempts
    );
}

#[tokio::test]
async fn retries_then_succeeds_on_same_model() {
    let (cloud, _local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .mount(&cloud)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text("ok")))
        .mount(&cloud)
        .await;
    let r = router(&yaml)
        .complete(
            Task::Review,
            &CompletionRequest::from_prompt("hi", 32),
            &CallContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(r.model, "strong");
    assert!(r.failed_attempts.is_empty());
}

#[tokio::test]
async fn client_errors_are_not_retried() {
    let (cloud, local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400))
        .expect(1)
        .mount(&cloud)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text("ok")))
        .mount(&local)
        .await;
    router(&yaml)
        .complete(
            Task::Review,
            &CompletionRequest::from_prompt("hi", 32),
            &CallContext::default(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn local_only_repo_never_reaches_external_provider() {
    let (cloud, local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text("leak!")))
        .expect(0)
        .mount(&cloud)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text("local answer")))
        .expect(1)
        .mount(&local)
        .await;
    let r = router(&yaml)
        .complete(
            Task::Review,
            &CompletionRequest::from_prompt("secret code", 32),
            &CallContext::new(DataEgress::LocalOnly),
        )
        .await
        .unwrap();
    assert_eq!(r.model, "cheap");
}

#[tokio::test]
async fn local_only_with_no_local_model_is_an_error_not_a_downgrade() {
    let (cloud, _local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&cloud)
        .await;
    let r = router(&yaml);
    let err = r
        .complete_with_model(
            "strong",
            &CompletionRequest::from_prompt("x", 8),
            &CallContext::new(DataEgress::LocalOnly),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, LlmError::NoModel { .. }), "{err}");
    assert!(err.to_string().contains("local-only"), "{err}");
}

#[tokio::test]
async fn job_budget_stops_call_before_it_is_made() {
    let (cloud, local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&cloud)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&local)
        .await;
    let job = BudgetAccount::new(
        "job",
        BudgetLimit {
            max_tokens: Some(100),
            max_cost_usd: None,
        },
    );
    let err = router(&yaml)
        .complete(
            Task::Review,
            &CompletionRequest::from_prompt("hi", 1000),
            &CallContext::default().with_budget(job),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, LlmError::BudgetExceeded { ref scope, .. } if scope == "job"),
        "{err}"
    );
}

#[tokio::test]
async fn budget_records_actual_usage_and_cost() {
    let (cloud, _local, yaml) = two_providers("").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text("ok")))
        .mount(&cloud)
        .await;
    let job = BudgetAccount::new("job", BudgetLimit::default());
    router(&yaml)
        .complete(
            Task::Review,
            &CompletionRequest::from_prompt("hi", 32),
            &CallContext::default().with_budget(job.clone()),
        )
        .await
        .unwrap();
    let (tokens, cost) = job.spent();
    assert_eq!(tokens, 18);
    assert!(cost > 0.0);
}

#[tokio::test]
async fn daily_budget_from_config_applies() {
    let (cloud, _local, yaml) =
        two_providers("budgets: { daily: { max_cost_usd: 0.0000001 } }").await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&cloud)
        .await;
    let err = router(&yaml)
        .complete_with_model(
            "strong",
            &CompletionRequest::from_prompt("hi", 32),
            &CallContext::default(),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("daily"), "{err}");
}

#[tokio::test]
async fn json_tool_mode_parses_and_self_corrects_once() {
    let server = MockServer::start().await;
    // First reply is invalid (unknown tool); the correction prompt gets a valid one.
    Mock::given(method("POST"))
        .and(body_string_contains(
            "Reply again with exactly one JSON object",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text(
            r#"{"tool": "report_finding", "arguments": {"title": "Off-by-one", "line": 7}}"#,
        )))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(openai_text(r#"{"tool": "delete_repo"}"#)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let yaml = format!(
        r#"
providers:
  local: {{ kind: openai_compatible, base_url: "{}/v1" }}
models:
  small: {{ provider: local, model: tiny, tool_mode: json }}
routes:
  review: [small]
"#,
        server.uri()
    );
    let mut req = CompletionRequest::from_prompt("Review", 128);
    req.tools = vec![report_tool()];
    let r = router(&yaml)
        .complete(Task::Review, &req, &CallContext::default())
        .await
        .unwrap();
    assert_eq!(r.response.stop_reason, StopReason::ToolUse);
    assert_eq!(r.response.tool_calls()[0].arguments["line"], 7);
    assert_eq!(
        r.response.usage.input_tokens, 20,
        "usage of both calls is summed"
    );

    // Native tools were not sent: the tool list went into the system prompt.
    let requests = server.received_requests().await.unwrap();
    let first: serde_json::Value = requests[0].body_json().unwrap();
    assert!(first.get("tools").is_none());
    assert!(
        first["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("report_finding")
    );
}

#[tokio::test]
async fn missing_api_key_env_is_a_clear_error() {
    let cfg = llm_config("providers: { c: { kind: anthropic, api_key_env: NOPE_NOT_SET } }");
    let err = fixloop_llm::Router::from_config(&cfg, &|_| None).unwrap_err();
    assert!(err.to_string().contains("NOPE_NOT_SET"), "{err}");
}

#[tokio::test]
async fn unimplemented_provider_kind_explains_alternative() {
    let cfg = llm_config("providers: { g: { kind: gemini, api_key_env: TEST_KEY } }");
    let err = fixloop_llm::Router::from_config(&cfg, &|_| Some("k".into())).unwrap_err();
    let hint = fixloop_core::Hinted::hint(&err).unwrap();
    assert!(hint.contains("openai_compatible"), "{hint}");
}
