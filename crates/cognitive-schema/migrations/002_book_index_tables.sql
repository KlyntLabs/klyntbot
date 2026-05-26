-- BookIndex tree nodes (hierarchical document structure)
CREATE TABLE IF NOT EXISTS book_tree_nodes (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES book_tree_nodes(id),
    node_type TEXT NOT NULL,
    content TEXT NOT NULL,
    title TEXT,
    level INTEGER NOT NULL DEFAULT 0,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tree_nodes_parent ON book_tree_nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_tree_nodes_source ON book_tree_nodes(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_tree_nodes_level ON book_tree_nodes(level);

-- FTS5 for keyword search within tree nodes
CREATE VIRTUAL TABLE IF NOT EXISTS book_tree_nodes_fts USING fts5(
    title, content,
    content='book_tree_nodes',
    content_rowid='rowid',
    tokenize='porter'
);

-- FTS5 sync triggers
CREATE TRIGGER IF NOT EXISTS book_tree_nodes_ai AFTER INSERT ON book_tree_nodes BEGIN
    INSERT INTO book_tree_nodes_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS book_tree_nodes_ad AFTER DELETE ON book_tree_nodes BEGIN
    INSERT INTO book_tree_nodes_fts(book_tree_nodes_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

CREATE TRIGGER IF NOT EXISTS book_tree_nodes_au AFTER UPDATE ON book_tree_nodes BEGIN
    INSERT INTO book_tree_nodes_fts(book_tree_nodes_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO book_tree_nodes_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS book_tree_nodes_update_ts AFTER UPDATE ON book_tree_nodes BEGIN
    UPDATE book_tree_nodes SET updated_at = datetime('now') WHERE id = new.id;
END;

-- GT-Link: entity-to-tree-node mapping
CREATE TABLE IF NOT EXISTS entity_tree_links (
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    tree_node_id TEXT NOT NULL REFERENCES book_tree_nodes(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (entity_id, tree_node_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_tree_links_node ON entity_tree_links(tree_node_id);
