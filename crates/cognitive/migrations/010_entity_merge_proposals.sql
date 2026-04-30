-- KCA Track 2: log entity merge proposals from the per-turn graph linker.
-- Actual merging is deferred to nightly Reforge Phase 6.5 to avoid
-- corrupting in-flight state.
CREATE TABLE IF NOT EXISTS entity_merge_proposals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_a_id TEXT NOT NULL,
    entity_b_id TEXT NOT NULL,
    canonical_name TEXT NOT NULL,
    reason TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    applied_at TEXT,
    FOREIGN KEY (entity_a_id) REFERENCES entities(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_b_id) REFERENCES entities(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entity_merge_proposals_pending
    ON entity_merge_proposals(applied_at) WHERE applied_at IS NULL;
