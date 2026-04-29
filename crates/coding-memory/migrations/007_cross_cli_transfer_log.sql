-- KCA Track 10: log cross-CLI rule promotions.
CREATE TABLE IF NOT EXISTS cross_cli_transfer_log (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL,
    rule_text_snapshot TEXT NOT NULL,
    from_sources TEXT NOT NULL, -- JSON array
    promoted_to_sources TEXT NOT NULL, -- JSON array
    support_strength INTEGER NOT NULL,
    decided_at TEXT NOT NULL DEFAULT (datetime('now')),
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected')),
    reason TEXT,
    reforge_run_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_cross_cli_transfer_rule ON cross_cli_transfer_log(rule_id);
CREATE INDEX IF NOT EXISTS idx_cross_cli_transfer_decided ON cross_cli_transfer_log(decided_at DESC);
