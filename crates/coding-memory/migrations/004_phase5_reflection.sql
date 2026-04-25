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
-- Coding-alert kind / severity columns on the existing mirror_snippets table
-- (mirror_snippets is owned by cognitive; we ALTER it in-place per
-- pre-release policy — no separate cross-crate migration needed)
-- ─────────────────────────────────────────────────────────────────────
ALTER TABLE mirror_snippets ADD COLUMN coding_alert_kind     TEXT NULL;
ALTER TABLE mirror_snippets ADD COLUMN coding_alert_severity TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_mirror_snippets_coding_kind
    ON mirror_snippets(coding_alert_kind, created_at)
    WHERE coding_alert_kind IS NOT NULL;
