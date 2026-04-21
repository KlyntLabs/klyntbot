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
    due_date             INTEGER,
    tags                 TEXT NOT NULL DEFAULT '[]',
    status               TEXT NOT NULL DEFAULT 'todo',
    task_type            TEXT NOT NULL DEFAULT 'manual',
    focused_at           INTEGER,
    focus_deadline       INTEGER,
    focus_expired_count  INTEGER NOT NULL DEFAULT 0,
    created_at           INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    updated_at           INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    completed_at         INTEGER,
    completed            INTEGER NOT NULL DEFAULT 0,
    total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
    estimated_minutes    INTEGER,
    actual_minutes       INTEGER,
    calendar_event_uid   TEXT,
    last_reminded_at     INTEGER,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          INTEGER NOT NULL DEFAULT 0,
    next_instance_date   INTEGER,
    acceptance_criteria  TEXT,
    agent_config         TEXT,
    execution_state      TEXT NOT NULL DEFAULT 'idle',
    spawned_execution_id TEXT,
    context_snapshot     TEXT,
    energy_level         TEXT DEFAULT 'medium',
    estimated_focus_blocks INTEGER,
    complexity_score     INTEGER,
    scheduled_start      INTEGER,
    scheduled_end        INTEGER,
    template_id          TEXT REFERENCES task_recurrence_templates(id) ON DELETE SET NULL
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
    created_at    INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_task_activity_task_id ON task_activity(task_id);
CREATE INDEX IF NOT EXISTS idx_task_activity_created_at ON task_activity(created_at);

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
    created_at      INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_task_attachments_task_id ON task_attachments(task_id);

-- ============================================================
-- Task Time Entries
-- ============================================================
CREATE TABLE IF NOT EXISTS task_time_entries (
    id            TEXT PRIMARY KEY,
    task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    INTEGER NOT NULL,
    ended_at      INTEGER,
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
    created_at  INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    applied_at  INTEGER
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
    project_id        TEXT,
    completed_at      INTEGER NOT NULL
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
    UPDATE tasks SET updated_at = (unixepoch('now') * 1000)
    WHERE id = NEW.id;
END;

-- 2. Auto-set completed_at when completed changes to 1
CREATE TRIGGER IF NOT EXISTS trg_tasks_completed
AFTER UPDATE OF completed ON tasks
FOR EACH ROW
WHEN NEW.completed = 1 AND OLD.completed = 0
BEGIN
    UPDATE tasks SET completed_at = (unixepoch('now') * 1000)
    WHERE id = NEW.id;
END;

-- ---------- Task recurrence templates (Phase 2) ----------
-- Note: source_task_id has NO FK to tasks — templates may outlive their
-- origin task (user could rename/split the source task without losing the
-- recurrence definition).
CREATE TABLE IF NOT EXISTS task_recurrence_templates (
    id                   TEXT PRIMARY KEY,
    source_task_id       TEXT NOT NULL,
    rrule                TEXT NOT NULL,
    iana_tz              TEXT NOT NULL,
    materialize_ahead    INTEGER NOT NULL DEFAULT 3,
    next_instance_at_ms  INTEGER,
    last_instance_at_ms  INTEGER,
    until_at_ms          INTEGER,
    count_remaining      INTEGER,
    enabled              INTEGER NOT NULL DEFAULT 1,
    created_at_ms        INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_recurrence_enabled
    ON task_recurrence_templates(enabled) WHERE enabled = 1;

-- Prevent duplicate instance rows if recover_in_flight replays a spawn cycle.
-- Applies only to recurrence instances (rows with a template_id set).
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_template_due_unique
    ON tasks(template_id, due_date)
    WHERE template_id IS NOT NULL;

-- ---------- Task alarms (Phase 2) ----------
CREATE TABLE IF NOT EXISTS task_alarms (
    id                  TEXT PRIMARY KEY,
    task_id             TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    rule_type           TEXT NOT NULL,                       -- 'relative_before'|'civil_time'|'absolute'
    offset_secs         INTEGER,
    day_offset          INTEGER,
    time_of_day         TEXT,                                -- 'HH:MM'
    iana_tz             TEXT,
    absolute_fire_at_ms INTEGER,
    channel_mask        INTEGER NOT NULL DEFAULT 0,
    priority_override   TEXT,
    misfire_policy      TEXT,
    grace_window_secs   INTEGER,
    created_at_ms       INTEGER NOT NULL,
    UNIQUE (task_id, rule_type, offset_secs, day_offset, time_of_day, absolute_fire_at_ms)
);
CREATE INDEX IF NOT EXISTS idx_task_alarms_task ON task_alarms(task_id);
