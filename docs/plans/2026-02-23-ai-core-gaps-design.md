# AI Core Feature Gaps Resolution

Date: 2026-02-23
Status: Approved

## Problem

Seven gaps identified in the AI core through parallel codebase scans:

1. **History windowing is message-count only** — `get_history(50)` ignores token budget, risks context overflow
2. **Abstractive compression is dead code** — `compress_async()` wired but never called from production path
3. **Subagent loop disconnected from ReactPlusEngine** — primitive `while iteration < 15` misses all engine improvements
4. **MemoryStore dumps all notes without relevance filtering** — wastes context budget on irrelevant memories
5. **`history_summaries` table is orphaned** — migration exists, zero Rust code uses it
6. **JSON Schema validator incomplete** — missing `oneOf`, `anyOf`, `pattern`, `additionalProperties`, `minItems/maxItems`
7. **Cost tracker hardcoded with substring matching** — misses most models, charges `gpt-4o-mini` at `gpt-4o` rates

## Design

### Gap 1: Token-Budget History Truncation

**File**: `crates/context_engine/src/assembler.rs`

Replace message-count slicing with token-budget-aware truncation in the assembler:

- Add `truncate_to_budget(messages: &[Message], budget_tokens: usize, counter: &dyn TokenCounter) -> Vec<Message>` in the assembler
- Walk backward from newest message, counting tokens via existing `TokenCounter` trait
- Stop when adding the next message would exceed budget
- `Session::get_history()` stays as-is (data accessor) — intelligence moves to assembler where token budget is known
- Integrates with existing budget waterfall at priority level `RecentHistory(3)`

### Gap 2: Activate Abstractive Compression

**File**: `crates/context_engine/src/assembler.rs` line 334

One-line change: `self.compressor.compress(...)` → `self.compressor.compress_async(...).await`

- `assemble_uncached()` is already async — safe to await
- When `SummaryProvider` is wired, abstractive mode activates automatically
- When no provider is wired, `compress_async` falls back to extractive — no behavior change

### Gap 3: Subagent Uses ReactPlusEngine

**File**: `crates/agent/src/subagent.rs` (lines 262-375)

Replace `run_subagent_task()` manual loop with `ReactPlusEngine`:

- Build `ToolRegistry` with subagent-allowed tools (fs, shell, web)
- Construct `ExecutionCore` with provider and registry
- Construct `ReactPlusEngine` with `max_iterations: 15`, `ReflectionMode::OnFailure`
- Call `engine.execute(messages, tools, params)` instead of manual loop
- Remove ~100 lines of manual loop code
- Gains: duplicate detection, fabrication detection, reflection, scratchpad

### Gap 4: Embedding-Based Memory Relevance Filtering

**Files**: `crates/agent/src/memory.rs`, `crates/agent/src/context_sources/memory.rs`, new migration

Add semantic filtering with query parameter:

- Add `get_relevant_memory(query: &str, limit: usize) -> String` to `MemoryStore`
- Embed query using existing `EmbeddingEngine`
- Add `vector(384)` column to `memory_notes` table (new migration) or new `memory_note_embeddings` table
- pgvector ANN search against query embedding at retrieval time
- Configurable similarity threshold (reuse `semanticThreshold` from todo search config)
- Update `MemorySource` to pass current user message as query
- Fallback: if embeddings unavailable, fall back to current dump-everything behavior

### Gap 5: Delete Orphaned history_summaries Table

**File**: New migration `20260223000001_drop_history_summaries.sql`

- `DROP TABLE IF EXISTS history_summaries;`
- Clean, reversible, no data loss (table was always empty)

### Gap 6: Expand JSON Schema Validator

**File**: `crates/tools-core/src/lib.rs` (lines 125-245)

Add missing keywords to `validate_value()`:

- `oneOf`: validate value matches exactly one subschema
- `anyOf`: validate value matches at least one subschema
- `pattern`: regex match on strings via `regex` crate
- `additionalProperties`: when `false`, reject keys not in `properties`
- `minItems/maxItems`: array length bounds
- Fix: add `minimum/maximum` range checking for `type: "number"` (currently only `integer` has it)

### Gap 7: Expanded Model Pricing Table

**File**: `crates/agent/src/output/cost_tracker.rs`

Replace substring matching with HashMap lookup:

- HashMap mapping exact model IDs to `(input_rate, output_rate, cache_read_rate, cache_write_rate)`
- Coverage: Claude 3.5 Sonnet/Haiku, Claude 4 Opus/Sonnet, GPT-4o/mini/turbo, Gemini 1.5/2.0 Pro/Flash, DeepSeek V3/R1, Qwen, Mistral
- Add cache token cost calculation using `usage.cache_read_tokens` and `usage.cache_write_tokens`
- Fallback: substring matching as degraded path for unknown model IDs
- Unknown models: `(0.0, 0.0)` — no crash

## Approach Summary

All recommended approaches were selected:
- Gap 1: Token-budget truncation in assembler
- Gap 2: Switch to compress_async
- Gap 3: Reuse ReactPlusEngine
- Gap 4: Embedding-based relevance filtering
- Gap 5: Drop migration
- Gap 6: Add most impactful JSON Schema keywords
- Gap 7: Expanded HashMap pricing table
