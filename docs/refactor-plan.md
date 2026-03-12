# Klyntbot Backend Refactor Plan
> Prepared: 2026-03-12
> Scope: Structural reorganization only — zero logic changes
> Author: Architecture Review

---

# ANALYSIS SUMMARY

## Executive Context

Klyntbot is a **151,684 LOC / 27-crate / 582-file** Rust workspace built on a disciplined 9-layer dependency architecture. The codebase has grown organically from a working prototype into a feature-rich AI-agent platform. The foundation is solid; the structural debt is concentrated in a small number of crates that have accumulated without consistent internal organization. This document is a zero-logic-change structural refactor plan.

---

## Part 1 — Analysis Summary

### 1.1 Current Strengths (Preserve 100%)

| Strength | Location |
|---|---|
| 9-layer upward-only dependency graph | Cargo.toml workspace |
| Proc-macro `#[derive(Tool)]` / `#[tool_actions]` zero-boilerplate tool system | `tools-core-macros` |
| `FeaturePackage` trait: tools + migrations + health as a single unit | `tools-core` |
| Dependency inversion via `Arc<dyn Trait>` for SpawnHandler, CronHandler, etc. | `agent`, `tools-core` |
| WAL SQLite + `StoragePool` (Clone+Send+Sync) + LanceDB vector store | `storage` |
| FSRS-based memory consolidation + BM25 + semantic hybrid retrieval | `cognitive` |
| ReAct execution loop with intent classification, circuit-breaker, cost tracking | `agent` |
| Feature-gated `email` channel | `channels` |
| WASM plugin runtime via Extism | `plugin-runtime` |
| MCP client + server (rmcp 0.17) | `mcp` |
| `#[serde(rename_all = "camelCase")]` config + env override | `config` |
| `workspace.dependencies` inheritance | `Cargo.toml` |

---

### 1.2 Pain Points & Structural Debt

#### A. Files Exceeding 400 Lines (Requiring Splits)

| File | LOC | Crate | Issue |
|---|---|---|---|
| `storage/repos/task_repo.rs` | 2,450 | storage | CRUD + filtering + search + analytics mixed in one file |
| `feature-tasks/types.rs` | 1,780 | feature-tasks | Domain types, status FSM, recurrence + search + serialization all co-located |
| `desktop-shared/commands.rs` | 1,516 | desktop-shared | All IPC command types in one flat file, no domain grouping |
| `app-core/init.rs` | 1,350 | app-core | Initialization + DI wiring + migration running in a single 1,350-line function chain |
| `agent/intent_pipeline/analysis.rs` | 1,356 | agent | Classification + constraint extraction + keyword tables + LLM fallback all in one module |
| `agent/agent_runtime/runtime.rs` | 1,354 | agent | 10-step pipeline + agent selection + MCP filtering + cost tracking mixed |
| `agent/cognitive_handlers.rs` | 1,282 | agent | All 6 handler trait implementations in one file |
| `agent/agent_loop/builder.rs` | 1,199 | agent | 50+ builder methods in a single file |
| `context_engine/assembler.rs` | 1,209 | context_engine | Context assembly + priority ordering + caching + history compressor calls in one struct |
| `app-core/handlers/chat.rs` | 1,114 | app-core | Chat + session + streaming + history + search all in one handler file |
| `storage/repos/action_repo.rs` | 1,190 | storage | Todo action CRUD + embedding + search + time-entry repos mixed |
| `feature-tasks/tool/mod.rs` | 917 | feature-tasks | All 11 tool actions in one module |
| `tools/ask_user.rs` | 922 | tools | Question engine + platform dispatch + retry logic + form builder in one file |
| `feature-productivity/intelligence/session_aggregator.rs` | 1,171 | feature-productivity | Aggregation + scoring + AI summarization in one class |
| `feature-productivity/types.rs` | 1,126 | feature-productivity | All productivity domain types in one flat file |
| `agent/agent_profile/types.rs` | 599 | agent | Profile types + tool filter + MCP filter + delegation logic mixed |
| `agent/persona.rs` | 617 | agent | Persona chain + scope + profile injection in one file |
| `agent/output/cost_tracker.rs` | 481 | agent | Pricing tables + record logic + budget alerts in one file |
| `config/loader.rs` | 786 | config | File I/O + env override + merge logic + defaults + validation in one file |
| `config/schema/mod.rs` | 509 | config | Root Config struct with all sub-struct imports |
| `providers/anthropic_native.rs` | 1,238 | providers | API call + streaming + thinking blocks + cache headers + retry all together |
| `providers/manager.rs` | 1,046 | providers | Failover + circuit breaker + provider registry + retry policy in one struct |
| `channels/discord.rs` | 1,255 | channels | WebSocket + Gateway + message parsing + interactions + attachment handling |
| `channels/slack.rs` | 1,134 | channels | Socket Mode + events API + block kit + thread handling all in one file |
| `channels/telegram.rs` | 1,069 | channels | Long polling + message types + inline keyboards + voice + file handling |
| `common/utils/stream_renderer.rs` | 557 | common | SSE formatting + diff detection + buffering in one file |
| `common/utils/terminal/thinking_renderer.rs` | 517 | common | Extended thinking rendering separate from general rendering |
| `common/utils/terminal/markdown.rs` | 478 | common | Terminal markdown rendering separate concern |
| `common/utils/date.rs` | 403 | common | Date parsing, formatting, humanizing, chrono helpers mixed |
| `feature-finance/tool/transactions.rs` | 1,003 | feature-finance | Transaction CRUD + categorization + search + reporting in one tool |
| `feature-finance/types.rs` | 698 | feature-finance | All finance domain types in one file |
| `app-core/handlers/productivity.rs` | 983 | app-core | All productivity commands in one handler |
| `app-core/handlers/finance.rs` | 791 | app-core | All finance commands in one handler |
| `storage/vector_store.rs` | 963 | storage | LanceDB init + table creation + insert + search + update all in one file |
| `cognitive/background.rs` | 1,169 | cognitive | SSE streaming + task dispatch + pipeline event routing in one file |

#### B. Missing Internal Module Organization (No Clear Hexagonal Boundaries)

The following crates have flat `src/` layouts where domain, application logic, and infrastructure concerns are interleaved:

- **`common`** — utils (terminal, stream, date) live next to domain types (error, types, prompts)
- **`config`** — schema types and file I/O loader are not separated
- **`agent`** — 50+ files in `src/` with no top-level grouping by concern (domain, application, handlers)
- **`tools`** — 20+ tool files are flat, with no grouping by domain (filesystem, web, ai, productivity, project-management)
- **`cognitive`** — domain types, repos, services, background tasks all in `src/` with no subfolder beyond `repos/` and `search/`
- **`storage`** — repos organized by entity but with no grouping by domain vertical (core, tasks, finance, cognitive, productivity)
- **`channels`** — platform files flat alongside `Channel` trait, formatter, and manager
- **`feature-*`** crates each have ad-hoc internal organization, not a consistent pattern

#### C. Incomplete Features / Dead Config

| Item | Location | Status |
|---|---|---|
| Feishu/Lark channel | `config/schema/channels.rs` + NO impl | Config only — never implemented |
| DingTalk channel | `config/schema/channels.rs` + NO impl | Config only — never implemented |
| Mochat channel | `config/schema/channels.rs` + NO impl | Config only — never implemented |
| Work context merge tracking | `app-core/handlers/work_context.rs:343-344` | TODO comment — not implemented |
| Inference loop metrics exposure | `app-core/handlers/work_context.rs:486` | TODO comment — not implemented |
| Forecast trend computation | `agent/handlers/forecast.rs:307` | TODO comment — not implemented |
| Feature-tasks Phase 3 | `docs/superpowers/plans/` | Design complete, implementation pending |
| Intent classification short-message preemption bug | `agent/intent_pipeline/analysis.rs` | BACK-001 — known bug, not fixed |
| Due date underutilization | `feature-tasks`, `agent/enrichment/` | BACK-002 — known gap |

#### D. `feature-todo` vs `feature-tasks` Legacy Overlap

Both crates implement task management. `feature-todo` is the legacy crate (25 action types, ActionRepo, ActionPatch, etc.). `feature-tasks` is the new agentic replacement. CLAUDE.md explicitly states: *"Legacy `feature-todo` crate can be removed once `feature-tasks` fully replaces it."* There is no migration path or deprecation marker at the code level.

#### E. `pub` Boundary Issues

Several crates expose implementation details unnecessarily:

- `storage` exports all row types directly (`ActionRow`, `TaskRow`, etc.) — these should be `pub(crate)` until consumed by specific repos
- `cognitive` exports individual repo constructors that should only be accessed via `UnifiedMemoryService`
- `agent` `cognitive_handlers.rs` is `pub` — it should be `pub(crate)` as it is only accessed via trait objects injected by `app-core/init.rs`
- `tools` exposes concrete tool structs directly — these could be accessed via `ToolRegistry` only

#### F. `app-core/init.rs` — Wiring God-Object

At 1,350 lines, `init.rs` constructs every component, wires every dependency, runs all migrations, and assembles the `AppCore` struct. This is a single-file composition root that is difficult to navigate, test in isolation, or extend.

#### G. `providers` — No Clear Port/Adapter Split

`anthropic_native.rs` (1,238 lines) contains API protocol details, extended thinking block parsing, cache header construction, streaming event dispatch, and retry policy — all in one struct. The `LlmProvider` trait (the port) and its implementations (the adapters) are organized by provider but not by concern.

#### H. `desktop-shared/commands.rs` — 1,516 Lines, No Domain Grouping

All Tauri IPC command types are in one file. They should be split by domain (tasks, finance, productivity, cognitive, etc.) to parallel the `app-core/handlers/` split.

---

### 1.3 Architecture Decisions to Preserve

- **Layer 0–8 crate boundaries** — Do not add or remove any crate
- **`FeaturePackage` trait** — Identical interface, just move implementations into cleaner files
- **`#[derive(Tool)]` macros** — Unchanged, tool structs just reorganized within their crates
- **Strict upward-only dependencies** — No new cross-layer imports
- **`StoragePool::connect_in_memory()` test pattern** — Preserve in all tests
- **`workspace.dependencies` inheritance** — All `Cargo.toml` version pins stay at workspace root
- **Feature gates** (`email`, `browser-integration`, `plugin-integration`) — Unchanged

---

## Part 2 — Proposed Full Folder Tree

### Design Principles Applied

1. **Hexagonal/Clean Layout** inside each large crate: `domain/` (entities, value objects, events), `application/` (use cases, services, handlers), `infrastructure/` (repos, external adapters), `adapters/` (platform-specific wiring)
2. **Vertical Slice** within `tools/`, `storage/repos/`, `desktop-shared/`: group by business domain (tasks, finance, productivity, cognitive)
3. **One concern per file** — no file exceeds ~400 lines after split
4. **Consistent `feature-*` internal structure** — all feature crates follow the same layout
5. **`pub(crate)` by default** — only promote to `pub` what consumers actually import

### Proposed Tree (delta from current — only changed crates shown)

```
klyntbot/                               ← workspace root (UNCHANGED)
├── Cargo.toml                          ← UNCHANGED
├── src/                                ← facade binary (UNCHANGED)
├── agents/                             ← UNCHANGED
├── tests/                              ← UNCHANGED
├── docs/
│   ├── architecture/                   ← UPDATED: add architecture.md refresh
│   └── ai-coding-rules.md              ← NEW: AI agent coding conventions
├── BACKLOG.md                          ← NEW: full backlog (see Part 4)
│
└── crates/
    ├── common/                         ← RESTRUCTURED (was flat)
    │   └── src/
    │       ├── lib.rs                  ← re-exports all pub types
    │       ├── errors/
    │       │   ├── mod.rs
    │       │   ├── klyntbot.rs         ← KlyntbotError (from error.rs)
    │       │   └── domain.rs           ← SessionError, ToolError, ChannelError, etc.
    │       ├── types/
    │       │   ├── mod.rs
    │       │   ├── core.rs             ← ChannelName, ChatId, MessageRole, SessionKey
    │       │   ├── entity_card.rs      ← EntityCard (from entity_card.rs)
    │       │   └── prompts.rs          ← InteractionRequest, FormResponse, Question, Answer
    │       └── utils/
    │           ├── mod.rs
    │           ├── date.rs             ← date helpers (UNCHANGED, still 403L but single concern)
    │           ├── stream_renderer.rs  ← split into:
    │           │     → sse.rs          ← SSE formatting (raw bytes, chunk assembly)
    │           │     → diff.rs         ← text diff detection
    │           │     → buffer.rs       ← streaming buffer logic
    │           └── terminal/
    │               ├── mod.rs
    │               ├── markdown.rs     ← UNCHANGED
    │               └── thinking.rs     ← thinking_renderer.rs renamed
    │
    ├── config/                         ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── schema/
    │       │   ├── mod.rs              ← root Config struct
    │       │   ├── agents.rs           ← AgentDefaults
    │       │   ├── channels.rs         ← all channel configs (UNCHANGED)
    │       │   ├── cognitive.rs        ← CognitiveConfig
    │       │   ├── conversation.rs     ← ConversationConfig
    │       │   ├── finance.rs          ← FinanceConfig
    │       │   ├── mcp.rs              ← McpConfig, McpServerConfig
    │       │   ├── packs.rs            ← PacksConfig
    │       │   ├── providers.rs        ← ProviderConfigs
    │       │   ├── tasks.rs            ← TasksConfig
    │       │   └── productivity.rs     ← ProductivityConfig
    │       └── loader/
    │           ├── mod.rs              ← re-exports load(), load_with_env_overrides()
    │           ├── file_io.rs          ← read/write config.json (from loader.rs)
    │           ├── env_override.rs     ← KLYNTBOT_* env var parsing (from loader.rs)
    │           ├── merge.rs            ← deep merge logic (from loader.rs)
    │           └── defaults.rs         ← default Config construction (from loader.rs)
    │
    ├── bus/                            ← UNCHANGED (already well-organized)
    │   └── src/
    │       ├── lib.rs
    │       ├── queue.rs                ← MessageBus (inbound/outbound mpsc)
    │       ├── domain_events.rs        ← DomainEventBus + all event variants
    │       ├── events.rs               ← InboundMessage, OutboundMessage
    │       └── learning_events.rs      ← LearningEventBus
    │
    ├── tools-core/                     ← UNCHANGED (already clean)
    ├── tools-core-macros/              ← UNCHANGED
    │
    ├── storage/                        ← RESTRUCTURED by domain vertical
    │   └── src/
    │       ├── lib.rs                  ← re-exports StoragePool, Repos, VectorStore
    │       ├── pool.rs                 ← StoragePool (UNCHANGED)
    │       ├── error.rs                ← UNCHANGED
    │       ├── vector_store/
    │       │   ├── mod.rs              ← VectorStore struct + public API
    │       │   ├── tables.rs           ← table creation + schema (from vector_store.rs)
    │       │   ├── search.rs           ← similarity search + ANN logic (from vector_store.rs)
    │       │   └── operations.rs       ← insert, update, delete (from vector_store.rs)
    │       ├── rows/                   ← UNCHANGED (already split by entity)
    │       └── repos/
    │           ├── mod.rs              ← Repos struct + pub use all repos
    │           ├── core/               ← session, project, area, objective, key_result
    │           │   ├── mod.rs
    │           │   ├── session_repo.rs
    │           │   ├── session_context.rs
    │           │   ├── project_repo.rs
    │           │   ├── area_repo.rs
    │           │   ├── objective.rs
    │           │   └── key_result.rs
    │           ├── tasks/              ← action + task repos
    │           │   ├── mod.rs
    │           │   ├── action_repo/
    │           │   │   ├── mod.rs      ← ActionRepo public API
    │           │   │   ├── crud.rs     ← create/read/update/delete (from action_repo.rs)
    │           │   │   ├── query.rs    ← filters + complex queries (from action_repo.rs)
    │           │   │   └── search.rs   ← FTS + embedding search (from action_repo.rs)
    │           │   ├── task_repo/
    │           │   │   ├── mod.rs      ← TaskRepo public API
    │           │   │   ├── crud.rs     ← create/read/update/delete (from task_repo.rs)
    │           │   │   ├── query.rs    ← TaskFilter, complex queries (from task_repo.rs)
    │           │   │   └── analytics.rs ← task analytics + reporting (from task_repo.rs)
    │           │   ├── task_group.rs
    │           │   └── task_execution_repo.rs
    │           ├── finance/            ← all 6 finance repos
    │           │   ├── mod.rs
    │           │   ├── account_repo.rs
    │           │   ├── transaction_repo.rs
    │           │   ├── budget_repo.rs
    │           │   ├── goal_repo.rs
    │           │   ├── investment_repo.rs
    │           │   └── liability_repo.rs
    │           ├── cognitive/          ← all cognitive memory repos
    │           │   ├── mod.rs
    │           │   ├── semantic_fact.rs
    │           │   ├── episodic_memory.rs
    │           │   ├── accumulated_observation.rs
    │           │   ├── procedural_rule.rs
    │           │   ├── annotation.rs
    │           │   └── event_log.rs
    │           ├── productivity/       ← productivity tracking repos
    │           │   ├── mod.rs
    │           │   └── [existing repos — no change, just relocated]
    │           ├── scheduling/
    │           │   ├── mod.rs
    │           │   ├── cron_repo.rs
    │           │   └── calendar_repo.rs
    │           ├── learning/
    │           │   ├── mod.rs
    │           │   ├── usage_repo.rs
    │           │   ├── outcome_repo.rs
    │           │   └── strategy_repo.rs
    │           └── shared/
    │               ├── mod.rs
    │               ├── custom_columns.rs
    │               └── entity_link.rs
    │
    ├── domain/                         ← EXPANDED (currently only 546 LOC)
    │   └── src/
    │       ├── lib.rs
    │       ├── para/
    │       │   ├── mod.rs
    │       │   ├── area.rs             ← Area, AreaPatch, AreaStatus
    │       │   └── project.rs          ← Project, ProjectPatch, ProjectStatus
    │       └── okr/
    │           ├── mod.rs
    │           ├── objective.rs        ← Objective, ObjectivePatch
    │           └── key_result.rs       ← KeyResult, KeyResultPatch, KrStatus
    │
    ├── providers/                      ← RESTRUCTURED (port + adapters)
    │   └── src/
    │       ├── lib.rs
    │       ├── port/
    │       │   ├── mod.rs
    │       │   └── provider.rs         ← LlmProvider trait + ChatParams + LlmResponse (from types.rs)
    │       ├── types/
    │       │   ├── mod.rs
    │       │   └── streaming.rs        ← streaming types + ToolCall (from types.rs)
    │       ├── adapters/
    │       │   ├── mod.rs
    │       │   ├── anthropic/
    │       │   │   ├── mod.rs          ← AnthropicNativeProvider struct + impl LlmProvider
    │       │   │   ├── client.rs       ← HTTP client setup, auth headers (from anthropic_native.rs)
    │       │   │   ├── streaming.rs    ← SSE stream parsing + thinking blocks (from anthropic_native.rs)
    │       │   │   ├── cache.rs        ← prompt cache header construction (from anthropic_native.rs)
    │       │   │   └── retry.rs        ← retry + rate limit handling (from anthropic_native.rs)
    │       │   └── openai_compat/
    │       │       ├── mod.rs          ← OpenAiCompatProvider (all non-Anthropic)
    │       │       ├── client.rs       ← HTTP client, auth (from openai_compat.rs)
    │       │       └── streaming.rs    ← SSE stream parsing (from openai_compat.rs)
    │       ├── manager/
    │       │   ├── mod.rs              ← ProviderManager public API
    │       │   ├── failover.rs         ← failover logic (from manager.rs)
    │       │   ├── circuit_breaker.rs  ← circuit breaker state (from manager.rs)
    │       │   └── retry.rs            ← retry policy (from manager.rs)
    │       └── registry/
    │           ├── mod.rs
    │           └── detection.rs        ← provider detection by API key prefix (from registry.rs)
    │
    ├── session/                        ← UNCHANGED (already small + clean)
    ├── scheduling/                     ← SLIGHTLY RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── types.rs                ← CronJob, CronPayload, CronSchedule (UNCHANGED)
    │       └── service/
    │           ├── mod.rs              ← CronService public API
    │           ├── scheduler.rs        ← scheduling logic (from service/mod.rs)
    │           ├── executor.rs         ← job execution + cleanup (from service/mod.rs)
    │           └── persistence.rs      ← load/save jobs to DB (from service/mod.rs)
    │
    ├── context_engine/                 ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── types/
    │       │   ├── mod.rs
    │       │   ├── assembled.rs        ← AssembledContext, ContextItem
    │       │   └── priority.rs         ← ContextPriority enum
    │       ├── budget.rs               ← UNCHANGED (BudgetAllocator)
    │       ├── token_counter.rs        ← UNCHANGED
    │       ├── inventory.rs            ← UNCHANGED
    │       ├── assembler/
    │       │   ├── mod.rs              ← ContextEngine public API
    │       │   ├── assembly.rs         ← main context selection algorithm (from assembler.rs)
    │       │   ├── cache.rs            ← SHA-256 cache logic (from assembler.rs)
    │       │   └── priority_sort.rs    ← waterfall token allocation (from assembler.rs)
    │       └── history_compressor/
    │           ├── mod.rs              ← HistoryCompressor public API
    │           ├── compressor.rs       ← summarization logic (from history_compressor.rs)
    │           └── modes.rs            ← CompressorMode enum + selection (from history_compressor.rs)
    │
    ├── cognitive/                      ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── domain/
    │       │   ├── mod.rs
    │       │   ├── fact.rs             ← SemanticFact, FactValidity, BiTemporalFact
    │       │   ├── episode.rs          ← EpisodicMemory, ConversationSnapshot
    │       │   ├── observation.rs      ← AccumulatedObservation, ImportanceScore
    │       │   ├── procedural.rs       ← ProceduralRule
    │       │   └── events.rs           ← PipelineEvent variants
    │       ├── application/
    │       │   ├── mod.rs
    │       │   ├── consolidation.rs    ← ConsolidationHandler (UNCHANGED logic)
    │       │   ├── extraction.rs       ← ExtractionHandler (UNCHANGED logic)
    │       │   ├── reflection.rs       ← ReflectionHandler (UNCHANGED logic)
    │       │   ├── situation.rs        ← SituationComputer (UNCHANGED logic)
    │       │   └── salience.rs         ← SalienceScorer (UNCHANGED logic)
    │       ├── infrastructure/
    │       │   ├── mod.rs
    │       │   ├── repos/              ← all repo types (UNCHANGED, just moved under infra/)
    │       │   │   ├── mod.rs
    │       │   │   ├── semantic_fact.rs
    │       │   │   ├── episodic_memory.rs
    │       │   │   ├── accumulated_observation.rs
    │       │   │   ├── procedural_rule.rs
    │       │   │   ├── annotation.rs
    │       │   │   └── event_log.rs
    │       │   └── search/             ← UNCHANGED (already in search/)
    │       │       ├── mod.rs
    │       │       └── bm25.rs
    │       ├── retrieval.rs            ← UNCHANGED (memory retrieval strategies)
    │       ├── memory_retriever.rs     ← UNCHANGED
    │       └── background/
    │           ├── mod.rs              ← background task dispatch
    │           ├── pipeline.rs         ← PipelineEvent SSE stream (from background.rs)
    │           └── dispatcher.rs       ← task routing + scheduling (from background.rs)
    │
    ├── tools/                          ← RESTRUCTURED by domain
    │   └── src/
    │       ├── lib.rs                  ← re-exports all tool structs + ToolRegistry
    │       ├── registry.rs             ← ToolRegistry (UNCHANGED)
    │       ├── embedding_engine.rs     ← EmbeddingEngine + EmbeddingStore (UNCHANGED)
    │       ├── ai/                     ← AI-native tools
    │       │   ├── mod.rs
    │       │   ├── memory_tool.rs      ← MemoryTool (UNCHANGED logic)
    │       │   ├── learning_tool.rs    ← LearningTool (UNCHANGED logic)
    │       │   ├── annotate.rs         ← AnnotateTool (UNCHANGED logic)
    │       │   ├── context_request.rs  ← ContextRequestTool (UNCHANGED logic)
    │       │   └── delegation.rs       ← DelegationTool (UNCHANGED logic)
    │       ├── system/                 ← OS/filesystem tools
    │       │   ├── mod.rs
    │       │   ├── filesystem/
    │       │   │   ├── mod.rs          ← FilesystemTool public API
    │       │   │   ├── read.rs         ← read action (from filesystem.rs)
    │       │   │   ├── write.rs        ← write action (from filesystem.rs)
    │       │   │   ├── list.rs         ← list action (from filesystem.rs)
    │       │   │   └── delete.rs       ← delete action (from filesystem.rs)
    │       │   ├── spawn.rs            ← SpawnTool (UNCHANGED logic)
    │       │   ├── glob_tool.rs        ← GlobTool (UNCHANGED logic)
    │       │   └── grep.rs             ← GrepTool (UNCHANGED logic)
    │       ├── web/                    ← Web/browser tools
    │       │   ├── mod.rs
    │       │   ├── browser/
    │       │   │   ├── mod.rs          ← BrowserTool public API
    │       │   │   ├── automation.rs   ← browser automation core (from browser.rs)
    │       │   │   └── extension.rs    ← Claude browser extension bridge (from browser.rs)
    │       │   └── web.rs              ← WebTool (UNCHANGED logic)
    │       ├── productivity/           ← productivity-domain tools
    │       │   ├── mod.rs
    │       │   ├── okr_tool.rs         ← OkrTool (UNCHANGED logic)
    │       │   ├── project_tool.rs     ← ProjectTool (UNCHANGED logic)
    │       │   ├── area_tool.rs        ← AreaTool (UNCHANGED logic)
    │       │   └── cron_tool.rs        ← CronTool (UNCHANGED logic)
    │       ├── interaction/            ← user interaction tools
    │       │   ├── mod.rs
    │       │   ├── ask_user/
    │       │   │   ├── mod.rs          ← AskUserTool public API
    │       │   │   ├── question.rs     ← question building (from ask_user.rs)
    │       │   │   ├── dispatch.rs     ← platform dispatch (from ask_user.rs)
    │       │   │   └── retry.rs        ← timeout + retry logic (from ask_user.rs)
    │       │   └── docs.rs             ← DocsTool (UNCHANGED logic)
    │       └── agent/                  ← agent management tools
    │           ├── mod.rs
    │           └── spawn.rs            ← SpawnAgentTool (UNCHANGED logic)
    │
    ├── channels/                       ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs                  ← Channel trait + re-exports
    │       ├── port/
    │       │   ├── mod.rs
    │       │   └── channel.rs          ← Channel trait + structured interaction types
    │       ├── manager.rs              ← ChannelManager (UNCHANGED)
    │       ├── formatter.rs            ← markdown→platform formatter (UNCHANGED)
    │       ├── utils.rs                ← UNCHANGED
    │       ├── adapters/               ← platform implementations
    │       │   ├── mod.rs
    │       │   ├── telegram/
    │       │   │   ├── mod.rs          ← TelegramChannel struct + impl Channel
    │       │   │   ├── polling.rs      ← long polling loop (from telegram.rs)
    │       │   │   ├── messages.rs     ← message type parsing (from telegram.rs)
    │       │   │   ├── keyboard.rs     ← inline keyboard builder (from telegram.rs)
    │       │   │   └── voice.rs        ← Groq transcription (from telegram.rs)
    │       │   ├── discord/
    │       │   │   ├── mod.rs          ← DiscordChannel struct + impl Channel
    │       │   │   ├── gateway.rs      ← WebSocket Gateway (from discord.rs)
    │       │   │   ├── messages.rs     ← message parsing (from discord.rs)
    │       │   │   └── components.rs   ← message components (from discord.rs)
    │       │   ├── slack/
    │       │   │   ├── mod.rs          ← SlackChannel struct + impl Channel
    │       │   │   ├── socket_mode.rs  ← Socket Mode WebSocket (from slack.rs)
    │       │   │   ├── events.rs       ← event parsing (from slack.rs)
    │       │   │   └── block_kit.rs    ← Block Kit builder (from slack.rs)
    │       │   ├── email/              ← feature-gated
    │       │   │   ├── mod.rs          ← EmailChannel struct + impl Channel
    │       │   │   ├── imap.rs         ← IMAP receive (from email.rs)
    │       │   │   └── smtp.rs         ← SMTP send (from email.rs)
    │       │   └── ws_manager.rs       ← WebSocket manager (UNCHANGED)
    │       └── stubs/                  ← Unimplemented channel stubs (NEW)
    │           ├── mod.rs
    │           ├── feishu.rs           ← FeishuChannel stub (compile-time visibility)
    │           ├── dingtalk.rs         ← DingTalkChannel stub
    │           └── mochat.rs           ← MochatChannel stub
    │
    ├── agent/                          ← RESTRUCTURED (largest refactor)
    │   └── src/
    │       ├── lib.rs                  ← re-exports AgentLoop, AgentRuntime
    │       ├── domain/
    │       │   ├── mod.rs
    │       │   ├── profile/
    │       │   │   ├── mod.rs          ← AgentProfile, AgentManager
    │       │   │   ├── types.rs        ← profile schema (from agent_profile/types.rs)
    │       │   │   ├── manager.rs      ← profile loading + caching (UNCHANGED)
    │       │   │   └── filter.rs       ← tool filter + MCP allowlist logic (from agent_profile/types.rs)
    │       │   ├── persona/
    │       │   │   ├── mod.rs
    │       │   │   ├── chain.rs        ← persona chain (from persona.rs)
    │       │   │   └── scope.rs        ← scope management (from persona.rs)
    │       │   └── confidence/         ← UNCHANGED (already well-organized)
    │       │       ├── mod.rs
    │       │       ├── evaluator.rs
    │       │       ├── log.rs
    │       │       └── types.rs
    │       ├── application/
    │       │   ├── mod.rs
    │       │   ├── agent_loop/
    │       │   │   ├── mod.rs          ← AgentLoop public API (from agent_loop/mod.rs)
    │       │   │   └── builder.rs      ← split into:
    │       │   │       → core_builder.rs     ← storage, providers, channels wiring
    │       │   │       → feature_builder.rs  ← feature packages + plugins
    │       │   │       → channel_builder.rs  ← channel setup
    │       │   │       → cognitive_builder.rs ← memory/cognitive wiring
    │       │   ├── runtime/
    │       │   │   ├── mod.rs          ← AgentRuntime public API
    │       │   │   ├── pipeline.rs     ← 10-step pipeline (from runtime.rs)
    │       │   │   ├── agent_select.rs ← agent selection logic (from runtime.rs)
    │       │   │   └── mcp_filter.rs   ← MCP tool filtering (from runtime.rs)
    │       │   ├── intent_pipeline/
    │       │   │   ├── mod.rs
    │       │   │   ├── analyzer.rs     ← IntentAnalyzer (orchestration)
    │       │   │   ├── classifier/
    │       │   │   │   ├── mod.rs
    │       │   │   │   ├── heuristic.rs  ← rule-based classification (from analysis.rs)
    │       │   │   │   ├── keywords.rs   ← keyword tables (from analysis.rs)
    │       │   │   │   └── llm.rs        ← LLM fallback classification (from analysis.rs)
    │       │   │   ├── router.rs       ← UNCHANGED (direct/reactive dispatch)
    │       │   │   ├── types.rs        ← UNCHANGED
    │       │   │   └── engines/        ← UNCHANGED (already clean)
    │       │   │       ├── mod.rs
    │       │   │       ├── direct.rs
    │       │   │       └── reactive.rs
    │       │   ├── execution/          ← UNCHANGED (already clean)
    │       │   │   ├── mod.rs
    │       │   │   ├── core.rs
    │       │   │   ├── scratchpad.rs
    │       │   │   └── types.rs
    │       │   ├── enrichment/         ← UNCHANGED (already clean)
    │       │   └── learning/           ← UNCHANGED (already clean)
    │       ├── handlers/               ← handler trait implementations
    │       │   ├── mod.rs
    │       │   ├── cognitive/          ← was cognitive_handlers.rs (1,282 lines → split)
    │       │   │   ├── mod.rs          ← wires all cognitive handler impls
    │       │   │   ├── consolidation.rs ← ConsolidationHandler impl
    │       │   │   ├── extraction.rs   ← ExtractionHandler impl
    │       │   │   ├── retrieval.rs    ← MemoryRetrievalHandler impl
    │       │   │   ├── reflection.rs   ← ReflectionHandler impl
    │       │   │   └── embedding.rs    ← EmbeddingHandler impl
    │       │   ├── decomposition.rs    ← UNCHANGED
    │       │   ├── forecast.rs         ← UNCHANGED
    │       │   ├── planning.rs         ← UNCHANGED
    │       │   ├── execution.rs        ← UNCHANGED
    │       │   ├── proactive.rs        ← UNCHANGED
    │       │   └── suggestion_applier.rs ← UNCHANGED
    │       ├── infrastructure/
    │       │   ├── mod.rs
    │       │   ├── context_sources/    ← UNCHANGED (already in subfolder)
    │       │   ├── content_registry/   ← UNCHANGED (already in subfolder)
    │       │   ├── skill_loader.rs     ← UNCHANGED
    │       │   └── output/             ← was output/
    │       │       ├── mod.rs
    │       │       ├── cost_tracker/
    │       │       │   ├── mod.rs      ← CostTracker public API
    │       │       │   ├── tracker.rs  ← record logic (from cost_tracker.rs)
    │       │       │   └── pricing.rs  ← pricing tables (from cost_tracker.rs)
    │       │       └── validator.rs    ← UNCHANGED
    │       └── services/
    │           ├── mod.rs
    │           ├── reminders.rs        ← UNCHANGED
    │           ├── recurring_tasks.rs  ← UNCHANGED
    │           ├── notifications.rs    ← UNCHANGED
    │           ├── session_cleanup.rs  ← UNCHANGED
    │           └── memory_maintenance.rs ← UNCHANGED
    │
    ├── feature-todo/                   ← MARK AS DEPRECATED (no restructuring needed)
    │   └── src/
    │       ├── lib.rs                  ← add #[deprecated] marker + doc comment
    │       └── [all existing files]    ← UNCHANGED
    │
    ├── feature-tasks/                  ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── domain/
    │       │   ├── mod.rs
    │       │   ├── task/               ← split from types.rs (1,780 lines)
    │       │   │   ├── mod.rs
    │       │   │   ├── core.rs         ← Task struct, TaskStatus, TaskPriority
    │       │   │   ├── recurrence.rs   ← TaskRecurrence, rrule parsing types
    │       │   │   └── execution.rs    ← TaskExecution, ExecutionStatus
    │       │   ├── activity.rs         ← TaskActivity, ActivityKind
    │       │   ├── suggestion.rs       ← TaskSuggestion types
    │       │   └── filters.rs          ← TaskFilter, TaskQuery
    │       ├── application/
    │       │   ├── mod.rs
    │       │   ├── tool/
    │       │   │   ├── mod.rs          ← tool action dispatch
    │       │   │   ├── create.rs       ← create action (from tool/mod.rs)
    │       │   │   ├── query.rs        ← query + search actions
    │       │   │   ├── mutate.rs       ← update + status actions
    │       │   │   ├── plan.rs         ← day planning action
    │       │   │   ├── decompose.rs    ← decomposition action
    │       │   │   ├── recurrence.rs   ← recurrence management actions
    │       │   │   └── execute.rs      ← execution + suggestion actions
    │       │   ├── forecast.rs         ← UNCHANGED
    │       │   └── scoring.rs          ← urgency + complexity scoring
    │       └── infrastructure/
    │           ├── mod.rs
    │           ├── rrule_utils.rs      ← UNCHANGED
    │           └── search.rs           ← UNCHANGED
    │
    ├── feature-finance/                ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── domain/
    │       │   ├── mod.rs
    │       │   ├── account.rs          ← account types (from types.rs)
    │       │   ├── transaction.rs      ← transaction types (from types.rs)
    │       │   ├── budget.rs           ← budget types (from types.rs)
    │       │   ├── goal.rs             ← goal types (from types.rs)
    │       │   └── investment.rs       ← investment + liability types (from types.rs)
    │       ├── application/
    │       │   ├── mod.rs
    │       │   ├── config.rs           ← FinanceConfig loading (from config.rs)
    │       │   └── price_service.rs    ← UNCHANGED (price lookup)
    │       └── infrastructure/
    │           ├── mod.rs
    │           └── tool/               ← split from tool/ (each file UNCHANGED logic)
    │               ├── mod.rs
    │               ├── transactions/
    │               │   ├── mod.rs      ← TransactionTool
    │               │   ├── crud.rs     ← create/read/update (from transactions.rs)
    │               │   ├── search.rs   ← search + filter (from transactions.rs)
    │               │   └── reports.rs  ← reporting (from transactions.rs)
    │               ├── accounts.rs     ← UNCHANGED
    │               ├── budgets.rs      ← UNCHANGED
    │               ├── goals.rs        ← UNCHANGED
    │               ├── investments.rs  ← UNCHANGED
    │               └── reports.rs      ← UNCHANGED
    │
    ├── feature-notes/                  ← UNCHANGED (already small + clean)
    ├── feature-coaching/               ← UNCHANGED (already well-organized)
    │
    ├── feature-productivity/           ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs
    │       ├── domain/
    │       │   ├── mod.rs
    │       │   ├── session.rs          ← FocusSession, SessionStatus (from types.rs)
    │       │   ├── activity.rs         ← ActivityEvent, TrackingRule (from types.rs)
    │       │   ├── distraction.rs      ← DistractionRecord, NudgeEvent (from types.rs)
    │       │   └── insights.rs         ← WeeklyInsight, MonthlyInsight types (from types.rs)
    │       ├── application/
    │       │   ├── mod.rs
    │       │   ├── focus.rs            ← UNCHANGED
    │       │   ├── auto_focus.rs       ← UNCHANGED
    │       │   ├── nudge.rs            ← UNCHANGED
    │       │   └── insights.rs         ← UNCHANGED
    │       └── infrastructure/
    │           ├── mod.rs
    │           ├── repos/              ← UNCHANGED (already in repos/)
    │           ├── tracker/            ← UNCHANGED (already in tracker/)
    │           ├── distraction/        ← UNCHANGED (already in distraction/)
    │           └── intelligence/       ← UNCHANGED (already in intelligence/)
    │               ├── session_aggregator/
    │               │   ├── mod.rs      ← public API
    │               │   ├── aggregation.rs ← core aggregation (from session_aggregator.rs)
    │               │   ├── scoring.rs  ← quality scoring (from session_aggregator.rs)
    │               │   └── summary.rs  ← AI summary generation (from session_aggregator.rs)
    │               └── [other files UNCHANGED]
    │
    ├── mcp/                            ← UNCHANGED (already well-organized)
    ├── plugin-runtime/                 ← UNCHANGED (already well-organized)
    ├── activity-log/                   ← UNCHANGED (already well-organized)
    │
    ├── app-core/                       ← RESTRUCTURED
    │   └── src/
    │       ├── lib.rs                  ← re-exports AppCore, EntityUpdate, HandlerResult
    │       ├── state.rs                ← UNCHANGED (AppCore struct, 176 lines)
    │       ├── init/
    │       │   ├── mod.rs              ← AppCore::new() public API
    │       │   ├── database.rs         ← DB init + migrations (from init.rs)
    │       │   ├── providers.rs        ← provider wiring (from init.rs)
    │       │   ├── agents.rs           ← agent loop construction (from init.rs)
    │       │   ├── channels.rs         ← channel setup (from init.rs)
    │       │   └── features.rs         ← feature package registration (from init.rs)
    │       └── handlers/
    │           ├── mod.rs
    │           ├── chat/
    │           │   ├── mod.rs          ← chat handler public API
    │           │   ├── session.rs      ← session management (from chat.rs)
    │           │   ├── streaming.rs    ← streaming response (from chat.rs)
    │           │   ├── history.rs      ← message history + search (from chat.rs)
    │           │   └── pipeline.rs     ← main message pipeline (from chat.rs)
    │           ├── tasks.rs            ← UNCHANGED (597 lines — acceptable)
    │           ├── finance.rs          ← UNCHANGED (791 lines — split deferred)
    │           ├── productivity/
    │           │   ├── mod.rs
    │           │   ├── sessions.rs     ← focus session ops (from productivity.rs)
    │           │   ├── tracking.rs     ← activity tracking ops (from productivity.rs)
    │           │   └── reports.rs      ← insights + reporting (from productivity.rs)
    │           ├── cognitive.rs        ← UNCHANGED (662 lines — acceptable)
    │           ├── notes.rs            ← UNCHANGED
    │           ├── projects.rs         ← UNCHANGED
    │           ├── timeline.rs         ← UNCHANGED
    │           ├── settings.rs         ← UNCHANGED
    │           ├── coaching.rs         ← UNCHANGED
    │           ├── work_context.rs     ← UNCHANGED
    │           ├── workflows.rs        ← UNCHANGED
    │           ├── columns.rs          ← UNCHANGED
    │           └── key_results.rs      ← UNCHANGED
    │
    ├── desktop-shared/                 ← RESTRUCTURED by domain
    │   └── src/
    │       ├── lib.rs
    │       ├── commands/
    │       │   ├── mod.rs              ← re-exports all command types
    │       │   ├── chat.rs             ← chat commands (from commands.rs)
    │       │   ├── tasks.rs            ← task commands (from commands.rs)
    │       │   ├── finance.rs          ← finance commands (from commands.rs)
    │       │   ├── productivity.rs     ← productivity commands (from commands.rs)
    │       │   ├── cognitive.rs        ← cognitive commands (from commands.rs)
    │       │   ├── notes.rs            ← notes commands (from commands.rs)
    │       │   ├── projects.rs         ← project commands (from commands.rs)
    │       │   ├── settings.rs         ← settings commands (from commands.rs)
    │       │   └── work_context.rs     ← work context commands (from commands.rs)
    │       ├── events/
    │       │   ├── mod.rs              ← re-exports all event types
    │       │   └── [domain-split from events.rs]
    │       ├── cognitive_commands.rs   ← UNCHANGED
    │       ├── entity_link_types.rs    ← UNCHANGED
    │       └── permissions.rs          ← UNCHANGED
    │
    └── desktop/                        ← MINOR RESTRUCTURE
        └── src/
            ├── lib.rs                  ← UNCHANGED
            ├── main.rs                 ← UNCHANGED
            ├── dev_server/
            │   ├── mod.rs              ← DevServer public API
            │   ├── routes.rs           ← HTTP route definitions (from dev_server.rs)
            │   └── handlers.rs         ← request handlers (from dev_server.rs)
            ├── commands/               ← UNCHANGED (all thin adapters)
            ├── focus_timer/
            │   ├── mod.rs
            │   └── timer.rs            ← UNCHANGED logic
            ├── oauth/                  ← UNCHANGED
            ├── file_watcher.rs         ← UNCHANGED
            └── shell_hook.rs           ← UNCHANGED
```

---

## Part 3 — Step-by-Step Refactor Plan

### Phase 0 — Preparation (No Code Changes)

**Goal:** Establish baseline and safety net before any moves.

```bash
# 0.1 Capture baseline
cargo nextest run --workspace 2>&1 | tee /tmp/baseline-tests.txt
cargo clippy --workspace --all-targets --all-features 2>&1 | tee /tmp/baseline-clippy.txt
cargo build --workspace 2>&1 | tee /tmp/baseline-build.txt

# 0.2 Count current test pass rate
grep "passed" /tmp/baseline-tests.txt

# 0.3 Create a git branch
git checkout -b refactor/structural-cleanup

# 0.4 Create workspace/scripts/ for helper commands
mkdir -p workspace/scripts
```

**Phase Gate:** All tests pass. Zero clippy warnings. Record counts.

---

### Phase 1 — `common` Restructure

**Effort:** Low | **Risk:** Medium (many dependents)

**Steps:**
1. Create `src/errors/`, `src/types/`, keep `src/utils/`
2. Move `error.rs` → `errors/klyntbot.rs`, extract domain error variants to `errors/domain.rs`
3. Move `types.rs` → `types/core.rs`, `entity_card.rs` → `types/entity_card.rs`, `prompts.rs` → `types/prompts.rs`
4. Split `utils/stream_renderer.rs` (557L) → `utils/sse.rs` + `utils/diff.rs` + `utils/buffer.rs`
5. Rename `utils/terminal/thinking_renderer.rs` → `utils/terminal/thinking.rs`
6. Update `lib.rs` with all new `mod` declarations + `pub use` re-exports (all existing public names preserved)
7. Run: `cargo build --workspace && cargo nextest run --workspace`

**Cargo.toml changes:** None (only internal structure changes)

**Verification:** All downstream crates (all 26 others) must compile without changes because `lib.rs` re-exports identical names.

---

### Phase 2 — `config` Restructure

**Effort:** Low | **Risk:** Low (config is read-only from other crates)

**Steps:**
1. Create `src/loader/` directory
2. Split `loader.rs` (786L) into:
   - `loader/file_io.rs` — `read_config_file()`, `write_config_file()`
   - `loader/env_override.rs` — `apply_env_overrides()`
   - `loader/merge.rs` — `merge_config()` deep merge logic
   - `loader/defaults.rs` — `Config::default_with_env()`
   - `loader/mod.rs` — re-exports `load()`, `load_with_env_overrides()`
3. `src/schema/mod.rs` stays (509L) — it's a re-export hub, acceptable size
4. Update `src/lib.rs` with new mod path
5. Run: `cargo build --workspace`

---

### Phase 3 — `storage` Repo Vertical-Slice Grouping

**Effort:** Medium | **Risk:** Medium

**Steps:**
1. Create subdirectories: `repos/core/`, `repos/tasks/`, `repos/finance/`, `repos/cognitive/`, `repos/productivity/`, `repos/scheduling/`, `repos/learning/`, `repos/shared/`
2. Move each repo file into its domain folder (e.g., `repos/task_repo.rs` → `repos/tasks/task_repo/`)
3. For `task_repo.rs` (2,450L), split into:
   - `tasks/task_repo/crud.rs` — create/read/update/delete operations
   - `tasks/task_repo/query.rs` — TaskFilter + complex queries
   - `tasks/task_repo/analytics.rs` — reporting + aggregation
   - `tasks/task_repo/mod.rs` — TaskRepo struct + pub API
4. For `action_repo.rs` (1,190L), split into:
   - `tasks/action_repo/crud.rs`
   - `tasks/action_repo/query.rs`
   - `tasks/action_repo/search.rs` — FTS + embedding search
   - `tasks/action_repo/mod.rs`
5. Split `vector_store.rs` (963L) into `vector_store/` subdir
6. Update `repos/mod.rs` and `lib.rs` with new paths, maintain all existing `pub` exports
7. **Critical:** All `sqlx::query!()` macros must remain verbatim — do not touch SQL strings
8. Run: `cargo nextest run --workspace`

---

### Phase 4 — `providers` Port/Adapter Split

**Effort:** Medium | **Risk:** Medium

**Steps:**
1. Create `src/port/`, `src/adapters/anthropic/`, `src/adapters/openai_compat/`, `src/manager/`, `src/registry/`
2. Extract `LlmProvider` trait + `ChatParams` + `LlmResponse` → `port/provider.rs`
3. Split `anthropic_native.rs` (1,238L):
   - `adapters/anthropic/client.rs` — HTTP client setup
   - `adapters/anthropic/streaming.rs` — SSE parsing + thinking blocks
   - `adapters/anthropic/cache.rs` — cache header logic
   - `adapters/anthropic/mod.rs` — `AnthropicNativeProvider` struct + `impl LlmProvider`
4. Split `manager.rs` (1,046L):
   - `manager/failover.rs` — failover logic
   - `manager/circuit_breaker.rs` — circuit breaker state
   - `manager/retry.rs` — retry policy
   - `manager/mod.rs` — `ProviderManager` public API
5. Move `registry.rs` → `registry/mod.rs` + `registry/detection.rs`
6. Maintain all existing pub exports in `lib.rs`
7. Run: `cargo build --workspace && cargo nextest run -p providers`

---

### Phase 5 — `channels` Platform Split

**Effort:** Medium | **Risk:** Medium

**Steps:**
1. Create `src/port/`, `src/adapters/`, `src/stubs/`
2. Move `Channel` trait → `port/channel.rs`
3. Create platform subdirs: `adapters/telegram/`, `adapters/discord/`, `adapters/slack/`, `adapters/email/`
4. Split `telegram.rs` (1,069L):
   - `adapters/telegram/polling.rs` — long polling loop
   - `adapters/telegram/messages.rs` — message type parsing
   - `adapters/telegram/keyboard.rs` — inline keyboard builder
   - `adapters/telegram/voice.rs` — Groq transcription
   - `adapters/telegram/mod.rs` — TelegramChannel struct
5. Split `discord.rs` (1,255L) into `adapters/discord/{gateway,messages,components,mod}.rs`
6. Split `slack.rs` (1,134L) into `adapters/slack/{socket_mode,events,block_kit,mod}.rs`
7. Split `email.rs` (659L) into `adapters/email/{imap,smtp,mod}.rs` (preserve `#[cfg(feature = "email")]`)
8. Create `stubs/feishu.rs`, `stubs/dingtalk.rs`, `stubs/mochat.rs` with `unimplemented!()` bodies and `#[doc = "NOT IMPLEMENTED"]` markers
9. Run: `cargo build --workspace && cargo nextest run -p channels`

---

### Phase 6 — `cognitive` Clean Architecture Layout

**Effort:** Medium | **Risk:** Low (cognitive is isolated)

**Steps:**
1. Create `src/domain/`, `src/application/`, `src/infrastructure/`, `src/background/`
2. Move domain types to `src/domain/` (fact, episode, observation, procedural, events)
3. Move service logic: `consolidation.rs`, `extraction.rs`, `reflection.rs`, `situation.rs`, `salience.rs` → `src/application/`
4. Move repos → `src/infrastructure/repos/`
5. Move `search/` → `src/infrastructure/search/`
6. Split `background.rs` (1,169L) → `background/pipeline.rs` + `background/dispatcher.rs`
7. Update `lib.rs` with new paths, preserve all pub exports
8. Run: `cargo nextest run -p cognitive`

---

### Phase 7 — `tools` Domain Grouping

**Effort:** Medium | **Risk:** Medium (many tests touch tools)

**Steps:**
1. Create `src/ai/`, `src/system/`, `src/web/`, `src/productivity/`, `src/interaction/`, `src/agent/`
2. Move each tool file to its domain group
3. Split `ask_user.rs` (922L) → `interaction/ask_user/{question,dispatch,retry,mod}.rs`
4. Split `browser.rs` (740L) → `web/browser/{automation,extension,mod}.rs`
5. Split `filesystem.rs` (640L) → `system/filesystem/{read,write,list,delete,mod}.rs`
6. Update `lib.rs` — all existing exported names must remain identical (no rename)
7. Update `registry.rs` if it has hardcoded module paths
8. Run: `cargo nextest run -p tools && cargo nextest run --workspace`

---

### Phase 8 — `agent` Internal Reorganization (Largest Phase)

**Effort:** High | **Risk:** High (most complex crate)

**Sub-steps (do in order, verify after each):**

**8a — Domain layer:**
1. Create `src/domain/profile/`, `src/domain/persona/`, `src/domain/confidence/`
2. Move `agent_profile/` → `src/domain/profile/`
3. Extract filter logic from `agent_profile/types.rs` → `domain/profile/filter.rs`
4. Split `persona.rs` (617L) → `domain/persona/chain.rs` + `domain/persona/scope.rs`
5. Move `confidence/` → `src/domain/confidence/`
6. Run: `cargo build -p agent`

**8b — Application layer:**
1. Create `src/application/agent_loop/`, `src/application/runtime/`, `src/application/intent_pipeline/`
2. Move `agent_loop/` → `src/application/agent_loop/`
3. Split `agent_loop/builder.rs` (1,199L) → four files: `core_builder.rs`, `feature_builder.rs`, `channel_builder.rs`, `cognitive_builder.rs`
4. Move `agent_runtime/` → `src/application/runtime/`
5. Split `agent_runtime/runtime.rs` (1,354L):
   - `runtime/pipeline.rs` — 10-step pipeline
   - `runtime/agent_select.rs` — agent selection
   - `runtime/mcp_filter.rs` — MCP tool filtering
   - `runtime/mod.rs` — AgentRuntime struct + public API
6. Move `intent_pipeline/` → `src/application/intent_pipeline/`
7. Split `intent_pipeline/analysis.rs` (1,356L):
   - `intent_pipeline/classifier/heuristic.rs` — rule-based classification
   - `intent_pipeline/classifier/keywords.rs` — keyword tables
   - `intent_pipeline/classifier/llm.rs` — LLM fallback
   - `intent_pipeline/analyzer.rs` — IntentAnalyzer orchestration
8. Run: `cargo build -p agent`

**8c — Handlers layer:**
1. Create `src/handlers/cognitive/`
2. Split `cognitive_handlers.rs` (1,282L):
   - `handlers/cognitive/consolidation.rs`
   - `handlers/cognitive/extraction.rs`
   - `handlers/cognitive/retrieval.rs`
   - `handlers/cognitive/reflection.rs`
   - `handlers/cognitive/embedding.rs`
   - `handlers/cognitive/mod.rs`
3. Move all other handler files: `handlers/{decomposition,forecast,planning,execution,proactive,suggestion_applier}.rs`
4. Run: `cargo build -p agent`

**8d — Infrastructure layer:**
1. Create `src/infrastructure/`
2. Move `context_sources/`, `content_registry/`, `skill_loader.rs` → `src/infrastructure/`
3. Move `output/` → `src/infrastructure/output/`
4. Split `output/cost_tracker.rs` (481L):
   - `output/cost_tracker/tracker.rs` — record logic
   - `output/cost_tracker/pricing.rs` — pricing tables
   - `output/cost_tracker/mod.rs`
5. Run: `cargo build -p agent`

**8e — Services layer:**
1. Create `src/services/`
2. Move `reminders.rs`, `recurring_tasks.rs`, `notifications.rs`, `session_cleanup_service.rs`, `memory_maintenance_service.rs` → `src/services/`
3. Run: `cargo nextest run -p agent`

**8f — Update lib.rs:**
1. Rebuild `lib.rs` with all `mod` declarations in new paths
2. Preserve all existing `pub use` exports (AgentLoop, AgentRuntime, etc.)
3. Run: `cargo nextest run --workspace`

---

### Phase 9 — `context_engine` Restructure

**Effort:** Low | **Risk:** Low

**Steps:**
1. Create `src/assembler/`, `src/history_compressor/`, `src/types/`
2. Split `assembler.rs` (1,209L):
   - `assembler/assembly.rs` — selection algorithm
   - `assembler/cache.rs` — SHA-256 cache
   - `assembler/priority_sort.rs` — waterfall allocation
   - `assembler/mod.rs` — ContextEngine public API
3. Split `history_compressor.rs` (740L):
   - `history_compressor/compressor.rs` — summarization
   - `history_compressor/modes.rs` — CompressorMode selection
   - `history_compressor/mod.rs`
4. Run: `cargo build --workspace`

---

### Phase 10 — Feature Crates Restructure

**Effort:** Medium | **Risk:** Low (feature crates are isolated)

**Steps (apply to each feature crate in parallel):**

**`feature-tasks`:**
1. Create `src/domain/task/`, `src/application/tool/`
2. Split `types.rs` (1,780L):
   - `domain/task/core.rs` — Task struct + status FSM
   - `domain/task/recurrence.rs` — recurrence types
   - `domain/task/execution.rs` — TaskExecution
   - `domain/activity.rs`, `domain/suggestion.rs`, `domain/filters.rs`
3. Split `tool/mod.rs` (917L) into per-action files in `application/tool/`
4. Move `forecast.rs`, `scoring.rs`, `complexity.rs` → `application/`
5. Move `rrule_utils.rs`, `search.rs` → `infrastructure/`

**`feature-finance`:**
1. Split `types.rs` (698L) into `domain/` per entity
2. Split `tool/transactions.rs` (1,003L) into `infrastructure/tool/transactions/{crud,search,reports,mod}.rs`

**`feature-productivity`:**
1. Create `src/domain/`
2. Split `types.rs` (1,126L) into domain types by entity
3. Split `intelligence/session_aggregator.rs` (1,171L) into three files

**`feature-todo`:**
1. Add `#[deprecated(note = "Use feature-tasks. This crate will be removed post-1.0.")]` to `lib.rs`
2. Add doc comment to `lib.rs` explaining deprecation timeline
3. No other changes

Run: `cargo nextest run --workspace` after all feature crates done.

---

### Phase 11 — `app-core` Restructure

**Effort:** Medium | **Risk:** High (many callers)

**Steps:**
1. Create `src/init/`
2. Split `init.rs` (1,350L):
   - `init/database.rs` — DB pool + migration runner
   - `init/providers.rs` — provider factory + failover setup
   - `init/agents.rs` — AgentLoop builder calls
   - `init/channels.rs` — channel setup
   - `init/features.rs` — FeaturePackage registration
   - `init/mod.rs` — `AppCore::new()` — calls each sub-init in order
3. Split `handlers/chat.rs` (1,114L):
   - `handlers/chat/session.rs`
   - `handlers/chat/streaming.rs`
   - `handlers/chat/history.rs`
   - `handlers/chat/pipeline.rs`
   - `handlers/chat/mod.rs`
4. Split `handlers/productivity.rs` (983L):
   - `handlers/productivity/sessions.rs`
   - `handlers/productivity/tracking.rs`
   - `handlers/productivity/reports.rs`
   - `handlers/productivity/mod.rs`
5. Verify `state.rs` (176L) is unchanged
6. Run: `cargo nextest run --workspace`

---

### Phase 12 — `desktop-shared` Domain Split

**Effort:** Low | **Risk:** Low

**Steps:**
1. Create `src/commands/`
2. Read through `commands.rs` (1,516L) and group by domain (chat, tasks, finance, productivity, cognitive, notes, projects, settings, work_context)
3. Create one file per domain in `src/commands/`
4. Create `src/commands/mod.rs` with `pub use` for all types
5. Same for `events/`
6. Update `lib.rs`
7. Run: `cargo build --workspace`

---

### Phase 13 — `desktop` Dev Server Split

**Effort:** Low | **Risk:** Low

**Steps:**
1. Create `src/dev_server/`
2. Split `dev_server.rs` (592L) → `routes.rs` + `handlers.rs` + `mod.rs`
3. Run: `cargo build -p desktop`

---

### Phase 14 — New Docs: `ai-coding-rules.md`

Create `/docs/ai-coding-rules.md` with:
- Module boundary conventions
- Which `pub`/`pub(crate)` to use and when
- Tool registration pattern
- FeaturePackage implementation checklist
- Dependency inversion pattern for new handlers
- Test conventions (always `StoragePool::connect_in_memory()`)
- How to add a new channel (Channel trait implementation checklist)
- How to add a new LLM provider

---

### Phase 15 — Final Verification

```bash
# 15.1 Full build
cargo build --workspace

# 15.2 All tests (compare to baseline)
cargo nextest run --workspace 2>&1 | tee /tmp/post-refactor-tests.txt
diff /tmp/baseline-tests.txt /tmp/post-refactor-tests.txt

# 15.3 Zero clippy warnings
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error"

# 15.4 Formatting clean
cargo fmt --all --check

# 15.5 Doc tests
cargo test --workspace --doc

# 15.6 Feature gate builds
cargo build --workspace --no-default-features
cargo build --workspace --all-features

# 15.7 Desktop UI
cd desktop-ui && bun run build && bun run lint:fix

# 15.8 Count remaining large files
find crates -name "*.rs" | xargs wc -l | awk '$1 > 400' | sort -rn
```

---

## Part 4 — Complete Backlog.md

See separate `BACKLOG.md` file at workspace root.

---

## Part 5 — Verification Commands

### Per-Phase Verification Script

```bash
#!/usr/bin/env bash
# workspace/scripts/verify.sh
# Run after each refactor phase

set -e

PHASE=${1:-"all"}

echo "=== Klyntbot Refactor Verification: Phase $PHASE ==="

# Build check
echo "→ Building workspace..."
cargo build --workspace 2>&1 | tail -5
echo "✓ Build OK"

# Test suite
echo "→ Running test suite..."
RESULT=$(cargo nextest run --workspace --output-format json 2>/dev/null | \
  jq -r '"Passed: " + (.summary.pass | tostring) + " / Failed: " + (.summary.fail | tostring)')
echo "✓ Tests: $RESULT"

# Clippy
echo "→ Running clippy..."
WARNINGS=$(cargo clippy --workspace --all-targets --all-features 2>&1 | grep -c "^warning" || true)
ERRORS=$(cargo clippy --workspace --all-targets --all-features 2>&1 | grep -c "^error" || true)
echo "✓ Clippy: $ERRORS errors, $WARNINGS warnings"
if [ "$ERRORS" -gt "0" ]; then exit 1; fi

# Format
echo "→ Checking format..."
cargo fmt --all --check && echo "✓ Format OK" || echo "✗ Format issues found"

# Large file check
echo "→ Files > 400 lines..."
find crates -name "*.rs" | xargs wc -l 2>/dev/null | \
  awk '$1 > 400 && $2 != "total"' | sort -rn | head -20

echo "=== Verification complete ==="
```

### Targeted Crate Tests

```bash
# Test a single crate after its phase
cargo nextest run -p common
cargo nextest run -p config
cargo nextest run -p storage
cargo nextest run -p providers
cargo nextest run -p channels
cargo nextest run -p cognitive
cargo nextest run -p tools
cargo nextest run -p agent
cargo nextest run -p context_engine
cargo nextest run -p feature-tasks
cargo nextest run -p feature-finance
cargo nextest run -p feature-productivity
cargo nextest run -p feature-coaching
cargo nextest run -p app-core
cargo nextest run -p desktop-shared

# Integration tests
cargo nextest run --test '*'
cargo test --workspace --doc
```

### Dead Code Scan

```bash
# Find remaining allow(dead_code) suppression (should be minimal)
grep -rn "#\[allow(dead_code" crates/ --include="*.rs"

# Find remaining TODOs (should match BACKLOG.md)
grep -rn "TODO\|FIXME\|HACK" crates/ --include="*.rs"

# Find unimplemented! / todo! macros (should only be in stubs/)
grep -rn "unimplemented!()\|todo!()" crates/ --include="*.rs"
```

### Import Boundary Verification

```bash
# Ensure no cross-layer violations (manual + grep)
# L0 (common) should not import any workspace crate
grep -rn "use agent::\|use tools::\|use storage::\|use channels::" \
  crates/common/src/ --include="*.rs"
# Should output: nothing

# L1 should not import L3+
grep -rn "use agent::\|use cognitive::\|use app_core::" \
  crates/config/src/ crates/bus/src/ --include="*.rs"
# Should output: nothing
```

---

## Addendum: AI Coding Rules (to become `/docs/ai-coding-rules.md`)

```markdown
# AI Coding Rules for Klyntbot

## 0. The Prime Directive
NEVER change logic, function signatures, trait implementations, or behavior
during a structural refactor. Move code; do not rewrite it.

## 1. Module Public Visibility
- `pub` — only for types consumed by other workspace crates or the klyntbot facade
- `pub(crate)` — for types shared within a crate but not part of its public contract
- `pub(super)` — for implementation helpers visible only to the parent module
- Default private — for module-internal state

## 2. New Tool Checklist
1. Create struct in correct domain folder (ai/, system/, web/, productivity/, interaction/, agent/)
2. Implement `ToolExecute<P>` where P: `ToolParams`
3. Add `#[derive(Tool, ToolParams)]` to struct
4. Register in `ToolRegistry::default()` in `tools/src/registry.rs`
5. Add to agent profile's `tools` list in `agents/{name}/AGENT.md`
6. Write at least one unit test using `MockRoutingContext`

## 3. New Feature Package Checklist
1. Create `crates/feature-{name}/`
2. Implement `FeaturePackage` trait (tools, migration, config_default, health_check)
3. Define `FeatureMigration` with unique `feature_name` + monotonic `version`
4. Register in `app-core/src/init/features.rs`
5. Add crate to workspace `Cargo.toml` members + `[workspace.dependencies]`
6. Add dependency to `klyntbot/Cargo.toml` if needed in facade

## 4. New Channel Checklist
1. Create `crates/channels/src/adapters/{name}/mod.rs` (+ sub-files)
2. Implement `Channel` trait: `start`, `stop`, `send`, `send_typing`, `send_structured`
3. Add config in `crates/config/src/schema/channels.rs`
4. Register in `ChannelManager::from_config()` in `channels/src/manager.rs`
5. Remove from `stubs/` if it was a stub

## 5. New LLM Provider Checklist
1. Implement `LlmProvider` trait in `providers/src/adapters/{name}/`
2. Register detection logic in `providers/src/registry/detection.rs`
3. Add config struct in `config/src/schema/providers.rs`
4. Add pricing table to `agent/src/infrastructure/output/cost_tracker/pricing.rs`

## 6. Handler Trait Pattern (Dependency Inversion)
- Define trait in the lower-layer crate that NEEDS the behavior
- Implement in `agent` crate (or wherever has access to all dependencies)
- Inject as `Arc<dyn Trait>` in constructors — never import `agent` from lower layers
- Handler impls go in `agent/src/handlers/{domain}/`

## 7. Storage & Testing
- ALL tests use `StoragePool::connect_in_memory()` — never a file path
- `StoragePool::from_existing()` skips migrations — only use for already-migrated pools
- Write repo tests in `storage/src/repos/{domain}/{name}/` adjacent to the repo file
- No external dependencies needed in CI (SQLite is embedded)

## 8. Timestamps
- Always: `chrono::Utc::now()` — all timestamps are UTC
- Frontend: parse via `new Date(iso)`, use `toLocaleTimeString()`, never `.slice()` ISO strings
- Shared helper: `formatTime()` in `desktop-ui/src/shared/lib/dates.ts`

## 9. Config Changes
- Require app restart — no hot-reload
- Access secrets via `config.providers.anthropic.api_key.expose()` — never log or serialize
- Env overrides format: `KLYNTBOT_{SECTION}__{KEY}=value` (double underscore for nesting)

## 10. Commit Convention
- `feat(scope): description` — new behavior
- `refactor(scope): description` — structural change, no behavior change
- `fix(scope): description` — bug fix
- `test(scope): description` — test only
- Zero clippy warnings required before merging
```
