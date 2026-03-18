-- Cognitive memory system: semantic facts, episodic memories, procedural rules, event logs, FTS5 search, annotations

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

CREATE UNIQUE INDEX IF NOT EXISTS idx_coaching_strategies_type_domain
    ON coaching_strategies(strategy_type, domain);

CREATE INDEX IF NOT EXISTS idx_semantic_facts_project ON semantic_facts(project_id);
CREATE INDEX IF NOT EXISTS idx_episodic_memories_project ON episodic_memories(project_id);
CREATE INDEX IF NOT EXISTS idx_procedural_rules_project ON procedural_rules(project_id);

-- ── Event logs ──

-- Domain event log: persists broadcast domain events for historical analysis
CREATE TABLE IF NOT EXISTS domain_event_log (
    id          TEXT PRIMARY KEY,
    event_type  TEXT NOT NULL,
    domain      TEXT NOT NULL,
    salience    TEXT NOT NULL,
    payload     TEXT NOT NULL DEFAULT '{}',
    timestamp   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_domain_event_log_timestamp
    ON domain_event_log (timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_domain_event_log_domain
    ON domain_event_log (domain);

-- Pipeline event log: persists extraction + consolidation events
CREATE TABLE IF NOT EXISTS pipeline_event_log (
    id              TEXT PRIMARY KEY,
    event_kind      TEXT NOT NULL,  -- 'extraction' or 'consolidation'
    observation     TEXT,           -- for extractions: the input text
    facts_extracted INTEGER,        -- for extractions: count of facts found
    operation       TEXT,           -- for consolidations: 'add', 'update', 'delete'
    fact_triple     TEXT,           -- for consolidations: "subject.predicate = object"
    timestamp       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_pipeline_event_log_timestamp
    ON pipeline_event_log (timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_pipeline_event_log_kind
    ON pipeline_event_log (event_kind);

-- Persist accumulated observations so they survive restarts.
-- The background consolidation service buffers low-salience events here
-- until they cross the promotion threshold (>=5 events across >=3 days).

CREATE TABLE IF NOT EXISTS accumulated_observations (
    id              TEXT PRIMARY KEY,
    event_type_key  TEXT NOT NULL,
    domain          TEXT NOT NULL,
    content         TEXT NOT NULL,
    importance      REAL NOT NULL,
    source_event    TEXT NOT NULL,
    observed_at     TEXT NOT NULL,
    day_key         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_accum_event_type ON accumulated_observations(event_type_key);

-- ── FTS5 full-text search ──

-- Full-text index for semantic facts
CREATE VIRTUAL TABLE IF NOT EXISTS semantic_facts_fts USING fts5(
    id UNINDEXED,
    domain,
    subject,
    predicate,
    object,
    memory_type,
    content='semantic_facts',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Keep FTS in sync with semantic_facts
CREATE TRIGGER IF NOT EXISTS semantic_facts_ai AFTER INSERT ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS semantic_facts_ad AFTER DELETE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS semantic_facts_au AFTER UPDATE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

-- Full-text index for episodic memories
CREATE VIRTUAL TABLE IF NOT EXISTS episodic_memories_fts USING fts5(
    id UNINDEXED,
    domain,
    content,
    summary,
    content='episodic_memories',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS episodic_memories_ai AFTER INSERT ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(rowid, id, domain, content, summary)
    VALUES (new.rowid, new.id, new.domain, new.content, new.summary);
END;

CREATE TRIGGER IF NOT EXISTS episodic_memories_ad AFTER DELETE ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(episodic_memories_fts, rowid, id, domain, content, summary)
    VALUES ('delete', old.rowid, old.id, old.domain, old.content, old.summary);
END;

CREATE TRIGGER IF NOT EXISTS episodic_memories_au AFTER UPDATE ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(episodic_memories_fts, rowid, id, domain, content, summary)
    VALUES ('delete', old.rowid, old.id, old.domain, old.content, old.summary);
    INSERT INTO episodic_memories_fts(rowid, id, domain, content, summary)
    VALUES (new.rowid, new.id, new.domain, new.content, new.summary);
END;

-- Full-text index for procedural rules
CREATE VIRTUAL TABLE IF NOT EXISTS procedural_rules_fts USING fts5(
    id UNINDEXED,
    domain,
    rule_text,
    content='procedural_rules',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS procedural_rules_ai AFTER INSERT ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(rowid, id, domain, rule_text)
    VALUES (new.rowid, new.id, new.domain, new.rule_text);
END;

CREATE TRIGGER IF NOT EXISTS procedural_rules_ad AFTER DELETE ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(procedural_rules_fts, rowid, id, domain, rule_text)
    VALUES ('delete', old.rowid, old.id, old.domain, old.rule_text);
END;

CREATE TRIGGER IF NOT EXISTS procedural_rules_au AFTER UPDATE ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(procedural_rules_fts, rowid, id, domain, rule_text)
    VALUES ('delete', old.rowid, old.id, old.domain, old.rule_text);
    INSERT INTO procedural_rules_fts(rowid, id, domain, rule_text)
    VALUES (new.rowid, new.id, new.domain, new.rule_text);
END;

-- ── Annotations ──

CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT DEFAULT '',
    author TEXT NOT NULL DEFAULT 'agent',
    priority INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    access_count INTEGER DEFAULT 0,
    UNIQUE(target_type, target_id, content)
);

CREATE INDEX IF NOT EXISTS idx_annotations_target ON annotations(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_annotations_tags ON annotations(tags);
CREATE INDEX IF NOT EXISTS idx_annotations_priority ON annotations(priority);

-- FTS5 for annotation search
CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts USING fts5(
    id UNINDEXED,
    target_type,
    target_id,
    content,
    tags,
    content='annotations',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS annotations_ai AFTER INSERT ON annotations BEGIN
    INSERT INTO annotations_fts(rowid, id, target_type, target_id, content, tags)
    VALUES (new.rowid, new.id, new.target_type, new.target_id, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_ad AFTER DELETE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, id, target_type, target_id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.target_type, old.target_id, old.content, old.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_au AFTER UPDATE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, id, target_type, target_id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.target_type, old.target_id, old.content, old.tags);
    INSERT INTO annotations_fts(rowid, id, target_type, target_id, content, tags)
    VALUES (new.rowid, new.id, new.target_type, new.target_id, new.content, new.tags);
END;

-- ── Dead-letter queue ──

-- Dead-letter queue for observations that failed LLM processing.
-- Observations are stored for later reprocessing when the LLM recovers.
CREATE TABLE IF NOT EXISTS failed_observations (
    id TEXT PRIMARY KEY,
    observation_json TEXT NOT NULL,
    failure_reason TEXT NOT NULL,
    failed_stage TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    next_retry_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_failed_observations_eligible
    ON failed_observations(retry_count, next_retry_at);

-- ── Unified Knowledge Graph ─────────────────────────────────────

CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    description TEXT,
    source TEXT NOT NULL DEFAULT 'extracted',
    source_id TEXT,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    mention_count INTEGER NOT NULL DEFAULT 1,
    metadata JSON,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);

CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    name, description,
    content='entities',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO entities_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
    INSERT INTO entities_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;

CREATE TABLE IF NOT EXISTS entity_relationships (
    id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL,
    strength REAL NOT NULL DEFAULT 0.5,
    evidence TEXT,
    valid_from TEXT,
    valid_until TEXT,
    source TEXT NOT NULL DEFAULT 'extracted',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_relationships_source ON entity_relationships(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_relationships_target ON entity_relationships(target_entity_id);
CREATE INDEX IF NOT EXISTS idx_relationships_type ON entity_relationships(relationship_type);

-- ── Persona Registry ────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS insight_personas (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    expertise TEXT NOT NULL,
    perspective TEXT NOT NULL,
    tone TEXT NOT NULL DEFAULT 'analytical',
    icon TEXT NOT NULL DEFAULT '🧠',
    source TEXT NOT NULL DEFAULT 'builtin',
    domains JSON NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    relevance_score REAL NOT NULL DEFAULT 0.5,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_personas_source ON insight_personas(source);
CREATE INDEX IF NOT EXISTS idx_personas_active ON insight_personas(is_active);

CREATE TABLE IF NOT EXISTS insight_persona_pins (
    note_id TEXT NOT NULL,
    persona_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (note_id, persona_id)
);

-- ── Flashcards (FSRS spaced repetition) ─────────────────────────

CREATE TABLE IF NOT EXISTS flashcards (
    id TEXT PRIMARY KEY,
    source_note_id TEXT,
    source_session_id TEXT,
    insight_review_id TEXT,
    deck TEXT NOT NULL DEFAULT 'general',
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'short_answer',
    choices JSON,
    stability REAL NOT NULL DEFAULT 1.0,
    difficulty REAL NOT NULL DEFAULT 0.5,
    due_at TEXT,
    last_reviewed_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_flashcards_source_note ON flashcards(source_note_id);
CREATE INDEX IF NOT EXISTS idx_flashcards_due ON flashcards(due_at);
CREATE INDEX IF NOT EXISTS idx_flashcards_deck ON flashcards(deck);
CREATE INDEX IF NOT EXISTS idx_flashcards_insight ON flashcards(insight_review_id);

-- ── Insight Reviews (versioned, replaces old insight_review_cache) ──────────

DROP TABLE IF EXISTS insight_review_cache;

CREATE TABLE IF NOT EXISTS insight_reviews (
    id                  TEXT PRIMARY KEY,
    note_id             TEXT NOT NULL,
    version             INTEGER NOT NULL DEFAULT 1,
    generated_at        TEXT NOT NULL,
    content             TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    scope_config        TEXT NOT NULL DEFAULT '{"scopeType":"backlinks","radius":0.72,"nodeIds":[],"includeCognitive":true,"deepDive":false,"mergeThreshold":0.6}',
    persona_ids         TEXT NOT NULL DEFAULT '[]',
    parent_insight_id   TEXT REFERENCES insight_reviews(id),
    token_cost_usd      REAL,
    superseded_at       TEXT,
    UNIQUE(note_id, version)
);

CREATE INDEX IF NOT EXISTS idx_insight_reviews_note ON insight_reviews(note_id, version);
CREATE INDEX IF NOT EXISTS idx_insight_reviews_hash ON insight_reviews(input_hash);
CREATE INDEX IF NOT EXISTS idx_insight_reviews_parent ON insight_reviews(parent_insight_id);

CREATE TABLE IF NOT EXISTS insight_progress_snapshots (
    id                  TEXT PRIMARY KEY,
    insight_review_id   TEXT NOT NULL REFERENCES insight_reviews(id) ON DELETE CASCADE,
    version             INTEGER NOT NULL,
    flashcard_success   REAL NOT NULL DEFAULT 0.0,
    semantic_drift      REAL NOT NULL DEFAULT 0.0,
    gap_closure         REAL NOT NULL DEFAULT 0.0,
    quiz_score          REAL NOT NULL DEFAULT 0.0,
    overall_progress    REAL NOT NULL DEFAULT 0.0,
    computed_at         TEXT NOT NULL,
    UNIQUE(insight_review_id, version)
);

CREATE INDEX IF NOT EXISTS idx_progress_insight ON insight_progress_snapshots(insight_review_id, version);
