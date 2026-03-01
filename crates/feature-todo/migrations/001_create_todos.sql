-- Feature migration: action tables (IF NOT EXISTS — core migration owns these)
CREATE TABLE IF NOT EXISTS actions (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    area_id              TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    project_id           TEXT REFERENCES projects(id) ON DELETE SET NULL,
    key_result_id        TEXT REFERENCES key_results(id) ON DELETE SET NULL,
    parent_id            TEXT REFERENCES actions(id) ON DELETE SET NULL,
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
    total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
    estimated_minutes    INTEGER,
    calendar_event_uid   TEXT,
    last_reminded_at     TEXT,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          INTEGER NOT NULL DEFAULT 0,
    next_instance_date   TEXT
);

CREATE INDEX IF NOT EXISTS idx_actions_status ON actions(status);
CREATE INDEX IF NOT EXISTS idx_actions_area_id ON actions(area_id);
CREATE INDEX IF NOT EXISTS idx_actions_project_id ON actions(project_id);
CREATE INDEX IF NOT EXISTS idx_actions_key_result_id ON actions(key_result_id);
CREATE INDEX IF NOT EXISTS idx_actions_parent_id ON actions(parent_id);
CREATE INDEX IF NOT EXISTS idx_actions_due_date ON actions(due_date);
CREATE INDEX IF NOT EXISTS idx_actions_focused_at ON actions(focused_at) WHERE focused_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_actions_is_template ON actions(is_template) WHERE is_template = 1;

CREATE TABLE IF NOT EXISTS action_attachments (
    id              TEXT PRIMARY KEY,
    action_id       TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_action_attachments_action_id ON action_attachments(action_id);

CREATE TABLE IF NOT EXISTS action_time_entries (
    id            TEXT PRIMARY KEY,
    action_id     TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    note          TEXT
);

CREATE INDEX IF NOT EXISTS idx_action_time_entries_action_id ON action_time_entries(action_id);

CREATE TABLE IF NOT EXISTS action_dependencies (
    task_id    TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    blocker_id TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, blocker_id),
    CHECK (task_id != blocker_id)
);

CREATE INDEX IF NOT EXISTS idx_action_dependencies_blocker_id ON action_dependencies(blocker_id);
