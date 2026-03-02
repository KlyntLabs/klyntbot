-- Session context: categorizes chat sessions by entity type and PARA ancestry.
CREATE TABLE IF NOT EXISTS session_context (
    session_key  TEXT PRIMARY KEY REFERENCES sessions(key) ON DELETE CASCADE,
    context_type TEXT NOT NULL DEFAULT 'general',
    entity_kind  TEXT,
    entity_id    TEXT,
    area_id      TEXT,
    project_id   TEXT,
    is_ephemeral INTEGER NOT NULL DEFAULT 0,
    is_pinned    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_session_context_area ON session_context(area_id);
CREATE INDEX IF NOT EXISTS idx_session_context_entity ON session_context(entity_kind, entity_id);
CREATE INDEX IF NOT EXISTS idx_session_context_ephemeral ON session_context(is_ephemeral, is_pinned);
