//! Workers: lease jobs and run them.

use std::sync::Arc;
use std::time::Duration;

use fixloop_agents::ReviewError;
use fixloop_agents::reviewer::{PrReview, ReviewTrigger, prompt, review_pull_request};
use fixloop_core::scm::RepoRef;
use fixloop_store::queue::Job;
use fixloop_store::reviews::ReviewRecord;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::AppState;

/// Job kind for PR reviews.
pub const REVIEW_JOB: &str = "review";
const LEASE_SECS: i64 = 120;
const HEARTBEAT: Duration = Duration::from_secs(30);
const IDLE: Duration = Duration::from_secs(2);

/// Run one worker until `shutdown` turns true. Finishes the current job first.
pub async fn run(state: AppState, worker_id: String, mut shutdown: watch::Receiver<bool>) {
    tracing::info!(worker = %worker_id, "worker started");
    loop {
        if *shutdown.borrow() {
            break;
        }
        if let Err(e) = state.store.reap_expired().await {
            tracing::warn!(error = %e, "could not reclaim expired jobs");
        }
        match state.store.lease(&worker_id, LEASE_SECS).await {
            Ok(Some(job)) => run_job(&state, &worker_id, job).await,
            Ok(None) => {
                tokio::select! {
                    () = tokio::time::sleep(IDLE) => {}
                    _ = shutdown.changed() => {}
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "could not lease a job");
                tokio::time::sleep(IDLE * 5).await;
            }
        }
    }
    tracing::info!(worker = %worker_id, "worker stopped");
}

async fn run_job(state: &AppState, worker_id: &str, job: Job) {
    let span = tracing::info_span!("job", job.id = job.id, job.kind = %job.kind, job.attempt = job.attempt);
    let _enter = span.enter();
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let hb_store = state.store.clone();
    let hb_worker = worker_id.to_owned();
    let job_id = job.id;
    let heartbeat = tokio::spawn(async move {
        loop {
            tokio::select! {
                () = tokio::time::sleep(HEARTBEAT) => {
                    if let Err(e) = hb_store.heartbeat(job_id, &hb_worker, LEASE_SECS).await {
                        tracing::warn!(error = %e, "heartbeat failed");
                    }
                }
                _ = stop_rx.changed() => break,
            }
        }
    });

    let result = match job.kind.as_str() {
        REVIEW_JOB => run_review(state, &job).await,
        other => Err(JobError::Permanent(format!("unknown job kind `{other}`"))),
    };
    let _ = stop_tx.send(true);
    let _ = heartbeat.await;

    let outcome = match &result {
        Ok(()) => state.store.complete(job.id, worker_id).await,
        Err(JobError::Retryable(m)) => state.store.fail(job.id, worker_id, m, true).await,
        Err(JobError::Permanent(m)) => state.store.fail(job.id, worker_id, m, false).await,
    };
    match (&result, outcome) {
        (_, Err(e)) => tracing::error!(error = %e, "could not record job outcome"),
        (Ok(()), Ok(())) => tracing::info!("job succeeded"),
        (Err(e), Ok(())) => tracing::warn!(error = %e, "job failed"),
    }
}

/// A job failure, and whether to try again.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// Transient: retry with backoff.
    #[error("{0}")]
    Retryable(String),
    /// Will not succeed on retry.
    #[error("{0}")]
    Permanent(String),
}

fn classify(e: &ReviewError) -> JobError {
    use fixloop_core::scm::ScmError;
    use fixloop_llm::LlmError;
    let retry = match e {
        ReviewError::Llm(
            LlmError::AllFailed { .. } | LlmError::Timeout { .. } | LlmError::Transport { .. },
        ) => true,
        ReviewError::Llm(l) => l.is_retryable(),
        ReviewError::Scm(ScmError::Transport(_)) => true,
        ReviewError::Scm(ScmError::Api { status, .. }) => *status == 429 || *status >= 500,
        _ => false,
    };
    if retry {
        JobError::Retryable(e.to_string())
    } else {
        JobError::Permanent(e.to_string())
    }
}

async fn run_review(state: &AppState, job: &Job) -> Result<(), JobError> {
    let p = &job.payload;
    let repo = p
        .get("repo")
        .and_then(Value::as_str)
        .and_then(RepoRef::parse)
        .ok_or_else(|| JobError::Permanent("job payload has no valid repo".into()))?;
    let number = p
        .get("pr")
        .and_then(Value::as_u64)
        .ok_or_else(|| JobError::Permanent("job payload has no pr".into()))?;
    let installation = p
        .get("installation_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| JobError::Permanent("job payload has no installation_id".into()))?;
    let trigger = match (
        p.get("trigger").and_then(Value::as_str),
        p.get("by").and_then(Value::as_str),
    ) {
        (Some("command"), Some(user)) => ReviewTrigger::Command(user.to_owned()),
        _ => ReviewTrigger::Automatic,
    };
    let github = state
        .github
        .as_ref()
        .ok_or_else(|| JobError::Permanent("GitHub is not configured on this server".into()))?;
    let client = github.installation(installation);
    let budget = state.router.job_budget(format!("job-{}", job.id));
    let outcome = review_pull_request(
        &client,
        &state.router,
        &repo,
        number,
        &trigger,
        Some(Arc::clone(&budget)),
    )
    .await
    .map_err(|e| classify(&e))?;
    let subject = format!("{repo}#{number}");
    match outcome {
        PrReview::Done { head_sha, result } => {
            let record = ReviewRecord {
                repo: &repo.to_string(),
                pr_number: i64::try_from(number).unwrap_or(i64::MAX),
                head_sha: &head_sha,
                job_id: Some(job.id),
                model: &result.model,
                prompt_version: prompt::PROMPT_VERSION,
                conclusion: result.conclusion(),
                tokens: i64::try_from(result.usage.total()).unwrap_or(i64::MAX),
                cost_usd: result.cost_usd,
                findings: &result.findings,
            };
            state
                .store
                .record_review(&record)
                .await
                .map_err(|e| JobError::Retryable(e.to_string()))?;
            let _ = state
                .store
                .audit("fixloop", "review.published", &subject, &json!({
                    "head_sha": head_sha, "findings": result.findings.len(), "model": result.model,
                    "tokens": result.usage.total(), "cost_usd": result.cost_usd
                }))
                .await;
        }
        PrReview::Unauthorized(user) => {
            let _ = state
                .store
                .audit(
                    "fixloop",
                    "command.refused",
                    &subject,
                    &json!({"user": user}),
                )
                .await;
        }
        PrReview::SkippedDraft => {}
    }
    Ok(())
}
