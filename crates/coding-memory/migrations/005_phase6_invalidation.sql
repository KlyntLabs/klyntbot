-- Phase-6: pending invalidations queue + anchored-symbol functional indexes.
--
-- Pre-release (per CLAUDE.md) — direct schema additions are authorized.

-- Holds GitCommit events that arrived while desktop was offline. Drained on
-- next daemon startup; invalidation runs against the recorded commit info.
CREATE TABLE IF NOT EXISTS pending_invalidations (
    id            TEXT PRIMARY KEY,
    repo_root     TEXT NOT NULL,
    commit_hash   TEXT NOT NULL,
    parent_hash   TEXT,
    changed_files TEXT NOT NULL,             -- JSON array of relative paths
    received_at   TEXT NOT NULL DEFAULT (datetime('now')),
    processed_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_pending_invalidations_unprocessed
    ON pending_invalidations(received_at)
    WHERE processed_at IS NULL;

-- Functional indexes for fast anchored-symbol lookup.
-- Used by `GitInvalidationHandler` to find facts anchored to changed files.
CREATE INDEX IF NOT EXISTS idx_anchored_symbol_file_facts
    ON semantic_facts(json_extract(metadata, '$.anchoredSymbols'))
    WHERE json_extract(metadata, '$.anchoredSymbols') IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_anchored_symbol_file_episodic
    ON episodic_memories(json_extract(metadata, '$.anchoredSymbols'))
    WHERE json_extract(metadata, '$.anchoredSymbols') IS NOT NULL;
