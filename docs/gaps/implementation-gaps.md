# Implementation Gap Report

> Compiled from 6 subsystem deep-dive analyses on 2026-02-19.
> Source docs: `docs/subsystems/01-06`, `docs/architecture/00-overview.md`

---

## Executive Summary

Klyntbot is a 105K-line Rust AI agent framework with 17 crates, 15+ tools, and 6 chat platform integrations. The codebase is architecturally sound with clean dependency layering and no circular dependencies. However, the ongoing JSONL-to-PostgreSQL migration is approximately **80% complete**, leaving several subsystems in a fragile dual-mode state. The gap report identifies **52 gaps** across 6 severity tiers.

**Top 3 systemic issues:**
1. **JSONL/SQL dual-mode persistence** — 6+ subsystems maintain parallel storage backends, doubling maintenance burden and creating behavior divergence
2. **Zero SQL integration test coverage** — all unit tests use the file backend; the production SQL path is untested
3. **AgentLoop god object** — 2,357-line file with ~30 fields, ~450-line constructor, and two parallel processing paths

---

## Gap Index

| ID | Severity | Subsystem | Title |
|----|----------|-----------|-------|
| **P0 — Critical (blocks correctness)** |
| G-01 | P0 | Agent | SQL TodoPatch missing fields (`calendar_event_uid`, `next_instance_date`, `last_reminded_at`) |
| G-02 | P0 | Provider | Anthropic native provider lacks streaming implementation |
| G-03 | P0 | Tools | Plan execution passes `{}` as tool arguments |
| **P1 — High (significant functionality gap)** |
| G-04 | P1 | Cross-cutting | All SQL backend paths lack integration test coverage |
| G-05 | P1 | Cross-cutting | No migration utility from JSONL files to SQL |
| G-06 | P1 | Cross-cutting | Legacy JSONL stores still present (~3,000 lines of dead code) |
| G-07 | P1 | Provider | Context assembly uses char/4 estimation, not provider token counting |
| G-08 | P1 | Provider | Memory retrieval not wired in ContextEngine (3 budget priorities unused) |
| G-09 | P1 | Dashboard | Dashboard not wired into `serve.rs` |
| G-10 | P1 | Calendar | Server-wins is the only conflict resolution strategy |
| G-11 | P1 | Scheduling | CronSchedule timezone field parsed but silently ignored |
| G-12 | P1 | Config | 7 legacy `*_store_path()` methods still exposed |
| **P2 — Medium (quality/maintainability)** |
| G-13 | P2 | Agent | AgentLoop is 2,357 lines — god object pattern |
| G-14 | P2 | Agent | Legacy vs Pipeline (v2) dual processing path |
| G-15 | P2 | Agent | Two plan executors maintained simultaneously |
| G-16 | P2 | Agent | File-based memory (daily notes + MEMORY.md) not in PostgreSQL |
| G-17 | P2 | Agent | File-based learning stores (outcomes, thresholds, decision log) |
| G-18 | P2 | Agent | ToolConfidenceMap exists but not wired into ConfidenceEvaluator |
| G-19 | P2 | Agent | StrategyTracker computes stats but doesn't feed back into classification |
| G-20 | P2 | Provider | Session stores string roles, losing tool_calls/multipart/reasoning |
| G-21 | P2 | Provider | ProviderManager streaming has no retry logic |
| G-22 | P2 | Provider | No per-model context window mapping (all return default 128K) |
| G-23 | P2 | Provider | SQL session `list()` always returns `message_count: 0` |
| G-24 | P2 | Provider | No structured output support (`response_format` parameter) |
| G-25 | P2 | Provider | `create_provider()` doesn't create ProviderManager with failover |
| G-26 | P2 | Dashboard | Dashboard uses legacy JSONL stores instead of SQL repos |
| G-27 | P2 | Dashboard | `system_status` query returns hardcoded stub values |
| G-28 | P2 | Dashboard | `do_config_patch` mutation validates but doesn't apply |
| G-29 | P2 | Dashboard | `search_todos` semantic/hybrid modes return empty stubs |
| G-30 | P2 | Calendar | Sync state uses parallel file/SQL backends (not unified) |
| G-31 | P2 | Calendar | No CalDAV event caching (every call hits remote) |
| G-32 | P2 | Scheduling | CronStore uses flat JSON (full rewrite) while Goal/Plan use JSONL journal |
| G-33 | P2 | Scheduling | 100ms polling interval is inefficient for long-wait jobs |
| G-34 | P2 | Goal | No state transition validation (unlike PlanStatus) |
| G-35 | P2 | Goal | GoalStore doesn't create backup before compaction |
| G-36 | P2 | Plan | SQL upsert uses try-create/catch-duplicate (not idempotent) |
| G-37 | P2 | Storage | StorageError→KlyntbotError conversion loses structured variant info |
| G-38 | P2 | Storage | Dynamic query builder uses manual parameter index tracking |
| G-39 | P2 | Tools | MemoryTool `search_all` loads all todos in-memory for keyword search |
| G-40 | P2 | Tools | Conversation embedding store complexity (TokioRwLock + AsyncOnceCell) |
| **P3 — Low (minor improvements)** |
| G-41 | P3 | Common | `MessageRole::from("unknown")` silently falls back to User |
| G-42 | P3 | Bus | `InboundMessage::validate()` not called by `publish_inbound()` |
| G-43 | P3 | Provider | Hardcoded Groq endpoint in TranscriptionProvider |
| G-44 | P3 | Provider | HistoryCompressor purely extractive (first 100 chars only) |
| G-45 | P3 | Provider | No health check endpoint for providers |
| G-46 | P3 | Provider | Session compaction only on JSONL path (SQL grows unbounded) |
| G-47 | P3 | Provider | Model parameter overrides exist but never applied at request time |
| G-48 | P3 | Provider | Anthropic API version hardcoded (`2023-06-01`) |
| G-49 | P3 | Tools | RRULE V1 limitations (no COUNT, UNTIL, EXDATE) |
| G-50 | P3 | Tools | No tool authorization/permission model |
| G-51 | P3 | Channels | No tests for Discord, Slack, WhatsApp, QQ, Email channels |
| G-52 | P3 | Channels | No graceful WebSocket close frames on shutdown |

---

## Detailed Gap Descriptions

### P0 — Critical

#### G-01: SQL TodoPatch Missing Fields
**Subsystem**: Agent (reminders, recurring tasks, calendar sync)
**Source**: `04-agent-core.md` §19.1 #3

`storage::TodoPatch` does not include `calendar_event_uid`, `next_instance_date`, or `last_reminded_at`. This means:
- Calendar sync cannot update the event UID link via SQL path
- Recurring task spawner cannot advance `next_instance_date` via SQL path
- Reminder engine cannot set `last_reminded_at` to prevent duplicate alerts

These operations have best-effort-only SQL support; they silently skip the update.

**Fix**: Add these 3 fields to `TodoPatch` and the corresponding `UPDATE` SQL in `TodoRepo::update()`.

#### G-02: Anthropic Native Provider Lacks Streaming
**Subsystem**: Providers
**Source**: `02-providers-context.md` §9.1 G1

`AnthropicNativeProvider::supports_streaming()` returns `true` but `chat_stream()` uses the default single-chunk fallback (calls `chat()` and wraps result). The OpenAI-compatible provider has full SSE streaming. This degrades real-time UX when using Anthropic's native API.

**Fix**: Implement SSE streaming for Anthropic's `event: message_start/content_block_delta/message_stop` format.

#### G-03: Plan Execution Passes Empty Tool Arguments
**Subsystem**: Tools / Agent
**Source**: `03-tools-system.md` §10 #1, `06-domain-features.md` §7.4

The legacy `PlanExecutor::execute_step()` passes `{}` as tool arguments for all tools. This means tools must infer parameters from context, which most cannot do. The v2 `PlanExecuteEngine` in `execution/plan_execute.rs` has richer parameter generation but it's unclear when each path is used.

**Fix**: Either wire v2 engine as the primary path or add LLM-based parameter generation to the legacy executor.

---

### P1 — High

#### G-04: No SQL Integration Test Coverage
**Subsystem**: Cross-cutting (Goal, Plan, Scheduling, Calendar, Session)
**Source**: All 6 subsystem docs

Every unit test suite uses the JSONL file backend. The SQL code paths through `GoalRepo`, `PlanRepo`, `CronRepo`, `CalendarSyncRepo`, and `SessionRepo` are completely untested. This is the production path.

**Fix**: Create a shared test fixture with `PgPool` for integration testing. Add `#[sqlx::test]` or equivalent for each repo operation.

#### G-05: No JSONL-to-SQL Migration Utility
**Subsystem**: Cross-cutting
**Source**: `06-domain-features.md` §7.6

Users with existing JSONL files from pre-v0.4.0 have no automated migration path to PostgreSQL. Data must be manually re-entered or a custom script written.

**Fix**: Add `migrate_to_sql()` methods that read JSONL journals and write to SQL repos. Could be a CLI subcommand (`klyntbot migrate`).

#### G-06: Legacy JSONL Stores Still Present (~3,000 LoC)
**Subsystem**: Tools
**Source**: `03-tools-system.md` §10 #2

`todo_store.rs` (2,300 lines), `project_store.rs` (400 lines), and `embedding_store.rs` (300 lines) are superseded by PostgreSQL repos but remain in the codebase. They add maintenance burden and confusion about which path is active.

**Fix**: After completing G-05 (migration utility), deprecate and remove these files.

#### G-07: Context Assembly Uses Character Estimation
**Subsystem**: Provider / Context Engine
**Source**: `02-providers-context.md` §9.1 G2

`ContextEngine::assemble()` uses `text.len() / 4` for token estimation regardless of provider. Anthropic has a real token counting API, but it's only used at the provider level, not in context assembly. Budget allocation can be 25%+ off.

**Fix**: Wire the active provider's `count_tokens()` into `ContextEngine`. Use char estimation as fallback only.

#### G-08: Memory Retrieval Not Wired in ContextEngine
**Subsystem**: Provider / Context Engine
**Source**: `02-providers-context.md` §9.1 G3

`ContextRequest.memory_path` is accepted but never used. `Priority::RetrievedMemory`, `Priority::BootstrapPersona`, and `Priority::Skills` are defined in the budget allocator but never allocated tokens. 3 of 8 priority levels are dead code.

**Fix**: Implement embedding-based memory retrieval via `EmbeddingRepo` and integrate into assembly pipeline.

#### G-09: Dashboard Not Wired to `serve.rs`
**Subsystem**: Dashboard / CLI
**Source**: `05-channels-cli-dashboard.md` §5.1

The dashboard crate has a complete GraphQL schema, WebSocket chat, event bus, and config watcher, but `serve.rs` does not create or start `DashboardServer`. The entire dashboard is unreachable.

**Fix**: Add `DashboardServer::start()` to the serve command alongside existing services.

#### G-10: Server-Wins Only Conflict Resolution
**Subsystem**: Calendar
**Source**: `06-domain-features.md` §7.1

The sync engine's `resolve_conflict()` always returns the server version. Local user changes to calendar events may be silently overwritten.

**Fix**: Add configurable strategies: `ServerWins`, `ClientWins`, `LastWriteWins`, `Manual`.

#### G-11: Cron Timezone Silently Ignored
**Subsystem**: Scheduling
**Source**: `06-domain-features.md` §7.2

`CronSchedule::Cron` has a `tz: Option<String>` field that is parsed and stored but ignored in `compute_next_run()`. All cron expressions evaluate in UTC regardless of configured timezone.

**Fix**: Pass the timezone to `cron::Schedule` computation.

#### G-12: Legacy Store Path Methods Still Exposed
**Subsystem**: Config
**Source**: `01-foundation-storage.md` §6.1

`Config` still exposes 7 methods pointing to JSONL flat files (`todo_store_path()`, `embedding_store_path()`, etc.) that are superseded by PostgreSQL. These may still have active callers.

**Fix**: Audit all callers. Deprecate with `#[deprecated]`, then remove.

---

### P2 — Medium

#### G-13: AgentLoop God Object
**Source**: `04-agent-core.md` §19.1 #1
2,357 lines, ~30 fields, ~450-line constructor. Hard to test, reason about, or modify safely.

**Fix**: Split into focused subcomponents (MessageProcessor, PlanRunner, ToolExecutor, SessionHandler).

#### G-14: Legacy vs Pipeline Dual Processing Path
**Source**: `04-agent-core.md` §19.1 #2
Both `run_agent_loop()` (legacy) and `pipeline.process_message()` (v2) are maintained simultaneously. Unclear which is active in production.

**Fix**: Choose one path, deprecate the other.

#### G-15: Two Plan Executors
**Source**: `04-agent-core.md` §19.2 #6
`plan_executor.rs` (legacy, single-cycle, `{}` args) and `execution/plan_execute.rs` (v2, multi-cycle, reflection) coexist.

**Fix**: Migrate to v2 engine exclusively.

#### G-16: File-Based Memory Not in PostgreSQL
**Source**: `04-agent-core.md` §19.2 #8
Daily notes (`workspace/memory/YYYY-MM-DD.md`) and `MEMORY.md` live on the filesystem, inconsistent with the "all state in PostgreSQL" goal.

#### G-17: File-Based Learning Stores
**Source**: `04-agent-core.md` §19.2 #9
OutcomeStore JSONL, AdaptiveThresholds file, DecisionLogger JSONL all persist to filesystem.

#### G-18: ToolConfidenceMap Not Wired
**Source**: `04-agent-core.md` §19.3 #13
Struct exists for per-tool confidence thresholds but not integrated into `ConfidenceEvaluator` decision logic.

#### G-19: StrategyTracker No Feedback Loop
**Source**: `04-agent-core.md` §19.3 #14
Accuracy stats computed but not used to adjust orchestrator strategy selection.

#### G-20: Session Loses Structured Message Data
**Source**: `02-providers-context.md` §9.1 G4
Sessions store `role: String` + `content: String`, losing tool calls, multipart content, and reasoning content.

#### G-21: ProviderManager Streaming No Retry
**Source**: `02-providers-context.md` §9.2 G5
Rate-limited streaming requests fail immediately. Non-streaming has 3-attempt retry with exponential backoff.

#### G-22: No Per-Model Context Window Mapping
**Source**: `02-providers-context.md` §9.2 G6
`OpenAiCompatProvider` returns default 128K for all models (GPT-4 is 8K, GPT-3.5 is 16K).

#### G-23: SQL Session List Missing Message Count
**Source**: `02-providers-context.md` §9.2 G7
`list_sessions()` returns `message_count: 0` for all sessions in SQL mode.

#### G-24: No Structured Output Support
**Source**: `02-providers-context.md` §9.2 G8
`ProviderCapabilities.structured_outputs` always `false`. No JSON mode parameter.

#### G-25: create_provider() Doesn't Create ProviderManager
**Source**: `02-providers-context.md` §9.2 G9
Factory returns single provider without failover wrapping.

#### G-26: Dashboard Uses Legacy JSONL Stores
**Source**: `05-channels-cli-dashboard.md` §5.3 #1
`DashboardContext` holds `Arc<RwLock<TodoStore>>` and `Arc<RwLock<ProjectStore>>` (legacy) instead of SQL repos.

#### G-27: Dashboard system_status Stub
**Source**: `05-channels-cli-dashboard.md` §5.1
Returns hardcoded values, not live agent data.

#### G-28: Dashboard config_patch Stub
**Source**: `05-channels-cli-dashboard.md` §5.1
Validates section name but doesn't apply the JSON patch.

#### G-29: Dashboard search_todos Semantic/Hybrid Stubs
**Source**: `05-channels-cli-dashboard.md` §5.1
Non-keyword search modes return empty vectors.

#### G-30: Calendar Sync State Dual Backend
**Source**: `06-domain-features.md` §7.1
Parallel file/SQL exports without unified `from_repo()` pattern.

#### G-31: No CalDAV Event Caching
**Source**: `06-domain-features.md` §7.1
Every `get_events()` call hits the remote server.

#### G-32: CronStore Inconsistent Persistence
**Source**: `06-domain-features.md` §7.2
Flat JSON (full rewrite) while Goal/Plan use JSONL journals.

#### G-33: Cron 100ms Polling Inefficiency
**Source**: `06-domain-features.md` §7.2
Unnecessary CPU wake-ups for long-wait jobs. Should use `sleep_until()` with exact wake time.

#### G-34: GoalStatus No Transition Validation
**Source**: `06-domain-features.md` §7.3
Any status can transition to any other, unlike PlanStatus's enforced state machine.

#### G-35: GoalStore No Compaction Backup
**Source**: `06-domain-features.md` §7.3
PlanStore creates `.jsonl.bak` before compaction; GoalStore does not.

#### G-36: Plan SQL Upsert Not Idempotent
**Source**: `06-domain-features.md` §7.4
Uses try-create/catch-duplicate instead of `INSERT ... ON CONFLICT DO UPDATE`.

#### G-37: StorageError Conversion Loses Structure
**Source**: `01-foundation-storage.md` §6.2
`Storage(String)` variant converts via `.to_string()`, losing `NotFound` vs `Conflict` distinction.

#### G-38: Dynamic Query Builder Manual Indices
**Source**: `01-foundation-storage.md` §6.3
`TodoRepo::list()` and `ProjectRepo::list()` build SQL strings manually. Risk of off-by-one errors.

#### G-39: MemoryTool search_all O(n) In-Memory
**Source**: `03-tools-system.md` §10 #6
Loads all todos via `todo_repo.list()` then filters in-memory. Should use SQL `ILIKE` or `tsvector`.

#### G-40: ConversationEmbeddingStore Complexity
**Source**: `03-tools-system.md` §10 #5
`TokioRwLock` + `AsyncOnceCell` adds complexity now that PostgreSQL is the primary backend.

---

### P3 — Low

#### G-41: MessageRole Silent Fallback
**Source**: `01-foundation-storage.md` §6.5
Unknown role strings silently become `User`. Could mask bugs.

#### G-42: Bus Validation Not Auto-Called
**Source**: `01-foundation-storage.md` §6.6
`InboundMessage::validate()` (64KB check) exists but isn't called by `publish_inbound()`.

#### G-43: Hardcoded Groq Endpoint
**Source**: `02-providers-context.md` §9.3 G11
Can't use alternative Whisper providers.

#### G-44: Extractive-Only History Compression
**Source**: `02-providers-context.md` §9.3 G12
Just first 100 chars per message. Loses nuance.

#### G-45: No Provider Health Check
**Source**: `02-providers-context.md` §9.3 G14

#### G-46: No SQL Session Compaction
**Source**: `02-providers-context.md` §9.3 G15
SQL sessions grow unbounded (JSONL path compacts at 1000 messages).

#### G-47: Model Overrides Not Applied
**Source**: `02-providers-context.md` §9.3 G16
`ProviderRegistry::get_model_overrides()` exists but never called in request path.

#### G-48: Anthropic API Version Hardcoded
**Source**: `02-providers-context.md` §9.3 G17
`ANTHROPIC_VERSION = "2023-06-01"`.

#### G-49: RRULE V1 Limitations
**Source**: `03-tools-system.md` §10 #3
No COUNT, UNTIL, EXDATE support. Can't do "every Monday for 4 weeks".

#### G-50: No Tool Authorization Model
**Source**: `03-tools-system.md` §10 #7
Any tool callable by LLM with any parameters. No per-user restrictions.

#### G-51: Channel Test Coverage Gaps
**Source**: `05-channels-cli-dashboard.md` §5.2
Zero tests for Discord, Slack, WhatsApp, QQ, Email. Only Telegram (9 tests) and WebSocketManager (7 tests).

#### G-52: No Graceful WebSocket Close
**Source**: `05-channels-cli-dashboard.md` §5.3 #6
Channels set `running = false` but don't send WebSocket close frames.

---

## Test Coverage Summary

| Area | Unit Tests | Integration Tests | SQL Tests | Assessment |
|------|-----------|-------------------|-----------|------------|
| common | 27 | 0 | N/A | Good |
| config | 106 | 0 | N/A | Excellent |
| bus | 20 | 0 | N/A | Good |
| storage (repos) | 0 | 13 suites | 13 suites | Good (SQL-only) |
| providers | 45 | 0 | N/A | Moderate (OpenAI compat untested) |
| context_engine | 14 | 0 | N/A | Moderate |
| session | 15 | 0 | 0 | Moderate (no SQL tests) |
| tools | 200+ | 0 | N/A | Good (unit level) |
| agent | 100+ | 0 | 0 | Moderate (no integration) |
| calendar | 50 | 0 | 0 | Good (no live CalDAV tests) |
| scheduling | 29 | 0 | 0 | Moderate (no SQL tests) |
| goal | 20 | 0 | 0 | Moderate (no SQL tests) |
| plan | 14 | 0 | 0 | Moderate (no SQL tests) |
| heartbeat | 1 | 0 | N/A | Weak |
| channels | 16 | 0 | N/A | Weak (only Telegram + WsManager) |
| cli | 50 | 0 | N/A | Moderate (wizard well-tested) |
| dashboard | 0 | 0 | N/A | None |
| **Total** | **~2,722** | **47 files** | **13 suites** | |

---

## Recommended Action Plan

### Phase 1: Correctness & Stability (Weeks 1-2)
1. **G-01**: Add missing fields to `storage::TodoPatch`
2. **G-03/G-15**: Choose v2 plan executor, deprecate legacy
3. **G-04**: Set up shared SQL test fixture; add tests for critical repos
4. **G-11**: Wire timezone into cron schedule computation

### Phase 2: Complete SQL Migration (Weeks 3-4)
5. **G-05**: Build `klyntbot migrate` CLI command
6. **G-06**: Remove legacy JSONL stores (todo_store, project_store, embedding_store)
7. **G-12**: Audit and remove legacy store path methods
8. **G-16/G-17**: Migrate memory store and learning stores to PostgreSQL
9. **G-30**: Unify calendar sync state to `from_repo()` pattern

### Phase 3: Provider & Context Quality (Weeks 5-6)
10. **G-02**: Implement Anthropic native streaming
11. **G-07**: Wire provider token counting into ContextEngine
12. **G-08**: Implement memory retrieval in context assembly
13. **G-20**: Extend session message format to preserve tool calls/reasoning
14. **G-22**: Add per-model context window mapping

### Phase 4: Dashboard & Integration (Weeks 7-8)
15. **G-09**: Wire DashboardServer into serve.rs
16. **G-26**: Migrate dashboard to use SQL repos
17. **G-27/G-28/G-29**: Implement dashboard stub resolvers
18. **G-13/G-14**: Refactor AgentLoop into focused subcomponents

### Phase 5: Polish & Hardening (Ongoing)
19. **G-10**: Add configurable conflict resolution strategies
20. **G-51**: Add channel integration tests (Discord, Slack priority)
21. **G-50**: Add tool authorization model
22. Remaining P3 items as capacity allows

---

## Document Map

| Document | Path | Content |
|----------|------|---------|
| Architecture Overview | `docs/architecture/00-overview.md` | High-level crate map, message journey, patterns |
| Foundation & Storage | `docs/subsystems/01-foundation-storage.md` | common, config, bus, storage (Layers 0-1.5) |
| Providers & Context | `docs/subsystems/02-providers-context.md` | providers, context_engine, session (Layer 2) |
| Tools System | `docs/subsystems/03-tools-system.md` | All 15+ tools, 11 handler traits (Layer 3) |
| Agent Core | `docs/subsystems/04-agent-core.md` | Agent loop, pipeline, learning, execution (Layer 5) |
| Channels, CLI, Dashboard | `docs/subsystems/05-channels-cli-dashboard.md` | 6 channels, CLI, dashboard (Layers 4-6) |
| Domain Features | `docs/subsystems/06-domain-features.md` | Calendar, scheduling, goal, plan, heartbeat (Layer 2) |
| **This Report** | `docs/gaps/implementation-gaps.md` | Consolidated gap report with action plan |

---

*Compiled by team-lead from 6 parallel subsystem analyses, 2026-02-19*
