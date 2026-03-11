-- Interaction log: raw data for pattern analysis
CREATE TABLE IF NOT EXISTS interaction_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    agent_name TEXT NOT NULL,
    tool_names TEXT NOT NULL DEFAULT '[]',
    channel TEXT NOT NULL,
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_interaction_log_timestamp
    ON interaction_log(timestamp DESC);
