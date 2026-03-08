# R2: Unify Memory Systems — Design

> Date: 2026-03-08 | Status: Approved | Depends on: R1 (completed)

## Problem

Two parallel memory systems coexist independently:

1. **Old system** (`MemoryStore` in `agent/src/memory.rs`): Diary-style key-value notes (`memory_notes` SQLite table) + per-message conversation embeddings (`conv_embeddings` LanceDB table). Injected via `LearningContextSource` (priority 55, deprecated) and `ConversationMemoryRetriever`.

2. **Cognitive system** (`cognitive` crate): Structured SPO triples (`semantic_facts`), episodic events, procedural rules, FSRS decay, consolidation pipeline. Injected via `CognitiveContextSource` (priority 60). R1 connected vector search here.

Neither system cross-references the other. Two separate context injection paths. The `MemoryTool` name is misleading (only searches conversations). Maintenance burden is doubled.

## Approach

**Cognitive crate becomes the single owner of all memory.** Conversation recall moves into cognitive as a new module. Memory notes are dropped (cognitive extraction pipeline replaces diary-style notes). Breaking changes acceptable (pre-production).

### Rejected alternatives

- **B: Just remove notes layer** — Half-measure; two injection paths remain for conversation recall.
- **C: Fold conversations into episodic memories** — Granularity mismatch; episodic memories are event summaries, conversation embeddings are per-message.

## Architecture

### New modules in `cognitive` crate

```
cognitive/src/
├── conversation_recall.rs   ← NEW: ConversationRecallService
├── memory_retriever.rs      ← NEW: CognitiveMemoryRetriever (impl MemoryRetriever)
├── embedder.rs              ← MODIFIED: add TextEmbedder trait
├── context_source.rs        ← MODIFIED: absorb confidence threshold text
└── ... (rest unchanged)
```

### Layer diagram

```
L5: cognitive  ← owns ALL memory (facts + recall + episodes + rules)
    agent      ← implements TextEmbedder, wires everything
L4: tools      ← MemoryTool stays, uses ConversationRecallHandler trait
L3: context_engine ← MemoryRetriever trait (cognitive implements it)
L2: storage    ← VectorStore, SqlitePool (unchanged)
```

### Dependency inversion for embedding

Cognitive can't depend on `EmbeddingEngine` (in `tools` L4) without adding a coupling. Same pattern as `SemanticFactEmbedder`:

```rust
// cognitive/src/embedder.rs
#[async_trait]
pub trait TextEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}
```

Agent implements this wrapping `EmbeddingEngine`. Cognitive's `ConversationRecallService` takes `Arc<dyn TextEmbedder>`.

## ConversationRecallService

Core struct in `cognitive/src/conversation_recall.rs`:

```rust
pub struct ConversationRecallService {
    vector_store: VectorStore,
    embedder: Arc<dyn TextEmbedder>,
    config: RecallConfig,
}

pub struct RecallConfig {
    pub decay_half_life_days: f64,  // default: 138 (~0.995/day)
    pub default_threshold: f32,     // default: 0.4
    pub default_limit: usize,       // default: 5
}

pub struct RecallResult {
    pub id: String,
    pub content: String,
    pub score: f32,          // similarity x decay
    pub raw_similarity: f32,
    pub created_at: DateTime<Utc>,
}

pub struct RecallMetadata {
    pub session_key: String,
    pub channel: String,
    pub role: String,
    pub timestamp: DateTime<Utc>,
}
```

Methods: `store_message`, `search`, `delete_older_than`, `count`.

Time-decay formula (moved from `ConversationEmbeddingStore`):
```
decay_factor = 0.5^(1 / half_life_days)
adjusted_score = raw_similarity * decay_factor^days_old
```

Key difference from old system: `store_message` handles embedding internally (callers pass text, not vectors). Cognitive fully controls the embed-to-store pipeline.

## CognitiveMemoryRetriever

In `cognitive/src/memory_retriever.rs`. Implements `context_engine::MemoryRetriever`:

```rust
pub struct CognitiveMemoryRetriever {
    recall: Arc<ConversationRecallService>,
}

impl MemoryRetriever for CognitiveMemoryRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        self.recall.search(query, limit, self.recall.config.default_threshold).await
            .map(|r| r.into_iter().map(to_memory_entry).collect())
    }
}
```

Replaces `ConversationMemoryRetriever` from agent. `ContextEngine` calls `retrieve_memory()` the same way — just gets a cognitive-backed implementation.

## Deletions

### Files removed

| File | Replacement |
|------|-------------|
| `agent/src/memory.rs` (MemoryStore) | ConversationRecallService + cognitive pipeline |
| `agent/src/context_sources/learning.rs` (LearningContextSource) | CognitiveContextSource |
| `agent/src/context_sources/memory.rs` (MemorySource) | Already unused |
| `agent/src/conversation_memory_retriever.rs` | CognitiveMemoryRetriever |
| `storage/src/repos/memory_note.rs` (MemoryNoteRepo) | Cognitive facts replace diary notes |
| `tools/src/conversation_embedding.rs` (ConversationEmbeddingStore) | Logic moves to ConversationRecallService |

### Tables removed

| Table | Rationale |
|-------|-----------|
| `memory_notes` (SQLite) | Replaced by semantic facts + episodic memories |
| `memory_note_embeddings` (LanceDB) | No longer needed |

`conv_embeddings` (LanceDB) stays — owned by cognitive now.

## Tool changes

`ConversationEmbeddingHandler` trait renamed to `ConversationRecallHandler`. Stays in `tools` (L4) as the interface `MemoryTool` uses. Slimmed to trait + types only (store logic removed).

```rust
#[async_trait]
pub trait ConversationRecallHandler: Send + Sync {
    async fn search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<RecallResult>>;
    async fn embed_message(&self, id: &str, content: &str, metadata: RecallMetadata) -> Result<()>;
    async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64>;
    async fn count(&self) -> Result<usize>;
}
```

Implementation in agent (`ConversationRecallHandlerImpl`) delegates to `ConversationRecallService`.

## Builder wiring (AgentLoopBuilder)

```
Before:
  → MemoryStore (EmbeddingEngine + VectorStore + MemoryNoteRepo)
  → ConversationEmbeddingHandlerImpl
  → ConversationMemoryRetriever → ContextEngine
  → LearningContextSource (priority 55)
  → CognitiveContextSource (priority 60)

After:
  → TextEmbedderImpl (wraps EmbeddingEngine)
  → ConversationRecallService (TextEmbedder + VectorStore + config)
  → ConversationRecallHandlerImpl (wraps service) → MemoryTool
  → CognitiveMemoryRetriever (wraps service) → ContextEngine
  → CognitiveContextSource (priority 60, absorbs confidence threshold text)
```

## Confidence threshold instructions

`LearningContextSource` injected confidence threshold text alongside memory. This moves to `CognitiveContextSource` — appended to the formatted prompt when adaptive thresholds are active. Small addition, not a separate context source.

## Data migration

No production data exists. Migration removes the `memory_notes` table. No one-time extraction needed.

## Testing strategy

### Unit tests (cognitive)

- `store_and_search` — store message, search it back, verify score
- `time_decay` — newer messages score higher than older ones
- `delete_older_than` — prune works, count decreases
- `retriever_maps_results` — CognitiveMemoryRetriever maps RecallResult to MemoryEntry
- `empty_query` — returns empty vec

Use mock `TextEmbedder` returning deterministic vectors. No fastembed needed.

### Integration tests (agent)

- `builder_wires_recall_service` — builder creates service when embedding enabled
- `builder_without_embedding` — builder works without embedding engine
- `full_pipeline_embed_and_retrieve` — message → store → retrieve → results appear

Use `StoragePool::connect_in_memory()` + ephemeral LanceDB.

### Existing test updates

All tests referencing `MemoryStore`, `LearningContextSource`, `ConversationMemoryRetriever`, or `ConversationEmbeddingStore` need updating.
