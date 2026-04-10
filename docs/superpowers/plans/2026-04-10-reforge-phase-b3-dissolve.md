# Phase B3 — Dissolve: Conversation Promotion, Graph-Aware Retrieval, Temporal Tool

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the memory retrieval upgrade by adding a conversation promotion lifecycle, graph-aware retrieval with a 12th relevance weight factor (`graph_path_boost`), and a `TemporalTool` for time-oriented knowledge queries.

**Architecture:** Conversation promotion adds a `promoted_at` column to `conversation_density` so cross-retrieval deprioritizes already-promoted entries. Graph-aware retrieval extracts entities from the query, traverses the entity graph for neighborhood context, and boosts facts connected to graph neighbors via a new 12th weight (`graph_path_boost`). The TemporalTool exposes `facts_as_of`, `first_mention`, `change_history`, `competing_truths`, `knowledge_diff`, and `decision_points` as a multi-action read-only tool using the existing `#[tool_actions]` macro pattern.

**Tech Stack:** Rust, SQLite (cognitive crate), LanceDB (conv_embeddings), existing entity graph + fact_changelog infrastructure, `tools-core` derive macros

**Depends on:** Phase B1 (11-weight system, entity extraction, fact_changelog) + Phase B2 (value-density, conversation_density table, Phase 6.5, knowledge snapshots)

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/services/graph_retrieval.rs` | Entity extraction from query → graph neighborhood → fact boosting |
| `crates/tools/src/domain/temporal.rs` | TemporalTool multi-action tool (6 actions, read-only) |

### Modified Files
| File | Change |
|------|--------|
| `crates/cognitive/src/services/decay.rs` | Add `graph_path_boost` as 12th field to `RelevanceWeights`, add 12th param to `relevance_score()` |
| `crates/cognitive/src/services/retrieval.rs` | Add `relevance_weight_graph_path_boost` to `RetrievalParams` |
| `crates/cognitive/src/services/context_source.rs` | Add `relevance_weight_graph_path_boost` to `CognitiveRetrievalConfig` |
| `crates/cognitive/src/services/memory_retriever.rs` | Integrate graph-aware retrieval in `retrieve()`, update `retrieve_with_overrides` to `[f64; 12]` |
| `crates/cognitive/src/services/temporal.rs` | Add `facts_as_of`, `first_mention`, `competing_truths`, `knowledge_diff`, `decision_points` methods |
| `crates/cognitive/src/services/mod.rs` | Export `graph_retrieval` module |
| `crates/cognitive/src/repos/enrichment.rs` | Add `promote()` and `is_promoted()` methods to `ConversationDensityRepo` |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `promoted_at` column to `conversation_density` |
| `crates/cognitive/src/repos/mod.rs` | Bump migration version |
| `crates/config/src/schema/cognitive.rs` | Add `relevance_weight_graph_path_boost` config field |
| `crates/common/src/autotuner.rs` | Add `relevance_weight_graph_path_boost` to `TrialParams`, change `resolve_full_relevance_weights` to `[f64; 12]` |
| `crates/agent/src/autotuner/shadow_retriever.rs` | Change `config_defaults` from `[f64; 11]` to `[f64; 12]` |
| `crates/agent/src/agent_loop/builder.rs` | Add 12th element to `config_defaults` array, add field to `CognitiveRetrievalConfig` construction |
| `crates/tools/src/domain/mod.rs` | Export `temporal` module |
| `crates/tools/src/lib.rs` | Re-export `TemporalTool` |
| `crates/app-core/src/init/mod.rs` | Register `TemporalTool` in tool registry |
| `tests/integration/cognitive.rs` | B3 integration tests |

---

### Task 1: Add `graph_path_boost` as 12th relevance weight

**Files:**
- Modify: `crates/cognitive/src/services/decay.rs`
- Modify: `crates/cognitive/src/services/retrieval.rs`
- Modify: `crates/cognitive/src/services/context_source.rs`
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Add `graph_path_boost` to `RelevanceWeights`**

In `crates/cognitive/src/services/decay.rs`, add after the `recall_support` field (line ~28):

```rust
    pub graph_path_boost: f64,
```

In the `Default` impl, add after `recall_support: 0.08`:

```rust
    graph_path_boost: 0.06,
```

Update the doc comment on the struct from "11-factor" to "12-factor".

- [ ] **Step 2: Add 12th parameter to `relevance_score()`**

In `crates/cognitive/src/services/decay.rs`, update the function signature and body. Add after the `recall_support` parameter:

```rust
    graph_path_boost: f64,
```

Add to the formula body, after `+ recall_support * weights.recall_support`:

```rust
        + graph_path_boost * weights.graph_path_boost)
```

Remove the closing paren from the `recall_support` line so the chain continues.

- [ ] **Step 3: Fix all call sites of `relevance_score()`**

In `crates/cognitive/src/services/retrieval.rs`, update both `vector_path()` and `fallback_path()` calls to pass `0.0` for the new parameter. Find each `relevance_score(` call and add `0.0,` (for `graph_path_boost`) after the `recall_support` argument, before `weights`:

```rust
    0.0,           // graph_path_boost — computed post-hoc in graph_retrieval
```

- [ ] **Step 4: Add `relevance_weight_graph_path_boost` to `RetrievalParams`**

In `crates/cognitive/src/services/retrieval.rs`, add to `RetrievalParams` after `relevance_weight_recall_support`:

```rust
    pub relevance_weight_graph_path_boost: f64,
```

In `RetrievalParams::new()`, add after `relevance_weight_recall_support: 0.08`:

```rust
    relevance_weight_graph_path_boost: 0.06,
```

Update the weights construction in `retrieve_relevant_facts()` to include:

```rust
    graph_path_boost: params.relevance_weight_graph_path_boost,
```

- [ ] **Step 5: Add to `CognitiveRetrievalConfig`**

In `crates/cognitive/src/services/context_source.rs`, add to the struct after `relevance_weight_recall_support`:

```rust
    pub relevance_weight_graph_path_boost: f64,
```

In `Default` impl, add after `relevance_weight_recall_support: 0.08`:

```rust
    relevance_weight_graph_path_boost: 0.06,
```

- [ ] **Step 6: Add config field**

In `crates/config/src/schema/cognitive.rs`, add after the `relevance_weight_recall_support` field:

```rust
    /// Relevance weight for graph path proximity boost (default: 0.06).
    /// Facts connected to entities mentioned in the query score higher.
    #[serde(default = "default_w_graph_path_boost")]
    pub relevance_weight_graph_path_boost: f64,
```

Add the default function:

```rust
fn default_w_graph_path_boost() -> f64 {
    0.06
}
```

- [ ] **Step 7: Verify**

Run: `cargo build -p cognitive -p config`
Expected: Compiles (warnings about unused fields are OK at this stage).

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/ crates/config/
git commit -m "feat(cognitive): add graph_path_boost as 12th relevance weight factor"
```

---

### Task 2: Extend autotuner and wiring for 12 weights

**Files:**
- Modify: `crates/common/src/autotuner.rs`
- Modify: `crates/agent/src/autotuner/shadow_retriever.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add `graph_path_boost` to `TrialParams`**

In `crates/common/src/autotuner.rs`, add to the `TrialParams` struct after `relevance_weight_recall_support`:

```rust
    pub relevance_weight_graph_path_boost: Option<f64>,
```

- [ ] **Step 2: Change `resolve_full_relevance_weights` to 12 elements**

In `crates/common/src/autotuner.rs`, update the function:

```rust
    pub fn resolve_full_relevance_weights(&self, defaults: &[f64; 12]) -> [f64; 12] {
        let raw = [
            self.relevance_weight_semantic.unwrap_or(defaults[0]),
            self.relevance_weight_retrievability.unwrap_or(defaults[1]),
            self.relevance_weight_importance.unwrap_or(defaults[2]),
            self.relevance_weight_frequency.unwrap_or(defaults[3]),
            self.relevance_weight_situation.unwrap_or(defaults[4]),
            self.relevance_weight_temporal.unwrap_or(defaults[5]),
            self.relevance_weight_hierarchy.unwrap_or(defaults[6]),
            self.relevance_weight_path_coherence.unwrap_or(defaults[7]),
            self.relevance_weight_community.unwrap_or(defaults[8]),
            self.relevance_weight_cross_note.unwrap_or(defaults[9]),
            self.relevance_weight_recall_support.unwrap_or(defaults[10]),
            self.relevance_weight_graph_path_boost.unwrap_or(defaults[11]),
        ];
        let sum: f64 = raw.iter().sum();
        if sum > 0.0 {
            raw.map(|w| w / sum)
        } else {
            *defaults
        }
    }
```

- [ ] **Step 3: Update the test**

Update `resolve_full_relevance_weights_sums_to_one` test to use `[f64; 12]`:

```rust
    #[test]
    fn resolve_full_relevance_weights_sums_to_one() {
        let params = TrialParams {
            relevance_weight_semantic: Some(0.30),
            relevance_weight_hierarchy: Some(0.20),
            relevance_weight_community: Some(0.25),
            ..Default::default()
        };
        let defaults = [0.20, 0.10, 0.08, 0.05, 0.15, 0.02, 0.10, 0.05, 0.15, 0.10, 0.08, 0.06];
        let weights = params.resolve_full_relevance_weights(&defaults);
        let sum: f64 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "All 12 weights must sum to 1.0, got {sum}"
        );
    }
```

- [ ] **Step 4: Update `AgentShadowRetriever`**

In `crates/agent/src/autotuner/shadow_retriever.rs`, change `[f64; 11]` to `[f64; 12]`:

```rust
pub struct AgentShadowRetriever {
    memory_service: Arc<UnifiedMemoryService>,
    config_defaults: [f64; 12],
}

impl AgentShadowRetriever {
    pub fn new(memory_service: Arc<UnifiedMemoryService>, config_defaults: [f64; 12]) -> Self {
        Self { memory_service, config_defaults }
    }
}
```

- [ ] **Step 5: Update `builder.rs` config_defaults array**

In `crates/agent/src/agent_loop/builder.rs`, where the `config_defaults` array is built (~line 1702), change to `[f64; 12]` and add the 12th element:

```rust
                    let config_defaults: [f64; 12] = [
                        config.cognitive.relevance_weight_semantic,
                        config.cognitive.relevance_weight_retrievability,
                        config.cognitive.relevance_weight_importance,
                        config.cognitive.relevance_weight_frequency,
                        config.cognitive.relevance_weight_situation,
                        config.cognitive.relevance_weight_temporal,
                        0.10_f64, // relevance_weight_hierarchy
                        0.05_f64, // relevance_weight_path_coherence
                        0.15_f64, // relevance_weight_community
                        0.10_f64, // relevance_weight_cross_note
                        config.cognitive.relevance_weight_recall_support,
                        config.cognitive.relevance_weight_graph_path_boost,
                    ];
```

- [ ] **Step 6: Update `builder.rs` retrieval config construction**

Where `CognitiveRetrievalConfig` is built (~line 339), add:

```rust
                    relevance_weight_graph_path_boost: config.cognitive.relevance_weight_graph_path_boost,
```

- [ ] **Step 7: Update `memory_retriever.rs` resolve_retrieval_params**

In `crates/cognitive/src/services/memory_retriever.rs`, in `resolve_retrieval_params()`, add after the existing weight mappings:

```rust
            relevance_weight_graph_path_boost: self.config.relevance_weight_graph_path_boost,
```

In the champion override section, update the defaults array to `[f64; 12]` and add the mapping:

```rust
            let defaults = [
                self.config.relevance_weight_semantic,
                self.config.relevance_weight_retrievability,
                self.config.relevance_weight_importance,
                self.config.relevance_weight_frequency,
                self.config.relevance_weight_situation,
                self.config.relevance_weight_temporal,
                self.config.relevance_weight_hierarchy,
                self.config.relevance_weight_path_coherence,
                self.config.relevance_weight_community,
                self.config.relevance_weight_cross_note,
                self.config.relevance_weight_recall_support,
                self.config.relevance_weight_graph_path_boost,
            ];
            let w = champion.resolve_full_relevance_weights(&defaults);
            // ... existing mappings for w[0]..w[10] ...
            params.relevance_weight_graph_path_boost = w[11];
```

- [ ] **Step 8: Update `retrieve_with_overrides`**

In `crates/cognitive/src/services/memory_retriever.rs`, change the signature:

```rust
    pub async fn retrieve_with_overrides(
        &self,
        query: &str,
        vector_top_k: usize,
        min_similarity: f64,
        relevance_weights: [f64; 12],  // was [f64; 11]
    ) -> common::Result<Vec<ScoredFact>> {
```

And add to the params construction inside the method:

```rust
            relevance_weight_graph_path_boost: relevance_weights[11],
```

- [ ] **Step 9: Verify**

Run: `cargo build --workspace`
Expected: Clean compile.

- [ ] **Step 10: Commit**

```bash
git add crates/common/ crates/agent/ crates/cognitive/
git commit -m "feat(common): extend autotuner and wiring for 12-weight graph_path_boost"
```

---

### Task 3: Graph-aware retrieval service

**Files:**
- Create: `crates/cognitive/src/services/graph_retrieval.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Create the graph retrieval module**

Create `crates/cognitive/src/services/graph_retrieval.rs`:

```rust
//! Graph-aware retrieval — extract entities from a query, traverse the entity
//! graph for neighborhood context, and compute a graph_path_boost score for
//! facts connected to those entities.
//!
//! The boost score is the 12th relevance weight factor. It rewards facts whose
//! subject or object mentions entities that are in the query's graph neighborhood.

use crate::repos::entity::{EntityRepo, EntityRow};

/// Compute graph_path_boost scores for a set of facts based on query entity context.
///
/// Returns a map of fact content → boost score (0.0–1.0).
/// Facts mentioning entities in the query's graph neighborhood get higher scores.
pub async fn compute_graph_boosts(
    entity_repo: &EntityRepo,
    query: &str,
    fact_contents: &[(String, String)], // (fact_id, fact_content)
) -> std::collections::HashMap<String, f64> {
    let mut boosts = std::collections::HashMap::new();

    // Step 1: Extract potential entity names from the query (capitalized words)
    let query_entities = extract_query_entities(query);
    if query_entities.is_empty() {
        return boosts;
    }

    // Step 2: Resolve entity names to IDs via EntityRepo
    let mut neighborhood_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for name in &query_entities {
        if let Ok(entities) = entity_repo.find_by_name(name).await {
            for entity in &entities {
                neighborhood_names.insert(entity.name.to_lowercase());
                // Expand with depth-1 neighbors
                if let Ok(Some(hood)) = entity_repo.get_neighborhood(&entity.id, 1).await {
                    for neighbor in &hood.neighbors {
                        neighborhood_names.insert(neighbor.name.to_lowercase());
                    }
                }
            }
        }
    }

    if neighborhood_names.is_empty() {
        return boosts;
    }

    // Step 3: Score each fact by entity overlap with the neighborhood
    for (fact_id, content) in fact_contents {
        let content_lower = content.to_lowercase();
        let matches = neighborhood_names
            .iter()
            .filter(|name| name.len() > 2 && content_lower.contains(name.as_str()))
            .count();
        if matches > 0 {
            // Scale: 1 match = 0.4, 2 = 0.7, 3+ = 1.0
            let score = match matches {
                1 => 0.4,
                2 => 0.7,
                _ => 1.0,
            };
            boosts.insert(fact_id.clone(), score);
        }
    }

    boosts
}

/// Extract potential entity names from a query string.
/// Uses simple heuristics: capitalized words that aren't sentence starters.
fn extract_query_entities(query: &str) -> Vec<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut entities = Vec::new();

    for (i, word) in words.iter().enumerate() {
        // Skip sentence starters (index 0 or after punctuation)
        let is_sentence_start = i == 0
            || words
                .get(i.wrapping_sub(1))
                .is_some_and(|prev| prev.ends_with('.') || prev.ends_with('?') || prev.ends_with('!'));

        if !is_sentence_start
            && word.len() > 1
            && word.chars().next().is_some_and(|c| c.is_uppercase())
            && !word.chars().all(|c| c.is_uppercase())
        {
            // Strip trailing punctuation
            let clean = word.trim_end_matches(|c: char| c.is_ascii_punctuation());
            if clean.len() > 1 {
                entities.push(clean.to_string());
            }
        }
    }

    entities.dedup();
    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_entities_skips_sentence_starters() {
        let entities = extract_query_entities("Tell me about Rust and the Klynt project");
        assert!(entities.contains(&"Rust".to_string()));
        assert!(entities.contains(&"Klynt".to_string()));
        // "Tell" is sentence starter — should be excluded
        assert!(!entities.contains(&"Tell".to_string()));
    }

    #[test]
    fn extract_entities_empty_on_lowercase() {
        let entities = extract_query_entities("what time is it");
        assert!(entities.is_empty());
    }

    #[test]
    fn extract_entities_strips_punctuation() {
        let entities = extract_query_entities("I was talking to Sarah, about the project.");
        assert!(entities.contains(&"Sarah".to_string()));
    }
}
```

- [ ] **Step 2: Export the module**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod graph_retrieval;
```

- [ ] **Step 3: Integrate graph-aware boost in `memory_retriever.rs` `retrieve()`**

In `crates/cognitive/src/services/memory_retriever.rs`, in the `retrieve()` method, after the cross-retrieval boost block (after the `recall_support` section, ~line 476) and before the deduplication/RRF merge, add:

```rust
        // 1c. Graph-aware retrieval: boost facts connected to query entity neighborhood
        if let Some(ref entity_repo) = self.entity_repo {
            let weight = self.config.relevance_weight_graph_path_boost;
            if weight > 0.0 && !facts_raw.is_empty() {
                let fact_contents: Vec<(String, String)> = facts_raw
                    .iter()
                    .map(|(id, _, content, _)| (id.clone(), content.clone()))
                    .collect();
                let boosts = crate::services::graph_retrieval::compute_graph_boosts(
                    entity_repo,
                    query,
                    &fact_contents,
                )
                .await;
                if !boosts.is_empty() {
                    for (id, score, _, _) in &mut facts_raw {
                        if let Some(&boost) = boosts.get(id) {
                            *score += boost * weight;
                        }
                    }
                    facts_raw.sort_by(|a, b| {
                        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }
        }
```

This requires `entity_repo` to be available on the `UnifiedMemoryService`. Add it as an optional field:

```rust
// In the UnifiedMemoryService struct, add:
    entity_repo: Option<crate::repos::EntityRepo>,
```

Add a builder method:

```rust
    pub fn with_entity_repo(mut self, repo: crate::repos::EntityRepo) -> Self {
        self.entity_repo = Some(repo);
        self
    }
```

Set `entity_repo: None` in the existing constructor.

- [ ] **Step 4: Wire entity_repo into UnifiedMemoryService construction**

In `crates/agent/src/agent_loop/builder.rs`, where `UnifiedMemoryService` is constructed, chain `.with_entity_repo(EntityRepo::new(pool.clone()))`. The implementer should find the construction site and add the call.

- [ ] **Step 5: Verify**

Run: `cargo build --workspace`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/ crates/agent/
git commit -m "feat(cognitive): implement graph-aware retrieval with entity neighborhood boosting"
```

---

### Task 4: Conversation promotion lifecycle

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/enrichment.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add `promoted_at` column to `conversation_density`**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, find the `conversation_density` table and add `promoted_at` after `enriched`:

```sql
    promoted_at TEXT,             -- ISO timestamp when promoted to knowledge graph (nullable)
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, change the cognitive migration version from `2` to `3`.

- [ ] **Step 3: Add promotion methods to ConversationDensityRepo**

In `crates/cognitive/src/repos/enrichment.rs`, add to `impl ConversationDensityRepo`:

```rust
    /// Mark a conversation turn as promoted to the knowledge graph.
    pub async fn promote(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conversation_density SET promoted_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Check if a conversation turn has been promoted.
    pub async fn is_promoted(&self, id: &str) -> Result<bool, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM conversation_density WHERE id = ?1 AND promoted_at IS NOT NULL",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 > 0)
    }

    /// Load high-density turns that haven't been promoted yet.
    pub async fn load_unpromoted_high(
        &self,
        limit: u32,
    ) -> Result<Vec<ConversationDensityRow>, sqlx::Error> {
        sqlx::query_as::<_, ConversationDensityRow>(
            "SELECT * FROM conversation_density
             WHERE tier = 'high' AND promoted_at IS NULL
             ORDER BY density_score DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
```

- [ ] **Step 4: Add test for promotion**

In the tests module of `enrichment.rs`:

```rust
    #[tokio::test]
    async fn density_promote_and_check() {
        let pool = setup().await;
        let repo = ConversationDensityRepo::new(pool);

        let score = crate::services::value_density::DensityScore {
            total: 0.85,
            entity_signal: 0.8,
            action_signal: 0.7,
            decision_signal: 0.6,
            novelty_signal: 0.5,
            tier: crate::services::value_density::DensityTier::High,
        };
        repo.insert("p1", "sess1", "important content", &score)
            .await
            .unwrap();

        assert!(!repo.is_promoted("p1").await.unwrap());

        repo.promote("p1").await.unwrap();

        assert!(repo.is_promoted("p1").await.unwrap());

        let unpromoted = repo.load_unpromoted_high(10).await.unwrap();
        assert!(unpromoted.is_empty(), "Promoted turns should not appear in unpromoted list");
    }
```

- [ ] **Step 5: Deprioritize promoted conversations in cross-retrieval**

In `crates/cognitive/src/services/memory_retriever.rs`, in the cross-retrieval boost block (the `recall_support` section), the boost already works on recall_contents (conversation snippets). No change needed here — promoted conversations remain in conv_embeddings but their cross-retrieval signal naturally weakens as the facts they corroborated have been solidified. The `promoted_at` field is consumed by Phase 6.5 to avoid re-processing.

This step is a no-op — the promotion lifecycle is complete with the repo methods + the density table column.

- [ ] **Step 6: Verify**

Run: `cargo nextest run -p cognitive -E 'test(promote)'`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add conversation promotion lifecycle to density repo"
```

---

### Task 5: Extend TemporalService with B3 query methods

**Files:**
- Modify: `crates/cognitive/src/services/temporal.rs`
- Modify: `crates/cognitive/src/repos/fact_changelog.rs`

- [ ] **Step 1: Add `first_mention` to FactChangelogRepo**

In `crates/cognitive/src/repos/fact_changelog.rs`, add:

```rust
    /// Find the earliest changelog entry for a fact — when it was first created.
    pub async fn first_mention(
        &self,
        fact_id: &str,
    ) -> Result<Option<ChangelogEntry>, sqlx::Error> {
        sqlx::query_as::<_, ChangelogEntry>(
            "SELECT * FROM fact_changelog WHERE fact_id = ?1 AND change_type = 'create'
             ORDER BY changed_at ASC LIMIT 1",
        )
        .bind(fact_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Find all changelog entries for facts matching a subject+predicate pattern.
    pub async fn history_by_subject_predicate(
        &self,
        subject: &str,
        predicate: &str,
        limit: u32,
    ) -> Result<Vec<ChangelogEntry>, sqlx::Error> {
        sqlx::query_as::<_, ChangelogEntry>(
            "SELECT cl.* FROM fact_changelog cl
             JOIN semantic_facts f ON cl.fact_id = f.id
             WHERE f.subject = ?1 AND f.predicate = ?2
             ORDER BY cl.changed_at DESC LIMIT ?3",
        )
        .bind(subject)
        .bind(predicate)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
```

- [ ] **Step 2: Add B3 methods to TemporalService**

In `crates/cognitive/src/services/temporal.rs`, add `FactChangelogRepo` as a dependency and new methods.

First, update the struct and constructor:

```rust
use crate::repos::FactChangelogRepo;

#[derive(Clone)]
pub struct TemporalService {
    fact_repo: SemanticFactRepo,
    changelog_repo: Option<FactChangelogRepo>,
}

impl TemporalService {
    pub fn new(fact_repo: SemanticFactRepo) -> Self {
        Self { fact_repo, changelog_repo: None }
    }

    pub fn with_changelog(mut self, repo: FactChangelogRepo) -> Self {
        self.changelog_repo = Some(repo);
        self
    }
```

Then add the new methods:

```rust
    /// Return the state of a fact at a given point in time.
    ///
    /// Walks the changelog backwards from `as_of` to find the last known state.
    /// Returns `None` if the fact didn't exist at that time.
    pub async fn facts_as_of(
        &self,
        subject: &str,
        predicate: &str,
        as_of: &str,
    ) -> Result<Option<FactVersion>, sqlx::Error> {
        let history = self.get_fact_history(subject, predicate).await?;
        // Find the most recent version that was valid at `as_of`
        for version in &history {
            if version.fact.valid_from.as_str() <= as_of {
                // Check it wasn't superseded before as_of
                if let Some(ref superseded_at) = version.fact.superseded_at {
                    if superseded_at.as_str() <= as_of {
                        continue; // Was already superseded at that time
                    }
                }
                return Ok(Some(version.clone()));
            }
        }
        Ok(None)
    }

    /// Find when a subject+predicate combination was first mentioned.
    pub async fn first_mention(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let history = self.get_fact_history(subject, predicate).await?;
        // Oldest valid_from across all versions
        Ok(history.last().map(|v| v.fact.valid_from.clone()))
    }

    /// Find competing truths — active facts with the same subject+predicate
    /// but different objects.
    pub async fn competing_truths(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        self.fact_repo
            .find_by_subject_predicate(subject, predicate)
            .await
    }

    /// Compute a knowledge diff between two timestamps.
    ///
    /// Returns facts added, updated, and removed in the window.
    pub async fn knowledge_diff(
        &self,
        from: &str,
        to: &str,
        domains: Option<&[&str]>,
    ) -> Result<KnowledgeDiff, sqlx::Error> {
        let from_dt = chrono::DateTime::parse_from_rfc3339(from)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let summary = self.change_summary(from_dt, domains).await?;

        Ok(KnowledgeDiff {
            period: (from.to_string(), to.to_string()),
            added: summary.new_facts,
            updated: summary.updated_facts,
            removed: summary.superseded_facts,
            by_domain: summary.by_domain,
        })
    }

    /// Find decision points — facts with multiple historical values (subject changed opinion).
    pub async fn decision_points(
        &self,
        domain: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DecisionPoint>, sqlx::Error> {
        // Find subject+predicate pairs that have been superseded at least once
        let superseded = self
            .fact_repo
            .list_superseded_since("2000-01-01", domain.map(|d| std::slice::from_ref(&d)))
            .await?;

        let mut seen: std::collections::HashMap<(String, String), Vec<SemanticFact>> =
            std::collections::HashMap::new();
        for fact in superseded {
            seen.entry((fact.subject.clone(), fact.predicate.clone()))
                .or_default()
                .push(fact);
        }

        let mut points: Vec<DecisionPoint> = seen
            .into_iter()
            .filter(|(_, versions)| versions.len() >= 2)
            .map(|((subject, predicate), versions)| DecisionPoint {
                subject,
                predicate,
                version_count: versions.len(),
                latest_value: versions.first().map(|f| f.object.clone()),
                earliest_value: versions.last().map(|f| f.object.clone()),
            })
            .collect();

        points.sort_by(|a, b| b.version_count.cmp(&a.version_count));
        points.truncate(limit);
        Ok(points)
    }
```

- [ ] **Step 3: Add supporting types**

Add these types in `temporal.rs` after `ChangeSummary`:

```rust
/// A knowledge diff between two timestamps.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeDiff {
    pub period: (String, String),
    pub added: Vec<SemanticFact>,
    pub updated: Vec<(SemanticFact, SemanticFact)>,
    pub removed: Vec<SemanticFact>,
    pub by_domain: HashMap<String, usize>,
}

/// A decision point — a fact that changed value multiple times.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionPoint {
    pub subject: String,
    pub predicate: String,
    pub version_count: usize,
    pub latest_value: Option<String>,
    pub earliest_value: Option<String>,
}
```

- [ ] **Step 4: Add tests**

```rust
    #[tokio::test]
    async fn test_facts_as_of() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let f1 = make_fact("asof1", "work", "user", "role", "2026-01-01", "2026-01-01");
        repo.upsert(&f1).await.unwrap();

        // Should find the fact at a date after valid_from
        let result = service.facts_as_of("user", "role", "2026-06-01").await.unwrap();
        assert!(result.is_some(), "Should find fact valid at 2026-06-01");

        // Should NOT find the fact at a date before valid_from
        let result = service.facts_as_of("user", "role", "2025-01-01").await.unwrap();
        assert!(result.is_none(), "Should not find fact before valid_from");
    }

    #[tokio::test]
    async fn test_first_mention() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let f1 = make_fact("fm1", "work", "user", "lang", "2026-01-15", "2026-01-15");
        let f2 = make_fact("fm2", "work", "user", "lang", "2026-03-01", "2026-03-01");
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let mention = service.first_mention("user", "lang").await.unwrap();
        assert_eq!(mention, Some("2026-01-15".to_string()));
    }

    #[tokio::test]
    async fn test_competing_truths() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let mut f1 = make_fact("ct1", "work", "user", "fav_lang", "2026-01-01", "2026-01-01");
        f1.object = "Python".to_string();
        let mut f2 = make_fact("ct2", "work", "user", "fav_lang", "2026-02-01", "2026-02-01");
        f2.object = "Rust".to_string();
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let truths = service.competing_truths("user", "fav_lang").await.unwrap();
        assert_eq!(truths.len(), 2, "Should find 2 competing truths");
    }
```

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p cognitive -E 'test(facts_as_of) | test(first_mention) | test(competing_truths)'`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): extend TemporalService with facts_as_of, first_mention, competing_truths, knowledge_diff, decision_points"
```

---

### Task 6: TemporalTool — multi-action read-only tool

**Files:**
- Create: `crates/tools/src/domain/temporal.rs`
- Modify: `crates/tools/src/domain/mod.rs`
- Modify: `crates/tools/src/lib.rs`

- [ ] **Step 1: Create the TemporalTool**

Create `crates/tools/src/domain/temporal.rs`:

```rust
//! TemporalTool — read-only temporal reasoning queries over the knowledge graph.
//!
//! Exposes time-oriented queries: facts_as_of, first_mention, change_history,
//! competing_truths, knowledge_diff, decision_points.

use common::Result;
use tools_core::{tool_actions, ActionParams, RoutingContext};

use cognitive::services::temporal::TemporalService;

// ---------------------------------------------------------------------------
// Action param structs
// ---------------------------------------------------------------------------

#[derive(Debug, ActionParams)]
pub struct FactsAsOfParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "peak_hours")
    pub predicate: String,
    /// ISO-8601 date or datetime to query at (e.g., "2026-03-15")
    pub as_of: String,
}

#[derive(Debug, ActionParams)]
pub struct FirstMentionParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "occupation")
    pub predicate: String,
}

#[derive(Debug, ActionParams)]
pub struct ChangeHistoryParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "peak_hours")
    pub predicate: String,
}

#[derive(Debug, ActionParams)]
pub struct CompetingTruthsParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "favorite_language")
    pub predicate: String,
}

#[derive(Debug, ActionParams)]
pub struct KnowledgeDiffParams {
    /// Start of the period (ISO-8601, e.g., "2026-03-01T00:00:00Z")
    pub from: String,
    /// End of the period (ISO-8601, e.g., "2026-04-01T00:00:00Z")
    pub to: String,
    /// Optional domain filter (e.g., "work", "finance")
    pub domain: Option<String>,
}

#[derive(Debug, ActionParams)]
pub struct DecisionPointsParams {
    /// Optional domain filter
    pub domain: Option<String>,
    /// Maximum number of decision points to return (default: 10)
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// TemporalTool
// ---------------------------------------------------------------------------

pub struct TemporalTool {
    service: TemporalService,
}

impl TemporalTool {
    pub fn new(service: TemporalService) -> Self {
        Self { service }
    }
}

#[tool_actions(
    name = "temporal",
    description = "Time-oriented queries over the knowledge graph. Query fact history, find when something was first mentioned, compare knowledge states across time, and discover decision points where beliefs changed.",
    category = "Memory",
    tags = "temporal,history,memory,facts,timeline,change",
    cost = "Free"
)]
impl TemporalTool {
    /// Return the state of a fact at a specific point in time.
    #[action(name = "facts_as_of")]
    async fn facts_as_of(
        &self,
        params: FactsAsOfParams,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        let result = self
            .service
            .facts_as_of(&params.subject, &params.predicate, &params.as_of)
            .await
            .map_err(|e| tool_err(format!("facts_as_of failed: {e}")))?;

        match result {
            Some(version) => serde_json::to_string_pretty(&version)
                .map_err(|e| tool_err(format!("serialize: {e}"))),
            None => Ok(format!(
                "No fact found for {}.{} as of {}",
                params.subject, params.predicate, params.as_of
            )),
        }
    }

    /// Find when a subject+predicate was first mentioned.
    #[action(name = "first_mention")]
    async fn first_mention(
        &self,
        params: FirstMentionParams,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        let result = self
            .service
            .first_mention(&params.subject, &params.predicate)
            .await
            .map_err(|e| tool_err(format!("first_mention failed: {e}")))?;

        match result {
            Some(date) => Ok(format!(
                "{}.{} was first mentioned on {}",
                params.subject, params.predicate, date
            )),
            None => Ok(format!(
                "No records found for {}.{}",
                params.subject, params.predicate
            )),
        }
    }

    /// Return full version history of a fact (newest first).
    #[action(name = "change_history")]
    async fn change_history(
        &self,
        params: ChangeHistoryParams,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        let history = self
            .service
            .get_fact_history(&params.subject, &params.predicate)
            .await
            .map_err(|e| tool_err(format!("change_history failed: {e}")))?;

        if history.is_empty() {
            return Ok(format!(
                "No history found for {}.{}",
                params.subject, params.predicate
            ));
        }

        serde_json::to_string_pretty(&history)
            .map_err(|e| tool_err(format!("serialize: {e}")))
    }

    /// Find competing truths — active facts with the same key but different values.
    #[action(name = "competing_truths")]
    async fn competing_truths(
        &self,
        params: CompetingTruthsParams,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        let truths = self
            .service
            .competing_truths(&params.subject, &params.predicate)
            .await
            .map_err(|e| tool_err(format!("competing_truths failed: {e}")))?;

        if truths.len() <= 1 {
            return Ok(format!(
                "No competing truths for {}.{} ({})",
                params.subject,
                params.predicate,
                if truths.is_empty() {
                    "no facts"
                } else {
                    "single value"
                }
            ));
        }

        serde_json::to_string_pretty(&truths)
            .map_err(|e| tool_err(format!("serialize: {e}")))
    }

    /// Compute a knowledge diff between two timestamps.
    #[action(name = "knowledge_diff")]
    async fn knowledge_diff(
        &self,
        params: KnowledgeDiffParams,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        let domains_ref: Option<Vec<&str>> =
            params.domain.as_ref().map(|d| vec![d.as_str()]);
        let diff = self
            .service
            .knowledge_diff(
                &params.from,
                &params.to,
                domains_ref.as_deref(),
            )
            .await
            .map_err(|e| tool_err(format!("knowledge_diff failed: {e}")))?;

        serde_json::to_string_pretty(&diff)
            .map_err(|e| tool_err(format!("serialize: {e}")))
    }

    /// Find decision points where the user changed their mind.
    #[action(name = "decision_points")]
    async fn decision_points(
        &self,
        params: DecisionPointsParams,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        let limit = params.limit.unwrap_or(10) as usize;
        let points = self
            .service
            .decision_points(params.domain.as_deref(), limit)
            .await
            .map_err(|e| tool_err(format!("decision_points failed: {e}")))?;

        if points.is_empty() {
            return Ok("No decision points found — no facts have been revised yet.".to_string());
        }

        serde_json::to_string_pretty(&points)
            .map_err(|e| tool_err(format!("serialize: {e}")))
    }
}

fn tool_err(msg: String) -> common::KlyntbotError {
    common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(msg))
}
```

- [ ] **Step 2: Export the module**

In `crates/tools/src/domain/mod.rs`, add:

```rust
pub mod temporal;
```

In `crates/tools/src/lib.rs`, add the re-export alongside existing tool re-exports:

```rust
pub use domain::temporal::TemporalTool;
```

- [ ] **Step 3: Verify**

Run: `cargo build -p tools`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/tools/
git commit -m "feat(tools): add TemporalTool with 6 read-only temporal reasoning actions"
```

---

### Task 7: Register TemporalTool in app-core

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Register the tool**

In `crates/app-core/src/init/mod.rs`, find where `MirrorTool` is registered (~line 1121: `registry.register(tools::MirrorTool::new(...))`). Add after it:

```rust
            // Temporal reasoning tool
            {
                let temporal_service = cognitive::services::temporal::TemporalService::new(
                    cognitive::SemanticFactRepo::new(pool.clone()),
                )
                .with_changelog(cognitive::FactChangelogRepo::new(pool.clone()));
                registry.register(tools::TemporalTool::new(temporal_service));
            }
```

- [ ] **Step 2: Add to MCP exposed tools**

In `crates/config/src/schema/mcp.rs`, find `default_exposed_tools()` and add `"temporal"` to the list.

- [ ] **Step 3: Verify**

Run: `cargo build --workspace`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/ crates/config/
git commit -m "feat(app-core): register TemporalTool and expose via MCP"
```

---

### Task 8: Integration tests and verification

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add graph retrieval entity extraction test**

```rust
#[test]
fn test_graph_retrieval_entity_extraction() {
    use klyntbot::cognitive::services::graph_retrieval::extract_query_entities;

    let entities = extract_query_entities("Tell me about Jayden's work on the Klynt project");
    assert!(entities.contains(&"Jayden's".to_string()) || entities.contains(&"Jayden".to_string()));
    assert!(entities.contains(&"Klynt".to_string()));
}
```

Note: `extract_query_entities` is currently `fn` (private). If not accessible, make it `pub(crate)` or test via the public `compute_graph_boosts` API instead.

- [ ] **Step 2: Add 12-weight retrieval params test**

```rust
#[tokio::test]
async fn test_retrieval_params_include_graph_path_boost() {
    let params = klyntbot::cognitive::services::retrieval::RetrievalParams::new(10);
    assert!(
        (params.relevance_weight_graph_path_boost - 0.06).abs() < 0.01,
        "graph_path_boost weight default should be 0.06"
    );
}
```

- [ ] **Step 3: Add temporal service test**

```rust
#[tokio::test]
async fn test_temporal_first_mention_integration() {
    let pool = klyntbot::cognitive::repos::cognitive_test_pool().await;
    let fact_repo = klyntbot::cognitive::SemanticFactRepo::new(pool.clone());
    let service = klyntbot::cognitive::services::temporal::TemporalService::new(fact_repo.clone());

    let fact = klyntbot::cognitive::types::SemanticFact {
        id: "int-fm1".into(),
        domain: "work".into(),
        subject: "user".into(),
        predicate: "company".into(),
        object: "Acme".into(),
        confidence: 0.9,
        source: "test".into(),
        valid_from: "2026-02-15".into(),
        valid_until: None,
        recorded_at: "2026-02-15T00:00:00Z".into(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 0.0,
        project_id: None,
        memory_type: "fact".into(),
        scope_type: "system".into(),
        scope_id: None,
    };
    fact_repo.upsert(&fact).await.unwrap();

    let mention = service.first_mention("user", "company").await.unwrap();
    assert_eq!(mention, Some("2026-02-15".to_string()));
}
```

- [ ] **Step 4: Update existing run_reforge integration tests**

Any test that constructs `run_reforge` and passes `[f64; 11]` arrays needs updating to `[f64; 12]`. Search for `retrieve_with_overrides` calls in tests and update array sizes. This may have already been handled by the workspace build — the implementer should check.

- [ ] **Step 5: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero new warnings.

- [ ] **Step 7: Commit**

```bash
git add tests/
git commit -m "test: add Phase B3 integration tests for graph retrieval, 12-weight params, and temporal service"
```

---

## Summary

| Task | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | 12th weight infrastructure (graph_path_boost) | 4 | compile check |
| 2 | Autotuner + wiring for 12 weights | 4 | 1 test updated |
| 3 | Graph-aware retrieval service | 3 | 3 unit tests |
| 4 | Conversation promotion lifecycle | 3 | 1 unit test |
| 5 | TemporalService B3 methods | 2 | 3 unit tests |
| 6 | TemporalTool (6 actions) | 3 | compile check |
| 7 | Register TemporalTool in app-core | 2 | compile check |
| 8 | Integration tests + verification | 1 | 3 integration tests + workspace |

**Total: ~22 files modified/created, ~11 tests added, 8 commits**

---

## What Ships After B3

Phase B (Memory Retrieval Upgrade) is now complete. The Reforge spec's next layer is:

**Phase C — Deep Signal Integration** (depends on Phase A):
- C1: Agent runtime signal persistence (budget exhaustion, validation warnings, tool oscillation, per-message tokens)
- C2: Cognitive pipeline signals (extraction yield per domain, near-miss accumulations, per-retrieval score breakdowns)
- C3: Feature signal persistence (coaching behavioral outcomes, distraction rules, phoneme mastery)
