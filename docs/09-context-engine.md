# Context Engine

## Purpose

The `context_engine` crate (Layer 2) manages how conversation context is assembled for LLM calls. It solves the core problem of fitting a potentially unbounded conversation history, system prompt, tool definitions, and retrieved memories into a finite model context window. The crate is provider-agnostic -- it works with any LLM by accepting the context window size as input and producing a token-budgeted message list as output.

## Key Types

### ContextEngine

The top-level orchestrator. Holds a `HistoryCompressor`, an optional `MemoryRetriever`, pluggable `ContextSource` providers, and a bounded cache. Created with builder-style chaining:

```rust
ContextEngine::new()
    .with_token_counter(counter)
    .with_memory_retriever(retriever)
    .with_summary_provider(provider)
    .with_sources(sources)
```

### ContextRequest

Input to the assembly pipeline. Contains the user's message text, full conversation history, system prompt, an `ExecutionStrategy` (which affects budget allocation), tool definitions as JSON schemas, and the model's context window size.

### AssembledContext

Output from assembly. Contains the final ordered message list (system, memories, summaries, recent history), estimated total token count, and a `BudgetReport` showing per-priority allocations.

### ExecutionStrategy

Determines how the agent should process a request, which in turn affects budget allocation:

- **DirectResponse** -- simple question/answer, no tools, no memory retrieval.
- **ToolAssisted** -- may use tools up to N iterations; tool definitions and memories are budgeted.
- **AutonomousTask** -- full multi-step execution; same budgeting as ToolAssisted.
- **Clarification** -- needs more info from user; no tools, no memory retrieval.

### BudgetAllocator and BudgetConfig

`BudgetConfig` defines the total context window and how much to reserve for the response (default: 15%, leaving 85% for input). `BudgetAllocator` tracks token consumption across priority levels using a waterfall model -- higher priority content is allocated first, lower priority content gets whatever remains.

### Priority

Eight levels controlling allocation order (highest to lowest):

1. **SystemIdentity** -- the core system prompt, always allocated first.
2. **ActiveTask** -- current task context.
3. **ToolDefinitions** -- JSON schemas for available tools (skipped for DirectResponse/Clarification).
4. **RecentHistory** -- verbatim recent conversation messages.
5. **RetrievedMemory** -- embedding-based memory entries.
6. **CompressedHistory** -- summaries of older conversation history.
7. **BootstrapPersona** -- persona context.
8. **Skills** -- skill definitions.

### HistoryCompressor and CompressorConfig

Compresses conversation history to fit within a token budget. Configured with:

- **mode** -- `Extractive` (snippet-based, no LLM call) or `Abstractive` (LLM-generated summaries via SummaryProvider).
- **min_recent_messages** -- minimum messages to always keep verbatim (default: 4).
- **chunk_size** -- number of messages per summary chunk (default: 5).
- **snippet_length** -- max characters per extractive snippet (default: 200).

### CompressedHistory and HistorySummary

The compressor output. `CompressedHistory` contains a list of `HistorySummary` entries (covering older messages) plus recent messages kept verbatim. Each summary tracks its message range and token count.

### MemoryRetriever and MemoryEntry

An async trait for embedding-based memory lookup. Implementations live in higher layers (agent, storage) and perform actual vector-database queries. The context engine calls `retrieve(query, limit)` during assembly, and the returned `MemoryEntry` items (each with an id, content string, and similarity score) are injected as a system message under `Priority::RetrievedMemory`.

### SummaryProvider

An async trait for abstractive summarization. Implementations call an LLM to condense a slice of messages into a concise natural-language summary. Used by `HistoryCompressor` in Abstractive mode. Falls back to extractive summarization on error.

### TokenCounter, CharTokenCounter, TiktokenCounter

A sync trait for estimating token counts. Two built-in implementations:

- **CharTokenCounter** -- heuristic (4 characters = 1 token). Fast, always available.
- **TiktokenCounter** -- BPE-based using tiktoken-rs (cl100k_base encoding). Accurate for OpenAI-compatible models. Falls back to CharTokenCounter if initialization fails.

`best_token_counter()` tries TiktokenCounter first, falls back to CharTokenCounter. `default_token_counter()` always returns CharTokenCounter.

### ContextSource and SourceContext

A pluggable trait for system prompt assembly. Each source has a `name()`, a `priority()` (u8, higher = earlier in prompt), and a `provide()` method that returns an optional string section. Sources are sorted by priority descending and their non-empty outputs are joined with `\n\n---\n\n` separators. Implementations live in downstream crates (e.g., agent) -- this follows the same dependency inversion pattern as SpawnHandler and CronHandler.

`SourceContext` carries per-request metadata: channel name, chat ID, and the current user message (for relevance filtering).

## How It Works

### The Full Assembly Flow

```
ContextRequest
    |
    v
[1] Budget Initialization
    BudgetConfig::standard(context_window)
    Available input = context_window * 0.85
    |
    v
[2] System Prompt Allocation
    Count tokens for system prompt
    Allocate under Priority::SystemIdentity
    |
    v
[3] Tool Definitions Allocation (strategy-dependent)
    DirectResponse / Clarification -> skip (0 tokens)
    ToolAssisted / AutonomousTask -> count and allocate under Priority::ToolDefinitions
    |
    v
[4] Memory Retrieval (strategy-dependent)
    DirectResponse / Clarification -> skip
    Other strategies -> call MemoryRetriever.retrieve(message_text, limit)
    Format results as "[Relevant Context]\n- content (relevance: 0.95)\n..."
    Allocate under Priority::RetrievedMemory
    |
    v
[5] History Compression
    Calculate remaining budget after above allocations
    Pass history and budget to HistoryCompressor.compress_async()
        -> Always keeps min_recent_messages verbatim
        -> Expands recent window if budget allows (uses half remaining for extras)
        -> Summarizes older messages in chunks:
            Extractive mode: first 200-char snippet from each message
            Abstractive mode: LLM summary via SummaryProvider (falls back to extractive on error)
    Post-compression enforcement: if recent messages exceed budget, truncate from oldest
    Allocate under Priority::RecentHistory and Priority::CompressedHistory
    |
    v
[6] Message Assembly
    1. System prompt message
    2. Retrieved memory context (if any), as a system message
    3. Summary messages (if any), as system messages
    4. Recent conversation messages verbatim
    |
    v
AssembledContext { messages, token_count, budget_report }
```

### History Compression Strategy

The compressor uses a split-point strategy:

1. Start with the last `min_recent_messages` (default: 4) as the verbatim window.
2. Walk backward through older messages, adding each to the verbatim window if it fits within half the remaining budget. This ensures at least half the budget is available for summaries.
3. Everything before the split point gets summarized in chunks of `chunk_size` messages.
4. For extractive summaries, each message is reduced to a snippet (first sentence or first N characters, preferring sentence boundaries). The snippet function avoids cutting mid-abbreviation (e.g., "Dr.Smith").
5. For abstractive summaries, each chunk is sent to the SummaryProvider. On error, the chunk falls back to extractive.

### Caching

The engine caches assembled contexts using a bounded LRU cache (default capacity: 8 entries). Cache keys are SHA-256 hashes of the request inputs (system prompt, history length + last message, message text, strategy, tool count + first tool name, context window). The cache uses a generation counter -- calling `invalidate_cache()` increments the generation, making all existing entries stale without removing them immediately.

### System Prompt Assembly

The `build_system_prompt()` method queries all registered `ContextSource` providers concurrently using `join_all`, collects non-empty sections, and joins them with separator lines. This is called by the agent layer before constructing a `ContextRequest`, and the resulting string becomes the `system_prompt` field.

## Connections

**Depends on:**
- `providers` (Layer 2) -- for `Message` and `UserContent` types
- `common` (Layer 0) -- for utility functions
- `tiktoken-rs` -- for BPE token counting
- `sha2` -- for cache key hashing
- `futures-util` -- for `join_all` in source assembly
- `async-trait` -- for async traits

**Depended on by:**
- `agent` (Layer 5) -- constructs `ContextEngine`, injects `MemoryRetriever`, `SummaryProvider`, and `ContextSource` implementations, calls `assemble()` before each LLM call
