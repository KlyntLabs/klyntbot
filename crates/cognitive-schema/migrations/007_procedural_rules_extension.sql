-- Extend procedural_rules for Phase-5 coding synthesis.

ALTER TABLE procedural_rules ADD COLUMN effectiveness_score REAL NOT NULL DEFAULT 0.5;
ALTER TABLE procedural_rules ADD COLUMN stability REAL NOT NULL DEFAULT 1.0;
ALTER TABLE procedural_rules ADD COLUMN scope_repo_id TEXT;
ALTER TABLE procedural_rules ADD COLUMN last_applied TEXT;
ALTER TABLE procedural_rules ADD COLUMN application_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE procedural_rules ADD COLUMN metadata TEXT;

CREATE INDEX IF NOT EXISTS idx_procedural_scope_repo ON procedural_rules(scope_repo_id);
