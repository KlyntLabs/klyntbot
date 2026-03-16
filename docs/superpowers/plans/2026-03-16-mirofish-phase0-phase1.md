# MiroFish Phase 0 + Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the shared infrastructure (Unified Knowledge Graph, Persona Registry, multi-perspective prompt) and InsightForge context assembly — the two foundation layers that every future phase depends on.

**Architecture:** Phase 0 adds 4 new tables to the `cognitive` crate (entities, entity_relationships, insight_personas, insight_persona_pins), plus EntityRepo and PersonaRepo. Phase 1 adds the InsightForge module to `context_engine` with heuristic + LLM decomposition, pluggable DomainSearchers, per-session circuit breaker, and RRF merge. The agent runtime gets a one-line planning prompt tweak for multi-perspective reasoning.

**Tech Stack:** Rust (SQLite via sqlx, LanceDB vectors, tokio async), Tauri 2 IPC, `dashmap` for circuit breaker, custom RRF merge (reuses the algorithm from `tools_core::rrf_merge` but adapted for `MemoryEntry` since it doesn't implement `Searchable`).

**Spec:** `docs/superpowers/specs/2026-03-16-mirofish-integration-architecture.md` (v2)

**Scope note:** This plan covers Phase 0 (Shared Infrastructure) and Phase 1 (InsightForge). Phases 2-6 get separate plans. Phase 2 (Notes Insight Review) already has its own plan at `docs/superpowers/plans/2026-03-16-insight-review.md`.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/repos/entity.rs` | EntityRepo: CRUD for entities + relationships, graph neighborhood queries, fuzzy name search, merge |
| `crates/cognitive/src/repos/persona.rs` | PersonaRepo: CRUD for personas, selection algorithm, pin management, builtin seeding |
| `crates/context_engine/src/insight_forge/mod.rs` | InsightForge: orchestrates decomposition → parallel retrieval → RRF merge |
| `crates/context_engine/src/insight_forge/decomposer.rs` | QueryDecomposer trait + HeuristicDecomposer + LlmDecomposer |
| `crates/context_engine/src/insight_forge/domain_searcher.rs` | DomainSearcher trait definition |
| `crates/context_engine/src/insight_forge/circuit_breaker.rs` | Per-session circuit breaker using DashMap |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Append entities, entity_relationships, insight_personas, insight_persona_pins DDL after line 309 |
| `crates/cognitive/src/repos/mod.rs` | Register entity + persona modules, bump migration version |
| `crates/context_engine/src/lib.rs` | Export InsightForge, DomainSearcher, QueryDecomposer |
| `crates/context_engine/src/assembler/mod.rs` | Add InsightForge field to ContextEngine, builder methods, integration in retrieve_memory() |
| `crates/context_engine/src/memory_retriever.rs` | Add `Domain { name: String }` variant to MemorySource |
| `crates/bus/src/domain_events.rs` | Add ContradictionDetected variant (Phase 3 prep) |
| `crates/agent/src/agent_runtime/runtime.rs` | Multi-perspective planning prompt tweak in build_planning_prompt() |
| `crates/app-core/src/init/mod.rs` | Wire InsightForge into ContextEngine during init |

---

## Chunk 1: Schema + EntityRepo

### Task 1: Entity + Relationship Migration Schema

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql` (append after line 309)
- Modify: `crates/cognitive/src/repos/mod.rs` (bump migration version)

- [ ] **Step 1: Append entity tables DDL to migration**

Append to end of `crates/cognitive/migrations/001_cognitive_tables.sql`:

```sql
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
    name, description, tokenize='porter unicode61'
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
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, find the `cognitive_migrations()` function (around line 40). The current version is `1`. Change `version: 1` to `version: 2`. Since we use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` throughout, the full SQL is idempotent — bumping the version ensures existing dev databases re-run and pick up the new tables.

- [ ] **Step 3: Verify migration compiles**

Run: `cargo build -p cognitive`
Expected: compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add entities, relationships, personas schema"
```

---

### Task 2: EntityRepo — Tests First

**Files:**
- Create: `crates/cognitive/src/repos/entity.rs`

- [ ] **Step 1: Write EntityRepo test module**

Create `crates/cognitive/src/repos/entity.rs` with tests only (impl comes next):

```rust
use chrono::Utc;
use common::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

// ── Types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityRow {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
    pub source: String,
    pub source_id: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub mention_count: i64,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewEntity {
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
    pub source: String,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RelationshipRow {
    pub id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relationship_type: String,
    pub strength: f64,
    pub evidence: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewRelationship {
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relationship_type: String,
    pub evidence: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct GraphNeighborhood {
    pub center: EntityRow,
    pub entities: Vec<EntityRow>,
    pub relationships: Vec<RelationshipRow>,
}

// ── Repo ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EntityRepo {
    pool: SqlitePool,
}

// Implementation added in Task 3

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_upsert_and_find_by_name() {
        let pool = cognitive_test_pool().await;
        let repo = EntityRepo::new(pool.clone());

        let entity = repo
            .upsert_entity(NewEntity {
                name: "Rust".into(),
                entity_type: "technology".into(),
                description: Some("Systems programming language".into()),
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();

        assert_eq!(entity.name, "Rust");
        assert_eq!(entity.entity_type, "technology");
        assert_eq!(entity.mention_count, 1);

        // Upsert again — should increment mention_count
        let updated = repo
            .upsert_entity(NewEntity {
                name: "Rust".into(),
                entity_type: "technology".into(),
                description: None,
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();
        assert_eq!(updated.mention_count, 2);

        // Find by name
        let found = repo.find_by_name("Rust").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Rust");

        // FTS fuzzy search
        let fuzzy = repo.find_by_name("rust").await.unwrap();
        assert!(!fuzzy.is_empty());
    }

    #[tokio::test]
    async fn test_relationships_and_neighborhood() {
        let pool = cognitive_test_pool().await;
        let repo = EntityRepo::new(pool.clone());

        let e1 = repo
            .upsert_entity(NewEntity {
                name: "API Migration".into(),
                entity_type: "project".into(),
                description: None,
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();

        let e2 = repo
            .upsert_entity(NewEntity {
                name: "Auth Team".into(),
                entity_type: "organization".into(),
                description: None,
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();

        repo.upsert_relationship(NewRelationship {
            source_entity_id: e1.id.clone(),
            target_entity_id: e2.id.clone(),
            relationship_type: "depends_on".into(),
            evidence: Some("Auth team owns the identity service".into()),
            source: "extracted".into(),
        })
        .await
        .unwrap();

        let rels = repo.get_relationships(&e1.id).await.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].relationship_type, "depends_on");

        // Upsert same relationship — should increase strength
        repo.upsert_relationship(NewRelationship {
            source_entity_id: e1.id.clone(),
            target_entity_id: e2.id.clone(),
            relationship_type: "depends_on".into(),
            evidence: None,
            source: "extracted".into(),
        })
        .await
        .unwrap();

        let rels2 = repo.get_relationships(&e1.id).await.unwrap();
        assert!(rels2[0].strength > 0.5, "Strength should increase on repeat");

        // Neighborhood
        let hood = repo.get_neighborhood(&e1.id, 1).await.unwrap();
        assert_eq!(hood.center.id, e1.id);
        assert_eq!(hood.entities.len(), 1);
        assert_eq!(hood.entities[0].name, "Auth Team");
    }

    #[tokio::test]
    async fn test_merge_entities() {
        let pool = cognitive_test_pool().await;
        let repo = EntityRepo::new(pool.clone());

        let keep = repo
            .upsert_entity(NewEntity {
                name: "John Smith".into(),
                entity_type: "person".into(),
                description: Some("Manager".into()),
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();

        let merge = repo
            .upsert_entity(NewEntity {
                name: "John".into(),
                entity_type: "person".into(),
                description: None,
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();

        let e3 = repo
            .upsert_entity(NewEntity {
                name: "Project X".into(),
                entity_type: "project".into(),
                description: None,
                source: "extracted".into(),
                source_id: None,
            })
            .await
            .unwrap();

        // Create relationship to the "John" entity (will be merged)
        repo.upsert_relationship(NewRelationship {
            source_entity_id: e3.id.clone(),
            target_entity_id: merge.id.clone(),
            relationship_type: "managed_by".into(),
            evidence: None,
            source: "extracted".into(),
        })
        .await
        .unwrap();

        // Merge "John" into "John Smith"
        repo.merge_entities(&keep.id, &merge.id).await.unwrap();

        // "John" should be gone
        let found = repo.find_by_name("John").await.unwrap();
        assert!(found.iter().all(|e| e.id != merge.id));

        // Relationship should now point to "John Smith"
        let rels = repo.get_relationships(&keep.id).await.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].source_entity_id, e3.id);
    }
}
```

- [ ] **Step 2: Register module in mod.rs**

In `crates/cognitive/src/repos/mod.rs`, add after existing module declarations:
```rust
pub mod entity;
```
And add re-export:
```rust
pub use entity::{EntityRepo, EntityRow, NewEntity, RelationshipRow, NewRelationship, GraphNeighborhood};
```

- [ ] **Step 3: Run tests — verify they fail (no implementation)**

Run: `cargo nextest run -p cognitive -E 'test(entity)'`
Expected: compilation succeeds but tests fail (methods not implemented).

---

### Task 3: EntityRepo — Implementation

**Files:**
- Modify: `crates/cognitive/src/repos/entity.rs` (add impl block above tests)

- [ ] **Step 1: Implement EntityRepo methods**

Add the implementation above the `#[cfg(test)]` module in `entity.rs`:

```rust
impl EntityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_entity(&self, entity: NewEntity) -> Result<EntityRow> {
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        // Try to find existing by name (case-insensitive) + type
        let existing: Option<EntityRow> = sqlx::query_as(
            "SELECT * FROM entities WHERE LOWER(TRIM(name)) = LOWER(TRIM(?1)) AND entity_type = ?2 LIMIT 1"
        )
        .bind(&entity.name)
        .bind(&entity.entity_type)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(existing) = existing {
            // Increment mention_count + update last_seen
            sqlx::query(
                "UPDATE entities SET mention_count = mention_count + 1, last_seen_at = ?1, updated_at = ?1, description = COALESCE(?2, description) WHERE id = ?3"
            )
            .bind(&now)
            .bind(&entity.description)
            .bind(&existing.id)
            .execute(&self.pool)
            .await?;

            return Ok(sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
                .bind(&existing.id)
                .fetch_one(&self.pool)
                .await?);
        }

        sqlx::query(
            "INSERT INTO entities (id, name, entity_type, description, source, source_id, first_seen_at, last_seen_at, mention_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 1, ?7, ?7)"
        )
        .bind(&id)
        .bind(entity.name.trim())
        .bind(&entity.entity_type)
        .bind(&entity.description)
        .bind(&entity.source)
        .bind(&entity.source_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Vec<EntityRow>> {
        // Try exact match first, then FTS
        let exact: Vec<EntityRow> = sqlx::query_as(
            "SELECT * FROM entities WHERE LOWER(TRIM(name)) = LOWER(TRIM(?1))"
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await?;

        if !exact.is_empty() {
            return Ok(exact);
        }

        // FTS fuzzy search (subquery to avoid JOIN issues with FTS5 virtual tables)
        let fts: Vec<EntityRow> = sqlx::query_as(
            "SELECT * FROM entities WHERE rowid IN (SELECT rowid FROM entities_fts WHERE entities_fts MATCH ?1 LIMIT 10)"
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(fts)
    }

    pub async fn get_relationships(&self, entity_id: &str) -> Result<Vec<RelationshipRow>> {
        let rows = sqlx::query_as::<_, RelationshipRow>(
            "SELECT * FROM entity_relationships WHERE source_entity_id = ?1 OR target_entity_id = ?1 ORDER BY strength DESC"
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn upsert_relationship(&self, rel: NewRelationship) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        // Check if relationship already exists
        let existing: Option<RelationshipRow> = sqlx::query_as(
            "SELECT * FROM entity_relationships WHERE source_entity_id = ?1 AND target_entity_id = ?2 AND relationship_type = ?3 LIMIT 1"
        )
        .bind(&rel.source_entity_id)
        .bind(&rel.target_entity_id)
        .bind(&rel.relationship_type)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(existing) = existing {
            let new_strength = (existing.strength + 0.1).min(1.0);
            sqlx::query(
                "UPDATE entity_relationships SET strength = ?1, evidence = COALESCE(?2, evidence), updated_at = ?3 WHERE id = ?4"
            )
            .bind(new_strength)
            .bind(&rel.evidence)
            .bind(&now)
            .bind(&existing.id)
            .execute(&self.pool)
            .await?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO entity_relationships (id, source_entity_id, target_entity_id, relationship_type, strength, evidence, valid_from, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0.5, ?5, ?6, ?7, ?6, ?6)"
            )
            .bind(&id)
            .bind(&rel.source_entity_id)
            .bind(&rel.target_entity_id)
            .bind(&rel.relationship_type)
            .bind(&rel.evidence)
            .bind(&now)
            .bind(&rel.source)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_neighborhood(&self, entity_id: &str, depth: u8) -> Result<GraphNeighborhood> {
        let center = sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(&self.pool)
            .await?;

        // For depth 1: get direct connections
        let relationships = self.get_relationships(entity_id).await?;

        let mut neighbor_ids: Vec<String> = Vec::new();
        for rel in &relationships {
            if rel.source_entity_id == entity_id {
                neighbor_ids.push(rel.target_entity_id.clone());
            } else {
                neighbor_ids.push(rel.source_entity_id.clone());
            }
        }
        neighbor_ids.dedup();

        let mut entities = Vec::new();
        for nid in &neighbor_ids {
            if let Ok(row) = sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
                .bind(nid)
                .fetch_one(&self.pool)
                .await
            {
                entities.push(row);
            }
        }

        // For depth > 1: recursively expand (up to max_depth)
        if depth > 1 {
            let mut all_rels = relationships.clone();
            for nid in &neighbor_ids {
                let sub_rels = self.get_relationships(nid).await?;
                for rel in &sub_rels {
                    let other_id = if rel.source_entity_id == *nid {
                        &rel.target_entity_id
                    } else {
                        &rel.source_entity_id
                    };
                    if other_id != entity_id && !neighbor_ids.contains(other_id) {
                        if let Ok(row) = sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
                            .bind(other_id)
                            .fetch_one(&self.pool)
                            .await
                        {
                            entities.push(row);
                        }
                    }
                }
                all_rels.extend(sub_rels);
            }
            return Ok(GraphNeighborhood {
                center,
                entities,
                relationships: all_rels,
            });
        }

        Ok(GraphNeighborhood {
            center,
            entities,
            relationships,
        })
    }

    pub async fn merge_entities(&self, keep_id: &str, merge_id: &str) -> Result<()> {
        // Repoint all relationships from merge_id to keep_id
        sqlx::query(
            "UPDATE entity_relationships SET source_entity_id = ?1, updated_at = ?3 WHERE source_entity_id = ?2"
        )
        .bind(keep_id)
        .bind(merge_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE entity_relationships SET target_entity_id = ?1, updated_at = ?3 WHERE target_entity_id = ?2"
        )
        .bind(keep_id)
        .bind(merge_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Sum mention counts
        let merge_row = sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
            .bind(merge_id)
            .fetch_one(&self.pool)
            .await?;

        sqlx::query(
            "UPDATE entities SET mention_count = mention_count + ?1, updated_at = ?2 WHERE id = ?3"
        )
        .bind(merge_row.mention_count)
        .bind(Utc::now().to_rfc3339())
        .bind(keep_id)
        .execute(&self.pool)
        .await?;

        // Delete merged entity
        sqlx::query("DELETE FROM entities WHERE id = ?1")
            .bind(merge_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_entities_for_note(&self, note_id: &str) -> Result<Vec<EntityRow>> {
        let rows = sqlx::query_as::<_, EntityRow>(
            "SELECT * FROM entities WHERE source_id = ?1 ORDER BY mention_count DESC"
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
```

- [ ] **Step 2: Run tests — verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(entity)'`
Expected: all 3 tests pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p cognitive --all-targets`
Expected: 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/entity.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add EntityRepo with graph queries and merge"
```

---

### Task 4: PersonaRepo — Tests + Implementation

**Files:**
- Create: `crates/cognitive/src/repos/persona.rs`
- Modify: `crates/cognitive/src/repos/mod.rs` (register module)

- [ ] **Step 1: Write PersonaRepo with tests and implementation**

Create `crates/cognitive/src/repos/persona.rs`:

```rust
use chrono::Utc;
use common::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

// ── Types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PersonaRow {
    pub id: String,
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub source: String,
    pub domains: String, // JSON array
    pub is_active: i32,
    pub relevance_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewPersona {
    pub id: String, // Caller provides ID (builtin IDs are deterministic)
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub source: String,
    pub domains: Vec<String>,
}

// ── Repo ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PersonaRepo {
    pool: SqlitePool,
}

impl PersonaRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn seed_builtins(&self) -> Result<()> {
        let builtins = vec![
            NewPersona {
                id: "builtin-skeptic".into(),
                name: "The Skeptic".into(),
                role: "Critical Analyst".into(),
                expertise: "Logic, evidence evaluation, assumption testing".into(),
                perspective: "Challenges assumptions, demands evidence, identifies logical fallacies and unsupported claims.".into(),
                tone: "skeptical".into(),
                icon: "🔍".into(),
                source: "builtin".into(),
                domains: vec![],
            },
            NewPersona {
                id: "builtin-practitioner".into(),
                name: "The Practitioner".into(),
                role: "Industry Professional".into(),
                expertise: "Real-world application, implementation, operational concerns".into(),
                perspective: "Focuses on real-world application — what works in practice vs. theory? What are the implementation pitfalls?".into(),
                tone: "pragmatic".into(),
                icon: "🔧".into(),
                source: "builtin".into(),
                domains: vec![],
            },
            NewPersona {
                id: "builtin-connector".into(),
                name: "The Connector".into(),
                role: "Interdisciplinary Researcher".into(),
                expertise: "Cross-domain patterns, transferable concepts, unexpected parallels".into(),
                perspective: "Draws parallels to other fields, identifies transferable patterns, suggests unexpected connections.".into(),
                tone: "curious".into(),
                icon: "🔗".into(),
                source: "builtin".into(),
                domains: vec![],
            },
            NewPersona {
                id: "builtin-student".into(),
                name: "The Student".into(),
                role: "Curious Learner".into(),
                expertise: "Clarity testing, jargon detection, explanation quality".into(),
                perspective: "Asks the 'dumb' questions — tests whether explanations are clear, identifies jargon, flags concepts that need unpacking.".into(),
                tone: "inquisitive".into(),
                icon: "🎓".into(),
                source: "builtin".into(),
                domains: vec![],
            },
            NewPersona {
                id: "builtin-strategist".into(),
                name: "The Strategist".into(),
                role: "Systems Thinker".into(),
                expertise: "Big picture, second-order effects, trade-offs, long-term implications".into(),
                perspective: "Looks at the bigger picture — second-order effects, trade-offs, long-term implications, and strategic considerations.".into(),
                tone: "analytical".into(),
                icon: "♟️".into(),
                source: "builtin".into(),
                domains: vec![],
            },
            NewPersona {
                id: "builtin-devils-advocate".into(),
                name: "The Devil's Advocate".into(),
                role: "Contrarian Thinker".into(),
                expertise: "Counter-arguments, steelmanning opposing views".into(),
                perspective: "Deliberately argues the opposite position. Steelmans counterarguments the user hasn't considered.".into(),
                tone: "provocative".into(),
                icon: "😈".into(),
                source: "builtin".into(),
                domains: vec![],
            },
        ];

        for p in builtins {
            self.create(p).await.ok(); // Ignore duplicates
        }
        Ok(())
    }

    pub async fn create(&self, persona: NewPersona) -> Result<PersonaRow> {
        let now = Utc::now().to_rfc3339();
        let domains_json = serde_json::to_string(&persona.domains)?;

        sqlx::query(
            "INSERT OR IGNORE INTO insight_personas (id, name, role, expertise, perspective, tone, icon, source, domains, is_active, relevance_score, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 0.5, ?10, ?10)"
        )
        .bind(&persona.id)
        .bind(&persona.name)
        .bind(&persona.role)
        .bind(&persona.expertise)
        .bind(&persona.perspective)
        .bind(&persona.tone)
        .bind(&persona.icon)
        .bind(&persona.source)
        .bind(&domains_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_as::<_, PersonaRow>("SELECT * FROM insight_personas WHERE id = ?1")
            .bind(&persona.id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn list_active(&self) -> Result<Vec<PersonaRow>> {
        let rows = sqlx::query_as::<_, PersonaRow>(
            "SELECT * FROM insight_personas WHERE is_active = 1 ORDER BY source, name"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: &str) -> Result<Option<PersonaRow>> {
        let row = sqlx::query_as::<_, PersonaRow>("SELECT * FROM insight_personas WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn set_active(&self, id: &str, active: bool) -> Result<()> {
        sqlx::query("UPDATE insight_personas SET is_active = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(active as i32)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let row = self.get(id).await?;
        if let Some(r) = row {
            if r.source == "builtin" {
                return Err(common::KlyntbotError::internal("Cannot delete builtin personas"));
            }
        }
        sqlx::query("DELETE FROM insight_personas WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Select 3-4 personas for a note based on domain matching + fallback to defaults.
    pub async fn select_for_note(
        &self,
        note_tags: &[String],
        pinned_ids: &[String],
        default_ids: &[String],
    ) -> Result<Vec<PersonaRow>> {
        // If pinned, use those
        if !pinned_ids.is_empty() {
            let mut result = Vec::new();
            for id in pinned_ids {
                if let Some(p) = self.get(id).await? {
                    if p.is_active != 0 {
                        result.push(p);
                    }
                }
            }
            if !result.is_empty() {
                return Ok(result);
            }
        }

        // Try domain matching
        let all_active = self.list_active().await?;
        let mut scored: Vec<(PersonaRow, usize)> = all_active
            .into_iter()
            .map(|p| {
                let domains: Vec<String> = serde_json::from_str(&p.domains).unwrap_or_default();
                let match_count = note_tags
                    .iter()
                    .filter(|tag| domains.iter().any(|d| d.eq_ignore_ascii_case(tag)))
                    .count();
                (p, match_count)
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.relevance_score.partial_cmp(&a.0.relevance_score).unwrap_or(std::cmp::Ordering::Equal)));

        let domain_matched: Vec<PersonaRow> = scored
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(p, _)| p)
            .take(4)
            .collect();

        if domain_matched.len() >= 2 {
            return Ok(domain_matched);
        }

        // Fallback to defaults
        let mut result = domain_matched;
        for id in default_ids {
            if result.len() >= 3 {
                break;
            }
            if result.iter().any(|p| p.id == *id) {
                continue;
            }
            if let Some(p) = self.get(id).await? {
                if p.is_active != 0 {
                    result.push(p);
                }
            }
        }
        Ok(result)
    }

    // ── Pins ────────────────────────────────────────────────────

    pub async fn get_pins(&self, note_id: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT persona_id FROM insight_persona_pins WHERE note_id = ?1"
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn set_pins(&self, note_id: &str, persona_ids: &[String]) -> Result<()> {
        sqlx::query("DELETE FROM insight_persona_pins WHERE note_id = ?1")
            .bind(note_id)
            .execute(&self.pool)
            .await?;
        let now = Utc::now().to_rfc3339();
        for pid in persona_ids {
            sqlx::query(
                "INSERT INTO insight_persona_pins (note_id, persona_id, created_at) VALUES (?1, ?2, ?3)"
            )
            .bind(note_id)
            .bind(pid)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_seed_builtins_and_list() {
        let pool = cognitive_test_pool().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        let active = repo.list_active().await.unwrap();
        assert_eq!(active.len(), 6);
        assert!(active.iter().any(|p| p.id == "builtin-skeptic"));
    }

    #[tokio::test]
    async fn test_cannot_delete_builtin() {
        let pool = cognitive_test_pool().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        let result = repo.delete("builtin-skeptic").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_select_with_defaults() {
        let pool = cognitive_test_pool().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        let selected = repo
            .select_for_note(
                &[],
                &[],
                &["builtin-skeptic".into(), "builtin-practitioner".into(), "builtin-strategist".into()],
            )
            .await
            .unwrap();

        assert_eq!(selected.len(), 3);
    }

    #[tokio::test]
    async fn test_pins_override_selection() {
        let pool = cognitive_test_pool().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        repo.set_pins("note-1", &["builtin-connector".into()])
            .await
            .unwrap();
        let pins = repo.get_pins("note-1").await.unwrap();
        assert_eq!(pins, vec!["builtin-connector"]);

        let selected = repo
            .select_for_note(&[], &pins, &["builtin-skeptic".into()])
            .await
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "builtin-connector");
    }
}
```

- [ ] **Step 2: Register module in mod.rs**

In `crates/cognitive/src/repos/mod.rs`, add:
```rust
pub mod persona;
```
And re-export:
```rust
pub use persona::{PersonaRepo, PersonaRow, NewPersona};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(persona)'`
Expected: all 4 tests pass.

- [ ] **Step 4: Run clippy + check all cognitive tests**

Run: `cargo nextest run -p cognitive && cargo clippy -p cognitive --all-targets`
Expected: all tests pass, 0 clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/persona.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add PersonaRepo with builtin seeding and selection"
```

---

### Task 4b: Entity Backfill Script (Phase 0)

**Files:**
- Create: `crates/cognitive/src/repos/entity_backfill.rs` (one-time utility, can be removed post-backfill)

- [ ] **Step 1: Write backfill function**

Create `crates/cognitive/src/repos/entity_backfill.rs`:

```rust
use common::Result;
use sqlx::SqlitePool;
use tracing::info;

/// One-time backfill: convert existing SPO facts and note_entity_mentions into the unified knowledge graph.
/// Safe to run multiple times (uses INSERT OR IGNORE).
pub async fn backfill_entities(pool: &SqlitePool) -> Result<()> {
    // Step 1: Entities from SPO facts (non-"user" subjects)
    let fact_count = sqlx::query(
        "INSERT OR IGNORE INTO entities (id, name, entity_type, source, first_seen_at, last_seen_at, mention_count, created_at, updated_at)
         SELECT
           lower(hex(randomblob(16))),
           TRIM(subject),
           CASE
             WHEN predicate LIKE '%project%' OR predicate LIKE '%works_on%' THEN 'project'
             WHEN predicate LIKE '%tool%' OR predicate LIKE '%uses%' THEN 'technology'
             WHEN predicate LIKE '%person%' OR predicate LIKE '%knows%' THEN 'person'
             ELSE 'concept'
           END,
           'backfill',
           MIN(created_at), MAX(created_at), COUNT(*),
           MIN(created_at), MIN(created_at)
         FROM semantic_facts
         WHERE subject != 'user' AND TRIM(subject) != ''
         GROUP BY LOWER(TRIM(subject))"
    )
    .execute(pool)
    .await?
    .rows_affected();

    info!("Entity backfill: created {fact_count} entities from semantic facts");

    // Step 2: Entities from tasks referenced in note_entity_mentions
    let task_count = sqlx::query(
        "INSERT OR IGNORE INTO entities (id, name, entity_type, source, source_id, first_seen_at, last_seen_at, mention_count, created_at, updated_at)
         SELECT
           lower(hex(randomblob(16))),
           t.title, 'task', 'backfill', nem.entity_id,
           t.created_at, t.created_at, 1,
           t.created_at, t.created_at
         FROM note_entity_mentions nem
         JOIN tasks t ON nem.entity_id = t.id
         WHERE nem.entity_type = 'task'"
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0); // tasks table may not exist in test pools

    info!("Entity backfill: created {task_count} entities from task mentions");

    Ok(())
}
```

- [ ] **Step 2: Register module**

In `crates/cognitive/src/repos/mod.rs`, add: `pub mod entity_backfill;`

- [ ] **Step 3: Call during cognitive init (after migration)**

In `crates/app-core/src/init/cognitive.rs`, after migrations run and repos are created, add:

```rust
// One-time entity backfill (idempotent)
cognitive::repos::entity_backfill::backfill_entities(&pool).await.ok();
```

- [ ] **Step 4: Build and verify**

Run: `cargo build --workspace`
Expected: compiles. The backfill runs on next app start.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/entity_backfill.rs crates/cognitive/src/repos/mod.rs crates/app-core/src/init/cognitive.rs
git commit -m "feat(cognitive): add entity backfill from existing SPO facts"
```

---

## Chunk 2: InsightForge Core

### Task 5: DomainSearcher Trait + MemorySource Extension

**Files:**
- Create: `crates/context_engine/src/insight_forge/domain_searcher.rs`
- Modify: `crates/context_engine/src/memory_retriever.rs` (add Domain variant + Clone derive)

- [ ] **Step 1: Add Clone derive and Domain variant to MemoryEntry/MemorySource**

In `crates/context_engine/src/memory_retriever.rs`:

First, add `Clone` to `MemoryEntry` (around line 12 — currently only derives `Debug`):
```rust
#[derive(Debug, Clone)]
pub struct MemoryEntry {
```

Then add `Clone` to `MemorySource` if not already derived, and add the `Domain` variant:
```rust
#[derive(Debug, Clone)]
pub enum MemorySource {
    /// Extracted/consolidated semantic fact (FSRS-scored).
    CognitiveFact,
    /// Past conversation message (time-decay scored).
    ConversationRecall,
    /// Domain-specific search result (notes, tasks, finance, graph).
    Domain { name: String },
}
```

**Why Clone is needed:** InsightForge's RRF merge deduplicates by cloning entries into a HashMap. Without Clone, the merge won't compile.

- [ ] **Step 2: Create DomainSearcher trait**

Create `crates/context_engine/src/insight_forge/domain_searcher.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;

use crate::MemoryEntry;

/// Trait for searching domain-specific data (notes, tasks, finance, graph).
/// Feature crates (L4+) implement this trait; instances are injected at app startup as `Arc<dyn DomainSearcher>`.
#[async_trait]
pub trait DomainSearcher: Send + Sync {
    /// Human-readable domain name (e.g., "notes", "tasks", "finance", "graph").
    fn domain_name(&self) -> &str;

    /// Search this domain for entries relevant to the query.
    /// Returns MemoryEntry instances with source set to MemorySource::Domain.
    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry>;
}
```

**Convention:** All DomainSearcher instances are passed as `Arc<dyn DomainSearcher>` throughout the system (in InsightForge, in builder methods, in app-core wiring). This matches the existing pattern for `Arc<dyn MemoryRetriever>`.

- [ ] **Step 3: Create insight_forge module skeleton**

Create `crates/context_engine/src/insight_forge/mod.rs`:

```rust
pub mod circuit_breaker;
pub mod decomposer;
pub mod domain_searcher;

pub use circuit_breaker::CircuitBreaker;
pub use decomposer::{HeuristicDecomposer, QueryDecomposer};
pub use domain_searcher::DomainSearcher;
```

- [ ] **Step 4: Update context_engine lib.rs exports**

In `crates/context_engine/src/lib.rs`, add the module declaration (types will be progressively exported as Tasks 6-8 implement them):

```rust
pub mod insight_forge;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p context-engine`
Expected: compiles (with warnings about unused modules — that's fine, we'll fill them next).

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/insight_forge/ crates/context_engine/src/memory_retriever.rs crates/context_engine/src/lib.rs
git commit -m "feat(context-engine): add DomainSearcher trait and InsightForge module skeleton"
```

---

### Task 6: Circuit Breaker

**Files:**
- Create: `crates/context_engine/src/insight_forge/circuit_breaker.rs`

- [ ] **Step 1: Implement per-session circuit breaker with tests**

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

/// Per-session circuit breaker. After `threshold` failures within `cooldown`,
/// the circuit opens and `is_open()` returns true until cooldown expires.
pub struct CircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    /// Map of session_key → (failure_count, first_failure_time)
    state: DashMap<String, (u32, Instant)>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            threshold,
            cooldown: Duration::from_secs(cooldown_secs),
            state: DashMap::new(),
        }
    }

    /// Record a failure for the given session. Returns true if circuit is now open.
    pub fn record_failure(&self, session_key: &str) -> bool {
        let mut entry = self.state.entry(session_key.to_string()).or_insert((0, Instant::now()));
        let (count, first_failure) = entry.value_mut();

        // Reset if cooldown has passed
        if first_failure.elapsed() > self.cooldown {
            *count = 1;
            *first_failure = Instant::now();
            return false;
        }

        *count += 1;
        *count >= self.threshold
    }

    /// Check if the circuit is open (should skip the component).
    pub fn is_open(&self, session_key: &str) -> bool {
        if let Some(entry) = self.state.get(session_key) {
            let (count, first_failure) = entry.value();
            if first_failure.elapsed() > self.cooldown {
                // Cooldown expired — reset
                drop(entry);
                self.state.remove(session_key);
                return false;
            }
            *count >= self.threshold
        } else {
            false
        }
    }

    /// Clean up expired entries (call periodically or on session end).
    pub fn cleanup(&self) {
        self.state.retain(|_, (_, first_failure)| first_failure.elapsed() <= self.cooldown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, 300);
        assert!(!cb.is_open("session-1"));
        assert!(!cb.record_failure("session-1"));
        assert!(!cb.record_failure("session-1"));
        assert!(cb.record_failure("session-1")); // 3rd failure opens it
        assert!(cb.is_open("session-1"));
    }

    #[test]
    fn test_circuit_breaker_per_session() {
        let cb = CircuitBreaker::new(2, 300);
        cb.record_failure("session-1");
        cb.record_failure("session-1");
        assert!(cb.is_open("session-1"));
        assert!(!cb.is_open("session-2")); // Different session unaffected
    }

    #[test]
    fn test_circuit_breaker_resets_after_cooldown() {
        let cb = CircuitBreaker::new(2, 0); // 0-second cooldown for testing
        cb.record_failure("session-1");
        cb.record_failure("session-1");
        // Cooldown is 0s, so next check should see it as expired
        std::thread::sleep(Duration::from_millis(10));
        assert!(!cb.is_open("session-1"));
    }
}
```

- [ ] **Step 2: Add dashmap dependency**

Run: `cargo add dashmap -p context-engine`

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context-engine -E 'test(circuit_breaker)'`
Expected: all 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/insight_forge/circuit_breaker.rs crates/context_engine/Cargo.toml
git commit -m "feat(context-engine): add per-session circuit breaker"
```

---

### Task 7: QueryDecomposer — Heuristic + Trait

**Files:**
- Create: `crates/context_engine/src/insight_forge/decomposer.rs`

- [ ] **Step 1: Implement QueryDecomposer trait + HeuristicDecomposer**

```rust
use async_trait::async_trait;

/// Decomposes a user query into 3-5 sub-queries for multi-dimensional retrieval.
#[async_trait]
pub trait QueryDecomposer: Send + Sync {
    async fn decompose(&self, query: &str, context_hint: Option<&str>) -> Vec<String>;
}

/// Fast keyword-based decomposer. Zero cost, no LLM call.
/// Extracts noun phrases and generates sub-queries by combining with common dimensions.
pub struct HeuristicDecomposer;

impl HeuristicDecomposer {
    /// Extract key noun phrases from the query (simple heuristic).
    fn extract_key_terms(query: &str) -> Vec<String> {
        let stop_words: &[&str] = &[
            "i", "me", "my", "we", "our", "you", "your", "the", "a", "an", "is", "are", "was",
            "were", "be", "been", "being", "have", "has", "had", "do", "does", "did", "will",
            "would", "could", "should", "may", "might", "can", "shall", "to", "of", "in", "for",
            "on", "with", "at", "by", "from", "as", "into", "about", "like", "through", "after",
            "over", "between", "out", "against", "during", "without", "before", "under", "around",
            "among", "and", "but", "or", "nor", "not", "so", "yet", "both", "either", "neither",
            "each", "every", "all", "any", "few", "more", "most", "other", "some", "such", "no",
            "only", "own", "same", "than", "too", "very", "just", "because", "if", "when", "where",
            "how", "what", "which", "who", "whom", "this", "that", "these", "those", "it", "its",
            "help", "please", "want", "need", "think", "know", "get", "make", "go", "tell", "give",
            "show", "let", "try", "ask",
        ];

        query
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .filter(|w| w.len() > 2 && !stop_words.contains(&w.as_str()))
            .collect()
    }
}

#[async_trait]
impl QueryDecomposer for HeuristicDecomposer {
    async fn decompose(&self, query: &str, _context_hint: Option<&str>) -> Vec<String> {
        let terms = Self::extract_key_terms(query);

        if terms.is_empty() {
            return vec![query.to_string()];
        }

        let mut sub_queries = Vec::new();

        // Original query always included
        sub_queries.push(query.to_string());

        // Combine key terms with dimension suffixes
        let dimensions = ["background context", "current status", "related people and teams", "risks and blockers", "timeline and deadlines"];

        let topic = terms.join(" ");
        for dim in &dimensions {
            sub_queries.push(format!("{topic} {dim}"));
            if sub_queries.len() >= 5 {
                break;
            }
        }

        sub_queries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_heuristic_decomposer_produces_sub_queries() {
        let decomposer = HeuristicDecomposer;
        let subs = decomposer.decompose("Help me plan the API migration project", None).await;

        assert!(subs.len() >= 3, "Should produce at least 3 sub-queries, got {}", subs.len());
        assert!(subs.len() <= 6);
        assert_eq!(subs[0], "Help me plan the API migration project"); // Original always first
        assert!(subs[1].contains("api"));
    }

    #[tokio::test]
    async fn test_heuristic_with_short_query() {
        let decomposer = HeuristicDecomposer;
        let subs = decomposer.decompose("hi", None).await;

        // Short query with only stop words → returns original
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0], "hi");
    }

    #[tokio::test]
    async fn test_stop_word_filtering() {
        let terms = HeuristicDecomposer::extract_key_terms("help me plan the API migration for our team");
        // "plan" and "help" and "team" are stop words
        assert!(terms.contains(&"api".to_string()));
        assert!(terms.contains(&"migration".to_string()));
        assert!(!terms.contains(&"help".to_string()));
        assert!(!terms.contains(&"the".to_string()));
        assert!(!terms.contains(&"plan".to_string()));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p context-engine -E 'test(decomposer)'`
Expected: all 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/context_engine/src/insight_forge/decomposer.rs
git commit -m "feat(context-engine): add QueryDecomposer trait + HeuristicDecomposer"
```

---

### Task 8: InsightForge Orchestrator

**Files:**
- Modify: `crates/context_engine/src/insight_forge/mod.rs` (replace skeleton with full implementation)

- [ ] **Step 1: Implement InsightForge**

Replace the contents of `crates/context_engine/src/insight_forge/mod.rs`:

```rust
pub mod circuit_breaker;
pub mod decomposer;
pub mod domain_searcher;

pub use circuit_breaker::CircuitBreaker;
pub use decomposer::{HeuristicDecomposer, QueryDecomposer};
pub use domain_searcher::DomainSearcher;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use crate::memory_retriever::{MemoryEntry, MemoryRetriever};
use crate::ExecutionStrategy;

/// Configuration for InsightForge.
#[derive(Debug, Clone)]
pub struct InsightForgeConfig {
    pub enabled: bool,
    pub max_sub_queries: usize,
    pub per_source_limit: usize,
    pub total_limit: usize,
    pub per_source_timeout_ms: u64,
    pub decomposer_timeout_ms: u64,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_cooldown_secs: u64,
}

impl Default for InsightForgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_sub_queries: 5,
            per_source_limit: 5,
            total_limit: 15,
            per_source_timeout_ms: 800,
            decomposer_timeout_ms: 2000,
            circuit_breaker_threshold: 3,
            circuit_breaker_cooldown_secs: 300,
        }
    }
}

/// Multi-dimensional context retrieval engine.
/// Decomposes queries into sub-queries, searches multiple sources in parallel,
/// and merges results via RRF.
pub struct InsightForge {
    config: InsightForgeConfig,
    decomposer: Arc<dyn QueryDecomposer>,
    memory_retriever: Arc<dyn MemoryRetriever>,
    searchers: Vec<Arc<dyn DomainSearcher>>,
    circuit_breaker: CircuitBreaker,
}

impl InsightForge {
    pub fn new(
        config: InsightForgeConfig,
        decomposer: Arc<dyn QueryDecomposer>,
        memory_retriever: Arc<dyn MemoryRetriever>,
    ) -> Self {
        let cb = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_cooldown_secs,
        );
        Self {
            config,
            decomposer,
            memory_retriever,
            searchers: Vec::new(),
            circuit_breaker: cb,
        }
    }

    pub fn add_searcher(&mut self, searcher: Arc<dyn DomainSearcher>) {
        self.searchers.push(searcher);
    }

    /// Determine if InsightForge should activate for this request.
    /// Activates for all strategies except Clarification and very short messages.
    /// DirectResponse is included because analytical questions ("how does X relate to Y?")
    /// may be classified as Direct but still benefit from multi-dimensional context.
    pub fn should_activate(&self, strategy: &ExecutionStrategy, message: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Skip for clarifications and very short messages
        if matches!(strategy, ExecutionStrategy::Clarification { .. }) {
            return false;
        }
        if message.len() < 20 {
            return false;
        }

        // Activate for all non-clarification strategies (including DirectResponse)
        true
    }

    /// Multi-dimensional retrieval: decompose → parallel search → RRF merge.
    pub async fn retrieve(
        &self,
        query: &str,
        total_limit: usize,
        session_key: Option<&str>,
    ) -> Vec<MemoryEntry> {
        let session = session_key.unwrap_or("default");

        // Check circuit breaker
        if self.circuit_breaker.is_open(session) {
            tracing::debug!("InsightForge circuit breaker open for session {session}, falling back");
            return self.fallback_retrieve(query, total_limit).await;
        }

        // Step 1: Decompose
        let sub_queries = match timeout(
            Duration::from_millis(self.config.decomposer_timeout_ms),
            self.decomposer.decompose(query, None),
        )
        .await
        {
            Ok(subs) if subs.len() >= 2 => subs,
            Ok(subs) => {
                // Decomposer returned too few — use original + subs
                let mut merged = vec![query.to_string()];
                merged.extend(subs);
                merged
            }
            Err(_) => {
                tracing::warn!("InsightForge decomposer timed out, falling back");
                self.circuit_breaker.record_failure(session);
                return self.fallback_retrieve(query, total_limit).await;
            }
        };

        let sub_queries: Vec<String> = sub_queries
            .into_iter()
            .take(self.config.max_sub_queries)
            .collect();

        // Step 2: Parallel search across all sources for each sub-query
        let per_source_limit = self.config.per_source_limit;
        let source_timeout = Duration::from_millis(self.config.per_source_timeout_ms);

        let mut all_results: Vec<Vec<MemoryEntry>> = Vec::new();

        for sub_query in &sub_queries {
            let mut handles = Vec::new();

            // Memory retriever
            let mr = self.memory_retriever.clone();
            let sq = sub_query.clone();
            let st = source_timeout;
            handles.push(tokio::spawn(async move {
                timeout(st, mr.retrieve(&sq, per_source_limit))
                    .await
                    .unwrap_or_default()
            }));

            // Domain searchers
            for searcher in &self.searchers {
                let s = searcher.clone();
                let sq = sub_query.clone();
                let st = source_timeout;
                handles.push(tokio::spawn(async move {
                    timeout(st, s.search(&sq, per_source_limit))
                        .await
                        .unwrap_or_default()
                }));
            }

            let mut sub_query_results = Vec::new();
            for handle in handles {
                if let Ok(entries) = handle.await {
                    sub_query_results.extend(entries);
                }
            }
            all_results.push(sub_query_results);
        }

        // Step 3: RRF merge across sub-queries
        let merged = self.rrf_merge(&all_results, total_limit);
        merged
    }

    /// Fallback: single-query retrieval via memory retriever only (same as today).
    async fn fallback_retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.memory_retriever.retrieve(query, limit).await
    }

    /// Reciprocal Rank Fusion merge across multiple ranked lists.
    /// Items appearing in more lists rank higher.
    fn rrf_merge(&self, ranked_lists: &[Vec<MemoryEntry>], limit: usize) -> Vec<MemoryEntry> {
        const K: f64 = 60.0;
        let mut scores: HashMap<String, (f64, MemoryEntry)> = HashMap::new();

        for list in ranked_lists {
            for (rank, entry) in list.iter().enumerate() {
                let rrf_score = 1.0 / (K + rank as f64 + 1.0);
                scores
                    .entry(entry.id.clone())
                    .and_modify(|(score, _)| *score += rrf_score)
                    .or_insert((rrf_score, entry.clone()));
            }
        }

        let mut merged: Vec<(f64, MemoryEntry)> = scores.into_values().collect();
        merged.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Re-normalize scores to 0.0-1.0
        let max_score = merged.first().map(|(s, _)| *s).unwrap_or(1.0);
        merged
            .into_iter()
            .take(limit)
            .map(|(score, mut entry)| {
                entry.score = if max_score > 0.0 { score / max_score } else { 0.0 };
                entry
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_retriever::MemorySource;

    struct MockRetriever;

    #[async_trait::async_trait]
    impl MemoryRetriever for MockRetriever {
        async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
            vec![MemoryEntry {
                id: format!("mem-{}", query.len()),
                content: format!("Memory about: {query}"),
                score: 0.8,
                source: MemorySource::CognitiveFact,
                raw_score: 0.8,
            }]
            .into_iter()
            .take(limit)
            .collect()
        }
    }

    #[tokio::test]
    async fn test_insight_forge_retrieve_with_heuristic() {
        let forge = InsightForge::new(
            InsightForgeConfig::default(),
            Arc::new(HeuristicDecomposer),
            Arc::new(MockRetriever),
        );

        let results = forge
            .retrieve("Help me plan the API migration project", 10, None)
            .await;

        // Should have results from multiple sub-queries, deduplicated
        assert!(!results.is_empty());
        // Scores should be normalized 0.0-1.0
        assert!(results[0].score <= 1.0);
    }

    #[tokio::test]
    async fn test_should_activate() {
        let forge = InsightForge::new(
            InsightForgeConfig::default(),
            Arc::new(HeuristicDecomposer),
            Arc::new(MockRetriever),
        );

        // Short message → no (regardless of strategy)
        assert!(!forge.should_activate(&ExecutionStrategy::DirectResponse, "hi"));

        // Clarification → no
        assert!(!forge.should_activate(
            &ExecutionStrategy::Clarification { reason: "test".into() },
            "what do you mean by that long question?"
        ));

        // Tool-assisted with long message → yes
        assert!(forge.should_activate(
            &ExecutionStrategy::ToolAssisted { max_iterations: 10 },
            "Help me plan the API migration project"
        ));

        // DirectResponse with long message → yes (analytical queries benefit too)
        assert!(forge.should_activate(
            &ExecutionStrategy::DirectResponse,
            "How does the API migration relate to our Q2 goals?"
        ));
    }

    #[test]
    fn test_rrf_merge_deduplicates() {
        let forge = InsightForge::new(
            InsightForgeConfig::default(),
            Arc::new(HeuristicDecomposer),
            Arc::new(MockRetriever),
        );

        let list1 = vec![
            MemoryEntry { id: "a".into(), content: "A".into(), score: 0.9, source: MemorySource::CognitiveFact, raw_score: 0.9 },
            MemoryEntry { id: "b".into(), content: "B".into(), score: 0.8, source: MemorySource::CognitiveFact, raw_score: 0.8 },
        ];
        let list2 = vec![
            MemoryEntry { id: "a".into(), content: "A".into(), score: 0.7, source: MemorySource::CognitiveFact, raw_score: 0.7 },
            MemoryEntry { id: "c".into(), content: "C".into(), score: 0.6, source: MemorySource::CognitiveFact, raw_score: 0.6 },
        ];

        let merged = forge.rrf_merge(&[list1, list2], 10);

        // "a" appears in both lists → should rank highest
        assert_eq!(merged[0].id, "a");
        // Should have 3 unique items
        assert_eq!(merged.len(), 3);
    }
}
```

- [ ] **Step 2: Update lib.rs re-exports**

In `crates/context_engine/src/lib.rs`, add the re-exports (now that all types exist):

```rust
pub use insight_forge::{
    CircuitBreaker, DomainSearcher, HeuristicDecomposer, InsightForge, InsightForgeConfig,
    QueryDecomposer,
};
```

- [ ] **Step 3: Run all InsightForge tests**

Run: `cargo nextest run -p context-engine -E 'test(insight_forge)' && cargo nextest run -p context-engine -E 'test(circuit_breaker)' && cargo nextest run -p context-engine -E 'test(decomposer)'`
Expected: all tests pass.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p context-engine --all-targets`
Expected: 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/insight_forge/
git commit -m "feat(context-engine): add InsightForge orchestrator with RRF merge"
```

---

## Chunk 3: Integration + Agent Tweaks

### Task 9: Integrate InsightForge into ContextEngine

**Files:**
- Modify: `crates/context_engine/src/assembler/mod.rs` (add InsightForge field + builder + integration)

- [ ] **Step 1: Add InsightForge field to ContextEngine struct**

In `crates/context_engine/src/assembler/mod.rs`, add to the `ContextEngine` struct (around line 28-38):

```rust
// Add after the `sources` field:
    /// Optional InsightForge for multi-dimensional retrieval.
    insight_forge: Option<Arc<crate::insight_forge::InsightForge>>,
```

Add `use std::sync::Arc;` if not already imported (it likely is for `cache`).

- [ ] **Step 2: Update ALL constructor/builder methods that build ContextEngine from struct literals**

The `with_token_counter()` (line 62) and `with_compressor_config()` (line 76) methods construct `Self { ... }` with exhaustive field lists. Every such method must forward the new field:

```rust
// In every builder method that uses `Self { field1, field2, ... }`:
// Add: insight_forge: self.insight_forge,
```

Search for all occurrences of `Self {` in the file and add `insight_forge: self.insight_forge,` (or `insight_forge: None,` for constructors). Also ensure the `new()` function initializes `insight_forge: None`.

- [ ] **Step 3: Add InsightForge builder method**

Add after the existing builder methods (after `with_sources` around line 117):

```rust
    /// Wire InsightForge for multi-dimensional context retrieval.
    /// All DomainSearchers must be added to the forge BEFORE calling this method,
    /// since InsightForge is wrapped in Arc and becomes immutable.
    pub fn with_insight_forge(mut self, forge: crate::insight_forge::InsightForge) -> Self {
        self.insight_forge = Some(Arc::new(forge));
        self
    }
```

- [ ] **Step 4: Integrate in retrieve_memory()**

Find the `retrieve_memory()` method (around line 343). The method signature is `async fn retrieve_memory(&self, request: &ContextRequest) -> Option<String>` (returns `Option<String>`, NOT `Option<(String, usize)>`).

Replace ONLY the retrieval call at the top of the method — the part that gets `entries: Vec<MemoryEntry>`. Keep the entire existing formatting/partitioning logic below it unchanged:

```rust
    // Replace the retrieval call (approximately lines 345-347):
    // OLD:
    //   let entries = retriever.retrieve(&request.message_text, self.memory_retrieval_limit).await;
    // NEW:
    let entries = if let Some(ref forge) = self.insight_forge {
        if forge.should_activate(&request.strategy, &request.message_text) {
            forge
                .retrieve(&request.message_text, self.memory_retrieval_limit, None)
                .await
        } else {
            retriever
                .retrieve(&request.message_text, self.memory_retrieval_limit)
                .await
        }
    } else {
        retriever
            .retrieve(&request.message_text, self.memory_retrieval_limit)
            .await
    };

    // KEEP everything below this point unchanged:
    // - The partition into CognitiveFact/ConversationRecall/Domain groups
    // - The formatting into a string
    // - The return of Option<String>
```

**Critical:** Do NOT change the method signature or return type. Do NOT remove the existing formatting logic. Only swap out the single line that calls `retriever.retrieve()`.

- [ ] **Step 5: Build and run existing tests**

Run: `cargo nextest run -p context-engine`
Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/assembler/mod.rs
git commit -m "feat(context-engine): integrate InsightForge into ContextEngine"
```

---

### Task 10: Multi-Perspective Planning Prompt

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (tweak planning prompt)

- [ ] **Step 1: Update build_planning_prompt()**

Find `build_planning_prompt()` (around line 899). Add the multi-perspective preamble before the existing planning instructions:

```rust
fn build_planning_prompt(user_message: &str, tools: &[serde_json::Value]) -> String {
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();

    format!(
        "This is a complex request. Before executing:\n\
         1. Briefly consider the optimistic, skeptical, and practical angles.\n\
         2. Synthesize into a balanced approach.\n\
         3. Then create a step-by-step plan.\n\n\
         User request: {user_message}\n\n\
         Available tools: {tool_list}\n\n\
         Format each step as:\n\
         1. <description> [tool: <tool_name>]\n\
         2. ...\n\n\
         Keep the plan concise (3-7 steps). Then execute step 1.",
        user_message = user_message,
        tool_list = tool_names.join(", "),
    )
}
```

- [ ] **Step 2: Build and run existing agent tests**

Run: `cargo nextest run -p agent`
Expected: all existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): add multi-perspective reasoning to planning prompt"
```

---

### Task 11: Add ContradictionDetected DomainEvent (Phase 3 Prep)

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add ContradictionDetected variant**

In `crates/bus/src/domain_events.rs`, find the `DomainEvent` enum and add before the closing brace (around line 253):

```rust
    /// A newly extracted fact contradicts an existing user-stated fact.
    /// Surfaced conversationally by the agent.
    ContradictionDetected {
        existing_subject: String,
        existing_predicate: String,
        existing_object: String,
        new_object: String,
        confidence: f64,
    },
```

- [ ] **Step 2: Build workspace to verify no breakage**

Run: `cargo build --workspace`
Expected: compiles. Any match exhaustiveness warnings will appear in subscribers — these are expected and will be handled when Phase 3 is implemented. Add a `_ => {}` catch-all if needed in any existing match blocks.

- [ ] **Step 3: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add ContradictionDetected domain event (Phase 3 prep)"
```

---

### Task 12: Wire InsightForge in App Initialization

**Files:**
- Modify: `crates/app-core/src/init/mod.rs` (or the file that builds ContextEngine)

- [ ] **Step 1: Create InsightForge during init and pass to ContextEngine**

Find where `ContextEngine` is constructed during app initialization (in `crates/app-core/src/init/` — look for `.with_memory_retriever()`). Add InsightForge wiring:

```rust
// After creating the memory_retriever:
let mut insight_forge = {
    use context_engine::insight_forge::{InsightForge, InsightForgeConfig, HeuristicDecomposer};
    let config = InsightForgeConfig::default(); // TODO: read from Config in a later task
    InsightForge::new(
        config,
        Arc::new(HeuristicDecomposer),
        memory_retriever.clone(),  // Same retriever used by ContextEngine
    )
};

// Add DomainSearchers BEFORE wrapping in Arc (add_searcher takes &mut self).
// For Phase 1, no searchers yet — they'll be added in Phase 2+ as each
// feature crate implements DomainSearcher.
// Example for later:
//   insight_forge.add_searcher(Arc::new(NoteSearcher::new(...)));
//   insight_forge.add_searcher(Arc::new(GraphSearcher::new(...)));

// Then add to ContextEngine builder (wraps in Arc internally):
let context_engine = ContextEngine::new(/* existing args */)
    .with_memory_retriever(memory_retriever)
    .with_insight_forge(insight_forge)  // NEW — Arc wrapping happens inside
    // ... rest of existing builder calls
```

**Note:** DomainSearchers are additive — InsightForge works with just the memory retriever for now. Each Phase 2+ feature crate will implement `DomainSearcher` and register here.

- [ ] **Step 2: Build and run full workspace**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: everything compiles and all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/
git commit -m "feat(app-core): wire InsightForge into ContextEngine during init"
```

---

### Task 13: Seed Builtin Personas at Startup

**Files:**
- Modify: `crates/app-core/src/init/cognitive.rs` (or where cognitive repos are initialized)

- [ ] **Step 1: Call PersonaRepo.seed_builtins() during cognitive init**

Find `init_cognitive()` in `crates/app-core/src/init/cognitive.rs` (around line 13). After the cognitive repos are created, add:

```rust
// After repos are available:
let persona_repo = PersonaRepo::new(pool.clone().into());
persona_repo.seed_builtins().await.ok(); // Idempotent, safe to call every startup
```

- [ ] **Step 2: Build and run**

Run: `cargo build --workspace && cargo nextest run -p app-core`
Expected: compiles, tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/cognitive.rs
git commit -m "feat(app-core): seed builtin personas on startup"
```

---

### Task 14: Final Verification

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: 0 errors.

- [ ] **Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (may have pre-existing desktop exceptions).

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: no formatting issues.

- [ ] **Step 5: Final commit (if any formatting fixes needed)**

```bash
cargo fmt --all
git add -A && git commit -m "style: format after Phase 0 + Phase 1"
```
