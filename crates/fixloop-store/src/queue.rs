//! Durable job queue (ADR-0002). Jobs survive restarts; a crashed worker's
//! jobs are picked up again when their lease expires.

use serde_json::Value;
use sqlx::Row;

use crate::{Backend, Store, StoreError, now};

/// A leased job.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    /// Id.
    pub id: i64,
    /// Kind, e.g. `review`.
    pub kind: String,
    /// Payload.
    pub payload: Value,
    /// Attempt number, starting at 1.
    pub attempt: i64,
    /// Maximum attempts.
    pub max_attempts: i64,
    /// W3C trace context of whatever enqueued the job.
    pub trace_context: Option<String>,
}

/// A job to add.
#[derive(Debug, Clone)]
pub struct NewJob<'a> {
    /// Kind.
    pub kind: &'a str,
    /// Payload.
    pub payload: &'a Value,
    /// If set, a second job with the same key is not created.
    pub idempotency_key: Option<&'a str>,
    /// Lower runs first.
    pub priority: i64,
    /// Maximum attempts before the job fails for good.
    pub max_attempts: i64,
    /// Trace context to continue.
    pub trace_context: Option<&'a str>,
}

/// Result of [`Store::enqueue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enqueued {
    /// Job id.
    pub id: i64,
    /// False if a job with the same idempotency key already existed.
    pub created: bool,
}

/// Job counts by state, for metrics and health.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueueStats {
    /// Waiting to run.
    pub queued: i64,
    /// Running now.
    pub running: i64,
    /// Finished successfully.
    pub succeeded: i64,
    /// Failed for good.
    pub failed: i64,
}

const LEASE_PG: &str = "UPDATE jobs SET state = 'running', worker_id = $1, lease_until = $2, attempts = attempts + 1, updated_at = $3
     WHERE id = (SELECT id FROM jobs WHERE state = 'queued' AND run_after <= $3
                 ORDER BY priority, id LIMIT 1 FOR UPDATE SKIP LOCKED)
     RETURNING id, kind, payload, attempts, max_attempts, trace_context";

const LEASE_SQLITE: &str = "UPDATE jobs SET state = 'running', worker_id = $1, lease_until = $2, attempts = attempts + 1, updated_at = $3
     WHERE id = (SELECT id FROM jobs WHERE state = 'queued' AND run_after <= $3
                 ORDER BY priority, id LIMIT 1)
     RETURNING id, kind, payload, attempts, max_attempts, trace_context";

/// Backoff before retry `attempt` (1-based): 30s, 2m, 8m, … capped at 1h.
pub fn backoff_secs(attempt: i64) -> i64 {
    let exp = u32::try_from(attempt.clamp(1, 6) - 1).unwrap_or(0);
    (30 * 4_i64.pow(exp)).min(3600)
}

impl Store {
    /// Add a job.
    pub async fn enqueue(&self, job: NewJob<'_>) -> Result<Enqueued, StoreError> {
        let t = now();
        let payload = job.payload.to_string();
        let row = sqlx::query(
            "INSERT INTO jobs (kind, state, priority, payload, idempotency_key, max_attempts, run_after, trace_context, created_at, updated_at)
             VALUES ($1, 'queued', $2, $3, $4, $5, $6, $7, $6, $6)
             ON CONFLICT (idempotency_key) DO NOTHING
             RETURNING id",
        )
        .bind(job.kind)
        .bind(job.priority)
        .bind(&payload)
        .bind(job.idempotency_key)
        .bind(job.max_attempts)
        .bind(t)
        .bind(job.trace_context)
        .fetch_optional(self.pool())
        .await?;
        if let Some(r) = row {
            return Ok(Enqueued {
                id: r.try_get(0)?,
                created: true,
            });
        }
        let r = sqlx::query("SELECT id FROM jobs WHERE idempotency_key = $1")
            .bind(job.idempotency_key)
            .fetch_one(self.pool())
            .await?;
        Ok(Enqueued {
            id: r.try_get(0)?,
            created: false,
        })
    }

    /// Take the next ready job, if any, and hold it for `lease_secs`.
    pub async fn lease(&self, worker_id: &str, lease_secs: i64) -> Result<Option<Job>, StoreError> {
        let t = now();
        // SQLite runs one writer at a time, so its UPDATE is already atomic;
        // Postgres needs SKIP LOCKED so concurrent workers never collide.
        let sql = match self.backend() {
            Backend::Postgres => LEASE_PG,
            Backend::Sqlite => LEASE_SQLITE,
        };
        let row = sqlx::query(sql)
            .bind(worker_id)
            .bind(t + lease_secs)
            .bind(t)
            .fetch_optional(self.pool())
            .await?;
        let Some(r) = row else { return Ok(None) };
        let payload: String = r.try_get("payload")?;
        Ok(Some(Job {
            id: r.try_get("id")?,
            kind: r.try_get("kind")?,
            payload: serde_json::from_str(&payload)
                .map_err(|e| StoreError::Corrupt(e.to_string()))?,
            attempt: r.try_get("attempts")?,
            max_attempts: r.try_get("max_attempts")?,
            trace_context: r.try_get("trace_context")?,
        }))
    }

    /// Extend a lease. Returns false if the job is no longer ours.
    pub async fn heartbeat(
        &self,
        job_id: i64,
        worker_id: &str,
        lease_secs: i64,
    ) -> Result<bool, StoreError> {
        let t = now();
        let r = sqlx::query(
            "UPDATE jobs SET lease_until = $1, updated_at = $2 WHERE id = $3 AND worker_id = $4 AND state = 'running'",
        )
        .bind(t + lease_secs)
        .bind(t)
        .bind(job_id)
        .bind(worker_id)
        .execute(self.pool())
        .await?;
        Ok(r.rows_affected() == 1)
    }

    /// Mark a job done.
    pub async fn complete(&self, job_id: i64, worker_id: &str) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE jobs SET state = 'succeeded', lease_until = NULL, last_error = NULL, updated_at = $1
             WHERE id = $2 AND worker_id = $3 AND state = 'running'",
        )
        .bind(now())
        .bind(job_id)
        .bind(worker_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Record a failure. Retryable failures are re-queued with backoff until
    /// attempts run out.
    pub async fn fail(
        &self,
        job_id: i64,
        worker_id: &str,
        error: &str,
        retryable: bool,
    ) -> Result<(), StoreError> {
        let t = now();
        let row = sqlx::query("SELECT attempts, max_attempts FROM jobs WHERE id = $1 AND worker_id = $2 AND state = 'running'")
            .bind(job_id)
            .bind(worker_id)
            .fetch_optional(self.pool())
            .await?;
        let Some(r) = row else { return Ok(()) };
        let attempts: i64 = r.try_get(0)?;
        let max: i64 = r.try_get(1)?;
        let error: String = error.chars().take(4000).collect();
        if retryable && attempts < max {
            sqlx::query(
                "UPDATE jobs SET state = 'queued', worker_id = NULL, lease_until = NULL, last_error = $1, run_after = $2, updated_at = $3 WHERE id = $4",
            )
            .bind(&error)
            .bind(t + backoff_secs(attempts))
            .bind(t)
            .bind(job_id)
            .execute(self.pool())
            .await?;
        } else {
            sqlx::query("UPDATE jobs SET state = 'failed', lease_until = NULL, last_error = $1, updated_at = $2 WHERE id = $3")
                .bind(&error)
                .bind(t)
                .bind(job_id)
                .execute(self.pool())
                .await?;
        }
        Ok(())
    }

    /// Return jobs whose worker stopped heartbeating to the queue (or fail
    /// them if they are out of attempts). Returns how many were reclaimed.
    pub async fn reap_expired(&self) -> Result<u64, StoreError> {
        let t = now();
        let failed = sqlx::query(
            "UPDATE jobs SET state = 'failed', lease_until = NULL, last_error = 'worker stopped responding (lease expired)', updated_at = $1
             WHERE state = 'running' AND lease_until < $1 AND attempts >= max_attempts",
        )
        .bind(t)
        .execute(self.pool())
        .await?
        .rows_affected();
        let requeued = sqlx::query(
            "UPDATE jobs SET state = 'queued', worker_id = NULL, lease_until = NULL, updated_at = $1
             WHERE state = 'running' AND lease_until < $1",
        )
        .bind(t)
        .execute(self.pool())
        .await?
        .rows_affected();
        Ok(failed + requeued)
    }

    /// Counts by state.
    pub async fn queue_stats(&self) -> Result<QueueStats, StoreError> {
        let rows = sqlx::query("SELECT state, COUNT(*) FROM jobs GROUP BY state")
            .fetch_all(self.pool())
            .await?;
        let mut s = QueueStats::default();
        for r in rows {
            let state: String = r.try_get(0)?;
            let n: i64 = r.try_get(1)?;
            match state.as_str() {
                "queued" => s.queued = n,
                "running" => s.running = n,
                "succeeded" => s.succeeded = n,
                "failed" => s.failed = n,
                _ => {}
            }
        }
        Ok(s)
    }

    /// State and last error of a job.
    pub async fn job_state(
        &self,
        job_id: i64,
    ) -> Result<Option<(String, Option<String>)>, StoreError> {
        let r = sqlx::query("SELECT state, last_error FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(self.pool())
            .await?;
        r.map(|r| Ok((r.try_get(0)?, r.try_get(1)?))).transpose()
    }

    /// Test helper: make a job's lease look expired.
    #[doc(hidden)]
    pub async fn expire_lease_for_test(&self, job_id: i64) -> Result<(), StoreError> {
        sqlx::query("UPDATE jobs SET lease_until = 0 WHERE id = $1")
            .bind(job_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
