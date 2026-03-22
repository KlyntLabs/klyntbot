CREATE TABLE IF NOT EXISTS practice_sessions (
    id                   TEXT PRIMARY KEY,
    note_id              TEXT NOT NULL,
    source_lang          TEXT NOT NULL,
    target_lang          TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'in_progress',
    segments             TEXT NOT NULL,
    current_index        INTEGER NOT NULL DEFAULT 0,
    results              TEXT NOT NULL DEFAULT '[]',
    user_translation_doc TEXT,
    average_score        REAL,
    started_at           TEXT NOT NULL,
    completed_at         TEXT,
    updated_at           TEXT NOT NULL DEFAULT (datetime('now')),
    created_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_practice_sessions_note_id ON practice_sessions(note_id);
CREATE INDEX IF NOT EXISTS idx_practice_sessions_status ON practice_sessions(status);
