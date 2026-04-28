-- 006_recall_weights.sql
-- Persisted 12-axis recall ranking weights. Single-row table keyed on 'local'.
CREATE TABLE IF NOT EXISTS recall_weights (
    id TEXT PRIMARY KEY DEFAULT 'local',
    weights TEXT NOT NULL,           -- JSON array of 12 f64
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    source TEXT NOT NULL DEFAULT 'default'  -- 'default' | 'reforge_trained' | 'manual'
);
INSERT OR IGNORE INTO recall_weights (id, weights, source) VALUES (
    'local',
    '[0.35,0.05,0.10,0.05,0.05,0.20,0.05,0.05,0.02,0.02,0.05,0.01]',
    'default'
);
