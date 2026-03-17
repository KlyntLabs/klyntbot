# Insight Review V2 — Phase 2: Smart Merge + Scope Resolution

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add configurable scope resolution (backlinks/semantic/project/manual), smart merge deduplication with parent insight injection, cognitive context injection (medium tier), and real embedding storage — so insights are smarter, less redundant, and context-aware.

**Architecture:** New files `scope.rs`, `merge.rs`, `prompt_builder.rs` in `feature-insights`. A `ScopeResolver` resolves note IDs from `ScopeConfig`. `SmartMergeEngine` finds overlapping parent insights (Jaccard > 0.60) and injects their summaries into prompts. `PromptBuilder` assembles note context + cognitive data (facts, memories, rules) + optional parent context. A concrete `InsightEmbedderImpl` adapter in `app-core` wires `EmbeddingEngine` + `VectorStore` for real embedding storage. A concrete `CognitiveAccessorImpl` in `app-core` wires cognitive repos for context injection. The `InsightService` gains `scope_resolver`, `merge_engine`, and `prompt_builder` fields; `generate()` replaces the inline pipeline.

**Tech Stack:** Rust (tokio, sqlx, serde_json, sha2, fastembed), SQLite, LanceDB, existing `EmbeddingEngine` (384-dim paraphrase-multilingual-MiniLM-L12-v2)

**Spec:** `docs/superpowers/specs/2026-03-17-insight-review-v2-design.md` (Sections 4-6)

**Scope:** Backend only. No frontend changes. No config schema changes (all defaults live in code). Deep dive mode (Section 5.2) is Phase 4. Learning progress improvements (Section 7) are Phase 3.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/feature-insights/src/scope.rs` | `ScopeResolver` — resolves note IDs from `ScopeConfig` (backlinks/semantic/project/manual) |
| `crates/feature-insights/src/merge.rs` | `SmartMergeEngine` — finds parent insights via Jaccard overlap, builds merge context |
| `crates/feature-insights/src/prompt_builder.rs` | `PromptBuilder` — assembles full insight context from notes + cognitive data + parent context |
| `crates/storage/src/vector_store/insight_embeddings.rs` | Schema + table creation for `insight_embeddings` LanceDB table |
| `crates/app-core/src/adapters/mod.rs` | Module registration for app-core adapters |
| `crates/app-core/src/adapters/insight_embedder.rs` | `InsightEmbedderImpl` — concrete `InsightEmbedder` using `EmbeddingEngine` + `VectorStore` |
| `crates/app-core/src/adapters/cognitive_accessor.rs` | `CognitiveAccessorImpl` — concrete `CognitiveAccessor` using cognitive repos |

### Modified files

| File | Change |
|------|--------|
| `crates/feature-insights/src/lib.rs` | Add `pub mod scope`, `pub mod merge`, `pub mod prompt_builder` + re-exports |
| `crates/feature-insights/src/service.rs` | Add `scope_resolver`, `merge_engine`, `prompt_builder`, `cognitive` fields; update `new()` constructor; add `generate()` method |
| `crates/feature-insights/src/types.rs` | No changes needed |
| `crates/feature-insights/Cargo.toml` | No new deps needed (already has all required) |
| `crates/storage/src/vector_store/schemas.rs` | Add `insight_embedding_schema()` |
| `crates/storage/src/vector_store/mod.rs` | Create `insight_embeddings` table in `connect()`, add `ensure_indexes()` entry |
| `crates/app-core/src/lib.rs` | Add `pub mod adapters` |
| `crates/app-core/src/init/mod.rs` | Wire real `InsightEmbedderImpl` + `CognitiveAccessorImpl` into `InsightService::new()` |
| `crates/app-core/src/handlers/notes/insight.rs` | Pass `ScopeConfig` through pipeline; use `InsightService::generate()` |
| `crates/app-core/Cargo.toml` | Add `tools` dep (for `EmbeddingEngine`) |

---

## Chunk 1: Scope Resolution

### Task 1: ScopeResolver trait + default implementation

**Files:**
- Create: `crates/feature-insights/src/scope.rs`
- Modify: `crates/feature-insights/src/lib.rs`

The `ScopeResolver` resolves a `ScopeConfig` into a list of related note IDs. It's defined as a trait for testability (mock in tests, real impl in app-core). The default implementation needs access to `NoteRepo` (for backlinks and project-scoped notes) and `VectorStore` (for semantic search).

Since `ScopeResolver` needs infrastructure from higher layers (NoteRepo at L4, VectorStore at L2), we define the trait in `feature-insights` and implement it in `app-core` where both are available.

- [ ] **Step 1: Create scope.rs with ScopeResolver trait**

Create `crates/feature-insights/src/scope.rs`:

```rust
//! Scope resolution for insight generation.
//!
//! Resolves a `ScopeConfig` into a list of related note IDs that will be
//! fed into the insight context. Four scope types are supported:
//! - Backlinks: wikilink references (current default)
//! - Semantic: LanceDB embedding similarity
//! - Project: all notes in the same notebook
//! - Manual: user-selected note IDs

use async_trait::async_trait;

use crate::types::ScopeConfig;

/// Resolves note IDs from a scope configuration.
///
/// Defined here in `feature-insights` (L4), implemented in `app-core` (L7)
/// where `NoteRepo` and `VectorStore` are available. Injected into
/// `InsightService` as `Arc<dyn ScopeResolver>`.
#[async_trait]
pub trait ScopeResolver: Send + Sync {
    /// Resolve the scope config into a list of related note IDs.
    /// The returned IDs should NOT include `note_id` itself.
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String>;
}

/// No-op resolver for testing — returns empty scope.
pub struct NoopScopeResolver;

#[async_trait]
impl ScopeResolver for NoopScopeResolver {
    async fn resolve(&self, _note_id: &str, _config: &ScopeConfig) -> Vec<String> {
        Vec::new()
    }
}

/// Test helper: returns a fixed set of IDs.
#[cfg(test)]
pub struct FixedScopeResolver(pub Vec<String>);

#[cfg(test)]
#[async_trait]
impl ScopeResolver for FixedScopeResolver {
    async fn resolve(&self, _note_id: &str, _config: &ScopeConfig) -> Vec<String> {
        self.0.clone()
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod scope;
pub use scope::{ScopeResolver, NoopScopeResolver};
```

- [ ] **Step 3: Build**

Run: `cargo build -p feature-insights`

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/scope.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add ScopeResolver trait with noop impl"
```

---

### Task 2: SmartMergeEngine — Jaccard overlap + parent detection

**Files:**
- Create: `crates/feature-insights/src/merge.rs`
- Modify: `crates/feature-insights/src/lib.rs`

The merge engine detects when a new insight overlaps significantly with an existing one. It computes Jaccard similarity between scope note ID sets and returns the best parent if overlap exceeds the threshold.

- [ ] **Step 1: Write the failing test for scope_overlap**

Create `crates/feature-insights/src/merge.rs`:

```rust
//! Smart Merge Engine — deduplication and parent insight detection.
//!
//! Before generating a new insight, the merge engine checks:
//! 1. Exact hash match (same content → return cached)
//! 2. Scope overlap via Jaccard similarity (> threshold → set as parent)
//!
//! When a parent is found, the prompt builder injects the parent's synthesis
//! so the LLM focuses on what's new or different.

use std::collections::HashSet;

use crate::repo::InsightReviewRepo;
use crate::types::{InsightReviewRow, ScopeConfig};

/// Computes Jaccard similarity between two sets of note IDs.
pub fn scope_overlap(scope_a: &[String], scope_b: &[String]) -> f64 {
    let set_a: HashSet<&str> = scope_a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = scope_b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Result of a merge check.
#[derive(Debug)]
pub struct MergeResult {
    /// If Some, this insight should be the parent (high scope overlap).
    pub parent: Option<InsightReviewRow>,
    /// The Jaccard overlap score with the parent (0.0 if no parent).
    pub overlap_score: f64,
}

/// Engine for detecting overlapping insights and selecting parents.
#[derive(Debug, Clone)]
pub struct SmartMergeEngine {
    repo: InsightReviewRepo,
}

impl SmartMergeEngine {
    pub fn new(repo: InsightReviewRepo) -> Self {
        Self { repo }
    }

    /// Check for a parent insight among existing insights for the scope notes.
    ///
    /// Searches all non-superseded insights whose `note_id` is in `scope_note_ids`,
    /// parses their `scope_config` to extract their scope, and computes Jaccard
    /// overlap with the current scope. Returns the best match if above threshold.
    pub async fn find_parent(
        &self,
        note_id: &str,
        scope_note_ids: &[String],
        merge_threshold: f64,
    ) -> Result<MergeResult, sqlx::Error> {
        if scope_note_ids.is_empty() {
            return Ok(MergeResult {
                parent: None,
                overlap_score: 0.0,
            });
        }

        // Collect candidate insights: latest non-superseded insight for each note in scope
        let mut candidates: Vec<(InsightReviewRow, f64)> = Vec::new();

        for scope_note_id in scope_note_ids {
            if scope_note_id == note_id {
                continue; // Don't match against our own note's insights
            }
            if let Some(row) = self.repo.get_latest(scope_note_id).await? {
                // Parse the stored scope_config to get the note IDs that insight used
                let stored_scope: ScopeConfig =
                    serde_json::from_str(&row.scope_config).unwrap_or_default();
                let stored_ids = &stored_scope.node_ids;

                let overlap = scope_overlap(scope_note_ids, stored_ids);
                if overlap >= merge_threshold {
                    candidates.push((row, overlap));
                }
            }
        }

        // Pick the best parent: highest overlap, then most recent
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.0.generated_at.cmp(&a.0.generated_at))
        });

        match candidates.into_iter().next() {
            Some((row, score)) => Ok(MergeResult {
                parent: Some(row),
                overlap_score: score,
            }),
            None => Ok(MergeResult {
                parent: None,
                overlap_score: 0.0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_overlap_identical() {
        let a = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let b = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!((scope_overlap(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_partial() {
        let a = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let b = vec!["b".to_string(), "c".to_string(), "d".to_string()];
        // intersection={b,c}=2, union={a,b,c,d}=4 → 0.5
        assert!((scope_overlap(&a, &b) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_disjoint() {
        let a = vec!["a".to_string(), "b".to_string()];
        let b = vec!["c".to_string(), "d".to_string()];
        assert!((scope_overlap(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_empty() {
        let a: Vec<String> = vec![];
        let b: Vec<String> = vec![];
        assert!((scope_overlap(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_one_empty() {
        let a = vec!["a".to_string()];
        let b: Vec<String> = vec![];
        assert!((scope_overlap(&a, &b)).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p feature-insights -E 'test(scope_overlap)'`
Expected: all 5 tests pass.

- [ ] **Step 3: Register module in lib.rs**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod merge;
pub use merge::SmartMergeEngine;
```

- [ ] **Step 4: Build**

Run: `cargo build -p feature-insights`

- [ ] **Step 5: Commit**

```bash
git add crates/feature-insights/src/merge.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add SmartMergeEngine with Jaccard overlap and parent detection"
```

---

### Task 3: PromptBuilder — cognitive context assembly

**Files:**
- Create: `crates/feature-insights/src/prompt_builder.rs`
- Modify: `crates/feature-insights/src/lib.rs`

The PromptBuilder assembles the full context string for insight generation. It combines: (1) the target note, (2) related notes from scope resolution, (3) cognitive data from the CognitiveAccessor (medium tier: facts, memories, rules), and (4) optional parent insight summary from smart merge.

This replaces the inline `assemble_context()` in `app-core/handlers/notes/insight_context.rs`.

- [ ] **Step 1: Create prompt_builder.rs**

Create `crates/feature-insights/src/prompt_builder.rs`:

```rust
//! Prompt context builder for insight generation.
//!
//! Assembles the full context string from:
//! 1. Target note (title + body)
//! 2. Related notes from scope resolution
//! 3. Cognitive data (facts, episodic memories, procedural rules)
//! 4. Parent insight summary (from smart merge, if applicable)

use std::sync::Arc;

use crate::traits::CognitiveAccessor;
use crate::types::{InsightContent, InsightReviewRow, ScopeConfig};

/// Assembled context ready for prompt injection.
pub struct InsightContext {
    pub text: String,
    pub note_title: String,
    pub related_count: usize,
}

/// Note data needed by the prompt builder.
/// Uses simple types to avoid depending on feature_notes::NoteRow directly.
pub struct NoteData {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// Builds the full context for insight prompt injection.
pub struct PromptBuilder {
    cognitive: Arc<dyn CognitiveAccessor>,
}

impl PromptBuilder {
    pub fn new(cognitive: Arc<dyn CognitiveAccessor>) -> Self {
        Self { cognitive }
    }

    /// Assemble the full context for insight generation.
    ///
    /// - `note`: the target note
    /// - `related_notes`: notes resolved by the ScopeResolver
    /// - `scope_config`: controls whether cognitive data is included
    /// - `domains`: domain hints extracted from note tags
    /// - `parent`: optional parent insight from smart merge
    pub async fn build_context(
        &self,
        note: &NoteData,
        related_notes: &[NoteData],
        scope_config: &ScopeConfig,
        domains: &[String],
        parent: Option<&InsightReviewRow>,
    ) -> InsightContext {
        let mut sections: Vec<String> = Vec::new();

        // Section 1: Target note
        sections.push(format!("## Current Note: {}\n\n{}", note.title, note.body));

        // Section 2: Related notes
        for related in related_notes {
            let body_preview = truncate_body(&related.body, 2000);
            sections.push(format!(
                "## Related Note: {}\n\n{}",
                related.title, body_preview
            ));
        }

        // Section 3: Cognitive context (medium tier, when enabled)
        if scope_config.include_cognitive {
            let domain = domains.first().map(|s| s.as_str());

            let facts = self
                .cognitive
                .search_facts(&note.title, domain, 10)
                .await;
            if !facts.is_empty() {
                sections.push(format!(
                    "## Relevant Knowledge\n\n{}",
                    facts
                        .iter()
                        .map(|f| format!("- {f}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }

            let memories = self.cognitive.recent_memories(&note.id, 5).await;
            if !memories.is_empty() {
                sections.push(format!(
                    "## Recent Learning Sessions\n\n{}",
                    memories
                        .iter()
                        .map(|m| format!("- {m}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }

            if let Some(d) = domain {
                let rules = self.cognitive.domain_rules(d).await;
                if !rules.is_empty() {
                    sections.push(format!(
                        "## Domain Insights\n\n{}",
                        rules
                            .iter()
                            .map(|r| format!("- {r}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
            }
        }

        // Section 4: Parent insight context (from smart merge)
        if let Some(parent_row) = parent {
            if let Ok(parent_content) =
                serde_json::from_str::<InsightContent>(&parent_row.content)
            {
                let mut parent_sections = Vec::new();
                if let Some(ref syn) = parent_content.synthesis {
                    parent_sections
                        .push(format!("Prior synthesis:\n{}", truncate_body(syn, 1000)));
                }
                if let Some(ref gaps) = parent_content.gap_analysis {
                    // Extract just the gap bullet points (first 500 chars)
                    parent_sections.push(format!(
                        "Prior gaps identified:\n{}",
                        truncate_body(gaps, 500)
                    ));
                }
                if !parent_sections.is_empty() {
                    sections.push(format!(
                        "## Prior Analysis (from related insight, generated {})\n\n{}\n\n\
                        Focus on what's NEW, DIFFERENT, or CONTRADICTORY compared to this prior analysis. \
                        If the current note closes any prior gaps, note that explicitly.",
                        parent_row.generated_at,
                        parent_sections.join("\n\n")
                    ));
                }
            }
        }

        let related_count = related_notes.len();
        InsightContext {
            text: sections.join("\n\n"),
            note_title: note.title.clone(),
            related_count,
        }
    }
}

/// Truncate a string to approximately `max_chars`, breaking at a word boundary.
fn truncate_body(body: &str, max_chars: usize) -> &str {
    if body.len() <= max_chars {
        return body;
    }
    // Find the last space before max_chars
    match body[..max_chars].rfind(' ') {
        Some(pos) => &body[..pos],
        None => &body[..max_chars],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::NoopCognitiveAccessor;

    #[tokio::test]
    async fn test_build_context_basic() {
        let cognitive = Arc::new(NoopCognitiveAccessor);
        let builder = PromptBuilder::new(cognitive);

        let note = NoteData {
            id: "note-1".to_string(),
            title: "Test Note".to_string(),
            body: "Some content".to_string(),
        };
        let related = vec![NoteData {
            id: "note-2".to_string(),
            title: "Related".to_string(),
            body: "Related content".to_string(),
        }];
        let scope = ScopeConfig::default();

        let ctx = builder
            .build_context(&note, &related, &scope, &[], None)
            .await;

        assert!(ctx.text.contains("## Current Note: Test Note"));
        assert!(ctx.text.contains("## Related Note: Related"));
        assert_eq!(ctx.related_count, 1);
        assert_eq!(ctx.note_title, "Test Note");
    }

    #[tokio::test]
    async fn test_build_context_with_parent() {
        let cognitive = Arc::new(NoopCognitiveAccessor);
        let builder = PromptBuilder::new(cognitive);

        let note = NoteData {
            id: "note-1".to_string(),
            title: "Test".to_string(),
            body: "Content".to_string(),
        };

        let parent_content = InsightContent {
            synthesis: Some("Previous synthesis text".to_string()),
            gap_analysis: Some("- Missing topic X\n- Shallow coverage of Y".to_string()),
            ..Default::default()
        };
        let parent_row = InsightReviewRow {
            id: "parent-id".to_string(),
            note_id: "other-note".to_string(),
            version: 1,
            generated_at: "2026-03-17T00:00:00Z".to_string(),
            content: serde_json::to_string(&parent_content).unwrap(),
            input_hash: "hash".to_string(),
            scope_config: "{}".to_string(),
            persona_ids: "[]".to_string(),
            parent_insight_id: None,
            token_cost_usd: None,
            superseded_at: None,
        };

        let scope = ScopeConfig {
            include_cognitive: false,
            ..Default::default()
        };

        let ctx = builder
            .build_context(&note, &[], &scope, &[], Some(&parent_row))
            .await;

        assert!(ctx.text.contains("## Prior Analysis"));
        assert!(ctx.text.contains("Previous synthesis text"));
        assert!(ctx.text.contains("Missing topic X"));
        assert!(ctx.text.contains("NEW, DIFFERENT, or CONTRADICTORY"));
    }

    #[tokio::test]
    async fn test_build_context_no_cognitive_when_disabled() {
        let cognitive = Arc::new(NoopCognitiveAccessor);
        let builder = PromptBuilder::new(cognitive);

        let note = NoteData {
            id: "note-1".to_string(),
            title: "Test".to_string(),
            body: "Content".to_string(),
        };
        let scope = ScopeConfig {
            include_cognitive: false,
            ..Default::default()
        };

        let ctx = builder
            .build_context(&note, &[], &scope, &[], None)
            .await;

        // With noop cognitive + disabled flag, only the note section should appear
        assert!(ctx.text.contains("## Current Note"));
        assert!(!ctx.text.contains("## Relevant Knowledge"));
        assert!(!ctx.text.contains("## Recent Learning"));
    }

    #[test]
    fn test_truncate_body() {
        assert_eq!(truncate_body("hello world", 20), "hello world");
        assert_eq!(truncate_body("hello world foo bar", 11), "hello world");
        assert_eq!(truncate_body("hello world foo bar", 5), "hello");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p feature-insights -E 'test(build_context) | test(truncate)'`
Expected: all 4 tests pass.

- [ ] **Step 3: Register module in lib.rs**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod prompt_builder;
pub use prompt_builder::{PromptBuilder, InsightContext, NoteData};
```

- [ ] **Step 4: Build**

Run: `cargo build -p feature-insights`

- [ ] **Step 5: Commit**

```bash
git add crates/feature-insights/src/prompt_builder.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add PromptBuilder with cognitive context and parent injection"
```

---

## Chunk 2: Embedding Infrastructure

### Task 4: Add insight_embeddings LanceDB table

**Files:**
- Modify: `crates/storage/src/vector_store/schemas.rs`
- Modify: `crates/storage/src/vector_store/mod.rs`

Add the `insight_embeddings` table to LanceDB for storing insight content embeddings. This enables semantic drift calculation between versions and cross-note dedup search.

The schema follows the same pattern as existing embedding tables: `id (Utf8) | vector (384-dim) | updated_at (Utf8)`. The `id` column stores the `insight_review_id` (the row from `insight_reviews` table).

- [ ] **Step 1: Add schema definition**

In `crates/storage/src/vector_store/schemas.rs`, add at the end:

```rust
pub(crate) fn insight_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

- [ ] **Step 2: Create table in VectorStore::connect()**

In `crates/storage/src/vector_store/mod.rs`, find where tables are created in `connect()` (after `note_embeddings`), and add:

```rust
ensure_table(&db, "insight_embeddings", schemas::insight_embedding_schema()).await?;
```

- [ ] **Step 3: Add index in ensure_indexes()**

In the `ensure_indexes()` method, add `"insight_embeddings"` to the hardcoded array of table names that get IVF-PQ indexes. The existing implementation uses a static array — add the new table name to that array.

- [ ] **Step 4: Build**

Run: `cargo build -p storage`

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/vector_store/
git commit -m "feat(storage): add insight_embeddings LanceDB table"
```

---

### Task 5: InsightEmbedderImpl adapter in app-core

**Files:**
- Create: `crates/app-core/src/adapters/mod.rs`
- Create: `crates/app-core/src/adapters/insight_embedder.rs`
- Modify: `crates/app-core/src/lib.rs`
- Modify: `crates/app-core/Cargo.toml`

Concrete implementation of `InsightEmbedder` using `EmbeddingEngine` (from `tools` crate) and `VectorStore` (from `storage` crate). Follows the same adapter pattern as `NoteEmbeddingAdapter` in the `agent` crate.

- [ ] **Step 1: Add tools dependency to app-core**

In `crates/app-core/Cargo.toml`, add:
```toml
tools.workspace = true
```

- [ ] **Step 2: Create adapters module**

Create `crates/app-core/src/adapters/mod.rs`:
```rust
pub mod insight_embedder;
pub mod cognitive_accessor;
```

In `crates/app-core/src/lib.rs`, add:
```rust
pub mod adapters;
```

- [ ] **Step 3: Create InsightEmbedderImpl**

Create `crates/app-core/src/adapters/insight_embedder.rs`:

```rust
//! Concrete InsightEmbedder — wraps EmbeddingEngine + VectorStore.
//!
//! Follows the same adapter pattern as `NoteEmbeddingAdapter` in the `agent` crate.

use async_trait::async_trait;
use chrono::Utc;
use feature_insights::InsightEmbedder;
use std::sync::Arc;
use tools::embedding_engine::EmbeddingEngine;
use tracing::debug;

pub struct InsightEmbedderImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl InsightEmbedderImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }
}

#[async_trait]
impl InsightEmbedder for InsightEmbedderImpl {
    async fn embed_and_store(&self, insight_id: &str, content: &str) -> Result<(), String> {
        // Truncate to first 2000 chars for embedding (matching note pattern)
        let text = if content.len() > 2000 {
            &content[..2000]
        } else {
            content
        };

        // embed_async takes Arc<Self> as receiver: engine.clone().embed_async(text).await
        let vector = self
            .engine
            .clone()
            .embed_async(text.to_string())
            .await
            .map_err(|e| format!("embedding failed: {e}"))?;

        // upsert_embedding expects &[(&str, &str)] for extra fields
        let updated_at = Utc::now().to_rfc3339();
        let extra_fields: &[(&str, &str)] = &[("updated_at", &updated_at)];

        self.store
            .upsert_embedding("insight_embeddings", insight_id, &vector, extra_fields)
            .await
            .map_err(|e| format!("upsert failed: {e}"))?;

        debug!(insight_id, "embedded insight content");
        Ok(())
    }

    async fn similarity(&self, _id_a: &str, _id_b: &str) -> Option<f64> {
        // Phase 2 placeholder — semantic drift calculation requires fetching
        // raw vectors from LanceDB, which needs a new VectorStore method.
        // Returns None → semantic_drift defaults to 0.0 (no drift detected).
        // Phase 3 adds the full implementation with vector fetch + cosine_similarity.
        None
    }
}
```

**Note:** The `similarity()` method returns `None` for now — semantic drift calculation requires fetching raw vectors from LanceDB, which needs a new `VectorStore` method. This is acceptable because:
1. Semantic drift defaults to 0.0 (no drift) when similarity returns None
2. The `embed_and_store()` method is the critical path for Phase 2 (enables future similarity)
3. Phase 3 adds the full progress computation with vector fetch

- [ ] **Step 4: Build**

Run: `cargo build -p app-core`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/adapters/ crates/app-core/src/lib.rs crates/app-core/Cargo.toml
git commit -m "feat(app-core): add InsightEmbedderImpl adapter for LanceDB embedding storage"
```

---

### Task 6: CognitiveAccessorImpl adapter in app-core

**Files:**
- Create: `crates/app-core/src/adapters/cognitive_accessor.rs`

Concrete implementation of `CognitiveAccessor` that wraps cognitive repos (SemanticFactRepo, EpisodicMemoryRepo, ProceduralRuleRepo). Medium tier only — deep dive methods return empty results.

- [ ] **Step 1: Create CognitiveAccessorImpl**

Create `crates/app-core/src/adapters/cognitive_accessor.rs`:

```rust
//! Concrete CognitiveAccessor — wraps cognitive repos for insight context injection.
//!
//! Medium tier (Phase 2): search_facts, recent_memories, domain_rules.
//! Deep dive (Phase 4): user_model_summary, entity_neighborhood, fact_history.

use async_trait::async_trait;
use feature_insights::CognitiveAccessor;

/// Wraps cognitive repos to provide insight context data.
pub struct CognitiveAccessorImpl {
    fact_repo: cognitive::SemanticFactRepo,
    memory_repo: cognitive::EpisodicMemoryRepo,
    rule_repo: cognitive::ProceduralRuleRepo,
}

impl CognitiveAccessorImpl {
    pub fn new(
        fact_repo: cognitive::SemanticFactRepo,
        memory_repo: cognitive::EpisodicMemoryRepo,
        rule_repo: cognitive::ProceduralRuleRepo,
    ) -> Self {
        Self {
            fact_repo,
            memory_repo,
            rule_repo,
        }
    }
}

#[async_trait]
impl CognitiveAccessor for CognitiveAccessorImpl {
    async fn search_facts(&self, query: &str, domain: Option<&str>, limit: usize) -> Vec<String> {
        // SemanticFact fields: subject, predicate, object (NOT "value")
        // search_fts(query: &str, domain: Option<&str>, limit: usize)
        self.fact_repo
            .search_fts(query, domain, limit)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|f| format!("{} {} {}", f.subject, f.predicate, f.object))
            .collect()
    }

    async fn recent_memories(&self, _note_id: &str, limit: usize) -> Vec<String> {
        // EpisodicMemory fields: content, summary (Option<String>)
        // list_recent(limit: i64) — note: takes i64, not usize
        self.memory_repo
            .list_recent(limit as i64)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.summary.unwrap_or_else(|| m.content))
            .collect()
    }

    async fn domain_rules(&self, domain: &str) -> Vec<String> {
        // ProceduralRule fields: rule_text, confidence
        // list_active(domain: &str)
        self.rule_repo
            .list_active(domain)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| format!("{} (confidence: {:.0}%)", r.rule_text, r.confidence * 100.0))
            .collect()
    }

    // Deep dive methods — Phase 4
    async fn user_model_summary(&self, _domain: &str) -> Option<String> {
        None
    }

    async fn entity_neighborhood(&self, _note_id: &str, _depth: u8) -> Vec<String> {
        Vec::new()
    }

    async fn fact_history(&self, _subject: &str) -> Vec<String> {
        Vec::new()
    }
}
```

**Verified field names against actual types in `crates/cognitive/src/types.rs`:**
- `SemanticFact`: `subject`, `predicate`, `object` (NOT `value`)
- `EpisodicMemory`: `content`, `summary: Option<String>` (NOT always present)
- `ProceduralRule`: `rule_text`, `confidence`

**Verified method signatures:**
- `SemanticFactRepo::search_fts(query: &str, domain: Option<&str>, limit: usize)`
- `EpisodicMemoryRepo::list_recent(limit: i64)` — takes `i64`, not `usize`
- `ProceduralRuleRepo::list_active(domain: &str)`

- [ ] **Step 2: Build**

Run: `cargo build -p app-core`
Fix any field name mismatches based on actual row types.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/adapters/cognitive_accessor.rs
git commit -m "feat(app-core): add CognitiveAccessorImpl adapter for insight context injection"
```

---

## Chunk 3: Wire Everything Together

### Task 7: Update InsightService with new components

**Files:**
- Modify: `crates/feature-insights/src/service.rs`

Extend `InsightService` with the new Phase 2 components: `scope_resolver`, `merge_engine`, `prompt_builder`, `cognitive`. Add a `generate()` method that runs the full pipeline: scope → merge check → prompt build → (delegate LLM to caller). Update `store_insight()` to accept `scope_note_ids` for storing in `scope_config.node_ids`.

The LLM call itself stays in `app-core`'s insight handler (it depends on `DynProvider` and event emission). The service orchestrates everything up to and after the LLM call.

- [ ] **Step 1: Update InsightService struct and constructor**

In `crates/feature-insights/src/service.rs`, update the struct to add new fields:

```rust
pub struct InsightService {
    pub(crate) repo: InsightReviewRepo,
    pub(crate) progress_repo: InsightProgressRepo,
    pub(crate) scope_resolver: Arc<dyn ScopeResolver>,
    pub(crate) merge_engine: SmartMergeEngine,
    pub(crate) prompt_builder: PromptBuilder,
    pub(crate) cognitive: Arc<dyn CognitiveAccessor>,
    pub(crate) flashcards: Arc<dyn FlashcardAccessor>,
    pub(crate) embedder: Arc<dyn InsightEmbedder>,
    pub(crate) progress_weights: ProgressWeights,
}
```

Update `new()` to accept the new parameters. Keep the existing `check_cache()`, `store_insight()`, `get_latest()`, `list_versions()`, `update_tab()`, `compute_input_hash()` methods.

- [ ] **Step 2: Add resolve_and_prepare() method**

Add a method that runs scope resolution + merge check + context building — everything before the LLM call:

```rust
/// Pre-generation pipeline: resolve scope → check merge → build context.
///
/// Returns the assembled context, resolved scope IDs, and optional parent.
/// The caller uses this to make the LLM call, then passes results to `store_insight()`.
pub async fn resolve_and_prepare(
    &self,
    note: &NoteData,
    related_notes: &[NoteData],
    scope_config: &ScopeConfig,
    domains: &[String],
) -> Result<PreparedInsight, sqlx::Error> {
    // 1. Resolve scope
    let scope_note_ids = self
        .scope_resolver
        .resolve(&note.id, scope_config)
        .await;

    // 2. Check for parent via smart merge
    let merge_result = self
        .merge_engine
        .find_parent(&note.id, &scope_note_ids, scope_config.merge_threshold)
        .await?;

    // 3. Build full context
    let context = self
        .prompt_builder
        .build_context(
            note,
            related_notes,
            scope_config,
            domains,
            merge_result.parent.as_ref(),
        )
        .await;

    Ok(PreparedInsight {
        context,
        scope_note_ids,
        parent_insight_id: merge_result.parent.map(|p| p.id),
        overlap_score: merge_result.overlap_score,
    })
}
```

Add the `PreparedInsight` struct:

```rust
/// Result of pre-generation pipeline — ready for LLM call.
pub struct PreparedInsight {
    pub context: InsightContext,
    pub scope_note_ids: Vec<String>,
    pub parent_insight_id: Option<String>,
    pub overlap_score: f64,
}
```

- [ ] **Step 3: Update store_insight() to accept parent and scope IDs**

Update `store_insight()` signature to include parent and scope info:

```rust
pub async fn store_insight(
    &self,
    note_id: &str,
    content: &InsightContent,
    input_hash: &str,
    scope_config: &ScopeConfig,
    persona_ids: &[String],
    parent_insight_id: Option<&str>,
    scope_note_ids: &[String],
) -> Result<InsightReviewRow, sqlx::Error> {
```

Clone the `scope_config`, set `node_ids` to `scope_note_ids`, then serialize. This ensures future merge lookups can compare scopes:

```rust
let mut stored_scope = scope_config.clone();
stored_scope.node_ids = scope_note_ids.to_vec();
```

Pass this `stored_scope` to the repo's `insert()` along with `parent_insight_id`. This is critical for Smart Merge — `find_parent()` reads `scope_config.node_ids` from stored insights to compute Jaccard overlap. Without this, all pre-existing insights would have empty `node_ids` and never match as parents.

- [ ] **Step 4: Build**

Run: `cargo build -p feature-insights`
Fix any compilation errors.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-insights/src/service.rs
git commit -m "feat(feature-insights): extend InsightService with scope, merge, and prompt builder"
```

---

### Task 8: Wire real adapters into AppCore init

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

Replace the `NoopInsightEmbedder` and `NoopCognitiveAccessor` with real implementations. Also inject the `ScopeResolver` (noop for now — the real impl needs `NoteRepo` + `VectorStore` which we'll wire in a separate task).

- [ ] **Step 1: Update InsightService construction in init**

In `crates/app-core/src/init/mod.rs`, update the `insight_service` initialization to wire real adapters:

```rust
// Reuse the EmbeddingEngine already created for note embeddings (avoid double-loading the ~420MB model).
// Clone the Arc<EmbeddingEngine> from the note embedding handler block above.
// If vector store is unavailable, use noop.
let insight_embedder: Arc<dyn feature_insights::InsightEmbedder> =
    if let (Some(ref vs), Some(ref embedding_engine)) = (&vector_store_clone, &embedding_engine_arc) {
        Arc::new(crate::adapters::insight_embedder::InsightEmbedderImpl::new(
            Arc::clone(embedding_engine),
            vs.clone(),
        ))
    } else {
        Arc::new(feature_insights::NoopInsightEmbedder)
    };

let cognitive_accessor: Arc<dyn feature_insights::CognitiveAccessor> =
    Arc::new(crate::adapters::cognitive_accessor::CognitiveAccessorImpl::new(
        cognitive::SemanticFactRepo::new(storage_pool.inner().clone()),
        cognitive::EpisodicMemoryRepo::new(storage_pool.inner().clone()),
        cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone()),
    ));

let insight_repo = feature_insights::InsightReviewRepo::new(storage_pool.inner().clone());

insight_service: Some(Arc::new(feature_insights::InsightService::new(
    insight_repo.clone(),
    feature_insights::InsightProgressRepo::new(storage_pool.inner().clone()),
    Arc::new(feature_insights::NoopScopeResolver),  // Task 9 wires real impl
    feature_insights::SmartMergeEngine::new(insight_repo),
    feature_insights::PromptBuilder::new(Arc::clone(&cognitive_accessor)),
    cognitive_accessor,
    Arc::new(feature_insights::NoopFlashcardAccessor),  // Phase 3
    insight_embedder,
    feature_insights::ProgressWeights::default(),
))),
```

Note: `vector_store` may already have been moved by the time we reach insight init. Check the init flow and clone it earlier if needed. The variable `vector_store_clone` should be created before `vector_store` is moved into the agent init.

- [ ] **Step 2: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p app-core`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): wire real InsightEmbedder and CognitiveAccessor into AppCore init"
```

---

### Task 9: Wire ScopeResolver implementation in app-core

**Files:**
- Create: `crates/app-core/src/adapters/scope_resolver.rs`
- Modify: `crates/app-core/src/adapters/mod.rs`
- Modify: `crates/app-core/src/init/mod.rs`

The real `ScopeResolver` needs `NoteRepo` (for backlinks + project scope) and optionally `VectorStore` (for semantic scope). Since both are available in `app-core`, the adapter lives here.

- [ ] **Step 1: Create ScopeResolverImpl**

Create `crates/app-core/src/adapters/scope_resolver.rs`:

```rust
//! Concrete ScopeResolver — combines backlinks, semantic search, project scope, and manual IDs.

use async_trait::async_trait;
use feature_insights::{ScopeConfig, ScopeResolver, ScopeType};
use feature_notes::repo::NoteRepo;
use tracing::debug;

pub struct ScopeResolverImpl {
    note_repo: NoteRepo,
    vector_store: Option<storage::VectorStore>,
}

impl ScopeResolverImpl {
    pub fn new(note_repo: NoteRepo, vector_store: Option<storage::VectorStore>) -> Self {
        Self {
            note_repo,
            vector_store,
        }
    }

    async fn resolve_backlinks(&self, note_id: &str) -> Vec<String> {
        self.note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(note, _ctx)| note.id)
            .collect()
    }

    async fn resolve_semantic(&self, note_id: &str, radius: f64) -> Vec<String> {
        let Some(ref vs) = self.vector_store else {
            debug!("semantic scope requested but vector store unavailable, falling back to backlinks");
            return self.resolve_backlinks(note_id).await;
        };

        // To do semantic search, we need the note's embedding vector as a query.
        // The embedding_engine is needed to get the query vector.
        // For now, use the note_embeddings table: fetch the note's vector,
        // then search for similar notes.
        //
        // VectorStore::search_similar(table, query_vector, limit, threshold)
        // requires a query vector — we need to get it from the note's stored embedding.
        //
        // Since VectorStore doesn't expose a get_vector_by_id() method yet,
        // we fall back to backlinks + semantic boost: use backlinks as base,
        // then the embedding engine can be added when Phase 4 wires deep dive.
        //
        // TODO(Phase 4): Wire EmbeddingEngine into ScopeResolverImpl to enable
        // true semantic scope via vs.search_similar("note_embeddings", query, 20, radius)
        debug!("semantic scope: falling back to backlinks (vector fetch not yet wired)");
        self.resolve_backlinks(note_id).await
    }

    async fn resolve_project(&self, note_id: &str) -> Vec<String> {
        // Get the note's notebook_id, then list all notes in that notebook
        match self.note_repo.get_note(note_id).await {
            Ok(Some(note)) => {
                let notebook_id = note.notebook_id.as_deref();
                self.note_repo
                    .list_notes(notebook_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|n| n.id != note_id)
                    .map(|n| n.id)
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

#[async_trait]
impl ScopeResolver for ScopeResolverImpl {
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String> {
        let mut ids = match config.scope_type {
            ScopeType::Backlinks => self.resolve_backlinks(note_id).await,
            ScopeType::Semantic => self.resolve_semantic(note_id, config.radius).await,
            ScopeType::Project => self.resolve_project(note_id).await,
            ScopeType::Manual => config.node_ids.clone(),
        };

        // Always include backlinks regardless of scope type
        if !matches!(config.scope_type, ScopeType::Backlinks) {
            let backlinks = self.resolve_backlinks(note_id).await;
            for bl in backlinks {
                if !ids.contains(&bl) {
                    ids.push(bl);
                }
            }
        }

        ids.sort();
        ids.dedup();
        ids
    }
}
```

**IMPORTANT:** The semantic search needs the note's embedding vector as a query. The `VectorStore::search_similar()` method takes a query vector. The implementer must:
1. Read `VectorStore::search_similar()` in `crates/storage/src/vector_store/crud.rs` to confirm its signature
2. Fetch the note's embedding from `note_embeddings` table first, then use it as the query vector
3. If the note has no embedding, fall back to backlinks

This may require adding a `get_embedding()` method to `VectorStore`, or fetching via a LanceDB query. The implementer should check what methods are available and adapt.

- [ ] **Step 2: Register in adapters/mod.rs**

Add to `crates/app-core/src/adapters/mod.rs`:
```rust
pub mod scope_resolver;
```

- [ ] **Step 3: Wire into AppCore init**

Replace `NoopScopeResolver` with `ScopeResolverImpl` in init:

```rust
Arc::new(crate::adapters::scope_resolver::ScopeResolverImpl::new(
    note_repo.clone(),
    vector_store_clone.clone(),
)),
```

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run --workspace`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/adapters/ crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): add ScopeResolverImpl with backlinks, semantic, and project scope"
```

---

### Task 10: Update insight handler to use new pipeline

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

Update the insight handler to use `InsightService::resolve_and_prepare()` for context building instead of the inline pipeline. The LLM calls and event emission stay in the handler. The key change: scope resolution + merge + cognitive context injection now happen through the service.

- [ ] **Step 1: Update note_insight_review()**

The handler should:
1. Resolve scope via `service.resolve_and_prepare()` (new)
2. Check cache using the hash from resolved scope (existing, but hash now includes scope IDs)
3. Pass the prepared context to the pipeline (existing LLM call flow)
4. Store with parent info and scope IDs (updated `store_insight()`)

Read the current `insight.rs` handler carefully before making changes. The goal is minimal disruption — only the context assembly and storage calls change. The LLM pipeline (`stream_synthesis`, `generate_tab`, event emission) stays identical.

Key adapter pattern — convert `NoteRow` to `NoteData` and fetch notes for resolved scope:

```rust
// Convert the target note
let note_data = feature_insights::NoteData {
    id: note.id.clone(),
    title: note.title.clone(),
    body: note.body.clone(),
};

// Fetch related notes for the resolved scope IDs
// (replaces the old fetch_related_notes which only used backlinks)
let scope_ids = service.scope_resolver.resolve(note_id, &scope).await;
let mut related_note_data = Vec::new();
for id in &scope_ids {
    if let Ok(Some(n)) = self.note_repo.get_note(id).await {
        related_note_data.push(feature_insights::NoteData {
            id: n.id, title: n.title, body: n.body,
        });
    }
}

// Run the pre-generation pipeline
let prepared = service.resolve_and_prepare(&note_data, &related_note_data, &scope, &domains).await;

// Pass to pipeline:
InsightPipelineArgs {
    // ... existing fields ...
    context: prepared.context.text,
    note_title: prepared.context.note_title,
    parent_insight_id: prepared.parent_insight_id.clone(),
    scope_note_ids: prepared.scope_note_ids.clone(),
    scope_config: scope.clone(),
}
```

And in `run_insight_pipeline()`, after all tabs complete:

```rust
let _ = service.store_insight(
    &note_id,
    &content,
    &content_hash,
    &scope_config,
    &persona_ids,
    parent_insight_id.as_deref(),
    &scope_note_ids,
).await;
```

- [ ] **Step 2: Update InsightPipelineArgs**

Add new fields to the pipeline args struct:

```rust
struct InsightPipelineArgs {
    provider: providers::DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    insight_service: Option<Arc<feature_insights::InsightService>>,
    note_id: String,
    content_hash: String,
    context: String,
    note_title: String,
    params: providers::ChatParams,
    personas: Vec<cognitive::PersonaRow>,
    parent_insight_id: Option<String>,
    scope_note_ids: Vec<String>,
    scope_config: feature_insights::ScopeConfig,
}
```

- [ ] **Step 3: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run --workspace`

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(app-core): use InsightService pipeline for scope resolution and context building"
```

---

### Task 11: Final Verification

- [ ] **Step 1: Full test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Manual smoke test**

Start the app with `cargo tauri dev`. Open a note that has backlinks. Trigger insight review. Verify:
1. Insight generates with cognitive context (check if "Relevant Knowledge" or "Recent Learning Sessions" sections appear in the synthesis)
2. The insight is stored with a non-default `scope_config` in the DB
3. Embedding is stored in LanceDB (check logs for "embedded insight content")

- [ ] **Step 5: Commit if needed**

```bash
git add -A && git commit -m "style: format Insight Review V2 Phase 2"
```
