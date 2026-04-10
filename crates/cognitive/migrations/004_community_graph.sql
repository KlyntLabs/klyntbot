-- Community graph tables for Phase 2 Cognitive Fabric
-- Communities are clusters of related tree nodes discovered by Louvain
-- over shared-entity edges.

CREATE TABLE IF NOT EXISTS communities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    summary TEXT NOT NULL,
    member_count INTEGER NOT NULL DEFAULT 0,
    modularity_score REAL,
    stability REAL NOT NULL DEFAULT 1.0,
    top_entities TEXT,
    representative_paths TEXT,
    source_note_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_restructured_at TEXT      -- timestamp of last merge/split by Reforge
);

CREATE TABLE IF NOT EXISTS community_members (
    community_id TEXT NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    tree_node_id TEXT NOT NULL REFERENCES book_tree_nodes(id) ON DELETE CASCADE,
    membership_score REAL NOT NULL DEFAULT 0.0,
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (community_id, tree_node_id)
);

CREATE INDEX IF NOT EXISTS idx_community_members_node ON community_members(tree_node_id);
CREATE INDEX IF NOT EXISTS idx_communities_stability ON communities(stability);
