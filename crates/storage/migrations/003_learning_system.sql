-- User profile: explicit facts about the user
CREATE TABLE IF NOT EXISTS user_profile (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'user_explicit',
    confidence REAL NOT NULL DEFAULT 1.0,
    agent_name TEXT,
    last_confirmed TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(category, key)
);

-- Behavioral patterns: observed from interactions
CREATE TABLE IF NOT EXISTS behavioral_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_type TEXT NOT NULL,
    pattern_key TEXT NOT NULL,
    pattern_value TEXT NOT NULL DEFAULT '{}',
    sample_count INTEGER NOT NULL DEFAULT 0,
    last_updated TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(pattern_type, pattern_key)
);

-- Agent adaptations: per-agent user preferences
CREATE TABLE IF NOT EXISTS agent_adaptations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_name TEXT NOT NULL,
    preference_key TEXT NOT NULL,
    preference_value TEXT NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'satisfaction_signal',
    confidence REAL NOT NULL DEFAULT 0.5,
    last_updated TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(agent_name, preference_key)
);

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
