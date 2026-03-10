-- Work contexts: inferred units of work
CREATE TABLE IF NOT EXISTS work_contexts (
    id                  TEXT PRIMARY KEY,
    title               TEXT NOT NULL,
    description         TEXT,
    status              TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'paused', 'completed', 'archived')),
    context_type        TEXT NOT NULL DEFAULT 'general'
                        CHECK (context_type IN ('coding', 'research', 'communication',
                        'planning', 'review', 'meeting', 'learning', 'general')),
    embedding_id        TEXT,
    linked_project_id   TEXT,
    color               TEXT,
    tags                TEXT DEFAULT '[]',
    confidence          REAL NOT NULL DEFAULT 0.5,
    first_seen_at       TEXT NOT NULL,
    last_active_at      TEXT NOT NULL,
    total_duration_secs INTEGER NOT NULL DEFAULT 0,
    event_count         INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_wc_status ON work_contexts(status);
CREATE INDEX IF NOT EXISTS idx_wc_last_active ON work_contexts(last_active_at);
CREATE INDEX IF NOT EXISTS idx_wc_project ON work_contexts(linked_project_id);

-- Resource registry
CREATE TABLE IF NOT EXISTS work_resources (
    id              TEXT PRIMARY KEY,
    resource_type   TEXT NOT NULL CHECK (resource_type IN
                    ('file', 'url', 'repo', 'note', 'conversation', 'app', 'command')),
    resource_name   TEXT NOT NULL,
    resource_path   TEXT,
    resource_uri    TEXT,
    first_seen_at   TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    access_count    INTEGER NOT NULL DEFAULT 0,
    embedding_id    TEXT
);

CREATE INDEX IF NOT EXISTS idx_wr_type ON work_resources(resource_type);
CREATE INDEX IF NOT EXISTS idx_wr_name ON work_resources(resource_name);

-- Resource co-occurrence graph edges
CREATE TABLE IF NOT EXISTS resource_edges (
    source_id       TEXT NOT NULL,
    target_id       TEXT NOT NULL,
    edge_type       TEXT NOT NULL CHECK (edge_type IN
                    ('co_access', 'references', 'derived_from', 'related')),
    weight          REAL NOT NULL DEFAULT 1.0,
    first_seen_at   TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    PRIMARY KEY (source_id, target_id, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_re_source ON resource_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_re_target ON resource_edges(target_id);

-- Context-to-resource membership
CREATE TABLE IF NOT EXISTS work_context_resources (
    context_id          TEXT NOT NULL,
    resource_id         TEXT NOT NULL,
    relevance_score     REAL NOT NULL DEFAULT 0.5,
    first_associated_at TEXT NOT NULL,
    last_associated_at  TEXT NOT NULL,
    PRIMARY KEY (context_id, resource_id)
);

CREATE INDEX IF NOT EXISTS idx_wcr_resource ON work_context_resources(resource_id);

-- Context-to-action links
CREATE TABLE IF NOT EXISTS work_context_actions (
    context_id  TEXT NOT NULL,
    action_id   TEXT NOT NULL,
    linked_at   TEXT NOT NULL,
    PRIMARY KEY (context_id, action_id)
);

CREATE INDEX IF NOT EXISTS idx_wca_action ON work_context_actions(action_id);
