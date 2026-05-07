use sqlx::SqlitePool;

use crate::types::Narrative;

#[derive(Debug, Clone)]
pub struct NarrativeRepo {
    pool: SqlitePool,
}

impl NarrativeRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, narrative: &Narrative) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_narratives (id, narrative_date, narrative_text, key_moments, sentiment, total_focus_mins, total_meeting_mins, total_break_mins, quality_score, top_categories, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
               ON CONFLICT(narrative_date) DO UPDATE SET
                   narrative_text = excluded.narrative_text,
                   key_moments = excluded.key_moments,
                   sentiment = excluded.sentiment,
                   total_focus_mins = excluded.total_focus_mins,
                   total_meeting_mins = excluded.total_meeting_mins,
                   total_break_mins = excluded.total_break_mins,
                   quality_score = excluded.quality_score,
                   top_categories = excluded.top_categories"#,
        )
        .bind(&narrative.id)
        .bind(&narrative.narrative_date)
        .bind(&narrative.narrative_text)
        .bind(&narrative.key_moments)
        .bind(&narrative.sentiment)
        .bind(narrative.total_focus_mins)
        .bind(narrative.total_meeting_mins)
        .bind(narrative.total_break_mins)
        .bind(narrative.quality_score)
        .bind(&narrative.top_categories)
        .bind(&narrative.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_by_date(&self, date: &str) -> common::Result<Option<Narrative>> {
        let row = sqlx::query_as::<_, Narrative>(
            "SELECT * FROM productivity_narratives WHERE narrative_date = ?1",
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn list_range(&self, start: &str, end: &str) -> common::Result<Vec<Narrative>> {
        let rows = sqlx::query_as::<_, Narrative>(
            "SELECT * FROM productivity_narratives WHERE narrative_date >= ?1 AND narrative_date < ?2 ORDER BY narrative_date DESC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn sample_narrative(id: &str, date: &str) -> Narrative {
        Narrative {
            id: id.to_string(),
            narrative_date: date.to_string(),
            narrative_text: "You had a productive morning focused on coding.".to_string(),
            key_moments: Some(
                r#"[{"time":"09:30","description":"Deep work block","momentType":"deep_work"}]"#
                    .to_string(),
            ),
            sentiment: Some("positive".to_string()),
            total_focus_mins: 180,
            total_meeting_mins: 30,
            total_break_mins: 45,
            quality_score: Some(0.85),
            top_categories: Some(r#"["coding","documentation"]"#.to_string()),
            created_at: "2026-03-09T18:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get_by_date() {
        let pool = setup_pool().await;
        let repo = NarrativeRepo::new(pool);

        let narrative = sample_narrative("narr-1", "2026-03-09");
        repo.upsert(&narrative).await.unwrap();

        let fetched = repo.get_by_date("2026-03-09").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.total_focus_mins, 180);
        assert_eq!(fetched.sentiment.as_deref(), Some("positive"));

        // Upsert with updated text
        let updated = Narrative {
            narrative_text: "Updated narrative for the day.".to_string(),
            total_focus_mins: 200,
            ..narrative
        };
        repo.upsert(&updated).await.unwrap();
        let fetched = repo.get_by_date("2026-03-09").await.unwrap().unwrap();
        assert_eq!(fetched.narrative_text, "Updated narrative for the day.");
        assert_eq!(fetched.total_focus_mins, 200);
    }

    #[tokio::test]
    async fn test_list_range() {
        let pool = setup_pool().await;
        let repo = NarrativeRepo::new(pool);

        let n1 = sample_narrative("narr-r1", "2026-03-07");
        let n2 = sample_narrative("narr-r2", "2026-03-08");
        let n3 = sample_narrative("narr-r3", "2026-03-09");

        repo.upsert(&n1).await.unwrap();
        repo.upsert(&n2).await.unwrap();
        repo.upsert(&n3).await.unwrap();

        let results = repo.list_range("2026-03-07", "2026-03-09").await.unwrap();
        assert_eq!(results.len(), 2);
        // Ordered DESC
        assert_eq!(results[0].narrative_date, "2026-03-08");
        assert_eq!(results[1].narrative_date, "2026-03-07");
    }
}
