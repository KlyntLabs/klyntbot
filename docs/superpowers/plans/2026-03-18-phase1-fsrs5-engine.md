# Phase 1: FSRS-5 Engine + Card Types + Review Log

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the simplified flashcard scheduler with a full FSRS-5 engine, add 5 card types, rename question/answer to front/back, and add a review_log for future personal parameter training.

**Architecture:** New `fsrs5.rs` module in `cognitive/src/services/` owns the FSRS-5 math (separate from existing `decay.rs` which is unchanged). The `flashcards` table DDL is consolidated in-place (pre-release, no migration). `FlashcardRepo::record_review` is rewritten to use FSRS-5. A `review_log` table captures every review for future weight training. `FlashcardRow`, `NewFlashcard`, `CardType`, and all downstream types are updated.

**Tech Stack:** Rust, sqlx (SQLite), chrono, serde_json, uuid

**Spec:** `docs/superpowers/specs/2026-03-18-learning-system-design.md` — Sections: "FSRS-5 Engine", "Data Model"

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `crates/cognitive/src/services/fsrs5.rs` | FSRS-5 algorithm: retrievability, stability update, difficulty update, interval computation |
| Modify | `crates/cognitive/src/services/mod.rs` | Add `pub mod fsrs5;` |
| Modify | `crates/cognitive/migrations/001_cognitive_tables.sql:L400-L426` | Rewrite flashcards DDL + add `review_log` + `fsrs_parameters` tables |
| Modify | `crates/cognitive/src/repos/flashcard.rs` | Rewrite `CardType`, `NewFlashcard`, `FlashcardRow`, `FlashcardRepo` methods, add `ReviewLogEntry` |
| Modify | `crates/cognitive/src/repos/mod.rs:L25-L27` | Update re-exports |
| Modify | `crates/cognitive/src/lib.rs:L34-L36` | Update re-exports |
| Modify | `crates/desktop-shared/src/commands/notes.rs:L239-L253` | Update `FlashcardResponse` fields |
| Modify | `crates/app-core/src/handlers/notes/flashcard.rs` | Update `flashcard_to_response`, `record_review` to pass `recall_speed_ms` |
| Modify | `crates/app-core/src/handlers/notes/insight.rs:L393-L438` | Update `insight_save_flashcards` for new `NewFlashcard` shape |
| Modify | `crates/app-core/src/adapters/flashcard_accessor.rs` | Update raw SQL for renamed columns |
| Modify | `crates/desktop/src/commands/notes.rs` | Update Tauri command params for `recall_speed_ms` |

---

### Task 1: FSRS-5 Algorithm Module

**Files:**
- Create: `crates/cognitive/src/services/fsrs5.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Write failing tests for FSRS-5 retrievability**

In `crates/cognitive/src/services/fsrs5.rs`:

```rust
//! FSRS-5 spaced repetition scheduler for flashcards.
//!
//! Separate from `decay.rs` which handles cognitive memory retrieval scoring.
//! This module implements the full FSRS-5 algorithm with 19 learned weights.

/// Default FSRS-5 weights (from the FSRS-5 paper).
pub const DEFAULT_WEIGHTS: [f64; 19] = [
    0.40255, 1.18385, 3.173, 15.69105,   // w0-w3: initial stability for rating 1-4
    7.1949, 0.5345, 1.4604,               // w4-w6: difficulty
    0.0046, 1.54575, 0.1192, 1.01925,     // w7-w10: stability after success
    1.9395, 0.11, 0.29605, 2.2698,        // w11-w14: stability after failure
    0.2315, 2.9898, 0.51655, 0.6621,      // w15-w18: short-term scheduling
];

/// FSRS-5 retrievability: probability of recall.
/// R = (1 + elapsed_days / (9 * S))^(-1)
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64 {
    if stability <= 0.0 || elapsed_days < 0.0 {
        return 0.0;
    }
    (1.0 + elapsed_days / (9.0 * stability)).powf(-1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrievability_at_zero_elapsed() {
        let r = retrievability(0.0, 1.0);
        assert!((r - 1.0).abs() < 0.001, "R should be 1.0 at t=0, got {r}");
    }

    #[test]
    fn test_retrievability_at_stability() {
        // At t = S, R = (1 + 1/9)^(-1) = 9/10 = 0.9
        let r = retrievability(5.0, 5.0);
        assert!((r - 0.9).abs() < 0.001, "R should be 0.9 at t=S, got {r}");
    }

    #[test]
    fn test_retrievability_decays() {
        let r1 = retrievability(1.0, 1.0);
        let r10 = retrievability(10.0, 1.0);
        assert!(r1 > r10, "R should decay over time");
    }

    #[test]
    fn test_retrievability_zero_stability() {
        assert_eq!(retrievability(1.0, 0.0), 0.0);
        assert_eq!(retrievability(1.0, -1.0), 0.0);
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(fsrs5)'`
Expected: 4 PASS

- [ ] **Step 3: Add `pub mod fsrs5;` to services/mod.rs**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod fsrs5;
```

- [ ] **Step 4: Run tests again to confirm module wiring**

Run: `cargo nextest run -p cognitive -E 'test(fsrs5)'`
Expected: 4 PASS

- [ ] **Step 5: Add difficulty update function with tests**

Append to `crates/cognitive/src/services/fsrs5.rs`:

```rust
/// Initial difficulty for a given first rating (1=again..4=easy).
/// D₀(G) = w₄ - exp(w₅ * (G - 1)) + 1
fn initial_difficulty(rating: u8, w: &[f64; 19]) -> f64 {
    let g = rating as f64;
    (w[4] - (w[5] * (g - 1.0)).exp() + 1.0).clamp(1.0, 10.0)
}

/// Mean reversion towards D₀(4) to prevent difficulty from drifting too far.
fn mean_revert(init: f64, current: f64, w: &[f64; 19]) -> f64 {
    w[7] * init + (1.0 - w[7]) * current
}

/// Update difficulty after a review.
/// D' = w₇ · D₀(4) + (1 - w₇) · (D - w₆ · (G - 3))
pub fn next_difficulty(current_d: f64, rating: u8, w: &[f64; 19]) -> f64 {
    let d0_4 = initial_difficulty(4, w);
    let delta = current_d - w[6] * (rating as f64 - 3.0);
    mean_revert(d0_4, delta, w).clamp(1.0, 10.0)
}
```

And tests:

```rust
    #[test]
    fn test_initial_difficulty_easy_is_lowest() {
        let d1 = initial_difficulty(1, &DEFAULT_WEIGHTS);
        let d4 = initial_difficulty(4, &DEFAULT_WEIGHTS);
        assert!(d1 > d4, "Again should produce higher difficulty than Easy");
    }

    #[test]
    fn test_next_difficulty_decreases_on_easy() {
        let d = 5.0;
        let d_new = next_difficulty(d, 4, &DEFAULT_WEIGHTS);
        assert!(d_new < d, "Easy rating should decrease difficulty");
    }

    #[test]
    fn test_next_difficulty_increases_on_again() {
        let d = 5.0;
        let d_new = next_difficulty(d, 1, &DEFAULT_WEIGHTS);
        assert!(d_new > d, "Again rating should increase difficulty");
    }

    #[test]
    fn test_difficulty_clamped() {
        let d_low = next_difficulty(1.0, 4, &DEFAULT_WEIGHTS);
        assert!(d_low >= 1.0, "Difficulty should not go below 1.0");
        let d_high = next_difficulty(10.0, 1, &DEFAULT_WEIGHTS);
        assert!(d_high <= 10.0, "Difficulty should not exceed 10.0");
    }
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(fsrs5)'`
Expected: 8 PASS

- [ ] **Step 7: Add stability update functions with tests**

Append to `crates/cognitive/src/services/fsrs5.rs`:

```rust
/// Stability after a successful review (rating >= 2).
/// S'_r = S · (e^w₈ · (11 - D) · S^(-w₉) · (e^(w₁₀·(1-R)) - 1) · hard_penalty · easy_bonus)
pub fn next_stability_success(
    s: f64,
    d: f64,
    r: f64,
    rating: u8,
    w: &[f64; 19],
) -> f64 {
    let hard_penalty = if rating == 2 { w[15] } else { 1.0 };
    let easy_bonus = if rating == 4 { w[16] } else { 1.0 };
    let s_new = s * ((w[8]).exp()
        * (11.0 - d)
        * s.powf(-w[9])
        * ((w[10] * (1.0 - r)).exp() - 1.0)
        * hard_penalty
        * easy_bonus
        + 1.0);
    s_new.max(0.01)
}

/// Stability after a failed review (rating == 1, "Again").
/// S'_f = w₁₁ · D^(-w₁₂) · ((S+1)^w₁₃ - 1) · e^(w₁₄·(1-R))
pub fn next_stability_failure(s: f64, d: f64, r: f64, w: &[f64; 19]) -> f64 {
    let s_new = w[11]
        * d.powf(-w[12])
        * ((s + 1.0).powf(w[13]) - 1.0)
        * (w[14] * (1.0 - r)).exp();
    s_new.clamp(0.01, s) // failure stability must be less than current
}

/// Compute the interval (in days) from stability and desired retention.
/// I = S · 9 · (1/R - 1)
pub fn next_interval(stability: f64, desired_retention: f64) -> f64 {
    let retention = desired_retention.clamp(0.7, 0.99);
    let interval = stability * 9.0 * (1.0 / retention - 1.0);
    interval.max(1.0).round()
}

/// Full FSRS-5 review computation. Returns (new_stability, new_difficulty, interval_days).
pub fn schedule_review(
    stability: f64,
    difficulty: f64,
    elapsed_days: f64,
    rating: u8,
    desired_retention: f64,
    w: &[f64; 19],
) -> (f64, f64, f64) {
    let r = retrievability(elapsed_days, stability);
    let new_d = next_difficulty(difficulty, rating, w);

    let new_s = if rating == 1 {
        next_stability_failure(stability, new_d, r, w)
    } else {
        next_stability_success(stability, new_d, r, rating, w)
    };

    let interval = next_interval(new_s, desired_retention);
    (new_s, new_d, interval)
}

/// Initial stability for a new card's first review, based on rating.
/// S₀(G) = w[G-1]
pub fn initial_stability(rating: u8, w: &[f64; 19]) -> f64 {
    let idx = (rating as usize).saturating_sub(1).min(3);
    w[idx].max(0.01)
}
```

And tests:

```rust
    #[test]
    fn test_stability_increases_on_good() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let s_new = next_stability_success(s, d, r, 3, &DEFAULT_WEIGHTS);
        assert!(s_new > s, "Good review should increase stability, got {s_new}");
    }

    #[test]
    fn test_stability_decreases_on_again() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let s_new = next_stability_failure(s, d, r, &DEFAULT_WEIGHTS);
        assert!(s_new < s, "Again should decrease stability");
        assert!(s_new > 0.0, "Stability should stay positive");
    }

    #[test]
    fn test_hard_penalty_reduces_growth() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let s_good = next_stability_success(s, d, r, 3, &DEFAULT_WEIGHTS);
        let s_hard = next_stability_success(s, d, r, 2, &DEFAULT_WEIGHTS);
        assert!(s_hard < s_good, "Hard should grow less than Good");
    }

    #[test]
    fn test_easy_bonus_increases_growth() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let s_good = next_stability_success(s, d, r, 3, &DEFAULT_WEIGHTS);
        let s_easy = next_stability_success(s, d, r, 4, &DEFAULT_WEIGHTS);
        assert!(s_easy > s_good, "Easy should grow more than Good");
    }

    #[test]
    fn test_next_interval_at_90_percent() {
        let interval = next_interval(10.0, 0.9);
        // I = 10 * 9 * (1/0.9 - 1) = 10 * 9 * 0.111 = 10.0
        assert!((interval - 10.0).abs() < 1.0, "Expected ~10 day interval, got {interval}");
    }

    #[test]
    fn test_schedule_review_integration() {
        let (s, d, i) = schedule_review(5.0, 5.0, 5.0, 3, 0.9, &DEFAULT_WEIGHTS);
        assert!(s > 5.0, "Stability should increase on Good");
        assert!(i > 0.0, "Interval should be positive");
        assert!(d > 0.0 && d <= 10.0, "Difficulty should be in [1, 10]");
    }

    #[test]
    fn test_initial_stability_by_rating() {
        let s1 = initial_stability(1, &DEFAULT_WEIGHTS);
        let s4 = initial_stability(4, &DEFAULT_WEIGHTS);
        assert!(s4 > s1, "Easy first review should have higher initial stability");
    }
```

- [ ] **Step 8: Run all FSRS-5 tests**

Run: `cargo nextest run -p cognitive -E 'test(fsrs5)'`
Expected: 15 PASS

- [ ] **Step 9: Commit**

```bash
git add crates/cognitive/src/services/fsrs5.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): add FSRS-5 algorithm module with full test coverage"
```

---

### Task 2: Rewrite Flashcards DDL + Add review_log + fsrs_parameters

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql:L400-L426`

- [ ] **Step 1: Replace the flashcards DDL block**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, replace lines 400-426 with:

```sql
-- ── Flashcards (FSRS-5 spaced repetition) ─────────────────────────

CREATE TABLE IF NOT EXISTS flashcards (
    id TEXT PRIMARY KEY,
    source_note_id TEXT,
    source_context TEXT,
    deck TEXT NOT NULL DEFAULT 'general',
    front TEXT NOT NULL,
    back TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'basic',
    cloze_data TEXT,
    vocab_data TEXT,
    image_data TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    stability REAL NOT NULL DEFAULT 1.0,
    difficulty REAL NOT NULL DEFAULT 5.0,
    due_at TEXT,
    last_reviewed_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',
    suspended INTEGER NOT NULL DEFAULT 0,
    recall_speed_ms INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_flashcards_source_note ON flashcards(source_note_id);
CREATE INDEX IF NOT EXISTS idx_flashcards_due ON flashcards(due_at);
CREATE INDEX IF NOT EXISTS idx_flashcards_deck ON flashcards(deck);
CREATE INDEX IF NOT EXISTS idx_flashcards_state ON flashcards(state);

-- ── FSRS-5 personal parameters ───────────────────────────────────

CREATE TABLE IF NOT EXISTS fsrs_parameters (
    id TEXT PRIMARY KEY DEFAULT 'local',
    weights TEXT NOT NULL,
    desired_retention REAL NOT NULL DEFAULT 0.9,
    trained_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0
);

-- Insert default weights on creation
INSERT OR IGNORE INTO fsrs_parameters (id, weights)
VALUES ('local', '[0.40255,1.18385,3.173,15.69105,7.1949,0.5345,1.4604,0.0046,1.54575,0.1192,1.01925,1.9395,0.11,0.29605,2.2698,0.2315,2.9898,0.51655,0.6621]');

-- ── Review log (feeds FSRS-5 weight training) ────────────────────

CREATE TABLE IF NOT EXISTS review_log (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL REFERENCES flashcards(id) ON DELETE CASCADE,
    rating INTEGER NOT NULL,
    elapsed_days REAL NOT NULL,
    scheduled_days REAL NOT NULL,
    recall_speed_ms INTEGER,
    state TEXT NOT NULL,
    reviewed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_review_log_card ON review_log(card_id);
CREATE INDEX IF NOT EXISTS idx_review_log_reviewed ON review_log(reviewed_at);
```

- [ ] **Step 2: Bump the cognitive migration version**

In `crates/cognitive/src/repos/mod.rs`, update the first `FeatureMigration` entry:

```rust
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 6,
            description: "FSRS-5 flashcard schema + review_log + fsrs_parameters".to_string(),
            sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
        },
```

Also update version 5 → 7 for BookIndex (so both re-run):

```rust
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 7,
            description: "Add BookIndex tree nodes and GT-Link tables".to_string(),
            sql: include_str!("../../migrations/002_book_index_tables.sql").to_string(),
        },
```

- [ ] **Step 3: Verify DDL is syntactically valid**

Run: `cargo check -p cognitive 2>&1 | head -5`
Expected: Compilation errors in `flashcard.rs` referencing old column names (`question`, `answer`, `choices`, etc.). This is expected — the DDL changed but the Rust types haven't been updated yet. Tasks 3-5 fix this. The workspace will not compile until Task 5 is complete.

- [ ] **Step 4: Commit DDL changes**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): rewrite flashcard DDL for FSRS-5 + add review_log, fsrs_parameters"
```

---

### Task 3: Rewrite FlashcardRow, CardType, NewFlashcard, ReviewQuality

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs:L1-L78`

- [ ] **Step 1: Replace types at top of flashcard.rs**

Replace lines 1-78 with:

```rust
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
    pub fn from_str(s: &str) -> Self {
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

impl ReviewQuality {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

// ── Input / row types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NewFlashcard {
    pub source_note_id: Option<String>,
    pub source_context: Option<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FlashcardRow {
    pub id: String,
    pub source_note_id: Option<String>,
    pub source_context: Option<String>,
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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
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
```

- [ ] **Step 2: Verify it compiles (will have errors in repo methods — expected)**

Run: `cargo check -p cognitive 2>&1 | head -20`
Expected: Errors in repo methods referencing old column names. This is correct — we fix them next.

- [ ] **Step 3: Commit type changes**

```bash
git add crates/cognitive/src/repos/flashcard.rs
git commit -m "feat(cognitive): rewrite flashcard types for FSRS-5 (CardType, NewFlashcard, FlashcardRow)"
```

---

### Task 4: Rewrite FlashcardRepo Methods

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs:L80-L289` (the `impl FlashcardRepo` block)

- [ ] **Step 1: Rewrite create_batch**

Replace the entire `impl FlashcardRepo` block (keep the `struct FlashcardRepo` and `new()`):

```rust
impl FlashcardRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
                    (id, source_note_id, source_context,
                     deck, front, back, card_type,
                     cloze_data, vocab_data, image_data, tags,
                     stability, difficulty, due_at, last_reviewed_at,
                     review_count, lapses, state, suspended, recall_speed_ms,
                     created_at, updated_at)
                VALUES
                    (?1, ?2, ?3,
                     ?4, ?5, ?6, ?7,
                     ?8, ?9, ?10, ?11,
                     ?12, ?13, ?14, NULL,
                     0, 0, 'new', 0, NULL,
                     ?15, ?15)
                "#,
            )
            .bind(&id)
            .bind(&card.source_note_id)
            .bind(&card.source_context)
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

        // Load FSRS parameters (or use defaults)
        let (weights, desired_retention) = self.load_fsrs_params().await?;

        // Compute elapsed days since last review (or creation)
        let last_review = card.last_reviewed_at.as_deref().unwrap_or(&card.created_at);
        let elapsed_days = chrono::DateTime::parse_from_rfc3339(last_review)
            .map(|dt| {
                let elapsed = Utc::now() - dt.with_timezone(&Utc);
                elapsed.num_seconds() as f64 / 86400.0
            })
            .unwrap_or(0.0)
            .max(0.0);

        let rating = quality.as_u8();
        let old_stability = card.stability;

        // Use FSRS-5 for cards that have been reviewed before
        let (new_stability, new_difficulty, interval_days) = if card.state == "new" {
            // First review: use initial stability
            let s0 = crate::services::fsrs5::initial_stability(rating, &weights);
            let d0 = crate::services::fsrs5::initial_difficulty(rating, &weights);
            let interval = crate::services::fsrs5::next_interval(s0, desired_retention);
            (s0, d0, interval)
        } else {
            crate::services::fsrs5::schedule_review(
                old_stability,
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
        let due_at = now + chrono::Duration::seconds((interval_days * 86400.0) as i64);
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
            // Previous scheduled interval
            card.due_at
                .as_deref()
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .and_then(|due| {
                    chrono::DateTime::parse_from_rfc3339(last_review)
                        .ok()
                        .map(|lr| (due - lr).num_seconds() as f64 / 86400.0)
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
            WHERE suspended = 0
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

    /// Delete all cards in a deck.
    pub async fn delete_deck(&self, deck: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM flashcards WHERE deck = ?1")
            .bind(deck)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
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
                let w: Vec<f64> = serde_json::from_str(&p.weights).unwrap_or_default();
                if w.len() == 19 {
                    let mut arr = [0.0; 19];
                    arr.copy_from_slice(&w);
                    Ok((arr, p.desired_retention))
                } else {
                    Ok((crate::services::fsrs5::DEFAULT_WEIGHTS, p.desired_retention))
                }
            }
            None => Ok((crate::services::fsrs5::DEFAULT_WEIGHTS, 0.9)),
        }
    }
}
```

Note: `initial_difficulty` in `fsrs5.rs` needs to be made `pub` for this to compile.

- [ ] **Step 2: Make `initial_difficulty` public in fsrs5.rs**

In `crates/cognitive/src/services/fsrs5.rs`, change:

```rust
fn initial_difficulty(rating: u8, w: &[f64; 19]) -> f64 {
```

to:

```rust
pub fn initial_difficulty(rating: u8, w: &[f64; 19]) -> f64 {
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p cognitive`
Expected: PASS (may have warnings about unused imports, that's fine)

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/flashcard.rs crates/cognitive/src/services/fsrs5.rs
git commit -m "feat(cognitive): rewrite FlashcardRepo with FSRS-5 scheduling + review_log"
```

---

### Task 5: Rewrite Tests

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs` (tests module at bottom)

- [ ] **Step 1: Replace the entire tests module**

Replace `#[cfg(test)] mod tests { ... }` at the bottom of `flashcard.rs`:

```rust
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
        }
    }

    fn vocab_card(deck: &str) -> NewFlashcard {
        NewFlashcard {
            source_note_id: Some("note-lang".to_string()),
            source_context: Some("食べてみる is used in casual speech".to_string()),
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
        assert!(updated.recall_speed_ms == Some(2500));
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

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM review_log WHERE card_id = ?1")
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
    async fn test_fsrs_params_loaded_from_db() {
        let (_pool, repo) = setup().await;
        let (weights, retention) = repo.load_fsrs_params().await.unwrap();
        assert_eq!(weights.len(), 19);
        assert!((retention - 0.9).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Run all cognitive tests**

Run: `cargo nextest run -p cognitive`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/repos/flashcard.rs
git commit -m "test(cognitive): rewrite flashcard tests for FSRS-5 schema"
```

---

### Task 6: Update Re-exports

**Files:**
- Modify: `crates/cognitive/src/repos/mod.rs:L25-L27`
- Modify: `crates/cognitive/src/lib.rs:L34-L36`

- [ ] **Step 1: Update repos/mod.rs re-exports**

Replace line 25-27:

```rust
pub use flashcard::{
    CardType, DeckSummary, FlashcardRepo, FlashcardRow, NewFlashcard, ReviewLogEntry,
    ReviewQuality,
};
```

- [ ] **Step 2: Update lib.rs re-exports**

Remove the `#[allow(deprecated)]` and update the re-export block (lines 34-36) to:

```rust
pub use repos::{
    CardType, DeckSummary, FlashcardRepo, FlashcardRow, NewFlashcard, ReviewLogEntry,
    ReviewQuality,
};
```

- [ ] **Step 3: Run cargo check on cognitive**

Run: `cargo check -p cognitive`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/mod.rs crates/cognitive/src/lib.rs
git commit -m "refactor(cognitive): update flashcard re-exports for FSRS-5 types"
```

---

### Task 7: Update Downstream Crates (desktop-shared, app-core, desktop)

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs:L239-L253`
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs`
- Modify: `crates/app-core/src/handlers/notes/insight.rs:L393-L438`
- Modify: `crates/app-core/src/adapters/flashcard_accessor.rs`
- Modify: `crates/desktop/src/commands/notes.rs` (flashcard commands)
- Modify: `crates/desktop-shared/src/commands/notes.rs:L274-L279`

- [ ] **Step 1: Update FlashcardResponse in desktop-shared**

Replace `FlashcardResponse` (lines 239-253):

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardResponse {
    pub id: String,
    pub deck: String,
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
    pub image_data: Option<serde_json::Value>,
    pub tags: serde_json::Value,
    pub source_note_id: Option<String>,
    pub source_context: Option<String>,
    pub stability: f64,
    pub difficulty: f64,
    pub due_at: Option<String>,
    pub state: String,
    pub review_count: i64,
    pub recall_speed_ms: Option<i64>,
    pub created_at: String,
}
```

- [ ] **Step 2: Update FlashcardReviewParams to include recall_speed_ms**

Replace `FlashcardReviewParams` (lines 274-279):

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardReviewParams {
    pub card_id: String,
    pub quality: String,
    pub recall_speed_ms: Option<i64>,
}
```

- [ ] **Step 3: Update flashcard_to_response in app-core**

Replace the function in `crates/app-core/src/handlers/notes/flashcard.rs`:

```rust
pub(super) fn flashcard_to_response(r: cognitive::FlashcardRow) -> FlashcardResponse {
    FlashcardResponse {
        id: r.id,
        deck: r.deck,
        front: r.front,
        back: r.back,
        card_type: r.card_type,
        cloze_data: r
            .cloze_data
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        vocab_data: r
            .vocab_data
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        image_data: r
            .image_data
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        tags: serde_json::from_str(&r.tags).unwrap_or(serde_json::Value::Array(vec![])),
        source_note_id: r.source_note_id,
        source_context: r.source_context,
        stability: r.stability,
        difficulty: r.difficulty,
        due_at: r.due_at,
        state: r.state,
        review_count: r.review_count,
        recall_speed_ms: r.recall_speed_ms,
        created_at: r.created_at,
    }
}
```

- [ ] **Step 4: Update record_review handler to pass recall_speed_ms**

In the same file, update `flashcard_record_review`:

```rust
    pub async fn flashcard_record_review(
        &self,
        params: FlashcardReviewParams,
    ) -> Result<FlashcardResponse, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let quality = match params.quality.as_str() {
            "again" => ReviewQuality::Again,
            "hard" => ReviewQuality::Hard,
            "good" => ReviewQuality::Good,
            "easy" => ReviewQuality::Easy,
            _ => {
                return Err(ApiError::new(
                    "VALIDATION",
                    "Invalid review quality: must be again|hard|good|easy",
                ))
            }
        };
        let card = repo
            .record_review(&params.card_id, quality, params.recall_speed_ms)
            .await
            .map_err(|e: sqlx::Error| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(card))
    }
```

- [ ] **Step 5: Update insight_save_flashcards for new NewFlashcard shape**

In `crates/app-core/src/handlers/notes/insight.rs`, update the mapping in `insight_save_flashcards` (around line 406):

```rust
            let cards: Vec<cognitive::NewFlashcard> = params
                .questions
                .iter()
                .map(|q| {
                    let (stability, difficulty) = match q.difficulty.as_str() {
                        "easy" => (4.0, 3.0),
                        "hard" => (0.8, 7.0),
                        _ => (2.0, 5.0),
                    };
                    cognitive::NewFlashcard {
                        source_note_id: Some(params.note_id.clone()),
                        source_context: None,
                        deck: params.deck_name.clone(),
                        front: q.question.clone(),
                        back: q.correct_answer.clone(),
                        card_type: cognitive::CardType::Basic,
                        cloze_data: None,
                        vocab_data: None,
                        image_data: None,
                        tags: vec![],
                        stability,
                        difficulty,
                    }
                })
                .collect();
```

Note: difficulty values now use the FSRS-5 scale (1-10) instead of the old 0-1 scale.

- [ ] **Step 6: Update FlashcardAccessorImpl raw SQL**

In `crates/app-core/src/adapters/flashcard_accessor.rs`, the `insight_review_id` column no longer exists in the schema. Rather than silently querying the wrong column, return 0.0 explicitly until Phase 3 when `feature-learning` takes over progress tracking with proper `source_note_id`-based queries:

```rust
    async fn review_success_rate(&self, _insight_review_id: &str, _days: i32) -> f64 {
        // TODO(phase-3): Replace with source_note_id-based query in feature-learning.
        // The old insight_review_id column was removed in the FSRS-5 migration.
        // Progress tracking will be rewired in the feature-learning crate.
        0.0
    }
```

This makes the temporary degradation explicit. `ProgressComputer` will receive 0.0 for the flashcard weight (40%) until Phase 3 wires it properly.

- [ ] **Step 7: Verify full workspace compiles**

Run: `cargo check --workspace`
Expected: PASS (with possible warnings)

- [ ] **Step 8: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs \
       crates/app-core/src/handlers/notes/flashcard.rs \
       crates/app-core/src/handlers/notes/insight.rs \
       crates/app-core/src/adapters/flashcard_accessor.rs
git commit -m "feat: update downstream crates for FSRS-5 flashcard schema"
```

---

### Task 8: Update Tauri Commands

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs` (flashcard Tauri commands)

- [ ] **Step 1: Check current Tauri command signatures**

Read the flashcard-related Tauri commands in `crates/desktop/src/commands/notes.rs` to find exact line numbers and current signatures.

- [ ] **Step 2: Update flashcard_record_review Tauri command**

The Tauri command just delegates to `AppCore::flashcard_record_review(params)`. Since `FlashcardReviewParams` now includes `recall_speed_ms`, the Tauri command itself doesn't need signature changes — the frontend will pass the new field. Verify the command compiles.

- [ ] **Step 3: Verify desktop crate compiles**

Run: `cargo check -p desktop`
Expected: PASS

- [ ] **Step 4: Commit (if any changes needed)**

```bash
git add crates/desktop/src/commands/notes.rs
git commit -m "fix(desktop): update Tauri flashcard commands for FSRS-5 params"
```

---

### Task 9: Run Full Test Suite + Clippy

**Files:** None (verification only)

- [ ] **Step 1: Run all workspace tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any that appear)

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: PASS (fix any formatting issues with `cargo fmt --all`)

- [ ] **Step 4: Run doc tests**

Run: `cargo test --workspace --doc`
Expected: PASS

- [ ] **Step 5: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting from FSRS-5 migration"
```
