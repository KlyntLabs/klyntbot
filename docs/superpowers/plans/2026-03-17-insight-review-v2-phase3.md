# Insight Review V2 — Phase 3: Learning Progress

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compute and display learning progress for insight reviews — tracking flashcard success, semantic drift between versions, gap closure, and quiz scores — enabling an evolution timeline that shows how understanding of a topic changes over time.

**Architecture:** New `progress.rs` in `feature-insights` houses `ProgressComputer` which computes 4 progress signals. A `FlashcardAccessorImpl` adapter in `app-core` implements the `FlashcardAccessor` trait using the spec's FSRS-derived success rate SQL. `VectorStore` gains a `get_embedding()` method for raw vector retrieval, enabling `InsightEmbedderImpl.similarity()` to compute cosine distance between versions. A new `get_evolution` Tauri command surfaces the timeline with progress snapshots and change notes. A cron job refreshes stale progress snapshots daily.

**Tech Stack:** Rust (tokio, sqlx, serde_json), SQLite (FSRS metrics in flashcards table), LanceDB (embedding vectors), existing `EmbeddingEngine::cosine_similarity()` static method

**Spec:** `docs/superpowers/specs/2026-03-17-insight-review-v2-design.md` (Section 7)

**Scope:** Backend only. No frontend. The evolution timeline API is the deliverable — the frontend component is Phase 4.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/feature-insights/src/progress.rs` | `ProgressComputer` — computes 4 signals (flashcard, drift, gap closure, quiz) and generates change notes |
| `crates/app-core/src/adapters/flashcard_accessor.rs` | `FlashcardAccessorImpl` — concrete `FlashcardAccessor` using `FlashcardRepo` with FSRS success rate SQL |

### Modified files

| File | Change |
|------|--------|
| `crates/storage/src/vector_store/crud.rs` | Add `get_embedding()` method for raw vector retrieval by ID |
| `crates/app-core/src/adapters/insight_embedder.rs` | Implement `similarity()` using `get_embedding()` + `EmbeddingEngine::cosine_similarity()` |
| `crates/app-core/src/adapters/mod.rs` | Add `pub mod flashcard_accessor` |
| `crates/app-core/src/init/mod.rs` | Replace `NoopFlashcardAccessor` with `FlashcardAccessorImpl` |
| `crates/feature-insights/src/service.rs` | Add `compute_progress()` and `get_evolution()` methods |
| `crates/feature-insights/src/lib.rs` | Add `pub mod progress` + re-exports |
| `crates/feature-insights/src/types.rs` | No changes — `ProgressSnapshotRow` and `ProgressWeights` already exist |
| `crates/desktop-shared/src/commands/notes.rs` | Add `InsightEvolutionResponse` and `InsightEvolutionPoint` DTOs |
| `crates/desktop/src/commands/notes.rs` | Add `note_insight_get_evolution` Tauri command + DEV_COMMANDS + dispatch_dev |
| `crates/desktop/src/main.rs` | Register `note_insight_get_evolution` in invoke_handler |
| `crates/app-core/src/handlers/notes/insight.rs` | Add `note_insight_get_evolution()` handler method |
| `crates/app-core/src/init/cron.rs` | Register daily progress refresh cron job |

---

## Chunk 1: Progress Computation Engine

### Task 1: ProgressComputer with gap closure + change notes

**Files:**
- Create: `crates/feature-insights/src/progress.rs`
- Modify: `crates/feature-insights/src/lib.rs`

The `ProgressComputer` computes 4 progress signals. Two of them (flashcard success, semantic drift) depend on external accessors injected via traits. The other two (gap closure, quiz score) are computed from insight content directly. This task implements gap closure + change note generation + the orchestrator that calls all 4 signals.

- [ ] **Step 1: Create progress.rs with ProgressComputer**

Create `crates/feature-insights/src/progress.rs`:

```rust
//! Progress computation for insight reviews.
//!
//! Computes 4 learning signals:
//! 1. Flashcard success (from FlashcardAccessor — weight 0.40)
//! 2. Semantic drift (from InsightEmbedder — weight 0.25)
//! 3. Gap closure (from content comparison — weight 0.20)
//! 4. Quiz score (from self-assessment — weight 0.15)

use std::sync::Arc;

use crate::progress_repo::InsightProgressRepo;
use crate::repo::InsightReviewRepo;
use crate::traits::{FlashcardAccessor, InsightEmbedder};
use crate::types::{InsightContent, InsightReviewRow, ProgressSnapshotRow, ProgressWeights};

/// Computes learning progress for insight reviews.
pub struct ProgressComputer {
    repo: InsightReviewRepo,
    progress_repo: InsightProgressRepo,
    flashcards: Arc<dyn FlashcardAccessor>,
    embedder: Arc<dyn InsightEmbedder>,
    weights: ProgressWeights,
}

impl ProgressComputer {
    pub fn new(
        repo: InsightReviewRepo,
        progress_repo: InsightProgressRepo,
        flashcards: Arc<dyn FlashcardAccessor>,
        embedder: Arc<dyn InsightEmbedder>,
        weights: ProgressWeights,
    ) -> Self {
        Self {
            repo,
            progress_repo,
            flashcards,
            embedder,
            weights,
        }
    }

    /// Compute and store a progress snapshot for a specific insight.
    ///
    /// Gathers all 4 signals and stores the result via InsightProgressRepo.
    pub async fn compute(
        &self,
        insight: &InsightReviewRow,
        note_body: &str,
    ) -> Result<ProgressSnapshotRow, sqlx::Error> {
        // 1. Flashcard success (from FSRS metrics)
        let flashcard_success = self
            .flashcards
            .review_success_rate(&insight.id, 30)
            .await;

        // 2. Semantic drift (cosine distance from previous version)
        let semantic_drift = self.compute_semantic_drift(insight).await;

        // 3. Gap closure (compare previous gaps against current note body)
        let gap_closure = self.compute_gap_closure(insight, note_body).await?;

        // 4. Quiz score — placeholder until quiz response persistence is implemented
        // For now, 0.0 (no quiz data). Phase 4 will persist user quiz answers.
        let quiz_score = 0.0;

        self.progress_repo
            .upsert(
                &insight.id,
                insight.version,
                flashcard_success,
                semantic_drift,
                gap_closure,
                quiz_score,
                &self.weights,
            )
            .await
    }

    /// Compute semantic drift between this version and the previous one.
    ///
    /// Returns 0.0 for v1 (no previous version) or when embeddings are unavailable.
    async fn compute_semantic_drift(&self, insight: &InsightReviewRow) -> f64 {
        if insight.version <= 1 {
            return 0.0;
        }

        // Find the previous version's insight ID
        let versions = match self.repo.list_versions(&insight.note_id).await {
            Ok(v) => v,
            Err(_) => return 0.0,
        };

        let prev = versions
            .iter()
            .find(|v| v.version == insight.version - 1);

        let Some(prev) = prev else {
            return 0.0;
        };

        // Cosine similarity → drift = 1.0 - similarity
        match self.embedder.similarity(&insight.id, &prev.id).await {
            Some(sim) => (1.0 - sim).clamp(0.0, 1.0),
            None => 0.0, // Embeddings not available
        }
    }

    /// Compute gap closure: what fraction of previous version's gaps are addressed.
    ///
    /// Parses gap bullets from previous version's gap_analysis, checks how many
    /// topic keywords appear in the current note body (case-insensitive substring match).
    async fn compute_gap_closure(
        &self,
        insight: &InsightReviewRow,
        note_body: &str,
    ) -> Result<f64, sqlx::Error> {
        if insight.version <= 1 {
            return Ok(0.0);
        }

        let versions = self.repo.list_versions(&insight.note_id).await?;
        let prev = versions
            .iter()
            .find(|v| v.version == insight.version - 1);

        let Some(prev) = prev else {
            return Ok(0.0);
        };

        let prev_content: InsightContent =
            serde_json::from_str(&prev.content).unwrap_or_default();

        let Some(ref gaps_text) = prev_content.gap_analysis else {
            return Ok(0.0);
        };

        let gap_topics = extract_gap_topics(gaps_text);
        if gap_topics.is_empty() {
            return Ok(0.0);
        }

        let body_lower = note_body.to_lowercase();
        let closed = gap_topics
            .iter()
            .filter(|topic| body_lower.contains(&topic.to_lowercase()))
            .count();

        Ok(closed as f64 / gap_topics.len() as f64)
    }
}

/// Extract topic keywords from gap analysis text.
///
/// Looks for lines starting with `- **` (markdown bold bullet) and extracts
/// the bold text as the topic. Falls back to extracting first words after `- `.
fn extract_gap_topics(gaps_text: &str) -> Vec<String> {
    let mut topics = Vec::new();
    for line in gaps_text.lines() {
        let trimmed = line.trim();
        // Pattern: "- **Topic Name** — description"
        if let Some(rest) = trimmed.strip_prefix("- **") {
            if let Some(end) = rest.find("**") {
                let topic = rest[..end].trim().to_string();
                if !topic.is_empty() {
                    topics.push(topic);
                }
                continue;
            }
        }
        // Pattern: "- Topic text here"
        if let Some(rest) = trimmed.strip_prefix("- ") {
            // Take first 3 words as the topic
            let topic: String = rest.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            if !topic.is_empty() && topic.len() > 3 {
                topics.push(topic);
            }
        }
    }
    topics
}

/// Generate a human-readable change note comparing two progress snapshots.
pub fn generate_change_note(
    current: &ProgressSnapshotRow,
    previous: Option<&ProgressSnapshotRow>,
) -> String {
    let Some(prev) = previous else {
        return "Initial insight generated".to_string();
    };
    let delta = current.overall_progress - prev.overall_progress;
    let direction = if delta >= 0.0 { "Improved" } else { "Declined" };
    let pct = (delta.abs() * 100.0).round() as i32;

    if pct == 0 {
        return "No significant change".to_string();
    }

    let signals = [
        (
            "flashcard reviews",
            current.flashcard_success - prev.flashcard_success,
        ),
        (
            "content stability",
            (1.0 - current.semantic_drift) - (1.0 - prev.semantic_drift),
        ),
        ("gap closure", current.gap_closure - prev.gap_closure),
        ("quiz performance", current.quiz_score - prev.quiz_score),
    ];
    let best = signals
        .iter()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    format!("{direction} {pct}% — driven by {}", best.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_gap_topics_bold_bullets() {
        let gaps = "## Gaps\n\
            - **Distributed consensus** — not covered\n\
            - **CAP theorem** — only mentioned briefly\n\
            - Some other text";
        let topics = extract_gap_topics(gaps);
        assert_eq!(topics, vec!["Distributed consensus", "CAP theorem"]);
    }

    #[test]
    fn test_extract_gap_topics_plain_bullets() {
        let gaps = "- Missing coverage of event sourcing patterns\n\
            - Shallow treatment of CQRS";
        let topics = extract_gap_topics(gaps);
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0], "Missing coverage of");
        assert_eq!(topics[1], "Shallow treatment of");
    }

    #[test]
    fn test_extract_gap_topics_empty() {
        assert!(extract_gap_topics("No bullet points here").is_empty());
        assert!(extract_gap_topics("").is_empty());
    }

    #[test]
    fn test_change_note_initial() {
        let current = sample_snapshot(0.25, 0.0, 0.0, 0.0, 0.25);
        assert_eq!(
            generate_change_note(&current, None),
            "Initial insight generated"
        );
    }

    #[test]
    fn test_change_note_improved() {
        let prev = sample_snapshot(0.2, 0.0, 0.0, 0.0, 0.33);
        let current = sample_snapshot(0.8, 0.1, 0.5, 0.0, 0.63);
        let note = generate_change_note(&current, Some(&prev));
        assert!(note.starts_with("Improved"));
        assert!(note.contains("flashcard reviews"));
    }

    #[test]
    fn test_change_note_no_change() {
        let snap = sample_snapshot(0.5, 0.0, 0.0, 0.0, 0.45);
        assert_eq!(
            generate_change_note(&snap, Some(&snap)),
            "No significant change"
        );
    }

    fn sample_snapshot(
        flashcard: f64,
        drift: f64,
        gap: f64,
        quiz: f64,
        overall: f64,
    ) -> ProgressSnapshotRow {
        ProgressSnapshotRow {
            id: "snap-1".to_string(),
            insight_review_id: "insight-1".to_string(),
            version: 1,
            flashcard_success: flashcard,
            semantic_drift: drift,
            gap_closure: gap,
            quiz_score: quiz,
            overall_progress: overall,
            computed_at: "2026-03-17T00:00:00Z".to_string(),
        }
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod progress;
pub use progress::{ProgressComputer, generate_change_note};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-insights -E 'test(gap_topics) | test(change_note)'`
Expected: all 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/progress.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add ProgressComputer with gap closure and change notes"
```

---

## Chunk 2: Accessor Implementations

### Task 2: FlashcardAccessorImpl adapter

**Files:**
- Create: `crates/app-core/src/adapters/flashcard_accessor.rs`
- Modify: `crates/app-core/src/adapters/mod.rs`

Implements `FlashcardAccessor` using the FSRS-derived success rate SQL from the spec. The SQL computes success from `state`, `lapses`, `stability`, and `review_count` columns in the `flashcards` table.

Note: `FlashcardRepo` doesn't have a method for querying by `insight_review_id`. Rather than modifying the cognitive crate, we'll use `sqlx` directly with a raw query — the `FlashcardAccessorImpl` takes a `SqlitePool` and runs the FSRS query inline. This keeps the cognitive crate unchanged.

- [ ] **Step 1: Create FlashcardAccessorImpl**

Create `crates/app-core/src/adapters/flashcard_accessor.rs`:

```rust
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
```

- [ ] **Step 2: Register in adapters/mod.rs**

In `crates/app-core/src/adapters/mod.rs`, add:
```rust
pub mod flashcard_accessor;
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/adapters/flashcard_accessor.rs crates/app-core/src/adapters/mod.rs
git commit -m "feat(app-core): add FlashcardAccessorImpl with FSRS success rate computation"
```

---

### Task 3: VectorStore.get_embedding() + real InsightEmbedder.similarity()

**Files:**
- Modify: `crates/storage/src/vector_store/crud.rs`
- Modify: `crates/app-core/src/adapters/insight_embedder.rs`

Add a `get_embedding()` method to `VectorStore` that fetches a raw embedding vector by ID from a LanceDB table. Then implement the `similarity()` method in `InsightEmbedderImpl` to use it.

- [ ] **Step 1: Add get_embedding() to VectorStore**

In `crates/storage/src/vector_store/crud.rs`, add after `search_similar()`:

```rust
    /// Fetch a raw embedding vector by ID from a table.
    ///
    /// Returns None if the ID is not found. Used for computing cosine
    /// similarity between specific embeddings (e.g., semantic drift).
    pub async fn get_embedding(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<Vec<f32>>, StorageError> {
        let tbl = self
            .db
            .open_table(table)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("open table {table}: {e}")))?;

        let predicate = format!("id = '{}'", sanitize_predicate_value(id)?);

        let results = tbl
            .query()
            .only_if(predicate)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("query {table} by id: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("collect results: {e}")))?;

        for batch in &batches {
            if batch.num_rows() == 0 {
                continue;
            }
            let vector_col = batch
                .column_by_name("vector")
                .ok_or_else(|| StorageError::Vector("missing vector column".to_string()))?;

            let list_arr = vector_col
                .as_any()
                .downcast_ref::<arrow_array::FixedSizeListArray>()
                .ok_or_else(|| StorageError::Vector("vector column is not FixedSizeList".to_string()))?;

            let values = list_arr
                .value(0)
                .as_any()
                .downcast_ref::<arrow_array::Float32Array>()
                .ok_or_else(|| StorageError::Vector("vector values are not Float32".to_string()))?;

            return Ok(Some(values.values().to_vec()));
        }

        Ok(None)
    }
```

**IMPORTANT:** The `sanitize_predicate_value` function is in the same file (`crud.rs`). The implementer should check the exact import path — it may be `self::sanitize_predicate_value` or just `sanitize_predicate_value` since it's in the same module. Also verify that `use futures_util::TryStreamExt;` is already imported (needed for `.try_collect()`). Check the existing imports at the top of `crud.rs`.

- [ ] **Step 2: Implement InsightEmbedderImpl.similarity()**

In `crates/app-core/src/adapters/insight_embedder.rs`, replace the `similarity()` placeholder:

```rust
    async fn similarity(&self, id_a: &str, id_b: &str) -> Option<f64> {
        let vec_a = self
            .store
            .get_embedding("insight_embeddings", id_a)
            .await
            .ok()??;
        let vec_b = self
            .store
            .get_embedding("insight_embeddings", id_b)
            .await
            .ok()??;

        Some(tools::embedding_engine::EmbeddingEngine::cosine_similarity(
            &vec_a, &vec_b,
        ))
    }
```

- [ ] **Step 3: Build**

Run: `cargo build -p storage -p app-core`

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/vector_store/crud.rs crates/app-core/src/adapters/insight_embedder.rs
git commit -m "feat(storage): add get_embedding() for raw vector retrieval; implement similarity()"
```

---

## Chunk 3: Service + API Wiring

### Task 4: Wire FlashcardAccessorImpl into AppCore init

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

Replace `NoopFlashcardAccessor` with the real `FlashcardAccessorImpl`.

- [ ] **Step 1: Replace NoopFlashcardAccessor in init**

In `crates/app-core/src/init/mod.rs`, find the `insight_service` construction block and replace:
```rust
Arc::new(feature_insights::NoopFlashcardAccessor), // Phase 3
```
with:
```rust
Arc::new(crate::adapters::flashcard_accessor::FlashcardAccessorImpl::new(
    storage_pool.inner().clone(),
)),
```

- [ ] **Step 2: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p app-core`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): wire real FlashcardAccessorImpl into InsightService"
```

---

### Task 5: Add compute_progress() and get_evolution() to InsightService

**Files:**
- Modify: `crates/feature-insights/src/service.rs`

Add methods that delegate to `ProgressComputer` and assemble evolution timeline data.

- [ ] **Step 1: Add ProgressComputer field and methods**

In `crates/feature-insights/src/service.rs`, the service already has `progress_repo`, `flashcards`, `embedder`, and `progress_weights`. Rather than adding a `ProgressComputer` field (which would duplicate the same dependencies), add methods directly on `InsightService` that construct a temporary `ProgressComputer` or inline the logic.

The cleanest approach: add a `progress_computer()` factory method and two public methods:

```rust
    /// Create a ProgressComputer from the service's shared dependencies.
    fn progress_computer(&self) -> ProgressComputer {
        ProgressComputer::new(
            self.repo.clone(),
            self.progress_repo.clone(),
            Arc::clone(&self.flashcards),
            Arc::clone(&self.embedder),
            self.progress_weights.clone(),
        )
    }

    /// Compute and store a progress snapshot for an insight.
    pub async fn compute_progress(
        &self,
        insight_id: &str,
        note_body: &str,
    ) -> Result<ProgressSnapshotRow, sqlx::Error> {
        let insight = self
            .repo
            .get(insight_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        self.progress_computer().compute(&insight, note_body).await
    }

    /// Get the evolution timeline for a note — versions + progress + change notes.
    pub async fn get_evolution(
        &self,
        note_id: &str,
    ) -> Result<Vec<(InsightReviewRow, Option<ProgressSnapshotRow>)>, sqlx::Error> {
        let versions = self.repo.list_versions(note_id).await?;
        let timeline = self.progress_repo.get_timeline(note_id).await?;

        // Match versions with their progress snapshots
        let result: Vec<_> = versions
            .into_iter()
            .map(|v| {
                let snapshot = timeline
                    .iter()
                    .find(|s| s.insight_review_id == v.id)
                    .cloned();
                (v, snapshot)
            })
            .collect();

        Ok(result)
    }
```

Add required imports at the top of service.rs:
```rust
use crate::progress::ProgressComputer;
use crate::types::ProgressSnapshotRow;
```

- [ ] **Step 2: Update store_insight() to use real progress computation**

In `store_insight()`, replace the all-zeros initial progress with a real computation when a note body is available. Since `store_insight()` doesn't have the note body, keep the initial all-zeros snapshot — progress is recomputed by the cron job or explicitly via `compute_progress()`.

(No change needed — the current initial-zeros approach is correct. The cron job handles recomputation.)

- [ ] **Step 3: Build**

Run: `cargo build -p feature-insights`

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/service.rs
git commit -m "feat(feature-insights): add compute_progress() and get_evolution() to InsightService"
```

---

### Task 6: Evolution DTOs + Tauri command

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/app-core/src/handlers/notes/insight.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/main.rs`

Add the `InsightEvolutionResponse` DTO and the `note_insight_get_evolution` Tauri command.

- [ ] **Step 1: Add DTOs to desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add after `InsightVersionResponse`:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionResponse {
    pub note_id: String,
    pub note_title: String,
    pub versions: Vec<InsightEvolutionPoint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionPoint {
    pub version: i64,
    pub date: String,
    pub flashcard_success: f64,
    pub semantic_drift: f64,
    pub gap_closure: f64,
    pub quiz_score: f64,
    pub overall_progress: f64,
    pub change_note: String,
}
```

- [ ] **Step 2: Add handler in app-core**

In `crates/app-core/src/handlers/notes/insight.rs`, add to the `impl AppCore` block:

```rust
    pub async fn note_insight_get_evolution(
        &self,
        note_id: &str,
    ) -> Result<InsightEvolutionResponse, ApiError> {
        let service = self
            .insight_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let evolution = service
            .get_evolution(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let mut points = Vec::new();
        let mut prev_snapshot: Option<feature_insights::ProgressSnapshotRow> = None;

        // Iterate oldest-first for change note generation
        for (version, snapshot) in evolution.iter().rev() {
            let change_note = match snapshot {
                Some(snap) => {
                    feature_insights::generate_change_note(snap, prev_snapshot.as_ref())
                }
                None => "No progress data".to_string(),
            };

            points.push(InsightEvolutionPoint {
                version: version.version,
                date: version.generated_at.clone(),
                flashcard_success: snapshot.as_ref().map_or(0.0, |s| s.flashcard_success),
                semantic_drift: snapshot.as_ref().map_or(0.0, |s| s.semantic_drift),
                gap_closure: snapshot.as_ref().map_or(0.0, |s| s.gap_closure),
                quiz_score: snapshot.as_ref().map_or(0.0, |s| s.quiz_score),
                overall_progress: snapshot.as_ref().map_or(0.0, |s| s.overall_progress),
                change_note,
            });

            if let Some(snap) = snapshot {
                prev_snapshot = Some(snap.clone());
            }
        }

        Ok(InsightEvolutionResponse {
            note_id: note_id.to_string(),
            note_title: note.title,
            versions: points,
        })
    }
```

- [ ] **Step 3: Add Tauri command, DEV_COMMANDS, dispatch_dev, main.rs**

Follow the exact same pattern as `note_insight_list_versions` (added in Phase 1, Task 8):

In `crates/desktop/src/commands/notes.rs`:
```rust
#[tauri::command]
pub async fn note_insight_get_evolution(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<InsightEvolutionResponse, ApiError> {
    state.note_insight_get_evolution(&note_id).await
}
```

Add `"note_insight_get_evolution"` to `DEV_COMMANDS` after `"note_insight_list_versions"`.

Add dispatch arm:
```rust
        "note_insight_get_evolution" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_get_evolution(&id).await)
        }
```

In `crates/desktop/src/main.rs`, add `commands::notes::note_insight_get_evolution,` after `note_insight_list_versions`.

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop` (DEV_COMMANDS test)

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/ crates/desktop/ crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(desktop): add insight evolution timeline Tauri command"
```

---

### Task 7: Daily progress refresh cron job

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

Register a cron job that recomputes progress snapshots for insights that have had flashcard reviews since their last snapshot.

- [ ] **Step 1: Add cron job constant and registration**

In `crates/app-core/src/init/cron.rs`, add the job constant alongside the existing ones:

```rust
const JOB_INSIGHT_PROGRESS: &str = "__klyntbot_insight_progress_refresh";
```

In `ensure_cron_jobs()`, add the job using the `ensure_job!` macro (same pattern as `JOB_DAILY_DIGEST`):

```rust
    ensure_job!(
        JOB_INSIGHT_PROGRESS,
        scheduling::CronSchedule::Cron {
            expr: "0 3 * * *".to_string(),
            tz: None,
        },
        "Recompute insight learning progress snapshots"
    );
```

**Cron handler registration** — the handler needs `InsightService` which isn't available in `init_cron()`. Register it in `init/mod.rs` **after** AppCore is assembled, before `spawn_background()`. This follows the same pattern as `spawn_post_core_services`.

In `crates/app-core/src/init/mod.rs`, add after the AppCore construction (after `let core = AppCore { ... };`):

```rust
        // ── Register insight progress cron handler ────────────────────────
        if let Some(ref insight_svc) = core.insight_service {
            let svc = Arc::clone(insight_svc);
            let note_repo_clone = core.note_repo.clone();
            let rt = tokio::runtime::Handle::current();
            cron_service.register_handler(
                cron::JOB_INSIGHT_PROGRESS,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let svc = Arc::clone(&svc);
                    let note_repo = note_repo_clone.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            cron::refresh_insight_progress(&svc, &note_repo).await
                        })
                    })
                }),
            );
        }
```

Make `JOB_INSIGHT_PROGRESS` and `refresh_insight_progress` public in `cron.rs` so `init/mod.rs` can reference them.

Add the refresh function:

```rust
async fn refresh_insight_progress(
    svc: &feature_insights::InsightService,
    note_repo: &feature_notes::repo::NoteRepo,
) -> Result<Option<String>, String> {
    // Get all notes that have insights
    // For each, compute progress on the latest version
    let mut refreshed = 0u32;

    // Query all distinct note_ids from insight_reviews that have progress snapshots
    // For simplicity, iterate latest insights and recompute
    // A production optimization would only recompute when flashcard data changed
    let all_notes = note_repo
        .list_notes(None)
        .await
        .map_err(|e| e.to_string())?;

    for note in &all_notes {
        if let Ok(Some(latest)) = svc.get_latest(&note.id).await {
            if let Err(e) = svc.compute_progress(&latest.id, &note.body).await {
                tracing::debug!("progress refresh failed for {}: {e}", note.id);
            } else {
                refreshed += 1;
            }
        }
    }

    if refreshed > 0 {
        Ok(Some(format!("Refreshed {refreshed} insight progress snapshots")))
    } else {
        Ok(None)
    }
}
```

- [ ] **Step 2: Build + test**

Run: `cargo build --workspace`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/
git commit -m "feat(app-core): add daily cron job for insight progress refresh"
```

---

### Task 8: Final Verification

- [ ] **Step 1: Full test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Manual smoke test**

Start the app with `cargo tauri dev`. Generate an insight for a note. Then test the evolution endpoint:

```bash
NOTE_ID="<your-note-id>"
curl -s http://localhost:3456/api/note_insight_get_evolution \
  -X POST -H "Content-Type: application/json" \
  -d "{\"noteId\": \"$NOTE_ID\"}" | python3 -m json.tool
```

Expected: JSON with `noteId` and `versions` array containing at least one `InsightEvolutionPoint` with `changeNote: "Initial insight generated"`.

- [ ] **Step 5: Commit if needed**

```bash
cargo fmt --all
git add -A && git commit -m "style: format Insight Review V2 Phase 3"
```
