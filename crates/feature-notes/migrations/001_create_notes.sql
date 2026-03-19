-- Feature migration: notes tables
CREATE TABLE IF NOT EXISTS notebooks (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT REFERENCES notebooks(id) ON DELETE SET NULL,
    title       TEXT NOT NULL CHECK(length(trim(title)) > 0),
    icon        TEXT,
    color       TEXT,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_notebooks_parent_id ON notebooks(parent_id);

CREATE TABLE IF NOT EXISTS notes (
    id          TEXT PRIMARY KEY,
    notebook_id TEXT REFERENCES notebooks(id) ON DELETE SET NULL,
    title       TEXT NOT NULL CHECK(length(trim(title)) > 0),
    body        TEXT NOT NULL DEFAULT '',
    body_html   TEXT,
    pinned      INTEGER NOT NULL DEFAULT 0,
    archived    INTEGER NOT NULL DEFAULT 0,
    icon        TEXT,
    color       TEXT,
    embedding_updated_at TEXT,
    split_content   TEXT,
    split_mode      TEXT,
    perspective_config TEXT,
    last_visited_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_notes_notebook_id ON notes(notebook_id);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(pinned) WHERE pinned = 1;
CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);
CREATE INDEX IF NOT EXISTS idx_notes_archived ON notes(archived) WHERE archived = 1;

CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);

CREATE TABLE IF NOT EXISTS note_links (
    source_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, target_id),
    CHECK (source_id != target_id)
);

CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_id);

CREATE TABLE IF NOT EXISTS note_entity_mentions (
    note_id     TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    PRIMARY KEY (note_id, entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_note_entity_mentions_entity
    ON note_entity_mentions(entity_type, entity_id);

CREATE TABLE IF NOT EXISTS note_versions (
    id         TEXT PRIMARY KEY,
    note_id    TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_note_versions_note_id ON note_versions(note_id);

-- FTS5 virtual table for full-text search on notes (external content)
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    body,
    content='notes',
    content_rowid='rowid',
    tokenize='porter'
);

-- Triggers to keep FTS5 index in sync with notes table
CREATE TRIGGER IF NOT EXISTS notes_fts_insert AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;

CREATE TRIGGER IF NOT EXISTS notes_fts_delete AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body);
END;

CREATE TRIGGER IF NOT EXISTS notes_fts_update AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body);
    INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;

-- Inbox items table
CREATE TABLE IF NOT EXISTS inbox_items (
    id         TEXT PRIMARY KEY,
    content    TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_inbox_items_status ON inbox_items(status);
