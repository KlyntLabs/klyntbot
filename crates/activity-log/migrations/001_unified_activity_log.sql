-- unified_activity_log: single source of truth for all activity events
CREATE TABLE IF NOT EXISTS unified_activity_log (
    id              TEXT PRIMARY KEY,         -- ULID (time-sortable)
    timestamp       TEXT NOT NULL,            -- ISO-8601 UTC
    source          TEXT NOT NULL,            -- os_window | browser | terminal | chat | tool_call | file_system | calendar | ide | note | domain_event | task | focus_session
    actor           TEXT NOT NULL DEFAULT 'user',  -- user | system | ai_agent
    resource_type   TEXT,                     -- file | url | repo | note | conversation | command | app | task | project
    resource_id     TEXT,                     -- Unique identifier
    resource_name   TEXT,                     -- Human-readable
    action          TEXT NOT NULL,            -- view | edit | run | prompt | reply | build | create | delete | switch | search | start | end | complete
    content_preview TEXT,                     -- First 500 chars
    content_hash    TEXT,                     -- SHA-256 for dedup
    metadata        TEXT,                     -- Source-specific JSON
    app_name        TEXT,                     -- For OS events
    project_id      TEXT,                     -- Auto-detected or linked
    work_context_id TEXT,                     -- FK to work_contexts (Phase 2, nullable initially)
    embedding_id    TEXT,                     -- FK to LanceDB (Phase 2, nullable initially)
    duration_secs   INTEGER,                 -- For duration-based events
    session_key     TEXT,                     -- For chat events
    is_sensitive    BOOLEAN NOT NULL DEFAULT FALSE
);

-- Indexes for primary query patterns
CREATE INDEX IF NOT EXISTS idx_ual_timestamp ON unified_activity_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_ual_source ON unified_activity_log(source, timestamp);
CREATE INDEX IF NOT EXISTS idx_ual_resource ON unified_activity_log(resource_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_ual_project ON unified_activity_log(project_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_ual_context ON unified_activity_log(work_context_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_ual_action ON unified_activity_log(action, timestamp);
CREATE INDEX IF NOT EXISTS idx_ual_hash ON unified_activity_log(content_hash);
CREATE INDEX IF NOT EXISTS idx_ual_session ON unified_activity_log(session_key, timestamp);
