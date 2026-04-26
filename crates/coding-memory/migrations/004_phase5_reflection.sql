-- Phase 5 — Reflection (Reforge + Mirror).
-- Pre-release direct DDL per CLAUDE.md.

-- ─────────────────────────────────────────────────────────────────────
-- Session-end light pass cache (read by Phase 4 SessionStart "open threads")
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS session_summaries (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL,
    repo_id      TEXT,
    summarised_at TEXT NOT NULL DEFAULT (datetime('now')),
    summary_md   TEXT NOT NULL,
    token_count  INTEGER NOT NULL,
    actor_id     TEXT NOT NULL DEFAULT 'local_user'
);
CREATE INDEX IF NOT EXISTS idx_session_summaries_session
    ON session_summaries(session_id);
CREATE INDEX IF NOT EXISTS idx_session_summaries_repo
    ON session_summaries(repo_id, summarised_at);

-- ─────────────────────────────────────────────────────────────────────
-- Pattern effectiveness EMA log (PatternEffectivenessSource feed)
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS pattern_effectiveness_log (
    id              TEXT PRIMARY KEY,
    pattern_id      TEXT NOT NULL,
    pattern_kind    TEXT NOT NULL,    -- 'workflow_pattern'|'failure_pattern'|'project_skill'|'retrieval_skill'
    repo_id         TEXT,
    measured_at     TEXT NOT NULL DEFAULT (datetime('now')),
    outcome         TEXT NOT NULL,    -- 'success'|'partial'|'failure'|'inconclusive'
    score_before    REAL NOT NULL,
    score_after     REAL NOT NULL,
    evidence        TEXT
);
CREATE INDEX IF NOT EXISTS idx_pattern_eff_log_pattern
    ON pattern_effectiveness_log(pattern_id, measured_at);
CREATE INDEX IF NOT EXISTS idx_pattern_eff_log_repo
    ON pattern_effectiveness_log(repo_id, measured_at);

-- ─────────────────────────────────────────────────────────────────────
-- Selective-delete signal audit log (Phase 6 stability halvings)
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS selective_delete_log (
    id                   TEXT PRIMARY KEY,
    memory_id            TEXT NOT NULL,
    memory_kind          TEXT NOT NULL,    -- 'semantic_fact'|'episodic_memory'
    retrievals_observed  INTEGER NOT NULL,
    citations_observed   INTEGER NOT NULL,
    stability_before     REAL NOT NULL,
    stability_after      REAL NOT NULL,
    applied_at           TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_selective_delete_memory
    ON selective_delete_log(memory_id, applied_at);

-- ─────────────────────────────────────────────────────────────────────
-- skill_versions extensions for project-skill evolver
-- (coding_alert_kind / coding_alert_severity on mirror_snippets are added
-- by cognitive migration 008; do not duplicate here.)
-- ─────────────────────────────────────────────────────────────────────
ALTER TABLE skill_versions ADD COLUMN source_pattern_id TEXT NULL;
ALTER TABLE skill_versions ADD COLUMN status TEXT DEFAULT 'active';

CREATE INDEX IF NOT EXISTS idx_skill_versions_source_pattern
    ON skill_versions(source_pattern_id) WHERE source_pattern_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_skill_versions_status
    ON skill_versions(status, scope, scope_repo_id);
