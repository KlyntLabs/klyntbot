-- 002_learning_loop.sql
-- Adds chat_id to strategy_records for reaction-based satisfaction.
-- Replaces JSON blob columns on goals with typed plan-completion columns.

-- Strategy records: add chat_id for linking reactions to records
ALTER TABLE strategy_records ADD COLUMN chat_id TEXT;
CREATE INDEX idx_strategy_records_chat_id ON strategy_records(chat_id);

-- Goals: add typed plan-completion columns
ALTER TABLE goals ADD COLUMN plans_completed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE goals ADD COLUMN plans_failed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE goals ADD COLUMN avg_duration_ms INTEGER;
ALTER TABLE goals ADD COLUMN last_plan_at TEXT;
