-- KCA Track 10/12: actor source for cross-CLI cognitive transfer and skill discovery.
ALTER TABLE episodic_memories ADD COLUMN actor_id TEXT;

CREATE INDEX IF NOT EXISTS idx_episodic_actor_id
    ON episodic_memories(actor_id);
