-- KCA Track 9-typing: type edges as causal/correlational/temporal/structural.
-- Pre-release: in-place ALTER per CLAUDE.md.

ALTER TABLE entity_relationships ADD COLUMN edge_type TEXT NOT NULL DEFAULT 'correlational';

-- Lookup index for edge_type filtering (e.g., causal-only retrieval).
CREATE INDEX IF NOT EXISTS idx_entity_relationships_edge_type
    ON entity_relationships(edge_type);

-- Unique index for ON CONFLICT in upsert_relationship_typed.
CREATE UNIQUE INDEX IF NOT EXISTS uniq_entity_relationships_triple
    ON entity_relationships(source_entity_id, target_entity_id, relationship_type)
    WHERE valid_until IS NULL;

-- Constraint: enforce known values.
-- SQLite doesn't support adding a CHECK to an existing table without copy-and-rename;
-- we enforce in application code (EdgeType::parse) instead.
