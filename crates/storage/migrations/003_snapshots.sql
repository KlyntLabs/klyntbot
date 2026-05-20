-- Migration: create snapshots table for unified assistant file edit tracking
-- Created: 2026-05-20

CREATE TABLE IF NOT EXISTS snapshots (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key                 TEXT NOT NULL,
    message_id                  TEXT,
    file_path                   TEXT NOT NULL,
    content_before              BLOB NOT NULL,
    file_existed                INTEGER NOT NULL DEFAULT 0,
    content_hash                TEXT NOT NULL,
    ghost_commit_sha            TEXT,
    ghost_repo_root             TEXT,
    ghost_preexisting_untracked_json TEXT,
    created_at                  INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_snapshots_session_key ON snapshots(session_key);
CREATE INDEX IF NOT EXISTS idx_snapshots_message_id ON snapshots(message_id);
