-- Distillation retry queue — transient LLM failures park here until the
-- provider recovers. Phase-1 consolidated migration excluded this because it
-- belongs to the Phase-3 Distiller; its presence never changes reads in prior
-- phases.

CREATE TABLE IF NOT EXISTS ingest_distillation_retry (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    turn_id         TEXT,
    reason          TEXT NOT NULL,
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    next_due_at     TEXT NOT NULL DEFAULT (datetime('now')),
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_distillation_retry_due
    ON ingest_distillation_retry(next_due_at);
