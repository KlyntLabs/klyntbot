//! Repository for the `flashcards` table — FSRS-based spaced-repetition scheduling.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Card type ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    MultipleChoice,
    ShortAnswer,
}

impl std::fmt::Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardType::MultipleChoice => write!(f, "multiple_choice"),
            CardType::ShortAnswer => write!(f, "short_answer"),
        }
    }
}

// ── Review quality ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ReviewQuality {
    Again,
    Hard,
    Good,
    Easy,
}

// ── Input / row types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NewFlashcard {
    pub source_note_id: Option<String>,
    pub insight_review_id: Option<String>,
    pub deck: String,
    pub question: String,
    pub answer: String,
    pub card_type: CardType,
    pub choices: Option<serde_json::Value>,
    pub stability: f64,
    pub difficulty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FlashcardRow {
    pub id: String,
    pub source_note_id: Option<String>,
    pub source_session_id: Option<String>,
    pub insight_review_id: Option<String>,
    pub deck: String,
    pub question: String,
    pub answer: String,
    pub card_type: String,
    pub choices: Option<String>,
    pub stability: f64,
    pub difficulty: f64,
    pub due_at: Option<String>,
    pub last_reviewed_at: Option<String>,
    pub review_count: i64,
    pub lapses: i64,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeckSummary {
    pub name: String,
    pub card_count: i64,
    pub due_count: i64,
}

// ── Repository ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FlashcardRepo {
    pool: SqlitePool,
}

impl FlashcardRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a batch of new flashcards. All cards are immediately due (`due_at = now`).
    pub async fn create_batch(&self, cards: Vec<NewFlashcard>) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let mut rows = Vec::with_capacity(cards.len());

        for card in cards {
            let id = Uuid::new_v4().to_string();
            let card_type_str = card.card_type.to_string();
            let choices_str = card
                .choices
                .as_ref()
                .map(|v| v.to_string());

            sqlx::query(
                r#"
                INSERT INTO flashcards
                    (id, source_note_id, source_session_id, insight_review_id,
                     deck, question, answer, card_type, choices,
                     stability, difficulty, due_at, last_reviewed_at,
                     review_count, lapses, state, created_at, updated_at)
                VALUES
                    (?1, ?2, NULL, ?3,
                     ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, NULL,
                     0, 0, 'new', ?12, ?12)
                "#,
            )
            .bind(&id)
            .bind(&card.source_note_id)
            .bind(&card.insight_review_id)
            .bind(&card.deck)
            .bind(&card.question)
            .bind(&card.answer)
            .bind(&card_type_str)
            .bind(&choices_str)
            .bind(card.stability)
            .bind(card.difficulty)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;

            let row = sqlx::query_as::<_, FlashcardRow>(
                "SELECT * FROM flashcards WHERE id = ?1",
            )
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;

            rows.push(row);
        }

        Ok(rows)
    }

    /// Fetch cards in `deck` that are due for review (due_at <= now or NULL).
    pub async fn get_due_cards(&self, deck: &str, limit: i64) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE deck = ?1 AND (due_at IS NULL OR due_at <= ?2)
            ORDER BY due_at ASC
            LIMIT ?3
            "#,
        )
        .bind(deck)
        .bind(&now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Record a review for a card and update FSRS scheduling fields.
    pub async fn record_review(&self, id: &str, quality: ReviewQuality) -> Result<FlashcardRow, sqlx::Error> {
        let card = sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        let current_stability = card.stability;
        let current_lapses = card.lapses;

        let (new_stability, new_state, new_lapses) = match quality {
            ReviewQuality::Again => {
                let s = (current_stability / 2.0).max(0.1);
                (s, "relearning".to_string(), current_lapses + 1)
            }
            ReviewQuality::Hard => {
                (current_stability, "review".to_string(), current_lapses)
            }
            ReviewQuality::Good => {
                let s = crate::services::decay::update_stability(current_stability, true, 90.0);
                (s, "review".to_string(), current_lapses)
            }
            ReviewQuality::Easy => {
                let grown =
                    crate::services::decay::update_stability(current_stability, true, 90.0);
                let s = current_stability + (grown - current_stability) * 1.3;
                (s, "review".to_string(), current_lapses)
            }
        };

        let now = Utc::now();
        let stability_secs = (new_stability * 86_400.0) as i64;
        let next_due = now + Duration::seconds(stability_secs);
        let now_str = now.to_rfc3339();
        let due_str = next_due.to_rfc3339();

        sqlx::query(
            r#"
            UPDATE flashcards
            SET stability = ?1,
                due_at = ?2,
                last_reviewed_at = ?3,
                review_count = review_count + 1,
                state = ?4,
                lapses = ?5,
                updated_at = ?3
            WHERE id = ?6
            "#,
        )
        .bind(new_stability)
        .bind(&due_str)
        .bind(&now_str)
        .bind(&new_state)
        .bind(new_lapses)
        .bind(id)
        .execute(&self.pool)
        .await?;

        let updated = sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        Ok(updated)
    }

    /// List all cards for a given source note, newest first.
    pub async fn list_by_note(&self, note_id: &str) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, FlashcardRow>(
            "SELECT * FROM flashcards WHERE source_note_id = ?1 ORDER BY created_at DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Return a summary of every deck (card count + due count).
    pub async fn list_decks(&self) -> Result<Vec<DeckSummary>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();

        #[derive(sqlx::FromRow)]
        struct Row {
            name: String,
            card_count: i64,
            due_count: i64,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT
                deck AS name,
                COUNT(*) AS card_count,
                SUM(CASE WHEN due_at IS NULL OR due_at <= ?1 THEN 1 ELSE 0 END) AS due_count
            FROM flashcards
            GROUP BY deck
            "#,
        )
        .bind(&now)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DeckSummary {
                name: r.name,
                card_count: r.card_count,
                due_count: r.due_count,
            })
            .collect())
    }

    /// Delete all cards in a deck. Returns the number of rows deleted.
    pub async fn delete_deck(&self, deck: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM flashcards WHERE deck = ?1")
            .bind(deck)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (SqlitePool, FlashcardRepo) {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = FlashcardRepo::new(pool.clone());
        (pool, repo)
    }

    fn sample_card(deck: &str, source_note_id: Option<&str>) -> NewFlashcard {
        NewFlashcard {
            source_note_id: source_note_id.map(|s| s.to_string()),
            insight_review_id: None,
            deck: deck.to_string(),
            question: "What is 2 + 2?".to_string(),
            answer: "4".to_string(),
            card_type: CardType::ShortAnswer,
            choices: None,
            stability: 1.0,
            difficulty: 0.5,
        }
    }

    #[tokio::test]
    async fn test_create_batch_and_list_by_note() {
        let (_pool, repo) = setup().await;

        let cards = vec![
            sample_card("math", Some("note-1")),
            sample_card("math", Some("note-1")),
        ];

        let created = repo.create_batch(cards).await.unwrap();
        assert_eq!(created.len(), 2);

        let by_note = repo.list_by_note("note-1").await.unwrap();
        assert_eq!(by_note.len(), 2);
    }

    #[tokio::test]
    async fn test_get_due_cards() {
        let (_pool, repo) = setup().await;

        let cards = vec![sample_card("science", Some("note-2"))];
        repo.create_batch(cards).await.unwrap();

        let due = repo.get_due_cards("science", 10).await.unwrap();
        assert_eq!(due.len(), 1, "Newly created card should be immediately due");
    }

    #[tokio::test]
    async fn test_record_review_updates_fsrs() {
        let (_pool, repo) = setup().await;

        let cards = vec![sample_card("history", Some("note-3"))];
        let created = repo.create_batch(cards).await.unwrap();
        let card_id = &created[0].id;
        let original_stability = created[0].stability;

        let updated = repo.record_review(card_id, ReviewQuality::Good).await.unwrap();

        assert!(
            updated.stability > original_stability,
            "Stability should increase after a Good review"
        );
        assert_eq!(updated.state, "review");
        assert_eq!(updated.review_count, 1);
    }

    #[tokio::test]
    async fn test_list_decks() {
        let (_pool, repo) = setup().await;

        let cards = vec![
            sample_card("test-deck", Some("note-4")),
            sample_card("test-deck", Some("note-4")),
        ];
        repo.create_batch(cards).await.unwrap();

        let decks = repo.list_decks().await.unwrap();
        let target = decks.iter().find(|d| d.name == "test-deck");
        assert!(target.is_some(), "test-deck should appear in list_decks");
        assert_eq!(target.unwrap().card_count, 2);
    }
}
