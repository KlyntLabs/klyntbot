//! Cross-database entity relation CRUD.

use sqlx::{FromRow, SqlitePool};

use crate::types::EntityRelation;
use common::Result;

#[derive(Debug, FromRow)]
struct RelationRow {
    id: String,
    source_id: String,
    source_db_id: String,
    target_id: String,
    target_db_id: String,
    relation_type: String,
    inferred: bool,
    confidence: Option<f64>,
    created_at: String,
}

impl From<RelationRow> for EntityRelation {
    fn from(r: RelationRow) -> Self {
        Self {
            id: r.id,
            source_id: r.source_id,
            source_db_id: r.source_db_id,
            target_id: r.target_id,
            target_db_id: r.target_db_id,
            relation_type: r.relation_type,
            inferred: r.inferred,
            confidence: r.confidence,
            created_at: r.created_at,
        }
    }
}

/// Input for creating a relation.
pub struct CreateRelationInput<'a> {
    pub source_id: &'a str,
    pub source_db_id: &'a str,
    pub target_id: &'a str,
    pub target_db_id: &'a str,
    pub relation_type: &'a str,
    pub inferred: bool,
    pub confidence: Option<f64>,
}

/// Create a relation between two entities (possibly across databases).
pub async fn create_relation(
    pool: &SqlitePool,
    input: CreateRelationInput<'_>,
) -> Result<EntityRelation> {
    let CreateRelationInput {
        source_id,
        source_db_id,
        target_id,
        target_db_id,
        relation_type,
        inferred,
        confidence,
    } = input;
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO entity_relations (id, source_id, source_db_id, target_id, target_db_id, relation_type, inferred, confidence, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(source_id)
    .bind(source_db_id)
    .bind(target_id)
    .bind(target_db_id)
    .bind(relation_type)
    .bind(inferred)
    .bind(confidence)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("Failed to create relation: {e}")))?;

    Ok(EntityRelation {
        id,
        source_id: source_id.to_string(),
        source_db_id: source_db_id.to_string(),
        target_id: target_id.to_string(),
        target_db_id: target_db_id.to_string(),
        relation_type: relation_type.to_string(),
        inferred,
        confidence,
        created_at: now,
    })
}

/// Delete a relation by ID.
pub async fn delete_relation(pool: &SqlitePool, relation_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM entity_relations WHERE id = ?")
        .bind(relation_id)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

/// List all relations where the given entity is source OR target.
pub async fn list_relations_for_entity(
    pool: &SqlitePool,
    entity_id: &str,
    db_id: &str,
) -> Result<Vec<EntityRelation>> {
    let rows: Vec<RelationRow> = sqlx::query_as(
        "SELECT * FROM entity_relations \
         WHERE (source_id = ? AND source_db_id = ?) OR (target_id = ? AND target_db_id = ?) \
         ORDER BY created_at ASC",
    )
    .bind(entity_id)
    .bind(db_id)
    .bind(entity_id)
    .bind(db_id)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(rows.into_iter().map(EntityRelation::from).collect())
}

/// List all relations between two databases (in either direction).
pub async fn list_relations_between_databases(
    pool: &SqlitePool,
    db_id_a: &str,
    db_id_b: &str,
) -> Result<Vec<EntityRelation>> {
    let rows: Vec<RelationRow> = sqlx::query_as(
        "SELECT * FROM entity_relations \
         WHERE (source_db_id = ? AND target_db_id = ?) OR (source_db_id = ? AND target_db_id = ?) \
         ORDER BY created_at ASC",
    )
    .bind(db_id_a)
    .bind(db_id_b)
    .bind(db_id_b)
    .bind(db_id_a)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(rows.into_iter().map(EntityRelation::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::test_helpers::setup_test_pool().await
    }

    #[tokio::test]
    async fn test_create_and_list_relations() {
        let pool = setup().await;

        let rel = create_relation(
            &pool,
            CreateRelationInput {
                source_id: "e1",
                source_db_id: "db1",
                target_id: "e2",
                target_db_id: "db2",
                relation_type: "blocks",
                inferred: false,
                confidence: Some(0.95),
            },
        )
        .await
        .unwrap();
        assert_eq!(rel.relation_type, "blocks");
        assert_eq!(rel.confidence, Some(0.95));

        // List for source entity
        let rels = list_relations_for_entity(&pool, "e1", "db1").await.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].target_id, "e2");

        // List for target entity
        let rels = list_relations_for_entity(&pool, "e2", "db2").await.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].source_id, "e1");
    }

    #[tokio::test]
    async fn test_delete_relation() {
        let pool = setup().await;

        let rel = create_relation(
            &pool,
            CreateRelationInput {
                source_id: "e1",
                source_db_id: "db1",
                target_id: "e2",
                target_db_id: "db2",
                relation_type: "related",
                inferred: false,
                confidence: None,
            },
        )
        .await
        .unwrap();
        delete_relation(&pool, &rel.id).await.unwrap();

        let rels = list_relations_for_entity(&pool, "e1", "db1").await.unwrap();
        assert!(rels.is_empty());
    }

    #[tokio::test]
    async fn test_list_relations_between_databases() {
        let pool = setup().await;

        create_relation(
            &pool,
            CreateRelationInput {
                source_id: "e1",
                source_db_id: "db1",
                target_id: "e2",
                target_db_id: "db2",
                relation_type: "related",
                inferred: false,
                confidence: None,
            },
        )
        .await
        .unwrap();
        create_relation(
            &pool,
            CreateRelationInput {
                source_id: "e3",
                source_db_id: "db2",
                target_id: "e4",
                target_db_id: "db1",
                relation_type: "depends_on",
                inferred: true,
                confidence: Some(0.8),
            },
        )
        .await
        .unwrap();
        // Unrelated
        create_relation(
            &pool,
            CreateRelationInput {
                source_id: "e5",
                source_db_id: "db3",
                target_id: "e6",
                target_db_id: "db4",
                relation_type: "related",
                inferred: false,
                confidence: None,
            },
        )
        .await
        .unwrap();

        let rels = list_relations_between_databases(&pool, "db1", "db2")
            .await
            .unwrap();
        assert_eq!(rels.len(), 2);
    }
}
