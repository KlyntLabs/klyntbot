-- Productivity V2: buckets, distraction patterns, insights, focus source

-- 5-minute activity buckets (365-day retention)
CREATE TABLE IF NOT EXISTS activity_buckets (
    bucket_start      TEXT NOT NULL,
    date              TEXT NOT NULL,
    dominant_app      TEXT,
    dominant_site     TEXT,
    dominant_category TEXT,
    productive_secs   INTEGER NOT NULL DEFAULT 0,
    neutral_secs      INTEGER NOT NULL DEFAULT 0,
    distracting_secs  INTEGER NOT NULL DEFAULT 0,
    idle_secs         INTEGER NOT NULL DEFAULT 0,
    context_switches  INTEGER NOT NULL DEFAULT 0,
    focus_depth       REAL,
    tick_count        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket_start)
);
CREATE INDEX IF NOT EXISTS idx_buckets_date ON activity_buckets(date);

-- Distraction pattern tracking
CREATE TABLE IF NOT EXISTS distraction_patterns (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    date                  TEXT NOT NULL,
    hour_of_day           INTEGER NOT NULL,
    hours_active_today    REAL NOT NULL,
    mins_since_break      REAL NOT NULL,
    preceding_app         TEXT,
    preceding_category    TEXT,
    preceding_duration_mins REAL,
    distraction_app       TEXT NOT NULL,
    distraction_category  TEXT,
    recovery_secs         INTEGER,
    created_at            TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_distraction_date ON distraction_patterns(date);

-- Heuristic insight cards
CREATE TABLE IF NOT EXISTS insight_cards (
    id              TEXT PRIMARY KEY,
    insight_type    TEXT NOT NULL,
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    sentiment       TEXT NOT NULL,
    metric_value    REAL,
    baseline_value  REAL,
    date            TEXT NOT NULL,
    dismissed       BOOLEAN NOT NULL DEFAULT FALSE,
    generated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_insights_date ON insight_cards(date);
CREATE UNIQUE INDEX IF NOT EXISTS idx_insights_type_date ON insight_cards(insight_type, date);

-- Add source column to focus_sessions
ALTER TABLE focus_sessions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

-- Add deep work fields to daily_summaries
ALTER TABLE daily_summaries ADD COLUMN deep_work_blocks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_summaries ADD COLUMN deep_work_secs INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_summaries ADD COLUMN avg_recovery_secs REAL;
