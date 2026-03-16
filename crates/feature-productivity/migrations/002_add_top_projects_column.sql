-- Add top_projects column to daily_summaries (already in v1 for fresh installs).
ALTER TABLE daily_summaries ADD COLUMN top_projects TEXT;
