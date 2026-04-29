-- KCA Track 8: hierarchical episodic compression.
ALTER TABLE episodic_memories ADD COLUMN tier TEXT NOT NULL DEFAULT 'raw'
    CHECK (tier IN ('raw', 'hourly', 'daily', 'weekly'));
ALTER TABLE episodic_memories ADD COLUMN parent_id TEXT;
ALTER TABLE episodic_memories ADD COLUMN child_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_episodic_tier_recorded
    ON episodic_memories(tier, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_episodic_parent
    ON episodic_memories(parent_id);

-- Track which raw episodics have been rolled into hourly summaries.
ALTER TABLE episodic_memories ADD COLUMN rolled_up_at TEXT;
