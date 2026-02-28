-- Agent coordination task board for subagent work tracking

CREATE TABLE agent_tasks (
    id              TEXT PRIMARY KEY,
    session_key     TEXT NOT NULL,
    description     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending','claimed','running','completed','failed')),
    owner_agent_id  TEXT,
    parent_task_id  TEXT REFERENCES agent_tasks(id) ON DELETE CASCADE,
    result          TEXT,
    error           TEXT,
    blocked_by      TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_agent_tasks_session ON agent_tasks(session_key);
CREATE INDEX idx_agent_tasks_status ON agent_tasks(status);
CREATE INDEX idx_agent_tasks_owner ON agent_tasks(owner_agent_id);

-- Tool usage analytics (P3 but adding migration now to avoid future migration)
CREATE TABLE tool_usage (
    id              TEXT PRIMARY KEY,
    tool_name       TEXT NOT NULL,
    action          TEXT,
    session_key     TEXT,
    channel         TEXT,
    intent_category TEXT,
    success         INTEGER NOT NULL DEFAULT 1,
    duration_ms     INTEGER,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_tool_usage_tool ON tool_usage(tool_name);
CREATE INDEX idx_tool_usage_created ON tool_usage(created_at);
