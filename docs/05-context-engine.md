# Context Engine

The `context_engine` crate (Layer 2) is the token budget allocator and context window assembler for the agent. It decides what fits in an LLM's context window, compresses conversation history when it does not, retrieves relevant memories from vector storage, and assembles the final ordered message list sent to the provider.

**Crate root:** `crates/context_engine/src/lib.rs`
**Dependencies:** `common`, `providers`, `tiktoken-rs`, `sha2`, `async-trait`, `futures-util`, `chrono`

---

## Section 1: Narrative Overview

### What This Crate Does

Every LLM call has a fixed context window (e.g., 128k tokens for Claude). The context engine's job is to pack the most useful information into that window without exceeding it. It does this through a five-stage pipeline:

1. **Budget allocation** -- reserve space for system prompt, tool definitions, memories, and history using a priority-based waterfall.
2. **Memory retrieval** -- query a vector store for memories relevant to the current user message.
3. **History compression** -- shrink older conversation turns into summaries so recent turns fit verbatim.
4. **System prompt assembly** -- combine pluggable context sources (identity, skills, persona) into a single system prompt.
5. **Final assembly** -- produce an ordered `Vec<Message>` with estimated token counts and a budget report.

### Budget Calculation Strategy

The budget uses a **waterfall allocation** model. The total context window is split into two regions:

- **Response reserve** (15% by default): tokens set aside for the model's reply. Never allocated for input.
- **Available input** (85%): the remaining tokens for system prompt, tools, memories, and history.

Within the available input budget, content is allocated by priority order. Each `Priority` level requests tokens, and `BudgetAllocator::allocate()` grants up to the remaining budget. Once a level is allocated, the remaining pool shrinks. Higher-priority content always gets served first.

The eight priority levels, from highest to lowest:

| Priority | Enum Value | Typical Content |
|----------|-----------|-----------------|
| 0 | `SystemIdentity` | System prompt |
| 1 | `ActiveTask` | Current task context |
| 2 | `ToolDefinitions` | JSON schemas for available tools |
| 3 | `RecentHistory` | Recent conversation messages kept verbatim |
| 4 | `RetrievedMemory` | Embedding-based memory entries |
| 5 | `CompressedHistory` | Summaries of older conversation turns |
| 6 | `BootstrapPersona` | Persona bootstrapping content |
| 7 | `Skills` | Loaded skill definitions |

The execution strategy influences allocation: `DirectResponse` and `Clarification` strategies skip tool definitions (0 tokens) and memory retrieval entirely, keeping the budget lean for simple exchanges.

### Token Counting

Token counting is abstracted behind the `TokenCounter` trait (`crates/context_engine/src/token_counter.rs:9`), which is intentionally synchronous to avoid async overhead in tight estimation loops.

Two implementations ship with the crate:

- **`CharTokenCounter`** -- heuristic: `text.len().div_ceil(4)` (4 characters per token). Zero-cost, always available.
- **`TiktokenCounter`** -- BPE tokenizer backed by `tiktoken-rs` using the `cl100k_base` encoding (the same encoding used by GPT-4 and Claude-compatible APIs). Provides accurate counts. Falls back to `CharTokenCounter` if BPE initialization fails.

The factory function `best_token_counter()` tries `TiktokenCounter` first and falls back with a warning. The `ContextEngine` defaults to `CharTokenCounter` but accepts any `Arc<dyn TokenCounter>` via `with_token_counter()`.

Per-message estimation adds overhead for structured message types:
- `Assistant` messages: text tokens + 20 (for role/tool_call framing)
- `Tool` messages: text tokens + 10 (for tool_call_id framing)
- `User::MultiPart` messages: `parts.len() * 10` (rough estimate for multi-modal content)

### Context Assembler Design

`ContextEngine` (`crates/context_engine/src/assembler.rs:120`) is the main entry point. Its `assemble()` method accepts a `ContextRequest` and returns an `AssembledContext`. The assembly pipeline:

1. **Check cache** -- SHA-256 keyed cache (`ContextCache`) avoids redundant assembly for identical requests. The cache holds up to 8 entries (FIFO eviction) and uses a generation counter so `invalidate_cache()` logically expires all entries without clearing the map.

2. **Allocate system prompt** -- always first, under `Priority::SystemIdentity`.

3. **Allocate tool definitions** -- only for `ToolAssisted` and `AutonomousTask` strategies. Serializes each tool JSON and sums token estimates.

4. **Retrieve memories** -- calls the optional `MemoryRetriever` (skipped for `DirectResponse` and `Clarification`). Formats results as a `[Relevant Context]` system message with per-entry relevance scores. Allocates under `Priority::RetrievedMemory`.

5. **Compress history** -- hands the full conversation history and the remaining budget to `HistoryCompressor::compress_async()`. Gets back recent messages (verbatim) and summaries (compressed older turns).

6. **Post-compression truncation** -- if recent messages alone exceed the history budget (e.g., a single enormous tool result), the assembler drops the oldest recent messages one at a time until the budget is met, always keeping at least one message.

7. **Budget summaries** -- summaries from compression are included via a scanning accumulator that stops when the remaining post-recent budget is exhausted.

8. **Build final message list** -- ordered as: system prompt, memory context (if any), summaries (if any), recent messages verbatim.

### History Compressor

`HistoryCompressor` (`crates/context_engine/src/history_compressor.rs:71`) decides which messages to keep verbatim and which to summarize.

**Split strategy:**
- Always keep at least `min_recent_messages` (default: 4) from the end of history.
- Expand the recent window backward if the budget allows, using up to half the remaining budget for extra recent messages.
- Everything before the split point gets chunked (default `chunk_size`: 5 messages per chunk) and summarized.

**Compression modes** (`CompressorMode` at line 57):
- **Extractive** (default): no LLM call. Takes the first snippet (up to `snippet_length` characters, default 200) from each message in the chunk. Prefixed with "Earlier in this conversation:". The snippet extraction (`first_snippet` at line 332) is careful about boundaries: it prefers sentence-ending punctuation followed by whitespace, then newlines, then word boundaries, and hard-cuts as a last resort. It also avoids cutting mid-abbreviation (e.g., "Dr.Smith").
- **Abstractive**: sends each chunk to a `SummaryProvider` for LLM-generated summarization. Falls back to extractive per-chunk on provider error. Requires both `mode == Abstractive` and a wired `SummaryProvider`.

The sync `compress()` method always uses extractive mode. The async `compress_async()` method supports abstractive mode when configured.

### Memory Retriever

`MemoryRetriever` (`crates/context_engine/src/memory_retriever.rs:18`) is an `async_trait` designed for dependency inversion. The context engine defines the trait at Layer 2; implementations live in higher layers (agent, storage) where the actual vector database (LanceDB) is available.

During assembly, the engine calls `retrieve(query, limit)` with the user's message text and a configurable limit (default: 5 entries). Each returned `MemoryEntry` has an `id`, `content`, and `score` (0.0--1.0 cosine similarity). Results are formatted into a `[Relevant Context]` system message with per-entry scores.

Memory retrieval is skipped entirely for `DirectResponse` and `Clarification` strategies since simple greetings and clarification requests do not benefit from RAG.

### Summary Provider

`SummaryProvider` (`crates/context_engine/src/summary_provider.rs:11`) is another dependency-inversion trait. It accepts a slice of `Message` values and returns a `Result<String, String>`. Implementations in higher layers call an LLM to produce a concise natural-language summary.

The trait is object-safe (no generic methods) so it can be stored as `Arc<dyn SummaryProvider>`. On error, the history compressor falls back to extractive summarization for that chunk, ensuring the pipeline never fails due to a summary provider issue.

**LlmSummaryProvider** (`crates/agent/src/llm_summary_provider.rs`) is the production implementation of `SummaryProvider`. It lives in the `agent` crate (Layer 5) since it depends on `providers::DynProvider`. Construction takes a `DynProvider` and a model name string. When `summarize()` is called:

1. If the message slice is empty, returns an empty string immediately.
2. Formats each `User` and `Assistant` message into a `"Role: text"` transcript, skipping system and tool messages. Multi-part user content is represented as `"[multipart]"`.
3. Sends a single prompt to the LLM asking for a 2-3 sentence summary preserving key facts and decisions, with `max_tokens` set to 256.
4. On success, returns the LLM's response text. On failure, logs a warning via `tracing::warn` and returns the error as `Err(String)`, which causes the history compressor to fall back to extractive summarization for that chunk.

The provider is wired into the `ContextEngine` via `with_summary_provider()` during `AgentLoop` construction in `builder.rs`. It is only active when the `HistoryCompressor` is configured in `Abstractive` mode.

### Context Sources and Their Priority

`ContextSource` (`crates/context_engine/src/source.rs:29`) is a pluggable trait for contributing sections to the system prompt. Each source has:
- `name()` -- human-readable label for logging.
- `priority()` -- `u8` value; higher values appear earlier in the assembled prompt.
- `provide(ctx)` -- async method returning `Option<String>`.

Sources are registered via `ContextEngine::with_sources()`, which sorts them by priority (descending). During `build_system_prompt()`, all sources are queried concurrently via `join_all`, empty results are filtered, and sections are joined with `\n\n---\n\n` separators.

The `SourceContext` struct passed to each source includes the channel name, chat ID, and optionally the current user message for relevance filtering.

---

## Section 2: API Reference

### `Priority` enum

**File:** `crates/context_engine/src/budget.rs:5`

```rust
pub enum Priority {
    SystemIdentity = 0,
    ActiveTask = 1,
    ToolDefinitions = 2,
    RecentHistory = 3,
    RetrievedMemory = 4,
    CompressedHistory = 5,
    BootstrapPersona = 6,
    Skills = 7,
}
```

Derives: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`. Ordered from highest to lowest priority (lower numeric value = higher priority).

---

### `BudgetConfig`

**File:** `crates/context_engine/src/budget.rs:18`

| Field | Type | Description |
|-------|------|-------------|
| `total_context_window` | `usize` | Total tokens in the model's context window |
| `response_reserve_pct` | `f32` | Fraction reserved for the response (0.0--1.0) |

**Methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `standard` | `fn standard(window: usize) -> Self` | Creates config with 15% response reserve |
| `response_reserve` | `fn response_reserve(&self) -> usize` | Tokens reserved for response generation |
| `available_input` | `fn available_input(&self) -> usize` | Tokens available for input content (total - reserve) |

---

### `BudgetAllocator`

**File:** `crates/context_engine/src/budget.rs:44`

| Field | Type | Description |
|-------|------|-------------|
| `config` | `BudgetConfig` | Budget configuration (private) |
| `allocations` | `HashMap<Priority, usize>` | Per-priority token counts (private) |

**Methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: BudgetConfig) -> Self` | Create allocator with given config |
| `total_allocated` | `fn total_allocated(&self) -> usize` | Sum of all allocated tokens |
| `remaining` | `fn remaining(&self) -> usize` | Tokens still available (`available_input - total_allocated`) |
| `allocate` | `fn allocate(&mut self, priority: Priority, tokens: usize)` | Allocate up to `tokens`, capped at remaining budget |
| `try_allocate` | `fn try_allocate(&mut self, priority: Priority, tokens: usize) -> usize` | Same as `allocate` but returns how many tokens were actually allocated |
| `get` | `fn get(&self, priority: Priority) -> usize` | Current allocation for a specific priority (0 if unset) |
| `report` | `fn report(&self) -> BudgetReport` | Generate a budget usage report |

---

### `BudgetReport`

**File:** `crates/context_engine/src/budget.rs:50`

Derives: `Clone`.

| Field | Type | Description |
|-------|------|-------------|
| `total_window` | `usize` | Total context window size |
| `total_allocated` | `usize` | Sum of all allocations |
| `remaining` | `usize` | Available input minus total allocated |
| `per_priority` | `Vec<(Priority, usize)>` | Per-priority breakdown, sorted by priority |

---

### `TokenCounter` trait

**File:** `crates/context_engine/src/token_counter.rs:9`

Bounds: `Send + Sync`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `estimate_text` | `fn estimate_text(&self, text: &str) -> usize` | Estimate token count for a string |

---

### `CharTokenCounter`

**File:** `crates/context_engine/src/token_counter.rs:15`

Unit struct. Implements `TokenCounter` with `text.len().div_ceil(4)`.

---

### `TiktokenCounter`

**File:** `crates/context_engine/src/token_counter.rs:27`

| Field | Type | Description |
|-------|------|-------------|
| `bpe` | `CoreBPE` | tiktoken BPE model (private) |

**Methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Option<Self>` | Create with `cl100k_base` encoding. Returns `None` on init failure. |

Implements `TokenCounter` via `bpe.encode_with_special_tokens(text).len()`.

Manually implements `Send` and `Sync` (CoreBPE holds compiled regex + read-only data).

---

### Free Functions (token_counter)

**File:** `crates/context_engine/src/token_counter.rs:52-71`

| Function | Signature | Description |
|----------|-----------|-------------|
| `default_token_counter` | `fn default_token_counter() -> Arc<dyn TokenCounter>` | Returns `CharTokenCounter` |
| `best_token_counter` | `fn best_token_counter() -> Arc<dyn TokenCounter>` | Tries `TiktokenCounter`, falls back to `CharTokenCounter` with warning |

---

### `ExecutionStrategy` enum

**File:** `crates/context_engine/src/assembler.rs:22`

Derives: `Debug`, `Clone`.

| Variant | Fields | Description |
|---------|--------|-------------|
| `DirectResponse` | -- | Simple Q&A, no tools |
| `ToolAssisted` | `max_iterations: u32` | May use tools up to N rounds |
| `AutonomousTask` | `max_iterations: u32` | Full multi-step autonomous execution |
| `Clarification` | `reason: String` | Need more info from user |

---

### `ContextRequest`

**File:** `crates/context_engine/src/assembler.rs:34`

| Field | Type | Description |
|-------|------|-------------|
| `message_text` | `String` | User's message (used for memory lookup) |
| `history` | `Vec<Message>` | Full conversation history |
| `system_prompt` | `String` | System prompt to prepend |
| `strategy` | `ExecutionStrategy` | Affects budget allocation decisions |
| `tool_definitions` | `Vec<serde_json::Value>` | Tool JSON schemas |
| `context_window` | `usize` | Model's context window size |

---

### `AssembledContext`

**File:** `crates/context_engine/src/assembler.rs:50`

Derives: `Clone`.

| Field | Type | Description |
|-------|------|-------------|
| `messages` | `Vec<Message>` | Ordered messages ready for the LLM |
| `token_count` | `usize` | Estimated total token count |
| `budget_report` | `BudgetReport` | Budget allocation breakdown |

---

### `ContextEngine`

**File:** `crates/context_engine/src/assembler.rs:120`

| Field | Type | Description |
|-------|------|-------------|
| `compressor` | `HistoryCompressor` | History compression engine (private) |
| `token_counter` | `Arc<dyn TokenCounter>` | Token estimator (private) |
| `memory_retriever` | `Option<Arc<dyn MemoryRetriever>>` | Optional memory retriever (private) |
| `memory_retrieval_limit` | `usize` | Max memory entries per query (default: 5, private) |
| `cache` | `Arc<Mutex<ContextCache>>` | LRU-like assembly cache (private) |
| `sources` | `Vec<Box<dyn ContextSource>>` | Pluggable system prompt sources (private) |

Implements `Default` (char-based counter, extractive compression, no memory retriever, empty sources, cache capacity 8).

**Builder methods** (all return `Self` for chaining):

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Alias for `Default::default()` |
| `with_token_counter` | `fn with_token_counter(self, counter: Arc<dyn TokenCounter>) -> Self` | Override token counter; also rebuilds compressor |
| `with_compressor_config` | `fn with_compressor_config(self, config: CompressorConfig) -> Self` | Override compressor configuration |
| `with_memory_retrieval_limit` | `fn with_memory_retrieval_limit(self, limit: usize) -> Self` | Set max memory entries per query |
| `with_memory_retriever` | `fn with_memory_retriever(self, retriever: Arc<dyn MemoryRetriever>) -> Self` | Wire in embedding-based memory retrieval |
| `with_summary_provider` | `fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self` | Set LLM-backed abstractive summarization |
| `with_sources` | `fn with_sources(mut self, sources: Vec<Box<dyn ContextSource>>) -> Self` | Register context sources (sorted by priority descending) |

**Core methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `assemble` | `async fn assemble(&self, request: ContextRequest) -> AssembledContext` | Cache-aware context assembly |
| `build_system_prompt` | `async fn build_system_prompt(&self, channel: &str, chat_id: &str, message: Option<&str>) -> String` | Assemble system prompt from registered sources |
| `invalidate_cache` | `async fn invalidate_cache(&self)` | Increment cache generation (logically expires all entries) |

---

### `CompressorMode` enum

**File:** `crates/context_engine/src/history_compressor.rs:57`

Derives: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.

| Variant | Description |
|---------|-------------|
| `Extractive` | Snippet-based, no LLM call |
| `Abstractive` | LLM-generated summary via `SummaryProvider` |

---

### `CompressorConfig`

**File:** `crates/context_engine/src/history_compressor.rs:33`

Derives: `Debug`, `Clone`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `snippet_length` | `usize` | `200` | Max characters per message snippet (extractive mode) |
| `mode` | `CompressorMode` | `Extractive` | Compression mode |
| `chunk_size` | `usize` | `5` | Messages per summary chunk |
| `min_recent_messages` | `usize` | `4` | Minimum recent messages always kept verbatim |

---

### `HistorySummary`

**File:** `crates/context_engine/src/history_compressor.rs:12`

| Field | Type | Description |
|-------|------|-------------|
| `content` | `String` | The summarized text |
| `message_range` | `(usize, usize)` | Start and end indices in the original history |
| `token_count` | `usize` | Estimated token count for this summary |

---

### `CompressedHistory`

**File:** `crates/context_engine/src/history_compressor.rs:22`

| Field | Type | Description |
|-------|------|-------------|
| `summaries` | `Vec<HistorySummary>` | Compressed older message chunks |
| `recent_messages` | `Vec<Message>` | Recent messages kept verbatim |
| `total_tokens` | `usize` | Estimated total tokens (summaries + recent) |

---

### `HistoryCompressor`

**File:** `crates/context_engine/src/history_compressor.rs:71`

| Field | Type | Description |
|-------|------|-------------|
| `token_counter` | `Arc<dyn TokenCounter>` | Token estimator (private) |
| `config` | `CompressorConfig` | Compression settings (private) |
| `summary_provider` | `Option<Arc<dyn SummaryProvider>>` | Optional LLM summarizer (private) |

**Constructors:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(min_recent: usize, token_counter: Arc<dyn TokenCounter>) -> Self` | Create with custom min_recent and counter |
| `with_config` | `fn with_config(min_recent: usize, token_counter: Arc<dyn TokenCounter>, config: CompressorConfig) -> Self` | Create with full config (overrides config's `min_recent_messages` with the argument) |
| `from_config` | `fn from_config(token_counter: Arc<dyn TokenCounter>, config: CompressorConfig) -> Self` | Create from config directly (uses config's `min_recent_messages`) |
| `with_defaults` | `fn with_defaults(min_recent: usize) -> Self` | Create with char-based counter |
| `with_summary_provider` | `fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self` | Set abstractive summary provider (builder) |

**Core methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `compress` | `fn compress(&self, history: &[Message], budget_tokens: usize) -> CompressedHistory` | Synchronous compression (always extractive) |
| `compress_async` | `async fn compress_async(&self, history: &[Message], budget_tokens: usize) -> CompressedHistory` | Async compression (supports abstractive mode) |
| `extractive_summary` | `fn extractive_summary(messages: &[Message]) -> String` | Static: extractive summary with default snippet length (200) |
| `extractive_summary_with_length` | `fn extractive_summary_with_length(messages: &[Message], snippet_length: usize) -> String` | Static: extractive summary with custom snippet length |

---

### `MemoryRetriever` trait

**File:** `crates/context_engine/src/memory_retriever.rs:18`

Bounds: `Send + Sync` (via `#[async_trait]`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `retrieve` | `async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry>` | Retrieve up to `limit` memories relevant to query |

---

### `MemoryEntry`

**File:** `crates/context_engine/src/memory_retriever.rs:4`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Unique identifier (e.g., todo ID or memory key) |
| `content` | `String` | Text content of the memory |
| `score` | `f64` | Similarity score (0.0--1.0, higher = more relevant) |

---

### `SummaryProvider` trait

**File:** `crates/context_engine/src/summary_provider.rs:11`

Bounds: `Send + Sync` (via `#[async_trait]`). Object-safe.

| Method | Signature | Description |
|--------|-----------|-------------|
| `summarize` | `async fn summarize(&self, messages: &[Message]) -> Result<String, String>` | Summarize messages into a single string. Returns `Err` on failure. |

---

### `ContextSource` trait

**File:** `crates/context_engine/src/source.rs:29`

Bounds: `Send + Sync` (via `#[async_trait]`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `name` | `fn name(&self) -> &str` | Human-readable name for logging |
| `priority` | `fn priority(&self) -> u8` | Ordering priority (higher = earlier in prompt) |
| `provide` | `async fn provide(&self, ctx: &SourceContext) -> Option<String>` | Produce a context section, or `None` to skip |

---

### `SourceContext`

**File:** `crates/context_engine/src/source.rs:11`

Derives: `Debug`, `Clone`.

| Field | Type | Description |
|-------|------|-------------|
| `channel` | `String` | Channel name (e.g., "telegram", "discord", "cli") |
| `chat_id` | `String` | Chat/conversation ID |
| `message` | `Option<String>` | Current user message (for relevance filtering) |
