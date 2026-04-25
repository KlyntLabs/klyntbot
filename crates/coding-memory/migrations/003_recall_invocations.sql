-- Phase 4 telemetry: every recall invocation (passive or active).
CREATE TABLE IF NOT EXISTS recall_invocations (
    id              TEXT PRIMARY KEY,
    occurred_at     TEXT NOT NULL,
    session_id      TEXT,
    turn_id         TEXT,
    repo_id         TEXT,
    layer           TEXT NOT NULL,            -- 'index' | 'timeline' | 'fetch' | 'dead_end' |
                                              -- 'facts_as_of' | 'change_history' | 'decision_points' |
                                              -- 'session_start_inject' | 'user_prompt_inject'
    query           TEXT NOT NULL,
    coverage_score  REAL,
    skill_used      TEXT,                     -- empty if no escalation; csv otherwise
    latency_ms      INTEGER NOT NULL,
    result_ids      TEXT NOT NULL,            -- JSON array of UUIDs
    rendered_tokens INTEGER,                  -- only set for inject layers
    metadata        TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_recall_invocations_session
    ON recall_invocations(session_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_recall_invocations_repo
    ON recall_invocations(repo_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_recall_invocations_layer
    ON recall_invocations(layer, occurred_at DESC);
