# Learning and Memory Systems

This document covers two interconnected subsystems: the **Learning System** (adaptive confidence thresholds, outcome tracking, strategy analysis) and the **Memory System** (persistent memory notes, conversation embeddings, semantic retrieval). Both rely on a shared **Embedding System** (fastembed + LanceDB) for vector operations.

---

## Section 1: Narrative Overview

### Learning System Design

The learning system is a closed-loop feedback mechanism. Every tool execution is recorded as an `OutcomeRecord`, which feeds a periodic analyzer. The analyzer computes per-tool success rates bucketed by confidence score ranges, then suggests an optimal confidence threshold. An adaptive threshold engine smoothly adjusts the live threshold used by the `ConfidenceEvaluator`, completing the feedback loop.

**Privacy-by-omission**: `OutcomeRecord` intentionally omits tool arguments and user messages. Only the tool name, success/failure, duration, confidence score, and a hashed session key are persisted. Session keys are pseudonymized with FNV-1a hashing while preserving the channel prefix for analytics (e.g., `"telegram:abc123"` becomes `"telegram:a1b2c3d4"`).

**Key files:**
- `crates/agent/src/learning/mod.rs` (lines 1-23) -- module root and re-exports
- `crates/agent/src/learning/types.rs` (lines 1-112) -- all domain types

#### Adaptive Strategies

`AdaptiveThresholds` manages the confidence threshold lifecycle. It loads persisted state from SQLite (`learning_state` table, key `"adaptive_thresholds"`), applies analysis results with step-limited changes (max +/-0.05 per cycle), and saves updated state back to SQL.

Safeguards:
- **Cold-start protection**: No adaptation occurs until `min_outcomes` records exist (default: 50).
- **Step limiting**: Maximum threshold change per cycle is 0.05, preventing wild swings.
- **Bound clamping**: Threshold always stays within configurable `[min_threshold, max_threshold]`.
- **History tracking**: Every change is logged as a `ThresholdChange` with from/to values and reason.

**Key file:** `crates/agent/src/learning/adaptive.rs` (lines 1-173)

#### Tool Confidence

`ToolConfidenceMap` provides per-tool threshold overrides on top of the global threshold. High-risk tools like `shell` can require higher confidence (e.g., 0.9) while safe tools like `web_search` can accept lower confidence (e.g., 0.5). When no tool-specific threshold is set, the global threshold applies.

**Key file:** `crates/agent/src/learning/tool_confidence.rs` (lines 1-49)

#### Outcome Tracking

The `OutcomeRecorder` is the entry point for recording tool execution outcomes. It sits in the agent loop and captures:
- Tool name, success/failure, duration
- Error category (timeout, permission, not_found, validation, network, unknown)
- Confidence score and dimension breakdown from the pre-tool assessment
- Execution mode (Chat or PlanStep)

The recorder also implements `feature_todo::EnrichmentFeedbackHandler`, capturing enrichment accept/override signals as the sole explicit user feedback path. Both enrichment feedback and tool outcomes feed into `LearningAnalyzer`.

**Key file:** `crates/agent/src/learning/recorder.rs` (lines 1-123)

#### Analysis Pipeline

`LearningAnalyzer` is a stateless computation engine. Given all outcomes and enrichment feedback, it produces:
1. **Per-tool stats**: total calls, success count, avg duration, success rates bucketed into 5 confidence bands: [0.0-0.3), [0.3-0.5), [0.5-0.7), [0.7-0.85), [0.85-1.0).
2. **Suggested threshold**: The lowest confidence band where success rate >= 80% (with at least 5 data points). Defaults to 0.7.
3. **Threshold confidence**: Based on data volume -- 0.9 for 100+ outcomes, 0.7 for 50+, 0.5 for 20+, 0.2 otherwise.
4. **Enrichment stats**: Acceptance/override rates per field and overall.

**Key file:** `crates/agent/src/learning/analyzer.rs` (lines 1-186)

#### Strategy Tracking

`StrategyRecord` tracks the accuracy of the intent classification system. Each record captures the predicted strategy (e.g., "DirectResponse"), the actually used strategy (which may differ after escalation), escalation count, iterations used, and response time. `compute_stats()` aggregates these into accuracy, average escalations, and average iterations per strategy type.

**Key file:** `crates/agent/src/learning/strategy_tracker.rs` (lines 1-71)

#### Outcome Storage

`OutcomeStore` provides a dual-backend abstraction: `Backend::Sqlite` for production (via `storage::OutcomeRepo`) and `Backend::InMemory` for tests. Both backends support recording outcomes, recording enrichment feedback, querying outcomes by date range, and listing all data.

**Key file:** `crates/agent/src/learning/outcome_store.rs` (lines 1-176)

### How the Agent Learns Over Time

The learning loop operates as follows:

1. **Record**: Every tool call in the agent loop is timed. On completion, `OutcomeRecorder::record_tool_outcome()` stores the result.
2. **Analyze**: `LearningService` runs periodically (configurable interval) or on-demand via `trigger_analysis()`. It reads all outcomes, runs `LearningAnalyzer::analyze()`, and applies results through `AdaptiveThresholds`.
3. **Adapt**: If the analysis suggests a different threshold and enough data exists, the threshold is adjusted (step-limited). The new value is written atomically to the `Arc<AtomicU32>` shared with `ConfidenceEvaluator`.
4. **Decide**: On the next user message, `ConfidenceEvaluator` uses the updated threshold (lock-free read) to decide whether to proceed with tool calls or ask for clarification.
5. **Broadcast**: `LearningService` publishes `LearningEvent::AnalysisCompleted` and optionally `LearningEvent::ThresholdChanged` via the `LearningEventBus` for downstream consumers.

### MemoryStore -- Persistent Memory Notes

`MemoryStore` manages two categories of memory backed by SQLite (`MemoryNoteRepo`):

- **Daily notes**: Keyed by date string (`"YYYY-MM-DD"`). Append-only within a day. Used for transient context like "user mentioned they have a meeting at 3pm".
- **Long-term memory**: A single document keyed by `LONG_TERM_KEY`. Overwritten entirely on update. Used for persistent facts like "user prefers dark mode" or "user's timezone is PST".

When embedding support is configured, `MemoryStore` also performs fire-and-forget embedding of notes into LanceDB (`"memory_note_embeddings"` table) for semantic retrieval. The `get_relevant_memory()` method embeds the query, searches LanceDB for similar notes, and falls back to `get_memory_context()` (dump everything) if embeddings are unavailable or no matches exceed the threshold.

**Key file:** `crates/agent/src/memory.rs` (lines 1-227)

### MemoryTool -- User Interaction via Chat

The `MemoryTool` (tool name: `"memory"`) exposes semantic search over conversation history and todos to the LLM. It supports four actions:

- **`search_conversations`**: Semantic search over past conversations using cosine similarity. Returns matching messages with role, preview, similarity score, and session key.
- **`search_all`**: Unified search across todos and conversations using Reciprocal Rank Fusion (RRF) to merge keyword and semantic results from both sources.
- **`purge`**: Delete conversation embeddings by filter (all, by session key, or before a date). Requires explicit filter parameter to prevent accidental data loss.
- **`status`**: Show conversation embedding store statistics (total embeddings, availability, indexed channels, date range).

The tool uses builder-pattern injection for its dependencies: `ConversationEmbeddingHandler`, `TodoRepo`, `EmbeddingHandler`, `VectorStore`, and configurable threshold/RRF parameters.

**Key file:** `crates/tools/src/memory_tool.rs` (lines 1-513)

### ConversationMemoryRetriever -- Automatic Contextual Recall

`ConversationMemoryRetriever` implements the `MemoryRetriever` trait from the `context_engine` crate. It is injected into `ContextEngine` at startup and called automatically on every incoming message during context assembly.

The retrieval pipeline:
1. Embed the user's query using `EmbeddingEngine::embed_async()` (CPU-bound work offloaded to `spawn_blocking`).
2. Search LanceDB via `ConversationEmbeddingStore::search_similar()` -- cross-channel (not scoped to current session).
3. Apply time-decay scoring: `score = similarity * decay_factor^days_old`. Default decay factor is 0.995 (half-life ~138 days).
4. Return results as `Vec<MemoryEntry>` with id, content, and score.

Unlike the `MemoryTool` (which is explicitly invoked by the LLM), this retriever works transparently in the background to inject relevant past conversations into every context window.

**Key file:** `crates/agent/src/conversation_memory_retriever.rs` (lines 1-83)

### MemoryMaintenanceService -- Cleanup

`MemoryMaintenanceService` is a background tokio task that periodically prunes old conversation embeddings from LanceDB. It uses the standard `CancellationToken` + `tokio::select!` pattern for graceful shutdown.

Behavior:
- Runs at a configurable interval (specified in hours).
- Skips the first tick to avoid running immediately on startup.
- Builds a date predicate (`created_at < 'cutoff'`) and calls `VectorStore::delete_where()`.
- Logs the count of deleted embeddings when pruning actually removes records.

**Key file:** `crates/agent/src/memory_maintenance_service.rs` (lines 1-92)

### Embedding System

The embedding system provides local vector generation and persistence. No external API calls are made -- all embedding happens on-device using fastembed.

#### EmbeddingEngine

`EmbeddingEngine` wraps `fastembed::TextEmbedding` with lazy initialization. The model (`paraphrase-multilingual-MiniLM-L12-v2`, 384 dimensions, ~420MB) is downloaded on first use, not at construction. When compiled without the `semantic-search` feature, all embed methods return errors while the struct remains available for API compatibility.

Key capabilities:
- **Single embed**: `embed(&str) -> Result<Vec<f32>>` -- synchronous, holds model lock.
- **Batch embed**: `embed_batch(&[&str]) -> Result<Vec<Vec<f32>>>` -- more efficient for multiple texts.
- **Async embed**: `embed_async(Arc<Self>, String) -> Result<Vec<f32>>` -- runs on `spawn_blocking` to avoid blocking the tokio runtime. Callers use `engine.clone().embed_async(text).await`.
- **Cosine similarity**: Static method `cosine_similarity(&[f32], &[f32]) -> f64` with NaN handling and zero-norm safety.

**Key file:** `crates/tools/src/embedding_engine.rs` (lines 1-296)

#### EmbeddingStore (In-Memory Cache)

`EmbeddingStore` is a lightweight `HashMap<String, EmbeddingRecord>` cache. It supports upsert, delete, get, get_all, and `ids_missing_embeddings()` for identifying todos that need embedding. Persistence is handled by `storage::VectorStore` (LanceDB), not this store.

**Key file:** `crates/tools/src/embedding_store.rs` (lines 1-73)

#### TodoEmbeddingHandler

`TodoEmbeddingHandlerImpl` implements `feature_todo::EmbeddingHandler` (defined in the `feature-todo` crate at Layer 2.5). It composes searchable text as `"{title} {description} {tags}"`, generates an embedding via the shared `EmbeddingEngine`, and persists it to LanceDB's `"todo_embeddings"` table with the model name as metadata.

This follows the dependency inversion pattern: the trait is defined in `feature-todo` (Layer 2.5), the implementation lives in `agent` (Layer 5), and is injected as `Arc<dyn EmbeddingHandler>` at construction.

**Key file:** `crates/agent/src/todo_embedding_handler.rs` (lines 1-67)

#### ConversationEmbeddingHandler

`ConversationEmbeddingHandlerImpl` implements the `ConversationEmbeddingHandler` trait (defined in the `tools` crate at Layer 3). It:
- Composes text with a role prefix (`"User: hello"` or `"Assistant: response"`).
- Generates embeddings and validates dimension (must be 384).
- Creates a `ConversationEmbeddingRecord` with preview (first 100 chars), full content, session key, and timestamps.
- Stores in LanceDB's `"conv_embeddings"` table via `ConversationEmbeddingStore`.
- All operations are best-effort: errors are logged but never propagated.
- Explicit searches (via `search()`) use `decay_factor=1.0` (no time decay) for unbiased results.

The `ConversationEmbeddingStore` wraps `storage::VectorStore` and handles upsert (delete-then-insert), similarity search, purge (with filter predicates), and status queries.

**Key files:**
- `crates/agent/src/conversation_embedding_handler.rs` (lines 1-115)
- `crates/tools/src/conversation_embedding.rs` (lines 1-201)

### Confidence System

The confidence system evaluates whether the LLM is confident enough in its understanding of user intent before executing tool calls. When confidence is low, it auto-triggers clarification instead of proceeding.

#### ConfidenceAssessment

A `ConfidenceAssessment` captures:
- **Composite score** (0.0-1.0): Overall confidence.
- **Phase**: `PreTool` or `PostTool`.
- **Reasoning**: LLM's explanation for the score.
- **Dimensions**: Three sub-scores -- `intent_clarity`, `tool_fit`, `info_sufficiency`.
- **Action**: `Proceed`, `Clarify { questions }`, or `Skip { reason }`.

The LLM emits confidence data in `<confidence>` XML tags within its text response. The evaluator parses and strips these blocks before the response reaches the user.

**Key file:** `crates/agent/src/confidence/types.rs` (lines 1-77)

#### ConfidenceEvaluator

The evaluator stores its threshold as `Arc<AtomicU32>` (f32 bits stored as u32) for lock-free reads on the hot path. The `LearningService` updates it atomically via `threshold_handle()`.

Decision logic:
- If `score >= threshold`: `DecisionAction::Proceed`.
- Otherwise: `DecisionAction::Clarify` with questions generated from whichever dimensions scored below threshold.

Per-tool thresholds: `decide_for_tool()` checks the `ToolConfidenceMap` for a tool-specific threshold before falling back to the global threshold.

The system prompt is dynamically generated by `confidence_prompt(threshold)` so the LLM's instructions stay in sync with the current adaptive threshold.

**Key files:**
- `crates/agent/src/confidence/evaluator.rs` (lines 1-179)
- `crates/agent/src/confidence/prompt.rs` (lines 1-25)

#### DecisionLogger

`DecisionLogger` persists `DecisionLogEntry` records to SQLite via `storage::DecisionLogRepo`. Each entry captures session key, iteration, tool names, user message preview, the full `ConfidenceAssessment`, and optional outcome. Logging is best-effort (errors are warned, not propagated).

**Key file:** `crates/agent/src/confidence/log.rs` (lines 1-73)

### LearningTool -- User Interaction via Chat

The `LearningTool` (tool name: `"learning"`) exposes learning system insights to the LLM. It follows the dependency inversion pattern: `LearningHandler` trait defined in `tools` (Layer 3), implemented by `LearningHandlerImpl` in `agent` (Layer 5).

Three actions:
- **`status`**: Returns current threshold, per-tool success rates, strategy accuracy, average response time, and suggested threshold.
- **`analyze`**: Triggers immediate analysis and returns fresh results.
- **`history`**: Returns the last N adaptive threshold changes with from/to values, reasons, and timestamps.

**Key file:** `crates/tools/src/learning_tool.rs` (lines 1-138)

### Learning Events via the Bus

`LearningEventBus` (in the `bus` crate) is a `tokio::sync::broadcast` wrapper. After each analysis cycle, `LearningService` publishes:

- **`AnalysisCompleted`**: Always published. Contains `total_outcomes` and `suggested_threshold`.
- **`ThresholdChanged`**: Published only when the threshold actually changes. Contains `old_threshold`, `new_threshold`, and `reason`.

Subscribers (AgentLoop, dashboards) call `bus.subscribe()` to get independent receivers. The bus has a default capacity of 16 events.

**Key file:** `crates/bus/src/learning_events.rs` (lines 1-59)

---

## Section 2: API Reference

### LearningService

**File:** `crates/agent/src/learning/service.rs` (lines 23-188)

```rust
pub struct LearningService { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(outcome_store: Arc<RwLock<OutcomeStore>>, adaptive: Arc<RwLock<AdaptiveThresholds>>, confidence_threshold: Option<Arc<AtomicU32>>, check_interval: StdDuration) -> Self` | Construct with dependencies. |
| `with_event_bus` | `fn with_event_bus(self, bus: Arc<bus::LearningEventBus>) -> Self` | Attach event bus for publishing events. |
| `start` | `fn start(&mut self)` | Spawn the background analysis loop. |
| `stop` | `async fn stop(&mut self)` | Cancel the background task and await completion. |
| `trigger_analysis` | `fn trigger_analysis(&self)` | Wake the background loop for immediate analysis. |
| `analyze_now` | `async fn analyze_now(&self) -> Result<()>` | Run analysis synchronously (for CLI use). |

### MemoryStore

**File:** `crates/agent/src/memory.rs` (lines 12-227)

```rust
pub struct MemoryStore { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(repo: storage::MemoryNoteRepo) -> Self` | SQL-backed store without embedding support. |
| `with_embeddings` | `fn with_embeddings(repo, embedding_store, embedding_engine, similarity_threshold) -> Self` | Store with LanceDB embedding-based relevance filtering. |
| `get_relevant_memory` | `async fn get_relevant_memory(&self, query: &str, limit: usize) -> String` | Retrieve memory filtered by semantic relevance. Falls back to `get_memory_context()`. |
| `get_memory_context` | `async fn get_memory_context(&self) -> String` | Get all memory (long-term + today's notes) for system prompt. |
| `read_today` | `async fn read_today(&self) -> Result<String>` | Read today's daily note. |
| `append_today` | `async fn append_today(&self, content: &str) -> Result<()>` | Append to today's daily note. Fire-and-forget embeds the note. |
| `read_long_term` | `async fn read_long_term(&self) -> Result<String>` | Read the long-term memory document. |
| `write_long_term` | `async fn write_long_term(&self, content: &str) -> Result<()>` | Overwrite the long-term memory document. Fire-and-forget embeds it. |
| `get_recent_memories` | `async fn get_recent_memories(&self, limit: usize) -> Result<Vec<(String, String)>>` | Get the N most recent memory entries (excludes long-term). Returns `(key, content)` pairs. |
| `list_memory_files` | `async fn list_memory_files(&self) -> Result<Vec<String>>` | List all memory note keys. |

### MemoryTool

**File:** `crates/tools/src/memory_tool.rs` (lines 20-513)

Tool name: `"memory"`

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `search_conversations` | `query` | `limit` (default 10), `threshold` (default 0.5) | Semantic search over past conversations. |
| `search_all` | `query` | `limit` (default 10), `threshold` (default 0.5) | Unified search across todos and conversations via RRF. |
| `purge` | `filter` (`all`, `session`, `before_date`) | `session_key` (for session filter), `before_date` (ISO 8601, for before_date filter) | Delete conversation embeddings. |
| `status` | none | none | Show conversation embedding store statistics. |

Builder methods: `with_conversation_handler()`, `with_threshold()`, `with_rrf_k()`, `with_todo_repo()`, `with_todo_embedding_handler()`, `with_embedding_store()`.

### LearningTool

**File:** `crates/tools/src/learning_tool.rs` (lines 64-138)

Tool name: `"learning"`

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `status` | none | none | Current learning data (threshold, per-tool stats, strategy accuracy). |
| `analyze` | none | none | Trigger fresh analysis and return results. |
| `history` | none | `limit` (default 10) | Last N adaptive threshold change records. |

### LearningHandler Trait

**File:** `crates/tools/src/learning_tool.rs` (lines 51-61)

```rust
#[async_trait]
pub trait LearningHandler: Send + Sync {
    async fn get_status(&self) -> Result<Option<LearningStatus>>;
    async fn analyze_now(&self) -> Result<LearningStatus>;
    async fn get_threshold_history(&self, limit: usize) -> Result<Vec<ThresholdEntry>>;
}
```

### ConversationMemoryRetriever

**File:** `crates/agent/src/conversation_memory_retriever.rs` (lines 20-83)

```rust
pub struct ConversationMemoryRetriever { /* ... */ }
```

Implements `context_engine::memory_retriever::MemoryRetriever`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(engine: Arc<EmbeddingEngine>, store: ConversationEmbeddingStore, threshold: f64, decay_factor: f64) -> Self` | Create retriever. `decay_factor` is per-day score multiplier (e.g., 0.995). |
| `retrieve` | `async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry>` | Embed query, search LanceDB with time-decay, return `MemoryEntry` list. |

### MemoryMaintenanceService

**File:** `crates/agent/src/memory_maintenance_service.rs` (lines 13-92)

```rust
pub struct MemoryMaintenanceService { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(store: VectorStore, max_age_days: u32, maintenance_interval_hours: u32, token: CancellationToken) -> Self` | Construct with LanceDB store, age limit, interval, and shutdown token. |
| `spawn` | `fn spawn(self)` | Consume self and spawn as a background tokio task. |

### EmbeddingEngine

**File:** `crates/tools/src/embedding_engine.rs` (lines 26-197)

```rust
pub struct EmbeddingEngine { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Create engine. Model NOT loaded until first embed call. |
| `embed` | `fn embed(&self, text: &str) -> Result<Vec<f32>>` | Generate 384-dim embedding. Lazy-loads model on first call. |
| `embed_batch` | `fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>` | Batch embed multiple texts (more efficient). |
| `embed_async` | `async fn embed_async(self: Arc<Self>, text: String) -> Result<Vec<f32>>` | Async wrapper using `spawn_blocking`. |
| `is_available` | `fn is_available(&self) -> bool` | Whether the model is loaded. |
| `model_name` | `fn model_name(&self) -> &str` | Returns `"paraphrase-multilingual-MiniLM-L12-v2"`. |
| `cosine_similarity` | `fn cosine_similarity(a: &[f32], b: &[f32]) -> f64` | Static. NaN-safe, zero-norm-safe cosine similarity. |

**Constant:** `EMBEDDING_DIM: usize = 384` (line 19)

**Feature gating:** When compiled without `semantic-search`, `embed()`, `embed_batch()`, and `embed_async()` return errors. `is_available()` returns false.

### EmbeddingHandler Trait (tools crate)

**File:** `crates/tools/src/embedding_engine.rs` (lines 203-213)

```rust
#[async_trait]
pub trait EmbeddingHandler: Send + Sync {
    async fn embed_todo(&self, todo: &Todo) -> Result<Option<EmbeddingRecord>>;
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
    fn is_available(&self) -> bool;
}
```

Production implementation: `EmbeddingEngineImpl` (lines 219-296).

### EmbeddingHandler Trait (feature-todo crate)

**File:** `crates/feature-todo/src/embedding.rs` (lines 14-21)

```rust
#[async_trait]
pub trait EmbeddingHandler: Send + Sync {
    async fn embed_todo(&self, todo: &Todo) -> Result<()>;
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
}
```

Production implementation: `TodoEmbeddingHandlerImpl` in `crates/agent/src/todo_embedding_handler.rs`.

### EmbeddingStore

**File:** `crates/tools/src/embedding_store.rs` (lines 25-66)

```rust
pub struct EmbeddingStore { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Create empty in-memory store. |
| `upsert` | `async fn upsert(&mut self, record: EmbeddingRecord) -> Result<()>` | Insert or update by ID. |
| `delete` | `async fn delete(&mut self, id: &str) -> Result<()>` | Remove by ID. |
| `get` | `async fn get(&self, id: &str) -> Result<Option<&EmbeddingRecord>>` | Lookup by ID. |
| `get_all` | `async fn get_all(&self) -> Result<&HashMap<String, EmbeddingRecord>>` | Return the entire index. |
| `ids_missing_embeddings` | `fn ids_missing_embeddings(&self, todo_ids: &[String]) -> Vec<String>` | Find IDs not in the index. |

#### EmbeddingRecord

**File:** `crates/tools/src/embedding_store.rs` (lines 14-20)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Record identifier (e.g., todo ID). |
| `embedding` | `Vec<f32>` | 384-dimensional vector. |
| `model` | `String` | Model name used for generation. |
| `embedded_at` | `DateTime<Utc>` | Timestamp of embedding creation. |

### ConversationEmbeddingHandler Trait

**File:** `crates/tools/src/conversation_embedding.rs` (lines 174-201)

```rust
#[async_trait]
pub trait ConversationEmbeddingHandler: Send + Sync {
    async fn embed_message(&self, session_key: &str, role: &str, content: &str, message_id: &str) -> Result<()>;
    async fn search(&self, query: &str, limit: usize, threshold: f64) -> Result<Vec<(ConversationEmbeddingRecord, f64)>>;
    async fn purge(&self, filter: PurgeFilter) -> Result<usize>;
    async fn status(&self) -> Result<ConversationEmbeddingStatus>;
    fn is_available(&self) -> bool;
}
```

Production implementation: `ConversationEmbeddingHandlerImpl` in `crates/agent/src/conversation_embedding_handler.rs`.

### ConversationEmbeddingStore

**File:** `crates/tools/src/conversation_embedding.rs` (lines 53-169)

```rust
pub struct ConversationEmbeddingStore { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(store: storage::VectorStore) -> Self` | Create from a `VectorStore`. |
| `upsert` | `async fn upsert(&self, record: ConversationEmbeddingRecord) -> Result<()>` | Delete-then-insert semantics. |
| `search_similar` | `async fn search_similar(&self, query_embedding: &[f32], limit: usize, threshold: f64, _decay_factor: f64) -> Result<Vec<(ConversationEmbeddingRecord, f64)>>` | LanceDB ANN cosine similarity search. |
| `status` | `async fn status(&self) -> Result<ConversationEmbeddingStatus>` | Count and metadata. |
| `purge` | `async fn purge(&self, filter: PurgeFilter) -> Result<usize>` | Delete by filter predicate. Returns deleted count. |

#### ConversationEmbeddingRecord

**File:** `crates/tools/src/conversation_embedding.rs` (lines 17-26)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Message UUID. |
| `session_key` | `String` | `"channel:chat_id"` format. |
| `role` | `String` | `"user"` or `"assistant"`. |
| `content_preview` | `String` | First 100 characters. |
| `content_full` | `String` | Full message content. |
| `embedding` | `Vec<f32>` | 384-dimensional vector. |
| `model` | `String` | Model name. |
| `embedded_at` | `DateTime<Utc>` | Embedding timestamp. |

#### PurgeFilter

**File:** `crates/tools/src/conversation_embedding.rs` (lines 29-37)

| Variant | Description |
|---------|-------------|
| `BySessionKey(String)` | Delete all embeddings for a specific session. |
| `Before(DateTime<Utc>)` | Delete embeddings created before a cutoff date. |
| `All` | Delete all embeddings. |

#### ConversationEmbeddingStatus

**File:** `crates/tools/src/conversation_embedding.rs` (lines 40-47)

| Field | Type | Description |
|-------|------|-------------|
| `total_embeddings` | `usize` | Total stored embeddings. |
| `indexed_channels` | `Vec<String>` | Distinct session keys. |
| `oldest_embedding` | `Option<DateTime<Utc>>` | Oldest record timestamp. |
| `newest_embedding` | `Option<DateTime<Utc>>` | Newest record timestamp. |
| `is_available` | `bool` | Whether the store is operational. |

### TodoEmbeddingHandlerImpl

**File:** `crates/agent/src/todo_embedding_handler.rs` (lines 17-67)

```rust
pub struct TodoEmbeddingHandlerImpl { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self` | Construct with shared engine and LanceDB store. |
| `compose_text` | `fn compose_text(todo: &Todo) -> String` | Private. Returns `"{title} {description} {tags}"`. |
| `embed_todo` | `async fn embed_todo(&self, todo: &Todo) -> Result<()>` | Generate and store embedding in `"todo_embeddings"` table. |
| `embed_query` | `async fn embed_query(&self, query: &str) -> Result<Vec<f32>>` | Generate query embedding vector. |

### ConfidenceAssessment

**File:** `crates/agent/src/confidence/types.rs` (lines 8-22)

| Field | Type | Description |
|-------|------|-------------|
| `score` | `f32` | Composite score 0.0-1.0. |
| `phase` | `AssessmentPhase` | `PreTool` or `PostTool`. |
| `reasoning` | `String` | LLM's explanation. |
| `dimensions` | `ConfidenceDimensions` | Sub-score breakdown. |
| `action` | `DecisionAction` | Recommended action (skipped in serialization). |
| `assessed_at` | `DateTime<Utc>` | Timestamp. |

#### ConfidenceDimensions

| Field | Type | Description |
|-------|------|-------------|
| `intent_clarity` | `f32` | How clear the user's intent is (0.0-1.0). |
| `tool_fit` | `f32` | How well the chosen tool matches (0.0-1.0). |
| `info_sufficiency` | `f32` | Whether enough info is available (0.0-1.0). |

#### DecisionAction

| Variant | Description |
|---------|-------------|
| `Proceed` | Execute tool calls (default). |
| `Clarify { questions: Vec<String> }` | Ask user for clarification. |
| `Skip { reason: String }` | Skip the tool call. |

#### DecisionLogEntry

**File:** `crates/agent/src/confidence/types.rs` (lines 56-66)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Entry UUID. |
| `session_key` | `String` | Session identifier. |
| `iteration` | `usize` | Agent loop iteration. |
| `tool_names` | `Vec<String>` | Tools being called. |
| `user_message_preview` | `String` | Truncated user message. |
| `assessment` | `ConfidenceAssessment` | Full assessment. |
| `outcome` | `Option<String>` | Post-execution outcome. |
| `created_at` | `DateTime<Utc>` | Timestamp. |

### ConfidenceEvaluator

**File:** `crates/agent/src/confidence/evaluator.rs` (lines 19-179)

```rust
pub struct ConfidenceEvaluator { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(threshold: f32) -> Self` | Create with global threshold (clamped to 0.0-1.0). |
| `new_with_map` | `fn new_with_map(threshold: f32, tool_map: ToolConfidenceMap) -> Self` | Create with per-tool overrides. |
| `threshold` | `fn threshold(&self) -> f32` | Lock-free read of current threshold. |
| `threshold_handle` | `fn threshold_handle(&self) -> Arc<AtomicU32>` | Get atomic handle for external updates. |
| `parse_assessment` | `fn parse_assessment(&self, content: &str) -> Option<ConfidenceAssessment>` | Parse `<confidence>` block from LLM output. |
| `decide` | `fn decide(&self, assessment: &ConfidenceAssessment) -> DecisionAction` | Decide using global threshold. |
| `decide_for_tool` | `fn decide_for_tool(&self, assessment: &ConfidenceAssessment, tool_name: Option<&str>) -> DecisionAction` | Decide using per-tool or global threshold. |

**Free function:** `strip_confidence_blocks(content: &str) -> String` -- removes all `<confidence>...</confidence>` blocks from LLM output for user display.

### DecisionLogger

**File:** `crates/agent/src/confidence/log.rs` (lines 8-45)

```rust
pub struct DecisionLogger { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(repo: storage::DecisionLogRepo) -> Self` | Create with SQL repository. |
| `log` | `async fn log(&self, entry: &DecisionLogEntry)` | Append entry (best-effort, never propagates errors). |
| `recent` | `async fn recent(&self, limit: usize) -> Vec<DecisionLogEntry>` | Read most recent entries. |

### Learning Types

**File:** `crates/agent/src/learning/types.rs`

#### OutcomeRecord (lines 15-32)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID. |
| `session_key` | `String` | Hashed session key (`"channel:hash"`). |
| `tool_name` | `String` | Tool that was executed. |
| `success` | `bool` | Whether the execution succeeded. |
| `error_category` | `Option<String>` | Categorized error (timeout, permission, etc.). |
| `duration_ms` | `u64` | Execution time. |
| `confidence_score` | `Option<f32>` | Pre-tool confidence score. |
| `confidence_dimensions` | `Option<ConfidenceDimensions>` | Full dimension breakdown. |
| `execution_mode` | `ExecutionMode` | `Chat` or `PlanStep { plan_id, step_index }`. |
| `created_at` | `DateTime<Utc>` | Timestamp. |

#### AnalysisResult (lines 43-52)

| Field | Type | Description |
|-------|------|-------------|
| `computed_at` | `DateTime<Utc>` | When the analysis ran. |
| `total_outcomes` | `usize` | Total records analyzed. |
| `per_tool_stats` | `HashMap<String, ToolStats>` | Stats keyed by tool name. |
| `suggested_threshold` | `f32` | Recommended threshold. |
| `threshold_confidence` | `f32` | Confidence in the suggestion (0.0-1.0). |
| `enrichment_stats` | `EnrichmentStats` | Enrichment acceptance rates. |

#### AdaptiveThresholdState (lines 96-102)

| Field | Type | Description |
|-------|------|-------------|
| `current_threshold` | `f32` | Active threshold value. |
| `last_analysis` | `Option<AnalysisResult>` | Most recent analysis. |
| `threshold_history` | `Vec<ThresholdChange>` | All past changes. |
| `updated_at` | `DateTime<Utc>` | Last update time. |

### OutcomeRecorder

**File:** `crates/agent/src/learning/recorder.rs` (lines 19-101)

```rust
pub struct OutcomeRecorder { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(store: Arc<RwLock<OutcomeStore>>) -> Self` | Construct with shared outcome store. |
| `record_tool_outcome` | `async fn record_tool_outcome(&self, tool_name, success, error_category, duration_ms, confidence, execution_mode, session_key)` | Record a tool outcome (best-effort). |
| `record_feedback` | `async fn record_feedback(&self, feedback: EnrichmentFeedbackEntry) -> Result<()>` | Implements `EnrichmentFeedbackHandler`. |

**Free function:** `categorize_error(error_msg: &str) -> &'static str` -- maps error messages to categories: `"timeout"`, `"permission"`, `"not_found"`, `"validation"`, `"network"`, `"unknown"`.

### OutcomeStore

**File:** `crates/agent/src/learning/outcome_store.rs` (lines 70-176)

```rust
pub struct OutcomeStore { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(repo: storage::OutcomeRepo) -> Self` | SQL-backed store (production). |
| `new_in_memory` | `fn new_in_memory() -> Self` | In-memory store (tests). |
| `record` | `async fn record(&self, outcome: OutcomeRecord) -> Result<()>` | Store a tool outcome. |
| `record_feedback` | `async fn record_feedback(&self, feedback: EnrichmentFeedbackEntry) -> Result<()>` | Store enrichment feedback. |
| `outcomes_since` | `async fn outcomes_since(&self, cutoff: DateTime<Utc>) -> Result<Vec<OutcomeRecord>>` | Query by date range. |
| `get_all_outcomes` | `async fn get_all_outcomes(&self) -> Result<Vec<OutcomeRecord>>` | Get all outcomes. |
| `get_all_feedback` | `async fn get_all_feedback(&self) -> Result<Vec<EnrichmentFeedbackEntry>>` | Get all enrichment feedback. |

### AdaptiveThresholds

**File:** `crates/agent/src/learning/adaptive.rs` (lines 17-173)

```rust
pub struct AdaptiveThresholds { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `async fn new(repo, initial_threshold, min_threshold, max_threshold, min_outcomes) -> Self` | Load from SQL or create fresh state. |
| `new_in_memory` | `fn new_in_memory(initial_threshold, min_threshold, max_threshold, min_outcomes) -> Self` | In-memory (tests). `save()` is a no-op. |
| `current_threshold` | `fn current_threshold(&self) -> f32` | Current threshold value. |
| `state` | `fn state(&self) -> &AdaptiveThresholdState` | Full state for reporting. |
| `apply_analysis` | `fn apply_analysis(&mut self, analysis: &AnalysisResult) -> Option<f32>` | Apply results, return new threshold if changed. |
| `save` | `async fn save(&self) -> Result<()>` | Persist state to SQL (no-op for in-memory). |

**Constant:** `MAX_THRESHOLD_STEP: f32 = 0.05` -- maximum change per adjustment cycle.

### LearningAnalyzer

**File:** `crates/agent/src/learning/analyzer.rs` (lines 16-186)

```rust
pub struct LearningAnalyzer;
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `analyze` | `fn analyze(outcomes: &[OutcomeRecord], feedback: &[EnrichmentFeedbackEntry]) -> AnalysisResult` | Static. Compute stats, suggest threshold, compute enrichment stats. |

### ToolConfidenceMap

**File:** `crates/agent/src/learning/tool_confidence.rs` (lines 8-49)

```rust
pub struct ToolConfidenceMap { /* ... */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(default_threshold: f32) -> Self` | Create with global default. |
| `get_threshold` | `fn get_threshold(&self, tool_name: &str) -> f32` | Get threshold (per-tool or default). |
| `set_threshold` | `fn set_threshold(&mut self, tool_name: &str, threshold: f32)` | Set per-tool override (clamped to 0.0-1.0). |
| `default_threshold` | `fn default_threshold(&self) -> f32` | Get the global default. |
| `overrides_count` | `fn overrides_count(&self) -> usize` | Number of tool-specific overrides. |
| `custom_tools` | `fn custom_tools(&self) -> Vec<&str>` | Tool names with custom thresholds. |

### StrategyRecord and StrategyStats

**File:** `crates/agent/src/learning/strategy_tracker.rs` (lines 7-71)

#### StrategyRecord

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | `DateTime<Utc>` | When the strategy was executed. |
| `request_id` | `String` | Request identifier. |
| `predicted_strategy` | `String` | Strategy predicted by the classifier. |
| `actual_strategy` | `String` | Strategy actually used (may differ after escalation). |
| `escalation_count` | `u32` | Number of escalations. |
| `iterations_used` | `u32` | Tool iterations consumed. |
| `max_iterations` | `u32` | Maximum allowed by the strategy. |
| `success` | `bool` | Whether the request succeeded. |
| `user_satisfaction` | `Option<f32>` | Optional feedback score. |
| `response_time_ms` | `u64` | End-to-end time. |

#### StrategyStats

| Field | Type | Description |
|-------|------|-------------|
| `accuracy` | `f32` | Fraction where predicted == actual. |
| `avg_escalations` | `f32` | Average escalations per request. |
| `avg_iterations` | `f32` | Average iterations per request. |
| `sample_count` | `usize` | Total records analyzed. |

**Free function:** `compute_stats(strategy: &str, records: &[StrategyRecord]) -> StrategyStats`

### LearningEventBus and LearningEvent

**File:** `crates/bus/src/learning_events.rs` (lines 1-59)

#### LearningEvent

| Variant | Fields | Description |
|---------|--------|-------------|
| `ThresholdChanged` | `old_threshold: f32`, `new_threshold: f32`, `reason: String` | Threshold was adjusted. |
| `AnalysisCompleted` | `total_outcomes: usize`, `suggested_threshold: f32` | Analysis cycle finished. |

#### LearningEventBus

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(capacity: usize) -> Self` | Create with broadcast channel capacity. |
| `publish` | `async fn publish(&self, event: LearningEvent)` | Publish to all subscribers (no-op with no subscribers). |
| `subscribe` | `fn subscribe(&self) -> broadcast::Receiver<LearningEvent>` | Get independent event receiver. |

### LearningTool Types

**File:** `crates/tools/src/learning_tool.rs` (lines 17-46)

#### LearningStatus

| Field | Type | Description |
|-------|------|-------------|
| `current_threshold` | `f32` | Active confidence threshold. |
| `total_strategy_records` | `i64` | Total strategy tracking records. |
| `strategy_accuracy` | `f64` | Overall classification accuracy. |
| `avg_response_time_ms` | `i64` | Average end-to-end time. |
| `avg_satisfaction` | `Option<f64>` | Average user satisfaction (if available). |
| `suggested_threshold` | `f32` | Threshold suggested by analysis. |
| `per_tool` | `HashMap<String, ToolSummary>` | Per-tool call/success/duration stats. |

#### ToolSummary

| Field | Type | Description |
|-------|------|-------------|
| `total_calls` | `i64` | Total invocations. |
| `success_count` | `i64` | Successful invocations. |
| `avg_duration_ms` | `i64` | Average execution time. |

#### ThresholdEntry

| Field | Type | Description |
|-------|------|-------------|
| `from` | `f32` | Previous threshold. |
| `to` | `f32` | New threshold. |
| `reason` | `String` | Human-readable reason. |
| `timestamp` | `DateTime<Utc>` | When the change occurred. |
