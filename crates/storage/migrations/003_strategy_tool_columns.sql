-- Add tool outcome columns to strategy_records for learning consolidation.
-- These are nullable: multi-tool turns leave them NULL.
ALTER TABLE strategy_records ADD COLUMN tool_name TEXT;
ALTER TABLE strategy_records ADD COLUMN tool_success INTEGER;
ALTER TABLE strategy_records ADD COLUMN tool_duration_ms INTEGER;
