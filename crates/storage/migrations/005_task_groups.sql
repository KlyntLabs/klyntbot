-- Task groups: collapsible sections within a project view
PRAGMA foreign_keys = ON;

CREATE TABLE task_groups (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    color       TEXT,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_task_groups_project_id ON task_groups(project_id);

-- Allow tasks to belong to a group
ALTER TABLE actions ADD COLUMN group_id TEXT REFERENCES task_groups(id) ON DELETE SET NULL;
CREATE INDEX idx_actions_group_id ON actions(group_id);
