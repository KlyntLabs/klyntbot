-- KCA Track 4: track last micro-Reforge run + counters.
CREATE TABLE IF NOT EXISTS micro_reforge_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_run_at TEXT,
    turns_since_last_run INTEGER NOT NULL DEFAULT 0,
    last_turn_count_at_run INTEGER NOT NULL DEFAULT 0,
    total_runs INTEGER NOT NULL DEFAULT 0,
    total_rules_promoted INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO micro_reforge_state (id, last_run_at, turns_since_last_run) VALUES (1, NULL, 0);

-- Audit log of each micro-Reforge invocation.
CREATE TABLE IF NOT EXISTS micro_reforge_runs (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    trigger TEXT NOT NULL CHECK (trigger IN ('turn_threshold', 'minute_threshold', 'manual')),
    turn_count_at_run INTEGER NOT NULL,
    proposed_rule_count INTEGER NOT NULL DEFAULT 0,
    accepted_rule_count INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_micro_reforge_runs_started ON micro_reforge_runs(started_at DESC);
