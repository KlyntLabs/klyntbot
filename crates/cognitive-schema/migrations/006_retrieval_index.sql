-- ── AI signal retrieval index ─────────────────────────────────────
-- Projects AiSignal events into a queryable index for recall/retrieval.
-- Writes are filtered: Discard-salience and very-low-importance signals
-- are skipped so the index stays small relative to domain_event_log.

CREATE TABLE IF NOT EXISTS ai_signal_index (
    id TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    content TEXT NOT NULL,
    importance REAL NOT NULL,
    salience TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    timestamp TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_signal_domain_ts
    ON ai_signal_index(domain, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_ai_signal_domain_importance
    ON ai_signal_index(domain, importance DESC);
