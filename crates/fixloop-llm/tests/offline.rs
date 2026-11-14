//! Offline mode: with only local models configured, Fixloop must make zero
//! calls to outside AI APIs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use fixloop_core::FixloopConfig;
use fixloop_core::config::Task;
use fixloop_llm::egress::EgressGuard;
use fixloop_llm::{CallContext, CompletionRequest, LlmError};
use url::Url;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn offline_yaml(local_uri: &str) -> String {
    format!(
        r#"
offline: true
providers:
  ollama: {{ kind: openai_compatible, base_url: "{local_uri}/v1" }}
models:
  local: {{ provider: ollama, model: some-local-model }}
routes:
  review: [local]
  triage: [local]
"#
    )
}

#[tokio::test]
async fn offline_router_only_talks_to_the_local_endpoint() {
    let local = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text("offline ok")))
        .expect(2)
        .mount(&local)
        .await;

    let router = router(&offline_yaml(&local.uri()));

    // The only reachable endpoint is the local server.
    let port = Url::parse(&local.uri()).unwrap().port().unwrap();
    assert_eq!(router.egress_allowlist(), [format!("127.0.0.1:{port}")]);
    for m in router.models() {
        assert_eq!(m.provider.locality(), fixloop_core::domain::Locality::Local);
    }

    for task in [Task::Review, Task::Triage] {
        let r = router
            .complete(
                task,
                &CompletionRequest::from_prompt("hi", 16),
                &CallContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(r.response.text(), "offline ok");
    }
    // `expect(2)` on the mock verifies every request went to the local server.
}

#[test]
fn offline_config_rejects_every_external_provider_kind() {
    for kind in [
        "anthropic",
        "openai",
        "gemini",
        "azure_openai",
        "bedrock",
        "vertex",
    ] {
        let yaml = format!(
            "llm: {{ offline: true, providers: {{ p: {{ kind: {kind}, api_key_env: K }} }} }}"
        );
        let dir = std::env::temp_dir().join(format!("fixloop-offline-{kind}.yaml"));
        std::fs::write(&dir, yaml).unwrap();
        let err = FixloopConfig::load(Some(&dir)).unwrap_err();
        assert!(err.to_string().contains("offline"), "{kind}: {err}");
        let _ = std::fs::remove_file(dir);
    }
}

#[test]
fn offline_router_build_rejects_external_provider_even_without_validation() {
    // Defence in depth: the router re-checks even if config validation was skipped.
    let mut cfg = llm_config("providers: { c: { kind: anthropic, api_key_env: TEST_KEY } }");
    cfg.offline = true;
    let err = fixloop_llm::Router::from_config(&cfg, &|_| Some("k".into())).unwrap_err();
    assert!(err.to_string().contains("offline"), "{err}");
}

#[test]
fn egress_guard_blocks_every_public_ai_host() {
    let guard = EgressGuard::new([&Url::parse("http://localhost:11434/v1").unwrap()]);
    for host in fixloop_core::config::PUBLIC_AI_HOSTS {
        let url = Url::parse(&format!("https://{host}/v1/chat/completions")).unwrap();
        assert!(
            matches!(guard.check(&url), Err(LlmError::EgressBlocked { .. })),
            "{host} was not blocked"
        );
    }
}
