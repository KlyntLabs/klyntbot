-- Usage log for frecency calculation (exponential decay)
CREATE TABLE IF NOT EXISTS launcher_usage_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    used_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_usage_log_item ON launcher_usage_log(item_id, kind);
CREATE INDEX IF NOT EXISTS idx_usage_log_time ON launcher_usage_log(used_at);

-- Pinned launcher items for default view
CREATE TABLE IF NOT EXISTS launcher_pins (
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (item_id, kind)
);
CREATE INDEX IF NOT EXISTS idx_launcher_pins_position ON launcher_pins(position);

-- Clipboard history
CREATE TABLE IF NOT EXISTS clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'text',
    source_app TEXT,
    preview TEXT,
    file_path TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

-- FTS5 index for clipboard search
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts USING fts5(
    content, preview, content='clipboard_history', content_rowid='id'
);

-- FTS5 sync triggers
CREATE TRIGGER IF NOT EXISTS clipboard_fts_insert AFTER INSERT ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(rowid, content, preview)
    VALUES (new.id, new.content, new.preview);
END;

CREATE TRIGGER IF NOT EXISTS clipboard_fts_delete AFTER DELETE ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
    VALUES ('delete', old.id, old.content, old.preview);
END;

CREATE TRIGGER IF NOT EXISTS clipboard_fts_update AFTER UPDATE ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
    VALUES ('delete', old.id, old.content, old.preview);
    INSERT INTO clipboard_fts(rowid, content, preview)
    VALUES (new.id, new.content, new.preview);
END;
