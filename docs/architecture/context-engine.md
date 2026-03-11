# Context Engine

## Overview

The context engine (`crates/context_engine/`) assembles the system prompt, conversation history, memory context, and tool definitions into a token-budgeted payload for the LLM. It sits at layer L3 of the workspace hierarchy and is consumed by the agent runtime during request processing.

The engine's job is to answer: given a context window of N tokens, what combination of system prompt, retrieved memories, compressed history, and tool schemas should we send to the model?

**Key files:**
- `crates/context_engine/src/assembler.rs` -- `ContextEngine`, `AssembledContext`, assembly pipeline
- `crates/context_engine/src/budget.rs` -- `BudgetAllocator`, `Priority` enum, waterfall allocation
- `crates/context_engine/src/source.rs` -- `ContextSource` trait, `SourceContext`
- `crates/context_engine/src/history_compressor.rs` -- Extractive and abstractive compression
- `crates/context_engine/src/ttl_cache.rs` -- `TtlCache` for context source outputs
- `crates/context_engine/src/memory_retriever.rs` -- `MemoryRetriever` trait for embedding-based RAG

## Context Sources

Context sources are pluggable providers of system prompt sections. They implement the `ContextSource` trait:

```rust
#[async_trait]
pub trait ContextSource: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u8;           // Higher = earlier in prompt
    async fn provide(&self, ctx: &SourceContext) -> Option<String>;
    fn estimated_tokens(&self) -> usize; // Default: 500
}
```

Sources are registered via `ContextEngine::with_sources()`, which sorts them by priority (descending). Each source receives a `SourceContext` containing channel name, chat ID, the current message, and optional project scope.

### Example sources (in `crates/agent/src/context_sources/`)

| Source | Priority | TTL | Token Estimate | Description |
|--------|----------|-----|----------------|-------------|
| `AreaSource` | 75 | 60s | 400 | Active areas (PARA areas of responsibility) |
| `TodoSource` | 70 | 60s | 600 | Active tasks summary |
| `ProductivityContextSource` | 55 | 60s (tier 1), 600s (tier 2) | 800 | Focus sessions, quality scores, energy forecasts, patterns |

Each source owns a `TtlCache` for its `provide()` output. On cache miss, the source queries its repository, caches the result, and returns it.

### System prompt assembly

`ContextEngine::build_system_prompt()` calls all registered sources concurrently via `join_all`, collects non-empty results, and joins them with `\n\n---\n\n` separators. This produces the full system prompt before the assembly pipeline allocates token budget.

## Token Budget Allocation

### The 8 priorities

Content is allocated in priority order via a waterfall algorithm. Higher-priority content is guaranteed space first; lower-priority content gets whatever remains.

| Priority | Enum Value | Description |
|----------|------------|-------------|
| 0 | `SystemIdentity` | Core system prompt (agent personality, instructions) |
| 1 | `ActiveTask` | Current task context |
| 2 | `ToolDefinitions` | JSON schemas for available tools |
| 3 | `RecentHistory` | Most recent conversation messages (verbatim) |
| 4 | `RetrievedMemory` | Embedding-based memory retrieval results |
| 5 | `CompressedHistory` | Summaries of older conversation segments |
| 6 | `BootstrapPersona` | User model / persona context |
| 7 | `Skills` | Agent skill definitions |

### Budget split

The total context window is split:
- **85% for input** (system prompt + history + tools + memory)
- **15% reserved for response generation**

For a 128K token model: 108,800 tokens for input, 19,200 reserved for response.

### Waterfall algorithm

1. Create a `BudgetAllocator` with `BudgetConfig::standard(context_window)`
2. Allocate system prompt tokens at `SystemIdentity` priority
3. Allocate tool definition tokens at `ToolDefinitions` (zero for `DirectResponse` and `Clarification` strategies)
4. Allocate retrieved memory tokens at `RetrievedMemory` (skipped for `Clarification` strategy)
5. The remaining budget goes to history -- compressed history and recent messages share this pool
6. If budget drops below 15% of available input after any allocation, a warning is emitted

Each `allocate()` call is capped at the remaining budget. Content is never allocated beyond what is available.

## History Compression

When conversation history exceeds the remaining token budget, the history compressor reduces it to fit. Two modes are supported:

- **Extractive** (default): Keeps the most recent N messages verbatim and drops older ones entirely. No LLM call needed.
- **Abstractive**: Chunks older messages into segments and summarizes each via a `SummaryProvider` (LLM call). Recent messages are still kept verbatim.

The compressor is configured via `CompressorConfig`:
- `mode`: `Extractive` or `Abstractive`
- `min_recent_messages`: Number of most recent messages always kept verbatim
- `chunk_size`: Number of messages per summary chunk (abstractive mode)

Post-compression enforcement: if even the recent messages exceed the budget (e.g., very long tool results), the assembler truncates from the oldest recent message until the budget is satisfied.

## Caching

### Assembly cache (SHA-256 keyed LRU)

The `ContextEngine` maintains a bounded LRU cache of `AssembledContext` results with a default capacity of 8 entries.

**Cache key computation** uses SHA-256 over:
- System prompt text
- History length + last message content
- User message text
- Strategy discriminant (1 byte)
- Tool definition count + first tool name
- Context window size

This produces a deterministic 64-character hex string. Cache entries naturally expire when any input changes (e.g., tool execution appends to history, changing the key). No explicit invalidation is needed.

**Eviction**: When the cache reaches capacity, the oldest entry (front of a `VecDeque`) is evicted.

### TtlCache for context sources

`TtlCache` is a single-value cache with time-based expiration, designed for context source outputs. Each source (e.g., `TodoSource`, `AreaSource`) creates its own `TtlCache` instance with a configured TTL (typically 60 seconds).

```rust
pub struct TtlCache {
    inner: Mutex<Option<CachedEntry>>,
    ttl_secs: i64,
}
```

- Uses `std::sync::Mutex` (not tokio) since the critical section is CPU-only (timestamp comparison + string clone)
- `get()` returns `Some(content)` if the entry exists and has not expired, otherwise `None`
- `set()` stores a value and resets the expiration timer
- Poison-resistant: uses `unwrap_or_else(|e| e.into_inner())` to recover from panicked lock holders

## Context Assembly Flow

The `ContextEngine::assemble()` method orchestrates the full pipeline:

1. **Cache check** -- Compute SHA-256 cache key from request inputs. Return cached result on hit.

2. **System prompt allocation** -- Estimate token count for the system prompt and allocate at `SystemIdentity` priority.

3. **Tool definitions allocation** -- For `ToolAssisted` and `AutonomousTask` strategies, estimate and allocate tool schema tokens. `DirectResponse` and `Clarification` strategies allocate zero.

4. **Memory retrieval** -- For all strategies except `Clarification`, query the `MemoryRetriever` with the user's message. Format results as a `[Relevant Context]` block and allocate at `RetrievedMemory` priority. `DirectResponse` mode includes memory retrieval for personalized answers.

5. **History compression** -- Pass conversation history and remaining budget to the `HistoryCompressor`. It returns recent messages (verbatim) and optional summaries (compressed older segments). If recent messages alone exceed the budget, truncate from the oldest.

6. **Message assembly** -- Build the final message list in order:
   - System message (prompt)
   - Retrieved memory (if any, as system message)
   - Compressed summaries (if any, as system messages)
   - Recent conversation messages (verbatim)

7. **Inventory injection** -- Build a `ContextInventory` from registered sources. If any sources are deferred (not loaded due to budget), inject an inventory summary into the prompt so the agent knows what additional context is available via `expand()`.

8. **Cache store** -- Insert the assembled result into the LRU cache.

### Expanding deferred context

After initial assembly, the agent can load deferred context sources via `ContextEngine::expand()`:

1. Look up the named source in registered sources
2. Call `provide()` to get the content
3. Estimate token cost -- reject if it exceeds `budget_remaining`
4. Clone the current context, append the new system message, update token counts and inventory
5. Increment the context version

This allows the agent to progressively load more context within the same conversation turn as needed.
