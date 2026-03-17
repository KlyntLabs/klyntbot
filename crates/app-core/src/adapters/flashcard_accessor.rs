//! Concrete FlashcardAccessor — computes FSRS-derived success rate from flashcard metrics.
//!
//! Uses raw SQL against the flashcards table since FlashcardRepo doesn't have
//! a method for querying by insight_review_id with FSRS computation.

use async_trait::async_trait;
use feature_insights::FlashcardAccessor;
use sqlx::SqlitePool;

pub struct FlashcardAccessorImpl {
    pool: SqlitePool,
}

impl FlashcardAccessorImpl {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FlashcardAccessor for FlashcardAccessorImpl {
    /// Compute average review success rate from FSRS metrics for an insight's flashcards.
    ///
    /// Success is derived from state + lapses + stability:
    /// - review + no lapses: MIN(1.0, stability / 10.0) — high stability = mastery
    /// - review + lapses: MAX(0.2, MIN(0.7, stability / 10.0)) — recovered but weaker
    /// - relearning: 0.1 — currently struggling
    /// - new/no data: 0.0
    /// Note: `_days` lookback window is not yet applied — uses all-time FSRS state.
    /// A rolling window filter (WHERE last_reviewed_at >= date('now', '-N days'))
    /// can be added when time-windowed progress tracking is needed.
    async fn review_success_rate(&self, insight_review_id: &str, _days: i64) -> f64 {
        let result: Option<f64> = sqlx::query_scalar(
            r#"
            SELECT AVG(
                CASE
                    WHEN state = 'review' AND lapses = 0 THEN
                        MIN(1.0, stability / 10.0)
                    WHEN state = 'review' AND lapses > 0 THEN
                        MAX(0.2, MIN(0.7, stability / 10.0))
                    WHEN state = 'relearning' THEN 0.1
                    ELSE 0.0
                END
            ) as success_rate
            FROM flashcards
            WHERE insight_review_id = ?1
              AND review_count > 0
            "#,
        )
        .bind(insight_review_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(None);

        result.unwrap_or(0.0)
    }
}
