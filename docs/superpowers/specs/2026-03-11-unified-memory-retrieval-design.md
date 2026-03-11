# Unified Memory Retrieval Paths

> Design spec for recommendation #3 from system-architecture-analysis.md
> Date: 2026-03-11

## Problem

Conversation recall (`conv_embeddings`) and cognitive facts (`cognitive_fact_embeddings`) use separate retrieval paths with different scoring models. Both inject context into the LLM prompt independently — conversation recall via `MemoryRetriever` in the assembler, cognitive facts via `CognitiveContextSource`. This causes:

- No unified relevance ranking across memory types
- Potential duplicate content in the prompt
- Two scoring models (time-decay vs FSRS 5-factor) that aren't comparable
- No coordination between the two injection points

## Approach: Unified MemoryService + Enriched Trait (Approach C)

Create a `UnifiedMemoryService` in the cognitive crate that owns both retrieval paths, merges results via RRF, and deduplicates. Enrich the `MemoryRetriever` trait with source metadata. Strip `CognitiveContextSource` down to static identity context only.

### Architecture

```
ContextEngine::assemble()
├── CognitiveContextSource (static only, priority 60)
│   ├── Identity facts (top N by confidence x stability per domain)
│   ├── Procedural rules
│   └── Confidence calibration
│
└── retrieve_memory() via UnifiedMemoryService
    ├── ConversationRecallService (time-decay scoring)
    ├── retrieve_relevant_facts() (FSRS 5-factor scoring)
    ├── Score normalization (min-max per source)
    ├── RRF merge (k=60.0)
    ├── Deduplication (facts win over overlapping recalls)
    └── Grouped output: facts section + recalls section
```

## Component Changes

### 1. Enriched MemoryRetriever Trait

File: `crates/context_engine/src/memory_retriever.rs`

Add `MemorySource` enum (`CognitiveFact`, `ConversationRecall`) and enrich `MemoryEntry` with `source: MemorySource` and `raw_score: f64`. Trait signature unchanged: `retrieve(query, limit) -> Vec<MemoryEntry>`.

### 2. UnifiedMemoryService

File: `crates/cognitive/src/memory_retriever.rs`

Replaces `CognitiveMemoryRetriever`. Fields:

- `recall: Option<Arc<ConversationRecallService>>` — conversation recall (optional, may be disabled)
- `fact_repo: SemanticFactRepo` — for FSRS fact retrieval
- `embedder: Option<Arc<dyn SemanticFactEmbedder>>` — for vector search
- `config: CognitiveRetrievalConfig` — weights, thresholds, limits
- `situation: Option<Arc<Mutex<UserSituation>>>` — situational boost

Retrieve flow:

1. Fetch from both sources concurrently via `tokio::join!`
2. Min-max normalize scores within each source to 0.0-1.0
3. RRF merge: `rrf_score = sum(1/(k + rank_in_source))` with k=60.0
4. Deduplicate: substring match of fact `(subject, predicate, object)` against recall content. Facts win.
5. Sort by RRF score, truncate to limit

### 3. Slimmed CognitiveContextSource

File: `crates/cognitive/src/context_source.rs`

Remove:
- Dynamic tier block (lines 240-302) — the `retrieve_relevant_facts()` call
- `embedder` field
- `situation` field
- Dynamic retrieval config fields (weights, vector_top_k, min_similarity)

Keep:
- Static tier: top facts by `confidence x stability` per domain
- Procedural rules
- Confidence calibration
- Cache (for static model only)

### 4. Assembler Formatting

File: `crates/context_engine/src/assembler.rs`

Update `retrieve_memory()` to partition results by `MemorySource` and format as grouped sections:

```
[Relevant Context]

## Relevant Facts
- user: peak_hours = 10am-12pm (relevance: 0.85)

## Related Conversations
- "I usually start deep work around 10am" (relevance: 0.78)
```

Facts appear first (more structured). Either section omitted if empty.

### 5. Builder Wiring

File: `crates/agent/src/agent_loop/builder.rs`

Replace `CognitiveMemoryRetriever::new(recall)` with:

```rust
UnifiedMemoryService::new(fact_repo)
    .with_recall(recall_service)
    .with_embedder_opt(embedder)
    .with_config(retrieval_config)
    .with_situation(situation)
```

Move embedder, situation, and dynamic config from `CognitiveContextSource` builder to `UnifiedMemoryService` builder.

## Testing Strategy

| Test | Location | Validates |
|------|----------|-----------|
| RRF merge correctness | `cognitive/src/memory_retriever.rs` | Two ranked lists merge with expected scores |
| Deduplication | `cognitive/src/memory_retriever.rs` | Facts win over overlapping recalls |
| Single source only | `cognitive/src/memory_retriever.rs` | Works with recall-only or facts-only |
| Empty results | `cognitive/src/memory_retriever.rs` | Graceful when both sources return nothing |
| Score normalization | `cognitive/src/memory_retriever.rs` | Min-max produces 0.0-1.0 range |
| Static-only context source | `cognitive/src/context_source.rs` | Renders static facts + rules, no dynamic section |
| Assembler grouped output | `context_engine/src/assembler.rs` | Facts and recalls in separate sections |
| Builder wiring | `agent/src/agent_loop/builder.rs` | UnifiedMemoryService injected into ContextEngine |

All tests use ephemeral SQLite + mock embedder/recall.

## Files Changed

| File | Change |
|------|--------|
| `crates/context_engine/src/memory_retriever.rs` | Add `MemorySource`, enrich `MemoryEntry` |
| `crates/cognitive/src/memory_retriever.rs` | Replace `CognitiveMemoryRetriever` with `UnifiedMemoryService` |
| `crates/cognitive/src/context_source.rs` | Remove dynamic tier, embedder, situation |
| `crates/context_engine/src/assembler.rs` | Grouped formatting by source type |
| `crates/agent/src/agent_loop/builder.rs` | Wire `UnifiedMemoryService` with both sources |
| Existing tests across all changed files | Update mocks for new `MemoryEntry` fields |
