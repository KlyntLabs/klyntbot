//! Concrete FlashcardAccessor — computes FSRS-derived success rate from flashcard metrics.
//!
//! The old `insight_review_id` column was removed in the FSRS-5 migration.
//! This stub returns 0.0 until phase-3 replaces it with a `source_note_id`-based query.

use async_trait::async_trait;
use feature_insights::FlashcardAccessor;

pub struct FlashcardAccessorImpl;

impl FlashcardAccessorImpl {
    pub fn new(_pool: sqlx::SqlitePool) -> Self {
        Self
    }
}

#[async_trait]
impl FlashcardAccessor for FlashcardAccessorImpl {
    async fn review_success_rate(&self, _insight_review_id: &str, _days: i64) -> f64 {
        // TODO(phase-3): Replace with source_note_id-based query in feature-learning.
        // The old insight_review_id column was removed in the FSRS-5 migration.
        0.0
    }
}
