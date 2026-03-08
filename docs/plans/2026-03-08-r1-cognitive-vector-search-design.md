# R1: Connect Vector Search to Cognitive Retrieval

> Date: 2026-03-08 | Status: Approved | Impact: Critical (highest-impact recommendation)

## Problem

The FSRS relevance formula weights `semantic_similarity` at 0.30 (largest single factor), but `retrieval.rs:58` hardcodes it to `0.5`. Every fact scores identically on 30% of its weight, reducing the 5-factor formula to effectively 4 factors. The vector store infrastructure (LanceDB + fastembed 384-dim) exists and is proven for todos and conversations, but has zero connection to the cognitive memory system.

## Solution: A+ (Intent-Driven Multi-Domain Retrieval)

**Approach 1** (trait in `cognitive`, implement in `agent`) using the existing dependency inversion pattern.

### Key Design Decisions

- **Query source (C+):** Use the condensed intent summary from `IntentAnalyzer::analyze()` (already computed in AgentRuntime step 4) instead of raw user message. Cleaner, shorter, language-normalized.
- **Multi-domain pre-filter:** Primary domain from IntentAnalysis + active task/project domains + recent episodic memory domains. Reduces search space to ~50-300 facts before vector search.
- **Two-tier context injection (A+):** Static identity tier (always present) + dynamic relevant facts tier (query-dependent). Fallback to `importance × stability` when retrieval is weak.
- **Trait name:** `SemanticFactEmbedder` (not `CognitiveEmbeddingHandler`).

---

## Section 1: Data Layer — Embeddings Table

### LanceDB table: `cognitive_fact_embeddings`

| Column | Type | Purpose |
|--------|------|---------|
| `fact_id` | String | Matches `SemanticFact.id` (1:1) |
| `vector` | FixedSizeList<Float32>(384) | Embedding vector |
| `domain` | String | Pre-filtering |
| `text` | String | Embedded text (debug/reindex) |
| `importance` | Float32 | For hybrid scoring |
| `stability` | Float32 | FSRS value |
| `confidence` | Float32 | Fact confidence |
| `updated_at` | String | ISO timestamp |

**Embedding text formula:** `"{subject} {predicate} {object}"` (SPO triples are already concise semantic units).

**Write path:** On every `SemanticFactRepo::upsert()`, the consolidation service also calls `embedder.embed_and_store_fact(fact)` via fire-and-forget `tokio::spawn`.

**Delete path:** On fact supersession/archival, call `embedder.remove_embedding(fact_id)`.

**Backfill:** `CognitiveVectorInitializer` handles first-startup re-embedding + CLI `klyntbot cognitive reindex`.

**Index:** HNSW created after backfill completes.

---

## Section 2: Trait Definition — `SemanticFactEmbedder`

Defined in `crates/cognitive/src/embedder.rs`:

```rust
#[async_trait]
pub trait SemanticFactEmbedder: Send + Sync {
    /// Embed a semantic fact and store its vector in LanceDB.
    async fn embed_and_store_fact(&self, fact: &SemanticFact) -> common::Result<()>;

    /// Remove the embedding for a superseded/archived fact.
    async fn remove_embedding(&self, fact_id: &str) -> common::Result<()>;

    /// Search for facts similar to a query, pre-filtered by domains.
    /// Returns (fact_id, cosine_similarity) pairs, sorted desc.
    async fn search_similar(
        &self,
        query: &str,
        domains: &[&str],
        top_k: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<(String, f64)>>;

    /// Re-embed all active facts (backfill/reindex).
    async fn reindex_all(&self, facts: &[SemanticFact]) -> common::Result<usize>;

    /// Whether the embedder is available (model loaded).
    fn is_available(&self) -> bool;
}
```

---

## Section 3: Retrieval — `retrieve_relevant_facts()`

Replaces `retrieve_facts()` in `crates/cognitive/src/retrieval.rs`.

```rust
pub async fn retrieve_relevant_facts(
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    query: &str,
    domains: &[&str],
    limit: usize,
    vector_top_k: usize,       // default 30
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error>
```

### Two-path flow

**Vector path** (when embedder available + non-empty query):
1. `embedder.search_similar(query, domains, top_k=30, min_similarity=0.55)` → `Vec<(fact_id, similarity)>`
2. Batch-load facts from SQL: `repo.get_batch(&fact_ids)`
3. Score each: `relevance_score(real_similarity, retrievability, confidence, freq, situational_boost)`
4. Sort by score, truncate to `limit`, call `update_stability()` + increment `access_count`

**Fallback path** (embedder `None`, query empty, or vector returns < 3 results):
1. Load all active facts across requested domains from SQL
2. Score with `semantic_similarity = 0.5` + rank by `importance × stability`
3. Sort, truncate, record access

### Return type

```rust
pub struct ScoredFact {
    pub fact: SemanticFact,
    pub score: f64,
    pub similarity: Option<f64>,  // None = fallback path
}
```

---

## Section 4: Context Assembly — Two-Tier Injection

### `CognitiveContextSource` changes

```rust
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    cache: Mutex<Option<CachedModel>>,
}
```

### Two tiers in `provide()`

**Static tier** — Top 8-10 highest-importance facts + active procedural rules. Always included. Uses `importance × stability` ranking (no vector search). "Who is the user" baseline.

**Dynamic tier** — Top 12-15 `retrieve_relevant_facts()` results using `SourceContext.intent_summary` as query. Only when `intent_summary` is `Some`. "What's relevant right now."

### Output format

```markdown
# User Understanding

## Identity
- user: name = "Jayden"
## Preferences
- user: work_style = "deep focus"
## Learned Patterns
### productivity
- User works best after morning exercise (confidence: 80%, signals: 5)

---

## Relevant Personal Context (for this conversation)
- user: peak_hours = 10am-12pm (relevance: 0.87)
- user: current_project = klyntbot (relevance: 0.82)
```

Relevance scores only shown when > 0.6. Dynamic tier truncated first when token budget is tight.

### Caching
- Static tier: 60s TTL (existing)
- Dynamic tier: 30s TTL keyed by intent summary hash

### Config flag
`cognitive.dynamicFactsEnabled` (default `true`) — disables dynamic tier for ultra-minimal users.

---

## Section 5: Wiring & Integration

### 5.1 `SemanticFactEmbedderImpl` in `agent` crate

```rust
// crates/agent/src/cognitive_embedder.rs
pub struct SemanticFactEmbedderImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}
```

### 5.2 Boot sequence (`CognitiveVectorInitializer`)

Dedicated service handles:
1. `ensure_cognitive_table()` — create LanceDB table if missing
2. Detect first boot (no embeddings) → spawn background `reindex_all()`
3. Create HNSW index after backfill

Called from `AppCore::init()` after `VectorStore::new()` and `EmbeddingEngine::new()`.

### 5.3 Consolidation service

```rust
pub struct ConsolidationService {
    fact_repo: SemanticFactRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    // ...existing fields...
}
```

- `ADD`/`UPDATE` → `tokio::spawn(embedder.embed_and_store_fact(&fact))` (fire-and-forget)
- `DELETE` → `tokio::spawn(embedder.remove_embedding(&fact_id))`
- Embedding failures log warnings, never block SQL operations

### 5.4 Intent summary passthrough

```rust
pub struct SourceContext {
    pub channel: String,
    pub chat_id: String,
    pub message: Option<String>,
    pub intent_summary: Option<String>,  // NEW
}
```

Set in `AgentRuntime::process_message()` step 6 from `IntentAnalysis.reasoning`.
`CognitiveContextSource` uses `intent_summary` as vector query, falls back to `message`.

### 5.5 Config

```json
{
  "cognitive": {
    "dynamicFactsEnabled": true,
    "staticFactLimit": 10,
    "dynamicFactLimit": 15,
    "vectorTopK": 30,
    "minSimilarity": 0.55
  }
}
```

### 5.6 Files touched

| File | Change |
|------|--------|
| `crates/cognitive/src/embedder.rs` | **NEW** — `SemanticFactEmbedder` trait |
| `crates/cognitive/src/retrieval.rs` | `retrieve_facts` → `retrieve_relevant_facts` |
| `crates/cognitive/src/context_source.rs` | Two-tier injection, embedder dep |
| `crates/cognitive/src/lib.rs` | Export new module |
| `crates/storage/src/vector_store.rs` | New table + `ensure_cognitive_table()` |
| `crates/context_engine/src/source.rs` | `intent_summary` field |
| `crates/agent/src/cognitive_embedder.rs` | **NEW** — `SemanticFactEmbedderImpl` |
| `crates/agent/src/agent_runtime/runtime.rs` | Pass intent summary to SourceContext |
| `crates/app-core/src/lib.rs` | Wire embedder in `init()` |
| `crates/cognitive/src/consolidation.rs` | Embed on upsert, remove on delete |
| `crates/config/src/lib.rs` | `CognitiveConfig` struct |

### 5.7 Error handling

- All embedding calls are non-blocking (`tokio::spawn`)
- Embedding failures → `warn!` log, never fail SQL upsert or consolidation
- When `embedder` is `None` → graceful degradation to fallback path (same behavior as today)

---

## Section 6: Testing Strategy

### Unit tests

**`cognitive/src/retrieval.rs`:**
- `test_retrieve_relevant_facts_with_mock_embedder` — mock returns known similarities, verify FSRS re-ranking
- `test_fallback_when_embedder_is_none`
- `test_fallback_when_query_is_empty`
- `test_fallback_when_vector_returns_few_results` — < 3 results triggers fallback merge
- `test_scored_fact_records_access` — verify stability/access_count updates

**`cognitive/src/context_source.rs`:**
- `test_static_tier_always_included`
- `test_dynamic_tier_with_intent_summary`
- `test_dynamic_tier_hidden_when_disabled`
- `test_relevance_score_hidden_below_threshold` — scores ≤ 0.6 not shown

**`cognitive/src/embedder.rs`:**
- `MockSemanticFactEmbedder` — configurable similarity scores, call tracking

**`storage/src/vector_store.rs`:**
- `test_cognitive_table_creation`
- `test_upsert_and_search_cognitive_embeddings`

### Integration test

Feature-gated `#[cfg(feature = "semantic-search")]`:
- Insert facts → embed with real `EmbeddingEngine` → `retrieve_relevant_facts()` with natural language query → verify semantic relevance of top results
