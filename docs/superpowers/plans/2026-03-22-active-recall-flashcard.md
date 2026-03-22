# Active Recall Flashcard System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the passive flashcard review (show front → reveal back → self-rate) into an active recall system with AI-graded typed answers, multi-mode review, knowledge graph propagation, and session intelligence.

**Architecture:** Grading pipeline lives in `app-core` handlers (mirrors `practice_submit_unit` pattern). FSRS-5 stays untouched in `cognitive` — continuous scores (0.0–1.0) map to discrete ratings (1–4) via thresholds. Card embeddings use existing LanceDB + fastembed infrastructure. Frontend replaces the 141-line `FlashcardReview` with a composable component tree and `useActiveReview` state machine hook.

**Tech Stack:** Rust (app-core handlers, cognitive repos, bus events), TypeScript/React (desktop-ui components), SQLite (schema changes), LanceDB (card embeddings), fastembed (semantic pre-filter), LLM provider (grading + Socratic + distractors)

**Spec:** `docs/superpowers/specs/2026-03-22-active-recall-flashcard-design.md`

---

## File Structure

### Rust backend — new files

| File | Responsibility |
|------|---------------|
| `crates/app-core/src/handlers/notes/grading.rs` | `flashcard_submit_answer` + `flashcard_explain_answer` handlers, grading pipeline (exact → semantic → LLM), score-to-rating mapping |
| `crates/app-core/src/handlers/notes/review_session.rs` | `flashcard_save_session`, `flashcard_get_session` handlers, session persistence |
| `crates/app-core/src/handlers/notes/graph_propagation.rs` | FIRe algorithm, prerequisite finder, `flashcard_get_prerequisites` handler |
| `crates/app-core/src/handlers/notes/distractors.rs` | `flashcard_generate_distractors` handler, LLM prompt + post-filter |
| `crates/cognitive/src/repos/review_session.rs` | `ReviewSessionRepo` — CRUD for `review_sessions` table |
| `crates/cognitive/src/repos/deck_preference.rs` | `DeckPreferenceRepo` — CRUD for `deck_preferences` table |

### Rust backend — modified files

| File | Changes |
|------|---------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add 5 columns to `flashcards`, add `deck_preferences` + `review_sessions` tables |
| `crates/cognitive/src/repos/flashcard.rs` | Update `FlashcardRow` + `NewFlashcard` structs, add `find_related_cards`, `apply_propagation_boost` methods |
| `crates/cognitive/src/repos/mod.rs` | Re-export new repos |
| `crates/cognitive/src/lib.rs` | Re-export new repos |
| `crates/bus/src/domain_events.rs` | Add `FlashcardSessionCompleted` variant |
| `crates/config/src/schema/learning.rs` | Add `ActiveRecallConfig` nested struct |
| `crates/desktop-shared/src/commands/notes.rs` | Add `FlashcardSubmitAnswerParams`, `GradeResultResponse`, `FlashcardExplainParams`, `FlashcardDistractorParams`, `ReviewSessionResponse` types |
| `crates/desktop/src/commands/notes.rs` | Add Tauri command wrappers + `DEV_COMMANDS` entries |
| `crates/desktop/src/dev_server/mod.rs` | Add dev dispatch entries |
| `crates/app-core/src/handlers/notes/mod.rs` | Add `pub mod grading; pub mod review_session; pub mod graph_propagation; pub mod distractors;` |
| `crates/app-core/src/handlers/notes/card_generation.rs` | Update `flashcard_save_generated` to embed cards + use `difficulty_estimate` |
| `crates/feature-learning/src/card_generator.rs` | Add `difficulty_estimate` + `prerequisite_concepts` to generation prompt |
| `crates/feature-learning/src/types.rs` | Add fields to `GeneratedCard` |
| `crates/storage/src/vector_store/mod.rs` | Register `flashcard_embeddings` table schema |

### TypeScript frontend — new files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/hooks/useActiveReview.ts` | State machine hook replacing `useFlashcards` |
| `desktop-ui/src/features/notes/components/review/ActiveReviewSession.tsx` | Root component, layout switching (compact/fullscreen) |
| `desktop-ui/src/features/notes/components/review/ReviewCard.tsx` | Card display orchestrator (front → input → grade) |
| `desktop-ui/src/features/notes/components/review/CardFront.tsx` | Question display with type badge and deck label |
| `desktop-ui/src/features/notes/components/review/TypedAnswerInput.tsx` | Textarea with char count, Enter to submit |
| `desktop-ui/src/features/notes/components/review/SelfGradeInput.tsx` | Current show/reveal/rate flow (escape hatch) |
| `desktop-ui/src/features/notes/components/review/MultipleChoiceInput.tsx` | 4-option selector with AI distractors |
| `desktop-ui/src/features/notes/components/review/ClozeInput.tsx` | Inline fill-in-the-blank with fuzzy matching |
| `desktop-ui/src/features/notes/components/review/VoiceInput.tsx` | Record button, waveform, live transcript |
| `desktop-ui/src/features/notes/components/review/GradeDisplay.tsx` | Score badge, diff highlights, expected answer, Socratic suggestion |
| `desktop-ui/src/features/notes/components/review/GradeActions.tsx` | Confirm rating, override, explain, save as insight, jump to source |
| `desktop-ui/src/features/notes/components/review/ModeSelector.tsx` | Per-deck/per-card mode toggle |
| `desktop-ui/src/features/notes/components/review/SocraticPanel.tsx` | Deep-dive explanation panel |
| `desktop-ui/src/features/notes/components/review/SessionProgress.tsx` | Progress bar, remaining count, avg score |
| `desktop-ui/src/features/notes/components/review/SessionSummary.tsx` | End screen: timed beats, reflection pulse, actions |
| `desktop-ui/src/features/notes/components/review/PropagationRipple.tsx` | Inline graph propagation notification |

### TypeScript frontend — modified files

| File | Changes |
|------|---------|
| `desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx` | Replace internals with `ActiveReviewSession layout="compact"` |
| `desktop-ui/src/features/notes/hooks/useFlashcards.ts` | Deprecate (replaced by `useActiveReview`) |
| `desktop-ui/src/shared/types/notes.ts` | Add `GradeResult`, `AnswerMode`, `SessionSummary` types |

---

## Task 1: Schema + Struct Foundation

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql:490-530`
- Modify: `crates/cognitive/src/repos/flashcard.rs:56-98`
- Create: `crates/cognitive/src/repos/review_session.rs`
- Create: `crates/cognitive/src/repos/deck_preference.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/lib.rs`
- Modify: `crates/bus/src/domain_events.rs:329-360`
- Modify: `crates/config/src/schema/learning.rs`
- Test: `crates/cognitive/src/repos/flashcard.rs` (existing tests)

- [ ] **Step 1: Add new columns to flashcards DDL**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, add to the `flashcards` CREATE TABLE (after `recall_speed_ms`):

```sql
    back_embedding_updated_at TEXT,
    preferred_mode TEXT,
    difficulty_estimate INTEGER,
    prerequisite_concepts TEXT,
    card_distractors TEXT,
```

- [ ] **Step 2: Add deck_preferences and review_sessions tables**

Append to the same migration file after the `review_log` section:

```sql
-- Active recall: deck mode preferences
CREATE TABLE IF NOT EXISTS deck_preferences (
    deck TEXT PRIMARY KEY,
    answer_mode TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Active recall: review session history
CREATE TABLE IF NOT EXISTS review_sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    cards_reviewed INTEGER DEFAULT 0,
    avg_score REAL,
    duration_seconds INTEGER,
    modes_used TEXT,
    propagation_count INTEGER DEFAULT 0,
    weak_card_ids TEXT,
    session_data TEXT,
    status TEXT DEFAULT 'active'
);

CREATE INDEX IF NOT EXISTS idx_review_sessions_status ON review_sessions(status);
CREATE INDEX IF NOT EXISTS idx_review_sessions_started ON review_sessions(started_at);
```

- [ ] **Step 3: Update FlashcardRow struct**

In `crates/cognitive/src/repos/flashcard.rs`, add fields to `FlashcardRow` (after `recall_speed_ms`):

```rust
    pub back_embedding_updated_at: Option<String>,
    pub preferred_mode: Option<String>,
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<String>,
    pub card_distractors: Option<String>,
```

- [ ] **Step 4: Update NewFlashcard struct**

Add to `NewFlashcard`:

```rust
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<String>,
```

- [ ] **Step 5: Update create_batch SQL**

In `FlashcardRepo::create_batch` (around line 137), update the INSERT statement to include the new columns. Set `back_embedding_updated_at`, `preferred_mode`, `card_distractors` to NULL. Use `card.difficulty_estimate` and `card.prerequisite_concepts` from the input.

- [ ] **Step 6: Update all existing NewFlashcard construction sites**

Search for `NewFlashcard {` across the codebase. Add `difficulty_estimate: None, prerequisite_concepts: None` to each construction site. Key locations:
- `crates/app-core/src/handlers/notes/card_generation.rs` (flashcard_save_generated)
- `crates/app-core/src/handlers/notes/insight.rs` (insight_save_flashcards)
- `crates/app-core/src/handlers/notes/practice.rs` (save weak units)
- Any test files constructing `NewFlashcard`

- [ ] **Step 7: Add DomainEvent::FlashcardSessionCompleted**

In `crates/bus/src/domain_events.rs`, add variant to the `DomainEvent` enum (after `PracticeSessionCompleted`):

```rust
    FlashcardSessionCompleted {
        session_id: String,
        cards_reviewed: usize,
        avg_score: f64,
        weak_domains: Vec<String>,
        propagation_count: usize,
    },
```

- [ ] **Step 8: Add ActiveRecallConfig to LearningConfig**

In `crates/config/src/schema/learning.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRecallConfig {
    #[serde(default = "default_semantic_auto_accept")]
    pub semantic_auto_accept_threshold: f64,
    #[serde(default = "default_semantic_auto_fail")]
    pub semantic_auto_fail_threshold: f64,
    #[serde(default = "default_graph_propagation_strength")]
    pub graph_propagation_strength: String,
    #[serde(default = "default_graph_propagation_daily_cap")]
    pub graph_propagation_daily_cap: usize,
    #[serde(default = "default_answer_mode")]
    pub default_answer_mode: String,
}

fn default_semantic_auto_accept() -> f64 { 0.78 }
fn default_semantic_auto_fail() -> f64 { 0.45 }
fn default_graph_propagation_strength() -> String { "gentle".into() }
fn default_graph_propagation_daily_cap() -> usize { 15 }
fn default_answer_mode() -> String { "auto".into() }
```

Add field to `LearningConfig`:

```rust
    #[serde(default)]
    pub active_recall: ActiveRecallConfig,
```

- [ ] **Step 9: Create ReviewSessionRepo**

Create `crates/cognitive/src/repos/review_session.rs`:

```rust
use sqlx::SqlitePool;
use crate::repos::flashcard::FlashcardRow;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReviewSessionRow {
    pub id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub cards_reviewed: i32,
    pub avg_score: Option<f64>,
    pub duration_seconds: Option<i32>,
    pub modes_used: Option<String>,
    pub propagation_count: i32,
    pub weak_card_ids: Option<String>,
    pub session_data: Option<String>,
    pub status: String,
}

pub struct ReviewSessionRepo {
    pool: SqlitePool,
}

impl ReviewSessionRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn create(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO review_sessions (id, started_at, status) VALUES (?1, ?2, 'active')")
            .bind(id).bind(&now)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn complete(&self, id: &str, data: &str, cards_reviewed: i32, avg_score: f64, duration_seconds: i32, modes_used: &str, propagation_count: i32, weak_card_ids: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE review_sessions SET completed_at = ?1, cards_reviewed = ?2, avg_score = ?3, duration_seconds = ?4, modes_used = ?5, propagation_count = ?6, weak_card_ids = ?7, session_data = ?8, status = 'completed' WHERE id = ?9")
            .bind(&now).bind(cards_reviewed).bind(avg_score).bind(duration_seconds)
            .bind(modes_used).bind(propagation_count).bind(weak_card_ids).bind(data).bind(id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn abandon(&self, id: &str, cards_reviewed: i32) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE review_sessions SET completed_at = ?1, cards_reviewed = ?2, status = 'abandoned' WHERE id = ?3")
            .bind(&now).bind(cards_reviewed).bind(id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_active(&self) -> Result<Option<ReviewSessionRow>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM review_sessions WHERE status = 'active' ORDER BY started_at DESC LIMIT 1")
            .fetch_optional(&self.pool).await
    }
}
```

- [ ] **Step 10: Create DeckPreferenceRepo**

Create `crates/cognitive/src/repos/deck_preference.rs`:

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeckPreferenceRow {
    pub deck: String,
    pub answer_mode: String,
    pub updated_at: String,
}

pub struct DeckPreferenceRepo {
    pool: SqlitePool,
}

impl DeckPreferenceRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn get(&self, deck: &str) -> Result<Option<DeckPreferenceRow>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM deck_preferences WHERE deck = ?1")
            .bind(deck).fetch_optional(&self.pool).await
    }

    pub async fn set(&self, deck: &str, mode: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO deck_preferences (deck, answer_mode, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(deck) DO UPDATE SET answer_mode = ?2, updated_at = ?3")
            .bind(deck).bind(mode).bind(&now)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_all(&self) -> Result<Vec<DeckPreferenceRow>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM deck_preferences ORDER BY deck")
            .fetch_all(&self.pool).await
    }
}
```

- [ ] **Step 11: Wire new repos in mod.rs and lib.rs**

In `crates/cognitive/src/repos/mod.rs`, add:
```rust
pub mod review_session;
pub mod deck_preference;
pub use review_session::{ReviewSessionRepo, ReviewSessionRow};
pub use deck_preference::{DeckPreferenceRepo, DeckPreferenceRow};
```

In `crates/cognitive/src/lib.rs`, add the re-exports.

- [ ] **Step 11b: Add new repos to AppCore state**

In `crates/app-core/src/state.rs`, add new fields to `AppCore`:

```rust
    pub review_session_repo: Option<cognitive::ReviewSessionRepo>,
    pub deck_preference_repo: Option<cognitive::DeckPreferenceRepo>,
```

Add accessor methods following the existing `flashcard_repo()` pattern:

```rust
    pub fn review_session_repo(&self) -> Result<&cognitive::ReviewSessionRepo, ApiError> {
        self.review_session_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Review session repo not available"))
    }

    pub fn deck_preference_repo(&self) -> Result<&cognitive::DeckPreferenceRepo, ApiError> {
        self.deck_preference_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Deck preference repo not available"))
    }
```

Wire them in the `AppCore` builder where `flashcard_repo` is initialized.

Add `flashcard_save_mode_preference` and `flashcard_get_mode_preference` handler methods to `AppCore` that delegate to `deck_preference_repo`.

- [ ] **Step 11c: Add Default impl for ActiveRecallConfig**

In `crates/config/src/schema/learning.rs`, add:

```rust
impl Default for ActiveRecallConfig {
    fn default() -> Self {
        Self {
            semantic_auto_accept_threshold: default_semantic_auto_accept(),
            semantic_auto_fail_threshold: default_semantic_auto_fail(),
            graph_propagation_strength: default_graph_propagation_strength(),
            graph_propagation_daily_cap: default_graph_propagation_daily_cap(),
            default_answer_mode: default_answer_mode(),
        }
    }
}
```

And add `active_recall: ActiveRecallConfig::default()` to the `LearningConfig::default()` impl.

- [ ] **Step 12: Run tests and verify**

Run: `cargo nextest run -p cognitive`

Expected: All existing tests pass. The schema changes are pre-release consolidated, so existing `connect_in_memory()` tests will pick up the new DDL.

- [ ] **Step 13: Commit**

```bash
git add -A && git commit -m "feat(active-recall): add schema, repos, config, and domain event foundation"
```

---

## Task 2: Card Embedding Pipeline

**Files:**
- Modify: `crates/storage/src/vector_store/mod.rs`
- Modify: `crates/app-core/src/handlers/notes/card_generation.rs`
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs`
- Test: inline tests

- [ ] **Step 1: Register flashcard_embeddings table in VectorStore**

In `crates/storage/src/vector_store/mod.rs`, add `flashcard_embeddings` to the table initialization list (follow the pattern of `note_embeddings`). The schema: `id TEXT, vector FixedSizeList<Float32, 384>, card_id TEXT, side TEXT, timestamp TEXT`.

- [ ] **Step 2: Add embed_flashcard_batch helper to app-core**

In `crates/app-core/src/handlers/notes/card_generation.rs`, add a helper function:

```rust
/// Embed both front and back of flashcards into LanceDB.
/// Uses Arc<EmbeddingEngine> because embed_async takes Arc<Self> as receiver.
async fn embed_flashcard_batch(
    embedding_engine: Arc<EmbeddingEngine>,
    vector_store: &VectorStore,
    cards: &[(String, String, String)], // (card_id, front, back)
) -> Result<()> {
    for (card_id, front, back) in cards {
        let front_text = common::truncate_at_boundary(front, 2000);
        if let Ok(vec) = embedding_engine.clone().embed_async(front_text.to_string()).await {
            let id = format!("{card_id}_front");
            let extras: &[(&str, &str)] = &[("card_id", card_id), ("side", "front")];
            let _ = vector_store.upsert_embedding("flashcard_embeddings", &id, &vec, extras).await;
        }
        let back_text = common::truncate_at_boundary(back, 2000);
        if let Ok(vec) = embedding_engine.clone().embed_async(back_text.to_string()).await {
            let id = format!("{card_id}_back");
            let extras: &[(&str, &str)] = &[("card_id", card_id), ("side", "back")];
            let _ = vector_store.upsert_embedding("flashcard_embeddings", &id, &vec, extras).await;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Call embedding in flashcard_save_generated**

After the `FlashcardRepo::create_batch` call in `flashcard_save_generated`, spawn a background task to embed all saved cards:

```rust
let cards_to_embed: Vec<_> = saved_cards.iter()
    .map(|c| (c.id.clone(), c.front.clone(), c.back.clone()))
    .collect();
let engine = self.embedding_engine.clone();
let store = self.vector_store.clone();
tokio::spawn(async move {
    let _ = embed_flashcard_batch(&*engine, &store, &cards_to_embed).await;
});
```

Update the `back_embedding_updated_at` column after successful embedding.

- [ ] **Step 4: Also embed on single card create**

In `flashcard_create` handler, do the same for single card creation.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p app-core -E 'test(flashcard)'`

Expected: Existing tests still pass. Embedding is fire-and-forget so tests won't wait for it.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(active-recall): add card embedding pipeline for semantic grading"
```

---

## Task 3: Grading Pipeline Handler

**Files:**
- Create: `crates/app-core/src/handlers/notes/grading.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Test: inline tests in `grading.rs`

- [ ] **Step 1: Define shared types in desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardSubmitAnswerParams {
    pub card_id: String,
    pub user_answer: String,
    pub mode: String, // "typed" | "voice" | "cloze_fill"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeResultResponse {
    pub score: Option<f64>,
    pub suggested_rating: String,
    pub grading_method: String,
    pub explanation: Option<String>,
    pub diff_highlights: Vec<DiffSegmentResponse>,
    pub expected_answer: String,
    pub coaching_nudge: Option<String>,
    pub socratic_suggestion: Option<String>,
    pub key_concepts_present: Vec<String>,
    pub key_concepts_missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSegmentResponse {
    pub text: String,
    pub status: String, // "match" | "missing" | "extra" | "partial"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardExplainParams {
    pub card_id: String,
    pub user_answer: String,
    pub grade_explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardExplainResponse {
    pub explanation: String,
    pub saved_as_memory: bool,
}
```

- [ ] **Step 2: Write failing test for score_to_rating mapping**

In `crates/app-core/src/handlers/notes/grading.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_to_rating() {
        assert_eq!(score_to_rating(1.0), "easy");
        assert_eq!(score_to_rating(0.85), "easy");
        assert_eq!(score_to_rating(0.84), "good");
        assert_eq!(score_to_rating(0.60), "good");
        assert_eq!(score_to_rating(0.59), "hard");
        assert_eq!(score_to_rating(0.30), "hard");
        assert_eq!(score_to_rating(0.29), "again");
        assert_eq!(score_to_rating(0.0), "again");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p app-core -E 'test(score_to_rating)'`

Expected: FAIL — function not defined.

- [ ] **Step 4: Implement score_to_rating**

```rust
pub fn score_to_rating(score: f64) -> &'static str {
    if score >= 0.85 { "easy" }
    else if score >= 0.60 { "good" }
    else if score >= 0.30 { "hard" }
    else { "again" }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo nextest run -p app-core -E 'test(score_to_rating)'`

Expected: PASS.

- [ ] **Step 6: Write failing test for exact_match grading**

```rust
#[test]
fn test_exact_match_grading() {
    let result = grade_exact_match("  The forgetting curve  ", "the forgetting curve");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 1.0);

    let result = grade_exact_match("wrong answer", "the forgetting curve");
    assert!(result.is_none());
}
```

- [ ] **Step 7: Implement grade_exact_match**

```rust
fn grade_exact_match(user_answer: &str, expected: &str) -> Option<f64> {
    if user_answer.trim().to_lowercase() == expected.trim().to_lowercase() {
        Some(1.0)
    } else {
        None
    }
}
```

- [ ] **Step 8: Write and implement grade_semantic (cosine comparison)**

This function takes cosine similarity and config thresholds, returns score or None (meaning "proceed to LLM"):

```rust
fn grade_semantic(cosine_sim: f64, accept_threshold: f64, fail_threshold: f64) -> Option<f64> {
    if cosine_sim >= accept_threshold {
        // Scale 0.85-1.0 based on how far above threshold
        let bonus = (cosine_sim - accept_threshold) / (1.0 - accept_threshold);
        Some(0.85 + bonus * 0.15)
    } else if cosine_sim <= fail_threshold {
        Some(0.15)
    } else {
        None // Borderline — needs LLM
    }
}
```

Test it:

```rust
#[test]
fn test_semantic_grading() {
    // Above accept threshold
    assert!(grade_semantic(0.90, 0.78, 0.45).unwrap() > 0.85);
    // Below fail threshold
    assert_eq!(grade_semantic(0.30, 0.78, 0.45), Some(0.15));
    // Borderline — None
    assert_eq!(grade_semantic(0.60, 0.78, 0.45), None);
}
```

- [ ] **Step 9: Implement the LLM grading prompt builder**

```rust
fn build_grading_prompt(front: &str, back: &str, user_answer: &str, source_context: Option<&str>) -> (String, String) {
    let system = r#"You are grading a flashcard answer. Compare the user's answer against the expected answer.
Return ONLY a JSON object with these fields:
- score: float 0.0-1.0 (how well the answer captures the key concepts)
- explanation: string (brief explanation of what was right/wrong)
- key_concepts_present: array of strings (concepts the user got right)
- key_concepts_missing: array of strings (concepts the user missed)
- coaching_nudge: string or null (optional encouraging tip)
- socratic_suggestion: string or null (optional Socratic question to deepen understanding)"#;

    let mut user_prompt = format!(
        "Question: {front}\nExpected answer: {back}\nUser's answer: {user_answer}"
    );
    if let Some(ctx) = source_context {
        user_prompt.push_str(&format!("\nSource context: {ctx}"));
    }

    (system.to_string(), user_prompt)
}
```

- [ ] **Step 10: Implement the full flashcard_submit_answer handler**

In `grading.rs`, implement the `AppCore` method following the `practice_submit_unit` pattern:

```rust
impl AppCore {
    pub async fn flashcard_submit_answer(
        &self,
        params: FlashcardSubmitAnswerParams,
    ) -> Result<GradeResultResponse, ApiError> {
        // 1. Fetch card (get_by_id returns Option — unwrap or error)
        let repo = self.flashcard_repo()?;
        let card = repo.get_by_id(&params.card_id).await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Card not found"))?;

        // 2. Try exact match
        if let Some(score) = grade_exact_match(&params.user_answer, &card.back) {
            return Ok(build_grade_response(score, "exact_match", &card, None));
        }

        // 3. Semantic pre-filter
        let config = self.config.read().await;
        let active_recall = &config.learning.active_recall;
        let accept_threshold = active_recall.semantic_auto_accept_threshold;
        let fail_threshold = active_recall.semantic_auto_fail_threshold;
        drop(config);

        let cosine_sim = self.compute_answer_similarity(&params.card_id, &params.user_answer).await;

        if let Some(score) = grade_semantic(cosine_sim, accept_threshold, fail_threshold) {
            return Ok(build_grade_response(score, "semantic_auto", &card, None));
        }

        // 4. LLM grading (borderline) — follows practice_submit_unit pattern
        let (system_prompt, user_prompt) = build_grading_prompt(
            &card.front, &card.back, &params.user_answer, card.source_context.as_deref(),
        );
        let provider = self.cognitive_provider.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;
        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let messages = vec![
            providers::Message::System { content: system_prompt },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider.chat(&messages, None, &chat_params).await;

        match response {
            Ok(llm_response) => {
                let text = llm_response.content
                    .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;
                let cleaned = common::helpers::strip_llm_fences(&text);
                match serde_json::from_str::<LlmGradeResponse>(cleaned) {
                    Ok(llm_result) => Ok(build_grade_response_from_llm(llm_result, &card)),
                    Err(_) => {
                        // Parse failure — fall back to semantic score
                        Ok(build_grade_response(
                            scale_semantic_to_score(cosine_sim), "semantic_auto", &card,
                            Some("AI grading temporarily unavailable — using semantic match instead."),
                        ))
                    }
                }
            }
            Err(_) => {
                // LLM failure — fall back to semantic
                Ok(build_grade_response(
                    scale_semantic_to_score(cosine_sim), "semantic_auto", &card,
                    Some("AI grading temporarily unavailable — using semantic match instead."),
                ))
            }
        }
    }
}
```

The `compute_answer_similarity` method embeds the user answer via `EmbeddingEngine` (using `Arc<Self>::embed_async(text)`), then searches `flashcard_embeddings` for `{card_id}_back` and computes cosine similarity.

- [ ] **Step 11: Implement flashcard_explain_answer handler**

```rust
impl AppCore {
    pub async fn flashcard_explain_answer(
        &self,
        params: FlashcardExplainParams,
    ) -> Result<FlashcardExplainResponse, ApiError> {
        let repo = self.flashcard_repo()?;
        let card = repo.get_by_id(&params.card_id).await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Card not found"))?;

        let system = "You are a Socratic tutor. The student answered a flashcard question and received feedback. Help them understand the gap in their knowledge through guided questioning and gentle coaching. Be encouraging but precise.".to_string();
        let user_prompt = format!(
            "Question: {}\nExpected answer: {}\nStudent's answer: {}\nPrevious feedback: {}\n\nHelp the student understand what they missed and why it matters.",
            card.front, card.back, params.user_answer, params.grade_explanation
        );

        let provider = self.cognitive_provider.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;
        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider.chat(&messages, None, &chat_params).await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;
        let text = response.content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        Ok(FlashcardExplainResponse {
            explanation: text,
            saved_as_memory: false, // TODO: auto-save as episodic memory for low scores
        })
    }
}
```

- [ ] **Step 12: Add module declaration**

In `crates/app-core/src/handlers/notes/mod.rs`, add:
```rust
pub mod grading;
```

- [ ] **Step 13: Run tests**

Run: `cargo nextest run -p app-core -E 'test(grading)'`

Expected: All grading unit tests pass.

- [ ] **Step 14: Commit**

```bash
git add -A && git commit -m "feat(active-recall): implement grading pipeline (exact + semantic + LLM)"
```

---

## Task 4: Distractor Generation Handler

**Files:**
- Create: `crates/app-core/src/handlers/notes/distractors.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`

- [ ] **Step 1: Add shared types**

In `crates/desktop-shared/src/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardDistractorParams {
    pub card_id: String,
    #[serde(default = "default_distractor_count")]
    pub count: usize,
}
fn default_distractor_count() -> usize { 3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardDistractorResponse {
    pub distractors: Vec<String>,
    pub cached: bool,
}
```

- [ ] **Step 2: Implement distractor generation**

Create `crates/app-core/src/handlers/notes/distractors.rs`:

```rust
impl AppCore {
    pub async fn flashcard_generate_distractors(
        &self,
        params: FlashcardDistractorParams,
    ) -> Result<FlashcardDistractorResponse, ApiError> {
        let repo = self.flashcard_repo()?;
        let card = repo.get_by_id(&params.card_id).await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Card not found"))?;

        // Check cache first
        if let Some(cached) = &card.card_distractors {
            if let Ok(distractors) = serde_json::from_str::<Vec<String>>(cached) {
                if !distractors.is_empty() {
                    return Ok(FlashcardDistractorResponse { distractors, cached: true });
                }
            }
        }

        // Generate via LLM — follows practice_submit_unit pattern
        let system = "Generate plausible but incorrect distractors for a multiple-choice flashcard. Each distractor must be: same length and style as the correct answer, semantically related but clearly wrong, not a trick answer. Return JSON: {\"distractors\": [\"...\", \"...\", \"...\"]}".to_string();
        let user_prompt = format!(
            "Question: {}\nCorrect answer: {}\nSource context: {}\nGenerate {} distractors.",
            card.front, card.back, card.source_context.as_deref().unwrap_or(""), params.count
        );

        let provider = self.cognitive_provider.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;
        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider.chat(&messages, None, &chat_params).await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;
        let text = response.content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let cleaned = common::helpers::strip_llm_fences(&text);
        let parsed: serde_json::Value = serde_json::from_str(cleaned)
            .map_err(|e| ApiError::new("PARSE_ERROR", e.to_string()))?;
        let distractors: Vec<String> = parsed["distractors"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        // Cache in DB
        let cache_json = serde_json::to_string(&distractors)
            .map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string()))?;
        repo.update_distractors(&params.card_id, &cache_json).await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        Ok(FlashcardDistractorResponse { distractors, cached: false })
    }
}
```

- [ ] **Step 3: Add update_distractors repo method**

In `crates/cognitive/src/repos/flashcard.rs`, add:

```rust
pub async fn update_distractors(&self, id: &str, distractors_json: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE flashcards SET card_distractors = ?1 WHERE id = ?2")
        .bind(distractors_json).bind(id)
        .execute(&self.pool).await?;
    Ok(())
}
```

- [ ] **Step 4: Add module declaration and commit**

Add `pub mod distractors;` to `mod.rs`.

```bash
git add -A && git commit -m "feat(active-recall): add distractor generation for multiple choice mode"
```

---

## Task 5: Tauri Command Wiring

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Add Tauri command wrappers**

In `crates/desktop/src/commands/notes.rs`, add commands following the existing pattern (thin wrappers that delegate to `AppCore`):

```rust
#[tauri::command]
pub async fn flashcard_submit_answer(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardSubmitAnswerParams,
) -> Result<GradeResultResponse, ApiError> {
    state.flashcard_submit_answer(params).await
}

#[tauri::command]
pub async fn flashcard_explain_answer(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardExplainParams,
) -> Result<FlashcardExplainResponse, ApiError> {
    state.flashcard_explain_answer(params).await
}

#[tauri::command]
pub async fn flashcard_generate_distractors(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardDistractorParams,
) -> Result<FlashcardDistractorResponse, ApiError> {
    state.flashcard_generate_distractors(params).await
}

#[tauri::command]
pub async fn flashcard_save_mode_preference(
    state: State<'_, Arc<AppCore>>,
    deck: String,
    mode: String,
) -> Result<(), ApiError> {
    state.flashcard_save_mode_preference(&deck, &mode).await
}

#[tauri::command]
pub async fn flashcard_get_mode_preference(
    state: State<'_, Arc<AppCore>>,
    deck: String,
) -> Result<Option<DeckPreferenceResponse>, ApiError> {
    state.flashcard_get_mode_preference(&deck).await
}

#[tauri::command]
pub async fn flashcard_save_session(
    state: State<'_, Arc<AppCore>>,
    params: ReviewSessionSaveParams,
) -> Result<(), ApiError> {
    state.flashcard_save_session(params).await
}

#[tauri::command]
pub async fn flashcard_get_prerequisites(
    state: State<'_, Arc<AppCore>>,
    card_id: String,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_get_prerequisites(&card_id).await
}
```

- [ ] **Step 2: Add to DEV_COMMANDS**

Add all new command names to the `DEV_COMMANDS` const in the same file.

- [ ] **Step 3: Add dev server dispatch entries**

In `crates/desktop/src/dev_server/mod.rs`, add dispatch entries for each new command.

- [ ] **Step 4: Register commands in Tauri builder**

Add all new commands to the `tauri::Builder::default().invoke_handler(tauri::generate_handler![...])` list.

- [ ] **Step 5: Run the dev server coverage test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`

Expected: PASS — all new commands are in both `DEV_COMMANDS` and dev server dispatch.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(active-recall): wire Tauri commands and dev server dispatch"
```

---

## Task 6: Difficulty Estimation in Card Generation

**Files:**
- Modify: `crates/feature-learning/src/card_generator.rs`
- Modify: `crates/feature-learning/src/types.rs`
- Modify: `crates/app-core/src/handlers/notes/card_generation.rs`

- [ ] **Step 1: Add fields to GeneratedCard**

In `crates/feature-learning/src/types.rs`, add to `GeneratedCard`:

```rust
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<Vec<String>>,
```

- [ ] **Step 2: Update generation prompt**

In `crates/feature-learning/src/card_generator.rs`, update the system prompt in `build_generation_prompt` to add:

```
For each card, also include:
- "difficulty_estimate": integer 1-5 (1=recall a single fact, 2=understand a concept, 3=apply knowledge, 4=analyze relationships, 5=synthesize multiple concepts)
- "prerequisite_concepts": array of 0-3 strings naming concepts the learner should already know to answer this card
```

- [ ] **Step 3: Map difficulty to initial FSRS-5 parameters in save handler**

In `crates/app-core/src/handlers/notes/card_generation.rs`, update `flashcard_save_generated` to use `difficulty_estimate` when constructing `NewFlashcard`:

```rust
fn difficulty_to_fsrs(estimate: Option<i32>) -> (f64, f64) {
    match estimate.unwrap_or(3) {
        1 => (4.0, 3.0),
        2 => (3.0, 4.0),
        3 => (2.0, 5.0),
        4 => (1.2, 6.5),
        _ => (0.8, 8.0), // 5+
    }
}
```

- [ ] **Step 4: Run existing card generation tests**

Run: `cargo nextest run -p feature-learning`

Expected: PASS (new fields are `Option`, so existing JSON parsing won't break).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(active-recall): add difficulty estimation to card generation"
```

---

## Task 7: Knowledge Graph Propagation

**Files:**
- Create: `crates/app-core/src/handlers/notes/graph_propagation.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`
- Modify: `crates/cognitive/src/repos/flashcard.rs`
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs`

- [ ] **Step 1: Add find_related_cards to FlashcardRepo**

In `crates/cognitive/src/repos/flashcard.rs`:

```rust
/// Find cards related to the given card via note_links.
pub async fn find_cards_linked_by_notes(&self, source_note_id: &str, exclude_card_id: &str) -> Result<Vec<FlashcardRow>, sqlx::Error> {
    sqlx::query_as::<_, FlashcardRow>(
        r#"SELECT f.* FROM flashcards f
           INNER JOIN note_links nl ON f.source_note_id = nl.target_id OR f.source_note_id = nl.source_id
           WHERE (nl.source_id = ?1 OR nl.target_id = ?1)
             AND f.id != ?2
             AND f.suspended = 0
           GROUP BY f.id
           LIMIT 20"#
    )
    .bind(source_note_id).bind(exclude_card_id)
    .fetch_all(&self.pool).await
}

/// Find cards sharing the same atom domain.
pub async fn find_cards_same_domain(&self, atom_id: &str, exclude_card_id: &str) -> Result<Vec<FlashcardRow>, sqlx::Error> {
    sqlx::query_as::<_, FlashcardRow>(
        r#"SELECT f.* FROM flashcards f
           INNER JOIN knowledge_atoms ka ON f.atom_id = ka.id
           INNER JOIN knowledge_atoms source_ka ON source_ka.id = ?1
           WHERE ka.domain = source_ka.domain
             AND f.id != ?2
             AND f.suspended = 0
           LIMIT 20"#
    )
    .bind(atom_id).bind(exclude_card_id)
    .fetch_all(&self.pool).await
}

/// Apply fractional boost: extend due_at by a fraction of the current interval.
pub async fn apply_propagation_boost(&self, card_id: &str, boost_fraction: f64) -> Result<(), sqlx::Error> {
    // Only boost cards due within 48 hours, cap at 20% of interval
    let capped = boost_fraction.min(0.20);
    sqlx::query(
        r#"UPDATE flashcards
           SET due_at = datetime(due_at, '+' || CAST(ROUND(
               MAX(0, MIN(?1, 0.20)) * ROUND(julianday(due_at) - julianday(COALESCE(last_reviewed_at, created_at))) * 86400
           ) AS INTEGER) || ' seconds')
           WHERE id = ?2
             AND due_at IS NOT NULL
             AND julianday(due_at) - julianday('now') <= 2.0"#
    )
    .bind(capped).bind(card_id)
    .execute(&self.pool).await?;
    Ok(())
}
```

- [ ] **Step 2: Implement propagation algorithm**

Create `crates/app-core/src/handlers/notes/graph_propagation.rs`:

```rust
use common::Result;

impl AppCore {
    /// Run FIRe propagation after a card review. Returns count of boosted cards.
    pub async fn propagate_review(
        &self,
        card: &FlashcardRow,
        quality: &str, // "again" | "hard" | "good" | "easy"
    ) -> Result<usize> {
        let quality_factor = match quality {
            "easy" => 1.0,
            "good" => 0.8,
            "hard" => 0.3,
            _ => 0.0, // "again" — no positive propagation
        };

        if quality_factor == 0.0 && quality != "again" {
            return Ok(0);
        }

        let mut boosted = 0;
        let repo = self.flashcard_repo()?;

        // 1. Cards linked via note_links
        if let Some(note_id) = &card.source_note_id {
            let linked = repo.find_cards_linked_by_notes(note_id, &card.id).await.unwrap_or_default();
            for related in &linked {
                let boost = 1.0 * quality_factor * 0.15;
                repo.apply_propagation_boost(&related.id, boost).await.ok();
                boosted += 1;
            }
        }

        // 2. Cards sharing same atom domain
        if let Some(atom_id) = &card.atom_id {
            let domain_cards = repo.find_cards_same_domain(atom_id, &card.id).await.unwrap_or_default();
            for related in &domain_cards {
                let boost = 0.5 * quality_factor * 0.15;
                repo.apply_propagation_boost(&related.id, boost).await.ok();
                boosted += 1;
            }
        }

        // 3. Negative propagation on "again"
        if quality == "again" {
            if let Some(note_id) = &card.source_note_id {
                let linked = repo.find_cards_linked_by_notes(note_id, &card.id).await.unwrap_or_default();
                for related in &linked {
                    repo.apply_propagation_penalty(&related.id, 0.08).await.ok();
                }
            }
        }

        Ok(boosted)
    }

    /// Find prerequisite cards for injection on wrong answers.
    pub async fn flashcard_get_prerequisites(&self, card_id: &str) -> Result<Vec<FlashcardResponse>> {
        let card = self.repos.flashcard().get_by_id(card_id).await?;
        let mut prerequisites = Vec::new();

        if let Some(note_id) = &card.source_note_id {
            // Find cards whose source notes are linked FROM this card's source note
            let linked = self.repos.flashcard()
                .find_cards_linked_by_notes(note_id, card_id).await
                .unwrap_or_default();

            for related in linked.into_iter().take(3) {
                // Prefer cards that are due soon
                prerequisites.push(flashcard_to_response(related));
            }
        }

        Ok(prerequisites)
    }
}
```

- [ ] **Step 3: Add apply_propagation_penalty to FlashcardRepo**

```rust
pub async fn apply_propagation_penalty(&self, card_id: &str, max_penalty: f64) -> Result<(), sqlx::Error> {
    let penalty = max_penalty.min(0.08);
    sqlx::query(
        r#"UPDATE flashcards
           SET due_at = datetime(due_at, '-' || CAST(ROUND(
               MIN(?1, 0.08) * ROUND(julianday(due_at) - julianday(COALESCE(last_reviewed_at, created_at))) * 86400
           ) AS INTEGER) || ' seconds')
           WHERE id = ?2
             AND due_at IS NOT NULL
             AND julianday(due_at) - julianday('now') <= 3.0"#
    )
    .bind(penalty).bind(card_id)
    .execute(&self.pool).await?;
    Ok(())
}
```

- [ ] **Step 4: Trigger propagation after record_review**

In `crates/app-core/src/handlers/notes/flashcard.rs`, after the `flashcard_record_review` handler calls `FlashcardRepo::record_review`, spawn a background task:

```rust
let card_clone = updated_card.clone();
let core = self.clone();
let quality_clone = params.quality.clone();
tokio::spawn(async move {
    let _ = core.propagate_review(&card_clone, &quality_clone).await;
});
```

- [ ] **Step 5: Add module declaration and run tests**

Add `pub mod graph_propagation;` to mod.rs.

Run: `cargo nextest run -p app-core -E 'test(flashcard)'`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(active-recall): implement knowledge graph propagation (FIRe)"
```

---

## Task 8: Frontend — useActiveReview Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useActiveReview.ts`
- Modify: `desktop-ui/src/shared/types/notes.ts`

- [ ] **Step 1: Add TypeScript types**

In `desktop-ui/src/shared/types/notes.ts`:

```typescript
export type AnswerMode = "typed" | "self_grade" | "multiple_choice" | "cloze_fill" | "voice" | "auto";

export interface GradeResult {
  score: number | null;
  suggestedRating: string;
  gradingMethod: string;
  explanation: string | null;
  diffHighlights: Array<{ text: string; status: "match" | "missing" | "extra" | "partial" }>;
  expectedAnswer: string;
  coachingNudge: string | null;
  socraticSuggestion: string | null;
  keyConceptsPresent: string[];
  keyConceptsMissing: string[];
}

export interface SessionStats {
  cardsReviewed: number;
  totalScore: number;
  modeUsage: Record<string, { count: number; totalScore: number }>;
  weakCards: Array<{ front: string; score: number }>;
  propagationCount: number;
  startTime: number;
}
```

- [ ] **Step 2: Implement useActiveReview hook**

Create `desktop-ui/src/features/notes/hooks/useActiveReview.ts`:

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";
import type { AnswerMode, GradeResult, SessionStats } from "@shared/types/notes";
import type { Flashcard, DeckSummary, ReviewQuality } from "./useFlashcards";

type SessionPhase = "idle" | "deck_picker" | "reviewing" | "complete";
type CardPhase = "answering" | "grading" | "graded" | "socratic" | "confirming";

export function useActiveReview() {
  const [phase, setPhase] = useState<SessionPhase>("idle");
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [cardPhase, setCardPhase] = useState<CardPhase>("answering");
  const [gradeResult, setGradeResult] = useState<GradeResult | null>(null);
  const [selectedMode, setSelectedMode] = useState<AnswerMode>("typed");
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [sessionId, setSessionId] = useState<string>("");
  const statsRef = useRef<SessionStats>({
    cardsReviewed: 0, totalScore: 0, modeUsage: {},
    weakCards: [], propagationCount: 0, startTime: Date.now(),
  });

  const fetchDecks = useCallback(async () => {
    const result = await ipc<DeckSummary[]>("flashcard_list_decks", {});
    setDecks(result);
    setPhase("deck_picker");
  }, []);

  const startReview = useCallback(async (deck: string) => {
    const due = await ipc<Flashcard[]>("flashcard_get_due", { deck, limit: 20 });
    if (due.length === 0) return;
    const id = crypto.randomUUID();
    setSessionId(id);
    setCards(due);
    setCurrentIndex(0);
    setCardPhase("answering");
    setGradeResult(null);
    statsRef.current = {
      cardsReviewed: 0, totalScore: 0, modeUsage: {},
      weakCards: [], propagationCount: 0, startTime: Date.now(),
    };
    setPhase("reviewing");

    // Load deck mode preference
    try {
      const pref = await ipc<{ answerMode: string } | null>("flashcard_get_mode_preference", { deck });
      if (pref?.answerMode) setSelectedMode(pref.answerMode as AnswerMode);
    } catch { /* use default */ }
  }, []);

  const submitAnswer = useCallback(async (userAnswer: string) => {
    const card = cards[currentIndex];
    if (!card) return;
    setCardPhase("grading");

    try {
      const result = await ipc<GradeResult>("flashcard_submit_answer", {
        cardId: card.id,
        userAnswer,
        mode: selectedMode,
      });
      setGradeResult(result);
      setCardPhase("graded");
    } catch {
      setCardPhase("answering");
    }
  }, [cards, currentIndex, selectedMode]);

  const confirmRating = useCallback(async (quality?: ReviewQuality) => {
    const card = cards[currentIndex];
    if (!card) return;
    setCardPhase("confirming");

    const finalQuality = quality ?? (gradeResult?.suggestedRating as ReviewQuality) ?? "good";
    await ipc("flashcard_record_review", {
      cardId: card.id,
      quality: finalQuality,
      recallSpeedMs: null,
    });

    // Track stats
    const stats = statsRef.current;
    stats.cardsReviewed += 1;
    if (gradeResult?.score != null) {
      stats.totalScore += gradeResult.score;
      if (gradeResult.score < 0.6) {
        stats.weakCards.push({ front: card.front, score: gradeResult.score });
      }
    }

    // Advance
    const nextIndex = currentIndex + 1;
    if (nextIndex >= cards.length) {
      setPhase("complete");
    } else {
      setCurrentIndex(nextIndex);
      setCardPhase("answering");
      setGradeResult(null);
    }
  }, [cards, currentIndex, gradeResult]);

  const requestExplanation = useCallback(async () => {
    setCardPhase("socratic");
  }, []);

  const switchMode = useCallback((mode: AnswerMode) => {
    setSelectedMode(mode);
  }, []);

  const skipCard = useCallback(() => {
    const nextIndex = currentIndex + 1;
    if (nextIndex >= cards.length) {
      setPhase("complete");
    } else {
      setCurrentIndex(nextIndex);
      setCardPhase("answering");
      setGradeResult(null);
    }
  }, [cards.length, currentIndex]);

  const current = cards[currentIndex] ?? null;
  const remaining = Math.max(0, cards.length - currentIndex);
  const avgScore = statsRef.current.cardsReviewed > 0
    ? statsRef.current.totalScore / statsRef.current.cardsReviewed
    : 0;

  return {
    phase, cardPhase, decks, current, remaining, gradeResult,
    selectedMode, sessionId, avgScore,
    stats: statsRef.current,
    fetchDecks, startReview, submitAnswer, confirmRating,
    requestExplanation, switchMode, skipCard,
  };
}
```

- [ ] **Step 3: Run frontend tests**

Run: `cd desktop-ui && bun run test`

Expected: PASS (new hook is not imported anywhere yet, so no regressions).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat(active-recall): implement useActiveReview state machine hook"
```

---

## Task 9: Frontend — Core Review Components

**Files:**
- Create: all files in `desktop-ui/src/features/notes/components/review/`
- Modify: `desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx`

- [ ] **Step 1: Create CardFront component**

Create `desktop-ui/src/features/notes/components/review/CardFront.tsx`:

```tsx
import { BookOpen } from "lucide-react";
import type { Flashcard } from "../../hooks/useFlashcards";

interface CardFrontProps {
  card: Flashcard;
}

export function CardFront({ card }: CardFrontProps) {
  return (
    <div className="rounded-lg bg-white/[0.03] p-4">
      <div className="flex items-center gap-2 mb-2">
        <BookOpen size={12} className="text-muted-foreground" />
        <span className="text-[9px] text-dim uppercase tracking-wider">{card.deck}</span>
        <span className="text-[9px] text-dim px-1.5 py-0.5 rounded bg-white/[0.04]">{card.cardType}</span>
      </div>
      <p className="text-[13px] text-foreground whitespace-pre-wrap leading-relaxed">{card.front}</p>
    </div>
  );
}
```

- [ ] **Step 2: Create TypedAnswerInput component**

Create `desktop-ui/src/features/notes/components/review/TypedAnswerInput.tsx`:

```tsx
import { useRef, useEffect } from "react";

interface TypedAnswerInputProps {
  onSubmit: (answer: string) => void;
  disabled?: boolean;
  initialValue?: string;
}

export function TypedAnswerInput({ onSubmit, disabled, initialValue = "" }: TypedAnswerInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const valueRef = useRef(initialValue);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      const value = valueRef.current.trim();
      if (value) onSubmit(value);
    }
  };

  return (
    <div className="relative">
      <textarea
        ref={textareaRef}
        defaultValue={initialValue}
        onChange={(e) => { valueRef.current = e.target.value; }}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        placeholder="Type your answer..."
        rows={3}
        className="w-full rounded-lg bg-white/[0.04] border border-border p-3 text-[12px] text-foreground placeholder:text-dim resize-none focus:outline-none focus:ring-1 focus:ring-accent"
      />
      <div className="absolute bottom-2 right-3 text-[9px] text-dim">
        Enter to submit · Shift+Enter for newline
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create SelfGradeInput component**

Create `desktop-ui/src/features/notes/components/review/SelfGradeInput.tsx` — extract the current show/reveal/rate logic from `FlashcardReview.tsx`:

```tsx
import { ChevronRight } from "lucide-react";
import { useState } from "react";
import type { Flashcard } from "../../hooks/useFlashcards";
import type { ReviewQuality } from "../../hooks/useFlashcards";

interface SelfGradeInputProps {
  card: Flashcard;
  onRate: (quality: ReviewQuality) => void;
}

export function SelfGradeInput({ card, onRate }: SelfGradeInputProps) {
  const [revealed, setRevealed] = useState(false);

  if (!revealed) {
    return (
      <button
        type="button"
        onClick={() => setRevealed(true)}
        className="flex items-center justify-center gap-1 w-full text-[10px] px-3 py-2 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground hover:bg-white/[0.08]"
      >
        <ChevronRight size={10} /> Show Answer
      </button>
    );
  }

  return (
    <>
      <div className="rounded-lg bg-white/[0.04] border border-border p-3">
        <p className="text-[11px] text-foreground whitespace-pre-wrap">{card.back}</p>
      </div>
      <div className="flex gap-2 justify-center">
        {(["again", "hard", "good", "easy"] as const).map((q) => (
          <button
            key={q}
            type="button"
            onClick={() => onRate(q)}
            className="text-[10px] px-3 py-1.5 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.08] capitalize"
          >
            {q}
          </button>
        ))}
      </div>
    </>
  );
}
```

- [ ] **Step 4: Create GradeDisplay component**

Create `desktop-ui/src/features/notes/components/review/GradeDisplay.tsx`:

```tsx
import type { GradeResult } from "@shared/types/notes";

interface GradeDisplayProps {
  result: GradeResult;
}

const scoreBadge = (score: number | null) => {
  if (score === null) return { text: "Self-rated", color: "text-dim" };
  if (score >= 0.85) return { text: "Nailed it!", color: "text-green-400" };
  if (score >= 0.60) return { text: "Close", color: "text-yellow-400" };
  if (score >= 0.30) return { text: "Partial", color: "text-orange-400" };
  return { text: "Missed", color: "text-red-400" };
};

export function GradeDisplay({ result }: GradeDisplayProps) {
  const badge = scoreBadge(result.score);

  return (
    <div className="space-y-3">
      {/* Score badge */}
      <div className="flex items-center gap-2">
        <span className={`text-[14px] font-semibold ${badge.color}`}>{badge.text}</span>
        {result.score !== null && (
          <span className="text-[10px] text-dim">{Math.round(result.score * 100)}%</span>
        )}
        <span className="text-[9px] text-dim px-1.5 py-0.5 rounded bg-white/[0.04]">{result.gradingMethod}</span>
      </div>

      {/* Diff highlights */}
      {result.diffHighlights.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {result.diffHighlights.map((seg, i) => (
            <span
              key={i}
              className={`text-[10px] px-1.5 py-0.5 rounded ${
                seg.status === "match" ? "bg-green-500/20 text-green-300" :
                seg.status === "missing" ? "bg-red-500/20 text-red-300" :
                seg.status === "partial" ? "bg-yellow-500/20 text-yellow-300" :
                "bg-white/[0.04] text-dim"
              }`}
            >
              {seg.text}
            </span>
          ))}
        </div>
      )}

      {/* Expected answer */}
      <div className="rounded-lg bg-white/[0.04] border border-border p-3">
        <p className="text-[9px] text-dim mb-1">Expected answer</p>
        <p className="text-[11px] text-foreground whitespace-pre-wrap">{result.expectedAnswer}</p>
      </div>

      {/* Explanation */}
      {result.explanation && (
        <p className="text-[10px] text-muted-foreground leading-relaxed">{result.explanation}</p>
      )}

      {/* Socratic suggestion (collapsible) */}
      {result.socraticSuggestion && (
        <details className="rounded-lg bg-white/[0.03] p-2">
          <summary className="text-[10px] text-accent cursor-pointer">Deepen your understanding...</summary>
          <p className="text-[10px] text-muted-foreground mt-2 leading-relaxed">{result.socraticSuggestion}</p>
        </details>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Create GradeActions component**

Create `desktop-ui/src/features/notes/components/review/GradeActions.tsx`:

```tsx
import type { GradeResult } from "@shared/types/notes";
import type { ReviewQuality } from "../../hooks/useFlashcards";

interface GradeActionsProps {
  result: GradeResult;
  onConfirm: (quality?: ReviewQuality) => void;
  onExplain: () => void;
  onSaveInsight: () => void;
  onJumpToSource: () => void;
}

export function GradeActions({ result, onConfirm, onExplain, onSaveInsight, onJumpToSource }: GradeActionsProps) {
  return (
    <div className="space-y-2">
      {/* Suggested rating + confirm */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => onConfirm()}
          className="flex-1 text-[10px] px-3 py-1.5 rounded-md bg-accent/20 text-accent hover:bg-accent/30"
        >
          Confirm: {result.suggestedRating} (Enter)
        </button>
      </div>

      {/* Rating overrides */}
      <div className="flex gap-1.5 justify-center">
        {(["again", "hard", "good", "easy"] as const).map((q, i) => (
          <button
            key={q}
            type="button"
            onClick={() => onConfirm(q)}
            className="text-[9px] px-2 py-1 rounded bg-white/[0.04] text-dim hover:text-foreground hover:bg-white/[0.06] capitalize"
          >
            {i + 1}:{q}
          </button>
        ))}
      </div>

      {/* Action row */}
      <div className="flex gap-2 justify-center pt-1">
        <button type="button" onClick={onExplain} className="text-[9px] text-accent hover:underline">
          (e) Explain
        </button>
        <button type="button" onClick={onSaveInsight} className="text-[9px] text-muted-foreground hover:underline">
          (s) Save insight
        </button>
        <button type="button" onClick={onJumpToSource} className="text-[9px] text-muted-foreground hover:underline">
          (j) Source note
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 6: Create ReviewCard orchestrator**

Create `desktop-ui/src/features/notes/components/review/ReviewCard.tsx` — composes CardFront + AnswerInput + GradeDisplay + GradeActions based on `cardPhase`:

```tsx
import type { AnswerMode, GradeResult } from "@shared/types/notes";
import type { Flashcard, ReviewQuality } from "../../hooks/useFlashcards";
import { CardFront } from "./CardFront";
import { TypedAnswerInput } from "./TypedAnswerInput";
import { SelfGradeInput } from "./SelfGradeInput";
import { GradeDisplay } from "./GradeDisplay";
import { GradeActions } from "./GradeActions";

type CardPhase = "answering" | "grading" | "graded" | "socratic" | "confirming";

interface ReviewCardProps {
  card: Flashcard;
  cardPhase: CardPhase;
  mode: AnswerMode;
  gradeResult: GradeResult | null;
  onSubmitAnswer: (answer: string) => void;
  onConfirmRating: (quality?: ReviewQuality) => void;
  onExplain: () => void;
  onSaveInsight: () => void;
  onJumpToSource: () => void;
  onSelfRate: (quality: ReviewQuality) => void;
}

export function ReviewCard({
  card, cardPhase, mode, gradeResult,
  onSubmitAnswer, onConfirmRating, onExplain, onSaveInsight, onJumpToSource, onSelfRate,
}: ReviewCardProps) {
  return (
    <div className="flex flex-col gap-3">
      <CardFront card={card} />

      {/* Input phase */}
      {cardPhase === "answering" && (
        mode === "self_grade"
          ? <SelfGradeInput card={card} onRate={onSelfRate} />
          : <TypedAnswerInput onSubmit={onSubmitAnswer} />
      )}

      {/* Grading spinner */}
      {cardPhase === "grading" && (
        <div className="flex items-center justify-center py-4">
          <div className="animate-spin h-4 w-4 border-2 border-accent border-t-transparent rounded-full" />
          <span className="ml-2 text-[10px] text-dim">Grading...</span>
        </div>
      )}

      {/* Grade result */}
      {(cardPhase === "graded" || cardPhase === "socratic" || cardPhase === "confirming") && gradeResult && (
        <>
          <GradeDisplay result={gradeResult} />
          <GradeActions
            result={gradeResult}
            onConfirm={onConfirmRating}
            onExplain={onExplain}
            onSaveInsight={onSaveInsight}
            onJumpToSource={onJumpToSource}
          />
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 7: Create SessionProgress component**

Create `desktop-ui/src/features/notes/components/review/SessionProgress.tsx`:

```tsx
import { X } from "lucide-react";

interface SessionProgressProps {
  remaining: number;
  total: number;
  avgScore: number;
  onExit: () => void;
}

export function SessionProgress({ remaining, total, avgScore, onExit }: SessionProgressProps) {
  const completed = total - remaining;
  const pct = total > 0 ? (completed / total) * 100 : 0;

  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1 rounded-full bg-white/[0.06] overflow-hidden">
        <div className="h-full bg-accent rounded-full transition-all duration-300" style={{ width: `${pct}%` }} />
      </div>
      <span className="text-[9px] text-dim shrink-0">{remaining} left</span>
      {completed > 0 && <span className="text-[9px] text-dim shrink-0">{Math.round(avgScore * 100)}%</span>}
      <button type="button" onClick={onExit} className="p-1 text-dim hover:text-foreground"><X size={12} /></button>
    </div>
  );
}
```

- [ ] **Step 8: Create ActiveReviewSession root component**

Create `desktop-ui/src/features/notes/components/review/ActiveReviewSession.tsx`:

```tsx
import { useEffect } from "react";
import { useActiveReview } from "../../hooks/useActiveReview";
import { ReviewCard } from "./ReviewCard";
import { SessionProgress } from "./SessionProgress";
import type { ReviewQuality } from "../../hooks/useFlashcards";

interface ActiveReviewSessionProps {
  layout: "compact" | "fullscreen";
  onClose: () => void;
}

export function ActiveReviewSession({ layout, onClose }: ActiveReviewSessionProps) {
  const review = useActiveReview();

  useEffect(() => {
    review.fetchDecks();
  }, [review.fetchDecks]);

  // Keyboard shortcuts
  useEffect(() => {
    if (review.phase !== "reviewing") return;
    const handler = (e: KeyboardEvent) => {
      if (review.cardPhase === "graded") {
        if (e.key === "Enter") { review.confirmRating(); return; }
        if (e.key === "1") { review.confirmRating("again"); return; }
        if (e.key === "2") { review.confirmRating("hard"); return; }
        if (e.key === "3") { review.confirmRating("good"); return; }
        if (e.key === "4") { review.confirmRating("easy"); return; }
        if (e.key === "e") { review.requestExplanation(); return; }
      }
      if (e.key === "Tab") { e.preventDefault(); review.switchMode(review.selectedMode === "self_grade" ? "typed" : "self_grade"); }
      if (e.key === "Escape") { onClose(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [review.phase, review.cardPhase, review.selectedMode, review.confirmRating, review.requestExplanation, review.switchMode, onClose]);

  // Deck picker
  if (review.phase === "idle" || review.phase === "deck_picker") {
    // Reuse existing DeckPicker pattern from FlashcardReview
    const dueDecks = review.decks.filter((d) => d.dueCount > 0);
    if (dueDecks.length === 0) {
      return <div className="flex flex-col items-center justify-center gap-3 py-8"><p className="text-[11px] text-dim">No cards due for review</p></div>;
    }
    return (
      <div className="flex flex-col gap-3 p-3">
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-foreground font-medium">Choose a deck</span>
        </div>
        <div className="space-y-1.5">
          {dueDecks.map((d) => (
            <button key={d.name} type="button" onClick={() => review.startReview(d.name)}
              className="w-full flex items-center gap-2 p-2 rounded-lg bg-white/[0.03] hover:bg-white/[0.06] text-left">
              <span className="text-[11px] text-foreground truncate flex-1">{d.name}</span>
              <span className="text-[10px] text-dim shrink-0">{d.dueCount} due</span>
            </button>
          ))}
        </div>
      </div>
    );
  }

  // Complete
  if (review.phase === "complete") {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-8">
        <p className="text-[12px] text-foreground font-medium">Review complete!</p>
        <p className="text-[10px] text-dim">{review.stats.cardsReviewed} cards · {Math.round(review.avgScore * 100)}% avg</p>
        <button type="button" onClick={onClose}
          className="text-[10px] px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground">
          Done
        </button>
      </div>
    );
  }

  // Active review
  if (!review.current) return null;

  return (
    <div className={`flex flex-col gap-3 ${layout === "compact" ? "p-3" : "p-6 max-w-2xl mx-auto"}`}>
      <SessionProgress
        remaining={review.remaining}
        total={review.cards.length}
        avgScore={review.avgScore}
        onExit={onClose}
      />
      <ReviewCard
        card={review.current}
        cardPhase={review.cardPhase}
        mode={review.selectedMode}
        gradeResult={review.gradeResult}
        onSubmitAnswer={review.submitAnswer}
        onConfirmRating={review.confirmRating}
        onExplain={review.requestExplanation}
        onSaveInsight={() => {}}
        onJumpToSource={() => {}}
        onSelfRate={(q: ReviewQuality) => review.confirmRating(q)}
      />
      {/* Mode indicator */}
      <div className="flex items-center justify-center gap-2">
        <span className="text-[9px] text-dim">Mode: {review.selectedMode}</span>
        <button type="button" onClick={() => review.switchMode(review.selectedMode === "self_grade" ? "typed" : "self_grade")}
          className="text-[9px] text-accent hover:underline">
          Tab: switch
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 9: Replace FlashcardReview internals**

In `desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx`, replace the component body:

```tsx
import { ActiveReviewSession } from "../review/ActiveReviewSession";

export function FlashcardReview({ onClose }: { onClose: () => void }) {
  return <ActiveReviewSession layout="compact" onClose={onClose} />;
}
```

- [ ] **Step 10: Run frontend tests and lint**

Run: `cd desktop-ui && bun run lint:fix && bun run test`

Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add -A && git commit -m "feat(active-recall): implement core review components and wire into FlashcardReview"
```

---

## Task 10: Multiple Choice + Cloze + Voice Input Modes

**Files:**
- Create: `desktop-ui/src/features/notes/components/review/MultipleChoiceInput.tsx`
- Create: `desktop-ui/src/features/notes/components/review/ClozeInput.tsx`
- Create: `desktop-ui/src/features/notes/components/review/VoiceInput.tsx`
- Create: `desktop-ui/src/features/notes/components/review/ModeSelector.tsx`
- Modify: `desktop-ui/src/features/notes/components/review/ReviewCard.tsx`

- [ ] **Step 1: Create MultipleChoiceInput**

```tsx
import { useState } from "react";

interface MultipleChoiceInputProps {
  correctAnswer: string;
  distractors: string[];
  onSelect: (answer: string) => void;
}

export function MultipleChoiceInput({ correctAnswer, distractors, onSelect }: MultipleChoiceInputProps) {
  const [selected, setSelected] = useState<string | null>(null);
  // Shuffle options once
  const [options] = useState(() => {
    const all = [correctAnswer, ...distractors];
    return all.sort(() => Math.random() - 0.5);
  });

  return (
    <div className="space-y-1.5">
      {options.map((opt, i) => (
        <button
          key={i}
          type="button"
          onClick={() => { setSelected(opt); onSelect(opt); }}
          className={`w-full text-left p-2.5 rounded-lg text-[11px] border transition-colors ${
            selected === opt
              ? "border-accent bg-accent/10 text-foreground"
              : "border-border bg-white/[0.03] text-muted-foreground hover:bg-white/[0.06]"
          }`}
        >
          <span className="text-[10px] text-dim mr-2">{String.fromCharCode(65 + i)}.</span>
          {opt}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Create ClozeInput**

```tsx
import { useState } from "react";

interface ClozeInputProps {
  clozeText: string; // text with {{c1::hidden}} markers
  onSubmit: (filledText: string) => void;
}

export function ClozeInput({ clozeText, onSubmit }: ClozeInputProps) {
  const blanks = clozeText.match(/\{\{c\d+::([^}]+)\}\}/g) || [];
  const [answers, setAnswers] = useState<string[]>(blanks.map(() => ""));

  const parts = clozeText.split(/\{\{c\d+::[^}]+\}\}/);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      onSubmit(answers.join(" | "));
    }
  };

  return (
    <div className="rounded-lg bg-white/[0.04] border border-border p-3 text-[11px] text-foreground leading-relaxed">
      {parts.map((part, i) => (
        <span key={i}>
          {part}
          {i < blanks.length && (
            <input
              type="text"
              value={answers[i]}
              onChange={(e) => {
                const next = [...answers];
                next[i] = e.target.value;
                setAnswers(next);
              }}
              onKeyDown={handleKeyDown}
              placeholder="..."
              className="inline-block w-24 mx-1 px-2 py-0.5 rounded bg-white/[0.06] border border-accent/30 text-foreground text-[11px] focus:outline-none focus:ring-1 focus:ring-accent"
            />
          )}
        </span>
      ))}
      <div className="mt-2 text-[9px] text-dim">Enter to submit</div>
    </div>
  );
}
```

- [ ] **Step 3: Create VoiceInput**

```tsx
import { Mic, Square } from "lucide-react";
import { useCallback, useRef, useState } from "react";

interface VoiceInputProps {
  onSubmit: (transcript: string) => void;
}

export function VoiceInput({ onSubmit }: VoiceInputProps) {
  const [recording, setRecording] = useState(false);
  const [transcript, setTranscript] = useState("");
  const recognitionRef = useRef<SpeechRecognition | null>(null);

  const start = useCallback(() => {
    const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!SpeechRecognition) { return; }
    const recognition = new SpeechRecognition();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.onresult = (e) => {
      let text = "";
      for (let i = 0; i < e.results.length; i++) {
        text += e.results[i][0].transcript;
      }
      setTranscript(text);
    };
    recognition.start();
    recognitionRef.current = recognition;
    setRecording(true);
  }, []);

  const stop = useCallback(() => {
    recognitionRef.current?.stop();
    setRecording(false);
    if (transcript.trim()) onSubmit(transcript.trim());
  }, [transcript, onSubmit]);

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={recording ? stop : start}
          className={`p-2 rounded-full ${recording ? "bg-red-500/20 text-red-400" : "bg-white/[0.06] text-muted-foreground"} hover:bg-white/[0.08]`}
        >
          {recording ? <Square size={14} /> : <Mic size={14} />}
        </button>
        <span className="text-[10px] text-dim">{recording ? "Recording... tap to stop" : "Tap to start"}</span>
      </div>
      {transcript && (
        <div className="rounded-lg bg-white/[0.04] p-2 text-[11px] text-muted-foreground">{transcript}</div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create ModeSelector**

```tsx
import type { AnswerMode } from "@shared/types/notes";

interface ModeSelectorProps {
  current: AnswerMode;
  onChange: (mode: AnswerMode) => void;
}

const modes: Array<{ value: AnswerMode; label: string }> = [
  { value: "typed", label: "Typed" },
  { value: "self_grade", label: "Self-grade" },
  { value: "multiple_choice", label: "Multiple choice" },
  { value: "voice", label: "Voice" },
];

export function ModeSelector({ current, onChange }: ModeSelectorProps) {
  return (
    <div className="flex items-center gap-1">
      {modes.map((m) => (
        <button
          key={m.value}
          type="button"
          onClick={() => onChange(m.value)}
          className={`text-[9px] px-2 py-0.5 rounded-full ${
            current === m.value ? "bg-accent/20 text-accent" : "text-dim hover:text-foreground"
          }`}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 5: Update ReviewCard to support all modes**

In `ReviewCard.tsx`, update the answering phase to render the appropriate input based on `mode`:

```tsx
{cardPhase === "answering" && (
  <>
    {mode === "self_grade" && <SelfGradeInput card={card} onRate={onSelfRate} />}
    {mode === "typed" && <TypedAnswerInput onSubmit={onSubmitAnswer} />}
    {mode === "multiple_choice" && <MultipleChoiceInput correctAnswer={card.back} distractors={[]} onSelect={onSubmitAnswer} />}
    {mode === "cloze_fill" && card.cardType === "cloze" && <ClozeInput clozeText={card.front} onSubmit={onSubmitAnswer} />}
    {mode === "voice" && <VoiceInput onSubmit={onSubmitAnswer} />}
    {mode === "auto" && <TypedAnswerInput onSubmit={onSubmitAnswer} />}
  </>
)}
```

- [ ] **Step 6: Run lint and tests**

Run: `cd desktop-ui && bun run lint:fix && bun run test`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(active-recall): add multiple choice, cloze, voice, and mode selector"
```

---

## Task 11: Session Summary + Reflection Pulse

**Files:**
- Create: `desktop-ui/src/features/notes/components/review/SessionSummary.tsx`
- Modify: `desktop-ui/src/features/notes/components/review/ActiveReviewSession.tsx`

- [ ] **Step 1: Create SessionSummary component**

Create `desktop-ui/src/features/notes/components/review/SessionSummary.tsx` with three timed beats:

```tsx
import { useEffect, useState } from "react";
import type { SessionStats } from "@shared/types/notes";

interface SessionSummaryProps {
  stats: SessionStats;
  onClose: () => void;
  onSaveInsight: () => void;
  onReviewWeak: () => void;
}

export function SessionSummary({ stats, onClose, onSaveInsight, onReviewWeak }: SessionSummaryProps) {
  const [beat, setBeat] = useState(0);
  const [reflection, setReflection] = useState("");
  const [showReflection, setShowReflection] = useState(false);

  useEffect(() => {
    const t1 = setTimeout(() => setBeat(1), 1000);
    const t2 = setTimeout(() => setBeat(2), 2000);
    const t3 = setTimeout(() => {
      // Context-aware reflection pulse
      const avg = stats.cardsReviewed > 0 ? stats.totalScore / stats.cardsReviewed : 1;
      if (avg < 0.75 || stats.weakCards.length > 2 || stats.propagationCount > 5) {
        setShowReflection(true);
      }
    }, 3000);
    return () => { clearTimeout(t1); clearTimeout(t2); clearTimeout(t3); };
  }, [stats]);

  const avgPct = stats.cardsReviewed > 0 ? Math.round((stats.totalScore / stats.cardsReviewed) * 100) : 0;
  const duration = Math.round((Date.now() - stats.startTime) / 1000 / 60);

  return (
    <div className="flex flex-col items-center gap-4 py-6 px-4">
      {/* Beat 1: Score ring */}
      <div className="relative w-20 h-20">
        <svg className="w-full h-full -rotate-90" viewBox="0 0 36 36">
          <circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor" className="text-white/[0.06]" strokeWidth="2" />
          <circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor"
            className={avgPct >= 85 ? "text-green-400" : avgPct >= 60 ? "text-yellow-400" : "text-orange-400"}
            strokeWidth="2" strokeDasharray={`${avgPct} ${100 - avgPct}`} strokeLinecap="round"
            style={{ transition: "stroke-dasharray 1s ease-out" }}
          />
        </svg>
        <span className="absolute inset-0 flex items-center justify-center text-[16px] font-semibold text-foreground">{avgPct}%</span>
      </div>

      {/* Beat 2: Stats */}
      {beat >= 1 && (
        <div className="flex flex-wrap gap-3 justify-center animate-in fade-in duration-500">
          <span className="text-[10px] text-dim">{stats.cardsReviewed} cards · {duration}min</span>
        </div>
      )}

      {/* Beat 3: Narrative */}
      {beat >= 2 && (
        <div className="space-y-1 text-center animate-in fade-in duration-500">
          {stats.propagationCount > 0 && (
            <p className="text-[10px] text-accent">Strengthened {stats.propagationCount} knowledge connections</p>
          )}
          {stats.weakCards.length > 0 && (
            <p className="text-[10px] text-orange-400">{stats.weakCards.length} weak spots surfaced</p>
          )}
        </div>
      )}

      {/* Reflection pulse (context-aware) */}
      {showReflection && (
        <div className="w-full max-w-sm space-y-2 animate-in fade-in duration-700">
          <p className="text-[10px] text-dim italic text-center">What felt different about today's answers?</p>
          <textarea
            value={reflection}
            onChange={(e) => setReflection(e.target.value)}
            rows={2}
            placeholder="Optional — your future self will see this..."
            className="w-full rounded-lg bg-white/[0.04] border border-border p-2 text-[10px] text-foreground placeholder:text-dim resize-none focus:outline-none focus:ring-1 focus:ring-accent"
          />
        </div>
      )}

      {/* Actions */}
      <div className="flex flex-wrap gap-2 justify-center pt-2">
        {stats.weakCards.length > 0 && (
          <button type="button" onClick={onReviewWeak}
            className="text-[10px] px-3 py-1.5 rounded-md bg-accent/20 text-accent hover:bg-accent/30">
            Review weak spots
          </button>
        )}
        <button type="button" onClick={onSaveInsight}
          className="text-[10px] px-3 py-1.5 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground">
          Save as insight
        </button>
        <button type="button" onClick={onClose}
          className="text-[10px] px-3 py-1.5 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground">
          Done
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Wire SessionSummary into ActiveReviewSession**

Replace the simple "Review complete!" section in `ActiveReviewSession.tsx` with `<SessionSummary>`.

- [ ] **Step 3: Run lint and tests**

Run: `cd desktop-ui && bun run lint:fix && bun run test`

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat(active-recall): add session summary with timed beats and reflection pulse"
```

---

## Task 12: Review Session Persistence + Graph Propagation Handler

**Files:**
- Create: `crates/app-core/src/handlers/notes/review_session.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`

- [ ] **Step 1: Add shared types**

In `crates/desktop-shared/src/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSessionSaveParams {
    pub session_id: String,
    pub cards_reviewed: i32,
    pub avg_score: f64,
    pub duration_seconds: i32,
    pub modes_used: Vec<String>,
    pub propagation_count: i32,
    pub weak_card_ids: Vec<String>,
    pub session_data: String,
    pub status: String, // "completed" | "abandoned"
}
```

- [ ] **Step 2: Implement save handler**

Create `crates/app-core/src/handlers/notes/review_session.rs`:

```rust
impl AppCore {
    pub async fn flashcard_save_session(&self, params: ReviewSessionSaveParams) -> Result<(), ApiError> {
        let repo = self.review_session_repo()?;

        if params.status == "abandoned" {
            repo.abandon(&params.session_id, params.cards_reviewed).await?;
        } else {
            let modes_json = serde_json::to_string(&params.modes_used)?;
            let weak_json = serde_json::to_string(&params.weak_card_ids)?;
            repo.complete(
                &params.session_id, &params.session_data,
                params.cards_reviewed, params.avg_score, params.duration_seconds,
                &modes_json, params.propagation_count, &weak_json,
            ).await?;
        }

        // Publish domain event
        self.bus.publish(DomainEvent::FlashcardSessionCompleted {
            session_id: params.session_id,
            cards_reviewed: params.cards_reviewed as usize,
            avg_score: params.avg_score,
            weak_domains: vec![],
            propagation_count: params.propagation_count as usize,
        });

        Ok(())
    }
}
```

- [ ] **Step 3: Add module declaration, run tests, commit**

```bash
git add -A && git commit -m "feat(active-recall): add review session persistence and domain event"
```

---

## Task 13: Integration Testing

**Files:**
- Test: `tests/integration/` or inline

- [ ] **Step 1: Write integration test for grading pipeline**

Test the full flow: create card → embed → submit answer → verify grade result. Use `StoragePool::connect_in_memory()`.

```rust
#[tokio::test]
async fn test_grading_exact_match() {
    // Setup app-core with in-memory storage
    // Create a card with front="What is FSRS?" back="Free Spaced Repetition Scheduler"
    // Submit answer "Free Spaced Repetition Scheduler"
    // Assert score == 1.0, grading_method == "exact_match"
}

#[tokio::test]
async fn test_grading_wrong_answer() {
    // Submit "I don't know"
    // Assert score < 0.3, grading_method != "exact_match"
}
```

- [ ] **Step 2: Write integration test for propagation**

```rust
#[tokio::test]
async fn test_propagation_boosts_linked_cards() {
    // Create two notes with a link between them
    // Create cards for both notes
    // Review card A with "easy"
    // Assert card B's due_at was extended
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`

Expected: All tests pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

Expected: 0 warnings.

- [ ] **Step 5: Run frontend lint + tests**

Run: `cd desktop-ui && bun run lint && bun run test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "test(active-recall): add integration tests for grading and propagation"
```

---

## Summary

| Task | What it delivers | Estimated steps |
|------|------------------|-----------------|
| 1 | Schema, repos, config, domain event | 13 |
| 2 | Card embedding pipeline | 6 |
| 3 | Grading pipeline (exact + semantic + LLM) | 14 |
| 4 | Distractor generation | 4 |
| 5 | Tauri command wiring | 6 |
| 6 | Difficulty estimation in card generation | 5 |
| 7 | Knowledge graph propagation (FIRe) | 6 |
| 8 | useActiveReview hook | 4 |
| 9 | Core review components (CardFront, TypedAnswer, GradeDisplay, etc.) | 11 |
| 10 | Multi-mode inputs (MC, cloze, voice, mode selector) | 7 |
| 11 | Session summary + reflection pulse | 4 |
| 12 | Session persistence + domain event | 3 |
| 13 | Integration testing + final verification | 6 |
| **Total** | **Full active recall system** | **89 steps** |

Build order: Tasks must be done sequentially 1→13. Task 1 (schema/types) is the foundation for all others. Tasks 2-7 (backend) each depend on Task 1. Tasks 8-11 (frontend) depend on Task 5 (Tauri wiring). Tasks 12-13 tie everything together.

**Note:** The graph propagation queries (Task 7) join across `flashcards` (cognitive migration) and `note_links` (feature-notes migration). Both tables are in the same SQLite database (`data.db`), so cross-table joins work at runtime. The propagation queries live in `app-core` handlers (not `FlashcardRepo`) to keep this cross-feature coupling at the handler layer.

**Spec features deferred to follow-up tasks (not in this plan):**
- First-Review Tutorial Deck (built-in onboarding)
- Adaptive mode suggestions ("You score 18% higher with voice")
- Per-deck propagation disable toggle
- Difficulty adjustment after 5+ reviews
- Adaptive propagation ripple sampling (40%→20% taper)
