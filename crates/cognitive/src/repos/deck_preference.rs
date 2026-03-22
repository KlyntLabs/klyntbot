//! Repository for the `deck_preferences` table — per-deck answer mode preferences.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

// ── Row type ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeckPreferenceRow {
    pub deck: String,
    pub answer_mode: String,
    pub updated_at: String,
}

// ── Repository ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DeckPreferenceRepo {
    pool: SqlitePool,
}

impl DeckPreferenceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get preference for a specific deck. Returns `None` if not set.
    pub async fn get(&self, deck: &str) -> Result<Option<DeckPreferenceRow>, sqlx::Error> {
        sqlx::query_as::<_, DeckPreferenceRow>(
            "SELECT * FROM deck_preferences WHERE deck = ?1",
        )
        .bind(deck)
        .fetch_optional(&self.pool)
        .await
    }

    /// Set (upsert) the answer mode for a deck.
    pub async fn set(&self, deck: &str, mode: &str) -> Result<DeckPreferenceRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO deck_preferences (deck, answer_mode, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(deck) DO UPDATE SET answer_mode = excluded.answer_mode, updated_at = excluded.updated_at
            "#,
        )
        .bind(deck)
        .bind(mode)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get(deck).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Get all deck preferences.
    pub async fn get_all(&self) -> Result<Vec<DeckPreferenceRow>, sqlx::Error> {
        sqlx::query_as::<_, DeckPreferenceRow>(
            "SELECT * FROM deck_preferences ORDER BY deck ASC",
        )
        .fetch_all(&self.pool)
        .await
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> DeckPreferenceRepo {
        let pool = crate::repos::cognitive_test_pool().await;
        DeckPreferenceRepo::new(pool)
    }

    #[tokio::test]
    async fn test_get_missing() {
        let repo = setup().await;
        assert!(repo.get("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let repo = setup().await;
        let pref = repo.set("japanese", "typed").await.unwrap();
        assert_eq!(pref.deck, "japanese");
        assert_eq!(pref.answer_mode, "typed");

        let fetched = repo.get("japanese").await.unwrap().unwrap();
        assert_eq!(fetched.answer_mode, "typed");
    }

    #[tokio::test]
    async fn test_upsert_updates_mode() {
        let repo = setup().await;
        repo.set("math", "multiple_choice").await.unwrap();
        let updated = repo.set("math", "typed").await.unwrap();
        assert_eq!(updated.answer_mode, "typed");
    }

    #[tokio::test]
    async fn test_get_all() {
        let repo = setup().await;
        repo.set("deck-a", "typed").await.unwrap();
        repo.set("deck-b", "multiple_choice").await.unwrap();

        let all = repo.get_all().await.unwrap();
        assert!(all.len() >= 2);
        assert!(all.iter().any(|p| p.deck == "deck-a"));
        assert!(all.iter().any(|p| p.deck == "deck-b"));
    }
}
