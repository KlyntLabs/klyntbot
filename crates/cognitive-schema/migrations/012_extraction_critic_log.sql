-- KCA Track 5: log every critic verdict for nightly re-evaluation by Reforge.
CREATE TABLE IF NOT EXISTS extraction_critic_log (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL,
    verdict TEXT NOT NULL CHECK (verdict IN ('grounded', 'hallucinated', 'ambiguous')),
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    reviewed_by_reforge_at TEXT,
    FOREIGN KEY (fact_id) REFERENCES semantic_facts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_extraction_critic_unreviewed
    ON extraction_critic_log(reviewed_by_reforge_at) WHERE reviewed_by_reforge_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_extraction_critic_fact
    ON extraction_critic_log(fact_id);
