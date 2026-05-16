# Crate: `context_engine`

> **Status:** 🟢 Stable
> **Subsystem:** [04 — Agent Runtime](../subsystems/04-agent-runtime.md)
> **Status last verified:** 2026-05-16
> **One-liner:** Token-budgeted context assembly + tiered history compression for every LLM call

---

## TL;DR

The token-budget and assembly layer for every LLM call. Owns `ContextEngine` (orchestrator), `BudgetAllocator` (8-level priority system), `TieredHistoryCompressor` (per-turn history compression — extractive-first with LLM fallback), `ContextSource` trait (~30 impls register via this), `MemoryRetriever` + `MemoryScorer` + `TokenCounter` + `SummaryProvider` abstractions, and `InsightForge` (multi-dimensional retrieval orchestrator with circuit breaker).

The crate is **bounded and coherent**: it doesn't talk to LLMs (that's `providers`), doesn't own memory storage (that's `storage` + `cognitive`), and doesn't dispatch tools (that's `agent`). Its single responsibility is "given a request and a token budget, produce the assembled context the LLM will see."

The most important constant: `LOW_BUDGET_THRESHOLD = 0.15` — emits warnings when fewer than 15% of context tokens remain. The most important env flag: `KCA_DISABLE_COMPRESSION=1` — bypasses `TieredHistoryCompressor` entirely (Letta benchmark mode).

---

## Module map

```
crates/context_engine/src/
├── lib.rs                  ← Public re-exports
│
├── assembler/
│   ├── mod.rs              ← ContextEngine — orchestrator
│   ├── cache.rs            ← ContextCache (LRU)
│   └── types.rs            ← ContextRequest, AssembledContext, ExecutionStrategy, CompressionStats
│
├── budget.rs               ← BudgetAllocator, BudgetConfig, Priority (8 levels), LOW_BUDGET_THRESHOLD = 0.15
│
├── history_compressor/
│   ├── mod.rs              ← Re-exports, TIER1_INSTRUCTIONS, TIER2_INSTRUCTIONS
│   ├── tiered.rs           ← TieredHistoryCompressor — full pipeline
│   ├── grouping.rs         ← group_into_turns
│   ├── snippet.rs          ← first_snippet (extractive helper)
│   └── types.rs            ← AssignedTier, ConversationTurn, TierSummary, CompressedHistory, CompressionTier
│
├── source.rs               ← ContextSource trait + SourceContext
│
├── enhancement/
│   ├── mod.rs
│   ├── pipeline.rs         ← QueryPipeline + RankingPipeline
│   ├── prf.rs              ← Pseudo-Relevance Feedback stage
│   └── heuristic_rerank.rs ← Heuristic reranker stage
│
├── insight_forge/
│   ├── mod.rs              ← InsightForge — multi-dim retrieval orchestrator
│   ├── circuit_breaker.rs  ← Per-domain circuit breaker
│   ├── decomposer.rs       ← Query decomposer trait
│   └── llm_decomposer.rs   ← LLM-driven query decomposition
│
├── rewriter.rs             ← RetrievalContext, CorrectionContext, UserSituationSnapshot, ActiveView
├── memory_retriever.rs     ← MemoryRetriever trait + MemoryEntry, MemorySource
├── memory_scorer.rs        ← MemoryScorer trait (cognitive scoring entry)
├── token_counter.rs        ← TokenCounter trait + CharTokenCounter, AnthropicTokenCounter, TiktokenCounter
├── summary_provider.rs     ← SummaryProvider trait
├── ttl_cache.rs            ← Generic TtlCache
└── inventory.rs            ← ContextInventory for deferred source tracking
```

---

## Public API surface

### `ContextEngine`

```rust
pub struct ContextEngine { /* opaque */ }

impl ContextEngine {
    pub fn new(config: HistoryCompressionConfig) -> Self;

    /// Build the system prompt — joins all registered ContextSource outputs.
    /// Highest-priority source (SoulContextSource at priority 50) is read live from disk.
    pub async fn build_system_prompt(
        &self,
        channel: &str,
        chat_id: &str,
        message: Option<&str>,
        session_mode: common::SessionMode,
    ) -> String;

    /// Assemble the full context for a request.
    pub async fn assemble(&self, request: ContextRequest) -> AssembledContext;

    /// Assemble using a previously prefetched memory blob.
    pub async fn assemble_with_prefetched(
        &self,
        request: ContextRequest,
        prefetched: Option<(String, usize, Option<EnhancementTrace>)>,
    ) -> AssembledContext;

    /// Prefetch memory before LLM call (used by KCA Track 7 predictive cache).
    pub async fn prefetch_memory(
        &self,
        message: &str,
        session_key: Option<String>,
        retrieval_context: Option<RetrievalContext>,
    ) -> Option<(String, usize, Option<EnhancementTrace>)>;

    /// Expand context to include a specific source's full output.
    pub async fn expand(
        &self,
        current: &AssembledContext,
        source_name: &str,
        source_ctx: &SourceContext,
    ) -> Result<AssembledContext>;

    pub fn register_source(&mut self, source: Box<dyn ContextSource>);

    pub fn tier0_config(&self) -> &config::schema::TierZeroConfig;

    // Builder methods (fluent)
    pub fn with_token_counter(self, counter: Arc<dyn TokenCounter>) -> Self;
    pub fn with_memory_retriever(self, retriever: Arc<dyn MemoryRetriever>) -> Self;
    pub fn with_summary_provider(self, provider: Arc<dyn SummaryProvider>) -> Self;
    pub fn with_sources(self, sources: Vec<Box<dyn ContextSource>>) -> Self;
    pub fn with_insight_forge(self, forge: Arc<InsightForge>) -> Self;
    pub fn with_query_pipeline(self, pipeline: Arc<QueryPipeline>) -> Self;
    pub fn with_ranking_pipeline(self, pipeline: Arc<RankingPipeline>) -> Self;
    pub fn with_memory_scorer(self, scorer: Arc<dyn MemoryScorer>) -> Self;
}
```

### `ContextSource` trait + `SourceContext`

```rust
#[async_trait]
pub trait ContextSource: Send + Sync {
    fn name(&self) -> &str;

    /// Higher = more important. Built-in priorities:
    /// - 50: SoulContextSource (the soul file)
    /// - 40: SkillListingSource
    /// - 30s: high-importance domain sources
    /// - 20s: standard
    /// - 10s: nice-to-have
    fn priority(&self) -> u8;

    /// If true, BudgetAllocator never truncates this source.
    fn protected(&self) -> bool { false }

    /// Return the source's content for this request, or None to skip.
    async fn provide(&self, ctx: &SourceContext) -> Option<String>;

    /// Estimated token count for budget planning. Should be ≤ actual.
    fn estimated_tokens(&self) -> usize { 0 }
}

pub struct SourceContext {
    pub channel: String,
    pub chat_id: String,
    pub session_key: Option<String>,
    pub message: Option<String>,
    pub intent_summary: Option<String>,    // ⚠️ always None today (vestigial)
    pub session_mode: SessionMode,
    pub user_situation: Option<UserSituationSnapshot>,
    // ...
}
```

### `BudgetAllocator`

```rust
pub struct BudgetAllocator { /* opaque */ }

impl BudgetAllocator {
    pub fn new(config: BudgetConfig) -> Self;

    /// Total context window size from config.
    pub fn config_total_window(&self) -> usize;

    /// Total tokens allocated across all priorities.
    pub fn total_allocated(&self) -> usize;

    /// Tokens still available for allocation.
    pub fn remaining(&self) -> usize;

    /// Allocate up to `tokens` for the given priority, capped at remaining budget.
    /// Emits a warning when the budget drops below 15% of available input.
    pub fn allocate(&mut self, priority: Priority, tokens: usize);

    /// Best-effort allocation; returns the number actually allocated.
    pub fn try_allocate(&mut self, priority: Priority, tokens: usize) -> usize;

    /// Get the current allocation for a specific priority.
    pub fn get(&self, priority: Priority) -> usize;

    /// Generate a budget usage report.
    pub fn report(&self) -> BudgetReport;
}

pub struct BudgetConfig {
    pub total_context_window: usize,
    pub response_reserve_pct: f32,        // default: 0.15
}

impl BudgetConfig {
    pub fn standard(window: usize) -> Self;
    pub fn response_reserve(&self) -> usize;
    pub fn available_input(&self) -> usize;
}

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

pub struct BudgetReport {
    pub total_window: usize,
    pub total_allocated: usize,
    pub remaining: usize,
    pub per_priority: Vec<(Priority, usize)>,
}
```

### `TieredHistoryCompressor`

```rust
pub struct TieredHistoryCompressor { /* opaque */ }

impl TieredHistoryCompressor {
    pub fn new(
        token_counter: Arc<dyn TokenCounter>,
        config: HistoryCompressionConfig,
    ) -> Self;

    pub async fn compress(
        &self,
        history: &[Message],
        budget_tokens: usize,
        tier0_count: usize,
    ) -> CompressedHistory;
}

pub struct HistoryCompressionConfig {
    pub tier0_count: usize,               // recent messages preserved verbatim (default 8)
    pub high_relevance_threshold: f64,
    pub use_cognitive_scoring: bool,
    pub target_ratio: f32,                // compressed/original token ratio target
    pub min_turn_tokens: u32,             // skip turns smaller than this
    pub batch_size: usize,                // default 5
    // ...
}

pub struct CompressedHistory {
    pub summaries: Vec<TierSummary>,
    pub recent_messages: Vec<Message>,    // verbatim tier-0
    pub preamble: Option<String>,
    pub total_tokens: usize,
}

pub struct ConversationTurn {
    pub user_msg: Option<Message>,
    pub assistant_msg: Option<Message>,
    pub tool_messages: Vec<Message>,
    pub started_at: Option<Timestamp>,
}

pub enum AssignedTier { Detailed, Condensed }

pub struct TierSummary {
    pub tier: AssignedTier,
    pub turn_indices: Vec<usize>,
    pub summary_text: String,
    pub tokens: usize,
}

pub enum CompressionTier {
    Tier0Verbatim,
    Tier1Detailed,
    Tier2Condensed,
}

pub const TIER1_INSTRUCTIONS: &str = "...";
pub const TIER2_INSTRUCTIONS: &str = "...";
```

### `ContextRequest` + `AssembledContext`

```rust
pub struct ContextRequest {
    pub message: String,
    pub session_key: Option<String>,
    pub session_mode: SessionMode,
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub history: Vec<Message>,
    pub strategy: ExecutionStrategy,
    pub retrieval_context: Option<RetrievalContext>,
    pub user_situation: Option<UserSituationSnapshot>,
}

pub struct AssembledContext {
    pub system_prompt: String,
    pub messages: Vec<Message>,           // includes compressed history + new message
    pub total_tokens: usize,
    pub budget_report: BudgetReport,
    pub compression_stats: CompressionStats,
    pub enhancement_trace: Option<EnhancementTrace>,
    pub sources_used: Vec<String>,
}

pub enum ExecutionStrategy {
    DirectResponse,                       // no tools; skips memory retrieval
    ToolAssisted { max_iterations: u32 },
    AutonomousTask,
    Clarification,                        // skips memory retrieval
}

pub struct CompressionStats {
    pub turns_compressed: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub ratio: f32,
}
```

### `MemoryRetriever` + supporting types

```rust
#[async_trait]
pub trait MemoryRetriever: Send + Sync {
    async fn retrieve(
        &self,
        query: &str,
        session_key: Option<&str>,
        ctx: Option<&RetrievalContext>,
    ) -> Result<Vec<MemoryEntry>>;
}

pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub source: MemorySource,
    pub score: f64,
    pub created_at: Timestamp,
    pub metadata: Value,
}

pub enum MemorySource {
    Semantic,
    Episodic,
    Notes,
    Procedural,
    Task,
    Custom(String),
}
```

### `MemoryScorer`

```rust
#[async_trait]
pub trait MemoryScorer: Send + Sync {
    async fn score_batch(
        &self,
        turns: &[ConversationTurn],
        query: &str,
    ) -> Result<Vec<f64>>;
}
```

`TieredHistoryCompressor` consumes this to drive tier assignment.

### `TokenCounter`

```rust
pub trait TokenCounter: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
    fn count_messages(&self, messages: &[Message]) -> usize;
}

pub struct CharTokenCounter;                              // chars / 4
pub struct AnthropicTokenCounter { /* model-specific */ }
pub struct TiktokenCounter { /* model-specific */ }
```

`AnthropicTokenCounter` and `TiktokenCounter` are accurate; `CharTokenCounter` is a fallback (4 chars/token approximation).

### `SummaryProvider`

```rust
#[async_trait]
pub trait SummaryProvider: Send + Sync {
    async fn summarize_batch(
        &self,
        items: Vec<SummarizationItem>,
    ) -> Result<Vec<String>>;
}

pub struct SummarizationItem {
    pub instructions: String,
    pub content: String,
    pub target_tokens: u32,
}
```

Implemented by `agent::adapters::llm_summary::LlmSummaryProvider`.

### `InsightForge`

```rust
pub struct InsightForge { /* opaque */ }

impl InsightForge {
    pub fn new(config: InsightForgeConfig) -> Self;

    pub async fn retrieve(
        &self,
        query: &str,
        ctx: &RetrievalContext,
    ) -> Result<EnhancementTrace>;

    pub fn register_domain(&mut self, name: &str, retriever: Arc<dyn MemoryRetriever>);
}

pub struct InsightForgeConfig {
    pub max_concurrent_domains: usize,
    pub per_domain_timeout: Duration,
    pub circuit_breaker_threshold: u32,
    // ...
}
```

Multi-dimensional retrieval — fan-out to multiple `MemoryRetriever` implementations (e.g., semantic memory, episodic memory, notes) in parallel, with per-domain circuit breaker. Failed domains don't block other retrievals.

### `QueryPipeline` + `RankingPipeline`

```rust
pub struct QueryPipeline { /* stages */ }
impl QueryPipeline {
    pub fn new() -> Self;
    pub fn add_stage(&mut self, stage: Arc<dyn QueryStage>);
    pub async fn run(&self, query: String, ctx: &RetrievalContext) -> Vec<String>;
}

pub struct RankingPipeline { /* stages */ }
impl RankingPipeline {
    pub fn new() -> Self;
    pub fn add_stage(&mut self, stage: Arc<dyn RankingStage>);
    pub async fn run(&self, items: Vec<MemoryEntry>, query: &str) -> Vec<MemoryEntry>;
}

#[async_trait]
pub trait QueryStage: Send + Sync {
    async fn process(&self, query: String, ctx: &RetrievalContext) -> Result<Vec<String>>;
}

#[async_trait]
pub trait RankingStage: Send + Sync {
    async fn process(&self, items: Vec<MemoryEntry>, query: &str) -> Result<Vec<MemoryEntry>>;
}
```

Built-in stages: PRF (Pseudo-Relevance Feedback), HeuristicRerank.

### `RetrievalContext` + related

```rust
pub struct RetrievalContext {
    pub session_key: Option<String>,
    pub session_mode: SessionMode,
    pub time_range: Option<(Timestamp, Timestamp)>,
    pub entity_filters: Vec<String>,
    pub user_situation: Option<UserSituationSnapshot>,
    pub active_view: Option<ActiveView>,
    pub correction_context: Option<CorrectionContext>,
}

pub struct CorrectionContext {
    pub previous_response: String,
    pub correction_signal: String,
}

pub struct UserSituationSnapshot {
    pub timestamp: Timestamp,
    pub active_tasks: Vec<String>,
    pub current_focus: Option<String>,
    pub recent_topics: Vec<String>,
    // ...
}

pub enum ActiveView {
    Tasks,
    Notes,
    Finance,
    Productivity,
    Coding,
    None,
}
```

### `TtlCache<K, V>`

```rust
pub struct TtlCache<K: Eq + Hash + Clone, V: Clone> { /* DashMap-backed */ }

impl<K, V> TtlCache<K, V> {
    pub fn new(ttl: Duration) -> Self;
    pub fn get(&self, key: &K) -> Option<V>;
    pub fn insert(&self, key: K, value: V);
    pub fn remove(&self, key: &K);
    pub fn cleanup_expired(&self);
}
```

### `ContextInventory`

```rust
pub struct ContextInventory { /* opaque */ }

impl ContextInventory {
    pub fn new() -> Self;
    pub fn defer(&self, source_name: String, estimated_tokens: usize);
    pub fn deferred(&self) -> Vec<DeferredSource>;
}
```

Tracks sources skipped due to budget so the agent can request them via `ContextEngine::expand` later.

---

## Internals

### `TieredHistoryCompressor` pipeline

```
1. Check KCA_DISABLE_COMPRESSION → return verbatim if set
2. Early exit if turn count ≤ tier0_count
3. Microcompact pre-pass:
   For COMPACTABLE_TOOLS (read_file, bash, grep, glob, web_search, web_fetch),
   outside tier0_count × 2 window,
   replace results with 150-char snippet
4. group_into_turns → split into ConversationTurn objects at tier0_start
5. Optional cognitive scoring via MemoryScorer::score_batch (if use_cognitive_scoring enabled)
6. Tier assignment:
   - Detailed: high score (≥ high_relevance_threshold) OR within recency window
   - Condensed: otherwise
7. compress_turns:
   - Batch consecutive same-tier turns (sub-batches of 5)
   - Extractive-first; only call LLM when extractive output exceeds target_ratio × original_tokens
   - Skip turns < 30 tokens
8. Return CompressedHistory { summaries, recent_messages, preamble, total_tokens }
```

**Microcompact window vs tier0 window:** Microcompact uses `tier0_count × 2` (covers history about to be compressed). MidLoopCompressor in `agent` uses fixed `MIN_RECENT_MESSAGES = 8`. The two are independent.

### Why extractive-first

LLM-based summarization adds latency + cost per turn. Extractive (first 150 chars, drop image parts) is free + deterministic. The `target_ratio` check decides when extractive is good enough — only fall back to LLM if extractive output exceeds the ratio.

### Priority-based budget enforcement

`BudgetAllocator::try_allocate` reserves budget per priority level. When budget is tight:
- Sources tagged `protected()` are never truncated
- Higher-priority sources allocated first
- `LOW_BUDGET_THRESHOLD = 0.15` triggers warnings during `allocate()` when remaining budget falls below 15% of available input

Sources can call `try_allocate(priority, estimated)` to learn if they fit, and gracefully shorten or skip themselves.

### `InsightForge` circuit breaker

Per-domain circuit breaker. After N consecutive failures from one domain (e.g., `notes` retrieval), opens that domain's breaker — subsequent calls skip it and return empty. Cooldown elapses, half-open probe, optional reset.

**Why per-domain:** A failing notes index shouldn't block semantic-memory retrieval. Each domain is independent.

### `ContextEngine::build_system_prompt` composition order

Sources are iterated in priority order (highest first). Each is called via `provide(ctx)`. Returned `Option<String>` values are joined with `\n\n`. Truncation happens via `BudgetAllocator` per source.

The first-priority source (`SoulContextSource` at priority 50) is read live from disk on every call (mtime-cached). Edits to `~/.klyntbot/KLYNTBOT.md` take effect on the next message.

### `ContextEngine::expand` (deferred source pattern)

When budget is tight, sources that don't fit are added to `ContextInventory`. The agent can later request a specific source via `expand(current, source_name, source_ctx)` — typically driven by a `context_request` tool call from the LLM.

### Vestigial `intent_summary`

`SourceContext::intent_summary` always `None` in the current flat runtime. Field is dead but not deleted. See [`TECH_DEBT.md`](../TECH_DEBT.md).

---

## Workflows

### Build system prompt + assemble context for a turn

```rust
let system_prompt = engine.build_system_prompt(
    "telegram", "12345", Some("hello"), SessionMode::Assistant,
).await;

let context = engine.assemble(ContextRequest {
    message: "hello".to_string(),
    session_key: Some(session_key.clone()),
    session_mode: SessionMode::Assistant,
    channel: ChannelName::new("telegram"),
    chat_id: ChatId::new("12345"),
    history: previous_messages,
    strategy: ExecutionStrategy::ToolAssisted { max_iterations: 10 },
    retrieval_context: None,
    user_situation: None,
}).await;

// context.system_prompt + context.messages now ready for the LLM call
```

### Prefetch + reuse (KCA Track 7)

```rust
// Background task fired after previous turn
let prefetched = engine.prefetch_memory(
    &predicted_query, Some(session_key.clone()), None,
).await;

// On next actual turn — reuse prefetched
let context = engine.assemble_with_prefetched(request, prefetched).await;
```

### Compress an oversized history

```rust
let compressed = compressor.compress(
    &full_history,
    /* budget_tokens */ 50_000,
    /* tier0_count */ 8,
).await;

let total_messages: Vec<Message> = compressed.summaries.iter()
    .flat_map(|s| { /* materialize summary as Message */ })
    .chain(compressed.recent_messages.iter().cloned())
    .collect();
```

### Custom retrieval pipeline

```rust
let mut pipeline = RankingPipeline::new();
pipeline.add_stage(Arc::new(PrfStage::new()));
pipeline.add_stage(Arc::new(HeuristicRerankStage::new()));

let ranked = pipeline.run(initial_items, query).await;
```

---

## Testing approach

### Unit-test a `ContextSource`

```rust
let source = MyContextSource::new(deps);
let ctx = SourceContext { /* fill */ };
let output = source.provide(&ctx).await;
assert!(output.is_some());
```

### Use `CharTokenCounter` for tests

```rust
let counter: Arc<dyn TokenCounter> = Arc::new(CharTokenCounter);
let n = counter.count_tokens("hello world");
// ~3 tokens (11 chars / 4)
```

Production code should use `AnthropicTokenCounter` or `TiktokenCounter`.

### Mock `MemoryRetriever`

```rust
struct ConstRetriever(Vec<MemoryEntry>);

#[async_trait]
impl MemoryRetriever for ConstRetriever {
    async fn retrieve(&self, _, _, _) -> Result<Vec<MemoryEntry>> {
        Ok(self.0.clone())
    }
}
```

### Test compression behavior

```rust
let compressor = TieredHistoryCompressor::new(
    Arc::new(CharTokenCounter),
    HistoryCompressionConfig {
        tier0_count: 8,
        high_relevance_threshold: 0.5,
        use_cognitive_scoring: false,    // skip scorer for unit tests
        target_ratio: 0.5,
        min_turn_tokens: 30,
        batch_size: 5,
    },
);

let result = compressor.compress(&history, 10_000, 8).await;
assert_eq!(result.recent_messages.len(), 8);
```

### Test KCA escape hatch

```rust
std::env::set_var("KCA_DISABLE_COMPRESSION", "1");
let result = compressor.compress(&long_history, 1_000, 8).await;
// Verify history returned verbatim
```

---

## Extension points

### Add a `ContextSource`

```rust
struct MySource { /* deps */ }

#[async_trait]
impl ContextSource for MySource {
    fn name(&self) -> &str { "my_source" }
    fn priority(&self) -> u8 { 25 }
    fn protected(&self) -> bool { false }
    fn estimated_tokens(&self) -> usize { 200 }
    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // Return None to skip, Some(text) to include
        Some(format!("Active session: {}", ctx.chat_id))
    }
}

// Register at startup
engine.register_source(Box::new(MySource::new(deps)));
```

**Choose priority deliberately:**
- 50 = soul (built-in)
- 40 = skill listing (built-in)
- 30s = high-importance domain (active task, current OKR)
- 20s = standard (recent activity)
- 10s = nice-to-have (background context)

### Add a `QueryStage` / `RankingStage`

```rust
struct MyStage;

#[async_trait]
impl QueryStage for MyStage {
    async fn process(&self, query: String, ctx: &RetrievalContext) -> Result<Vec<String>> {
        // E.g. query expansion, entity extraction, etc.
        Ok(vec![query.clone(), format!("expanded: {}", query)])
    }
}

pipeline.add_stage(Arc::new(MyStage));
```

### Add a `MemoryRetriever`

```rust
struct MyRetriever { /* deps */ }

#[async_trait]
impl MemoryRetriever for MyRetriever {
    async fn retrieve(
        &self, query: &str, session_key: Option<&str>, ctx: Option<&RetrievalContext>,
    ) -> Result<Vec<MemoryEntry>> {
        // Query backend, map to MemoryEntry
        Ok(vec![])
    }
}

// Register with InsightForge
forge.register_domain("my_domain", Arc::new(MyRetriever::new(deps)));
```

### Add a `MemoryScorer`

```rust
#[async_trait]
impl MemoryScorer for MyScorer {
    async fn score_batch(&self, turns: &[ConversationTurn], query: &str) -> Result<Vec<f64>> {
        // Score 0.0..1.0 per turn for relevance to query
        Ok(vec![0.5; turns.len()])
    }
}

engine = engine.with_memory_scorer(Arc::new(MyScorer));
```

### Add a `TokenCounter` impl

⚠️ The contract is: `count_tokens(text)` returns the model's actual token count. Inaccurate counters → budget violations. Prefer model-specific implementations.

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| `LOW_BUDGET_THRESHOLD` | `0.15` | `budget.rs:6` |
| Default `tier0_count` | `8` | `HistoryCompressionConfig::default` |
| Default `batch_size` | `5` | `HistoryCompressionConfig::default` |
| Default `target_ratio` | `0.5` | `HistoryCompressionConfig::default` |
| Default `min_turn_tokens` | `30` | `HistoryCompressionConfig::default` |
| `COMPACTABLE_TOOLS` (microcompact) | `["read_file", "bash", "grep", "glob", "web_search", "web_fetch"]` | `history_compressor/tiered.rs` |
| Microcompact snippet length | `150` chars | `snippet.rs::first_snippet` |

---

## Open questions

- **`intent_summary` is vestigial.** Always None. Decide: delete or repurpose.
- **`SoulContextSource` lives in `skill-system`** but is consumed here. Acceptable today; consider whether the trait should move to `context_engine` for crate hygiene.
- **8 priority levels may be overkill.** Most sources use 3-4 distinct levels. Could collapse to `Critical`/`High`/`Normal`/`Low`.
- **`COMPACTABLE_TOOLS` is hardcoded** in microcompact. Should be configurable per-deployment.
- **`InsightForge` per-domain circuit breaker is in-memory only** — restarts reset state. Acceptable for an idempotent system.
- **No metrics for compression effectiveness** — `CompressionStats` is per-call; no aggregate (avg ratio, p95 ratio, count of cognitive-scoring calls, etc.).
- **`ContextInventory` is rarely consumed** — most callers don't request expansion. Either remove or build a better story around `context_request` tool calls.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #3 + #9 for specifics.

---

## Cross-references

- [Subsystem 04 — Agent Runtime](../subsystems/04-agent-runtime.md) (parent)
- [`crates/agent.md`](./agent.md) (`AgentRuntime` consumes `ContextEngine`)
- [`crates/providers.md`](./providers.md) (`SummaryProvider` typically wraps `DynProvider`)
- [`crates/cognitive.md`](./cognitive.md) *(planned)* (`MemoryRetriever` + `MemoryScorer` impls)
- [`crates/tools-core.md`](./tools-core.md) (`RoutingContext` consumed indirectly)
