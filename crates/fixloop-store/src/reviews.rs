//! Stored reviews, findings and audit events.

use fixloop_core::domain::Finding;
use fixloop_core::scm::CheckConclusion;
use sqlx::Row;

use crate::{Store, StoreError, now};

/// A review to record.
#[derive(Debug, Clone)]
pub struct ReviewRecord<'a> {
    /// `owner/name`.
    pub repo: &'a str,
    /// PR number.
    pub pr_number: i64,
    /// Commit reviewed.
    pub head_sha: &'a str,
    /// Job that ran it.
    pub job_id: Option<i64>,
    /// Model name.
    pub model: &'a str,
    /// Prompt version.
    pub prompt_version: &'a str,
    /// pass / warn / fail.
    pub conclusion: CheckConclusion,
    /// Tokens used.
    pub tokens: i64,
    /// Cost in US dollars.
    pub cost_usd: f64,
    /// Findings published.
    pub findings: &'a [Finding],
}

impl Store {
    /// Save a review and its findings in one transaction; returns its id.
    pub async fn record_review(&self, r: &ReviewRecord<'_>) -> Result<i64, StoreError> {
        let t = now();
        let mut tx = self.pool().begin().await?;
        let conclusion = match r.conclusion {
            CheckConclusion::Success => "success",
            CheckConclusion::Neutral => "neutral",
            CheckConclusion::Failure => "failure",
        };
        let id: i64 = sqlx::query(
            "INSERT INTO reviews (repo, pr_number, head_sha, job_id, model, prompt_version, conclusion, tokens, cost_usd, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
        )
        .bind(r.repo)
        .bind(r.pr_number)
        .bind(r.head_sha)
        .bind(r.job_id)
        .bind(r.model)
        .bind(r.prompt_version)
        .bind(conclusion)
        .bind(r.tokens)
        .bind(r.cost_usd)
        .bind(t)
        .fetch_one(&mut *tx)
        .await?
        .try_get(0)?;
        for f in r.findings {
            sqlx::query(
                "INSERT INTO findings (review_id, repo, fingerprint, title, severity, category, path, line, end_line, rationale, confidence, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            )
            .bind(id)
            .bind(r.repo)
            .bind(&f.fingerprint)
            .bind(&f.title)
            .bind(f.severity.as_str())
            .bind(f.category.as_str())
            .bind(&f.path)
            .bind(i64::from(f.line))
            .bind(f.end_line.map(i64::from))
            .bind(&f.rationale)
            .bind(f64::from(f.confidence))
            .bind(t)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    /// Number of stored findings for a repository.
    pub async fn count_findings(&self, repo: &str) -> Result<i64, StoreError> {
        Ok(sqlx::query("SELECT COUNT(*) FROM findings WHERE repo = $1")
            .bind(repo)
            .fetch_one(self.pool())
            .await?
            .try_get(0)?)
    }

    /// Append an audit event.
    pub async fn audit(
        &self,
        actor: &str,
        action: &str,
        subject: &str,
        detail: &serde_json::Value,
    ) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO audit_events (at, actor, action, subject, detail) VALUES ($1, $2, $3, $4, $5)")
            .bind(now())
            .bind(actor)
            .bind(action)
            .bind(subject)
            .bind(detail.to_string())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Most recent audit events, newest first: (at, actor, action, subject).
    pub async fn recent_audit(
        &self,
        limit: i64,
    ) -> Result<Vec<(i64, String, String, String)>, StoreError> {
        let rows = sqlx::query(
            "SELECT at, actor, action, subject FROM audit_events ORDER BY id DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|r| Ok((r.try_get(0)?, r.try_get(1)?, r.try_get(2)?, r.try_get(3)?)))
            .collect()
    }
}
