-- Entity attention: decay-weighted attention seconds from activity_events.
-- Powers launcher personalization (Phase 4).

CREATE TABLE IF NOT EXISTS entity_attention (
    canonical_id   TEXT NOT NULL,
    kind           TEXT NOT NULL,            -- 'app' | 'site' | 'file' | 'note' | 'task'
    display_name   TEXT NOT NULL,
    -- Decay-weighted attention seconds (14-day half-life).
    attention_secs INTEGER NOT NULL DEFAULT 0,
    last_used_at   TEXT NOT NULL,            -- ISO 8601 UTC
    icon_hint      TEXT,
    category       TEXT,
    PRIMARY KEY (canonical_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_attention_kind_score
    ON entity_attention(kind, attention_secs DESC);
CREATE INDEX IF NOT EXISTS idx_attention_last_used
    ON entity_attention(last_used_at DESC);

-- FTS5 mirror keyed on canonical_id+kind.
CREATE VIRTUAL TABLE IF NOT EXISTS entity_attention_fts USING fts5(
    canonical_id UNINDEXED,
    kind UNINDEXED,
    display_name,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS entity_attention_ai AFTER INSERT ON entity_attention BEGIN
    INSERT INTO entity_attention_fts (canonical_id, kind, display_name)
    VALUES (NEW.canonical_id, NEW.kind, NEW.display_name);
END;

CREATE TRIGGER IF NOT EXISTS entity_attention_au AFTER UPDATE OF display_name ON entity_attention BEGIN
    DELETE FROM entity_attention_fts
        WHERE canonical_id = OLD.canonical_id AND kind = OLD.kind;
    INSERT INTO entity_attention_fts (canonical_id, kind, display_name)
        VALUES (NEW.canonical_id, NEW.kind, NEW.display_name);
END;

CREATE TRIGGER IF NOT EXISTS entity_attention_ad AFTER DELETE ON entity_attention BEGIN
    DELETE FROM entity_attention_fts
        WHERE canonical_id = OLD.canonical_id AND kind = OLD.kind;
END;
