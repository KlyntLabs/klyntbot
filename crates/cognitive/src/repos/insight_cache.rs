//! Repository for the `insight_review_cache` table — per-note AI insight caching.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Row type ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InsightCacheRow {
    pub id: String,
    pub note_id: String,
    pub content_hash: String,
    pub version: i64,
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<String>,
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
    pub persona_ids: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Repository ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InsightCacheRepo {
    pool: SqlitePool,
}

impl InsightCacheRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get the most recent cache entry for a note.
    pub async fn get(&self, note_id: &str) -> Result<Option<InsightCacheRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, InsightCacheRow>(
            "SELECT * FROM insight_review_cache WHERE note_id = ?1 ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(note_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Return the cache entry only if the content hash matches (content hasn't changed).
    pub async fn get_if_fresh(
        &self,
        note_id: &str,
        content_hash: &str,
    ) -> Result<Option<InsightCacheRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, InsightCacheRow>(
            "SELECT * FROM insight_review_cache WHERE note_id = ?1 AND content_hash = ?2 LIMIT 1",
        )
        .bind(note_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert or update a cache entry.
    ///
    /// On conflict, existing non-null values are preserved via `COALESCE` when
    /// the incoming value is `None`.  Returns the stored row after the upsert.
    pub async fn upsert(
        &self,
        note_id: &str,
        content_hash: &str,
        synthesis: Option<&str>,
        gap_analysis: Option<&str>,
        self_assessment: Option<&str>,
        concept_map: Option<&str>,
        perspectives: Option<&str>,
        persona_ids: Option<&str>,
    ) -> Result<InsightCacheRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO insight_review_cache
                (id, note_id, content_hash, version, synthesis, gap_analysis,
                 self_assessment, concept_map, perspectives, persona_ids, created_at, updated_at)
            VALUES
                (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
            ON CONFLICT(note_id, content_hash) DO UPDATE SET
                synthesis        = COALESCE(excluded.synthesis,        insight_review_cache.synthesis),
                gap_analysis     = COALESCE(excluded.gap_analysis,     insight_review_cache.gap_analysis),
                self_assessment  = COALESCE(excluded.self_assessment,  insight_review_cache.self_assessment),
                concept_map      = COALESCE(excluded.concept_map,      insight_review_cache.concept_map),
                perspectives     = COALESCE(excluded.perspectives,     insight_review_cache.perspectives),
                persona_ids      = COALESCE(excluded.persona_ids,      insight_review_cache.persona_ids),
                version          = insight_review_cache.version + 1,
                updated_at       = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(note_id)
        .bind(content_hash)
        .bind(synthesis)
        .bind(gap_analysis)
        .bind(self_assessment)
        .bind(concept_map)
        .bind(perspectives)
        .bind(persona_ids)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let row = self
            .get_if_fresh(note_id, content_hash)
            .await?
            .expect("row must exist after upsert");
        Ok(row)
    }

    /// Update a single tab column by name.
    ///
    /// `tab_name` must be one of: `"synthesis"`, `"gap_analysis"`,
    /// `"self_assessment"`, `"concept_map"`, `"perspectives"`.
    /// Using a match prevents SQL-injection via dynamic column names.
    pub async fn update_tab(
        &self,
        note_id: &str,
        content_hash: &str,
        tab_name: &str,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();

        let sql = match tab_name {
            "synthesis" => {
                "UPDATE insight_review_cache SET synthesis = ?1, updated_at = ?2 WHERE note_id = ?3 AND content_hash = ?4"
            }
            "gap_analysis" => {
                "UPDATE insight_review_cache SET gap_analysis = ?1, updated_at = ?2 WHERE note_id = ?3 AND content_hash = ?4"
            }
            "self_assessment" => {
                "UPDATE insight_review_cache SET self_assessment = ?1, updated_at = ?2 WHERE note_id = ?3 AND content_hash = ?4"
            }
            "concept_map" => {
                "UPDATE insight_review_cache SET concept_map = ?1, updated_at = ?2 WHERE note_id = ?3 AND content_hash = ?4"
            }
            "perspectives" => {
                "UPDATE insight_review_cache SET perspectives = ?1, updated_at = ?2 WHERE note_id = ?3 AND content_hash = ?4"
            }
            other => {
                return Err(sqlx::Error::Protocol(format!(
                    "unknown tab name: {other}"
                )))
            }
        };

        sqlx::query(sql)
            .bind(content)
            .bind(&now)
            .bind(note_id)
            .bind(content_hash)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> InsightCacheRepo {
        let pool = crate::repos::cognitive_test_pool().await;
        InsightCacheRepo::new(pool)
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let repo = setup().await;

        let note_id = "note-abc";
        let hash = "hash-001";

        let row = repo
            .upsert(note_id, hash, Some("synthesis text"), None, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(row.note_id, note_id);
        assert_eq!(row.content_hash, hash);
        assert_eq!(row.synthesis.as_deref(), Some("synthesis text"));
        assert!(row.gap_analysis.is_none());

        // get() should return the same row
        let fetched = repo.get(note_id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(
            fetched.unwrap().synthesis.as_deref(),
            Some("synthesis text")
        );

        // get_if_fresh with matching hash returns the row
        let fresh = repo.get_if_fresh(note_id, hash).await.unwrap();
        assert!(fresh.is_some());

        // get_if_fresh with wrong hash returns None
        let stale = repo.get_if_fresh(note_id, "wrong-hash").await.unwrap();
        assert!(stale.is_none());
    }

    #[tokio::test]
    async fn test_update_tab() {
        let repo = setup().await;

        let note_id = "note-xyz";
        let hash = "hash-002";

        // Create initial entry with synthesis only
        repo.upsert(note_id, hash, Some("initial synthesis"), None, None, None, None, None)
            .await
            .unwrap();

        // Update gap_analysis tab
        repo.update_tab(note_id, hash, "gap_analysis", "gap content")
            .await
            .unwrap();

        // Verify gap_analysis was updated and synthesis preserved
        let row = repo.get_if_fresh(note_id, hash).await.unwrap().unwrap();
        assert_eq!(row.gap_analysis.as_deref(), Some("gap content"));
        assert_eq!(row.synthesis.as_deref(), Some("initial synthesis"));
    }
}
