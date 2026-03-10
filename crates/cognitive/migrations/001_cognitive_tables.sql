-- Cognitive memory system tables

CREATE TABLE IF NOT EXISTS semantic_facts (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    subject         TEXT NOT NULL,
    predicate       TEXT NOT NULL,
    object          TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    source          TEXT NOT NULL DEFAULT 'observed',
    valid_from      TEXT NOT NULL,
    valid_until     TEXT,
    recorded_at     TEXT NOT NULL DEFAULT (datetime('now')),
    superseded_at   TEXT,
    superseded_by   TEXT,
    stability       REAL NOT NULL DEFAULT 1.0,
    last_accessed   TEXT,
    access_count    INTEGER NOT NULL DEFAULT 0,
    project_id      TEXT,  -- logical FK to projects.id (not enforced, separate database)
    memory_type     TEXT DEFAULT 'fact'
);

CREATE INDEX IF NOT EXISTS idx_semantic_facts_domain ON semantic_facts(domain);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_subject ON semantic_facts(subject, predicate);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_active ON semantic_facts(valid_until) WHERE valid_until IS NULL;

CREATE TABLE IF NOT EXISTS episodic_memories (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    content         TEXT NOT NULL,
    summary         TEXT,
    importance      REAL NOT NULL DEFAULT 0.5,
    occurred_at     TEXT NOT NULL,
    recorded_at     TEXT NOT NULL DEFAULT (datetime('now')),
    stability       REAL NOT NULL DEFAULT 1.0,
    last_accessed   TEXT,
    access_count    INTEGER NOT NULL DEFAULT 0,
    project_id      TEXT   -- logical FK to projects.id (not enforced, separate database)
);

CREATE INDEX IF NOT EXISTS idx_episodic_domain ON episodic_memories(domain);
CREATE INDEX IF NOT EXISTS idx_episodic_occurred ON episodic_memories(occurred_at);

CREATE TABLE IF NOT EXISTS procedural_rules (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    rule_text       TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    source          TEXT NOT NULL DEFAULT 'reflected',
    signal_count    INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    active          INTEGER NOT NULL DEFAULT 1,
    project_id      TEXT   -- logical FK to projects.id (not enforced, separate database)
);

CREATE INDEX IF NOT EXISTS idx_procedural_domain ON procedural_rules(domain);
CREATE INDEX IF NOT EXISTS idx_procedural_active ON procedural_rules(active) WHERE active = 1;

-- Archive tables (cold storage for superseded/decayed memories)
CREATE TABLE IF NOT EXISTS semantic_facts_archive (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    subject         TEXT NOT NULL,
    predicate       TEXT NOT NULL,
    object          TEXT NOT NULL,
    confidence      REAL NOT NULL,
    source          TEXT NOT NULL,
    valid_from      TEXT NOT NULL,
    valid_until     TEXT,
    recorded_at     TEXT NOT NULL,
    superseded_at   TEXT,
    superseded_by   TEXT,
    stability       REAL NOT NULL,
    last_accessed   TEXT,
    access_count    INTEGER NOT NULL,
    project_id      TEXT,
    memory_type     TEXT DEFAULT 'fact',
    archived_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS coaching_strategies (
    id              TEXT PRIMARY KEY,
    strategy_type   TEXT NOT NULL,
    domain          TEXT NOT NULL,
    times_used      INTEGER NOT NULL DEFAULT 0,
    times_accepted  INTEGER NOT NULL DEFAULT 0,
    times_led_to_improvement INTEGER NOT NULL DEFAULT 0,
    avg_improvement_magnitude REAL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    last_used       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_semantic_facts_project ON semantic_facts(project_id);
CREATE INDEX IF NOT EXISTS idx_episodic_memories_project ON episodic_memories(project_id);
CREATE INDEX IF NOT EXISTS idx_procedural_rules_project ON procedural_rules(project_id);
