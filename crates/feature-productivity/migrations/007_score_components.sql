ALTER TABLE productivity_quality_scores ADD COLUMN deep_work_ratio REAL NOT NULL DEFAULT 0.0;
ALTER TABLE productivity_quality_scores ADD COLUMN avg_session_length REAL NOT NULL DEFAULT 0.0;
ALTER TABLE productivity_quality_scores ADD COLUMN meeting_focus_ratio REAL NOT NULL DEFAULT 0.0;
