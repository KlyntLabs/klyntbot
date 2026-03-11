-- Dead-letter queue for observations that failed LLM processing.
-- Observations are stored for later reprocessing when the LLM recovers.
CREATE TABLE IF NOT EXISTS failed_observations (
    id TEXT PRIMARY KEY,
    observation_json TEXT NOT NULL,
    failure_reason TEXT NOT NULL,
    failed_stage TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    next_retry_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_failed_observations_eligible
    ON failed_observations(retry_count, next_retry_at);
