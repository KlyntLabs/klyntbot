-- KCA Track 12: candidate skills proposed by Reforge for user approval.
CREATE TABLE IF NOT EXISTS skill_proposals (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    yaml_frontmatter TEXT NOT NULL,
    body_markdown TEXT NOT NULL,
    source_rule_ids TEXT NOT NULL, -- JSON array
    avg_confidence REAL NOT NULL,
    proposed_at TEXT NOT NULL DEFAULT (datetime('now')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'superseded')),
    decided_at TEXT,
    decided_by TEXT,
    reforge_run_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_skill_proposals_status ON skill_proposals(status);
CREATE INDEX IF NOT EXISTS idx_skill_proposals_proposed_at ON skill_proposals(proposed_at DESC);
