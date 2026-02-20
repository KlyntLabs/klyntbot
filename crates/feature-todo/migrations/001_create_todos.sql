-- Feature migration: todo tables (IF NOT EXISTS for idempotency)
CREATE TABLE IF NOT EXISTS todos (
    id                   VARCHAR(8) PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    priority             SMALLINT,
    due_date             TIMESTAMPTZ,
    tags                 TEXT[] NOT NULL DEFAULT '{}',
    status               TEXT NOT NULL DEFAULT 'todo',
    focused_at           TIMESTAMPTZ,
    focus_deadline       TIMESTAMPTZ,
    focus_expired_count  INT NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at         TIMESTAMPTZ,
    parent_id            VARCHAR(8) REFERENCES todos(id) ON DELETE SET NULL,
    project_id           VARCHAR(8) REFERENCES projects(id) ON DELETE SET NULL,
    total_tracked_secs   BIGINT NOT NULL DEFAULT 0,
    estimated_minutes    INT,
    calendar_event_uid   TEXT,
    last_reminded_at     TIMESTAMPTZ,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          BOOLEAN NOT NULL DEFAULT FALSE,
    next_instance_date   TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_todos_status ON todos(status);
CREATE INDEX IF NOT EXISTS idx_todos_project_id ON todos(project_id);
CREATE INDEX IF NOT EXISTS idx_todos_parent_id ON todos(parent_id);
CREATE INDEX IF NOT EXISTS idx_todos_due_date ON todos(due_date);
CREATE INDEX IF NOT EXISTS idx_todos_tags ON todos USING GIN (tags);
CREATE INDEX IF NOT EXISTS idx_todos_focused_at ON todos(focused_at) WHERE focused_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_todos_is_template ON todos(is_template) WHERE is_template = TRUE;

CREATE TABLE IF NOT EXISTS todo_attachments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    todo_id         VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT[] NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_todo_attachments_todo_id ON todo_attachments(todo_id);

CREATE TABLE IF NOT EXISTS todo_time_entries (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    todo_id       VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TIMESTAMPTZ NOT NULL,
    ended_at      TIMESTAMPTZ,
    duration_secs BIGINT,
    note          TEXT
);

CREATE INDEX IF NOT EXISTS idx_todo_time_entries_todo_id ON todo_time_entries(todo_id);

CREATE TABLE IF NOT EXISTS todo_dependencies (
    task_id    VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    blocker_id VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, blocker_id),
    CHECK (task_id != blocker_id)
);

CREATE INDEX IF NOT EXISTS idx_todo_dependencies_blocker_id ON todo_dependencies(blocker_id);
