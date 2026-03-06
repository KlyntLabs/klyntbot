-- Project registry
CREATE TABLE IF NOT EXISTS productivity_projects (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    url_patterns TEXT,
    color TEXT,
    is_auto_detected INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Add project_id to existing tables
ALTER TABLE activity_events ADD COLUMN project_id TEXT REFERENCES productivity_projects(id);
ALTER TABLE activity_buckets ADD COLUMN dominant_project TEXT;
ALTER TABLE productivity_goals ADD COLUMN project_id TEXT REFERENCES productivity_projects(id);
