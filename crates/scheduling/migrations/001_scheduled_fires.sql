-- Canonical "when to fire" table. Every scheduled fire lives here.
CREATE TABLE scheduled_fires (
    id TEXT PRIMARY KEY,
    fire_at_ms INTEGER NOT NULL,
    kind TEXT NOT NULL,
    ref_id TEXT,
    payload TEXT NOT NULL DEFAULT '{}',
    dedup_prefix TEXT,
    fired INTEGER NOT NULL DEFAULT 0,
    firing_started_at_ms INTEGER,
    fired_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_scheduled_fires_pending
    ON scheduled_fires(fire_at_ms) WHERE fired = 0;

CREATE INDEX idx_scheduled_fires_dedup
    ON scheduled_fires(dedup_prefix) WHERE fired = 0;

CREATE INDEX idx_scheduled_fires_kind_ref
    ON scheduled_fires(kind, ref_id) WHERE fired = 0;
