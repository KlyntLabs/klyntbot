-- Status workflows and labels for customizable kanban columns
PRAGMA foreign_keys = ON;

-- ============================================================
-- Status Workflows
-- ============================================================
CREATE TABLE status_workflows (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    is_template       INTEGER NOT NULL DEFAULT 0,
    is_global_default INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Status Labels (belong to workflows)
-- ============================================================
CREATE TABLE status_labels (
    id           TEXT PRIMARY KEY,
    workflow_id  TEXT NOT NULL REFERENCES status_workflows(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    color        TEXT NOT NULL DEFAULT '#6b7280',
    status_group TEXT NOT NULL CHECK(status_group IN ('not_started', 'active', 'done', 'stuck')),
    position     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_status_labels_workflow_id ON status_labels(workflow_id);

-- ============================================================
-- Alter projects: add workflow_id
-- ============================================================
ALTER TABLE projects ADD COLUMN workflow_id TEXT REFERENCES status_workflows(id) ON DELETE SET NULL;

-- ============================================================
-- Alter actions: add status_label_id and position
-- ============================================================
ALTER TABLE actions ADD COLUMN status_label_id TEXT REFERENCES status_labels(id) ON DELETE SET NULL;
ALTER TABLE actions ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_actions_status_label_id ON actions(status_label_id);
CREATE INDEX idx_actions_position ON actions(position);

-- ============================================================
-- Seed: global default workflow
-- ============================================================
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_default', 'Default', 0, 1);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_backlog',     'wf_default', 'Backlog',     '#6b7280', 'not_started', 0),
    ('sl_todo',        'wf_default', 'Todo',        '#3b82f6', 'not_started', 1),
    ('sl_in_progress', 'wf_default', 'In Progress', '#eab308', 'active',      2),
    ('sl_in_review',   'wf_default', 'In Review',   '#f97316', 'active',      3),
    ('sl_done',        'wf_default', 'Done',        '#22c55e', 'done',        4),
    ('sl_blocked',     'wf_default', 'Blocked',     '#ef4444', 'stuck',       5);

-- ============================================================
-- Seed: template workflows
-- ============================================================

-- Simple
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_simple', 'Simple', 1, 0);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_simple_todo',        'wf_simple', 'Todo',        '#3b82f6', 'not_started', 0),
    ('sl_simple_in_progress', 'wf_simple', 'In Progress', '#eab308', 'active',      1),
    ('sl_simple_done',        'wf_simple', 'Done',        '#22c55e', 'done',        2);

-- Software Dev
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_swdev', 'Software Dev', 1, 0);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_swdev_backlog',     'wf_swdev', 'Backlog',     '#6b7280', 'not_started', 0),
    ('sl_swdev_todo',        'wf_swdev', 'Todo',        '#3b82f6', 'not_started', 1),
    ('sl_swdev_in_progress', 'wf_swdev', 'In Progress', '#eab308', 'active',      2),
    ('sl_swdev_in_review',   'wf_swdev', 'In Review',   '#f97316', 'active',      3),
    ('sl_swdev_done',        'wf_swdev', 'Done',        '#22c55e', 'done',        4),
    ('sl_swdev_blocked',     'wf_swdev', 'Blocked',     '#ef4444', 'stuck',       5);

-- Content Creation
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_content', 'Content Creation', 1, 0);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_content_idea',      'wf_content', 'Idea',      '#a855f7', 'not_started', 0),
    ('sl_content_drafting',  'wf_content', 'Drafting',  '#eab308', 'active',      1),
    ('sl_content_editing',   'wf_content', 'Editing',   '#f97316', 'active',      2),
    ('sl_content_published', 'wf_content', 'Published', '#22c55e', 'done',        3);

-- ============================================================
-- Migrate existing actions: map status text to status_label_id
-- ============================================================
UPDATE actions SET status_label_id = 'sl_todo'        WHERE status = 'todo';
UPDATE actions SET status_label_id = 'sl_in_progress' WHERE status = 'doing';
UPDATE actions SET status_label_id = 'sl_done'        WHERE status = 'done';
UPDATE actions SET status_label_id = 'sl_done'        WHERE status = 'archived';
UPDATE actions SET status_label_id = 'sl_backlog'     WHERE status NOT IN ('todo', 'doing', 'done', 'archived');
