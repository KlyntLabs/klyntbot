# Phase B1 — Bridge: Cross-Retrieval, Entity Extraction, Temporal Log

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bridge conversation memory and knowledge graph into fact retrieval — add cross-retrieval boost (11th relevance weight), real-time entity extraction, temporal fact changelog, and lowered recall thresholds.

**Architecture:** Extend the existing 10-factor retrieval scoring with an 11th `recall_support` weight computed from conversation evidence overlap. Extend the LLM extraction prompt to emit entities and relationships alongside facts. Add an append-only `fact_changelog` table wired into `SemanticFactRepo` mutations. Lower `RecallCollector` gates from 3/2 to 2/1 for faster knowledge promotion.

**Tech Stack:** Rust, SQLite (cognitive/storage crates), LanceDB (conv_embeddings), existing extraction pipeline, autotuner weight system

**Depends on:** Phase A (complete). Phase B2+B3 depend on this plan.

---

## Scope Note

Phase B has three sub-phases (B1→B2→B3). This plan covers **B1 (Bridge)** only — it ships independently and provides immediate value. B2 (Enrich: value-density classifier, batch graph enrichment, Phase 6.5) and B3 (Dissolve: conversation promoter, graph-aware retrieval, temporal tool) will be separate plans after B1 lands.

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/repos/fact_changelog.rs` | Append-only changelog repo for fact mutations |

### Modified Files
| File | Change |
|------|--------|
| `crates/cognitive/src/services/decay.rs` | Add `recall_support` field to `RelevanceWeights`, add 11th parameter to `relevance_score()` |
| `crates/cognitive/src/services/retrieval.rs` | Add `relevance_weight_recall_support` to `RetrievalParams`, thread through scoring |
| `crates/cognitive/src/services/context_source.rs` | Add `relevance_weight_recall_support` to `CognitiveRetrievalConfig` |
| `crates/cognitive/src/services/memory_retriever.rs` | Implement cross-retrieval boost in `retrieve()`, update `retrieve_with_overrides` to `[f64; 11]` |
| `crates/config/src/schema/cognitive.rs` | Add `relevance_weight_recall_support` config field |
| `crates/common/src/autotuner.rs` | Add `relevance_weight_recall_support` to `TrialParams`, change `resolve_full_relevance_weights` to `[f64; 11]` |
| `crates/agent/src/agent_loop/builder.rs` | Map new config field, update shadow retriever defaults to 11 elements |
| `crates/agent/src/autotuner/shadow_retriever.rs` | Change `config_defaults` from `[f64; 10]` to `[f64; 11]` |
| `crates/cognitive/src/services/extraction.rs` | Add `ExtractedEntity`, `ExtractedRelationship` types to `BatchExtractionResult` |
| `crates/agent/src/adapters/cognitive_handlers.rs` | Extend `EXTRACTION_SYSTEM_PROMPT` for entities, parse entity JSON |
| `crates/cognitive/src/pipeline/writer.rs` | Persist extracted entities via `EntityRepo` |
| `crates/cognitive/src/pipeline/mod.rs` | Thread `EntityRepo` through pipeline |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `fact_changelog` table |
| `crates/cognitive/src/repos/mod.rs` | Export `FactChangelogRepo`, bump migration version |
| `crates/cognitive/src/repos/semantic_fact.rs` | Wire changelog recording into `upsert()`, `supersede()` |
| `crates/cognitive/src/pipeline/recall_collector.rs` | Lower `CLUSTER_THRESHOLD` to 2, `SESSION_THRESHOLD` to 1 |
| `tests/integration/cognitive.rs` | Add B1 integration tests |

---

### Task 1: Add `recall_support` weight to scoring infrastructure

**Files:**
- Modify: `crates/cognitive/src/services/decay.rs`
- Modify: `crates/cognitive/src/services/retrieval.rs`
- Modify: `crates/cognitive/src/services/context_source.rs`
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Add recall_support to RelevanceWeights**

In `crates/cognitive/src/services/decay.rs`, add the field and update the formula:

```rust
// In RelevanceWeights struct (after cross_note field):
pub recall_support: f64,

// In Default impl (after cross_note: 0.10):
recall_support: 0.08,

// Update doc comment on struct:
/// Configurable relevance weights for the 11-factor scoring formula.
```

- [ ] **Step 2: Add 11th parameter to relevance_score()**

In `crates/cognitive/src/services/decay.rs`, add the parameter and include it in the formula:

```rust
pub fn relevance_score(
    semantic_similarity: f64,
    retrievability: f64,
    importance: f64,
    access_frequency: f64,
    situational_boost: f64,
    temporal_recency: f64,
    hierarchy_score: f64,
    path_coherence: f64,
    community_score: f64,
    cross_note_boost: f64,
    recall_support: f64,       // NEW
    weights: &RelevanceWeights,
) -> f64 {
    (semantic_similarity * weights.semantic
        + retrievability * weights.retrievability
        + importance * weights.importance
        + access_frequency * weights.frequency
        + situational_boost * weights.situation
        + temporal_recency * weights.temporal
        + hierarchy_score * weights.hierarchy
        + path_coherence * weights.path_coherence
        + community_score * weights.community
        + cross_note_boost * weights.cross_note
        + recall_support * weights.recall_support)  // NEW
        .clamp(0.0, 1.0)
}
```

- [ ] **Step 3: Fix all call sites of relevance_score()**

In `crates/cognitive/src/services/retrieval.rs`, update both `vector_path()` and `fallback_path()` to pass `0.0` for the new `recall_support` parameter (it gets computed later, outside `retrieve_relevant_facts`):

```rust
// In vector_path(), the relevance_score() call — add 0.0 before `weights`:
    0.0,           // recall_support — computed post-hoc in UnifiedMemoryService
    cross_note,
    weights,

// Same in fallback_path():
    0.0,           // recall_support
    cross_note,
    weights,
```

Note: the `0.0` goes after `cross_note_boost` and before `weights`, matching the new parameter order.

- [ ] **Step 4: Add recall_support to RetrievalParams**

In `crates/cognitive/src/services/retrieval.rs`, add the field to `RetrievalParams` and its defaults:

```rust
// After relevance_weight_cross_note field:
pub relevance_weight_recall_support: f64,

// In RetrievalParams::new():
relevance_weight_cross_note: 0.10,
relevance_weight_recall_support: 0.08,  // NEW
```

Update the weights construction in `retrieve_relevant_facts()`:

```rust
    let weights = RelevanceWeights {
        semantic: params.relevance_weight_semantic,
        retrievability: params.relevance_weight_retrievability,
        importance: params.relevance_weight_importance,
        frequency: params.relevance_weight_frequency,
        situation: params.relevance_weight_situation,
        temporal: params.relevance_weight_temporal,
        hierarchy: params.relevance_weight_hierarchy,
        path_coherence: params.relevance_weight_path_coherence,
        community: params.relevance_weight_community,
        cross_note: params.relevance_weight_cross_note,
        recall_support: params.relevance_weight_recall_support,  // NEW
    };
```

- [ ] **Step 5: Add to CognitiveRetrievalConfig**

In `crates/cognitive/src/services/context_source.rs`:

```rust
// In CognitiveRetrievalConfig struct (after relevance_weight_cross_note):
pub relevance_weight_recall_support: f64,

// In Default impl (after relevance_weight_cross_note: 0.10):
relevance_weight_recall_support: 0.08,
```

- [ ] **Step 6: Add config field**

In `crates/config/src/schema/cognitive.rs`, add after the `relevance_weight_temporal` field:

```rust
    /// Relevance weight for conversation recall support (default: 0.08).
    /// Facts corroborated by conversation evidence score higher.
    #[serde(default = "default_w_recall_support")]
    pub relevance_weight_recall_support: f64,
```

Add the default function alongside the other weight defaults:

```rust
fn default_w_recall_support() -> f64 {
    0.08
}
```

- [ ] **Step 7: Verify**

Run: `cargo build -p cognitive -p config`
Expected: Compiles (some warnings about unused field in retrieval_params — OK at this stage).

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/ crates/config/
git commit -m "feat(cognitive): add recall_support as 11th relevance weight factor"
```

---

### Task 2: Extend autotuner and wiring for 11 weights

**Files:**
- Modify: `crates/common/src/autotuner.rs`
- Modify: `crates/agent/src/autotuner/shadow_retriever.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add recall_support to TrialParams**

In `crates/common/src/autotuner.rs`, add to the `TrialParams` struct (after `relevance_weight_cross_note`):

```rust
pub relevance_weight_recall_support: Option<f64>,
```

- [ ] **Step 2: Change resolve_full_relevance_weights to 11 elements**

In `crates/common/src/autotuner.rs`, update the function signature and body:

```rust
    /// Resolve all 11 relevance weights to a normalized array that sums to 1.0.
    /// Returns [semantic, retrievability, importance, frequency, situation, temporal,
    ///          hierarchy, path_coherence, community, cross_note, recall_support].
    pub fn resolve_full_relevance_weights(&self, defaults: &[f64; 11]) -> [f64; 11] {
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
            self.relevance_weight_recall_support.unwrap_or(defaults[10]),  // NEW
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

Update `resolve_full_relevance_weights_sums_to_one` test to use `[f64; 11]`:

```rust
    #[test]
    fn resolve_full_relevance_weights_sums_to_one() {
        let params = TrialParams {
            relevance_weight_semantic: Some(0.30),
            relevance_weight_hierarchy: Some(0.20),
            relevance_weight_community: Some(0.25),
            ..Default::default()
        };
        let defaults = [0.20, 0.10, 0.08, 0.05, 0.15, 0.02, 0.10, 0.05, 0.15, 0.10, 0.08];
        let weights = params.resolve_full_relevance_weights(&defaults);
        let sum: f64 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "All 11 weights must sum to 1.0, got {sum}"
        );
    }
```

- [ ] **Step 4: Update AgentShadowRetriever**

In `crates/agent/src/autotuner/shadow_retriever.rs`, change `[f64; 10]` to `[f64; 11]`:

```rust
pub struct AgentShadowRetriever {
    memory_service: Arc<UnifiedMemoryService>,
    config_defaults: [f64; 11],
}

impl AgentShadowRetriever {
    pub fn new(memory_service: Arc<UnifiedMemoryService>, config_defaults: [f64; 11]) -> Self {
        Self {
            memory_service,
            config_defaults,
        }
    }
}
```

- [ ] **Step 5: Update builder.rs config_defaults array**

In `crates/agent/src/agent_loop/builder.rs`, where the `config_defaults` array is built (around line 1697), add the 11th element:

```rust
                    let config_defaults: [f64; 11] = [
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
                        config.cognitive.relevance_weight_recall_support, // NEW
                    ];
```

- [ ] **Step 6: Update builder.rs retrieval config construction**

In `crates/agent/src/agent_loop/builder.rs`, where `CognitiveRetrievalConfig` is built (around line 339), add:

```rust
                    relevance_weight_recall_support: config.cognitive.relevance_weight_recall_support,
```

After `relevance_weight_cross_note: 0.10,`.

- [ ] **Step 7: Update memory_retriever resolve_retrieval_params**

In `crates/cognitive/src/services/memory_retriever.rs`, in `resolve_retrieval_params()`, add after the existing weight mappings:

```rust
            relevance_weight_recall_support: self.config.relevance_weight_recall_support,
```

And in the champion override section, update the defaults array and mapping:

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
                self.config.relevance_weight_recall_support,  // NEW
            ];
            let w = champion.resolve_full_relevance_weights(&defaults);
            params.relevance_weight_semantic = w[0];
            params.relevance_weight_retrievability = w[1];
            params.relevance_weight_importance = w[2];
            params.relevance_weight_frequency = w[3];
            params.relevance_weight_situation = w[4];
            params.relevance_weight_temporal = w[5];
            params.relevance_weight_hierarchy = w[6];
            params.relevance_weight_path_coherence = w[7];
            params.relevance_weight_community = w[8];
            params.relevance_weight_cross_note = w[9];
            params.relevance_weight_recall_support = w[10];  // NEW
```

- [ ] **Step 8: Update retrieve_with_overrides**

In `crates/cognitive/src/services/memory_retriever.rs`, change the signature and body:

```rust
    pub async fn retrieve_with_overrides(
        &self,
        query: &str,
        vector_top_k: usize,
        min_similarity: f64,
        relevance_weights: [f64; 11],  // was [f64; 10]
    ) -> common::Result<Vec<ScoredFact>> {
```

And update the params construction inside the method to include:

```rust
            relevance_weight_recall_support: relevance_weights[10],  // NEW
```

- [ ] **Step 9: Verify**

Run: `cargo build --workspace`
Expected: Clean compile.

- [ ] **Step 10: Commit**

```bash
git add crates/common/ crates/agent/ crates/cognitive/
git commit -m "feat(common): extend autotuner and wiring for 11-weight recall_support"
```

---

### Task 3: Implement cross-retrieval boost in UnifiedMemoryService

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add recall_support_score helper**

Add this function at the module level in `memory_retriever.rs`, after `content_overlaps()`:

```rust
/// Compute recall support score for a fact based on conversation evidence.
///
/// For each recall result, checks if the fact's content has significant word
/// overlap with the conversation snippet. Returns the max overlap score
/// across all recalls, clamped to [0.0, 1.0].
///
/// This is the 11th relevance signal: facts corroborated by conversation
/// evidence are more likely to be relevant to the user's current context.
fn recall_support_score(fact_content: &str, recall_contents: &[&str]) -> f64 {
    if recall_contents.is_empty() {
        return 0.0;
    }
    let fact_lower = fact_content.to_lowercase();
    let fact_words: std::collections::HashSet<&str> = fact_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .collect();
    if fact_words.is_empty() {
        return 0.0;
    }

    let mut best = 0.0_f64;
    for recall in recall_contents {
        let recall_lower = recall.to_lowercase();
        let recall_words: std::collections::HashSet<&str> = recall_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();
        if recall_words.is_empty() {
            continue;
        }
        let intersection = fact_words.intersection(&recall_words).count();
        let union = fact_words.union(&recall_words).count();
        let jaccard = intersection as f64 / union as f64;
        best = best.max(jaccard);
    }

    // Scale: 0.3+ Jaccard overlap maps to 0.0-1.0 recall support
    ((best - 0.15) / 0.35).clamp(0.0, 1.0)
}
```

- [ ] **Step 2: Apply cross-retrieval boost in retrieve()**

In the `MemoryRetriever::retrieve()` implementation, after the `tokio::join!` that fetches `facts_raw`, `recalls_raw`, and `episodes_raw`, insert the cross-retrieval boost before the RRF merge:

```rust
        // 1. Fetch concurrently
        let (mut facts_raw, recalls_raw, episodes_raw) = tokio::join!(
            self.fetch_facts(query, limit),
            self.fetch_recalls(query, limit),
            self.fetch_episodes(query, 5)
        );

        // 1b. Cross-retrieval: boost facts corroborated by conversation evidence
        if !recalls_raw.is_empty() && !facts_raw.is_empty() {
            let weight = self.config.relevance_weight_recall_support;
            if weight > 0.0 {
                let recall_texts: Vec<&str> =
                    recalls_raw.iter().map(|(_, _, c)| c.as_str()).collect();
                for (_, score, content, _) in &mut facts_raw {
                    let support = recall_support_score(content, &recall_texts);
                    *score += support * weight;
                }
                // Re-sort after boosting
                facts_raw.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        if facts_raw.is_empty() && recalls_raw.is_empty() && episodes_raw.is_empty() {
```

Note: move the early-return check to after the boost block.

- [ ] **Step 3: Add unit tests for recall_support_score**

```rust
    #[test]
    fn test_recall_support_score_no_recalls() {
        assert_eq!(recall_support_score("user peak hours 10am", &[]), 0.0);
    }

    #[test]
    fn test_recall_support_score_strong_overlap() {
        let fact = "user: peak_hours = 10am-12pm [strong]";
        let recalls = &["I mentioned my peak hours are around 10am to 12pm yesterday"];
        let score = recall_support_score(fact, recalls);
        assert!(score > 0.3, "Strong overlap should give high support, got {score}");
    }

    #[test]
    fn test_recall_support_score_no_overlap() {
        let fact = "user: peak_hours = 10am-12pm [strong]";
        let recalls = &["Let's schedule the deployment for Friday afternoon"];
        let score = recall_support_score(fact, recalls);
        assert!(score < 0.1, "No overlap should give near-zero support, got {score}");
    }

    #[test]
    fn test_recall_support_score_partial_overlap() {
        let fact = "user: favorite_language = Rust [moderate]";
        let recalls = &["I was working on the Rust compiler yesterday for the project"];
        let score = recall_support_score(fact, recalls);
        assert!(score > 0.0, "Partial overlap should give some support, got {score}");
    }
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p cognitive -E 'test(recall_support)'`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): implement cross-retrieval boost from conversation evidence"
```

---

### Task 4: Extend extraction for entity and relationship extraction

**Files:**
- Modify: `crates/cognitive/src/services/extraction.rs`
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/cognitive/src/pipeline/writer.rs`

- [ ] **Step 1: Add entity types to extraction.rs**

In `crates/cognitive/src/services/extraction.rs`, add after `BatchExtractionResult`:

```rust
/// An entity extracted alongside facts from an observation.
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

/// A relationship between two extracted entities.
#[derive(Debug, Clone)]
pub struct ExtractedRelationship {
    pub source_name: String,
    pub target_name: String,
    pub relationship_type: String,
}
```

Add fields to `BatchExtractionResult`:

```rust
pub struct BatchExtractionResult {
    /// Facts grouped by source observation index.
    pub extractions: Vec<BatchExtraction>,
    /// Indices of observations that used heuristic fallback (LLM failed).
    pub fallback_indices: Vec<usize>,
    /// Entities discovered across all observations in the batch.
    pub entities: Vec<ExtractedEntity>,
    /// Relationships between discovered entities.
    pub relationships: Vec<ExtractedRelationship>,
}
```

- [ ] **Step 2: Fix all BatchExtractionResult construction sites**

The heuristic handler in `cognitive_handlers.rs` and any test code that constructs `BatchExtractionResult` needs the new fields. Add `entities: Vec::new(), relationships: Vec::new()` to every construction site.

In `crates/agent/src/adapters/cognitive_handlers.rs`, `extract_facts_batch_sync()`:

```rust
        cognitive::BatchExtractionResult {
            extractions,
            fallback_indices: Vec::new(),
            entities: Vec::new(),       // Heuristic handler doesn't extract entities
            relationships: Vec::new(),
        }
```

And in `LlmExtractionHandler::fallback_all()`:

```rust
        let mut result = self.fallback.extract_facts_batch_sync(observations);
        result.fallback_indices = (0..observations.len()).collect();
        result
```

(This already works since `fallback` returns the updated struct.)

In `crates/cognitive/src/services/extraction.rs` test `test_batch_extraction_result_structure`:

```rust
        let result = BatchExtractionResult {
            extractions: vec![...],
            fallback_indices: vec![1],
            entities: Vec::new(),
            relationships: Vec::new(),
        };
```

- [ ] **Step 3: Extend EXTRACTION_SYSTEM_PROMPT for entities**

In `crates/agent/src/adapters/cognitive_handlers.rs`, append to `EXTRACTION_SYSTEM_PROMPT`:

```rust
const EXTRACTION_SYSTEM_PROMPT: &str = "\
You are a semantic memory extraction agent. Given an observation about a user, \
extract structured facts as subject-predicate-object triples.\n\n\
Domains: identity, energy, work, finance, learning, preferences, general\n\
Subjects: usually \"user\", or \"project:<name>\", \"task:<id>\"\n\
Predicates: descriptive relationship (e.g., \"name\", \"favorite_language\", \"peak_hours\", \"occupation\")\n\
Object: the value (e.g., \"Jayden\", \"Rust\", \"10am-12pm\", \"software developer\")\n\n\
Rules:\n\
- Extract EVERY distinct fact from the observation as a separate triple\n\
- Set confidence based on certainty (user-stated = 1.0, inferred = 0.5-0.8)\n\
- Use source \"user_stated\" for explicit statements, \"observed\" for behavioral data, \"inferred\" for patterns\n\
- Return empty facts array if nothing meaningful can be extracted\n\
- Questions (e.g., 'What is my name?') are NOT facts — return empty array for questions\n\
- Be specific in predicates — use snake_case names like \"name\", \"occupation\", \"favorite_language\"\n\
- Be concise in objects — just the value, not the full sentence\n\n\
Additionally, extract named entities (people, organizations, projects, technologies, places) \
and relationships between them. Only extract entities explicitly mentioned.\n\n\
Respond with JSON in this exact format:\n\
{\"facts\": [{\"domain\": \"identity\", \"subject\": \"user\", \"predicate\": \"name\", \"object\": \"Jayden\", \"confidence\": 1.0, \"source\": \"user_stated\"}], \
\"entities\": [{\"name\": \"Klynt\", \"type\": \"project\", \"description\": \"AI assistant project\"}], \
\"relationships\": [{\"source\": \"Jayden\", \"target\": \"Klynt\", \"type\": \"works_on\"}]}";
```

- [ ] **Step 4: Add entity JSON parsing types**

In `crates/agent/src/adapters/cognitive_handlers.rs`, add alongside `ExtractedFactJson`:

```rust
#[derive(serde::Deserialize)]
struct ExtractedEntityJson {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
    description: Option<String>,
}

#[derive(serde::Deserialize)]
struct ExtractedRelationshipJson {
    source: String,
    target: String,
    #[serde(rename = "type")]
    relationship_type: String,
}
```

Update `ObservationExtraction` to include entities and relationships:

```rust
#[derive(serde::Deserialize)]
struct ObservationExtraction {
    observation_index: usize,
    facts: Vec<ExtractedFactJson>,
    #[serde(default)]
    entities: Vec<ExtractedEntityJson>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationshipJson>,
}
```

For single-observation extraction (the common path), also add a response struct:

```rust
#[derive(serde::Deserialize)]
struct SingleExtractionResponse {
    facts: Vec<ExtractedFactJson>,
    #[serde(default)]
    entities: Vec<ExtractedEntityJson>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationshipJson>,
}
```

- [ ] **Step 5: Wire entity parsing into LLM extraction**

In the `LlmExtractionHandler`'s `extract_facts_batch` implementation, after parsing facts from the LLM response, also collect entities and relationships. The implementer should find where `BatchExtractionResult` is constructed from the LLM JSON and add:

```rust
let entities: Vec<cognitive::ExtractedEntity> = parsed_entities
    .into_iter()
    .map(|e| cognitive::ExtractedEntity {
        name: e.name,
        entity_type: e.entity_type,
        description: e.description,
    })
    .collect();

let relationships: Vec<cognitive::ExtractedRelationship> = parsed_relationships
    .into_iter()
    .map(|r| cognitive::ExtractedRelationship {
        source_name: r.source,
        target_name: r.target,
        relationship_type: r.relationship_type,
    })
    .collect();
```

Include these in the returned `BatchExtractionResult`.

- [ ] **Step 6: Persist entities in pipeline writer**

In `crates/cognitive/src/pipeline/writer.rs`, add entity persistence. The writer's `execute_promotions` function (or equivalent) needs access to `EntityRepo`. Add entity persistence after fact promotions:

```rust
/// Persist extracted entities and relationships.
pub async fn persist_entities(
    entity_repo: &crate::repos::EntityRepo,
    entities: &[crate::extraction::ExtractedEntity],
    relationships: &[crate::extraction::ExtractedRelationship],
) {
    use std::collections::HashMap;

    // Upsert entities, collecting name → id mapping
    let mut name_to_id: HashMap<String, String> = HashMap::new();
    for entity in entities {
        let new_entity = crate::repos::entity::NewEntity {
            name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            description: entity.description.clone(),
            source: "extracted".to_string(),
            source_id: None,
            metadata: None,
        };
        match entity_repo.upsert_entity(&new_entity).await {
            Ok(row) => {
                name_to_id.insert(entity.name.to_lowercase(), row.id);
            }
            Err(e) => {
                tracing::debug!("Failed to upsert entity '{}': {e}", entity.name);
            }
        }
    }

    // Upsert relationships (only if both entities were resolved)
    for rel in relationships {
        let source_id = name_to_id.get(&rel.source_name.to_lowercase());
        let target_id = name_to_id.get(&rel.target_name.to_lowercase());
        if let (Some(src), Some(tgt)) = (source_id, target_id) {
            let new_rel = crate::repos::entity::NewRelationship {
                source_entity_id: src.clone(),
                target_entity_id: tgt.clone(),
                relationship_type: rel.relationship_type.clone(),
                evidence: None,
                source: "extracted".to_string(),
            };
            if let Err(e) = entity_repo.upsert_relationship(&new_rel).await {
                tracing::debug!("Failed to upsert relationship: {e}");
            }
        }
    }
}
```

- [ ] **Step 7: Call persist_entities from the background service**

The implementer should find where `BatchExtractionResult` is consumed in the pipeline (likely in `background.rs` or `writer.rs`) and add:

```rust
if !extraction_result.entities.is_empty() || !extraction_result.relationships.is_empty() {
    if let Some(ref entity_repo) = entity_repo {
        persist_entities(entity_repo, &extraction_result.entities, &extraction_result.relationships).await;
    }
}
```

The `entity_repo` must be threaded through the pipeline. Check `BackgroundConsolidationService` construction to see if `EntityRepo` is already available (it may already be wired for other purposes). If not, add `entity_repo: Option<EntityRepo>` as a field.

- [ ] **Step 8: Verify**

Run: `cargo build -p cognitive -p agent`
Expected: Compiles.

- [ ] **Step 9: Commit**

```bash
git add crates/cognitive/ crates/agent/
git commit -m "feat(cognitive): extend extraction pipeline with entity and relationship extraction"
```

---

### Task 5: Temporal fact changelog table and repo

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Create: `crates/cognitive/src/repos/fact_changelog.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`

- [ ] **Step 1: Add fact_changelog table**

Append to the end of `crates/cognitive/migrations/001_cognitive_tables.sql`:

```sql
-- Temporal fact changelog: append-only log of every fact mutation.
-- Enables facts_as_of(timestamp) state reconstruction and change auditing.
CREATE TABLE IF NOT EXISTS fact_changelog (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_id TEXT NOT NULL,
    change_type TEXT NOT NULL,     -- 'create', 'update', 'supersede', 'archive', 'confidence', 'convergence'
    field_changed TEXT,            -- which field was modified (NULL for create)
    old_value TEXT,                -- previous value (NULL for create)
    new_value TEXT,                -- new value
    source TEXT,                   -- what triggered the change (e.g. 'extraction', 'consolidation', 'reforge')
    changed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_fact_changelog_fact_id
    ON fact_changelog(fact_id, changed_at);

CREATE INDEX IF NOT EXISTS idx_fact_changelog_type
    ON fact_changelog(change_type, changed_at);
```

- [ ] **Step 2: Bump cognitive migration version**

In `crates/cognitive/src/repos/mod.rs`, find the `cognitive_migrations()` function and update the version for the main cognitive tables migration from `15` to `16` (or whatever the current version is — the implementer should check).

- [ ] **Step 3: Create FactChangelogRepo**

Create `crates/cognitive/src/repos/fact_changelog.rs`:

```rust
//! Append-only changelog for semantic fact mutations.
//!
//! Every create, update, supersede, and archive operation on `semantic_facts`
//! is recorded here. This enables `facts_as_of(timestamp)` state reconstruction
//! in Phase B3 and provides an audit trail for Reforge analysis.

use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChangelogEntry {
    pub id: i64,
    pub fact_id: String,
    pub change_type: String,
    pub field_changed: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub source: Option<String>,
    pub changed_at: String,
}

#[derive(Debug, Clone)]
pub struct FactChangelogRepo {
    pool: SqlitePool,
}

impl FactChangelogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a fact mutation in the changelog.
    pub async fn record(
        &self,
        fact_id: &str,
        change_type: &str,
        field_changed: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        source: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO fact_changelog (fact_id, change_type, field_changed, old_value, new_value, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(fact_id)
        .bind(change_type)
        .bind(field_changed)
        .bind(old_value)
        .bind(new_value)
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all changes for a specific fact, newest first.
    pub async fn history(&self, fact_id: &str) -> Result<Vec<ChangelogEntry>, sqlx::Error> {
        sqlx::query_as::<_, ChangelogEntry>(
            "SELECT * FROM fact_changelog WHERE fact_id = ?1 ORDER BY changed_at DESC",
        )
        .bind(fact_id)
        .fetch_all(&self.pool)
        .await
    }

    /// List all changes since a timestamp, newest first.
    pub async fn changes_since(
        &self,
        since: &str,
        limit: u32,
    ) -> Result<Vec<ChangelogEntry>, sqlx::Error> {
        sqlx::query_as::<_, ChangelogEntry>(
            "SELECT * FROM fact_changelog WHERE changed_at > ?1 ORDER BY changed_at DESC LIMIT ?2",
        )
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete entries older than N days.
    pub async fn prune(&self, max_age_days: u32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM fact_changelog WHERE changed_at < datetime('now', ?1)",
        )
        .bind(format!("-{max_age_days} days"))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
```

- [ ] **Step 4: Export the changelog repo**

In `crates/cognitive/src/repos/mod.rs`, add:

```rust
pub mod fact_changelog;
pub use fact_changelog::FactChangelogRepo;
```

- [ ] **Step 5: Add optional changelog to SemanticFactRepo**

In `crates/cognitive/src/repos/semantic_fact.rs`, add a changelog field:

```rust
#[derive(Debug, Clone)]
pub struct SemanticFactRepo {
    pool: SqlitePool,
    changelog: Option<FactChangelogRepo>,
}
```

Update the constructor and add a builder method:

```rust
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, changelog: None }
    }

    pub fn with_changelog(mut self, changelog: FactChangelogRepo) -> Self {
        self.changelog = Some(changelog);
        self
    }
```

- [ ] **Step 6: Wire changelog into upsert()**

At the end of `SemanticFactRepo::upsert()`, after the SQL execute succeeds, record the change:

```rust
        // Record in changelog (fire-and-forget)
        if let Some(ref cl) = self.changelog {
            let spo = format!("{} {} {}", fact.subject, fact.predicate, fact.object);
            if let Err(e) = cl.record(&fact.id, "create", None, None, Some(&spo), Some(&fact.source)).await {
                tracing::debug!("Changelog record failed: {e}");
            }
        }
```

- [ ] **Step 7: Wire changelog into supersede()**

At the end of `SemanticFactRepo::supersede()`, after the SQL execute:

```rust
        if let Some(ref cl) = self.changelog {
            if let Err(e) = cl.record(old_id, "supersede", Some("superseded_by"), None, Some(new_id), None).await {
                tracing::debug!("Changelog record failed: {e}");
            }
        }
```

- [ ] **Step 8: Add test for changelog**

In `fact_changelog.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn test_record_and_history() {
        let pool = setup().await;
        let repo = FactChangelogRepo::new(pool);

        repo.record("f1", "create", None, None, Some("user peak_hours 10am"), Some("extraction"))
            .await
            .unwrap();
        repo.record("f1", "update", Some("object"), Some("10am"), Some("9am"), Some("consolidation"))
            .await
            .unwrap();

        let history = repo.history("f1").await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].change_type, "update"); // newest first
        assert_eq!(history[1].change_type, "create");
    }

    #[tokio::test]
    async fn test_changes_since() {
        let pool = setup().await;
        let repo = FactChangelogRepo::new(pool);

        repo.record("f1", "create", None, None, Some("test"), None).await.unwrap();

        let changes = repo.changes_since("2020-01-01", 100).await.unwrap();
        assert!(!changes.is_empty());
    }
}
```

- [ ] **Step 9: Verify**

Run: `cargo nextest run -p cognitive -E 'test(changelog)'`
Expected: All pass.

- [ ] **Step 10: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add temporal fact changelog for mutation auditing"
```

---

### Task 6: Lower RecallCollector thresholds

**Files:**
- Modify: `crates/cognitive/src/pipeline/recall_collector.rs`

- [ ] **Step 1: Change the threshold constants**

In `crates/cognitive/src/pipeline/recall_collector.rs`:

```rust
/// Minimum messages in a cluster to promote a signal.
const CLUSTER_THRESHOLD: usize = 2;  // was: 3
/// Minimum distinct sessions required across cluster members.
const SESSION_THRESHOLD: usize = 1;  // was: 2
```

- [ ] **Step 2: Update the session_threshold_enforcement test**

The existing `test_session_threshold_enforcement` test expects `SESSION_THRESHOLD == 2`. Update it to reflect the new threshold of 1:

```rust
    #[test]
    fn test_single_session_now_promotes() {
        // With SESSION_THRESHOLD = 1, messages from a single session
        // that form a large enough cluster should now be promoted.
        let messages = vec![
            msg("How do I handle errors in Rust", "s1"),
            msg("Error handling in Rust with Result types", "s1"),
            msg("Rust error handling patterns best practices", "s1"),
        ];
        let clusters = cluster_messages(&messages);
        let largest = clusters.iter().max_by_key(|c| c.len()).unwrap();
        let sessions: HashSet<&str> = largest.iter().map(|m| m.session_key.as_str()).collect();
        // Even with 1 session, cluster meets both thresholds (2+ messages, 1+ session)
        assert!(largest.len() >= CLUSTER_THRESHOLD);
        assert!(sessions.len() >= SESSION_THRESHOLD);
    }
```

- [ ] **Step 3: Update confidence test if needed**

The `test_confidence_scaling` test uses `conf(3)` which assumed CLUSTER_THRESHOLD=3. With the new threshold=2, the minimum promoted cluster has 2 messages:

```rust
    #[test]
    fn test_confidence_scaling() {
        let conf = |n: usize| (0.4 + n as f64 * 0.1_f64).min(0.9);
        // 2 messages → 0.6 (now the minimum promoted cluster)
        assert!((conf(2) - 0.6).abs() < 0.01, "conf(2) = {}", conf(2));
        // 5 messages → 0.9 (capped)
        assert!((conf(5) - 0.9).abs() < 0.01, "conf(5) = {}", conf(5));
    }
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p cognitive -E 'test(recall)'`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): lower RecallCollector thresholds for faster knowledge promotion"
```

---

### Task 7: Integration tests and verification

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add cross-retrieval test**

```rust
#[tokio::test]
async fn test_cross_retrieval_boosts_corroborated_facts() {
    use klyntbot::cognitive::services::memory_retriever::recall_support_score;

    // Fact content that closely matches a conversation
    let fact = "user: peak_hours = works best between 10am and noon [strong]";
    let recall = &["I told you earlier that my peak hours are 10am to noon"];
    let score = recall_support_score(fact, recall);
    assert!(score > 0.2, "Corroborated fact should get recall support, got {score}");

    // Unrelated conversation should give no boost
    let unrelated = &["Can you help me with my grocery list for tonight"];
    let no_score = recall_support_score(fact, unrelated);
    assert!(no_score < 0.1, "Unrelated recall should give no support, got {no_score}");
}
```

Note: this requires `recall_support_score` to be `pub`. If it's private, either make it `pub(crate)` or test indirectly via the full retrieval path.

- [ ] **Step 2: Add fact changelog integration test**

```rust
#[tokio::test]
async fn test_fact_changelog_records_mutations() {
    let pool = klyntbot::cognitive::repos::cognitive_test_pool().await;
    let changelog = klyntbot::cognitive::FactChangelogRepo::new(pool.clone());
    let fact_repo = klyntbot::cognitive::SemanticFactRepo::new(pool.clone())
        .with_changelog(changelog.clone());

    // Create a fact — should record "create" in changelog
    let fact = klyntbot::cognitive::types::SemanticFact {
        id: "cl1".into(),
        domain: "test".into(),
        subject: "user".into(),
        predicate: "test_pred".into(),
        object: "value1".into(),
        confidence: 0.8,
        source: "test".into(),
        valid_from: "2026-04-01".into(),
        valid_until: None,
        recorded_at: "2026-04-01T00:00:00".into(),
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

    let history = changelog.history("cl1").await.unwrap();
    assert!(!history.is_empty(), "upsert should record a changelog entry");
    assert_eq!(history[0].change_type, "create");
}
```

- [ ] **Step 3: Add 11-weight retrieval params test**

```rust
#[tokio::test]
async fn test_retrieval_params_include_recall_support() {
    // Verify the weight field exists and has the expected default
    let params = klyntbot::cognitive::retrieval::RetrievalParams::new(10);
    assert!(
        (params.relevance_weight_recall_support - 0.08).abs() < 0.01,
        "recall_support weight default should be 0.08"
    );
}
```

- [ ] **Step 4: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass (including existing tests — no regressions).

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 6: Commit**

```bash
git add tests/
git commit -m "test: add Phase B1 integration tests for cross-retrieval, changelog, and 11-weight params"
```

---

## Summary

| Task | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | recall_support weight infrastructure | 4 | compile check |
| 2 | Autotuner + wiring for 11 weights | 4 | existing autotuner test updated |
| 3 | Cross-retrieval boost | 1 | 4 unit tests |
| 4 | Entity extraction in pipeline | 3 | compile check |
| 5 | Temporal fact changelog | 4 | 2 unit tests |
| 6 | RecallCollector threshold adjustment | 1 | existing tests updated |
| 7 | Integration tests + verification | 1 | 3 integration tests + workspace build |

**Total: ~18 files modified/created, ~9 tests added, 7 commits**

---

## Missing Methods Checklist

The implementer may find these methods missing and need to add them:

| Repo | Method | Needed For |
|------|--------|-----------|
| `EntityRepo` | `upsert_relationship()` | Task 4 Step 6 — verify it exists, signature in entity.rs |
| `BackgroundConsolidationService` | `entity_repo` field | Task 4 Step 7 — may need threading through pipeline |
| `SemanticFactRepo` | `find_by_subject_predicate()` | Already used by TemporalService — verify it exists |
| `SemanticFactRepo` | `search_archived_by_subject_predicate()` | Already used by TemporalService — verify it exists |

---

## What Ships After B1

**Phase B2 (Enrich)** — depends on B1's entity extraction and temporal log:
- B2a: Value-density classifier (heuristic scoring per conversation turn)
- B2b: Batch graph enrichment (LLM entity resolution)
- B2c: Reforge Phase 6.5: Graph Consolidation (new phase between Optimize and Compact)
- B2d: Temporal snapshots (nightly knowledge graph snapshots)

**Phase B3 (Dissolve)** — depends on B1+B2:
- B3a: Conversation promoter (promotion lifecycle for conv_embeddings)
- B3b: Graph-aware retrieval (entity graph → vector search → merge)
- B3c: Temporal reasoning tool (TemporalTool, multi-action, read-only)
