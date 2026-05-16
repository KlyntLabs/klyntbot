# Architecture Docs Verification — Final Report

> **Synthesized from:** 13 agent deliverables (inventory + 7 Phase-2 subsystem agents + 3 Phase-3 critical-crate agents + 2 Phase-4 tech-debt agents)  
> **Date:** 2026-05-16  
> **Commit verified:** `575b7014c`

---

## Executive Summary

| Metric | Value |
|---|---|
| **Total crates/packages** | **66** (64 workspace members + root `klyntbot` facade + excluded `plugin-sdk`) |
| **Actual compilable workspace members** | **64** (one entry `crates/desktop-ui` has no `Cargo.toml` and was removed from the workspace members list but the orphaned directory remains) |
| **Crates with dedicated deep-dive docs** | **11** (`agent`, `app-core`, `coding-ingest`, `coding-memory`, `cognitive`, `context_engine`, `desktop`, `mcp`, `providers`, `storage`, `tools-core`) |
| **Crates covered in subsystem docs** | **66** (every crate is mentioned in at least one `subsystems/NN-*.md`) |
| **Crates with NO architecture doc mention** | **0** |
| **Total findings** | **~420 verified accurate claims**, **~95 drift items**, **~25 wrong claims**, **~45 missing items** |
| **Overall health score** | **~72%** (weighting: accurate = 1.0, drift = 0.6, wrong = 0.0, missing = 0.3) |

**Bottom line:** The architecture doc system is a genuinely useful navigation spine, but the **critical-crate deep-dives (`crates/*.md`) have suffered significant API drift** — especially `providers`, `context_engine`, `agent`, `coding-memory`, and `cognitive`. The subsystem docs (`subsystems/NN-*.md`) fare better but still contain outdated file maps, wrong constant values, and stale stub/status claims. The `TECH_DEBT.md` inventory is mostly accurate (74 of 75 entries still valid) but misses 16 new findings uncovered by this verification.

---

## Per-Subsystem Health Score

| # | Subsystem | Status Badge Claim | Verdict | Issues |
|---|-----------|-------------------|---------|--------|
| 01 | Foundations | 🟢 Stable | 🟡 Drift | `common`: `memory.rs` purpose is wrong (purge hooks, not FSRS5); `MessageBus` described as MPMC but is mpsc; `ContextUpdateQueue` is `Mutex<VecDeque>` not channel. `config`: `HotConfig` field names drifted (`max_tool_iterations`, `safety_timeout_secs`, etc.). `bus`: `MessageBus` channel type imprecise. |
| 02 | Storage | 🟢 Stable | 🟡 Drift | `storage`: `circuit_breaker.rs` is factually wrong (claims per-repo breaker with half-open probes; reality is a single global deadline table). Repo count is ~53, doc says "~52". `VectorStore` API hand-wavy. `session`: accurate. |
| 03 | Providers | 🟢 Stable | 🔴 Wrong / Major Drift | `providers`: Entire `LlmProvider` trait doc is wrong (method names, signatures, `Box<dyn>` vs `Arc<dyn>`, missing methods). `ProviderCapabilities` field set completely mismatched. `ChatParams`, `CacheAnchor`, `DegradationLevel` all wrong. `DEFAULT_CONTEXT_WINDOW` doc says 200K, code is 128K. |
| 04 | Agent Runtime | 🟡 In Progress | 🔴 Major Drift | `agent`: `AgentEvent` ~50 variants but doc lists ~20 with wrong field names. `LoopFinishReason`, `ExecuteLoopResult`, `SafetyCap`, `ExecutionParams`, `SpawnParams` all have field/name mismatches. `context_engine`: **`Priority` enum is completely wrong** (doc says generic ladder; code uses semantic names). `BudgetReport`, `TieredHistoryCompressor`, `MemoryRetriever`, `TokenCounter`, `AssembledContext` all have signature/type mismatches. `skill-system`: accurate. |
| 05 | Cognitive Memory | 🟡 In Progress | 🟡 Drift | `cognitive`: `run_reforge` doc claims 26 params, code has 25. `UnifiedMemoryService` public API mis-documented. `RecallConfig`, `SituationInputs`, many repo methods have signature drift. Two name-collision traits correctly noted. `ai-core` / `ai-core-macros` / `autotuner`: minor drift only. |
| 06 | Scheduling | 🟡 In Progress | 🟡 Drift | `scheduling`: 3 `RecurrenceEngine` trait signatures wrong (`decrement_count` returns `Option<u32>`, `create_instance` returns `CreateInstanceOutcome`, `cancel_unfired_instances` returns `()`). Stale "CronService" log message confirmed. |
| 07 | Tools Framework | 🟢 Stable | 🟡 Drift | `tools-core`: `ToolRegistry::list_meta` and `by_category` return types wrong. `ToolCallInterceptor` documents `after`/`run_after` methods that do not exist; actual trait only has `before_call`. `tools-core-macros` / `tools`: accurate. |
| 08 | Assistant Features | 🟢 Stable | 🟡 Drift | `feature-finance`: claims 57 actions, actual enum has 64. `feature-productivity`: claims 20 tables, actual is 21–22. `voice-engine`: `tone_analyzer` is not a pure stub (has heuristic logic). 5 TODOs in voice pronunciation pipeline. `feature-language-learning`: 3 TODOs. Others accurate. |
| 09 | Coding Mode | 🟡 In Progress | 🔴 Wrong / Major Drift | **`coding-memory`: Doc claims 4 Reforge phases are all stubbed — FALSE.** `SessionEndPass` and `CrossSessionDedup` are fully implemented; only `CodingSynthesisPhase` and `RuleArtifactGenerationPhase` in `reforge_phase.rs` remain stubs. Doc also claims "no physical DELETE ever runs" — FALSE; `SessionEndPass` calls `delete_by_id`. Module tree is heavily mis-documented. `coding-ingest`: `EventKind` count is 22 not 21; many variant fields wrong. `klynt-core`: `session_key` type wrong, `LONG_RUNNING_TOOL_TIMEOUT` in wrong crate. `lsp-client`: all methods stubs (5 `TODO(T5)`). `feature-coding-todo`: 312 TODOs. `feature-coding-bash`: 18 TODOs. |
| 10 | Sandboxing | 🟢 Stable | 🟡 Drift | `approval`: file map lists non-existent `src/hook.rs` and `src/suggester.rs`. `klynt-process-hardening`: doc falsely claims "no tests"; two unit tests exist. |
| 11 | Channels & MCP | 🟡 In Progress | 🟡 Drift | `mcp`: `start_health_check` documented on wrong struct (actually on `McpManager`). `is_server_allowed` doesn't exist; actual method is `decide() -> AllowDecision`. `McpApprovalChannel::request` returns `ApprovalDecision` directly, not `Result`. Circuit breaker cooldown is 60s not 30s. `notifications`: TG/DC/Email unwired confirmed. `activity-log`: 3 type mismatches (`i64` vs `u64`, `Option<String>` vs `String`, `String` vs `ResourceEdgeType`). |
| 12 | Plugins & Platform | 🟠 Scaffolded | 🟡 Drift | `plugin-runtime`: host-function count is 12, doc claims 14. File map lists non-existent `src/sandbox.rs`. `platform-macos`: file name `ax_walker.rs` should be `ax_tree.rs`. 2 TODOs. |
| 13 | Desktop & Frontend | 🟢 Stable | 🟡 Drift | `desktop`: `main()` return type is `()` not `Result`. CI guard test count is 5, doc claims 4. `desktop-ui`: feature directory count is 32 not 33. 9 TODOs in TS tests. `app-core`: `init_with_sender` params and return type wrong in doc. `ThreadRuntime` trait signatures wrong. 11 TODOs/unimplemented in app-core. |
| 14 | Validation | 🟡 In Progress | 🟡 Drift | `kca-e2e`: 3 fixture files asserted in unit test do not exist on clean checkout (`longmembench_subset.jsonl`, `klynt_coding_bench.jsonl`, `hallucination_planted.jsonl`). `kca-bench`: `full_pipeline` bench is a stub. TTFT gate is no-op skeleton. |

---

## Per-Crate Coverage Map

| Crate | Dedicated Doc | Subsystem Mention | Coverage Depth | Verdict |
|---|---|---|---|---|
| `activity-log` | ❌ | 11-channels-mcp.md | Medium | Accurate; minor type field mismatches |
| `agent` | ✅ | 04-agent-runtime.md + 5 others | Deep | 🔴 Major API drift (~50 variants, wrong field names, renamed types) |
| `ai-core` | ❌ | 05-cognitive-memory.md + 2 others | Medium | Accurate; 1 missing file mention |
| `ai-core-macros` | ❌ | 05-cognitive-memory.md | Medium | Minor drift (`entity_bridge` vs `entity`, `coaching_signal` vs `coaching`) |
| `analytics` | ❌ | 08-assistant-features.md | Medium | Accurate |
| `app-core` | ✅ | 13-desktop-frontend.md + 5 others | Deep | 🔴 Major drift (`ThreadRuntime`, `init_with_sender` signatures, init oversimplified) |
| `approval` | ❌ | 10-sandboxing-security.md + 3 others | Medium | File map lists non-existent files |
| `autotuner` | ❌ | 05-cognitive-memory.md + 1 other | Medium | Minor drift (`CycleResult.health` is `Option`) |
| `bus` | ❌ | 01-foundations.md + 7 others | Medium | `MessageBus` channel type imprecise |
| `channels` | ❌ | 11-channels-mcp.md | Medium | Accurate; missing `shared.rs` mention |
| `coding-agents-md` | ❌ | 09-coding-mode.md | Medium | Accurate |
| `coding-ingest` | ✅ | 09-coding-mode.md | Deep | 🟡 Drift (`EventKind` 22 not 21, variant fields wrong, module names wrong) |
| `coding-memory` | ✅ | 09-coding-mode.md + 2 others | Deep | 🔴 **Wrong claims** (reforge phase status, physical DELETE invariant) + major module drift |
| `cognitive` | ✅ | 05-cognitive-memory.md + 4 others | Deep | 🟡 Drift (`run_reforge` param count, many signature mismatches) |
| `common` | ❌ | 01-foundations.md + 9 others | Medium | 🟡 Drift (`ports.rs` vs `ports/`, `ChannelMask` naming); `memory.rs` description wrong |
| `config` | ❌ | 01-foundations.md + 6 others | Medium | `HotConfig` field names drifted |
| `context_engine` | ✅ | 04-agent-runtime.md | Deep | 🔴 **Critical drift** (`Priority` enum completely wrong; `BudgetReport`, `MemoryRetriever`, `TokenCounter`, many more) |
| `desktop` | ✅ | 13-desktop-frontend.md | Deep | Minor drift (return types, test count) |
| `desktop-macros` | ❌ | 13-desktop-frontend.md | Medium | Accurate |
| `desktop-shared` | ❌ | 13-desktop-frontend.md | Medium | Accurate |
| `feature-alarms` | ❌ | 08-assistant-features.md + 1 other | Medium | Accurate |
| `feature-coaching` | ❌ | 08-assistant-features.md | Medium | Accurate |
| `feature-coding-bash` | ❌ | 09-coding-mode.md + 4 others | Medium | Accurate; 18 TODOs |
| `feature-coding-todo` | ❌ | 09-coding-mode.md + 1 other | Medium | Accurate; 312 TODOs |
| `feature-finance` | ❌ | 08-assistant-features.md | Medium | **Wrong** (57 actions claimed, actual 64) |
| `feature-focus` | ❌ | 08-assistant-features.md | Medium | Accurate |
| `feature-insights` | ❌ | 08-assistant-features.md + 1 other | Medium | Accurate |
| `feature-language-learning` | ❌ | 08-assistant-features.md | Medium | Accurate; 3 TODOs in pronunciation pipeline |
| `feature-launcher` | ❌ | 08-assistant-features.md | Medium | Accurate |
| `feature-learning` | ❌ | 08-assistant-features.md + 1 other | Medium | Accurate |
| `feature-notes` | ❌ | 08-assistant-features.md + 1 other | Medium | Accurate; tool actions under-documented |
| `feature-productivity` | ❌ | 08-assistant-features.md | Medium | Drift (20 tables claimed, actual 21–22) |
| `feature-tasks` | ❌ | 08-assistant-features.md + 2 others | Medium | Accurate |
| `kca-bench` | ❌ | 14-validation.md | Medium | Accurate; `full_pipeline` bench is stub |
| `kca-e2e` | ❌ | 14-validation.md | Medium | **Wrong** (3 missing fixture files cause test failure on clean checkout) |
| `klynt-core` | ❌ | 09-coding-mode.md + 3 others | Medium | Drift (register counts, `session_key` type, constant location) |
| `klynt-execpolicy` | ❌ | 09-coding-mode.md | Medium | Accurate |
| `klynt-git-utils` | ❌ | 09-coding-mode.md | Medium | Drift (excluded dirs count: 8 not 7) |
| `klynt-hooks` | ❌ | 09-coding-mode.md + 2 others | Medium | Drift (`HookExecutionMode` lives in `klynt-protocol`; `fail_open` ignored) |
| `klynt-process-hardening` | ❌ | 10-sandboxing-security.md | Medium | **Wrong** (doc claims no tests; 2 tests exist) |
| `klynt-protocol` | ❌ | 09-coding-mode.md | Medium | Accurate |
| `klynt-pty` | ❌ | 09-coding-mode.md | Medium | Accurate |
| `klynt-sandbox` | ❌ | 10-sandboxing-security.md + 1 other | Medium | Accurate |
| `klynt-sandbox-helper` | ❌ | 10-sandboxing-security.md | Medium | Accurate |
| `klynt-skill-loader` | ❌ | 09-coding-mode.md + 1 other | Medium | Accurate |
| `klynt-truncation` | ❌ | 09-coding-mode.md | Medium | Accurate |
| `klyntbot-server` | ❌ | 13-desktop-frontend.md | Medium | Accurate |
| `lsp-client` | ❌ | 09-coding-mode.md | Medium | All methods stubbed (5 `TODO(T5)`) |
| `mcp` | ✅ | 11-channels-mcp.md + 1 other | Deep | 🟡 Drift (`start_health_check` location, `decide` vs `is_server_allowed`, return types, cooldown) |
| `mcp-bridge` | ❌ | 11-channels-mcp.md | Medium | Accurate; missing `CONNECT_TIMEOUT`/`WRITE_TIMEOUT` mention |
| `notifications` | ❌ | 11-channels-mcp.md + 1 other | Medium | Accurate; TG/DC/Email unwired confirmed |
| `platform-capture` | ❌ | 12-plugins-platform.md | Medium | Accurate |
| `platform-input` | ❌ | 12-plugins-platform.md | Medium | Accurate |
| `platform-macos` | ❌ | 12-plugins-platform.md + 1 other | Medium | Drift (`ax_tree.rs` vs `ax_walker.rs`); 2 TODOs |
| `plugin-runtime` | ❌ | 12-plugins-platform.md | Medium | Drift (12 host functions not 14; `sandbox.rs` doesn't exist) |
| `plugin-sdk` | ❌ | 12-plugins-platform.md | Medium | Accurate; `db_query` stub confirmed |
| `providers` | ✅ | 03-providers.md + 4 others | Deep | 🔴 **Major drift** (entire `LlmProvider` trait, `DynProvider`, `ProviderCapabilities`, constants) |
| `scheduling` | ❌ | 06-scheduling.md + 1 other | Medium | Drift (3 recurrence trait signatures wrong) |
| `session` | ❌ | 02-storage.md | Medium | Accurate |
| `skill-system` | ❌ | 04-agent-runtime.md | Medium | Accurate |
| `storage` | ✅ | 02-storage.md + 6 others | Deep | 🟡 Drift (`circuit_breaker.rs` description wrong; ~53 repos not ~52) |
| `tools` | ❌ | 07-tools-framework.md + 2 others | Medium | Accurate |
| `tools-core` | ✅ | 07-tools-framework.md + 6 others | Deep | 🟡 Drift (`ToolRegistry` return types, interceptor `after` doesn't exist) |
| `tools-core-macros` | ❌ | 07-tools-framework.md + 1 other | Medium | Accurate |
| `voice-engine` | ❌ | 08-assistant-features.md | Medium | Drift (`tone_analyzer` has heuristic, not stub); 5 TODOs |

**Phantom crate note:** `klyntbot` (root facade) is documented but has no `Cargo.toml` in `crates/` — it is the workspace root package. This is correctly handled by the doc system.

**Removed-from-workspace note:** `desktop-ui` is no longer in `Cargo.toml` workspace members and has no `Cargo.toml`. The directory `crates/desktop-ui/src/bindings.ts` is orphaned. The inventory incorrectly marks it "In Workspace: ✅".

---

## Critical Findings (P0 / P1)

### Wrong Claims (factually incorrect)

| # | File:Line | What doc says | What code says | Severity |
|---|-----------|---------------|----------------|----------|
| 1 | `subsystems/01-foundations.md` — `common` module map | `src/memory.rs` contains "FSRS5 + salience helpers used by cognitive" | `memory.rs` has `set_purge_hook` / `purge_freed_memory` — allocator hooks. No FSRS5 code. | P1 |
| 2 | `subsystems/02-storage.md` — `circuit_breaker.rs` | "Per-repo circuit breaker state (degrades writes after consecutive failures)" with half-open probes | Only 3 functions (`ensure_table`, `load`, `save`) persisting a single global `open_until_utc` deadline. No per-repo tracking, no failure counting, no half-open logic. | P0 |
| 3 | `subsystems/09-coding-mode.md` — `coding-memory` reforge | "4 Reforge phases return `NotImplementedInPhase { required_phase: 5 }`" — lists all 4 as stubs | Only 2 phases (`CodingSynthesisPhase`, `RuleArtifactGenerationPhase` in `reforge_phase.rs`) are stubs. `SessionEndPass` and `CrossSessionDedup` in `reforge/` are **fully implemented** with DB queries, LLM calls, and file I/O. | P0 |
| 4 | `subsystems/09-coding-mode.md` — `coding-memory` DELETE invariant | "No physical DELETE ever runs through either path" / "Both keep all rows on disk" | `SessionEndPass::run` calls `ep_repo.delete_by_id(id)` for within-session dedup and stale-candidate resolution. | P0 |
| 5 | `subsystems/08-assistant-features.md` — `feature-finance` | "57 actions" | `parameters()` enum in `tool/mod.rs` contains **64** distinct action strings. | P1 |
| 6 | `subsystems/10-sandboxing-security.md` — `klynt-process-hardening` | "security-critical but no tests" | `src/lib.rs` lines 122–162 contain 2 unit tests for `env_keys_with_prefix`. | P1 |
| 7 | `subsystems/10-sandboxing-security.md` — `approval` file map | Claims `src/hook.rs` and `src/suggester.rs` exist | `ClassifyHook` is in `src/policy.rs`; `ApprovalSuggester` is in `src/gate.rs`. | P1 |
| 8 | `crates/providers.md` — `LlmProvider` trait | Documents `chat_completion`, `chat_completion_stream`, `health`, `model`, `embed`; `DynProvider = Box<dyn>` | Actual: `chat`, `chat_stream`, `health_check`, `default_model`; no `embed`; `DynProvider = Arc<dyn>`; plus 5 additional methods. | P0 |
| 9 | `crates/providers.md` — `DEFAULT_CONTEXT_WINDOW` | Claims `200_000` for Anthropic | Actual: `128_000` (`types.rs:652`). | P1 |
| 10 | `crates/providers.md` — `DegradationLevel` | Lists `Healthy`, `Slow`, `FailingOver`, `Exhausted` | Actual: `Fallback`, `Offline`. | P1 |
| 11 | `crates/context_engine.md` — `BudgetAllocator::Priority` | `Critical=0` through `VeryLow=7` (generic ladder) | Actual: `SystemIdentity=0`, `ActiveTask=1`, `ToolDefinitions=2`, `RecentHistory=3`, `RetrievedMemory=4`, `CompressedHistory=5`, `BootstrapPersona=6`, `Skills=7` (semantic names). | P0 |
| 12 | `crates/agent.md` — `AgentEvent` variants | Lists ~20 variants with incorrect field names (e.g., `ContentChunk { text }`, `Reasoning { text }`, `ToolStart { args_preview }`) | Actual ~50 variants. `ContentChunk { data }`, `ReasoningChunk { data }`, `ToolStart { args: Value }`, etc. | P0 |
| 13 | `crates/agent.md` — `LoopFinishReason` | Lists `FinalResponse`, `FabricatedResponse`, `EmptyResponse`, `SafetyCap` | Actual: `Completed`, `Cancelled`, `LoopDetected`, `SafetyTurnLimit`, `TokenLimit`. | P1 |
| 14 | `crates/agent.md` — `LONG_RUNNING_TOOL_TIMEOUT` | Claims constant is in `klynt-core` | Actual: `crates/agent/src/execution/core.rs:54`. | P1 |
| 15 | `crates/app-core.md` — `ThreadRuntime` trait | `start_turn(&self, params: StartTurnParams) -> Result<TurnHandle>` | Actual: `start_turn(&self, req: StartTurnRequest) -> Result<StartTurnOutcome, ApiError>`. | P1 |
| 16 | `crates/app-core.md` — `AppCore::init_with_sender` | Params are bare `Arc<dyn …>`; returns `Result<Arc<Self>>` | Actual: both params are `Option<Arc<dyn …>>`; returns `Result<(Self, EventChannels), String>`. | P1 |
| 17 | `subsystems/14-validation.md` — `kca-e2e` fixtures | Asserts 3 fixture files exist | `longmembench_subset.jsonl`, `klynt_coding_bench.jsonl`, `hallucination_planted.jsonl` are **absent** from repo. Test fails on clean checkout. | P1 |
| 18 | `00-overview.md` — cross-cutting finding #2 | "4 Reforge phases in `coding-memory` are stubs" | Only 2 of 4 are stubs. `SessionEndPass` and `CrossSessionDedup` are fully implemented. | P1 |
| 19 | `inventory.md` — `desktop-ui` workspace status | "In Workspace: ✅" | `desktop-ui` is **not** in root `Cargo.toml` workspace members and has no `Cargo.toml`. | P1 |
| 20 | `subsystems/12-plugins-platform.md` — `plugin-runtime` | "14 host functions" | Actual count is **12**. | P1 |

### Significant Drift (outdated but not false)

| # | File:Line | What doc says | What code says | Severity |
|---|-----------|---------------|----------------|----------|
| 1 | `subsystems/01-foundations.md` — `HotConfig` | Fields: `max_iterations`, `pipeline_timeout`, `monthly_budget` | Actual: `max_tool_iterations`, `safety_timeout_secs`, `monthly_budget_usd`, plus `per_thread_cost_ceiling_usd`, `cost_alert_at_percent`. | P1 |
| 2 | `subsystems/01-foundations.md` — `MessageBus` | "async MPMC queue" | Actual: `tokio::sync::mpsc` (single consumer per direction). Senders are cloneable, but receivers are not. | P2 |
| 3 | `crates/providers.md` — `ProviderCapabilities` | 9 fields (`supports_tools`, `supports_streaming`, etc.) | Actual: 9 completely different fields (`extended_thinking`, `structured_outputs`, `prompt_caching`, etc.). | P1 |
| 4 | `crates/providers.md` — `ChatParams` | `temperature: f32`, `max_tokens: u32`, `tools`, `tool_choice`, etc. | Actual: `temperature: Option<f32>`, `max_tokens: Option<u32>`, `response_format`, `role`, `session_key`. | P1 |
| 5 | `crates/context_engine.md` — `TieredHistoryCompressor::compress` | Takes `budget_tokens: usize` and uses it | Actual: `_budget_tokens: usize` (underscore prefix — unused/vestigial). | P1 |
| 6 | `crates/context_engine.md` — `CompressedHistory.preamble` | `Option<String>` | Actual: `Vec<Message>`. | P1 |
| 7 | `crates/context_engine.md` — `MemoryRetriever` trait | `retrieve(&self, query, session_key, ctx) -> Result<Vec<MemoryEntry>>` | Actual: `retrieve(&self, query, limit) -> Vec<MemoryEntry>`. | P1 |
| 8 | `crates/cognitive.md` — `run_reforge` | 26 parameters | Actual: 25 positional parameters. | P2 |
| 9 | `crates/cognitive.md` — `MirrorFacade` | `brain_state`, `recent_narratives`, `record_feedback` | Actual: `get_state`, `get_narratives`, `submit_feedback`. | P2 |
| 10 | `subsystems/06-scheduling.md` — `RecurrenceEngine` | 3 trait methods with simplified return types | Actual: `decrement_count -> Result<Option<u32>>`, `create_instance -> Result<CreateInstanceOutcome>`, `cancel_unfired_instances -> Result<()>`. | P2 |
| 11 | `subsystems/11-channels-mcp.md` — MCP cooldown | "cooldown 30s" in diagram | Actual: `McpCircuitBreaker::new(3, 60)` — 60 seconds. | P2 |
| 12 | `subsystems/07-tools-framework.md` — `ToolRegistry` | `list_meta() -> Vec<&ToolMetadata>`; `by_category() -> Vec<DynTool>` | Actual: `list_meta() -> Vec<(String, String, Vec<String>)>`; `by_category() -> Vec<&str>`. | P2 |
| 13 | `subsystems/09-coding-mode.md` — `EventKind` | 21 variants (9 base + 10 klynt-only + 2 background) | Actual: **22** variants (adds `GitCommit` to base group). | P2 |
| 14 | `subsystems/09-coding-mode.md` — `klynt-core` register counts | "Six read-only" / "thirteen primitive tools" | Actual: 7 read-only, 14 primitive (7+5+2), plus 8 recall stubs = 22 total. | P2 |
| 15 | `subsystems/13-desktop-frontend.md` — `desktop` return types | `main() -> Result<…>` / `run_desktop_app() -> Result<…>` | Actual: both return `()`. | P2 |
| 16 | `subsystems/13-desktop-frontend.md` — CI guard tests | 4 tests | Actual: 5 test files (4 guards + 1 smoke). | P2 |
| 17 | `subsystems/08-assistant-features.md` — `feature-productivity` | 20 tables | Actual: 21–22 `CREATE TABLE` statements in migration. | P2 |
| 18 | `crates/coding-ingest.md` — module tree | Lists `exclude_set.rs`, `repo_scope.rs`, `repos.rs` | Actual: `excludes.rs`, `scope.rs` + `scope_resolver.rs`, `store.rs`. Also omits `coverage/`, `transport/`, `desktop_lock.rs`, etc. | P2 |
| 19 | `crates/coding-memory.md` — module tree | Lists `distiller/boundary.rs`, `distiller/phase_a5.rs`, `reforge/session_end_pass.rs`, `symbols/tree_sitter.rs`, `symbols/anchors.rs`, `observation/mod.rs`, `retry/mod.rs` | All of these files **do not exist**. ~15 extra modules exist but are not documented. | P1 |
| 20 | `crates/coding-memory.md` — `Distiller::new` | `new(cognitive_provider, symbol_extractor, repos, config)` | Actual: `new(config, ingest_repo, writer, provider, retriever)`. | P1 |
| 21 | `crates/coding-memory.md` — `DistillerWriter` / `ReforgeWriter` APIs | Documented with `i64` IDs and simple signatures | Actual: uses `PreparedFact`/`PreparedEpisode`, `&str` IDs, requires `repo` parameters. | P1 |
| 22 | `subsystems/12-plugins-platform.md` — `plugin-runtime` file map | `src/sandbox.rs` | Functions live in `src/host/mod.rs`. | P2 |
| 23 | `subsystems/12-plugins-platform.md` — `platform-macos` file map | `computer_use/ax_walker.rs` | Actual: `computer_use/ax_tree.rs`. | P2 |
| 24 | `subsystems/13-desktop-frontend.md` — `desktop-ui` | 33 feature directories | Actual: 32 feature directories. | P3 |
| 25 | `subsystems/05-cognitive-memory.md` — reforge phase count | "3 LLM calls at the handler level" (Synthesize/Review/Narrate) | Acceptable summary, but extension hooks may add more LLM calls. Phase table correctly marks them as "(via hook)". | P3 |

---

## Tech Debt Delta

### Resolved (can be removed or updated in TECH_DEBT.md)

| # | Original Entry | Resolution | Evidence |
|---|---------------|------------|----------|
| R1 | Category 2: "4 Reforge phases in `coding-memory` all stubbed at `required_phase: 5`" | **OUTDATED**. `SessionEndPass` and `CrossSessionDedup` are fully implemented in `reforge/`. Only the legacy `reforge_phase.rs` stubs remain. Entry should be narrowed to the 2 remaining stubs. | Agent-5, Agent-9, Agent-10 verified `reforge/*.rs` contain full implementations wired in `app-core`. |
| R2 | Category 6: `app-core/src/init/mod.rs:1034` — default timezone TODO | **Code resolved, comment stale.** Line 1034 still has `TODO(phase-3.5)`, but line 1035 immediately uses `config.timezone.as_str()`. | Agent techdebt-2 verified. |
| R3 | Category 7: `crates/desktop-ui/` Specta stub colliding with `/desktop-ui/` | **Crate removed from workspace.** `crates/desktop-ui/Cargo.toml` no longer exists; directory removed from root `Cargo.toml` members. Orphaned `src/bindings.ts` remains as cleanup gap. | Agent techdebt-2 verified. |

### New Findings (not in TECH_DEBT.md)

| Sev | File:Line | Finding | Notes |
|-----|-----------|---------|-------|
| P1 | `crates/coding-ingest/Cargo.toml:30` + `crates/coding-memory/Cargo.toml:16` | **Circular dependency**: `coding-ingest` ↔ `coding-memory` via path deps | Violates acyclic crate graph invariant. |
| P1 | `crates/mcp-bridge/Cargo.toml:16-17` | **Upward dependency**: `mcp-bridge` (transport) depends on `app-core` + `desktop-shared` | Transport layer should not depend on application layer. |
| P2 | `crates/feature-coding-bash/Cargo.toml` | **Missing from `[workspace.dependencies]`** | Forces path-deps for itself and all its consumers. |
| P2 | `crates/approval/Cargo.toml:3` | `version = "0.1.0"` hardcoded; workspace declares `0.1.1` | Should use `version.workspace = true`. |
| P2 | `crates/mcp-bridge/Cargo.toml:4` | `edition = "2024"`; workspace declares `2021` | Should use `edition.workspace = true`. |
| P2 | `crates/app-core/src/tracing/registry.rs:70,91,94` | 3× `unimplemented!()` | Tracing registry stub. No production impact but incomplete. |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_dedup.rs:18,21,24` | 3× `unimplemented!()` | Test stubs. |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs:17,20,23` | 3× `unimplemented!()` | Test stubs. |
| P3 | `crates/desktop-ui/src/bindings.ts` | Orphaned file; crate removed from workspace | Cleanup gap. |
| P2 | 7 crates use `path = "../<crate>"` for deps that ARE in `[workspace.dependencies]` | `approval`, `context_engine`, `feature-coding-todo`, `feature-focus`, `feature-learning`, `feature-notes`, `klynt-skill-loader` | Convention inconsistency; should use `.workspace = true`. |
| P2 | `crates/cognitive/src/services/reforge/service.rs:1` | File-level doc still says "8 phases" | Actual code has 16+ phase markers. |
| P2 | `crates/coding-memory/src/reforge_phase.rs` | Dead-code stubs still exist alongside fully implemented versions in `reforge/` | Confusing duplication; old stubs should be deleted. |

---

## Missing Documentation

### Crates with no architecture doc

**None.** Every crate in the workspace is mentioned in at least one subsystem doc. However, **55 of 66 crates lack a dedicated deep-dive** (`crates/<crate>.md`). The 11 that have one are the most-touched crates; the gap is acceptable for the rest given the subsystem coverage.

### Modules not documented

The following modules/files exist in source but are **not mentioned in any architecture doc file map**:

| Crate | Missing Module/File | Significance |
|---|---|---|
| `common` | `src/time/{mod,convert,helpers}.rs` | Time-zone utilities (Jiff-based) |
| `common` | `src/ports/notification.rs` | `NotificationSender` trait |
| `storage` | `src/test_util.rs` | Test utilities |
| `storage` | `src/messages/render.rs` | Message rendering |
| `storage` | `src/vector_store/{cognitive,community,conv,crud,entity_embedding,maintenance,schemas,tree_node}.rs` | Vector store submodules |
| `tools` | `conversation_recall.rs`, `progress_handler.rs`, `search_utils.rs`, `semantic_fact_search.rs`, `todo_types.rs` | Secondary domain modules |
| `cognitive` | `src/services/graph_retrieval.rs` | Graph-path boost retrieval |
| `cognitive` | `src/services/atom_extraction.rs` | Atom-level extraction |
| `cognitive` | `src/services/extraction_critic_types.rs` | Critic types |
| `cognitive` | `src/services/graph_linker_types.rs` | Graph linker types |
| `cognitive` | `src/services/micro_reforge_types.rs` | Micro-reforge types |
| `cognitive` | `src/mirror/sources/coding_bash.rs` | Background job signal source |
| `coding-ingest` | `coverage/`, `transport/`, `desktop_lock.rs`, `git_invalidation.rs`, `pending_invalidations.rs`, `warn.rs` | Ingest support modules |
| `coding-memory` | `causal/`, `code_domain_searcher.rs`, `code_state.rs`, `counterfactual.rs`, `facts.rs`, `git_invalidation.rs`, `mirror/`, `problem_hash.rs`, `retrieval_skills/`, `scope.rs`, `sink/`, `skill_evolver/`, `skills.rs`, `reforge/{cross_cli_synthesis,managed_block,selective_delete,sensitivity_filter,session_summary_repo,symbol_validation,synth_handler,types}.rs` | ~15 modules completely omitted from doc file map |
| `agent` | `context_sources/todo.rs` | TODO context source |
| `agent` | `adapters/llm_summary.rs` | LLM summary adapter |
| `agent` | `confidence/`, `learning/`, `output/` | Subagent and output modules |
| `app-core` | `tracing/registry.rs` | Tracing registry (stubbed) |
| `channels` | `src/shared.rs` | Shared channel utilities |
| `notifications` | `channel/mod.rs:64` | TG/DC/Email dispatcher wiring gap |
| `mcp-bridge` | `protocol.rs` | `MAX_FRAME_BYTES = 1 MiB` |
| `desktop` | `specta_builder_smoke.rs` test | 5th test file |

### Features mentioned in code but not in any architecture doc

| Feature / Mechanism | Where in code | Why it matters |
|---|---|---|
| `InProcess` hook execution mode | `crates/klynt-protocol/src/lib.rs:79` | Variant exists in `HookExecutionMode` enum but **no dispatch path implements it** — only `Subprocess` is wired. Security/performance gap. |
| `Hook.fail_open` field | `crates/klynt-hooks/src/schema.rs:17` | Field exists in schema but is **never read** by dispatcher. Fail-open is hardcoded silently. |
| KCA env-only feature flags (6 total) | `agent`, `context_engine`, `cognitive` | `KCA_DISABLE_COMPRESSION`, `KCA_PHASE_4`, `KCA_PHASE_4_TOOL_DRIVEN`, `KCA_PHASE_4_LEGACY_NUDGE`, `KCA_COMMUNITY_SUMMARIES`, `KCA_REFORGE_COMPRESS`. Only discoverable by grep; undocumented at project level. |
| KCA Track 7 — predictive cache warming | `agent/src/agent_loop/builder.rs:746-795` | `LlmQueryPredictorHandler` + `cognitive::predictive_cache`. Wired and active but not documented outside source comments. |
| Focus-session message deferral | `agent/src/agent_loop/mod.rs:321-401` | Buffers inbound messages during focus sessions with auto-reply. Behavioral feature with no doc coverage. |
| `non_ui_policy` on `ToolKitBuilder` | `crates/klynt-core/src/registry/builder.rs` | Field exists but is not mentioned in coding-mode docs. |
| `register_recall` stubs (8 tools) | `crates/klynt-core/src/registry/builder.rs` | 8 recall tools are registered as stubs. Their existence is noted but not their full list/purpose. |
| `desktop --hook` short-circuit | `crates/desktop/src/main.rs:101-108` | Sub-10ms CLI hook path that bypasses Tauri/mimalloc. Undocumented outside source. |
| `mcp-bridge` `MAX_FRAME_BYTES` (1 MB) | `crates/mcp-bridge/src/protocol.rs:5` | IPC frame size limit. Not in user-facing docs. |
| `SkillSource` priority numbering | `klynt-skill-loader/src/index.rs:20-25` | Higher number wins on collision (reverse of common convention). Not documented. |
| `coding_approval_history` unbounded growth | `storage/src/repos/coding_approval_history.rs` | Table grows without retention policy or scheduled cleanup job. |
| Cross-feature shared tables | `feature-notes` + `feature-language-learning` | `practice_sessions` defined in notes but used by language-learning. No ownership registry. |

---

## Cross-Cutting Consistency

### 5 cross-cutting facts verification

| # | Fact | Verdict | Evidence |
|---|------|---------|----------|
| 1 | **"66 crates" claim** | ✅ **VERIFIED** | Root `Cargo.toml` lists 64 workspace members + root `klyntbot` package + excluded `plugin-sdk` = 66. `CLAUDE.md` and root `README.md` still incorrectly say "39 crates / 9 layers". |
| 2 | **"Multiple half-built features documented as shipped"** | ✅ **VERIFIED** (mostly) | `lsp-client` all stubs (5 `TODO(T5)`). Notification TG/DC/EM channels unwired. MCP server approval always declines. Plugin `agent_ask_user` stub. Voice pronunciation pipeline half-built (5 TODOs). Computer Use platform real but unwired. **BUT:** the claim "4 Reforge phases in coding-memory are stubs" is now **partially outdated** — only 2 remain stubs; `SessionEndPass` and `CrossSessionDedup` are fully implemented. |
| 3 | **"Migration debt visible in source"** | ✅ **VERIFIED** | `CronExecutor` + `TemporalScheduler` run side-by-side. `storage/Cargo.toml:7` depends on `ai-core`. Legacy `messages.content` column mirrored on every write. `LEGACY_COMMAND_NAMES` empty-but-present. Stale "CronService" log message at `app-core/src/init/temporal_scheduler.rs:99`. |
| 4 | **`desktop-ui` location confusion** | ✅ **VERIFIED** (partially resolved) | `crates/desktop-ui/` has no `Cargo.toml` and was removed from workspace members. Only orphaned `src/bindings.ts` remains. The actual React frontend is at repo root `/desktop-ui/`. CLAUDE.md hints at this but doesn't make the structural distinction explicit. |
| 5 | **"5 coding-ingest adapters, not 4"** | ✅ **VERIFIED** | `claude_code/`, `codex/`, `kimi_cli/`, `opencode/`, `git_post_commit.rs`. All implement `IngestAdapter`. CLAUDE.md says 4, omitting `git_post_commit`. Only `claude_code` and `codex` are hook-driven; others are poll-only. |

### Broken links

| Link | Referenced from | Status |
|------|-----------------|--------|
| `../00-overview.md` from `subsystems/*.md` | All 14 subsystem docs | ✅ **RESOLVES CORRECTLY** to `docs/architecture/00-overview.md`. Some Phase-2 agents incorrectly flagged this as broken due to path-resolution errors. |
| `docs/architecture/kca-game-changer.md` | `crates/kca-bench/src/lib.rs:3-5` | ❌ **BROKEN** — file does not exist. |
| `docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md` | `TECH_DEBT.md` + `00-overview.md` | ❌ **BROKEN** — file does not exist. |
| `../crates/app-core.md` from `13-desktop-frontend.md` | `13-desktop-frontend.md` | ⚠️ Marked as "planned" in doc; absence is documented and intentional. |
| `../crates/desktop.md` from `13-desktop-frontend.md` | `13-desktop-frontend.md` | ⚠️ Marked as "planned" in doc; absence is documented and intentional. |

### Contradictions between docs

| # | Contradiction | Between | Resolution |
|---|---------------|---------|------------|
| 1 | **Reforge phase status** | `coding-memory.md` says all 4 stubbed vs `cognitive.md` / `00-overview.md` describes them as wired hooks | `coding-memory.md` is **stale**. The `reforge/` implementations are live. Only `reforge_phase.rs` stubs remain. |
| 2 | **"No physical DELETE" invariant** | `coding-memory.md` claims no DELETE vs `coding-memory` source performs `delete_by_id` in `SessionEndPass` | Doc is **wrong**; `SessionEndPass` does physical deletes for within-session dedup and stale-candidate resolution. |
| 3 | **`DynProvider` type** | `providers.md` says `Box<dyn LlmProvider>` vs `agent.md` / `context_engine.md` / code uses `Arc<dyn LlmProvider>` | `providers.md` is stale; the `Arc<dyn>` change happened after it was written. |
| 4 | **Reforge parameter count** | `cognitive.md` says 26 vs `05-cognitive-memory.md` says 26 vs code has 25 | Minor numeric drift; likely a counting discrepancy. |
| 5 | **`EventKind` variant count** | `coding-ingest.md` says 21 vs `coding-memory.md` doesn't mention `GitCommit` vs code has 22 | Both docs undercount by 1. |
| 6 | **TTFT threshold** | `CLAUDE.md` claims p95 ≤ 15ms vs `00-overview.md` says 25ms and no-op skeleton vs `scripts/run_chat_perf_gates.sh:39` says "deferred to PR8" | `00-overview.md` is the most accurate; CLAUDE.md is stale. |
| 7 | **Bundle budget** | `CLAUDE.md` claims 30 kB gzipped vs `00-overview.md` says 350 kB vs `.size-limit.json` says 350 kB | `00-overview.md` is accurate; CLAUDE.md is off by an order of magnitude. |
| 8 | **Secondary window count** | `CLAUDE.md` says 4 vs `00-overview.md` / `desktop.md` say 5 | `coding:{repo_id}` window was added after CLAUDE.md was written. |

---

## Recommended Action Items

### P0 — Fix immediately

1. **Rewrite `crates/providers.md`** — The `LlmProvider` trait, `DynProvider`, `ProviderCapabilities`, `ChatParams`, `CacheAnchor`, `DegradationLevel`, and adapter constructors are all wrong. This doc will mislead anyone implementing a new provider. *(Effort: 2–3 hours)*
2. **Rewrite `crates/context_engine.md` `BudgetAllocator` section** — The `Priority` enum is completely wrong (generic ladder vs semantic names). `BudgetReport`, `TieredHistoryCompressor`, `MemoryRetriever`, `TokenCounter`, and `AssembledContext` fields are all mismatched. *(Effort: 2–3 hours)*
3. **Correct `subsystems/02-storage.md` `circuit_breaker.rs` description** — Either implement the claimed per-repo breaker logic or rewrite the doc to match the simple global deadline table. *(Effort: 30 min)*
4. **Update `subsystems/09-coding-mode.md` and `00-overview.md` reforge phase claims** — Remove the blanket "4 phases stubbed" claim. Document that `SessionEndPass` and `CrossSessionDedup` are implemented, while `CodingSynthesisPhase` and `RuleArtifactGenerationPhase` (in `reforge_phase.rs`) remain stubs. Remove the "no physical DELETE" invariant. *(Effort: 1 hour)*
5. **Fix `crates/coding-memory.md` module tree** — The file map lists ~8 non-existent files and omits ~15 real modules. *(Effort: 1 hour)*
6. **Add missing `kca-e2e` fixture files or remove the load-time assertions** — `cargo test -p kca-e2e` fails on clean checkout. *(Effort: 30 min)*
7. **Remove dead-code `reforge_phase.rs` stubs** — `CodingSynthesisPhase` and `RuleArtifactGenerationPhase` stubs in `reforge_phase.rs` are shadowed by real implementations in `reforge/`. The old file causes doc confusion. *(Effort: 15 min)*

### P1 — Next sprint

1. **Regenerate `crates/agent.md` `AgentEvent` reference** — ~50 variants with correct field names. Update `LoopFinishReason`, `ExecuteLoopResult`, `SafetyCap`, `ExecutionParams`, `SpawnParams`. *(Effort: 2 hours)*
2. **Fix `crates/app-core.md` public API signatures** — `ThreadRuntime` trait, `AppCore::init_with_sender` params/return type, field types. Label the init sequence table as "simplified". *(Effort: 1–2 hours)*
3. **Fix `subsystems/08-assistant-features.md` action counts** — `feature-finance` claims 57, actual 64. `feature-productivity` table count. *(Effort: 30 min)*
4. **Fix `subsystems/10-sandboxing-security.md` file map errors** — Remove `src/hook.rs` and `src/suggester.rs`. Add tests note for `klynt-process-hardening`. *(Effort: 30 min)*
5. **Fix `subsystems/11-channels-mcp.md` MCP drift** — Move `start_health_check` to `McpManager`. Replace `is_server_allowed` with `decide() -> AllowDecision`. Fix cooldown to 60s. Fix `activity-log` type fields (`i64` not `u64`, `Option<String>`, `String` not `ResourceEdgeType`). *(Effort: 1 hour)*
6. **Fix `subsystems/12-plugins-platform.md` counts** — 12 host functions not 14. `ax_tree.rs` not `ax_walker.rs`. *(Effort: 15 min)*
7. **Fix `subsystems/13-desktop-frontend.md` drift** — `main()` returns `()`, 5 test files, 32 feature dirs. Update `desktop-ui` workspace status to "removed; orphaned bindings.ts remains". *(Effort: 30 min)*
8. **Fix `subsystems/06-scheduling.md` recurrence trait signatures** — `decrement_count -> Result<Option<u32>>`, `create_instance -> Result<CreateInstanceOutcome>`, `cancel_unfired_instances -> Result<()>`. Add `CreateInstanceOutcome` type. *(Effort: 30 min)*
9. **Update `crates/cognitive.md` signature mismatches** — `run_reforge` param count, `UnifiedMemoryService` API, `RecallConfig` fields, `SituationInputs` fields, repo method names. *(Effort: 1–2 hours)*
10. **Address circular dependency `coding-ingest` ↔ `coding-memory`** — Break the cycle by moving shared types to a smaller crate or by using trait objects. *(Effort: 2–4 hours)*
11. **Address `mcp-bridge` upward dependency on `app-core`/`desktop-shared`** — Invert dependency or extract shared types to a lower-level crate. *(Effort: 2–4 hours)*
12. **Create missing docs** — `kca-game-changer.md` and `docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md` are referenced but absent. Either create them or remove references. *(Effort: 1 hour)*

### P2 — Backlog

1. **Normalize path-only deps to `workspace = true`** — 7 crates (`approval`, `context_engine`, `feature-coding-todo`, `feature-focus`, `feature-learning`, `feature-notes`, `klynt-skill-loader`) use `path = "../..."` for dependencies that are already in `[workspace.dependencies]`. *(Effort: 1 hour)*
2. **Add `feature-coding-bash` to `[workspace.dependencies]`** — Currently missing, forcing path-deps everywhere. *(Effort: 15 min)*
3. **Fix `approval` version drift** (`0.1.0` vs workspace `0.1.1`) and `mcp-bridge` edition drift (`2024` vs workspace `2021`). *(Effort: 15 min)*
4. **Clean up orphaned `crates/desktop-ui/src/bindings.ts`** — The file is dead weight; the real bindings are at `/desktop-ui/src/bindings.ts`. *(Effort: 5 min)*
5. **Add project-level documentation for KCA env flags** — 6 flags control production behavior but are only discoverable by grep. *(Effort: 30 min)*
6. **Add project-level documentation for predictive cache warming (KCA Track 7)** and focus-session message deferral. *(Effort: 30 min)*
7. **Update `CLAUDE.md` and root `README.md`** — Still claim "39 crates / 9 layers". Should reference the 14-subsystem model. *(Effort: 1 hour)*
8. **Fix stale `CronService` log message** at `app-core/src/init/temporal_scheduler.rs:99` — Change to "side-by-side with CronExecutor". *(Effort: 5 min)*
9. **Promote `DEFAULT_MATERIALIZE_AHEAD = 3`** to a config field. *(Effort: 30 min)*
10. **Resolve `feature-coding-bash` TODOs** (18) and `feature-coding-todo` TODOs (312) — these are the highest TODO densities in the workspace. *(Effort: varies)*
11. **Document `ToolOutput::Structured`** — Defined but zero production usage. Decide if it should be promoted or removed. *(Effort: 15 min)*
12. **Address `intent_pipeline` vestigial field** — `SourceContext::intent_summary` is always `None`. Delete or repurpose. *(Effort: 30 min)*

---

*Report compiled by synthesis-agent. All counts and file:line references are drawn directly from the 13 verification deliverables in `docs/architecture/.verification/`.*
