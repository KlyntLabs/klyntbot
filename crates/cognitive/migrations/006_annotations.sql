CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT DEFAULT '',
    author TEXT NOT NULL DEFAULT 'agent',
    priority INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    access_count INTEGER DEFAULT 0,
    UNIQUE(target_type, target_id, content)
);

CREATE INDEX IF NOT EXISTS idx_annotations_target ON annotations(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_annotations_tags ON annotations(tags);
CREATE INDEX IF NOT EXISTS idx_annotations_priority ON annotations(priority);

-- FTS5 for annotation search
CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts USING fts5(
    id UNINDEXED,
    target_type,
    target_id,
    content,
    tags,
    content='annotations',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS annotations_ai AFTER INSERT ON annotations BEGIN
    INSERT INTO annotations_fts(rowid, id, target_type, target_id, content, tags)
    VALUES (new.rowid, new.id, new.target_type, new.target_id, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_ad AFTER DELETE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, id, target_type, target_id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.target_type, old.target_id, old.content, old.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_au AFTER UPDATE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, id, target_type, target_id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.target_type, old.target_id, old.content, old.tags);
    INSERT INTO annotations_fts(rowid, id, target_type, target_id, content, tags)
    VALUES (new.rowid, new.id, new.target_type, new.target_id, new.content, new.tags);
END;
