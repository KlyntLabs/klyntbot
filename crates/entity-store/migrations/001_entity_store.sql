-- Registry of all user databases
CREATE TABLE IF NOT EXISTS databases (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    slug         TEXT UNIQUE NOT NULL,
    icon         TEXT,
    description  TEXT,
    template_id  TEXT,
    skill_id     TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- Field definitions — the schema of each database
CREATE TABLE IF NOT EXISTS database_fields (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    slug         TEXT NOT NULL,
    field_type   TEXT NOT NULL,
    options_json TEXT,
    position     INTEGER NOT NULL DEFAULT 0,
    required     INTEGER NOT NULL DEFAULT 0,
    hidden       INTEGER NOT NULL DEFAULT 0,
    ai_managed   INTEGER NOT NULL DEFAULT 0,
    ai_config    TEXT,
    default_value TEXT,
    created_at   TEXT NOT NULL,
    UNIQUE(database_id, slug)
);
CREATE INDEX IF NOT EXISTS idx_database_fields_db ON database_fields(database_id);

-- Views per database
CREATE TABLE IF NOT EXISTS database_views (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    view_type    TEXT NOT NULL,
    config_json  TEXT NOT NULL DEFAULT '{}',
    position     INTEGER NOT NULL DEFAULT 0,
    is_default   INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_database_views_db ON database_views(database_id);

-- Custom dashboards
CREATE TABLE IF NOT EXISTS dashboards (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Cross-database entity relations
CREATE TABLE IF NOT EXISTS entity_relations (
    id            TEXT PRIMARY KEY,
    source_id     TEXT NOT NULL,
    source_db_id  TEXT NOT NULL,
    target_id     TEXT NOT NULL,
    target_db_id  TEXT NOT NULL,
    relation_type TEXT NOT NULL DEFAULT 'related',
    inferred      INTEGER NOT NULL DEFAULT 0,
    confidence    REAL,
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entity_relations_source ON entity_relations(source_id, source_db_id);
CREATE INDEX IF NOT EXISTS idx_entity_relations_target ON entity_relations(target_id, target_db_id);

-- Schema evolution tracking
CREATE TABLE IF NOT EXISTS schema_evolutions (
    id            TEXT PRIMARY KEY,
    database_id   TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    action_type   TEXT NOT NULL,
    action_json   TEXT NOT NULL,
    confidence    REAL NOT NULL,
    reasoning     TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'proposed',
    source        TEXT NOT NULL DEFAULT 'reforge',
    created_at    TEXT NOT NULL,
    resolved_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_schema_evolutions_db ON schema_evolutions(database_id, status);

-- Per-database AI autonomy calibration
CREATE TABLE IF NOT EXISTS schema_autonomy (
    database_id      TEXT PRIMARY KEY REFERENCES databases(id) ON DELETE CASCADE,
    auto_threshold   REAL NOT NULL DEFAULT 0.9,
    suggest_threshold REAL NOT NULL DEFAULT 0.6,
    acceptance_rate  REAL NOT NULL DEFAULT 0.5,
    total_proposed   INTEGER NOT NULL DEFAULT 0,
    total_accepted   INTEGER NOT NULL DEFAULT 0,
    updated_at       TEXT NOT NULL
);
