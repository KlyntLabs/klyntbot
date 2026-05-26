//! Repository for the `knowledge_atoms` and `knowledge_topics` tables.

use jiff::Timestamp;
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

    pub async fn create(&self, atom: &NewKnowledgeAtom) -> Result<KnowledgeAtomRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Timestamp::now().to_string();

        // Only set last_interaction_ts for active atoms.
        let interaction_ts: Option<&str> = if atom.status == "active" {
            Some(&now)
        } else {
            None
        };

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
        .bind(interaction_ts)
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

    /// Find existing atom subjects in a domain (for dedup before batch create).
    pub async fn find_existing_subjects(
        &self,
        domain: &str,
        subjects: &[String],
    ) -> Result<std::collections::HashSet<String>, sqlx::Error> {
        if subjects.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let placeholders: Vec<String> =
            (0..subjects.len()).map(|i| format!("?{}", i + 2)).collect();
        let query = format!(
            "SELECT DISTINCT subject FROM knowledge_atoms WHERE subject IN ({}) AND domain = ?1 AND status != 'archived'",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, (String,)>(&query).bind(domain);
        for s in subjects {
            q = q.bind(s);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|(s,)| s).collect())
    }

    pub async fn get(&self, id: &str) -> Result<Option<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>("SELECT * FROM knowledge_atoms WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Returns non-archived atoms for a note, ordered by salience descending.
    pub async fn list_for_note(&self, note_id: &str) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"
            SELECT * FROM knowledge_atoms
            WHERE source_note_id = ?1
              AND status != 'archived'
            ORDER BY salience DESC
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Dismiss (archive) an atom.
    pub async fn dismiss(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = Timestamp::now().to_string();
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

    /// Update retention metrics after a flashcard review. Also touches last_interaction_ts.
    pub async fn update_retention(
        &self,
        id: &str,
        retention_pct: f64,
        stability: f64,
        difficulty: f64,
    ) -> Result<(), sqlx::Error> {
        let now = Timestamp::now().to_string();
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
        let now = Timestamp::now().to_string();
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
        let now = Timestamp::now().to_string();
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
        let topic_ids: Vec<(String,)> = sqlx::query_as("SELECT id FROM knowledge_topics")
            .fetch_all(&self.pool)
            .await?;
        for (id,) in &topic_ids {
            self.update_topic_aggregates(id).await?;
        }
        Ok(())
    }

    /// List active atoms that haven't been interacted with for N days.
    pub async fn list_stale_active(
        &self,
        stale_days: i64,
    ) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        let cutoff =
            (Timestamp::now() - jiff::SignedDuration::from_secs((stale_days) * 86400)).to_string();
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"
            SELECT * FROM knowledge_atoms
            WHERE status = 'active'
              AND (last_interaction_ts IS NULL OR last_interaction_ts < ?1)
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await
    }

    /// Apply salience decay and retention update to a single atom.
    pub async fn apply_decay(
        &self,
        id: &str,
        new_salience: f64,
        new_retention_pct: f64,
    ) -> Result<(), sqlx::Error> {
        let now = Timestamp::now().to_string();
        sqlx::query(
            "UPDATE knowledge_atoms SET salience = ?2, retention_pct = ?3, updated_at = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(new_salience)
        .bind(new_retention_pct)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a single topic by id.
    pub async fn get_topic(
        &self,
        topic_id: &str,
    ) -> Result<Option<KnowledgeTopicRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeTopicRow>("SELECT * FROM knowledge_topics WHERE id = ?1")
            .bind(topic_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// List non-archived atoms belonging to a topic, ordered by salience descending.
    pub async fn list_for_topic(
        &self,
        topic_id: &str,
    ) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"
            SELECT * FROM knowledge_atoms
            WHERE topic_id = ?1
              AND status != 'archived'
            ORDER BY salience DESC
            "#,
        )
        .bind(topic_id)
        .fetch_all(&self.pool)
        .await
    }

    /// List all topics with their atom counts and avg retention (for health dashboard).
    pub async fn list_topics_with_atoms(&self) -> Result<Vec<KnowledgeTopicRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeTopicRow>(
            "SELECT * FROM knowledge_topics WHERE atom_count > 0 ORDER BY avg_retention ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Find existing non-archived atoms matching a subject in other notes (for reinforcement detection).
    pub async fn find_by_subject_across_notes(
        &self,
        subject: &str,
        exclude_note_id: &str,
    ) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"
            SELECT * FROM knowledge_atoms
            WHERE subject = ?1
              AND source_note_id != ?2
              AND status != 'archived'
            "#,
        )
        .bind(subject)
        .bind(exclude_note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Boost an atom's salience (capped at 1.0) and record a secondary source.
    pub async fn boost_salience(
        &self,
        id: &str,
        boost: f64,
        referencing_note_id: &str,
    ) -> Result<f64, sqlx::Error> {
        let now = Timestamp::now().to_string();
        let atom =
            sqlx::query_as::<_, KnowledgeAtomRow>("SELECT * FROM knowledge_atoms WHERE id = ?1")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;

        let new_salience = (atom.salience + boost).min(1.0);

        // Append to secondary_sources JSON array
        let mut sources: Vec<serde_json::Value> = atom
            .secondary_sources
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        sources.push(serde_json::json!({ "noteId": referencing_note_id, "ts": &now }));
        let sources_json = serde_json::to_string(&sources).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            "UPDATE knowledge_atoms SET salience = ?2, secondary_sources = ?3, last_interaction_ts = ?4, updated_at = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(new_salience)
        .bind(&sources_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(new_salience)
    }

    /// Batch-fetch topic names by IDs.
    pub async fn get_topic_names(
        &self,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, String>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Default::default());
        }
        let placeholders: Vec<String> = (0..ids.len()).map(|i| format!("?{}", i + 1)).collect();
        let query = format!(
            "SELECT id, name FROM knowledge_topics WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, (String, String)>(&query);
        for id in ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().collect())
    }

    /// Count atoms created since the given RFC3339 timestamp.
    pub async fn count_created_since(&self, since: &str) -> Result<i64, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_atoms WHERE created_at > ?1")
                .bind(since)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    /// List fading important atoms (retention < 0.6, importance > 0.7, active).
    pub async fn list_fading_important(
        &self,
        limit: i64,
    ) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"SELECT * FROM knowledge_atoms
               WHERE status = 'active'
                 AND retention_pct < 0.6
                 AND personal_importance > 0.7
               ORDER BY retention_pct ASC
               LIMIT ?1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Check migration status: (migrated, atom_count).
    pub async fn migration_status(&self) -> Result<(bool, usize), sqlx::Error> {
        let sentinel: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM knowledge_atoms WHERE subject = '__atoms_migration_v1__'",
        )
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
    async fn test_dismiss() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "学ぶ".to_string(),
                atom_type: "vocabulary".to_string(),
                domain: "language:ja".to_string(),
                personal_importance: 0.5,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(atom.status, "active");

        repo.dismiss(&atom.id).await.unwrap();
        let dismissed = repo.get(&atom.id).await.unwrap().unwrap();
        assert_eq!(dismissed.status, "archived");
        assert!(dismissed.archived_at.is_some());
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

    #[tokio::test]
    async fn test_list_stale_active() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        // Create an atom with old interaction timestamp
        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "old atom".to_string(),
                atom_type: "concept".to_string(),
                domain: "test".to_string(),
                personal_importance: 0.7,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        // Manually set last_interaction_ts to 30 days ago
        let old_ts = (Timestamp::now() - jiff::SignedDuration::from_secs((30) * 86400)).to_string();
        sqlx::query("UPDATE knowledge_atoms SET last_interaction_ts = ?2 WHERE id = ?1")
            .bind(&atom.id)
            .bind(&old_ts)
            .execute(&repo.pool)
            .await
            .unwrap();

        // Create a fresh atom
        repo.create(&NewKnowledgeAtom {
            subject: "fresh atom".to_string(),
            atom_type: "concept".to_string(),
            domain: "test".to_string(),
            personal_importance: 0.7,
            status: "active".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let stale = repo.list_stale_active(7).await.unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].subject, "old atom");
    }

    #[tokio::test]
    async fn test_apply_decay() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "decaying".to_string(),
                atom_type: "concept".to_string(),
                domain: "test".to_string(),
                personal_importance: 0.7,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        repo.apply_decay(&atom.id, 0.5, 0.6).await.unwrap();
        let updated = repo.get(&atom.id).await.unwrap().unwrap();
        assert!((updated.salience - 0.5).abs() < 0.01);
        assert!((updated.retention_pct - 0.6).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_find_by_subject_across_notes() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        repo.create(&NewKnowledgeAtom {
            subject: "shared concept".to_string(),
            atom_type: "concept".to_string(),
            domain: "test".to_string(),
            source_note_id: Some("note-1".to_string()),
            personal_importance: 0.7,
            status: "active".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        repo.create(&NewKnowledgeAtom {
            subject: "shared concept".to_string(),
            atom_type: "concept".to_string(),
            domain: "test".to_string(),
            source_note_id: Some("note-2".to_string()),
            personal_importance: 0.7,
            status: "active".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let found = repo
            .find_by_subject_across_notes("shared concept", "note-1")
            .await
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source_note_id.as_deref(), Some("note-2"));
    }

    #[tokio::test]
    async fn test_boost_salience() {
        let pool = cognitive_test_pool().await;
        let repo = KnowledgeAtomRepo::new(pool);

        let atom = repo
            .create(&NewKnowledgeAtom {
                subject: "boostable".to_string(),
                atom_type: "concept".to_string(),
                domain: "test".to_string(),
                source_note_id: Some("note-1".to_string()),
                personal_importance: 0.7,
                status: "active".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        // Set initial salience to 0.5
        repo.apply_decay(&atom.id, 0.5, atom.retention_pct)
            .await
            .unwrap();

        let new_salience = repo.boost_salience(&atom.id, 0.3, "note-2").await.unwrap();
        assert!((new_salience - 0.8).abs() < 0.01);

        let updated = repo.get(&atom.id).await.unwrap().unwrap();
        assert!(updated.secondary_sources.is_some());
        let sources: Vec<serde_json::Value> =
            serde_json::from_str(updated.secondary_sources.as_deref().unwrap()).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["noteId"], "note-2");
    }
}
