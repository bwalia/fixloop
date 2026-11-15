//! Embedded, versioned migrations for both backends.

use crate::{Backend, Store, StoreError, now};

struct Migration {
    version: i64,
    name: &'static str,
    postgres: &'static str,
    sqlite: &'static str,
}

const ID_PG: &str = "BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY";
const ID_SQLITE: &str = "INTEGER PRIMARY KEY AUTOINCREMENT";

const V1: &str = r"
CREATE TABLE jobs (
    id {ID},
    kind TEXT NOT NULL,
    state TEXT NOT NULL,
    priority BIGINT NOT NULL DEFAULT 100,
    payload TEXT NOT NULL,
    idempotency_key TEXT UNIQUE,
    attempts BIGINT NOT NULL DEFAULT 0,
    max_attempts BIGINT NOT NULL DEFAULT 3,
    run_after BIGINT NOT NULL,
    lease_until BIGINT,
    worker_id TEXT,
    last_error TEXT,
    trace_context TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX jobs_ready ON jobs (state, run_after, priority, id);
CREATE TABLE reviews (
    id {ID},
    repo TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    head_sha TEXT NOT NULL,
    job_id BIGINT,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    conclusion TEXT NOT NULL,
    tokens BIGINT NOT NULL,
    cost_usd DOUBLE PRECISION NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX reviews_pr ON reviews (repo, pr_number);
CREATE TABLE findings (
    id {ID},
    review_id BIGINT NOT NULL REFERENCES reviews (id) ON DELETE CASCADE,
    repo TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    title TEXT NOT NULL,
    severity TEXT NOT NULL,
    category TEXT NOT NULL,
    path TEXT NOT NULL,
    line BIGINT NOT NULL,
    end_line BIGINT,
    rationale TEXT NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,
    state TEXT NOT NULL DEFAULT 'open',
    created_at BIGINT NOT NULL
);
CREATE INDEX findings_repo_fingerprint ON findings (repo, fingerprint);
CREATE TABLE audit_events (
    id {ID},
    at BIGINT NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    subject TEXT NOT NULL,
    detail TEXT NOT NULL
);
CREATE INDEX audit_events_at ON audit_events (at);
";

fn migrations() -> Vec<Migration> {
    vec![Migration {
        version: 1,
        name: "jobs, reviews, findings, audit",
        postgres: V1,
        sqlite: V1,
    }]
}

pub(crate) async fn run(store: &Store) -> Result<(), StoreError> {
    let pool = store.pool();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS fixloop_migrations (version BIGINT PRIMARY KEY, name TEXT NOT NULL, applied_at BIGINT NOT NULL)",
    )
    .execute(pool)
    .await?;
    for m in migrations() {
        let mut tx = pool.begin().await?;
        // Re-check inside the transaction; on Postgres take a lock so two
        // starting servers do not race.
        if store.backend() == Backend::Postgres {
            sqlx::query("LOCK TABLE fixloop_migrations IN EXCLUSIVE MODE")
                .execute(&mut *tx)
                .await?;
        }
        let done: Option<(i64,)> =
            sqlx::query_as("SELECT version FROM fixloop_migrations WHERE version = $1")
                .bind(m.version)
                .fetch_optional(&mut *tx)
                .await?;
        if done.is_some() {
            tx.rollback().await?;
            continue;
        }
        let (sql, id) = match store.backend() {
            Backend::Postgres => (m.postgres, ID_PG),
            Backend::Sqlite => (m.sqlite, ID_SQLITE),
        };
        let ddl = sql.replace("{ID}", id);
        for stmt in ddl.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            // Built only from the constants above, never from input.
            sqlx::query(sqlx::AssertSqlSafe(stmt.to_owned()))
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "INSERT INTO fixloop_migrations (version, name, applied_at) VALUES ($1, $2, $3)",
        )
        .bind(m.version)
        .bind(m.name)
        .bind(now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(
            version = m.version,
            name = m.name,
            "applied database migration"
        );
    }
    Ok(())
}
