//! HTTP routes.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use fixloop_scm::github::webhook;
use fixloop_store::queue::NewJob;
use serde_json::json;

use crate::AppState;
use crate::worker::REVIEW_JOB;

/// Largest webhook body accepted (GitHub caps payloads at 25 MB).
const MAX_BODY: usize = 25 * 1024 * 1024;

/// Build the router.
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(readyz))
        .route("/webhooks/github", post(github_webhook))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
        .with_state(state)
}

/// A plain status page, so the root URL shows the server is up. Says nothing
/// about configuration.
async fn index() -> &'static str {
    concat!(
        "Fixloop ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "POST /webhooks/github  GitHub App webhook\n",
        "GET  /healthz          liveness\n",
        "GET  /readyz           readiness (database)\n\n",
        "https://github.com/bwalia/fixloop\n",
    )
}

async fn readyz(State(s): State<AppState>) -> Response {
    match s.store.ping().await {
        Ok(()) => (StatusCode::OK, "ready").into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "readiness check failed");
            (StatusCode::SERVICE_UNAVAILABLE, "database unavailable").into_response()
        }
    }
}

fn header<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

async fn github_webhook(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(secret) = &s.webhook_secret else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub webhooks are not configured",
        )
            .into_response();
    };
    let sig = header(&headers, "x-hub-signature-256").unwrap_or_default();
    if !webhook::verify_signature(secret, &body, sig) {
        tracing::warn!("rejected GitHub webhook with a bad signature");
        return (StatusCode::UNAUTHORIZED, "bad signature").into_response();
    }
    let event = header(&headers, "x-github-event").unwrap_or_default();
    let delivery = header(&headers, "x-github-delivery").unwrap_or_default();
    if event == "ping" {
        return (StatusCode::OK, "pong").into_response();
    }
    let Some(req) = webhook::review_request(event, &body) else {
        return (StatusCode::ACCEPTED, "ignored").into_response();
    };
    let (trigger, by) = match &req.trigger {
        webhook::Trigger::PullRequest(action) => ("automatic", action.clone()),
        webhook::Trigger::Command(user) => ("command", user.clone()),
    };
    let payload = json!({
        "repo": req.repo.to_string(),
        "pr": req.number,
        "installation_id": req.installation_id,
        "trigger": trigger,
        "by": by,
    });
    let key = (!delivery.is_empty()).then(|| format!("github:{delivery}"));
    let job = NewJob {
        kind: REVIEW_JOB,
        payload: &payload,
        idempotency_key: key.as_deref(),
        priority: if trigger == "command" { 50 } else { 100 },
        max_attempts: 3,
        trace_context: None,
    };
    match s.store.enqueue(job).await {
        Ok(e) => {
            tracing::info!(job = e.id, created = e.created, repo = %req.repo, pr = req.number, "queued review");
            let _ = s
                .store
                .audit(
                    "github",
                    "review.queued",
                    &format!("{}#{}", req.repo, req.number),
                    &payload,
                )
                .await;
            (
                StatusCode::ACCEPTED,
                axum::Json(json!({"job_id": e.id, "duplicate": !e.created})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "could not queue review");
            // 5xx makes GitHub show the delivery as failed so it can be redelivered.
            (StatusCode::SERVICE_UNAVAILABLE, "could not queue").into_response()
        }
    }
}
