# Phase 2 Architecture Verification — Agent-2

**Verifier:** verify-agent-2  
**Docs:** `03-providers.md`, `04-agent-runtime.md`  
**Crates:** `providers`, `agent`, `context_engine`, `skill-system`  
**Date:** 2026-05-16

---

## Summary

| Crate | Status | Issues |
|---|---|---|
| `providers` | 🔴 Significant drift | 10+ factual errors / API mismatches |
| `agent` | 🟢 Mostly accurate | Minor line-number offsets |
| `context_engine` | 🟢 Accurate | Minor line-number offsets |
| `skill-system` | 🟢 Accurate | Minor line-number offsets |

**Total issues found:** 16 (4 critical factual errors, 6 API signature drifts, 6 line-number drifts)

---

## Per-Crate Findings

### `providers` (crate: `crates/providers/`)

#### ✅ Accurate
- **Module/file existence:** All claimed files exist (`lib.rs`, `types.rs`, `adapters/mod.rs`, `adapters/anthropic_native.rs`, `adapters/openai_compat.rs`, `adapters/transcription.rs`, `adapters/noop.rs`, `manager.rs`, `registry.rs`, `factory.rs`, `catalogue.rs`, `streaming.rs`, `testing.rs`).
- **ProviderRole enum:** `Distiller`, `ReforgeSynth`, `ReforgeRules` exist with correct variants.
- **Adapter re-exports:** `AnthropicNativeProvider`, `OpenAiCompatProvider`, `TranscriptionProvider`, `NoopProvider` all exported from `adapters/mod.rs`.
- **ProviderRegistry / PROVIDERS static:** Exists with `find_by_name`, `find_by_model`, `find_gateway`, `resolve_model`.
- **Cache breakpoint synthesis (behavior):** `anthropic_native.rs` lines 203–211 synthesize `LastSystem + Ephemeral` when no explicit breakpoints and `cache_system_prompt` is true. The doc's behavioral description is correct even though the exact function name differs.
- **Streaming is first-class:** `LlmStream` type alias exists (`Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>`).
- **Catalogue helpers:** `catalogue.rs` exists and is used by registry.
- **Test helpers:** `testing.rs` has `SingleResponseProvider`.
- **Dependencies:** `Cargo.toml` lists `reqwest`, `eventsource-stream` is not present but `futures-util` is used for SSE parsing via `streaming.rs`.

#### ⚠️ Drift
- **`DynProvider` type:** Doc claims `Box<dyn LlmProvider>`; actual code uses `Arc<dyn LlmProvider>` (`types.rs:419`). This is a significant type mismatch that affects every consumer.
- **`LlmProvider` trait method names:** Doc shows `chat_completion` / `chat_completion_stream`; actual code uses `chat` / `chat_stream` (`types.rs:310`, `320`).
- **`ProviderManager` fallback type:** Doc shows `fallback: Vec<DynProvider>`; actual code has `fallback: Option<DynProvider>` (`manager.rs:54`).
- **`CircuitBreakerConfig` fields:** Doc shows `failure_threshold: u32`, `cooldown: Duration`, `probe_timeout: Duration`; actual code has `failure_threshold: u32`, `reset_timeout_secs: u64` (`manager.rs:18–23`).
- **`DegradationLevel` variants:** Doc lists `Healthy`, `Slow`, `FailingOver`, `Exhausted`; actual code has only `Fallback`, `Offline` (`manager.rs:40–45`).
- **Factory signatures:**
  - `create_provider`: Doc shows `create_provider(spec: &ProviderSpec, config: &Config) -> Result<DynProvider, ProviderError>`; actual code is `create_provider(config: &Config) -> Result<(DynProvider, String)>` (`factory.rs:25`).
  - `create_cognitive_provider`: Doc shows `create_cognitive_provider(role: ProviderRole, config: &Config)`; actual code is `create_cognitive_provider(config: &Config) -> Result<Option<DynProvider>>` with no `role` parameter (`factory.rs:199`).
  - `cognitive_chat_params`: Doc shows `cognitive_chat_params(role: ProviderRole) -> ChatParams`; actual code is `cognitive_chat_params(config: &Config, default_max_tokens: u32) -> ChatParams` (`factory.rs:233`).
- **`CacheAnchor` variants:** Doc lists `LastSystem | LastUser | LastTool | LastN(usize)`; actual code has `LastSystem | LastTool | MessageIndex(usize)` — no `LastUser` or `LastN` (`types.rs:122–132`).
- **`ChatParams` fields:** Doc lists `temperature: f32`, `max_tokens: u32`, `tools`, `tool_choice`, `cache_breakpoints`, `cache_system_prompt`, `stop_sequences`, `reasoning_effort`; actual code has `temperature: Option<f32>`, `max_tokens: Option<u32>`, `response_format: Option<ResponseFormat>`, `role: Option<ProviderRole>`, `session_key: Option<String>` (`types.rs:143–154`).
- **`LlmStreamChunk` shape:** Doc describes it as an enum with variants `Text`, `ToolCallDelta`, `Usage`, `Reasoning`, `Done`, `Error`; actual code is a **struct** with fields `content`, `tool_call_delta`, `is_final`, `finish_reason`, `reasoning_content`, `usage` (`types.rs:59–78`). This is a complete structural mismatch.
- **`Message` enum variants:** Doc shows `System(String)`, `User(UserContent)`, `Assistant { content, tool_calls: Vec<ToolCall> }`, `Tool { tool_call_id, content: String }`, `ContextUpdate(String)`; actual code has `System { content: String }`, `User { content: UserContent }`, `Assistant { content, tool_calls: Option<Vec<ToolCallMessage>>, reasoning_content }`, `Tool { tool_call_id, name, content: ToolContent }`, `ContextUpdate { reason, content }` (`types.rs:526–555`).
- **`ToolCallDelta` field name:** Doc shows `arguments_partial: Option<String>`; actual code has `arguments: Option<String>` (`types.rs:86`).
- **`DEFAULT_CONTEXT_WINDOW`:** Doc claims `200_000` for Anthropic; actual code has `128_000` (`types.rs:652`).
- **Cache breakpoint synthesis line reference:** Doc cites `anthropic_native.rs:192–206`; actual function `prepare_cache_markers` starts at line 196 and the synthesis logic is at lines 203–211.

#### ❌ Wrong
- The `LlmProvider` trait signature in the doc is entirely wrong (method names, parameter types, return types). The doc shows `async fn chat_completion(&self, messages: Vec<Message>, params: ChatParams) -> Result<LlmResponse, ProviderError>` and `fn chat_completion_stream(...) -> LlmStream`, but the actual trait uses `&[Message]`, `&ChatParams`, `Option<&[Value]>` for tools, `&[CacheBreakpoint]`, and returns `Result<LlmResponse>` / `Result<LlmStream>`.
- The doc claims `Message::Tool.content` is "plain String today — no image-bearing schema", but the actual code has `ToolContent` enum with `Text(String)` and `MultiPart(Vec<ToolContentPart>)` including `ImageData` (`types.rs:597–609`). This contradicts the open-questions note that says `ContentPart::ImageData` is planned.

#### 🔍 Missing (in code, not in docs)
- `LlmProvider` trait has additional methods not documented: `supports_streaming()`, `default_model()`, `count_tokens()`, `context_window()`, `health_check()`, `classifier_provider()`, `list_models()`.
- `ChatParams` has builder methods (`new`, `with_temperature`, `with_max_tokens`, `with_response_format`, `with_role`, `with_session_key`) not mentioned.
- `ProviderCapabilities` has additional fields: `extended_thinking`, `structured_outputs`, `prompt_caching`, `explicit_cache_markers`, `native_token_counting`, `vision`, `streaming`, `tool_choice_required`, `parallel_tool_calls`.
- `Usage` has `cache_read_tokens` and `cache_write_tokens` fields not mentioned in the doc.

#### 📋 Tech Debt
- No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` found in `providers` crate.

---

### `agent` (crate: `crates/agent/`)

#### ✅ Accurate
- **All claimed files exist:** `lib.rs`, `agent_loop/mod.rs`, `agent_loop/builder.rs`, `agent_runtime/runtime.rs`, `execution/core.rs`, `execution/execute_loop.rs`, `execution/budget.rs`, `execution/types.rs`, `execution/mid_loop_compressor.rs`, `execution/live_context_refresher.rs`, `execution/loop_detector.rs`, `execution/cache_policy.rs`, `subagent_runtime.rs`, `subagent.rs`, `subagent_events.rs`, `events.rs`, `context_sources/`, `adapters/llm_summary.rs`, `adapters/cognitive_handlers.rs`, `confidence/`, `learning/`, `output/cost_tracker.rs`, `output/validator.rs`.
- **`AgentLoop`:** Exists with `run_with_rx`, focus-session deferral, correction detection.
- **`AgentRuntime`:** 3-phase pipeline (Prepare → Execute → Record) confirmed in `runtime.rs`.
- **`ExecutionCore`:** `run_cycle` exists with streaming, dedup, fabrication detection, approval gate.
- **`execute_loop`:** ReAct iteration confirmed with cancellation, mid-loop compression, LiveContextRefresher, LoopDetector, SafetyCap.
- **`SubagentRuntime`:** `spawn`, `spawn_detached`, `resume`, `kill` all exist.
- **Constants verified:**
  - `MAX_CONCURRENT_TOOLS = 10` (`execution/core.rs:60`) ✅
  - `MAX_TOOL_RESULT_LENGTH = 50_000` (`execution/core.rs:65`) ✅
  - `LONG_RUNNING_TOOL_TIMEOUT = 600 s` (`execution/core.rs:54`) ✅
  - Default `tool_timeout = 30 s` (`execution/types.rs:74`) ✅
  - `COMPRESSION_THRESHOLD = 0.70` (`execution/mid_loop_compressor.rs:15`) ✅
  - `MIN_RECENT_MESSAGES = 8` (`execution/mid_loop_compressor.rs:18`) ✅
  - `MIN_COMPRESSIBLE_TOKENS = 50` (`execution/mid_loop_compressor.rs:24`) ✅
  - `DEFAULT_TURN_CAP = 500` (`subagent_runtime.rs:20`) ✅
  - `CORRECTION_WINDOW_MINUTES = 15` (`agent_loop/mod.rs:28`) ✅
- **`SafetyCap::new(depth)` sets `max_turns = u32::MAX`** confirmed (`execution/budget.rs:100`).
- **`LoopDetector` thresholds:** Warning at 3, HardStop at 5 confirmed (`execution/loop_detector.rs:54`, `63`, `64`).
- **Fabrication detection skipped in coding mode:** Confirmed in `execution/core.rs:613` (`in_coding = routing_ctx.channel.as_str() == common::tool_channel::CODING_CHANNEL`).
- **KCA Phase-4 env flags:** All three flags confirmed at `agent_runtime/runtime.rs:575–586`.
- **Predictive cache warming (KCA Track 7):** Confirmed in `agent_runtime/runtime.rs:813–853` with `predictor.predict_next(&recent_turn_text, n)`.

#### ⚠️ Drift
- **Line-number offsets for constants:**
  - `DEFAULT_TURN_CAP` doc says line 21; actual line 20.
  - `LOW_BUDGET_THRESHOLD` doc says `context_engine/src/budget.rs:7`; actual line 6.
  - `MAX_DESCRIPTION_CHARS` doc says `skill-system/src/store.rs:14`; actual line 15.
  - `CORRECTION_WINDOW_MINUTES` doc says `agent/src/agent_loop/mod.rs:27`; actual line 28.
- **`AgentEvent` variants:** The doc mentions `AgentEvent` but does not enumerate all ~50 variants actually present in `events.rs`. Some variants (e.g., `SkillActivationConsidered`, `SkillActivated`) are vestigial — defined but never emitted by the agent runtime itself.

#### ❌ Wrong
- None found.

#### 🔍 Missing (in code, not in docs)
- `ExecutionParams` has many additional fields not documented: `planning_prompt`, `pipeline_timeout`, `pause_context_updates`, `hook_engine`, `cache_enabled`, `injector_registry`, `on_iteration`.
- `AgentRuntime` has additional builder methods and fields (e.g., `micro_reforge_service`, `predictive_cache`, `query_predictor`) not mentioned in the file map.

#### 📋 Tech Debt
- No actual `TODO` / `FIXME` / `unimplemented!()` / `todo!()` found in `agent` crate source. One false positive: `notes_integration_tests.rs:98` uses `json!({"title": "TODO"})` as test data, and `context_sources/todo.rs` defines `TODO_CACHE_TTL_SECS` — neither are tech debt.

---

### `context_engine` (crate: `crates/context_engine/`)

#### ✅ Accurate
- **All claimed files exist:** `lib.rs`, `assembler/mod.rs`, `assembler/cache.rs`, `assembler/types.rs`, `budget.rs`, `history_compressor/tiered.rs`, `history_compressor/{grouping,snippet,types,mod}.rs`, `source.rs`, `enhancement/{pipeline,prf,heuristic_rerank,mod}.rs`, `insight_forge/{mod,circuit_breaker,decomposer,llm_decomposer}.rs`, `memory_retriever.rs`, `memory_scorer.rs`, `token_counter.rs`, `summary_provider.rs`, `ttl_cache.rs`, `inventory.rs`, `rewriter.rs`.
- **`ContextEngine`:** Exists with `build_system_prompt`, `assemble`, `register_source`, `expand`.
- **`BudgetAllocator`:** Exists with 8 `Priority` levels (`SystemIdentity=0` through `Skills=7`).
- **`LOW_BUDGET_THRESHOLD = 0.15`:** Confirmed at `budget.rs:6`.
- **`TieredHistoryCompressor`:** Confirmed with `KCA_DISABLE_COMPRESSION=1` escape hatch at `history_compressor/tiered.rs:71–74`.
- **`ContextSource` trait:** Exists with `name()`, `priority()`, `provide()`, `estimated_tokens()`, `protected()`.
- **`TokenCounter` implementations:** `CharTokenCounter`, `AnthropicTokenCounter`, `TiktokenCounter` all exist.
- **`MemoryRetriever` / `MemoryEntry` / `MemorySource`:** All exist.

#### ⚠️ Drift
- **Line-number offset:** `KCA_DISABLE_COMPRESSION` doc says `tiered.rs:68`; actual logic starts at line 71.
- **`Priority` enum:** Doc says "8 priority levels" but the enum has 8 variants (0–7), which is correct. However, `BudgetConfig::standard` reserves 15% for response, not exactly matching the "15% low-budget warn" description (the warn is when remaining drops below 15% of available input, not 15% of total window).

#### ❌ Wrong
- None found.

#### 🔍 Missing (in code, not in docs)
- `ContextEngine::tier0_config()` method exists but is not documented.
- `AssembledContext` has additional fields: `inventory`, `budget_remaining`, `version`, `retrieved_memory_count`, `enhancement_trace`, `compression_stats`.

#### 📋 Tech Debt
- No `TODO` / `FIXME` / `unimplemented!()` / `todo!()` found in `context_engine` crate.

---

### `skill-system` (crate: `crates/skill-system/`)

#### ✅ Accurate
- **All claimed files exist:** `lib.rs`, `store.rs`, `listing.rs`, `soul.rs`, `defaults.rs`, `parser.rs`, `persona.rs`, `types.rs`.
- **`SkillStore`:** Loads `.md` / `SKILL.md` from disk; installs 6 built-in defaults.
- **`DEFAULT_SKILLS`:** Exactly 6 skills confirmed in `store.rs:18–43`: `task-management`, `finance-management`, `automation`, `notebook`, `learning`, `coding-orchestrator`.
- **`MAX_DESCRIPTION_CHARS = 250`:** Confirmed at `store.rs:15`.
- **`SoulContextSource`:** Priority 50, protected, mtime-cached live read — all confirmed in `soul.rs`.
- **`SkillListingSource`:** Priority 40, protected — confirmed in `listing.rs`.
- **`DEFAULT_SOUL` / `DEFAULT_CODING_SOUL`:** Confirmed in `soul.rs`.
- **`compiled_skill_defaults()`:** Confirmed in `defaults.rs`.

#### ⚠️ Drift
- **Line-number offset:** `MAX_DESCRIPTION_CHARS` doc says `store.rs:14`; actual line 15.

#### ❌ Wrong
- None found.

#### 🔍 Missing (in code, not in docs)
- `SkillFrontmatter` has additional fields: `scope`, `scope_repo_id`, `references` not mentioned.
- `SkillStore::list_for_scope()` method exists but not documented.

#### 📋 Tech Debt
- No `TODO` / `FIXME` / `unimplemented!()` / `todo!()` found in `skill-system` crate.

---

## Cross-Reference Check

| Link | In Doc | Target Exists? | Status |
|---|---|---|---|
| `../00-overview.md` | `03-providers.md` | ✅ `docs/architecture/00-overview.md` | OK |
| `./01-foundations.md` | `03-providers.md`, `04-agent-runtime.md` | ✅ | OK |
| `./04-agent-runtime.md` | `03-providers.md` | ✅ | OK |
| `./05-cognitive-memory.md` | `03-providers.md`, `04-agent-runtime.md` | ✅ | OK |
| `./11-channels-mcp.md` | `03-providers.md` | ✅ | OK |
| `../crates/providers.md` | `03-providers.md` | ✅ `docs/architecture/crates/providers.md` | OK |
| `./03-providers.md` | `04-agent-runtime.md` | ✅ | OK |
| `./07-tools-framework.md` | `04-agent-runtime.md` | ✅ | OK |
| `./10-sandboxing-security.md` | `04-agent-runtime.md` | ✅ | OK |
| `../crates/agent.md` | `04-agent-runtime.md` | ✅ `docs/architecture/crates/agent.md` | OK |
| `../crates/context_engine.md` | `04-agent-runtime.md` | ✅ `docs/architecture/crates/context_engine.md` | OK |
| `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md` | `03-providers.md` | ✅ | OK |
| `../TECH_DEBT.md` | Both docs | ✅ `docs/architecture/TECH_DEBT.md` | OK |

**All cross-references resolve correctly.**

---

## Recommendations

1. **`providers` doc needs a full rewrite** of the API reference section. The `LlmProvider` trait, `DynProvider`, `Message`, `ChatParams`, `LlmStreamChunk`, and factory signatures have all changed significantly since the doc was written.
2. **Update `DEFAULT_CONTEXT_WINDOW`** from `200_000` to `128_000`.
3. **Update line-number citations** for `agent`, `context_engine`, and `skill-system` constants (minor drift of 1–2 lines).
4. **Remove or update the note** about `Message::Tool.content` being "plain String" — `ToolContent` now supports multipart including `ImageData`.
5. **Remove or update the `ContentPart::ImageData` note** — while `ContentPart` still doesn't have `ImageData`, `ToolContentPart::ImageData` does exist.
