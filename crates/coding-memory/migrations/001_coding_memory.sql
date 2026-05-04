-- Phase-1 coding-memory schema consolidation.
--
-- Per CLAUDE.md pre-release policy: every column + table the 8-phase design
-- needs lands here. No incremental migrations between phases. Direct schema
-- changes authorized until first release.

-- === semantic_facts additions =============================================
-- scope_repo_id and metadata are now created by cognitive/001_cognitive_tables.sql

CREATE INDEX IF NOT EXISTS idx_semantic_facts_scope_repo
    ON semantic_facts(scope_repo_id);

-- === episodic_memories additions ==========================================
-- kind, scope_repo_id and metadata are now created by cognitive/001_cognitive_tables.sql
-- actor_id is added by cognitive/016_episodic_actor_id.sql (KCA Track 10/12).

CREATE INDEX IF NOT EXISTS idx_episodic_kind
    ON episodic_memories(kind);
CREATE INDEX IF NOT EXISTS idx_episodic_scope_repo
    ON episodic_memories(scope_repo_id);

-- === skill_versions additions =============================================

ALTER TABLE skill_versions ADD COLUMN scope TEXT DEFAULT 'global';
ALTER TABLE skill_versions ADD COLUMN scope_repo_id TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_skill_versions_scope_repo
    ON skill_versions(scope, scope_repo_id);

-- === ingest_event_log =====================================================

CREATE TABLE IF NOT EXISTS ingest_event_log (
    id             TEXT PRIMARY KEY,
    source         TEXT NOT NULL,
    session_id     TEXT NOT NULL,
    turn_id        TEXT,
    cwd            TEXT NOT NULL,
    repo_id        TEXT,
    occurred_at    TEXT NOT NULL,
    received_at    TEXT NOT NULL DEFAULT (datetime('now')),
    kind           TEXT NOT NULL,
    payload        TEXT NOT NULL,
    processed      BOOLEAN NOT NULL DEFAULT FALSE,
    processing     BOOLEAN NOT NULL DEFAULT FALSE,
    actor_id       TEXT NOT NULL DEFAULT 'local_user'
);

CREATE INDEX IF NOT EXISTS idx_ingest_event_log_session
    ON ingest_event_log(session_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_ingest_event_log_turn
    ON ingest_event_log(session_id, turn_id);
CREATE INDEX IF NOT EXISTS idx_ingest_event_log_unprocessed
    ON ingest_event_log(processed, received_at) WHERE processed = 0;
CREATE INDEX IF NOT EXISTS idx_ingest_event_log_repo
    ON ingest_event_log(repo_id, occurred_at);

-- === memory_causal_edges ==================================================

CREATE TABLE IF NOT EXISTS memory_causal_edges (
    id           TEXT PRIMARY KEY,
    from_id      TEXT NOT NULL,
    to_id        TEXT NOT NULL,
    edge_kind    TEXT NOT NULL,
    confidence   REAL NOT NULL DEFAULT 0.5,
    inferred_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_causal_from ON memory_causal_edges(from_id);
CREATE INDEX IF NOT EXISTS idx_causal_to ON memory_causal_edges(to_id);
CREATE INDEX IF NOT EXISTS idx_causal_kind
    ON memory_causal_edges(edge_kind);

-- === memory_utilization ===================================================

CREATE TABLE IF NOT EXISTS memory_utilization (
    id                TEXT PRIMARY KEY,
    memory_id         TEXT NOT NULL,
    retrieved_at      TEXT NOT NULL DEFAULT (datetime('now')),
    cited_in_response BOOLEAN NOT NULL DEFAULT FALSE,
    session_id        TEXT,
    turn_id           TEXT
);

CREATE INDEX IF NOT EXISTS idx_memory_util_memory
    ON memory_utilization(memory_id, retrieved_at);
CREATE INDEX IF NOT EXISTS idx_memory_util_session
    ON memory_utilization(session_id);

CREATE TABLE IF NOT EXISTS coding_reviews (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL,
    summary       TEXT NOT NULL,
    issues_json   TEXT NOT NULL,
    target        TEXT,
    delivery      TEXT,
    created_at    TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_coding_reviews_session
  ON coding_reviews(session_id, created_at DESC);

-- === klynt_sessions (owned by klynt-cli spec; consolidated here per §4) ====

CREATE TABLE IF NOT EXISTS klynt_sessions (
    id                 TEXT PRIMARY KEY,
    started_at         TEXT NOT NULL,
    ended_at           TEXT,
    cwd                TEXT NOT NULL,
    repo_id            TEXT,
    initial_prompt     TEXT,
    total_turns        INTEGER NOT NULL DEFAULT 0,
    total_cost_usd     REAL NOT NULL DEFAULT 0.0,
    total_tokens_in    INTEGER NOT NULL DEFAULT 0,
    total_tokens_out   INTEGER NOT NULL DEFAULT 0,
    actor_id           TEXT NOT NULL DEFAULT 'local_user'
);

CREATE INDEX IF NOT EXISTS idx_klynt_sessions_repo
    ON klynt_sessions(repo_id, started_at);
