-- Intent Pipeline: plan visibility, task linkage, enhanced strategy recording

ALTER TABLE plans ADD COLUMN visibility TEXT NOT NULL DEFAULT 'transparent';
ALTER TABLE plans ADD COLUMN task_id TEXT REFERENCES todos(id) ON DELETE SET NULL;

CREATE INDEX idx_plans_visibility ON plans(visibility);
CREATE INDEX idx_plans_task_id ON plans(task_id);

ALTER TABLE strategy_records ADD COLUMN complexity_signals TEXT NOT NULL DEFAULT '{}';
ALTER TABLE strategy_records ADD COLUMN execution_mode TEXT;
