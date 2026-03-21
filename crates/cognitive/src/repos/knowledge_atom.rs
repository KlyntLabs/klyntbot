//! Repository for the `knowledge_atoms` and `knowledge_topics` tables.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Row types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KnowledgeTopicRow {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub atom_count: i64,
    pub avg_retention: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KnowledgeAtomRow {
    pub id: String,
    pub subject: String,
    pub atom_type: String,
    pub domain: String,
    pub source_note_id: Option<String>,
    pub source_range: Option<String>,
    pub source_context: Option<String>,
    pub secondary_sources: Option<String>,
    pub semantic_fact_id: Option<String>,
    pub retention_pct: f64,
    pub stability: f64,
    pub difficulty: f64,
    pub personal_importance: f64,
    pub status: String,
    pub salience: f64,
    pub last_interaction_ts: Option<String>,
    pub archived_at: Option<String>,
    pub metadata: Option<String>,
    pub topic_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Input type ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct NewKnowledgeAtom {
    pub subject: String,
    pub atom_type: String,
    pub domain: String,
    pub source_note_id: Option<String>,
    pub source_range: Option<String>,
    pub source_context: Option<String>,
    pub secondary_sources: Option<String>,
    pub semantic_fact_id: Option<String>,
    pub personal_importance: f64,
    pub status: String,
    pub metadata: Option<String>,
    pub topic_id: Option<String>,
}

// ── Repository ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KnowledgeAtomRepo {
    pool: SqlitePool,
}

impl KnowledgeAtomRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(
        &self,
        atom: &NewKnowledgeAtom,
    ) -> Result<KnowledgeAtomRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO knowledge_atoms
                (id, subject, atom_type, domain,
                 source_note_id, source_range, source_context, secondary_sources,
                 semantic_fact_id, retention_pct, stability, difficulty,
                 personal_importance, status, salience, last_interaction_ts,
                 metadata, topic_id, created_at, updated_at)
            VALUES
                (?1, ?2, ?3, ?4,
                 ?5, ?6, ?7, ?8,
                 ?9, 1.0, 1.0, 5.0,
                 ?10, ?11, 1.0, ?12,
                 ?13, ?14, ?15, ?15)
            "#,
        )
        .bind(&id)
        .bind(&atom.subject)
        .bind(&atom.atom_type)
        .bind(&atom.domain)
        .bind(&atom.source_note_id)
        .bind(&atom.source_range)
        .bind(&atom.source_context)
        .bind(&atom.secondary_sources)
        .bind(&atom.semantic_fact_id)
        .bind(atom.personal_importance)
        .bind(&atom.status)
        .bind(&now)
        .bind(&atom.metadata)
        .bind(&atom.topic_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, KnowledgeAtomRow>("SELECT * FROM knowledge_atoms WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn create_batch(
        &self,
        atoms: Vec<NewKnowledgeAtom>,
    ) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        let mut rows = Vec::with_capacity(atoms.len());
        for atom in &atoms {
            rows.push(self.create(atom).await?);
        }
        Ok(rows)
    }

    pub async fn get(&self, id: &str) -> Result<Option<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>("SELECT * FROM knowledge_atoms WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Returns active + suggested atoms for a note, ordered by salience descending.
    pub async fn list_for_note(
        &self,
        note_id: &str,
    ) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"
            SELECT * FROM knowledge_atoms
            WHERE source_note_id = ?1
              AND status != 'archived'
            ORDER BY
                CASE status WHEN 'active' THEN 0 ELSE 1 END,
                salience DESC
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Accept a suggested atom: set status="active", importance, last_interaction_ts.
    pub async fn accept(
        &self,
        id: &str,
        personal_importance: f64,
    ) -> Result<KnowledgeAtomRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE knowledge_atoms
            SET status = 'active',
                personal_importance = ?2,
                last_interaction_ts = ?3,
                updated_at = ?3
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(personal_importance)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, KnowledgeAtomRow>("SELECT * FROM knowledge_atoms WHERE id = ?1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    /// Dismiss (archive) an atom.
    pub async fn dismiss(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE knowledge_atoms
            SET status = 'archived',
                archived_at = ?2,
                updated_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Restore an archived atom to active status.
    pub async fn restore(&self, id: &str) -> Result<KnowledgeAtomRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE knowledge_atoms
            SET status = 'active',
                archived_at = NULL,
                salience = 0.5,
                updated_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, KnowledgeAtomRow>("SELECT * FROM knowledge_atoms WHERE id = ?1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    /// Update retention metrics after a flashcard review. Also touches last_interaction_ts.
    pub async fn update_retention(
        &self,
        id: &str,
        retention_pct: f64,
        stability: f64,
        difficulty: f64,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE knowledge_atoms
            SET retention_pct = ?2,
                stability = ?3,
                difficulty = ?4,
                last_interaction_ts = ?5,
                updated_at = ?5
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(retention_pct)
        .bind(stability)
        .bind(difficulty)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Touch: update last_interaction_ts to now.
    pub async fn touch(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE knowledge_atoms SET last_interaction_ts = ?2, updated_at = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get or create a topic by name + domain.
    pub async fn get_or_create_topic(
        &self,
        name: &str,
        domain: &str,
    ) -> Result<KnowledgeTopicRow, sqlx::Error> {
        if let Some(existing) = sqlx::query_as::<_, KnowledgeTopicRow>(
            "SELECT * FROM knowledge_topics WHERE name = ?1 AND domain = ?2",
        )
        .bind(name)
        .bind(domain)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(existing);
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO knowledge_topics (id, name, domain, atom_count, avg_retention, created_at) VALUES (?1, ?2, ?3, 0, 1.0, ?4)",
        )
        .bind(&id)
        .bind(name)
        .bind(domain)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, KnowledgeTopicRow>("SELECT * FROM knowledge_topics WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
    }

    /// Recompute atom_count + avg_retention for a single topic.
    pub async fn update_topic_aggregates(&self, topic_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE knowledge_topics
            SET atom_count = (
                    SELECT COUNT(*) FROM knowledge_atoms
                    WHERE topic_id = ?1 AND status = 'active'
                ),
                avg_retention = COALESCE(
                    (SELECT AVG(retention_pct) FROM knowledge_atoms
                     WHERE topic_id = ?1 AND status = 'active'),
                    1.0
                )
            WHERE id = ?1
            "#,
        )
        .bind(topic_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Recompute aggregates for all topics.
    pub async fn update_all_topic_aggregates(&self) -> Result<(), sqlx::Error> {
        let topic_ids: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM knowledge_topics")
                .fetch_all(&self.pool)
                .await?;
        for (id,) in &topic_ids {
            self.update_topic_aggregates(id).await?;
        }
        Ok(())
    }

    /// Check migration status: (migrated, atom_count).
    pub async fn migration_status(&self) -> Result<(bool, usize), sqlx::Error> {
        let sentinel: Option<(String,)> =
            sqlx::query_as("SELECT id FROM knowledge_atoms WHERE subject = '__atoms_migration_v1__'")
                .fetch_optional(&self.pool)
                .await?;

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM knowledge_atoms WHERE subject != '__atoms_migration_v1__'",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((sentinel.is_some(), count.0 as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_create_and_get_atom() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "食べる".to_string(),
                atom_type: "vocabulary".to_string(),
                domain: "language:ja".to_string(),
                source_note_id: Some("note-1".to_string()),
                personal_importance: 0.8,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(atom.subject, "食べる");
        assert_eq!(atom.atom_type, "vocabulary");
        assert_eq!(atom.status, "active");

        let fetched = repo.get(&atom.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, atom.id);
        assert!((fetched.personal_importance - 0.8).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_list_for_note() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        for word in ["猫", "犬", "鳥"] {
            repo.create(&NewKnowledgeAtom {
                subject: word.to_string(),
                atom_type: "vocabulary".to_string(),
                domain: "language:ja".to_string(),
                source_note_id: Some("note-1".to_string()),
                personal_importance: 0.7,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
        }

        // Different note
        repo.create(&NewKnowledgeAtom {
            subject: "魚".to_string(),
            atom_type: "vocabulary".to_string(),
            domain: "language:ja".to_string(),
            source_note_id: Some("note-2".to_string()),
            personal_importance: 0.7,
            status: "active".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let for_note1 = repo.list_for_note("note-1").await.unwrap();
        assert_eq!(for_note1.len(), 3);
    }

    #[tokio::test]
    async fn test_accept_and_dismiss() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "学ぶ".to_string(),
                atom_type: "vocabulary".to_string(),
                domain: "language:ja".to_string(),
                personal_importance: 0.5,
                status: "suggested".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(atom.status, "suggested");

        let accepted = repo.accept(&atom.id, 0.9).await.unwrap();
        assert_eq!(accepted.status, "active");
        assert!((accepted.personal_importance - 0.9).abs() < f64::EPSILON);
        assert!(accepted.last_interaction_ts.is_some());

        repo.dismiss(&atom.id).await.unwrap();
        let dismissed = repo.get(&atom.id).await.unwrap().unwrap();
        assert_eq!(dismissed.status, "archived");
        assert!(dismissed.archived_at.is_some());
    }

    #[tokio::test]
    async fn test_restore_archived_atom() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "走る".to_string(),
                atom_type: "vocabulary".to_string(),
                domain: "language:ja".to_string(),
                personal_importance: 0.7,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        repo.dismiss(&atom.id).await.unwrap();
        let restored = repo.restore(&atom.id).await.unwrap();
        assert_eq!(restored.status, "active");
        assert!(restored.archived_at.is_none());
        assert!((restored.salience - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_get_or_create_topic() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let topic1 = repo
            .get_or_create_topic("Japanese Vocab", "language:ja")
            .await
            .unwrap();
        let topic2 = repo
            .get_or_create_topic("Japanese Vocab", "language:ja")
            .await
            .unwrap();

        assert_eq!(topic1.id, topic2.id);
        assert_eq!(topic1.name, "Japanese Vocab");
    }
}
