//! Store tests. Run on SQLite always, and on PostgreSQL when
//! `FIXLOOP_TEST_POSTGRES_URL` is set (CI sets it).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use fixloop_core::domain::{Category, Finding, Severity};
use fixloop_core::scm::CheckConclusion;
use fixloop_store::Store;
use fixloop_store::queue::NewJob;
use fixloop_store::reviews::ReviewRecord;
use serde_json::json;

async fn stores() -> Vec<(Store, Option<tempfile::TempDir>)> {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}?mode=rwc", dir.path().join("t.db").display());
    let mut v = vec![(Store::connect(&url).await.unwrap(), Some(dir))];
    if let Ok(admin) = std::env::var("FIXLOOP_TEST_POSTGRES_URL") {
        v.push((
            Store::connect(&fresh_pg_database(&admin).await)
                .await
                .expect("postgres test database"),
            None,
        ));
    }
    v
}

/// Tests run in parallel and the queue hands out any ready job, so each
/// test gets its own database.
async fn fresh_pg_database(admin_url: &str) -> String {
    use sqlx::Connection;
    sqlx::any::install_default_drivers();
    let name = format!(
        "fixloop_test_{:x}{:x}",
        fixloop_store::now(),
        rand_suffix() * 7919 + u128::from(std::process::id())
    );
    let mut conn = sqlx::AnyConnection::connect(admin_url)
        .await
        .expect("connect to admin database");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&mut conn)
        .await
        .expect("create test database");
    let (base, query) = admin_url
        .split_once('?')
        .map_or((admin_url, ""), |(b, q)| (b, q));
    let root = &base[..base.rfind('/').expect("database URL has a path")];
    if query.is_empty() {
        format!("{root}/{name}")
    } else {
        format!("{root}/{name}?{query}")
    }
}

fn job<'a>(payload: &'a serde_json::Value, key: Option<&'a str>) -> NewJob<'a> {
    NewJob {
        kind: "review",
        payload,
        idempotency_key: key,
        priority: 100,
        max_attempts: 2,
        trace_context: None,
    }
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}?mode=rwc", dir.path().join("m.db").display());
    Store::connect(&url).await.unwrap();
    Store::connect(&url).await.unwrap().ping().await.unwrap();
}

#[tokio::test]
async fn enqueue_lease_complete_and_idempotency() {
    for (s, _d) in stores().await {
        let key = format!(
            "delivery-{}",
            fixloop_store::now() as u128 * 1000 + rand_suffix()
        );
        let p = json!({"repo": "o/r", "pr": 1});
        let a = s.enqueue(job(&p, Some(&key))).await.unwrap();
        let b = s.enqueue(job(&p, Some(&key))).await.unwrap();
        assert!(a.created && !b.created);
        assert_eq!(a.id, b.id);

        let j = s.lease("w1", 60).await.unwrap().expect("job");
        assert_eq!((j.id, j.kind.as_str(), j.attempt), (a.id, "review", 1));
        assert_eq!(j.payload["pr"], 1);
        assert!(
            s.lease("w2", 60).await.unwrap().is_none(),
            "leased jobs are not handed out twice"
        );
        assert!(s.heartbeat(j.id, "w1", 60).await.unwrap());
        assert!(
            !s.heartbeat(j.id, "w2", 60).await.unwrap(),
            "only the owner can heartbeat"
        );
        s.complete(j.id, "w1").await.unwrap();
        assert_eq!(s.job_state(j.id).await.unwrap().unwrap().0, "succeeded");
    }
}

fn rand_suffix() -> u128 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u8(0);
    u128::from(h.finish() % 1_000_000)
}

#[tokio::test]
async fn retryable_failure_requeues_with_backoff_then_fails() {
    for (s, _d) in stores().await {
        let p = json!({});
        let id = s.enqueue(job(&p, None)).await.unwrap().id;
        let j = s.lease("w", 60).await.unwrap().unwrap();
        assert_eq!(j.id, id);
        s.fail(id, "w", "HTTP 503", true).await.unwrap();
        let (state, err) = s.job_state(id).await.unwrap().unwrap();
        assert_eq!(
            (state.as_str(), err.as_deref()),
            ("queued", Some("HTTP 503"))
        );
        // Backoff: not ready yet.
        assert!(s.lease("w", 60).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn non_retryable_failure_is_final() {
    for (s, _d) in stores().await {
        let p = json!({});
        let id = s.enqueue(job(&p, None)).await.unwrap().id;
        s.lease("w", 60).await.unwrap().unwrap();
        s.fail(id, "w", "bad config", false).await.unwrap();
        assert_eq!(s.job_state(id).await.unwrap().unwrap().0, "failed");
    }
}

#[tokio::test]
async fn expired_lease_is_reclaimed_after_crash() {
    for (s, _d) in stores().await {
        let p = json!({});
        let id = s.enqueue(job(&p, None)).await.unwrap().id;
        s.lease("crashed-worker", 60).await.unwrap().unwrap();
        s.expire_lease_for_test(id).await.unwrap();
        assert!(s.reap_expired().await.unwrap() >= 1);
        let j = s
            .lease("w2", 60)
            .await
            .unwrap()
            .expect("job is back in the queue");
        assert_eq!((j.id, j.attempt), (id, 2));
        // Out of attempts now (max 2): another crash fails it for good.
        s.expire_lease_for_test(id).await.unwrap();
        s.reap_expired().await.unwrap();
        assert_eq!(s.job_state(id).await.unwrap().unwrap().0, "failed");
    }
}

#[tokio::test]
async fn records_reviews_findings_and_audit() {
    for (s, _d) in stores().await {
        let f = Finding {
            title: "Off by one".into(),
            severity: Severity::High,
            category: Category::Bug,
            path: "src/a.rs".into(),
            line: 6,
            end_line: None,
            rationale: "r".into(),
            suggested_fix: None,
            suggestion: None,
            confidence: 0.9,
            fingerprint: "abc".into(),
        };
        let repo = format!("o/r{}", rand_suffix());
        let before = s.count_findings(&repo).await.unwrap();
        s.record_review(&ReviewRecord {
            repo: &repo,
            pr_number: 1,
            head_sha: "h",
            job_id: None,
            model: "m",
            prompt_version: "reviewer-v1",
            conclusion: CheckConclusion::Failure,
            tokens: 10,
            cost_usd: 0.01,
            findings: &[f],
        })
        .await
        .unwrap();
        assert_eq!(s.count_findings(&repo).await.unwrap(), before + 1);
        s.audit(
            "fixloop",
            "review.published",
            &repo,
            &json!({"findings": 1}),
        )
        .await
        .unwrap();
        let a = s.recent_audit(1).await.unwrap();
        assert_eq!(a[0].2, "review.published");
    }
}
