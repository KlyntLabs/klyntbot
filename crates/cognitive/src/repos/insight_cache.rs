//! Legacy repository shim — previously backed `insight_review_cache`; now
//! maps onto the new `insight_reviews` table.  Will be fully replaced by
//! `InsightReviewRepo` in a subsequent task (Task 3/9).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Row type ─────────────────────────────────────────────────────

/// Legacy row shape.  `content_hash` maps to `input_hash`; the tab columns
/// (`synthesis`, `gap_analysis`, etc.) are stored as JSON in `content`.
#[deprecated(note = "Use feature_insights::InsightReviewRow instead")]
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── Raw DB row ────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct InsightReviewsRow {
    id: String,
    note_id: String,
    version: i64,
    generated_at: String,
    content: String,
    input_hash: String,
    persona_ids: String,
    superseded_at: Option<String>,
}

#[allow(deprecated)]
impl InsightReviewsRow {
    fn into_cache_row(self) -> InsightCacheRow {
        // `content` is a JSON object with optional tab keys.
        let tabs: serde_json::Value = serde_json::from_str(&self.content)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        let tab = |k: &str| tabs.get(k).and_then(|v| v.as_str()).map(|s| s.to_owned());
        InsightCacheRow {
            id: self.id,
            note_id: self.note_id,
            content_hash: self.input_hash,
            version: self.version,
            synthesis: tab("synthesis"),
            gap_analysis: tab("gap_analysis"),
            self_assessment: tab("self_assessment"),
            concept_map: tab("concept_map"),
            perspectives: tab("perspectives"),
            persona_ids: if self.persona_ids == "[]" {
                None
            } else {
                Some(self.persona_ids)
            },
            created_at: self.generated_at.clone(),
            updated_at: self.superseded_at.unwrap_or(self.generated_at),
        }
    }
}

// ── Repository ───────────────────────────────────────────────────

#[deprecated(note = "Use feature_insights::InsightReviewRepo instead")]
#[derive(Debug, Clone)]
pub struct InsightCacheRepo {
    pool: SqlitePool,
}

#[allow(deprecated)]
impl InsightCacheRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get the most recent cache entry for a note.
    pub async fn get(&self, note_id: &str) -> Result<Option<InsightCacheRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, InsightReviewsRow>(
            "SELECT id, note_id, version, generated_at, content, input_hash, persona_ids, superseded_at \
             FROM insight_reviews WHERE note_id = ?1 ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(InsightReviewsRow::into_cache_row))
    }

    /// Return the cache entry only if the content hash matches (content hasn't changed).
    pub async fn get_if_fresh(
        &self,
        note_id: &str,
        content_hash: &str,
    ) -> Result<Option<InsightCacheRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, InsightReviewsRow>(
            "SELECT id, note_id, version, generated_at, content, input_hash, persona_ids, superseded_at \
             FROM insight_reviews WHERE note_id = ?1 AND input_hash = ?2 ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(InsightReviewsRow::into_cache_row))
    }

    /// Insert or update a cache entry.
    ///
    /// On conflict (same note + hash), bumps version and merges tab content via
    /// JSON merge.  Returns the stored row after the upsert.
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

        // Build content JSON from provided tabs.
        let mut tabs = serde_json::Map::new();
        if let Some(v) = synthesis {
            tabs.insert("synthesis".into(), serde_json::Value::String(v.to_owned()));
        }
        if let Some(v) = gap_analysis {
            tabs.insert(
                "gap_analysis".into(),
                serde_json::Value::String(v.to_owned()),
            );
        }
        if let Some(v) = self_assessment {
            tabs.insert(
                "self_assessment".into(),
                serde_json::Value::String(v.to_owned()),
            );
        }
        if let Some(v) = concept_map {
            tabs.insert(
                "concept_map".into(),
                serde_json::Value::String(v.to_owned()),
            );
        }
        if let Some(v) = perspectives {
            tabs.insert(
                "perspectives".into(),
                serde_json::Value::String(v.to_owned()),
            );
        }
        let content = serde_json::Value::Object(tabs).to_string();
        let persona_json = persona_ids.unwrap_or("[]");

        // Determine the next version for this note+hash combination.
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT version FROM insight_reviews WHERE note_id = ?1 AND input_hash = ?2 ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((existing_version,)) = existing {
            // Update existing row in-place (merge content).
            let next_version = existing_version + 1;
            sqlx::query(
                "UPDATE insight_reviews SET \
                    content = json_patch(content, ?1), \
                    version = ?2, \
                    persona_ids = COALESCE(?3, persona_ids), \
                    superseded_at = ?4 \
                 WHERE note_id = ?5 AND input_hash = ?6",
            )
            .bind(&content)
            .bind(next_version)
            .bind(persona_ids)
            .bind(&now)
            .bind(note_id)
            .bind(content_hash)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO insight_reviews \
                    (id, note_id, version, generated_at, content, input_hash, persona_ids) \
                 VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
            )
            .bind(&id)
            .bind(note_id)
            .bind(&now)
            .bind(&content)
            .bind(content_hash)
            .bind(persona_json)
            .execute(&self.pool)
            .await?;
        }

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

        // Validate tab name before building JSON patch.
        match tab_name {
            "synthesis" | "gap_analysis" | "self_assessment" | "concept_map" | "perspectives" => {}
            other => return Err(sqlx::Error::Protocol(format!("unknown tab name: {other}"))),
        }

        // Build a JSON patch: {"tab_name": "content"}
        let patch = serde_json::json!({ tab_name: content }).to_string();

        sqlx::query(
            "UPDATE insight_reviews \
             SET content = json_patch(content, ?1), superseded_at = ?2 \
             WHERE note_id = ?3 AND input_hash = ?4",
        )
        .bind(&patch)
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
#[allow(deprecated)]
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
            .upsert(
                note_id,
                hash,
                Some("synthesis text"),
                None,
                None,
                None,
                None,
                None,
            )
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
        repo.upsert(
            note_id,
            hash,
            Some("initial synthesis"),
            None,
            None,
            None,
            None,
            None,
        )
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
