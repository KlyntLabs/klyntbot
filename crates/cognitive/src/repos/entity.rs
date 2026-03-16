//! Repository for the `entities` and `entity_relationships` tables.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Row types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    pub neighbors: Vec<EntityRow>,
    pub relationships: Vec<RelationshipRow>,
}

// ── Repository ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EntityRepo {
    pool: SqlitePool,
}

impl EntityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert an entity by case-insensitive name + entity_type.
    /// If it already exists, increment `mention_count` and update `last_seen_at`.
    /// Returns the entity row (existing or newly created).
    pub async fn upsert_entity(&self, entity: &NewEntity) -> Result<EntityRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();

        // Check for existing entity by normalized name + type
        let existing = sqlx::query_as::<_, EntityRow>(
            "SELECT * FROM entities WHERE LOWER(TRIM(name)) = LOWER(TRIM(?1)) AND entity_type = ?2",
        )
        .bind(&entity.name)
        .bind(&entity.entity_type)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            // Increment mention_count and update last_seen_at
            sqlx::query(
                "UPDATE entities SET mention_count = mention_count + 1, last_seen_at = ?1, updated_at = ?1, description = COALESCE(?2, description) WHERE id = ?3",
            )
            .bind(&now)
            .bind(&entity.description)
            .bind(&row.id)
            .execute(&self.pool)
            .await?;

            sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
                .bind(&row.id)
                .fetch_one(&self.pool)
                .await
        } else {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO entities (id, name, entity_type, description, source, source_id,
                    first_seen_at, last_seen_at, mention_count, metadata, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 1, ?8, ?7, ?7)
                "#,
            )
            .bind(&id)
            .bind(&entity.name)
            .bind(&entity.entity_type)
            .bind(&entity.description)
            .bind(&entity.source)
            .bind(&entity.source_id)
            .bind(&now)
            .bind(&entity.metadata)
            .execute(&self.pool)
            .await?;

            sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
                .bind(&id)
                .fetch_one(&self.pool)
                .await
        }
    }

    /// Find entities by name: exact match first, then FTS5 fallback.
    pub async fn find_by_name(&self, query: &str) -> Result<Vec<EntityRow>, sqlx::Error> {
        // Try exact match first
        let exact = sqlx::query_as::<_, EntityRow>(
            "SELECT * FROM entities WHERE LOWER(TRIM(name)) = LOWER(TRIM(?1))",
        )
        .bind(query)
        .fetch_all(&self.pool)
        .await?;

        if !exact.is_empty() {
            return Ok(exact);
        }

        // FTS5 fallback via subquery (quote the query to handle special chars)
        let fts_query = format!("\"{}\"", query.replace('"', "\"\""));
        sqlx::query_as::<_, EntityRow>(
            "SELECT * FROM entities WHERE rowid IN (SELECT rowid FROM entities_fts WHERE entities_fts MATCH ?1 LIMIT 10)",
        )
        .bind(&fts_query)
        .fetch_all(&self.pool)
        .await
    }

    /// Get all relationships for a given entity (both directions).
    pub async fn get_relationships(
        &self,
        entity_id: &str,
    ) -> Result<Vec<RelationshipRow>, sqlx::Error> {
        sqlx::query_as::<_, RelationshipRow>(
            "SELECT * FROM entity_relationships WHERE source_entity_id = ?1 OR target_entity_id = ?1",
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Upsert a relationship. If same source+target+type exists, bump strength by 0.1 (capped at 1.0).
    pub async fn upsert_relationship(
        &self,
        rel: &NewRelationship,
    ) -> Result<RelationshipRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();

        let existing = sqlx::query_as::<_, RelationshipRow>(
            "SELECT * FROM entity_relationships WHERE source_entity_id = ?1 AND target_entity_id = ?2 AND relationship_type = ?3",
        )
        .bind(&rel.source_entity_id)
        .bind(&rel.target_entity_id)
        .bind(&rel.relationship_type)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            let new_strength = (row.strength + 0.1).min(1.0);
            sqlx::query(
                "UPDATE entity_relationships SET strength = ?1, updated_at = ?2, evidence = COALESCE(?3, evidence) WHERE id = ?4",
            )
            .bind(new_strength)
            .bind(&now)
            .bind(&rel.evidence)
            .bind(&row.id)
            .execute(&self.pool)
            .await?;

            sqlx::query_as::<_, RelationshipRow>("SELECT * FROM entity_relationships WHERE id = ?1")
                .bind(&row.id)
                .fetch_one(&self.pool)
                .await
        } else {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO entity_relationships (id, source_entity_id, target_entity_id,
                    relationship_type, strength, evidence, source, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, 0.5, ?5, ?6, ?7, ?7)
                "#,
            )
            .bind(&id)
            .bind(&rel.source_entity_id)
            .bind(&rel.target_entity_id)
            .bind(&rel.relationship_type)
            .bind(&rel.evidence)
            .bind(&rel.source)
            .bind(&now)
            .execute(&self.pool)
            .await?;

            sqlx::query_as::<_, RelationshipRow>("SELECT * FROM entity_relationships WHERE id = ?1")
                .bind(&id)
                .fetch_one(&self.pool)
                .await
        }
    }

    /// Get a neighborhood around an entity at depth 1 or 2.
    pub async fn get_neighborhood(
        &self,
        entity_id: &str,
        depth: u32,
    ) -> Result<Option<GraphNeighborhood>, sqlx::Error> {
        let center = sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
            .bind(entity_id)
            .fetch_optional(&self.pool)
            .await?;

        let center = match center {
            Some(c) => c,
            None => return Ok(None),
        };

        // Depth 1: direct connections
        let rels = self.get_relationships(entity_id).await?;
        let mut neighbor_ids: Vec<String> = Vec::new();
        for r in &rels {
            if r.source_entity_id == entity_id {
                neighbor_ids.push(r.target_entity_id.clone());
            } else {
                neighbor_ids.push(r.source_entity_id.clone());
            }
        }
        neighbor_ids.sort();
        neighbor_ids.dedup();

        let mut neighbors = self.get_entities_by_ids(&neighbor_ids).await?;
        let mut all_rels = rels;

        // Depth 2: expand neighbors' connections
        if depth >= 2 {
            let mut depth2_ids: Vec<String> = Vec::new();
            for nid in &neighbor_ids {
                let n_rels = self.get_relationships(nid).await?;
                for r in &n_rels {
                    let other = if r.source_entity_id == *nid {
                        &r.target_entity_id
                    } else {
                        &r.source_entity_id
                    };
                    if other != entity_id && !neighbor_ids.contains(other) {
                        depth2_ids.push(other.clone());
                    }
                    // Add relationship if not already present
                    if !all_rels.iter().any(|existing| existing.id == r.id) {
                        all_rels.push(r.clone());
                    }
                }
            }
            depth2_ids.sort();
            depth2_ids.dedup();
            let depth2_entities = self.get_entities_by_ids(&depth2_ids).await?;
            neighbors.extend(depth2_entities);
        }

        Ok(Some(GraphNeighborhood {
            center,
            neighbors,
            relationships: all_rels,
        }))
    }

    /// Merge `source_id` entity into `target_id`: repoint all relationships,
    /// sum mention_count, delete the source entity.
    pub async fn merge_entities(
        &self,
        source_id: &str,
        target_id: &str,
    ) -> Result<EntityRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        // Get source mention_count
        let source: EntityRow = sqlx::query_as("SELECT * FROM entities WHERE id = ?1")
            .bind(source_id)
            .fetch_one(&mut *tx)
            .await?;

        // Add source mention_count to target
        sqlx::query(
            "UPDATE entities SET mention_count = mention_count + ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(source.mention_count)
        .bind(&now)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;

        // Repoint relationships: source_entity_id
        sqlx::query(
            "UPDATE entity_relationships SET source_entity_id = ?1, updated_at = ?2 WHERE source_entity_id = ?3",
        )
        .bind(target_id)
        .bind(&now)
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

        // Repoint relationships: target_entity_id
        sqlx::query(
            "UPDATE entity_relationships SET target_entity_id = ?1, updated_at = ?2 WHERE target_entity_id = ?3",
        )
        .bind(target_id)
        .bind(&now)
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

        // Delete self-referencing relationships that arose from the merge
        sqlx::query("DELETE FROM entity_relationships WHERE source_entity_id = target_entity_id")
            .execute(&mut *tx)
            .await?;

        // Delete the source entity (FTS trigger handles cleanup)
        sqlx::query("DELETE FROM entities WHERE id = ?1")
            .bind(source_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        sqlx::query_as::<_, EntityRow>("SELECT * FROM entities WHERE id = ?1")
            .bind(target_id)
            .fetch_one(&self.pool)
            .await
    }

    /// Get entities linked to a note via `source_id`.
    pub async fn get_entities_for_note(
        &self,
        note_id: &str,
    ) -> Result<Vec<EntityRow>, sqlx::Error> {
        sqlx::query_as::<_, EntityRow>(
            "SELECT * FROM entities WHERE source_id = ?1 ORDER BY mention_count DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    // ── Helpers ─────────────────────────────────────────────────

    async fn get_entities_by_ids(&self, ids: &[String]) -> Result<Vec<EntityRow>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT * FROM entities WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, EntityRow>(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.fetch_all(&self.pool).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn person_entity(name: &str) -> NewEntity {
        NewEntity {
            name: name.to_string(),
            entity_type: "person".to_string(),
            description: None,
            source: "extracted".to_string(),
            source_id: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_find_by_name() {
        let pool = setup().await;
        let repo = EntityRepo::new(pool.clone());

        // First insert
        let row = repo.upsert_entity(&person_entity("Alice")).await.unwrap();
        assert_eq!(row.name, "Alice");
        assert_eq!(row.mention_count, 1);

        // Upsert again (case-insensitive match) — should increment
        let row2 = repo.upsert_entity(&person_entity("alice")).await.unwrap();
        assert_eq!(row2.id, row.id);
        assert_eq!(row2.mention_count, 2);

        // find_by_name exact match
        let found = repo.find_by_name("Alice").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, row.id);

        // find_by_name via FTS
        let fts = repo.find_by_name("alice").await.unwrap();
        assert!(!fts.is_empty());
    }

    #[tokio::test]
    async fn test_relationships_and_neighborhood() {
        let pool = setup().await;
        let repo = EntityRepo::new(pool.clone());

        let alice = repo.upsert_entity(&person_entity("Alice")).await.unwrap();
        let bob = repo.upsert_entity(&person_entity("Bob")).await.unwrap();
        let charlie = repo.upsert_entity(&person_entity("Charlie")).await.unwrap();

        // Alice -> Bob (colleague)
        let rel1 = repo
            .upsert_relationship(&NewRelationship {
                source_entity_id: alice.id.clone(),
                target_entity_id: bob.id.clone(),
                relationship_type: "colleague".into(),
                evidence: Some("seen in meeting".into()),
                source: "extracted".into(),
            })
            .await
            .unwrap();
        assert!((rel1.strength - 0.5).abs() < f64::EPSILON);

        // Upsert same relationship — strength should bump to 0.6
        let rel1_updated = repo
            .upsert_relationship(&NewRelationship {
                source_entity_id: alice.id.clone(),
                target_entity_id: bob.id.clone(),
                relationship_type: "colleague".into(),
                evidence: None,
                source: "extracted".into(),
            })
            .await
            .unwrap();
        assert!((rel1_updated.strength - 0.6).abs() < f64::EPSILON);

        // Bob -> Charlie
        repo.upsert_relationship(&NewRelationship {
            source_entity_id: bob.id.clone(),
            target_entity_id: charlie.id.clone(),
            relationship_type: "friend".into(),
            evidence: None,
            source: "extracted".into(),
        })
        .await
        .unwrap();

        // Neighborhood depth 1 from Alice: should see Bob but not Charlie
        let hood1 = repo.get_neighborhood(&alice.id, 1).await.unwrap().unwrap();
        assert_eq!(hood1.center.id, alice.id);
        assert_eq!(hood1.neighbors.len(), 1);
        assert_eq!(hood1.neighbors[0].id, bob.id);

        // Neighborhood depth 2: should see both Bob and Charlie
        let hood2 = repo.get_neighborhood(&alice.id, 2).await.unwrap().unwrap();
        assert_eq!(hood2.neighbors.len(), 2);
        let ids: Vec<&str> = hood2.neighbors.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&bob.id.as_str()));
        assert!(ids.contains(&charlie.id.as_str()));
    }

    #[tokio::test]
    async fn test_merge_entities() {
        let pool = setup().await;
        let repo = EntityRepo::new(pool.clone());

        let alice = repo.upsert_entity(&person_entity("Alice")).await.unwrap();
        // Bump Alice to 2 mentions
        repo.upsert_entity(&person_entity("Alice")).await.unwrap();
        let alice_dup = repo
            .upsert_entity(&NewEntity {
                name: "A. Smith".to_string(),
                entity_type: "person".to_string(),
                description: Some("Alice Smith".into()),
                source: "extracted".to_string(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();

        let bob = repo.upsert_entity(&person_entity("Bob")).await.unwrap();

        // alice_dup -> Bob relationship
        repo.upsert_relationship(&NewRelationship {
            source_entity_id: alice_dup.id.clone(),
            target_entity_id: bob.id.clone(),
            relationship_type: "colleague".into(),
            evidence: None,
            source: "extracted".into(),
        })
        .await
        .unwrap();

        // Merge alice_dup into alice
        let merged = repo.merge_entities(&alice_dup.id, &alice.id).await.unwrap();
        // mention_count: alice had 2, alice_dup had 1 → 3
        assert_eq!(merged.mention_count, 3);

        // alice_dup should be gone
        let gone = repo.find_by_name("A. Smith").await.unwrap();
        assert!(gone.is_empty());

        // Relationship should now point to alice
        let rels = repo.get_relationships(&alice.id).await.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].source_entity_id, alice.id);
        assert_eq!(rels[0].target_entity_id, bob.id);
    }
}
