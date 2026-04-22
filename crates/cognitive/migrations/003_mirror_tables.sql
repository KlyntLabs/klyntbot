-- Mirror Phase 1 tables

CREATE TABLE IF NOT EXISTS mirror_routing_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 1,
    total_messages INTEGER NOT NULL,
    distribution_json TEXT NOT NULL,
    fallback_rate REAL NOT NULL,
    avg_routing_confidence REAL NOT NULL,
    low_confidence_count INTEGER NOT NULL DEFAULT 0,
    user_feedback TEXT
);
CREATE INDEX IF NOT EXISTS idx_routing_snapshots_time ON mirror_routing_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS mirror_trend_narratives (
    id TEXT PRIMARY KEY,
    generated_at TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    routing_summary TEXT NOT NULL,
    improvement_highlights_json TEXT NOT NULL,
    experiment_summary TEXT NOT NULL,
    meta_rule_updates_json TEXT NOT NULL,
    full_narrative TEXT NOT NULL,
    user_feedback TEXT
);
CREATE INDEX IF NOT EXISTS idx_trend_narratives_time ON mirror_trend_narratives(generated_at);

CREATE TABLE IF NOT EXISTS mirror_snippets (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    alert_type TEXT NOT NULL,
    headline TEXT NOT NULL,
    body TEXT NOT NULL,
    action_json TEXT,
    user_feedback TEXT,
    dismissed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_snippets_created ON mirror_snippets(created_at);

CREATE TABLE IF NOT EXISTS mirror_meta_rules (
    id TEXT PRIMARY KEY,
    trigger_condition TEXT NOT NULL,
    action_json TEXT NOT NULL,
    source TEXT NOT NULL,
    effectiveness_score REAL NOT NULL DEFAULT 0.5,
    status TEXT NOT NULL DEFAULT 'pending',
    signal_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_meta_rules_status ON mirror_meta_rules(status);

CREATE TABLE IF NOT EXISTS mirror_brain_versions (
    version INTEGER PRIMARY KEY,
    trial_id TEXT,
    promoted_at TEXT NOT NULL,
    params_json TEXT NOT NULL,
    reason TEXT NOT NULL,
    parent_version INTEGER,
    metrics_json TEXT NOT NULL,
    reverted INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS mirror_trial_previews (
    id TEXT PRIMARY KEY,
    trial_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    preview_at TEXT NOT NULL,
    messages_scored INTEGER NOT NULL,
    early_signals_json TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    narrative TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_trial_previews_time ON mirror_trial_previews(preview_at);
CREATE INDEX IF NOT EXISTS idx_trial_previews_trial_id ON mirror_trial_previews(trial_id);

CREATE TABLE IF NOT EXISTS mirror_task_focus_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 1,
    focus_changes INTEGER NOT NULL,
    tasks_completed INTEGER NOT NULL,
    completion_rate REAL NOT NULL,
    longest_unfinished_secs INTEGER,
    top_tasks_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_focus_snapshots_time ON mirror_task_focus_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS mirror_finance_drift_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 24,
    total_transactions INTEGER NOT NULL,
    over_budget_count INTEGER NOT NULL,
    per_category_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_finance_drift_snapshots_time ON mirror_finance_drift_snapshots(captured_at);
