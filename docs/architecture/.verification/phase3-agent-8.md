# Phase 3 Agent-8 Verification Report

**Scope:** `agent`, `app-core`, `context_engine` architecture docs vs. source code
**Date:** 2026-05-16
**Verifiers:** agent.md (643 lines), app-core.md (746 lines), context_engine.md (823 lines)

---

## Summary

| Crate | Status | Key Issues |
|-------|--------|------------|
| `agent` | 🟡 Moderate drift | `AgentEvent` variants, `LoopFinishReason` names, `SafetyCap`/`ExecuteLoopResult` fields, `ExecutionParams` fields |
| `app-core` | 🟡 Moderate drift | `ThreadRuntime` trait signature, `init_with_sender` parameters, init-phase oversimplification, `title_service.rs` inaccuracy |
| `context_engine` | 🔴 Significant drift | `Priority` enum completely wrong, `BudgetReport` fields, `TieredHistoryCompressor` signatures/types, `MemoryRetriever` trait, `TokenCounter` trait, `SourceContext` fields |

**Overall:** All three docs contain load-bearing inaccuracies in public API signatures. The `context_engine` doc is the most divergent — its `BudgetAllocator` section describes an entirely different priority system than the code. Several of these mismatches will mislead anyone implementing against the documented API.

---

## `agent` Crate (`docs/architecture/crates/agent.md`)

### ✅ Verified Correct

| Claim | Source | Result |
|-------|--------|--------|
| `MAX_CONCURRENT_TOOLS = 10` | `execution/core.rs:60` | ✅ |
| `MAX_TOOL_RESULT_LENGTH = 50_000` | `execution/core.rs:65` | ✅ |
| `LONG_RUNNING_TOOL_TIMEOUT = 600s` | `execution/core.rs:54` | ✅ |
| `COMPRESSION_THRESHOLD = 0.70` | `execution/mid_loop_compressor.rs:15` | ✅ |
| `MIN_RECENT_MESSAGES = 8` | `execution/mid_loop_compressor.rs:18` | ✅ |
| `MIN_COMPRESSIBLE_TOKENS = 50` | `execution/mid_loop_compressor.rs:24` | ✅ |
| `DEFAULT_TURN_CAP = 500` | `subagent_runtime.rs:20` | ✅ |
| `CORRECTION_WINDOW_MINUTES = 15` | `agent_loop/mod.rs:28` | ✅ |
| `LoopDetector` Warning@3, HardStop@5 | `execution/loop_detector.rs:54` | ✅ |
| Main agent no turn cap (`u32::MAX`) | `execution/budget.rs` via `SafetyCap::new` | ✅ |
| `KCA_PHASE_4_TOOL_DRIVEN` / `KCA_PHASE_4_LEGACY_NUDGE` | `agent_runtime/runtime.rs:575-586` | ✅ |
| `MidLoopCompressor` extractive strategy | `execution/mid_loop_compressor.rs` | ✅ |
| `LiveContextRefresher` 20%/10% reserve | `execution/live_context_refresher.rs:13-16` | ✅ |
| `CachePolicy` 2–3 breakpoints | `execution/cache_policy.rs:23-55` | ✅ |
| `ContentChunk` emits streaming events | `events.rs` | ✅ |
| `SubagentManager` per-invocation `AgentTaskTool` | `subagent.rs:796-824` | ✅ |

### ❌ Drift — Public API

#### `AgentEvent` variant names and fields (Major)

Doc lists ~20 variants with incorrect field names. Code has ~50 variants.

| Doc | Code |
|-----|------|
| `ContentChunk { text: String }` | `ContentChunk { data: String }` |
| `Reasoning { text: String }` | `ReasoningChunk { data: String }` |
| `ToolStart { name, args_preview: String, call_id }` | `ToolStart { name, args: Value, agent: Option<String>, call_id: Option<String> }` |
| `ToolEnd { call_id, success, output_preview: String, duration_ms }` | `ToolEnd { name, success, duration_ms, result: Option<String>, agent: Option<String>, call_id: Option<String> }` |
| `IterationStart { iteration: u32 }` | `IterationStart { iteration: usize, max: usize }` |
| `ApprovalRequest { ... }` | `ApprovalRequested { ... }` (name mismatch) |
| `FinalResponse { content }` | `LoopCompleted { content }` (name mismatch) |
| `BudgetUpdate { ... }` | Not found in quick scan — may be renamed |
| `UsageReport { usage }` | Not found — may be `UsageUpdated` or similar |
| `PreCompactionRun { before, after }` | Not found — may be `CompressionApplied` |
| `LoopWarning { tool, repeat_count }` | `LoopWarning { iteration, message }` |
| `LoopHardStop { tool }` | `LoopHardStop { iteration, message }` |

**Impact:** Anyone subscribing to `AgentEvent` streams using the documented field names will get compile errors.

#### `LoopFinishReason` variants renamed (Major)

| Doc | Code |
|-----|------|
| `FinalResponse` | `Completed` |
| `FabricatedResponse` | *(folded into `Completed`)* |
| `EmptyResponse` | *(folded into `Completed`)* |
| `Cancelled` | `Cancelled` ✅ |
| `SafetyCap` | `SafetyTurnLimit` |
| `LoopDetected` | `LoopDetected` ✅ |
| *(missing)* | `TokenLimit` |

Doc also lists `SafetyCap` as a variant; code uses `SafetyTurnLimit`. `TokenLimit` is present in code but absent from doc.

#### `ExecuteLoopResult` fields mismatch (Major)

| Doc | Code |
|-----|------|
| `messages: Vec<Message>` | *(not present)* |
| `final_response: Option<String>` | `content: String` |
| `turns_used: u32` | `turns: u32` |
| `finish_reason: LoopFinishReason` | `finish_reason: LoopFinishReason` ✅ |
| `total_usage: Usage` | `usage: Usage` |
| `safety_cap_hit: bool` | `safety_cap_hit: bool` ✅ |
| *(missing)* | `tool_calls: Vec<String>` |

#### `SafetyCap` signature drift (Moderate)

| Doc | Code |
|-----|------|
| `with_limits(turns: u32, tokens: u32, depth: DepthMode)` | `with_limits(depth: DepthMode, max_tokens: u64, max_turns: u32)` |
| `max_total_tokens: u32` | `max_tokens: u64` |
| `used_tokens: u32` | `tokens_used: u64` |
| `would_exceed_turns`, `would_exceed_tokens`, `tick` | `turn_cap_hit()`, `token_cap_hit()` |

**Parameter order is reversed**, and token types are `u64` (not `u32`).

#### `ExecutionParams` field mismatch (Moderate)

Doc shows a simplified struct with ~8 fields. Code has 15+ fields:
- Missing from doc: `context_window`, `context_update_queue`, `pause_context_updates`, `hook_engine`, `cache_enabled`, `injector_registry`, `on_iteration`, `original_message`, `planning_prompt`, `pipeline_timeout`, `max_fabrication_retries`
- Doc shows `model: String`, `temperature: f32`, `max_tokens: u32` — these are actually on `ChatParams` (nested), not flat on `ExecutionParams`.

#### `CycleOutcome` variants simplified (Minor)

| Doc | Code |
|-----|------|
| `ToolsExecuted { count: u32 }` | `ToolsExecuted { results: Vec<ToolExecutionResult> }` |
| `FabricatedResponse { content, reason }` | `FabricatedResponse { content }` |

#### `AgentEvent` count underreported (Minor)

Doc says "~30 variants". Code has ~50 variants (counted from `events.rs`). Doc significantly under-represents the enum surface.

#### `SpawnParams` fields mismatch (Moderate)

| Doc | Code |
|-----|------|
| `task_id: String` | `description: String` |
| `task_description: String` | `prompt: String` |
| `agent_profile: String` | `model: Option<String>` |
| `workspace_cwd: PathBuf` | `workspace_path: PathBuf` |
| `depth: DepthMode` | `max_turns: Option<u32>` |
| *(missing)* | `parent_session_id: String`, `parent_agent_id: Option<String>` |

---

## `app-core` Crate (`docs/architecture/crates/app-core.md`)

### ✅ Verified Correct

| Claim | Source | Result |
|-------|--------|--------|
| `AppCore` ~50 fields, transport-agnostic | `app-core/src/state.rs` | ✅ |
| `StreamGuard` value-identity Drop | `runtime/mod.rs` | ✅ |
| `STREAM_GUARD_COUNTER: AtomicU64` | `runtime/mod.rs` | ✅ |
| `DesktopApprovalChannel` 600s timeout + oneshot | `desktop_approval_channel.rs` | ✅ |
| `ActiveTurns` + `ActiveTurnEntry` with `guard_id` | `runtime/mod.rs` | ✅ |
| Handler ~40 domain modules | `handlers/` tree | ✅ |
| `coding/` directory separate from `handlers/` | File tree | ✅ |

### ❌ Drift — Public API

#### `ThreadRuntime` trait signature (Major)

| Doc | Code |
|-----|------|
| `start_turn(&self, params: StartTurnParams) -> Result<TurnHandle>` | `start_turn(&self, req: StartTurnRequest) -> Result<StartTurnOutcome, ApiError>` |
| `is_active(&self, thread_id: &str) -> bool` | `is_active(&self, turn_id: &str) -> bool` |
| `active_turns(&self) -> Vec<String>` | `active_turns(&self) -> &ActiveTurns` |

`TurnHandle` in doc has `turn_id` + `cancel_token`. Code's `StartTurnOutcome` contains `thread_id`, `turn_id`, `generation`, `cancel_token`, `event_rx`.

#### `AppCore::init_with_sender` parameter wrapping (Major)

| Doc | Code |
|-----|------|
| `event_emitter: Arc<dyn AppEventEmitter>` | `event_emitter: Option<Arc<dyn AppEventEmitter>>` |
| `approval_channel: Arc<dyn ApprovalChannel>` | `_approval_channel: Option<Arc<dyn ApprovalChannel>>` |
| `config: Config` | `config_override: Option<config::Config>` |
| Returns `Result<Arc<Self>>` | Returns `Result<(Self, EventChannels), String>` |

Doc shows bare `Arc<…>`; code wraps both in `Option`. Return type is wrong — code returns a tuple with `EventChannels`.

#### `AppCore` field type mismatches (Moderate)

| Doc | Code |
|-----|------|
| `desktop_approval_channel: Arc<DesktopApprovalChannel>` | `desktop_approval_channel: Option<Arc<DesktopApprovalChannel>>` |
| `coding_policies: Arc<RwLock<CodingPolicies>>` | `coding_policies: Arc<DashMap<String, Arc<RwLock<CodingApprovalPolicy>>>>` |
| `pending_interactions: Arc<DashMap<String, (String, oneshot::Sender<FormResponse>)>>` | Not found in `state.rs` — may be on `StreamGuard` or elsewhere |

#### Init sequence oversimplified (Moderate)

Doc presents init as "14 phases" in a clean table. Actual `init/mod.rs`:
- Phases run concurrently (e.g., Productivity + Launcher)
- Sub-phases exist within coding init (10a–10d)
- Feature gates (`KCA_VECTOR=1` for cognitive embedder) control optional construction
- Subagent zombie sweep at startup
- `event_emitter` and `_approval_channel` are `Option`-wrapped, not mandatory

The table is a useful overview but should be labeled as simplified.

#### `coding/title_service.rs` inaccuracy (Minor)

Doc claims `title_service.rs` has `// TODO: LLM call` stub at L50. In fact, `autogenerate_title` is fully implemented with an actual LLM call (`providers::DynProvider::chat`). The TODO comment does not exist.

#### `AppEventEmitter` method signatures (Minor)

Doc shows `emit(&self, event: &str, payload: &serde_json::Value)` (2 params). Need to verify actual trait definition — may have additional params or different names.

---

## `context_engine` Crate (`docs/architecture/crates/context_engine.md`)

### ✅ Verified Correct

| Claim | Source | Result |
|-------|--------|--------|
| `LOW_BUDGET_THRESHOLD = 0.15` | `budget.rs:6` | ✅ |
| `COMPACTABLE_TOOLS` list | `history_compressor/tiered.rs` | ✅ |
| `KCA_DISABLE_COMPRESSION` escape hatch | `history_compressor/tiered.rs` | ✅ |
| Microcompact pre-pass (150-char snippets) | `history_compressor/tiered.rs` | ✅ |
| `TieredHistoryCompressor` extractive-first, LLM fallback | `history_compressor/tiered.rs` | ✅ |
| `ContextSource` trait definition | `source.rs` | ✅ |
| `default_token_counter()` returns `Arc<dyn TokenCounter>` | `token_counter.rs:67` | ✅ |
| `best_token_counter()` tries Tiktoken, falls back to Char | `token_counter.rs:100` | ✅ |
| `token_counter_for_model("claude")` → Anthropic | `token_counter.rs:118` | ✅ |
| `estimate_message_tokens` shared helper | `token_counter.rs:75` | ✅ |
| `InsightForge` RRF merge with `k=60` | `insight_forge/mod.rs:76` | ✅ |
| `InsightForge` 60% per-source budget | `insight_forge/mod.rs:352` | ✅ |
| `SourceContext` has `intent_summary: Option<String>` (vestigial) | `source.rs:21` | ✅ |
| `build_system_prompt` joins with `\n\n---\n\n` | `assembler/mod.rs` | ✅ |
| `version` incremented on `expand()` | `assembler/mod.rs` | ✅ |

### ❌ Drift — Public API

#### `BudgetAllocator::Priority` enum completely wrong (Critical)

| Doc | Code |
|-----|------|
| `Critical = 0` | `SystemIdentity = 0` |
| `VeryHigh = 1` | `ActiveTask = 1` |
| `High = 2` | `ToolDefinitions = 2` |
| `AboveNormal = 3` | `RecentHistory = 3` |
| `Normal = 4` | `RetrievedMemory = 4` |
| `BelowNormal = 5` | `CompressedHistory = 5` |
| `Low = 6` | `BootstrapPersona = 6` |
| `VeryLow = 7` | `Skills = 7` |

The doc describes a generic priority ladder. The code uses **semantic priority names** tied to context categories. This is a fundamental mismatch — anyone reading the doc to understand budget allocation will be completely misled.

#### `BudgetReport` fields mismatch (Major)

| Doc | Code |
|-----|------|
| `total: usize` | `total_window: usize` |
| `used: usize` | `total_allocated: usize` |
| `remaining: usize` | `remaining: usize` ✅ |
| `low_budget: bool` | *(not present)* |
| `by_priority: HashMap<Priority, usize>` | `per_priority: Vec<(Priority, usize)>` |

Doc has `low_budget` flag and `HashMap`; code uses `Vec` of tuples and has no flag.

#### `TieredHistoryCompressor::compress` ignores `budget_tokens` (Major)

Doc signature: `compress(&self, history: &[Message], budget_tokens: usize, tier0_count: usize)`

Code signature: `compress(&self, history: &[Message], _budget_tokens: usize, tier0_count: usize)`

The `_budget_tokens` parameter is **unused** (underscore prefix). The doc implies it's load-bearing; it's actually vestigial.

#### `CompressedHistory.preamble` type mismatch (Major)

Doc: `preamble: Option<String>`
Code: `preamble: Vec<Message>`

#### `TierSummary` fields mismatch (Major)

| Doc | Code |
|-----|------|
| `turn_indices: Vec<usize>` | `turn_range: (usize, usize)` |
| `summary_text: String` | `content: String` |
| `tokens: usize` | `token_count: usize` |
| *(missing)* | `cognitive_score: f64` |

#### `AssignedTier` missing `Verbatim` variant (Moderate)

Doc: `AssignedTier { Detailed, Condensed }`
Code: `AssignedTier { Verbatim, Detailed, Condensed }`

#### `ContextRequest` fields mismatch (Major)

| Doc | Code |
|-----|------|
| `message: String` | `message_text: String` |
| `session_key: Option<String>` | `session_key: Option<String>` ✅ |
| `session_mode: SessionMode` | *(not present — passed via `build_system_prompt`)* |
| `channel: ChannelName` | *(not present)* |
| `chat_id: ChatId` | *(not present)* |
| `history: Vec<Message>` | `history: Vec<Message>` ✅ |
| `strategy: ExecutionStrategy` | `strategy: ExecutionStrategy` ✅ |
| `retrieval_context: Option<RetrievalContext>` | `retrieval_context: Option<RetrievalContext>` ✅ |
| `user_situation: Option<UserSituationSnapshot>` | *(not present)* |
| *(missing)* | `system_prompt: String` |
| *(missing)* | `tool_definitions: Vec<serde_json::Value>` |
| *(missing)* | `context_window: usize` |
| *(missing)* | `enhancement_budget: EnhancementBudget` |
| *(missing)* | `tier0_count: Option<usize>` |

#### `AssembledContext` fields mismatch (Moderate)

| Doc | Code |
|-----|------|
| `system_prompt: String` | *(not present)* |
| `messages: Vec<Message>` | `messages: Vec<Message>` ✅ |
| `total_tokens: usize` | `token_count: usize` |
| `budget_report: BudgetReport` | `budget_report: BudgetReport` ✅ |
| `compression_stats: CompressionStats` | `compression_stats: Option<CompressionStats>` ✅ |
| `enhancement_trace: Option<EnhancementTrace>` | `enhancement_trace: Option<EnhancementTrace>` ✅ |
| `sources_used: Vec<String>` | *(not present)* |
| *(missing)* | `inventory: ContextInventory` |
| *(missing)* | `budget_remaining: usize` |
| *(missing)* | `version: u32` |
| *(missing)* | `retrieved_memory_count: usize` |

#### `MemoryRetriever` trait signature mismatch (Major)

| Doc | Code |
|-----|------|
| `retrieve(&self, query: &str, session_key: Option<&str>, ctx: Option<&RetrievalContext>) -> Result<Vec<MemoryEntry>>` | `retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry>` |

Code has no `session_key`, no `ctx`, no `Result` wrapper, and adds `limit: usize`.

#### `MemoryEntry` fields mismatch (Moderate)

| Doc | Code |
|-----|------|
| `created_at: Timestamp` | *(not present)* |
| `metadata: Value` | *(not present)* |
| *(missing)* | `raw_score: f64` |

#### `MemorySource` variants mismatch (Moderate)

| Doc | Code |
|-----|------|
| `Semantic` | `CognitiveFact` |
| `Episodic` | `ConversationRecall` |
| `Notes` | `EpisodicMemory` |
| `Procedural` | *(no equivalent)* |
| `Task` | *(no equivalent)* |
| `Custom(String)` | `Domain { name: String }` |

#### `TokenCounter` trait signature mismatch (Moderate)

| Doc | Code |
|-----|------|
| `count_tokens(&self, text: &str) -> usize` | `estimate_text(&self, text: &str) -> usize` |
| `count_messages(&self, messages: &[Message]) -> usize` | *(not on trait — `estimate_message_tokens` is a free function)* |

#### `SourceContext` fields mismatch (Moderate)

| Doc | Code |
|-----|------|
| `session_key: Option<String>` | *(not present)* |
| `user_situation: Option<UserSituationSnapshot>` | *(not present)* |
| *(missing)* | `project_id: Option<String>` |

#### `ContextEngine::assemble_with_prefetched` signature mismatch (Major)

| Doc | Code |
|-----|------|
| `assemble_with_prefetched(request, prefetched: Option<(String, usize, Option<EnhancementTrace>)>)` | `assemble_with_prefetched(request, prefetched_memory: Option<MemoryRetrievalResult>)` |

The doc shows a raw tuple; code uses a typed `MemoryRetrievalResult`.

#### `ContextEngine::prefetch_memory` signature mismatch (Major)

Doc shows this method exists; need to verify in code — may not exist with this signature.

#### `CompressionStats` fields mismatch (Minor)

| Doc | Code |
|-----|------|
| `turns_compressed: usize` | `tier0_kept: usize` |
| `tokens_before: usize` | `tier1_tokens: usize` |
| `tokens_after: usize` | `tier2_tokens: usize` |
| `ratio: f32` | `cognitive_scoring_used: bool` |
| *(missing)* | `delta_only: bool` |

#### `ExecutionStrategy::AutonomousTask` vs code (Minor)

Doc: `AutonomousTask` (no fields)
Code: `AutonomousTask { max_iterations: u32 }`

---

## Minor / Cosmetic Issues

### `agent`
- `events.rs` comment says "~30 variants" in module map; actual count is ~50.
- `cognitive_handlers.rs` is documented as containing "LlmQueryPredictorHandler (KCA Track 7 predictive cache)" — this may be outdated; the file now contains extraction, consolidation, graph linking, and coaching handlers.

### `app-core`
- Doc says `init/mod.rs` has "14 phases" — actual ordering has concurrent groups and sub-phases. The table is useful but should be marked as simplified.
- `coding/recall_stats_handler.rs` has a TODO at L33 (`recall_invocations repo`); doc correctly notes this.

### `context_engine`
- Doc `ContextSource::estimated_tokens()` default is `0`; code default is `500`.
- Doc `ContextSource::protected()` is documented but correct (default `false`).

---

## Recommendations

1. **`context_engine` Priority enum** — Rewrite the `BudgetAllocator` section entirely. The current doc is actively misleading.
2. **`agent` AgentEvent** — Regenerate from `events.rs` or add a note that the doc shows a subset. Field names are critically wrong.
3. **`agent` LoopFinishReason** — Update variant names to match code (`Completed`, `SafetyTurnLimit`, `TokenLimit`).
4. **`app-core` ThreadRuntime** — Update to `StartTurnRequest` / `StartTurnOutcome`.
5. **`app-core` init_with_sender** — Fix parameter types (`Option<Arc<…>>`) and return type.
6. **`context_engine` MemoryRetriever** — Update trait signature to match code (no `Result`, no `session_key`, add `limit`).
7. **`context_engine` TokenCounter** — Rename `count_tokens` → `estimate_text` in doc.
8. **All crates** — Add a prominent note: "Public API signatures verified against source on 2026-05-16. Some fields may have drifted."

---

*Report generated by function-by-function comparison of architecture docs against `crates/{agent,app-core,context_engine}/src/` on 2026-05-16.*
