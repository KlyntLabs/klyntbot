# Unified Memory Retrieval Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify conversation recall and cognitive fact retrieval into a single `UnifiedMemoryService` behind an enriched `MemoryRetriever` trait, with RRF merge, deduplication, and grouped prompt formatting.

**Architecture:** Two separate memory injection paths (conversation recall via `MemoryRetriever`, cognitive facts via `CognitiveContextSource`) are consolidated. `UnifiedMemoryService` fetches from both sources concurrently, normalizes scores, merges via RRF, and deduplicates. `CognitiveContextSource` is stripped to static identity context only.

**Tech Stack:** Rust, async_trait, tokio::join!, SQLite (ephemeral for tests)

**Spec:** `docs/superpowers/specs/2026-03-11-unified-memory-retrieval-design.md`

---

## File Structure

| File | Responsibility | Action |
|------|---------------|--------|
| `crates/context_engine/src/memory_retriever.rs` | `MemoryRetriever` trait + `MemoryEntry` + `MemorySource` types | Modify |
| `crates/context_engine/src/lib.rs` | Re-exports for context_engine | Modify |
| `crates/cognitive/src/memory_retriever.rs` | `UnifiedMemoryService` — merge, normalize, dedup | Rewrite |
| `crates/cognitive/src/context_source.rs` | `CognitiveContextSource` — static facts + rules only | Modify |
| `crates/cognitive/src/lib.rs` | Re-exports for cognitive crate | Modify |
| `crates/context_engine/src/assembler.rs` | `retrieve_memory()` — grouped formatting by source | Modify |
| `crates/agent/src/agent_loop/builder.rs` | Wire `UnifiedMemoryService` replacing `CognitiveMemoryRetriever` | Modify |

---

## Chunk 1: Enriched Trait + UnifiedMemoryService

### Task 1: Enrich `MemoryEntry` and add `MemorySource`

**Files:**
- Modify: `crates/context_engine/src/memory_retriever.rs`
- Modify: `crates/context_engine/src/lib.rs:19`

- [ ] **Step 1: Add `MemorySource` enum and update `MemoryEntry`**

In `crates/context_engine/src/memory_retriever.rs`, add the enum before `MemoryEntry` and add two fields:

```rust
/// Where a memory result originated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySource {
    /// Extracted/consolidated semantic fact (FSRS-scored).
    CognitiveFact,
    /// Past conversation message (time-decay scored).
    ConversationRecall,
}

/// A single retrieved memory entry.
pub struct MemoryEntry {
    /// Unique identifier (e.g., fact ID or conversation message ID).
    pub id: String,
    /// Text content of the memory.
    pub content: String,
    /// Final relevance score after merge (0.0–1.0; higher = more relevant).
    pub score: f64,
    /// Which retrieval path produced this entry.
    pub source: MemorySource,
    /// Original score from the source before RRF normalization.
    pub raw_score: f64,
}
```

- [ ] **Step 2: Update the re-export in `lib.rs`**

In `crates/context_engine/src/lib.rs`, line 19, change:

```rust
pub use memory_retriever::{MemoryEntry, MemoryRetriever};
```

to:

```rust
pub use memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};
```

- [ ] **Step 3: Fix the mock in `memory_retriever.rs` tests**

Update the `MockMemoryRetriever` in the same file's `#[cfg(test)]` block to include the new fields:

```rust
impl MemoryRetriever for MockMemoryRetriever {
    async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.entries
            .iter()
            .take(limit)
            .map(|e| MemoryEntry {
                id: e.id.clone(),
                content: e.content.clone(),
                score: e.score,
                source: e.source.clone(),
                raw_score: e.raw_score,
            })
            .collect()
    }
}
```

And update `test_memory_retriever_limit` and `test_memory_retriever_empty` to construct entries with `source: MemorySource::CognitiveFact` and `raw_score: <same as score>`.

- [ ] **Step 4: Run tests to verify**

Run: `cargo nextest run -p context_engine -E 'test(memory_retriever)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/memory_retriever.rs crates/context_engine/src/lib.rs
git commit -m "feat(context_engine): enrich MemoryEntry with MemorySource and raw_score"
```

---

### Task 2: Implement `UnifiedMemoryService`

**Files:**
- Rewrite: `crates/cognitive/src/memory_retriever.rs`
- Modify: `crates/cognitive/src/lib.rs:30`

This is the core of the unification. Replaces `CognitiveMemoryRetriever`.

- [ ] **Step 1: Write failing tests for RRF merge, dedup, and single-source modes**

Replace the test module in `crates/cognitive/src/memory_retriever.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use context_engine::memory_retriever::MemorySource;

    #[test]
    fn test_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<UnifiedMemoryService>();
    }

    #[test]
    fn test_normalize_scores_min_max() {
        let scores = vec![0.3, 0.6, 0.9];
        let normalized = normalize_scores(&scores);
        assert!((normalized[0] - 0.0).abs() < f64::EPSILON);
        assert!((normalized[1] - 0.5).abs() < f64::EPSILON);
        assert!((normalized[2] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_scores_single_element() {
        let scores = vec![0.5];
        let normalized = normalize_scores(&scores);
        assert!((normalized[0] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_scores_empty() {
        let scores: Vec<f64> = vec![];
        let normalized = normalize_scores(&scores);
        assert!(normalized.is_empty());
    }

    #[test]
    fn test_normalize_scores_all_equal() {
        let scores = vec![0.5, 0.5, 0.5];
        let normalized = normalize_scores(&scores);
        // All equal → all get 1.0
        assert!(normalized.iter().all(|&s| (s - 1.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_rrf_score_math() {
        // Verify RRF formula: 1/(k + rank + 1)
        let k = RRF_K;
        let rank_0 = 1.0 / (k + 0.0 + 1.0); // ~0.01639
        let rank_1 = 1.0 / (k + 1.0 + 1.0); // ~0.01613

        // Item at rank 0 in both lists gets 2 * rank_0
        let dual_score = rank_0 + rank_0;
        // Item at rank 0 in one list only
        let single_score = rank_0;

        assert!(
            dual_score > single_score,
            "Item in both lists should score higher than item in one"
        );
        assert!((rank_0 - 1.0 / 61.0).abs() < 1e-10);
        assert!((rank_1 - 1.0 / 62.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_retrieve_facts_only_no_recall() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = crate::repos::SemanticFactRepo::new(pool);

        // Insert a fact so fallback path returns something
        let fact = crate::types::SemanticFact {
            id: "f1".into(),
            domain: "preferences".into(),
            subject: "user".into(),
            predicate: "editor".into(),
            object: "neovim".into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06T10:00:00".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        };
        fact_repo.upsert(&fact).await.unwrap();

        let service = UnifiedMemoryService::new(fact_repo);
        // No recall wired — should still return facts
        let results = service.retrieve("what editor", 10).await;
        assert!(!results.is_empty(), "Should return facts even without recall");
        assert!(results.iter().all(|r| r.source == MemorySource::CognitiveFact));
    }

    #[tokio::test]
    async fn test_retrieve_empty_when_both_sources_empty() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = crate::repos::SemanticFactRepo::new(pool);

        let service = UnifiedMemoryService::new(fact_repo);
        let results = service.retrieve("anything", 10).await;
        assert!(results.is_empty(), "Should return empty when no data");
    }

    #[test]
    fn test_dedup_facts_win_over_recalls() {
        let fact_content = "user: peak_hours = 10am-12pm";
        let recall_content = "I mentioned my peak hours are around 10am-12pm yesterday";

        assert!(
            content_overlaps(fact_content, recall_content, "peak_hours"),
            "Should detect overlap via predicate substring"
        );
    }

    #[test]
    fn test_dedup_no_false_positive() {
        let fact_content = "user: peak_hours = 10am-12pm";
        let recall_content = "Let's schedule a meeting tomorrow";

        assert!(
            !content_overlaps(fact_content, recall_content, "peak_hours"),
            "Should not detect overlap when predicate absent from recall"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(memory_retriever)'`
Expected: FAIL — `UnifiedMemoryService`, `normalize_scores`, `rrf_merge`, `content_overlaps` not defined

- [ ] **Step 3: Implement the helper functions and `UnifiedMemoryService`**

Rewrite `crates/cognitive/src/memory_retriever.rs`:

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};
use tokio::sync::Mutex;
use tracing::warn;

use crate::context_source::CognitiveRetrievalConfig;
use crate::conversation_recall::ConversationRecallService;
use crate::embedder::SemanticFactEmbedder;
use crate::repos::SemanticFactRepo;
use crate::retrieval::{retrieve_relevant_facts, RetrievalParams};
use crate::situation::UserSituation;

/// RRF constant — same as used in retrieval.rs BM25 merge.
const RRF_K: f64 = 60.0;

/// Retrieval domains for cognitive facts.
const RETRIEVAL_DOMAINS: &[&str] = &[
    "identity",
    "energy",
    "work",
    "finance",
    "learning",
    "preferences",
    "relationships",
    "health",
    "other",
];

/// Unified memory service that merges conversation recall and cognitive facts.
///
/// Replaces `CognitiveMemoryRetriever`. Fetches from both sources concurrently,
/// normalizes scores via min-max, merges via RRF, deduplicates (facts win),
/// and returns a single ranked list.
pub struct UnifiedMemoryService {
    recall: Option<Arc<ConversationRecallService>>,
    fact_repo: SemanticFactRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    config: CognitiveRetrievalConfig,
    situation: Option<Arc<Mutex<UserSituation>>>,
}

impl UnifiedMemoryService {
    pub fn new(fact_repo: SemanticFactRepo) -> Self {
        Self {
            recall: None,
            fact_repo,
            embedder: None,
            config: CognitiveRetrievalConfig::default(),
            situation: None,
        }
    }

    pub fn with_recall(mut self, recall: Arc<ConversationRecallService>) -> Self {
        self.recall = Some(recall);
        self
    }

    pub fn with_recall_opt(mut self, recall: Option<Arc<ConversationRecallService>>) -> Self {
        self.recall = recall;
        self
    }

    pub fn with_embedder_opt(mut self, embedder: Option<Arc<dyn SemanticFactEmbedder>>) -> Self {
        self.embedder = embedder;
        self
    }

    pub fn with_config(mut self, config: CognitiveRetrievalConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_situation(mut self, situation: Arc<Mutex<UserSituation>>) -> Self {
        self.situation = Some(situation);
        self
    }

    async fn current_situational_boost(&self) -> f64 {
        let Some(ref s) = self.situation else {
            return 0.0;
        };
        let guard = s.lock().await;
        (guard.energy_level * 0.25
            + guard.focus_state * 0.30
            + guard.deadline_pressure * 0.25
            + (1.0 - guard.distraction_risk) * 0.20)
            .clamp(0.0, 1.0)
    }

    async fn fetch_facts(&self, query: &str, limit: usize) -> Vec<(String, f64, String)> {
        if !self.config.dynamic_facts_enabled || query.is_empty() {
            return Vec::new();
        }

        let situational_boost = self.current_situational_boost().await;
        let params = RetrievalParams {
            limit,
            vector_top_k: self.config.vector_top_k,
            min_similarity: self.config.min_similarity,
            situational_boost,
            max_stability: self.config.max_stability,
            relevance_weight_semantic: self.config.relevance_weight_semantic,
            relevance_weight_retrievability: self.config.relevance_weight_retrievability,
            relevance_weight_importance: self.config.relevance_weight_importance,
            relevance_weight_frequency: self.config.relevance_weight_frequency,
            relevance_weight_situation: self.config.relevance_weight_situation,
        };

        match retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            RETRIEVAL_DOMAINS,
            &params,
        )
        .await
        {
            Ok(facts) => facts
                .into_iter()
                .filter(|f| f.score > 0.3)
                .map(|f| {
                    let content = format!(
                        "{}: {} = {}",
                        f.fact.subject, f.fact.predicate, f.fact.object
                    );
                    (f.fact.id, f.score, content)
                })
                .collect(),
            Err(e) => {
                warn!("Cognitive fact retrieval failed: {e}");
                Vec::new()
            }
        }
    }

    async fn fetch_recalls(&self, query: &str, limit: usize) -> Vec<(String, f64, String)> {
        let Some(ref recall) = self.recall else {
            return Vec::new();
        };

        match recall
            .search(query, limit, recall.config().default_threshold)
            .await
        {
            Ok(results) => results
                .into_iter()
                .map(|r| (r.id, r.score, r.content))
                .collect(),
            Err(e) => {
                warn!("Conversation recall search failed: {e}");
                Vec::new()
            }
        }
    }
}

#[async_trait]
impl MemoryRetriever for UnifiedMemoryService {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // 1. Fetch concurrently
        let (facts_raw, recalls_raw) = tokio::join!(
            self.fetch_facts(query, limit),
            self.fetch_recalls(query, limit)
        );

        if facts_raw.is_empty() && recalls_raw.is_empty() {
            return Vec::new();
        }

        // 2. Build predicate set for deduplication (facts win)
        let fact_predicates: HashSet<String> = facts_raw
            .iter()
            .filter_map(|(_, _, content)| {
                // Extract predicate from "subject: predicate = object"
                content.split(": ").nth(1).and_then(|rest| {
                    rest.split(" = ").next().map(|p| p.to_lowercase())
                })
            })
            .collect();

        // 3. Filter recalls that overlap with facts
        let recalls_deduped: Vec<(String, f64, String)> = recalls_raw
            .into_iter()
            .filter(|(_, _, content)| {
                !fact_predicates
                    .iter()
                    .any(|pred| content.to_lowercase().contains(pred))
            })
            .collect();

        // 4. Normalize scores within each source
        let fact_scores: Vec<f64> = facts_raw.iter().map(|(_, s, _)| *s).collect();
        let recall_scores: Vec<f64> = recalls_deduped.iter().map(|(_, s, _)| *s).collect();
        let fact_norm = normalize_scores(&fact_scores);
        let recall_norm = normalize_scores(&recall_scores);

        // 5. RRF merge
        let mut rrf_scores: HashMap<String, (f64, String, MemorySource, f64)> = HashMap::new();

        for (rank, ((id, raw_score, content), &_norm)) in
            facts_raw.iter().zip(fact_norm.iter()).enumerate()
        {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = rrf_scores.entry(id.clone()).or_insert((
                0.0,
                content.clone(),
                MemorySource::CognitiveFact,
                *raw_score,
            ));
            entry.0 += rrf;
        }

        for (rank, ((id, raw_score, content), &_norm)) in
            recalls_deduped.iter().zip(recall_norm.iter()).enumerate()
        {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = rrf_scores.entry(id.clone()).or_insert((
                0.0,
                content.clone(),
                MemorySource::ConversationRecall,
                *raw_score,
            ));
            entry.0 += rrf;
        }

        // 6. Sort and truncate
        let mut results: Vec<MemoryEntry> = rrf_scores
            .into_iter()
            .map(|(id, (score, content, source, raw_score))| MemoryEntry {
                id,
                content,
                score,
                source,
                raw_score,
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        results
    }
}

/// Min-max normalize a list of scores to 0.0–1.0.
///
/// Single-element or all-equal lists return 1.0 for every entry.
fn normalize_scores(scores: &[f64]) -> Vec<f64> {
    if scores.is_empty() {
        return Vec::new();
    }
    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;
    if range < f64::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores.iter().map(|s| (s - min) / range).collect()
}

/// Check if a recall's content overlaps with a fact based on the fact's predicate.
fn content_overlaps(
    _fact_content: &str,
    recall_content: &str,
    predicate: &str,
) -> bool {
    recall_content.to_lowercase().contains(&predicate.to_lowercase())
}
```

- [ ] **Step 4: Update re-export in `crates/cognitive/src/lib.rs`**

Change line 30 from:

```rust
pub use memory_retriever::CognitiveMemoryRetriever;
```

to:

```rust
pub use memory_retriever::UnifiedMemoryService;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(memory_retriever)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/memory_retriever.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): implement UnifiedMemoryService with RRF merge and dedup"
```

---

## Chunk 2: Slim CognitiveContextSource + Assembler Formatting + Wiring

### Task 3: Strip `CognitiveContextSource` to static-only

**Files:**
- Modify: `crates/cognitive/src/context_source.rs`
- Modify: `crates/cognitive/src/lib.rs:24`

- [ ] **Step 1: Remove dynamic-tier fields from `CognitiveContextSource`**

`CognitiveRetrievalConfig` stays unchanged — it's used by `UnifiedMemoryService` and the builder. Only the `CognitiveContextSource` struct itself loses its dynamic fields.

Replace the struct definition (lines 70-78) with:

```rust
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    cache: Mutex<Option<CachedModel>>,
    static_fact_limit: usize,
    confidence_bits: Option<Arc<AtomicU32>>,
}
```

- [ ] **Step 2: Update `new()` and builder methods**

Replace `new()` and remove `with_embedder`, `with_embedder_opt`, `with_config`, `with_situation`, `current_situational_boost`:

```rust
impl CognitiveContextSource {
    pub fn new(fact_repo: SemanticFactRepo, rule_repo: ProceduralRuleRepo) -> Self {
        Self {
            fact_repo,
            rule_repo,
            cache: Mutex::new(None),
            static_fact_limit: 10,
            confidence_bits: None,
        }
    }

    pub fn with_static_fact_limit(mut self, limit: usize) -> Self {
        self.static_fact_limit = limit;
        self
    }

    pub fn with_confidence_threshold(mut self, bits: Arc<AtomicU32>) -> Self {
        self.confidence_bits = Some(bits);
        self
    }
    // load_rules_text and get_cached_or_load remain unchanged
}
```

- [ ] **Step 3: Remove dynamic tier from `provide()` and clean up**

Remove the entire dynamic tier block (lines 240-302) from `provide()`. The method should end after the rules section, going straight to confidence calibration.

Remove these now-unused imports:
- Line 15: `use crate::embedder::SemanticFactEmbedder;`
- Line 17: `use crate::situation::UserSituation;`
- Lines 248-251 (inline `use` inside `provide()`): `use crate::retrieval::retrieve_relevant_facts;` and `use crate::repos::USER_MODEL_DOMAINS;`

Update `static_fact_limit` references: change `self.config.static_fact_limit` to `self.static_fact_limit`.

Update the module doc comment at lines 1-5 and the struct doc comment at lines 63-69 to remove references to "Two-tier injection" and "dynamic vector-searched facts". The source is now static-only.

- [ ] **Step 4: Fix tests in `context_source.rs`**

Remove tests that relied on dynamic tier behavior:
- Remove `test_dynamic_tier_with_message_fallback`
- Remove `test_situational_boost_from_active_situation`
- Remove `test_situational_boost_low_when_depleted`

Keep: `test_context_source_returns_none_when_empty`, `test_context_source_includes_facts`, `test_context_source_includes_rules`, `test_priority_is_60`, `test_static_tier_without_message`, `test_static_facts_sorted_by_importance`

Add a new test to verify static-only with a message present:

```rust
#[tokio::test]
async fn test_static_only_with_message_present() {
    let pool = setup().await;
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let rule_repo = ProceduralRuleRepo::new(pool);

    fact_repo
        .upsert(&test_fact("identity", "name", "Jayden"))
        .await
        .unwrap();

    let source = CognitiveContextSource::new(fact_repo, rule_repo);
    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "c1".into(),
        message: Some("what are my peak hours".into()),
        intent_summary: None,
        project_id: None,
    };

    let result = source.provide(&ctx).await.unwrap();
    assert!(result.contains("User Understanding"));
    assert!(result.contains("Jayden"));
    // Dynamic section should NOT appear (moved to UnifiedMemoryService)
    assert!(!result.contains("Relevant Personal Context"));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(context_source)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/context_source.rs
git commit -m "refactor(cognitive): strip CognitiveContextSource to static-only"
```

---

### Task 4: Update assembler formatting with grouped sections

**Files:**
- Modify: `crates/context_engine/src/assembler.rs`

- [ ] **Step 1: Write failing test for grouped formatting**

Add to the test module in `assembler.rs`:

```rust
#[tokio::test]
async fn test_memory_retriever_groups_by_source() {
    let retriever = Arc::new(MockRetriever {
        entries: vec![
            ("user: editor = neovim".into(), 0.90, MemorySource::CognitiveFact),
            ("I mentioned I use neovim yesterday".into(), 0.75, MemorySource::ConversationRecall),
        ],
    });

    let engine = ContextEngine::new().with_memory_retriever(retriever);
    let request = ContextRequest {
        message_text: "what editor do I use?".into(),
        history: vec![],
        system_prompt: "You are helpful.".into(),
        strategy: ExecutionStrategy::ToolAssisted { max_iterations: 5 },
        tool_definitions: vec![],
        context_window: 128_000,
    };

    let result = engine.assemble(request).await;
    if let Message::System { content } = &result.messages[1] {
        assert!(content.contains("## Relevant Facts"), "Should have facts section");
        assert!(content.contains("## Related Conversations"), "Should have recalls section");
        // Facts should appear before conversations
        let facts_pos = content.find("## Relevant Facts").unwrap();
        let recalls_pos = content.find("## Related Conversations").unwrap();
        assert!(facts_pos < recalls_pos);
    } else {
        panic!("Second message should be System (memory)");
    }
}
```

**Note:** This requires updating the `MockRetriever` to use the new `MemoryEntry` fields. Update `MockRetriever`:

```rust
struct MockRetriever {
    entries: Vec<(String, f64, MemorySource)>,
}

#[async_trait]
impl MemoryRetriever for MockRetriever {
    async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.entries
            .iter()
            .take(limit)
            .map(|(content, score, source)| MemoryEntry {
                id: "test".into(),
                content: content.clone(),
                score: *score,
                source: source.clone(),
                raw_score: *score,
            })
            .collect()
    }
}
```

Update all existing `MockRetriever` usages to include the `MemorySource` field. Use `MemorySource::CognitiveFact` as the default for existing tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p context_engine -E 'test(memory_retriever_groups)'`
Expected: FAIL — `retrieve_memory` doesn't partition by source yet

- [ ] **Step 3: Update `retrieve_memory()` method**

Replace the `retrieve_memory` method (lines 433-451 of `assembler.rs`) with:

```rust
async fn retrieve_memory(&self, request: &ContextRequest) -> Option<String> {
    let retriever = self.memory_retriever.as_ref()?;
    let entries = retriever
        .retrieve(&request.message_text, self.memory_retrieval_limit)
        .await;
    if entries.is_empty() {
        return None;
    }

    let (facts, recalls): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|e| e.source == MemorySource::CognitiveFact);

    let mut text = "[Relevant Context]\n".to_string();

    if !facts.is_empty() {
        text.push_str("\n## Relevant Facts\n");
        for entry in &facts {
            text.push_str(&format!(
                "- {} (relevance: {:.2})\n",
                entry.content, entry.score
            ));
        }
    }

    if !recalls.is_empty() {
        text.push_str("\n## Related Conversations\n");
        for entry in &recalls {
            text.push_str(&format!(
                "- {} (relevance: {:.2})\n",
                entry.content, entry.score
            ));
        }
    }

    Some(text)
}
```

Add the import at the top of the file:

```rust
use crate::memory_retriever::MemorySource;
```

- [ ] **Step 4: Run all assembler tests**

Run: `cargo nextest run -p context_engine -E 'test(assembler)'`
Expected: PASS (including the new grouped test and all existing tests)

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "feat(context_engine): group memory retrieval output by source type"
```

---

### Task 5: Rewire builder to use `UnifiedMemoryService`

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Update the `CognitiveContextSource` construction**

At lines 277-284, replace:

```rust
let mut cog_source =
    cognitive::CognitiveContextSource::new(fact_repo.clone(), rule_repo)
        .with_embedder_opt(cognitive_embedder.clone())
        .with_config(retrieval_config)
        .with_confidence_threshold(Arc::clone(&confidence_bits));
if let Some(ref sit) = self.user_situation {
    cog_source = cog_source.with_situation(Arc::clone(sit));
}
```

with:

```rust
let cog_source =
    cognitive::CognitiveContextSource::new(fact_repo.clone(), rule_repo)
        .with_static_fact_limit(config.cognitive.static_fact_limit)
        .with_confidence_threshold(Arc::clone(&confidence_bits));
```

- [ ] **Step 2: Update the memory retriever wiring**

At lines 544-550, remove the old `if let Some(ref recall)` guard and the comment on line 544 (`// ── Wire automatic memory retrieval (CognitiveMemoryRetriever) ───`). Replace the entire block:

```rust
// ── Wire automatic memory retrieval (CognitiveMemoryRetriever) ───
let context_engine = if let Some(ref recall) = recall_service {
    let retriever = Arc::new(cognitive::CognitiveMemoryRetriever::new(Arc::clone(recall)));
    context_engine.with_memory_retriever(retriever)
} else {
    context_engine
};
```

with:

```rust
let context_engine = {
    let mut retriever = cognitive::UnifiedMemoryService::new(fact_repo.clone())
        .with_recall_opt(recall_service.clone())
        .with_embedder_opt(cognitive_embedder.clone())
        .with_config(retrieval_config);
    if let Some(ref sit) = self.user_situation {
        retriever = retriever.with_situation(Arc::clone(sit));
    }
    context_engine.with_memory_retriever(Arc::new(retriever))
};
```

Note: `retrieval_config` was already built at line 261. It now goes to `UnifiedMemoryService` instead of `CognitiveContextSource`. Move the `retrieval_config` construction block if needed to ensure it's in scope.

Also ensure `cognitive_embedder` is in scope at the retriever wiring site (it's built at line 250).

- [ ] **Step 3: Verify compilation**

Run: `cargo build --workspace`
Expected: Success with zero errors

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire UnifiedMemoryService replacing CognitiveMemoryRetriever"
```

---

### Task 6: Final verification and cleanup

- [ ] **Step 1: Verify no references to `CognitiveMemoryRetriever` remain**

Use the Grep tool to search for `CognitiveMemoryRetriever` across `crates/`.
Expected: Zero matches. If any remain, update the references to `UnifiedMemoryService`.

- [ ] **Step 2: Run full test suite + clippy + fmt**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```
Expected: All pass

- [ ] **Step 3: Final commit if any cleanup needed**

```bash
git commit -m "refactor: remove remaining CognitiveMemoryRetriever references"
```
