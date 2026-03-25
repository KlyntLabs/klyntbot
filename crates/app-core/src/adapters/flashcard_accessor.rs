//! Concrete FlashcardAccessor — computes FSRS-derived success rate from flashcard review data.
//!
//! Resolves insight_review_id → note_id via `insight_reviews`, then queries
//! `review_log` joined through `flashcards.source_note_id` for the last N days.
//! Success = rating >= 3 (Good/Easy), consistent with retention_history.rs.

use async_trait::async_trait;
use feature_insights::FlashcardAccessor;

pub struct FlashcardAccessorImpl {
    db: sqlx::SqlitePool,
}

impl FlashcardAccessorImpl {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { db: pool }
    }
}

#[async_trait]
impl FlashcardAccessor for FlashcardAccessorImpl {
    async fn review_success_rate(&self, insight_review_id: &str, days: i64) -> f64 {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query_scalar::<_, f64>(
            "SELECT AVG(CASE WHEN rl.rating >= 3 THEN 1.0 ELSE 0.0 END)
             FROM review_log rl
             JOIN flashcards fc ON fc.id = rl.card_id
             WHERE fc.source_note_id = (
                 SELECT note_id FROM insight_reviews WHERE id = ?1
             )
             AND rl.reviewed_at > ?2",
        )
        .bind(insight_review_id)
        .bind(&cutoff_str)
        .fetch_optional(&self.db)
        .await;

        match result {
            Ok(Some(rate)) => rate,
            _ => 0.0,
        }
    }
}
