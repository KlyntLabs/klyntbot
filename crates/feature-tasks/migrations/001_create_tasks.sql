-- Feature migration: task tables (feature-tasks crate)
-- Depends on: storage/001_initial (areas, projects, key_results, objectives),
--             storage/004_status_workflows (status_labels),
--             storage/005_task_groups (task_groups)
PRAGMA foreign_keys = ON;

-- ============================================================
-- Tasks
-- ============================================================
CREATE TABLE IF NOT EXISTS tasks (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    area_id              TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    project_id           TEXT REFERENCES projects(id) ON DELETE SET NULL,
    key_result_id        TEXT REFERENCES key_results(id) ON DELETE SET NULL,
    objective_id         TEXT REFERENCES objectives(id) ON DELETE SET NULL,
    parent_id            TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    status_label_id      TEXT REFERENCES status_labels(id) ON DELETE SET NULL,
    group_id             TEXT REFERENCES task_groups(id) ON DELETE SET NULL,
    priority             INTEGER,
    position             INTEGER NOT NULL DEFAULT 0,
    due_date             TEXT,
    tags                 TEXT NOT NULL DEFAULT '[]',
    status               TEXT NOT NULL DEFAULT 'todo',
    task_type            TEXT NOT NULL DEFAULT 'manual',
    focused_at           TEXT,
    focus_deadline       TEXT,
    focus_expired_count  INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at         TEXT,
    completed            INTEGER NOT NULL DEFAULT 0,
    total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
    estimated_minutes    INTEGER,
    actual_minutes       INTEGER,
    calendar_event_uid   TEXT,
    last_reminded_at     TEXT,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          INTEGER NOT NULL DEFAULT 0,
    next_instance_date   TEXT,
    acceptance_criteria  TEXT,
    agent_config         TEXT,
    execution_state      TEXT NOT NULL DEFAULT 'idle',
    spawned_execution_id TEXT,
    context_snapshot     TEXT,
    energy_level         TEXT DEFAULT 'medium',
    estimated_focus_blocks INTEGER,
    complexity_score     INTEGER,
    scheduled_start      TEXT,
    scheduled_end        TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_area_id ON tasks(area_id);
CREATE INDEX IF NOT EXISTS idx_tasks_project_id ON tasks(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_key_result_id ON tasks(key_result_id);
CREATE INDEX IF NOT EXISTS idx_tasks_objective_id ON tasks(objective_id);
CREATE INDEX IF NOT EXISTS idx_tasks_parent_id ON tasks(parent_id);
CREATE INDEX IF NOT EXISTS idx_tasks_due_date ON tasks(due_date);
CREATE INDEX IF NOT EXISTS idx_tasks_focused_at ON tasks(focused_at) WHERE focused_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tasks_is_template ON tasks(is_template) WHERE is_template = 1;
CREATE INDEX IF NOT EXISTS idx_tasks_task_type ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_execution_state ON tasks(execution_state);
CREATE INDEX IF NOT EXISTS idx_tasks_energy_level ON tasks(energy_level);
CREATE INDEX IF NOT EXISTS idx_tasks_status_label_id ON tasks(status_label_id);
CREATE INDEX IF NOT EXISTS idx_tasks_group_id ON tasks(group_id);

-- ============================================================
-- Task Activity (audit log)
-- ============================================================
CREATE TABLE IF NOT EXISTS task_activity (
    id            TEXT PRIMARY KEY,
    task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    activity_type TEXT NOT NULL,
    field_changed TEXT,
    old_value     TEXT,
    new_value     TEXT,
    actor_type    TEXT NOT NULL DEFAULT 'user',
    actor_id      TEXT,
    summary       TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_task_activity_task_id ON task_activity(task_id);
CREATE INDEX IF NOT EXISTS idx_task_activity_created_at ON task_activity(created_at);

-- ============================================================
-- Task Executions (agent execution records)
-- ============================================================
CREATE TABLE IF NOT EXISTS task_executions (
    id             TEXT PRIMARY KEY,
    task_id        TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    status         TEXT NOT NULL DEFAULT 'pending',
    agent_profile  TEXT,
    started_at     TEXT,
    completed_at   TEXT,
    duration_secs  INTEGER,
    tokens_used    INTEGER,
    cost_usd       REAL,
    input_context  TEXT,
    output_summary TEXT,
    error_message  TEXT,
    artifacts      TEXT,
    metrics        TEXT,
    retry_count    INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_task_executions_task_id ON task_executions(task_id);

-- ============================================================
-- Task Suggestions (AI-generated suggestions)
-- ============================================================
CREATE TABLE IF NOT EXISTS task_suggestions (
    id              TEXT PRIMARY KEY,
    task_id         TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    suggestion_type TEXT NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT,
    confidence      REAL NOT NULL DEFAULT 0.0,
    action_payload  TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    trigger         TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    resolved_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_suggestions_task_id ON task_suggestions(task_id);
CREATE INDEX IF NOT EXISTS idx_task_suggestions_pending ON task_suggestions(status) WHERE status = 'pending';

-- ============================================================
-- Task Attachments
-- ============================================================
CREATE TABLE IF NOT EXISTS task_attachments (
    id              TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    source          TEXT NOT NULL DEFAULT 'user',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_task_attachments_task_id ON task_attachments(task_id);

-- ============================================================
-- Task Time Entries
-- ============================================================
CREATE TABLE IF NOT EXISTS task_time_entries (
    id            TEXT PRIMARY KEY,
    task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    energy_level  TEXT,
    note          TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_time_entries_task_id ON task_time_entries(task_id);
CREATE INDEX IF NOT EXISTS idx_task_time_entries_started_at ON task_time_entries(started_at);

-- ============================================================
-- Task Dependencies
-- ============================================================
CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    blocker_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    dep_type   TEXT NOT NULL DEFAULT 'blocks',
    PRIMARY KEY (task_id, blocker_id),
    CHECK (task_id != blocker_id)
);

CREATE INDEX IF NOT EXISTS idx_task_dependencies_blocker_id ON task_dependencies(blocker_id);

-- ============================================================
-- Task Decompositions (pending decomposition plans)
-- ============================================================
CREATE TABLE IF NOT EXISTS task_decompositions (
    id          TEXT PRIMARY KEY,
    task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    plan        TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 0.0,
    status      TEXT NOT NULL DEFAULT 'pending',
    reasoning   TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    applied_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_decompositions_task_id ON task_decompositions(task_id);

-- ============================================================
-- Task Estimation History (estimation accuracy tracking)
-- ============================================================
CREATE TABLE IF NOT EXISTS task_estimation_history (
    id                TEXT PRIMARY KEY,
    task_id           TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    estimated_minutes INTEGER NOT NULL,
    actual_minutes    INTEGER NOT NULL,
    deviation_pct     REAL NOT NULL DEFAULT 0.0,
    complexity_score  INTEGER,
    energy_level      TEXT,
    tags              TEXT NOT NULL DEFAULT '[]',
    completed_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_estimation_history_task_id ON task_estimation_history(task_id);
CREATE INDEX IF NOT EXISTS idx_task_estimation_history_completed_at ON task_estimation_history(completed_at);

-- ============================================================
-- Triggers
-- ============================================================

-- 1. Auto-update updated_at on any row change
CREATE TRIGGER IF NOT EXISTS trg_tasks_updated_at
AFTER UPDATE ON tasks
FOR EACH ROW
BEGIN
    UPDATE tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE id = NEW.id;
END;

-- 2. Auto-set completed_at when completed changes to 1
CREATE TRIGGER IF NOT EXISTS trg_tasks_completed
AFTER UPDATE OF completed ON tasks
FOR EACH ROW
WHEN NEW.completed = 1 AND OLD.completed = 0
BEGIN
    UPDATE tasks SET completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE id = NEW.id;
END;

-- 3. Auto-calculate duration_secs when execution completed_at is set
CREATE TRIGGER IF NOT EXISTS trg_execution_duration
AFTER UPDATE OF completed_at ON task_executions
FOR EACH ROW
WHEN NEW.completed_at IS NOT NULL AND OLD.completed_at IS NULL AND NEW.started_at IS NOT NULL
BEGIN
    UPDATE task_executions
    SET duration_secs = CAST(
        (julianday(NEW.completed_at) - julianday(NEW.started_at)) * 86400.0
        AS INTEGER
    )
    WHERE id = NEW.id;
END;
