-- Mirror Phase 2: coding todo snapshots

CREATE TABLE IF NOT EXISTS mirror_todo_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 1,
    status_changes INTEGER NOT NULL DEFAULT 0,
    cancellations INTEGER NOT NULL DEFAULT 0,
    plans_proposed INTEGER NOT NULL DEFAULT 0,
    plans_ratified INTEGER NOT NULL DEFAULT 0,
    blocked_reason_clusters_json TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_todo_snapshots_time ON mirror_todo_snapshots(captured_at);
