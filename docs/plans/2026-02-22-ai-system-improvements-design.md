# AI System Improvements Design

**Date:** 2026-02-22
**Goal:** Address all 16 identified weaknesses across 5 domains to bring the AI system from 69/100 to production-grade quality.

## Scope

| # | Domain | Weakness | Severity | Solution |
|---|--------|----------|----------|----------|
| 1 | Memory | content_preview only 100 chars | Critical | Store full content in DB |
| 2 | Memory | No abstractive compression | High | LLM summary with caching |
| 3 | Memory | No memory decay/consolidation | Medium | Background maintenance service |
| 4 | Memory | IVFFlat needs VACUUM; HNSW better | Medium | Migrate to HNSW index |
| 5 | Session | No TTL/expiry | High | Background cleanup service |
| 6 | Session | Global write lock bottleneck | High | Per-session locking via DashMap |
| 7 | Session | History limit hardcoded at 50 | Low | Config-driven |
| 8 | Session | Magic reset string | Low | Typed API method |
| 9 | Token | chars/4 heuristic inaccurate | High | tiktoken-rs BPE tokenizer |
| 10 | Planning | AutonomousTask not connected to plan engine | High | Auto-generate plan on AutonomousTask |
| 11 | Planning | No auto step generation | Medium | LLM step generation on plan create |
| 12 | Agentic | Subagent spawning unbounded | Medium | Semaphore-based concurrency limit |
| 13 | Agentic | Enrichment keyword-only | Medium | LLM-backed enrichment (opt-in) |
| 14 | Agentic | Goal-Plan linkage unidirectional | Low | Goal decompose action |
| 15 | Misc | Hybrid search loads all todos | Medium | ID-based batch fetch |
| 16 | Misc | DefaultHasher collision risk | Low | SHA-256 cache keys |

---

## Domain 1: Memory System

### 1.1 Full Content Storage (#1)

**Problem:** `ConversationEmbeddingRecord.content_preview` stores only 100 chars. The LLM sees truncated memory during recall, severely limiting usefulness.

**Solution:**
- New migration: `ALTER TABLE conversation_embeddings ADD COLUMN content_full TEXT NOT NULL DEFAULT ''`
- `ConversationEmbeddingRecord` gains `content_full: String` field
- `ConversationEmbeddingHandlerImpl::embed_message()` stores the complete message content in `content_full`
- Keep `content_preview` (first 100 chars) for listings and debug output
- `ConversationMemoryRetriever` returns `content_full` in `MemoryEntry.content`
- `ConvEmbeddingRepo::insert()` and `search_similar()` updated to include `content_full`

**Files changed:**
- `crates/storage/migrations/` — new migration
- `crates/storage/src/repos/conv_embedding.rs` — SQL queries
- `crates/tools/src/conversation_embedding.rs` — `ConversationEmbeddingRecord` struct
- `crates/agent/src/conversation_embedding_handler.rs` — store full content
- `crates/agent/src/conversation_memory_retriever.rs` — return full content

### 1.2 Abstractive Compression with Caching (#2)

**Problem:** `HistoryCompressor` only does extractive 200-char snippets. The `Abstractive` enum variant exists but falls back to extractive.

**Solution:**
- New table: `history_summaries(id UUID PK, session_key TEXT, range_start INT, range_end INT, summary_text TEXT, model TEXT, created_at TIMESTAMPTZ)`
- New trait in `context_engine`: `SummaryProvider` with `async fn summarize(messages: &[Message]) -> Result<String>`
- Implementation in `agent`: `LlmSummaryProvider` wrapping `DynProvider`
- `HistoryCompressor::compress()` behavior when `mode == Abstractive`:
  1. Check `history_summaries` cache for matching `(session_key, range_start, range_end)`
  2. Cache hit: use cached `summary_text`
  3. Cache miss: call `SummaryProvider::summarize()`, store result in DB
  4. On LLM failure: graceful fallback to extractive mode
- Cache invalidation: when session compaction deletes messages, delete summaries with overlapping ranges
- `SummaryProvider` injected into `HistoryCompressor` via `ContextEngine` builder

**Config:** `config.context.compression_mode: "abstractive"` (default: `"extractive"`)

### 1.3 Memory Decay and Consolidation (#3)

**Problem:** Conversation embeddings and daily notes accumulate unboundedly with no relevance decay.

**Solution:**
- New `MemoryMaintenanceService` in `agent` crate (background tokio task)
- **Decay in search:** Modify `ConvEmbeddingRepo::search_similar()` to apply time-decay:
  ```sql
  ORDER BY (1.0 - (embedding <=> $1)) * power($decay_factor, EXTRACT(EPOCH FROM now() - created_at) / 86400.0) DESC
  ```
  where `$decay_factor` is configurable (default: 0.995, giving ~50% weight at 138 days)
- **Pruning:** Delete embeddings older than `max_age_days` (default: 90)
- **Daily note consolidation:** Notes older than 30 days merged into weekly summaries via LLM call
- Runs on configurable interval (default: daily at 3 AM via internal timer)

**Config:**
```json
{
  "memory": {
    "decay_half_life_days": 138,
    "max_age_days": 90,
    "consolidation_enabled": true,
    "maintenance_interval_hours": 24
  }
}
```

### 1.4 HNSW Index Upgrade (#4)

**Problem:** IVFFlat requires periodic `VACUUM ANALYZE` after bulk inserts for accuracy. HNSW handles continuous inserts without maintenance.

**Solution:**
- New migration:
  ```sql
  DROP INDEX IF EXISTS idx_todo_embeddings_ann;
  CREATE INDEX idx_todo_embeddings_ann ON todo_embeddings
      USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);

  DROP INDEX IF EXISTS idx_conv_embeddings_ann;
  CREATE INDEX idx_conv_embeddings_ann ON conversation_embeddings
      USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);
  ```
- HNSW params: `m=16` (connections per node), `ef_construction=64` (build-time search width)
- No application code changes needed — queries use the same `<=>` operator

---

## Domain 2: Session Management

### 2.1 Session TTL/Expiry (#5)

**Problem:** Old sessions accumulate in PostgreSQL indefinitely with no cleanup.

**Solution:**
- New `SessionCleanupService` in `agent` crate
- Background task running on configurable interval (default: hourly)
- Deletes sessions where `updated_at < now() - interval '$ttl_days days'`
- Cascade delete handles `session_messages` (FK already exists)
- Also purges orphaned `conversation_embeddings` for deleted session keys
- Logs count of cleaned sessions at `info` level

**Config:**
```json
{
  "session": {
    "ttl_days": 30,
    "cleanup_interval_hours": 1
  }
}
```

**Files:** New `agent/src/session_cleanup_service.rs`, wired in `AgentLoopBuilder::build()`

### 2.2 Per-Session Locking (#6)

**Problem:** Global `Arc<RwLock<SessionManager>>` blocks all concurrent messages.

**Solution:**
- Add `dashmap` dependency to `session` crate
- Replace internal `HashMap<String, Session> + VecDeque<String>` with `DashMap<String, Arc<Mutex<Session>>>`
- `SessionManager` becomes `Clone` (holds `PgPool` + `Arc<DashMap<...>>`)
- `AgentLoop` holds `SessionManager` directly (no more `Arc<RwLock<...>>`)
- Per-session flow:
  1. `get_or_create()` returns `Arc<Mutex<Session>>` — concurrent access to different sessions is lock-free
  2. Caller locks only the specific session it's working on
  3. `save()` operates on the session clone outside the session lock
- LRU eviction: Track access order via `DashMap` metadata or a separate `Mutex<VecDeque>` (only locked during eviction check, not per-message)
- Capacity check runs after each `get_or_create`, evicts oldest if >1000

**Files:**
- `crates/session/src/manager.rs` — major refactor
- `crates/agent/src/agent_loop/mod.rs` — remove `Arc<RwLock<>>` wrapper
- `crates/agent/src/agent_loop/builder.rs` — simplify wiring

### 2.3 Configurable History Limit (#7)

**Problem:** `DEFAULT_HISTORY_LIMIT = 50` hardcoded in `agent_loop/mod.rs:L36`.

**Solution:**
- Add `config.session.history_limit: usize` (default: 50)
- `AgentLoopBuilder` reads from config and passes to `AgentLoop`
- `AgentLoop::process_message()` uses `self.history_limit` instead of the constant

### 2.4 Typed Session Reset API (#8)

**Problem:** Telegram reset uses magic in-band string `"__RESET_SESSION__"` and fake sender_id.

**Solution:**
- Add `SessionManager::reset_session(key: &str) -> Result<()>`:
  - Removes session from cache
  - Calls `SessionRepo::delete_session(key)` (cascade deletes messages)
- Telegram channel handler calls `session_manager.reset_session()` directly on `/reset` command
- Remove the `"__RESET_SESSION__"` check from `AgentLoop::process_message()`
- Other channels can reuse `reset_session()` for their own reset commands

---

## Domain 3: Token Counting

### 3.1 tiktoken-rs Integration (#9)

**Problem:** `CharTokenCounter` uses `text.len() / 4` — inaccurate by 20-40% for non-ASCII or code.

**Solution:**
- Add `tiktoken-rs` dependency to `context_engine` crate
- New `TiktokenCounter` implementing `TokenCounter`:
  ```rust
  pub struct TiktokenCounter {
      bpe: CoreBPE,
  }
  impl TokenCounter for TiktokenCounter {
      fn estimate_text(&self, text: &str) -> usize {
          self.bpe.encode_with_special_tokens(text).len()
      }
  }
  ```
- Uses `cl100k_base` encoding (GPT-4/Claude-compatible)
- `AgentLoopBuilder` constructs `TiktokenCounter` (fallback to `CharTokenCounter` if init fails)
- `HistoryCompressor` and `BudgetAllocator` automatically use the accurate counter via the existing `Arc<dyn TokenCounter>` injection

**Files:**
- `crates/context_engine/Cargo.toml` — add tiktoken-rs
- `crates/context_engine/src/token_counter.rs` — add `TiktokenCounter`
- `crates/agent/src/agent_loop/builder.rs` — wire new counter

---

## Domain 4: Planning Engine

### 4.1 AutonomousTask Auto-Plan Generation (#10)

**Problem:** `AutonomousTask` strategy just runs `ReactPlusEngine` with higher iteration limits — no structured decomposition.

**Solution:**
- New `PlanGenerateEngine` in `agent/src/execution/plan_generate.rs`
- When `EngineDispatch` receives `AutonomousTask`:
  1. Calls LLM with decomposition prompt: "Break this task into 3-8 concrete steps"
  2. Parses JSON response: `[{description, reasoning, expected_tools}]`
  3. Creates `Plan` + `PlanStep` records via `PlanRepo`
  4. Calls `PlanExecutor::run_plan_execution()` (existing)
  5. Returns `DispatchResult` with plan output
- Decomposition prompt includes: user message, conversation history, available tools
- On parse failure: falls back to single-step plan with the original task as the only step
- `EngineDispatch` escalation chain becomes: Direct -> ReactPlus -> PlanGenerate

**Files:**
- New `crates/agent/src/execution/plan_generate.rs`
- `crates/agent/src/execution/dispatch.rs` — replace AutonomousTask handler
- `crates/agent/src/execution/mod.rs` — export new module

### 4.2 Automatic Step Generation (#11)

**Problem:** `PlanTool` creates plans with empty steps. Steps must be added manually.

**Solution:**
- Extract step generation logic from #10 into shared function:
  ```rust
  pub async fn generate_plan_steps(
      provider: &DynProvider,
      model: &str,
      description: &str,
      context: &[Message],
      available_tools: &[String],
  ) -> Result<Vec<PlanStepDraft>>
  ```
- `PlanTool::handle_create()` calls `generate_plan_steps()` after creating the plan
- New `PlanTool` action `"generate-steps"` for regenerating steps on existing plans
- `PlanGenerateEngine` (#10) reuses the same function

**Files:**
- New `crates/agent/src/plan_step_generator.rs` — shared generation logic
- `crates/tools/src/plan.rs` — wire into create action, add generate-steps action
- `crates/agent/src/execution/plan_generate.rs` — reuse shared function

---

## Domain 5: Agentic & Misc

### 5.1 Subagent Concurrency Limiter (#12)

**Problem:** `SubagentManager::spawn()` fires unlimited `tokio::spawn` tasks.

**Solution:**
- Add `semaphore: Arc<Semaphore>` to `SubagentManager`
- `spawn()` acquires permit before spawning, releases on task completion
- Default permits: 3 (configurable via `config.agents.max_concurrent_subagents`)
- Acquire with timeout (30s) — if at capacity, return error to the agent explaining concurrent limit

**Files:**
- `crates/agent/src/subagent.rs` — add semaphore
- `crates/config/src/lib.rs` — add config field

### 5.2 LLM-Backed Enrichment (#13)

**Problem:** `EnrichmentEngine` only uses keyword matching for priority/duration inference.

**Solution:**
- Add `provider: Option<DynProvider>` and `model: String` to `EnrichmentEngine`
- When `config.todo.enrichment.use_llm` is true and provider available:
  1. Call LLM with task title + description + few-shot examples
  2. Parse structured JSON response: `{priority, estimated_minutes, due_date_suggestion}`
  3. Use LLM confidence scores alongside keyword confidence
- Falls back to keyword heuristics when LLM unavailable or fails
- Keyword results serve as validation: if LLM and keyword disagree, use the higher-confidence one

**Config:** `config.todo.enrichment.use_llm: false` (opt-in)

**Files:**
- `crates/agent/src/enrichment/engine.rs` — add LLM path
- `crates/agent/src/agent_loop/builder.rs` — inject provider into enrichment

### 5.3 Goal Decomposition (#14)

**Problem:** Goals and plans live independently. No automatic decomposition.

**Solution:**
- New `GoalTool` action `"decompose"`: calls LLM to generate a plan from goal description
  1. Takes goal ID
  2. Reads goal details
  3. Calls `generate_plan_steps()` (reuse from #11)
  4. Creates a linked plan with `goal_id` FK
- New `GoalTool` action `"status"`: aggregates linked plan statuses + completion %

**Files:**
- `crates/tools/src/goal.rs` — add decompose and status actions
- Reuses `plan_step_generator.rs` from #11

### 5.4 Targeted Hybrid Search (#15)

**Problem:** `handle_search_hybrid()` calls `repo.list(&TodoFilter::default())` loading all todos.

**Solution:**
- New `TodoRepo::get_by_ids(ids: &[String]) -> Result<Vec<TodoRow>>`
  - `SELECT * FROM todos WHERE id = ANY($1)`
- `handle_search_hybrid()` flow:
  1. Run keyword search -> collect IDs
  2. Run semantic search -> collect IDs
  3. Union IDs
  4. `repo.get_by_ids(union_ids)` -> build lookup map
  5. RRF merge as before

**Files:**
- `crates/storage/src/repos/todo.rs` — add `get_by_ids()`
- `crates/tools/src/todo/actions/search.rs` — refactor hybrid search

### 5.5 Stable Context Cache Hashing (#16)

**Problem:** `DefaultHasher` in `compute_cache_key()` can silently collide.

**Solution:**
- Replace `DefaultHasher` with SHA-256 (`sha2` crate, already likely in dependency tree via sqlx)
- Hash function: `SHA256(system_prompt || history_len || last_message || strategy)`
- Returns hex string as cache key
- ~200ns per hash — negligible for once-per-message usage

**Files:**
- `crates/context_engine/src/assembler.rs` — replace `compute_cache_key()`
- `crates/context_engine/Cargo.toml` — add sha2 if not already present

---

## Migration Summary

New migrations needed:
1. `20260222000000_full_content_embeddings.sql` — add `content_full` column
2. `20260222000001_history_summaries.sql` — create `history_summaries` table
3. `20260222000002_hnsw_indexes.sql` — upgrade IVFFlat to HNSW

## New Dependencies

| Crate | Added to | Purpose |
|-------|----------|---------|
| `tiktoken-rs` | context_engine | Accurate BPE token counting |
| `dashmap` | session | Concurrent per-session map |
| `sha2` | context_engine | Stable cache hashing |

## Implementation Order

Recommended implementation sequence (dependency-aware):

1. **Phase 1 — Foundations** (no cross-cutting deps):
   - #16 SHA-256 cache key
   - #7 Configurable history limit
   - #8 Typed session reset
   - #4 HNSW index migration
   - #9 tiktoken-rs tokenizer
   - #15 Targeted hybrid search

2. **Phase 2 — Memory** (builds on Phase 1 tokenizer):
   - #1 Full content storage
   - #12 Subagent semaphore

3. **Phase 3 — Session & Services** (can parallelize with Phase 2):
   - #5 Session TTL cleanup
   - #6 Per-session locking (DashMap)

4. **Phase 4 — LLM-Dependent Features** (needs LLM integration patterns established):
   - #2 Abstractive compression
   - #3 Memory decay & consolidation
   - #13 LLM-backed enrichment

5. **Phase 5 — Planning & Goals** (builds on Phase 4 patterns):
   - #11 Auto step generation (shared function)
   - #10 AutonomousTask plan generation
   - #14 Goal decomposition
