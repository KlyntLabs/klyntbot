-- Migration: drop coding-mode artifacts for unified assistant
-- Created: 2026-05-20

-- Drop coding-only tables (if they exist from previous dev installs)
DROP TABLE IF EXISTS coding_snapshots;
DROP TABLE IF EXISTS coding_approval_history;
DROP TABLE IF EXISTS coding_todos;
DROP TABLE IF EXISTS coding_reviews;
DROP TABLE IF EXISTS coding_background_jobs;

-- Relax sessions CHECK constraint: recreate table without coding columns
-- SQLite doesn't support ALTER TABLE DROP COLUMN, so we recreate.
CREATE TABLE sessions_new (
    key        TEXT PRIMARY KEY,
    mode       TEXT NOT NULL DEFAULT 'assistant'
                 CHECK (mode IN ('assistant', 'subagent')),
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0,
    compressed_prefix      TEXT,
    compressed_through_idx INTEGER,
    compressed_at          INTEGER,
    approval_mode          TEXT NOT NULL DEFAULT 'default',
    total_cost_usd         REAL NOT NULL DEFAULT 0,
    total_tokens           INTEGER NOT NULL DEFAULT 0,
    parent_session_id      TEXT REFERENCES sessions(key) ON DELETE SET NULL,
    workspace_id           TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    forked_from_id         TEXT REFERENCES sessions(key) ON DELETE SET NULL,
    summary_message_id     TEXT,
    ephemeral              INTEGER NOT NULL DEFAULT 0,
    archived_at            INTEGER,
    last_event_at          INTEGER
);

INSERT INTO sessions_new SELECT
    key, mode, metadata, created_at, updated_at, project_id, conversation_type, pinned,
    compressed_prefix, compressed_through_idx, compressed_at, approval_mode, total_cost_usd,
    total_tokens, parent_session_id, workspace_id, forked_from_id, summary_message_id,
    ephemeral, archived_at, last_event_at
FROM sessions;

DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;

CREATE INDEX IF NOT EXISTS idx_sessions_mode ON sessions(mode);
CREATE INDEX IF NOT EXISTS idx_sessions_mode_updated_at ON sessions(mode, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON sessions(workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_forked_from ON sessions(forked_from_id);
CREATE INDEX IF NOT EXISTS idx_sessions_archived ON sessions(archived_at);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_last_event ON sessions(last_event_at DESC);
