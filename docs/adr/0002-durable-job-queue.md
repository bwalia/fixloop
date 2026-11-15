# ADR-0002: Durable job queue in the database (Postgres, SQLite fallback)

* Status: Accepted (implemented in Phase 2)
* Date: 2026-11-14

## Context
Jobs (review, fix, bugfix, sync, scan, eval) must survive restarts, scale across workers, support priorities, retries, cancellation, "waiting for approval" pauses and per-repo concurrency limits. The brief asks for a durable queue in Postgres and a SQLite single-user mode. Adding Redis/NATS/Kafka would add an operational dependency.

## Decision
Implement our own queue on the `job` table in `fixloop-store`:

* **Postgres**: lease with `SELECT … FOR UPDATE SKIP LOCKED` → set `state='leased', lease_until=now()+ttl, worker_id`. Workers heartbeat to extend the lease. A reaper requeues expired leases. `LISTEN/NOTIFY` wakes idle workers, with polling as a fallback.
* **SQLite**: single process, so leasing is a transactional `UPDATE … RETURNING` under SQLite's writer lock. No NOTIFY; an in-process channel wakes workers instead.
* Enqueue happens **in the same transaction** as the domain write and audit event (transactional outbox by construction).
* Idempotency key per job (e.g. webhook delivery id + kind) with a unique index.
* Retries use exponential backoff with jitter. `max_attempts` is per kind. Poison jobs are moved to `failed` with the full error.
* Per-repo concurrency: a lease query predicate that counts running jobs per repo.
* Each job stores its `traceparent`, so the trace continues from webhook to worker.

## Alternatives rejected
* Existing crates (`apalis`, `graphile_worker`-style ports): evaluated as options. We would still need SQLite parity, approval pauses and our own state machine. If a crate fits cleanly at implementation time, we adopt it and amend this ADR.
* An external broker: an extra moving part for small teams and air-gapped installs.

## Consequences
Throughput is bounded by Postgres, but that is well above our needs: jobs are minutes long and LLM-bound. The queue is behind a `JobQueue` trait, so a broker backend could be added later.

## Implementation notes (Phase 2)
* One code path for both databases through sqlx's `Any` driver. Postgres uses `FOR UPDATE SKIP LOCKED` in the lease query; SQLite runs with a single connection, which makes the lease `UPDATE` atomic.
* Migrations are embedded and versioned (`fixloop_migrations` table). On Postgres the table is locked so two starting servers cannot race.
* Leases last 120 s and are extended every 30 s. Each worker loop reclaims expired leases, and a job that runs out of attempts while its worker is gone fails with "worker stopped responding".
* Retries back off 30 s, 2 m, 8 m, … up to 1 h. Errors are classed retryable (model or GitHub outages, rate limits) or permanent (bad config, missing access).
* Webhook deliveries are deduplicated with the idempotency key `github:<X-GitHub-Delivery>`.
* `LISTEN/NOTIFY` wake-ups are not implemented yet. Idle workers poll every 2 s, which is fine at current scale.
* Tests run on SQLite always and on Postgres in CI, each test in its own fresh database.
