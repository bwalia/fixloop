//! Webhook endpoint behaviour.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fixloop_core::config::{BudgetsConfig, RetryConfig};
use fixloop_llm::Router;
use fixloop_scm::github::webhook::sign;
use fixloop_server::AppState;
use fixloop_store::Store;
use serde_json::json;
use tower::ServiceExt;

const SECRET: &[u8] = b"test-secret";

async fn state() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::connect(&format!(
        "sqlite://{}?mode=rwc",
        dir.path().join("s.db").display()
    ))
    .await
    .unwrap();
    let router = Router::new(
        vec![],
        BTreeMap::new(),
        RetryConfig::default(),
        BudgetsConfig::default(),
    );
    (
        AppState {
            store,
            github: None,
            webhook_secret: Some(Arc::new(SECRET.to_vec())),
            router: Arc::new(router),
        },
        dir,
    )
}

fn pr_opened() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "action": "opened",
        "repository": {"full_name": "o/r"},
        "installation": {"id": 9},
        "pull_request": {"number": 4, "draft": false}
    }))
    .unwrap()
}

fn req(event: &str, delivery: &str, body: &[u8], sig: &str) -> Request<Body> {
    Request::post("/webhooks/github")
        .header("x-github-event", event)
        .header("x-github-delivery", delivery)
        .header("x-hub-signature-256", sig)
        .body(Body::from(body.to_vec()))
        .unwrap()
}

#[tokio::test]
async fn bad_signature_is_rejected_and_nothing_is_queued() {
    let (s, _d) = state().await;
    let app = fixloop_server::http::routes(s.clone());
    let body = pr_opened();
    let res = app
        .oneshot(req("pull_request", "d1", &body, &sign(b"wrong", &body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(s.store.queue_stats().await.unwrap().queued, 0);
}

#[tokio::test]
async fn valid_pr_event_queues_one_job_even_if_redelivered() {
    let (s, _d) = state().await;
    let body = pr_opened();
    for _ in 0..2 {
        let app = fixloop_server::http::routes(s.clone());
        let res = app
            .oneshot(req(
                "pull_request",
                "same-delivery",
                &body,
                &sign(SECRET, &body),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
    }
    assert_eq!(s.store.queue_stats().await.unwrap().queued, 1);
    let job = s.store.lease("w", 60).await.unwrap().unwrap();
    assert_eq!(job.payload["repo"], "o/r");
    assert_eq!(job.payload["pr"], 4);
    assert_eq!(job.payload["installation_id"], 9);
}

#[tokio::test]
async fn irrelevant_events_are_acknowledged_but_ignored() {
    let (s, _d) = state().await;
    let body = serde_json::to_vec(
        &json!({"action": "closed", "repository": {"full_name": "o/r"},
        "installation": {"id": 9}, "pull_request": {"number": 4}}),
    )
    .unwrap();
    let app = fixloop_server::http::routes(s.clone());
    let res = app
        .oneshot(req("pull_request", "d2", &body, &sign(SECRET, &body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    assert_eq!(s.store.queue_stats().await.unwrap().queued, 0);
}

#[tokio::test]
async fn health_and_readiness() {
    let (s, _d) = state().await;
    for path in ["/healthz", "/readyz"] {
        let app = fixloop_server::http::routes(s.clone());
        let res = app
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
    }
}
