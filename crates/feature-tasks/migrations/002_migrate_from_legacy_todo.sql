-- Migration 002: Copy data from legacy feature-todo tables to feature-tasks tables.
-- Idempotent: uses INSERT OR IGNORE so re-running is safe.

-- 1. Migrate actions → tasks
INSERT OR IGNORE INTO tasks (
    id, title, description, area_id, project_id, key_result_id, parent_id,
    priority, due_date, tags, status, focused_at, focus_deadline,
    focus_expired_count, created_at, updated_at, completed_at,
    total_tracked_secs, estimated_minutes, calendar_event_uid,
    last_reminded_at, recurrence_rule, recurrence_parent_id,
    is_template, next_instance_date,
    -- New columns with defaults
    status_label_id, position, group_id,
    task_type, acceptance_criteria, agent_config, execution_state,
    spawned_execution_id, context_snapshot, energy_level,
    estimated_focus_blocks, actual_minutes, complexity_score,
    completed, objective_id
)
SELECT
    id, title, description, area_id, project_id, key_result_id, parent_id,
    priority, due_date, tags, status, focused_at, focus_deadline,
    focus_expired_count, created_at, updated_at, completed_at,
    total_tracked_secs, estimated_minutes, calendar_event_uid,
    last_reminded_at, recurrence_rule, recurrence_parent_id,
    is_template, next_instance_date,
    -- Defaults for new columns
    NULL,           -- status_label_id
    0,              -- position
    NULL,           -- group_id
    'manual',       -- task_type
    NULL,           -- acceptance_criteria
    NULL,           -- agent_config
    'idle',         -- execution_state
    NULL,           -- spawned_execution_id
    NULL,           -- context_snapshot
    NULL,           -- energy_level
    NULL,           -- estimated_focus_blocks
    NULL,           -- actual_minutes
    NULL,           -- complexity_score
    CASE WHEN status = 'done' THEN 1 ELSE 0 END,  -- completed
    key_result_id   -- objective_id (same as key_result_id for legacy tasks)
FROM actions
WHERE EXISTS (SELECT 1 FROM actions LIMIT 1);

-- 2. Migrate action_attachments → task_attachments
INSERT OR IGNORE INTO task_attachments (id, task_id, attachment_type, value, title, tags, created_at, source)
SELECT id, action_id, attachment_type, value, title, tags, created_at, 'user'
FROM action_attachments
WHERE EXISTS (SELECT 1 FROM action_attachments LIMIT 1);

-- 3. Migrate action_time_entries → task_time_entries
INSERT OR IGNORE INTO task_time_entries (id, task_id, source, started_at, ended_at, duration_secs, note, energy_level)
SELECT id, action_id, source, started_at, ended_at, duration_secs, note, NULL
FROM action_time_entries
WHERE EXISTS (SELECT 1 FROM action_time_entries LIMIT 1);

-- 4. Migrate action_dependencies → task_dependencies
INSERT OR IGNORE INTO task_dependencies (task_id, blocker_id, dep_type)
SELECT task_id, blocker_id, 'blocks'
FROM action_dependencies
WHERE EXISTS (SELECT 1 FROM action_dependencies LIMIT 1);

-- 5. Generate initial activity log entries for migrated tasks
INSERT OR IGNORE INTO task_activity (id, task_id, activity_type, field_changed, old_value, new_value, actor_type, actor_id, summary, created_at)
SELECT
    lower(hex(randomblob(16))),
    id,
    'migrated',
    NULL,
    NULL,
    'Migrated from legacy todo system',
    'system',
    NULL,
    'Task migrated from feature-todo to feature-tasks',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM tasks
WHERE id IN (SELECT id FROM actions);
