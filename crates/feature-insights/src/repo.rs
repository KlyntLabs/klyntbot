//! Repository for the `insight_reviews` table — versioned insight storage.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::types::{InsightReviewRow, ScopeConfig};

#[derive(Debug, Clone)]
pub struct InsightReviewRepo {
    pool: SqlitePool,
}

impl InsightReviewRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new insight review version. Automatically sets version = max + 1 for the note.
    pub async fn insert(
        &self,
        note_id: &str,
        content: &str,
        input_hash: &str,
        scope_config: &ScopeConfig,
        persona_ids: &[String],
        parent_insight_id: Option<&str>,
    ) -> Result<InsightReviewRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // Get next version number for this note
        let max_version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM insight_reviews WHERE note_id = ?1")
                .bind(note_id)
                .fetch_one(&self.pool)
                .await?;
        let version = max_version.unwrap_or(0) + 1;

        let scope_json =
            serde_json::to_string(scope_config).unwrap_or_else(|_| "{}".to_string());
        let persona_json =
            serde_json::to_string(persona_ids).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO insight_reviews
                (id, note_id, version, generated_at, content, input_hash,
                 scope_config, persona_ids, parent_insight_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&id)
        .bind(note_id)
        .bind(version)
        .bind(&now)
        .bind(content)
        .bind(input_hash)
        .bind(&scope_json)
        .bind(&persona_json)
        .bind(parent_insight_id)
        .execute(&self.pool)
        .await?;

        self.get(&id).await.map(|opt| opt.expect("just inserted"))
    }

    /// Get a single insight review by ID.
    pub async fn get(&self, id: &str) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>("SELECT * FROM insight_reviews WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Get the latest (highest version) insight for a note.
    pub async fn get_latest(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE note_id = ?1 AND superseded_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get insight by note_id and exact input_hash (cache hit check).
    pub async fn get_by_hash(
        &self,
        note_id: &str,
        input_hash: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE note_id = ?1 AND input_hash = ?2 AND superseded_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .bind(input_hash)
        .fetch_optional(&self.pool)
        .await
    }

    /// List all versions for a note, newest first.
    pub async fn list_versions(
        &self,
        note_id: &str,
    ) -> Result<Vec<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE note_id = ?1 ORDER BY version DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Soft-archive an insight version (mark as superseded).
    pub async fn supersede(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE insight_reviews SET superseded_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update content for a specific tab (used by regenerate_tab).
    pub async fn update_content(&self, id: &str, content: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE insight_reviews SET content = ?1 WHERE id = ?2")
            .bind(content)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::migrate!("../storage/migrations")
            .run(&pool)
            .await
            .unwrap();
        let migrations = cognitive::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(&pool, &migrations)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);

        let scope = ScopeConfig::default();
        let row = repo
            .insert(
                "note-1",
                r#"{"synthesis":"hello"}"#,
                "hash-abc",
                &scope,
                &[],
                None,
            )
            .await
            .unwrap();

        assert_eq!(row.note_id, "note-1");
        assert_eq!(row.version, 1);
        assert_eq!(row.input_hash, "hash-abc");
        assert!(row.parent_insight_id.is_none());

        let fetched = repo.get(&row.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, row.id);
    }

    #[tokio::test]
    async fn test_version_auto_increment() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        let v1 = repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();
        assert_eq!(v1.version, 1);

        let v2 = repo
            .insert("note-1", r#"{"synthesis":"v2"}"#, "hash-2", &scope, &[], None)
            .await
            .unwrap();
        assert_eq!(v2.version, 2);

        // Different note starts at 1
        let other = repo
            .insert("note-2", r#"{"synthesis":"v1"}"#, "hash-3", &scope, &[], None)
            .await
            .unwrap();
        assert_eq!(other.version, 1);
    }

    #[tokio::test]
    async fn test_get_latest_and_list_versions() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        repo.insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();
        repo.insert("note-1", r#"{"synthesis":"v2"}"#, "hash-2", &scope, &[], None)
            .await
            .unwrap();

        let latest = repo.get_latest("note-1").await.unwrap().unwrap();
        assert_eq!(latest.version, 2);

        let versions = repo.list_versions("note-1").await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, 2); // newest first
        assert_eq!(versions[1].version, 1);
    }

    #[tokio::test]
    async fn test_get_by_hash() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        repo.insert(
            "note-1",
            r#"{"synthesis":"v1"}"#,
            "hash-abc",
            &scope,
            &[],
            None,
        )
        .await
        .unwrap();

        let found = repo.get_by_hash("note-1", "hash-abc").await.unwrap();
        assert!(found.is_some());

        let not_found = repo.get_by_hash("note-1", "wrong-hash").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_supersede_hides_from_latest() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        let v1 = repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();

        repo.supersede(&v1.id).await.unwrap();

        // get_latest should not find superseded insights
        let latest = repo.get_latest("note-1").await.unwrap();
        assert!(latest.is_none());

        // but list_versions still includes them
        let versions = repo.list_versions("note-1").await.unwrap();
        assert_eq!(versions.len(), 1);
    }
}
