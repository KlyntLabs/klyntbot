-- Persist accumulated observations so they survive restarts.
-- The background consolidation service buffers low-salience events here
-- until they cross the promotion threshold (≥5 events across ≥3 days).

CREATE TABLE IF NOT EXISTS accumulated_observations (
    id              TEXT PRIMARY KEY,
    event_type_key  TEXT NOT NULL,
    domain          TEXT NOT NULL,
    content         TEXT NOT NULL,
    importance      REAL NOT NULL,
    source_event    TEXT NOT NULL,
    observed_at     TEXT NOT NULL,
    day_key         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_accum_event_type ON accumulated_observations(event_type_key);
