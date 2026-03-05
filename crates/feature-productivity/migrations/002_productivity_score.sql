-- Add productivity score to daily summaries
ALTER TABLE daily_summaries ADD COLUMN productivity_score REAL;

-- Goals table for daily/weekly targets
CREATE TABLE IF NOT EXISTS productivity_goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_type     TEXT NOT NULL DEFAULT 'daily',
    metric        TEXT NOT NULL,
    target_value  REAL NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Manual time entries
CREATE TABLE IF NOT EXISTS time_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    description   TEXT NOT NULL,
    category_id   TEXT REFERENCES activity_categories(id),
    project_id    TEXT,
    started_at    TEXT NOT NULL,
    duration_secs INTEGER NOT NULL,
    source        TEXT NOT NULL DEFAULT 'manual',
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_time_entries_started ON time_entries(started_at DESC);
