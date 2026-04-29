-- KCA Track 11: entity-community membership for online clustering.
CREATE TABLE IF NOT EXISTS entity_community_members (
    community_id TEXT NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    membership_score REAL NOT NULL DEFAULT 0.0,
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (community_id, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_community_members_entity ON entity_community_members(entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_community_members_community ON entity_community_members(community_id);
