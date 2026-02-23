-- Feature migration: todo tables (IF NOT EXISTS — core migration owns these)
CREATE TABLE IF NOT EXISTS todos (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    priority             INTEGER,
    due_date             TEXT,
    tags                 TEXT NOT NULL DEFAULT '[]',
    status               TEXT NOT NULL DEFAULT 'todo',
    focused_at           TEXT,
    focus_deadline       TEXT,
    focus_expired_count  INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at         TEXT,
    parent_id            TEXT REFERENCES todos(id) ON DELETE SET NULL,
    project_id           TEXT REFERENCES projects(id) ON DELETE SET NULL,
    total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
    estimated_minutes    INTEGER,
    calendar_event_uid   TEXT,
    last_reminded_at     TEXT,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          INTEGER NOT NULL DEFAULT 0,
    next_instance_date   TEXT
);

CREATE INDEX IF NOT EXISTS idx_todos_status ON todos(status);
CREATE INDEX IF NOT EXISTS idx_todos_project_id ON todos(project_id);
CREATE INDEX IF NOT EXISTS idx_todos_parent_id ON todos(parent_id);
CREATE INDEX IF NOT EXISTS idx_todos_due_date ON todos(due_date);
CREATE INDEX IF NOT EXISTS idx_todos_focused_at ON todos(focused_at) WHERE focused_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_todos_is_template ON todos(is_template) WHERE is_template = 1;

CREATE TABLE IF NOT EXISTS todo_attachments (
    id              TEXT PRIMARY KEY,
    todo_id         TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_todo_attachments_todo_id ON todo_attachments(todo_id);

CREATE TABLE IF NOT EXISTS todo_time_entries (
    id            TEXT PRIMARY KEY,
    todo_id       TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    note          TEXT
);

CREATE INDEX IF NOT EXISTS idx_todo_time_entries_todo_id ON todo_time_entries(todo_id);

CREATE TABLE IF NOT EXISTS todo_dependencies (
    task_id    TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    blocker_id TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, blocker_id),
    CHECK (task_id != blocker_id)
);

CREATE INDEX IF NOT EXISTS idx_todo_dependencies_blocker_id ON todo_dependencies(blocker_id);
