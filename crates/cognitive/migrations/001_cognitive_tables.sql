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
    convergence_score REAL NOT NULL DEFAULT 0.0,
    project_id      TEXT,  -- logical FK to projects.id (not enforced, separate database)
    memory_type     TEXT DEFAULT 'fact',
    scope_type      TEXT NOT NULL DEFAULT 'system',
    scope_id        TEXT,
    scope_repo_id   TEXT,
    metadata        TEXT
);

CREATE INDEX IF NOT EXISTS idx_semantic_facts_domain ON semantic_facts(domain);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_subject ON semantic_facts(subject, predicate);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_active ON semantic_facts(valid_until) WHERE valid_until IS NULL;
CREATE INDEX IF NOT EXISTS idx_semantic_facts_scope ON semantic_facts(scope_type, scope_id);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_recorded_at ON semantic_facts(recorded_at);

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
    project_id      TEXT,   -- logical FK to projects.id (not enforced, separate database)
    scope_type      TEXT NOT NULL DEFAULT 'system',
    scope_id        TEXT,
    scope_repo_id   TEXT,
    metadata        TEXT,
    kind            TEXT
);

CREATE INDEX IF NOT EXISTS idx_episodic_domain ON episodic_memories(domain);
CREATE INDEX IF NOT EXISTS idx_episodic_occurred ON episodic_memories(occurred_at);
CREATE INDEX IF NOT EXISTS idx_episodic_memories_scope ON episodic_memories(scope_type, scope_id);

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
    project_id      TEXT,   -- logical FK to projects.id (not enforced, separate database)
    scope_type      TEXT NOT NULL DEFAULT 'system',
    scope_id        TEXT
);

CREATE INDEX IF NOT EXISTS idx_procedural_domain ON procedural_rules(domain);
CREATE INDEX IF NOT EXISTS idx_procedural_active ON procedural_rules(active) WHERE active = 1;
CREATE INDEX IF NOT EXISTS idx_procedural_rules_scope ON procedural_rules(scope_type, scope_id);

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
    scope_type      TEXT NOT NULL DEFAULT 'system',
    scope_id        TEXT,
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
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    behavioral_positive INTEGER DEFAULT 0,
    behavioral_negative INTEGER DEFAULT 0
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
    day_key         TEXT NOT NULL,
    near_miss_count INTEGER DEFAULT 0
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
    mark_id TEXT,
    quoted_text TEXT,
    range_start INTEGER,
    range_end INTEGER,
    ai_suggestion TEXT
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

-- Aliases for an entity. Closes the gap where the question references
-- an entity by a non-canonical form ("Mel" for "Melissa", "Caroline's"
-- for "Caroline"). Populated at extraction time; expanded at query
-- time inside `extract_query_entities`. alias_type is purely advisory
-- — retrieval treats all aliases as equivalent matches.
CREATE TABLE IF NOT EXISTS entity_aliases (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    alias_type TEXT NOT NULL,        -- 'short_form' | 'possessive' | 'nickname' | 'full_form' | 'spelling'
    source TEXT NOT NULL,            -- 'derived' | 'extraction' | 'manual'
    created_at TEXT NOT NULL,
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE,
    UNIQUE(entity_id, alias)
);

CREATE INDEX IF NOT EXISTS idx_entity_aliases_alias ON entity_aliases(alias);
CREATE INDEX IF NOT EXISTS idx_entity_aliases_entity ON entity_aliases(entity_id);

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


-- ── Knowledge Topics ────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS knowledge_topics (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    domain          TEXT NOT NULL,
    atom_count      INTEGER NOT NULL DEFAULT 0,
    avg_retention   REAL NOT NULL DEFAULT 1.0,
    created_at      TEXT NOT NULL
);

-- ── Knowledge Atoms ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS knowledge_atoms (
    id                  TEXT PRIMARY KEY NOT NULL,
    subject             TEXT NOT NULL,
    atom_type           TEXT NOT NULL CHECK (atom_type IN ('vocabulary', 'concept', 'skill', 'fact', 'flashcard_weak_spot', 'socratic_exchange', 'translation_unit', 'procedure', 'reference', 'pattern', 'insight', 'relation')),
    domain              TEXT NOT NULL,
    source_note_id      TEXT,
    source_range        TEXT,
    source_context      TEXT,
    secondary_sources   TEXT,
    semantic_fact_id    TEXT,
    retention_pct       REAL NOT NULL DEFAULT 1.0,
    stability           REAL NOT NULL DEFAULT 1.0,
    difficulty          REAL NOT NULL DEFAULT 5.0,
    personal_importance REAL NOT NULL DEFAULT 0.7,
    status              TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested', 'active', 'archived')),
    salience            REAL NOT NULL DEFAULT 1.0,
    last_interaction_ts TEXT,
    archived_at         TEXT,
    metadata            TEXT,
    topic_id            TEXT REFERENCES knowledge_topics(id),
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_atoms_note ON knowledge_atoms(source_note_id) WHERE status != 'archived';
CREATE INDEX IF NOT EXISTS idx_atoms_last_interaction ON knowledge_atoms(last_interaction_ts) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_atoms_topic ON knowledge_atoms(topic_id);
CREATE INDEX IF NOT EXISTS idx_atoms_status ON knowledge_atoms(status, salience);
CREATE INDEX IF NOT EXISTS idx_atoms_subject ON knowledge_atoms(subject, domain) WHERE status != 'archived';

-- ── Flashcards (FSRS-5 spaced repetition) ─────────────────────────

CREATE TABLE IF NOT EXISTS flashcards (
    id TEXT PRIMARY KEY,
    source_note_id TEXT,
    source_context TEXT,
    atom_id         TEXT REFERENCES knowledge_atoms(id),
    deck TEXT NOT NULL DEFAULT 'general',
    front TEXT NOT NULL,
    back TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'basic',
    cloze_data TEXT,
    vocab_data TEXT,
    image_data TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    stability REAL NOT NULL DEFAULT 1.0,
    difficulty REAL NOT NULL DEFAULT 5.0,
    due_at TEXT,
    last_reviewed_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',
    suspended INTEGER NOT NULL DEFAULT 0,
    recall_speed_ms INTEGER,
    back_embedding_updated_at TEXT,
    preferred_mode TEXT,
    difficulty_estimate INTEGER,
    prerequisite_concepts TEXT,
    card_distractors TEXT,
    audio_ref TEXT,
    pronunciation_baseline REAL,
    last_pronunciation_score REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_flashcards_source_note ON flashcards(source_note_id);
CREATE INDEX IF NOT EXISTS idx_flashcards_due ON flashcards(due_at);
CREATE INDEX IF NOT EXISTS idx_flashcards_deck ON flashcards(deck);
CREATE INDEX IF NOT EXISTS idx_flashcards_state ON flashcards(state);
CREATE INDEX IF NOT EXISTS idx_flashcards_deck_due ON flashcards(deck, due_at);
CREATE INDEX IF NOT EXISTS idx_flashcards_state_due ON flashcards(state, due_at);

-- ── FSRS-5 personal parameters ───────────────────────────────────

CREATE TABLE IF NOT EXISTS fsrs_parameters (
    id TEXT PRIMARY KEY DEFAULT 'local',
    weights TEXT NOT NULL,
    desired_retention REAL NOT NULL DEFAULT 0.9,
    trained_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO fsrs_parameters (id, weights)
VALUES ('local', '[0.40255,1.18385,3.173,15.69105,7.1949,0.5345,1.4604,0.0046,1.54575,0.1192,1.01925,1.9395,0.11,0.29605,2.2698,0.2315,2.9898,0.51655,0.6621]');

-- ── Review log (feeds FSRS-5 weight training) ────────────────────

CREATE TABLE IF NOT EXISTS review_log (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL REFERENCES flashcards(id) ON DELETE CASCADE,
    rating INTEGER NOT NULL,
    elapsed_days REAL NOT NULL,
    scheduled_days REAL NOT NULL,
    recall_speed_ms INTEGER,
    state TEXT NOT NULL,
    reviewed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_review_log_card ON review_log(card_id);
CREATE INDEX IF NOT EXISTS idx_review_log_reviewed ON review_log(reviewed_at);

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
    debate_transcript   JSON,
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


-- Coaching intervention history (persistent log for dashboard + retroactive feedback)
CREATE TABLE IF NOT EXISTS coaching_intervention_log (
    id TEXT PRIMARY KEY,
    intervention_type TEXT NOT NULL,
    message TEXT NOT NULL,
    trigger_name TEXT NOT NULL,
    feedback TEXT,
    delivered_at TEXT NOT NULL,
    feedback_at TEXT,
    action_url TEXT
);

CREATE INDEX IF NOT EXISTS idx_coaching_intervention_log_delivered
    ON coaching_intervention_log(delivered_at DESC);

-- ── Atom Extraction Cache (content-hash dedup) ────────────────────
CREATE TABLE IF NOT EXISTS atom_extraction_cache (
    note_id       TEXT PRIMARY KEY NOT NULL,
    content_hash  TEXT NOT NULL,
    extracted_at  TEXT NOT NULL
);

-- ── Deck Preferences ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS deck_preferences (
    deck TEXT PRIMARY KEY,
    answer_mode TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- ── Review Sessions ───────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS review_sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    cards_reviewed INTEGER DEFAULT 0,
    avg_score REAL,
    duration_seconds INTEGER,
    modes_used TEXT,
    propagation_count INTEGER DEFAULT 0,
    weak_card_ids TEXT,
    session_data TEXT,
    status TEXT DEFAULT 'active'
);

CREATE INDEX IF NOT EXISTS idx_review_sessions_status ON review_sessions(status);
CREATE INDEX IF NOT EXISTS idx_review_sessions_started ON review_sessions(started_at);

-- ── Co-activation tracking ───────────────────────────────────────
-- Facts retrieved together strengthen each other (Hebbian co-retrieval)
CREATE TABLE IF NOT EXISTS co_activation (
    fact_id_a TEXT NOT NULL,
    fact_id_b TEXT NOT NULL,
    strength  REAL NOT NULL DEFAULT 1.0,
    last_fired TEXT NOT NULL,
    PRIMARY KEY (fact_id_a, fact_id_b)
);
CREATE INDEX IF NOT EXISTS idx_co_activation_a ON co_activation(fact_id_a);
CREATE INDEX IF NOT EXISTS idx_co_activation_b ON co_activation(fact_id_b);

-- Temporal fact changelog: append-only log of every fact mutation.
-- Audit trail for Reforge analysis and debugging. Pruned by cron.
CREATE TABLE IF NOT EXISTS fact_changelog (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_id TEXT NOT NULL,
    change_type TEXT NOT NULL,
    field_changed TEXT,
    old_value TEXT,
    new_value TEXT,
    source TEXT,
    changed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_fact_changelog_fact_id
    ON fact_changelog(fact_id, changed_at);

CREATE INDEX IF NOT EXISTS idx_fact_changelog_type
    ON fact_changelog(change_type, changed_at);

-- Conversation turn value-density scores.
-- Populated in real-time by background pipeline; queried by Reforge Phase 6.5.
CREATE TABLE IF NOT EXISTS conversation_density (
    id TEXT PRIMARY KEY,
    session_key TEXT NOT NULL,
    content_preview TEXT NOT NULL,
    density_score REAL NOT NULL,
    tier TEXT NOT NULL,          -- 'high', 'medium', 'low'
    entity_signal REAL NOT NULL,
    action_signal REAL NOT NULL,
    decision_signal REAL NOT NULL,
    novelty_signal REAL NOT NULL,
    enriched INTEGER NOT NULL DEFAULT 0,  -- 1 after graph enrichment processed this turn
    promoted_at TEXT,             -- ISO timestamp when promoted to knowledge graph (nullable)
    computed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_conversation_density_tier
    ON conversation_density(tier, enriched, computed_at);

-- Nightly knowledge graph snapshots for trend analysis.
CREATE TABLE IF NOT EXISTS knowledge_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_count INTEGER NOT NULL,
    entity_count INTEGER NOT NULL,
    relationship_count INTEGER NOT NULL,
    domain_summary TEXT,       -- JSON: {"work": 42, "finance": 15, ...}
    top_entities TEXT,         -- JSON: [{"name": "Rust", "mentions": 30}, ...]
    graph_metrics TEXT,        -- JSON: {"orphan_rate": 0.12, "avg_degree": 2.3, ...}
    snapshot_at TEXT NOT NULL DEFAULT (datetime('now'))
);
