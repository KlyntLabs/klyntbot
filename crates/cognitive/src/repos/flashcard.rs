//! Repository for the `flashcards` table — FSRS-5 spaced-repetition scheduling.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Card type ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Basic,
    Cloze,
    Vocabulary,
    Typed,
    ImageOcclusion,
}

impl std::fmt::Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardType::Basic => write!(f, "basic"),
            CardType::Cloze => write!(f, "cloze"),
            CardType::Vocabulary => write!(f, "vocabulary"),
            CardType::Typed => write!(f, "typed"),
            CardType::ImageOcclusion => write!(f, "image_occlusion"),
        }
    }
}

impl CardType {
    pub fn parse(s: &str) -> Self {
        match s {
            "cloze" => CardType::Cloze,
            "vocabulary" => CardType::Vocabulary,
            "typed" => CardType::Typed,
            "image_occlusion" => CardType::ImageOcclusion,
            _ => CardType::Basic,
        }
    }
}

// ── Review quality ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum ReviewQuality {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

// ── Input / row types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NewFlashcard {
    pub source_note_id: Option<String>,
    pub source_context: Option<String>,
    pub atom_id: Option<String>,
    pub deck: String,
    pub front: String,
    pub back: String,
    pub card_type: CardType,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
    pub image_data: Option<serde_json::Value>,
    pub tags: Vec<String>,
    pub stability: f64,
    pub difficulty: f64,
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FlashcardRow {
    pub id: String,
    pub source_note_id: Option<String>,
    pub source_context: Option<String>,
    pub atom_id: Option<String>,
    pub deck: String,
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub cloze_data: Option<String>,
    pub vocab_data: Option<String>,
    pub image_data: Option<String>,
    pub tags: String,
    pub stability: f64,
    pub difficulty: f64,
    pub due_at: Option<String>,
    pub last_reviewed_at: Option<String>,
    pub review_count: i64,
    pub lapses: i64,
    pub state: String,
    pub suspended: i64,
    pub recall_speed_ms: Option<i64>,
    pub back_embedding_updated_at: Option<String>,
    pub preferred_mode: Option<String>,
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<String>,
    pub card_distractors: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DeckSummary {
    pub name: String,
    pub card_count: i64,
    pub due_count: i64,
}

/// Entry written to review_log after every review.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ReviewLogEntry {
    pub id: String,
    pub card_id: String,
    pub rating: i64,
    pub elapsed_days: f64,
    pub scheduled_days: f64,
    pub recall_speed_ms: Option<i64>,
    pub state: String,
    pub reviewed_at: String,
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

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert a batch of new flashcards. All cards are immediately due.
    pub async fn create_batch(
        &self,
        cards: Vec<NewFlashcard>,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let mut rows = Vec::with_capacity(cards.len());

        for card in cards {
            let id = Uuid::new_v4().to_string();
            let card_type_str = card.card_type.to_string();
            let cloze_str = card.cloze_data.as_ref().map(|v| v.to_string());
            let vocab_str = card.vocab_data.as_ref().map(|v| v.to_string());
            let image_str = card.image_data.as_ref().map(|v| v.to_string());
            let tags_str = serde_json::to_string(&card.tags).unwrap_or_else(|_| "[]".to_string());

            sqlx::query(
                r#"
                INSERT INTO flashcards
                    (id, source_note_id, source_context, atom_id,
                     deck, front, back, card_type,
                     cloze_data, vocab_data, image_data, tags,
                     stability, difficulty, due_at, last_reviewed_at,
                     review_count, lapses, state, suspended, recall_speed_ms,
                     back_embedding_updated_at, preferred_mode,
                     difficulty_estimate, prerequisite_concepts, card_distractors,
                     created_at, updated_at)
                VALUES
                    (?1, ?2, ?3, ?4,
                     ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12,
                     ?13, ?14, ?15, NULL,
                     0, 0, 'new', 0, NULL,
                     NULL, NULL,
                     ?16, ?17, NULL,
                     ?18, ?18)
                "#,
            )
            .bind(&id)
            .bind(&card.source_note_id)
            .bind(&card.source_context)
            .bind(&card.atom_id)
            .bind(&card.deck)
            .bind(&card.front)
            .bind(&card.back)
            .bind(&card_type_str)
            .bind(&cloze_str)
            .bind(&vocab_str)
            .bind(&image_str)
            .bind(&tags_str)
            .bind(card.stability)
            .bind(card.difficulty)
            .bind(&now)
            .bind(card.difficulty_estimate)
            .bind(&card.prerequisite_concepts)
            .bind(&now)
            .execute(&self.pool)
            .await?;

            let row = sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
                .bind(&id)
                .fetch_one(&self.pool)
                .await?;

            rows.push(row);
        }

        Ok(rows)
    }

    /// Get the next review card for an atom: due card first, fallback to most recent.
    pub async fn next_for_atom(&self, atom_id: &str) -> Result<Option<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        // Single query: prioritize due cards, then fall back to most recently created
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE atom_id = ?1
              AND suspended = 0
            ORDER BY
                CASE WHEN due_at IS NULL OR due_at <= ?2 THEN 0 ELSE 1 END,
                due_at ASC,
                created_at DESC
            LIMIT 1
            "#,
        )
        .bind(atom_id)
        .bind(&now)
        .fetch_optional(&self.pool)
        .await
    }

    /// Find existing card fronts in a deck (for dedup before batch create).
    pub async fn find_existing_fronts(
        &self,
        deck: &str,
        fronts: &[String],
    ) -> Result<std::collections::HashSet<String>, sqlx::Error> {
        if fronts.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let placeholders: Vec<String> = (0..fronts.len()).map(|i| format!("?{}", i + 2)).collect();
        let query = format!(
            "SELECT DISTINCT front FROM flashcards WHERE front IN ({}) AND deck = ?1",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, (String,)>(&query).bind(deck);
        for f in fronts {
            q = q.bind(f);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|(f,)| f).collect())
    }

    /// Fetch cards in `deck` that are due for review.
    pub async fn get_due_cards(
        &self,
        deck: &str,
        limit: i64,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE deck = ?1
              AND suspended = 0
              AND (due_at IS NULL OR due_at <= ?2)
            ORDER BY due_at ASC
            LIMIT ?3
            "#,
        )
        .bind(deck)
        .bind(&now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Record a review using FSRS-5 scheduling. Logs to review_log.
    pub async fn record_review(
        &self,
        id: &str,
        quality: ReviewQuality,
        recall_speed_ms: Option<i64>,
    ) -> Result<FlashcardRow, sqlx::Error> {
        let card = sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        // Load FSRS parameters
        let (weights, desired_retention) = self.load_fsrs_params().await?;

        // Compute elapsed days since last review (or creation)
        let last_review_str = card.last_reviewed_at.as_deref().unwrap_or(&card.created_at);
        let last_review_dt = chrono::DateTime::parse_from_rfc3339(last_review_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        let elapsed_days = last_review_dt
            .map(|dt| (Utc::now() - dt).num_seconds() as f64 / 86400.0)
            .unwrap_or(0.0)
            .max(0.0);

        let rating = quality as u8;

        // Use FSRS-5
        let (new_stability, new_difficulty, interval_days) = if card.state == "new" {
            let s0 = crate::services::fsrs5::initial_stability(rating, &weights);
            let d0 = crate::services::fsrs5::initial_difficulty(rating, &weights);
            let interval = crate::services::fsrs5::next_interval(s0, desired_retention);
            (s0, d0, interval)
        } else {
            crate::services::fsrs5::schedule_review(
                card.stability,
                card.difficulty,
                elapsed_days,
                rating,
                desired_retention,
                &weights,
            )
        };

        let new_state = match quality {
            ReviewQuality::Again => "relearning",
            _ if card.state == "new" => "learning",
            _ => "review",
        };

        let new_lapses = if matches!(quality, ReviewQuality::Again) {
            card.lapses + 1
        } else {
            card.lapses
        };

        let now = Utc::now();
        let due_at = now + chrono::Duration::days(interval_days as i64);
        let now_str = now.to_rfc3339();
        let due_str = due_at.to_rfc3339();

        // Update card
        sqlx::query(
            r#"
            UPDATE flashcards
            SET stability = ?1, difficulty = ?2,
                due_at = ?3, last_reviewed_at = ?4,
                review_count = review_count + 1,
                state = ?5, lapses = ?6,
                recall_speed_ms = ?7, updated_at = ?4
            WHERE id = ?8
            "#,
        )
        .bind(new_stability)
        .bind(new_difficulty)
        .bind(&due_str)
        .bind(&now_str)
        .bind(new_state)
        .bind(new_lapses)
        .bind(recall_speed_ms)
        .bind(id)
        .execute(&self.pool)
        .await?;

        // Log review
        let scheduled_days = if card.state == "new" {
            0.0
        } else {
            card.due_at
                .as_deref()
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .and_then(|due| {
                    last_review_dt
                        .map(|lr| (due.with_timezone(&Utc) - lr).num_seconds() as f64 / 86400.0)
                })
                .unwrap_or(0.0)
        };

        let log_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO review_log (id, card_id, rating, elapsed_days, scheduled_days,
                                    recall_speed_ms, state, reviewed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&log_id)
        .bind(id)
        .bind(rating as i64)
        .bind(elapsed_days)
        .bind(scheduled_days)
        .bind(recall_speed_ms)
        .bind(&card.state)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    /// List all cards for a given source note.
    pub async fn list_by_note(&self, note_id: &str) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>(
            "SELECT * FROM flashcards WHERE source_note_id = ?1 ORDER BY created_at DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Return a summary of every deck.
    pub async fn list_decks(&self) -> Result<Vec<DeckSummary>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();

        sqlx::query_as::<_, DeckSummary>(
            r#"
            SELECT
                deck AS name,
                COUNT(*) AS card_count,
                SUM(CASE WHEN due_at IS NULL OR due_at <= ?1 THEN 1 ELSE 0 END) AS due_count
            FROM flashcards
            WHERE suspended = 0
            GROUP BY deck
            "#,
        )
        .bind(&now)
        .fetch_all(&self.pool)
        .await
    }

    /// Get a single card by ID.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Create a single flashcard (for manual creation). Immediately due.
    pub async fn create_single(&self, card: NewFlashcard) -> Result<FlashcardRow, sqlx::Error> {
        let mut rows = self.create_batch(vec![card]).await?;
        rows.pop().ok_or(sqlx::Error::RowNotFound)
    }

    /// Update a card's front, back, deck, tags, and type-specific data.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_card(
        &self,
        id: &str,
        front: &str,
        back: &str,
        deck: &str,
        tags: &[String],
        cloze_data: Option<&serde_json::Value>,
        vocab_data: Option<&serde_json::Value>,
    ) -> Result<FlashcardRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let tags_str = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        let cloze_str = cloze_data.map(|v| v.to_string());
        let vocab_str = vocab_data.map(|v| v.to_string());

        sqlx::query(
            r#"
            UPDATE flashcards
            SET front = ?1, back = ?2, deck = ?3, tags = ?4,
                cloze_data = ?5, vocab_data = ?6, updated_at = ?7
            WHERE id = ?8
            "#,
        )
        .bind(front)
        .bind(back)
        .bind(deck)
        .bind(&tags_str)
        .bind(&cloze_str)
        .bind(&vocab_str)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// List all cards in a deck (not just due).
    pub async fn list_all_in_deck(
        &self,
        deck: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE deck = ?1
            ORDER BY created_at DESC
            LIMIT ?2 OFFSET ?3
            "#,
        )
        .bind(deck)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete a single card by ID. Returns true if deleted.
    pub async fn delete_card(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM flashcards WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Toggle suspended state for a card.
    pub async fn suspend_card(
        &self,
        id: &str,
        suspended: bool,
    ) -> Result<FlashcardRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE flashcards SET suspended = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(suspended as i64)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Fetch all due cards across ALL decks.
    pub async fn get_all_due_cards(&self, limit: i64) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE suspended = 0
              AND (due_at IS NULL OR due_at <= ?1)
            ORDER BY due_at ASC
            LIMIT ?2
            "#,
        )
        .bind(&now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Fetch cards with 3+ lapses that are not suspended — struggling cards.
    pub async fn list_struggling_cards(
        &self,
        limit: i64,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>(
            r#"SELECT * FROM flashcards
               WHERE lapses >= 3 AND suspended = 0
               ORDER BY lapses DESC, review_count DESC
               LIMIT ?1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Get total due count across all decks.
    pub async fn total_due_count(&self) -> Result<i64, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM flashcards
            WHERE suspended = 0 AND (due_at IS NULL OR due_at <= ?1)
            "#,
        )
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Persist generated distractors for a card (caching for multiple-choice mode).
    pub async fn update_distractors(
        &self,
        id: &str,
        distractors_json: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE flashcards SET card_distractors = ?1 WHERE id = ?2")
            .bind(distractors_json)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Link a flashcard to a knowledge atom.
    pub async fn update_atom_id(&self, card_id: &str, atom_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE flashcards SET atom_id = ?1 WHERE id = ?2")
            .bind(atom_id)
            .bind(card_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Set `back_embedding_updated_at` to now for a card after its back embedding is computed.
    pub async fn update_embedding_timestamp(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE flashcards SET back_embedding_updated_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete all cards in a deck.
    pub async fn delete_deck(&self, deck: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM flashcards WHERE deck = ?1")
            .bind(deck)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Count flashcards linked to each atom_id.
    pub async fn count_by_atom_ids(
        &self,
        atom_ids: &[String],
    ) -> Result<std::collections::HashMap<String, i64>, sqlx::Error> {
        if atom_ids.is_empty() {
            return Ok(Default::default());
        }
        let placeholders: Vec<String> =
            (0..atom_ids.len()).map(|i| format!("?{}", i + 1)).collect();
        let query = format!(
            "SELECT atom_id, COUNT(*) FROM flashcards WHERE atom_id IN ({}) GROUP BY atom_id",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, (String, i64)>(&query);
        for id in atom_ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().collect())
    }

    /// Load FSRS-5 weights and desired retention.
    async fn load_fsrs_params(&self) -> Result<([f64; 19], f64), sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Params {
            weights: String,
            desired_retention: f64,
        }

        match sqlx::query_as::<_, Params>(
            "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
        )
        .fetch_optional(&self.pool)
        .await?
        {
            Some(p) => {
                let arr: [f64; 19] = serde_json::from_str(&p.weights)
                    .unwrap_or(crate::services::fsrs5::DEFAULT_WEIGHTS);
                Ok((arr, p.desired_retention))
            }
            None => Ok((crate::services::fsrs5::DEFAULT_WEIGHTS, 0.9)),
        }
    }

    /// Find flashcards whose `source_note_id` is connected via the `note_links` table
    /// to the given note. Both directions of the link are followed.
    /// Limit 20, excludes the reviewed card, skips suspended.
    pub async fn find_cards_linked_by_notes(
        &self,
        source_note_id: &str,
        exclude_card_id: &str,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT DISTINCT f.* FROM flashcards f
            INNER JOIN note_links nl
                ON (f.source_note_id = nl.target_id OR f.source_note_id = nl.source_id)
            WHERE (nl.source_id = ?1 OR nl.target_id = ?1)
              AND f.source_note_id != ?1
              AND f.id != ?2
              AND f.suspended = 0
            LIMIT 20
            "#,
        )
        .bind(source_note_id)
        .bind(exclude_card_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Find flashcards sharing the same atom domain as the given atom.
    /// Joins `flashcards` → `knowledge_atoms` on matching domain.
    /// Limit 20, excludes the reviewed card, skips suspended.
    pub async fn find_cards_same_domain(
        &self,
        atom_id: &str,
        exclude_card_id: &str,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT f.* FROM flashcards f
            INNER JOIN knowledge_atoms ka ON f.atom_id = ka.id
            WHERE ka.domain = (SELECT domain FROM knowledge_atoms WHERE id = ?1)
              AND f.id != ?2
              AND f.suspended = 0
            LIMIT 20
            "#,
        )
        .bind(atom_id)
        .bind(exclude_card_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Extend a card's `due_at` by a fraction of its current interval.
    /// Cap boost at 20% of the interval. Only applies to cards due within 48 hours.
    pub async fn apply_propagation_boost(
        &self,
        card_id: &str,
        boost_fraction: f64,
    ) -> Result<(), sqlx::Error> {
        let capped = boost_fraction.min(0.20);
        let now = Utc::now().to_rfc3339();

        // Only boost cards that are due within 48 hours from now.
        // The boost extends due_at by `capped * interval_seconds` where
        // interval = due_at - last_reviewed_at (or created_at).
        sqlx::query(
            r#"
            UPDATE flashcards
            SET due_at = datetime(due_at, '+' || CAST(
                ROUND(?1 * (julianday(due_at) - julianday(COALESCE(last_reviewed_at, created_at))) * 86400)
                AS INTEGER) || ' seconds'),
                updated_at = ?3
            WHERE id = ?2
              AND suspended = 0
              AND due_at IS NOT NULL
              AND due_at <= datetime(?3, '+48 hours')
              AND last_reviewed_at IS NOT NULL
            "#,
        )
        .bind(capped)
        .bind(card_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Reduce a card's `due_at` by a tiny fraction (pull it forward).
    /// Max penalty is 0.08. Only affects cards due within 72 hours.
    pub async fn apply_propagation_penalty(
        &self,
        card_id: &str,
        max_penalty: f64,
    ) -> Result<(), sqlx::Error> {
        let capped = max_penalty.min(0.08);
        let now = Utc::now().to_rfc3339();

        // Reduce due_at by `capped * interval_seconds` (pull due date closer).
        sqlx::query(
            r#"
            UPDATE flashcards
            SET due_at = datetime(due_at, '-' || CAST(
                ROUND(?1 * (julianday(due_at) - julianday(COALESCE(last_reviewed_at, created_at))) * 86400)
                AS INTEGER) || ' seconds'),
                updated_at = ?3
            WHERE id = ?2
              AND suspended = 0
              AND due_at IS NOT NULL
              AND due_at <= datetime(?3, '+72 hours')
              AND last_reviewed_at IS NOT NULL
            "#,
        )
        .bind(capped)
        .bind(card_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
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
            source_context: None,
            atom_id: None,
            deck: deck.to_string(),
            front: "What is 2 + 2?".to_string(),
            back: "4".to_string(),
            card_type: CardType::Basic,
            cloze_data: None,
            vocab_data: None,
            image_data: None,
            tags: vec![],
            stability: 1.0,
            difficulty: 5.0,
            difficulty_estimate: None,
            prerequisite_concepts: None,
        }
    }

    fn vocab_card(deck: &str) -> NewFlashcard {
        NewFlashcard {
            source_note_id: Some("note-lang".to_string()),
            source_context: Some("食べてみる is used in casual speech".to_string()),
            atom_id: None,
            deck: deck.to_string(),
            front: "食べてみる".to_string(),
            back: "to try eating".to_string(),
            card_type: CardType::Vocabulary,
            cloze_data: None,
            vocab_data: Some(serde_json::json!({
                "word": "食べてみる",
                "reading": "たべてみる",
                "meaning": "to try eating",
                "example_sentence": "新しいレストランで食べてみた",
                "part_of_speech": "verb"
            })),
            image_data: None,
            tags: vec!["japanese".to_string(), "n3".to_string()],
            stability: 1.0,
            difficulty: 5.0,
            difficulty_estimate: None,
            prerequisite_concepts: None,
        }
    }

    #[tokio::test]
    async fn test_create_batch_basic() {
        let (_pool, repo) = setup().await;
        let cards = vec![sample_card("math", Some("note-1"))];
        let created = repo.create_batch(cards).await.unwrap();

        assert_eq!(created.len(), 1);
        assert_eq!(created[0].front, "What is 2 + 2?");
        assert_eq!(created[0].back, "4");
        assert_eq!(created[0].card_type, "basic");
        assert_eq!(created[0].state, "new");
        assert_eq!(created[0].review_count, 0);
        assert_eq!(created[0].suspended, 0);
    }

    #[tokio::test]
    async fn test_create_batch_vocabulary_card() {
        let (_pool, repo) = setup().await;
        let cards = vec![vocab_card("japanese")];
        let created = repo.create_batch(cards).await.unwrap();

        assert_eq!(created[0].card_type, "vocabulary");
        assert!(created[0].vocab_data.is_some());
        assert!(created[0].source_context.is_some());

        let tags: Vec<String> = serde_json::from_str(&created[0].tags).unwrap();
        assert_eq!(tags, vec!["japanese", "n3"]);
    }

    #[tokio::test]
    async fn test_list_by_note() {
        let (_pool, repo) = setup().await;
        let cards = vec![
            sample_card("math", Some("note-1")),
            sample_card("math", Some("note-1")),
        ];
        repo.create_batch(cards).await.unwrap();

        let by_note = repo.list_by_note("note-1").await.unwrap();
        assert_eq!(by_note.len(), 2);
    }

    #[tokio::test]
    async fn test_get_due_cards() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![sample_card("science", Some("note-2"))])
            .await
            .unwrap();

        let due = repo.get_due_cards("science", 10).await.unwrap();
        assert_eq!(due.len(), 1, "New card should be immediately due");
    }

    #[tokio::test]
    async fn test_record_review_fsrs5_good() {
        let (_pool, repo) = setup().await;
        let created = repo
            .create_batch(vec![sample_card("history", Some("note-3"))])
            .await
            .unwrap();
        let card_id = &created[0].id;

        let updated = repo
            .record_review(card_id, ReviewQuality::Good, Some(2500))
            .await
            .unwrap();

        assert!(updated.stability > 0.0, "Stability should be positive");
        assert!(updated.difficulty > 0.0, "Difficulty should be set");
        assert_eq!(updated.review_count, 1);
        assert!(updated.due_at.is_some());
        assert_eq!(updated.recall_speed_ms, Some(2500));
    }

    #[tokio::test]
    async fn test_record_review_again_increases_lapses() {
        let (_pool, repo) = setup().await;
        let created = repo
            .create_batch(vec![sample_card("test", None)])
            .await
            .unwrap();
        let card_id = &created[0].id;

        // First review Good, then Again
        repo.record_review(card_id, ReviewQuality::Good, None)
            .await
            .unwrap();
        let updated = repo
            .record_review(card_id, ReviewQuality::Again, None)
            .await
            .unwrap();

        assert_eq!(updated.lapses, 1);
        assert_eq!(updated.state, "relearning");
    }

    #[tokio::test]
    async fn test_review_log_written() {
        let (pool, repo) = setup().await;
        let created = repo
            .create_batch(vec![sample_card("log-test", None)])
            .await
            .unwrap();
        let card_id = &created[0].id;

        repo.record_review(card_id, ReviewQuality::Good, Some(1500))
            .await
            .unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM review_log WHERE card_id = ?1")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1, "Should have 1 review log entry");
    }

    #[tokio::test]
    async fn test_list_decks() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("test-deck", Some("note-4")),
            sample_card("test-deck", Some("note-4")),
        ])
        .await
        .unwrap();

        let decks = repo.list_decks().await.unwrap();
        let target = decks.iter().find(|d| d.name == "test-deck");
        assert!(target.is_some());
        assert_eq!(target.unwrap().card_count, 2);
    }

    #[tokio::test]
    async fn test_delete_deck() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![sample_card("to-delete", None)])
            .await
            .unwrap();

        let deleted = repo.delete_deck("to-delete").await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = repo.get_due_cards("to-delete", 10).await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let (_pool, repo) = setup().await;
        let created = repo
            .create_batch(vec![sample_card("test", None)])
            .await
            .unwrap();
        let found = repo.get_by_id(&created[0].id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, created[0].id);
        let missing = repo.get_by_id("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_create_single() {
        let (_pool, repo) = setup().await;
        let card = repo
            .create_single(sample_card("single", None))
            .await
            .unwrap();
        assert_eq!(card.front, "What is 2 + 2?");
        assert_eq!(card.state, "new");
    }

    #[tokio::test]
    async fn test_update_card() {
        let (_pool, repo) = setup().await;
        let created = repo
            .create_single(sample_card("edit-test", None))
            .await
            .unwrap();
        let updated = repo
            .update_card(
                &created.id,
                "Updated front",
                "Updated back",
                "new-deck",
                &["tag1".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.front, "Updated front");
        assert_eq!(updated.back, "Updated back");
        assert_eq!(updated.deck, "new-deck");
        let tags: Vec<String> = serde_json::from_str(&updated.tags).unwrap();
        assert_eq!(tags, vec!["tag1"]);
    }

    #[tokio::test]
    async fn test_list_all_in_deck() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("list-all", None),
            sample_card("list-all", None),
            sample_card("other", None),
        ])
        .await
        .unwrap();
        let all = repo.list_all_in_deck("list-all", 100, 0).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_card() {
        let (_pool, repo) = setup().await;
        let created = repo.create_single(sample_card("del", None)).await.unwrap();
        assert!(repo.delete_card(&created.id).await.unwrap());
        assert!(repo.get_by_id(&created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_suspend_card() {
        let (_pool, repo) = setup().await;
        let created = repo
            .create_single(sample_card("suspend", None))
            .await
            .unwrap();
        assert_eq!(created.suspended, 0);
        let suspended = repo.suspend_card(&created.id, true).await.unwrap();
        assert_eq!(suspended.suspended, 1);
        let due = repo.get_due_cards("suspend", 10).await.unwrap();
        assert!(due.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_due_cards() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("deck-a", None),
            sample_card("deck-b", None),
        ])
        .await
        .unwrap();
        let all_due = repo.get_all_due_cards(100).await.unwrap();
        assert!(all_due.len() >= 2);
    }

    #[tokio::test]
    async fn test_total_due_count() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("count-a", None),
            sample_card("count-b", None),
        ])
        .await
        .unwrap();
        let count = repo.total_due_count().await.unwrap();
        assert!(count >= 2);
    }

    #[tokio::test]
    async fn test_fsrs_params_loaded_from_db() {
        let (_pool, repo) = setup().await;
        let (weights, retention) = repo.load_fsrs_params().await.unwrap();
        assert_eq!(weights.len(), 19);
        assert!((retention - 0.9).abs() < 0.01);
    }
}
