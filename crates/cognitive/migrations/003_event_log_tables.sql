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
