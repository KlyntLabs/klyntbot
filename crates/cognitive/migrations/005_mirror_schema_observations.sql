CREATE TABLE IF NOT EXISTS mirror_schema_observations (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL,
    field_id     TEXT NOT NULL,
    usage_type   TEXT NOT NULL,
    count        INTEGER NOT NULL DEFAULT 1,
    last_used_at TEXT NOT NULL,
    UNIQUE(database_id, field_id, usage_type)
);
CREATE INDEX IF NOT EXISTS idx_mirror_schema_obs_db ON mirror_schema_observations(database_id);
