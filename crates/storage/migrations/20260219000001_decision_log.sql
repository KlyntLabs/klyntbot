-- G-17: Decision log (replaces ~/.klyntbot/decision_log.jsonl)
--
-- Stores confidence assessment decisions for auditing and learning analysis.
-- Replaces append-only JSONL file at ~/.klyntbot/decision_log.jsonl.

CREATE TABLE decision_log (
    id                    TEXT PRIMARY KEY,
    session_key           TEXT NOT NULL,
    iteration             INTEGER NOT NULL,
    tool_names            JSONB NOT NULL DEFAULT '[]',
    user_message_preview  TEXT NOT NULL DEFAULT '',
    assessment            JSONB NOT NULL DEFAULT '{}',
    outcome               TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_decision_log_session ON decision_log(session_key);
CREATE INDEX idx_decision_log_created ON decision_log(created_at DESC);
