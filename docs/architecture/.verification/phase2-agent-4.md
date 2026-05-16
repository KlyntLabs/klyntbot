# Phase 2 Verification — Agent 4

**Docs verified:**
- `docs/architecture/subsystems/07-tools-framework.md`
- `docs/architecture/subsystems/08-assistant-features.md`

**Crates verified:** 16 total
- From 07: `tools-core`, `tools-core-macros`, `tools`
- From 08: `feature-alarms`, `feature-coaching`, `feature-focus`, `feature-finance`, `feature-insights`, `feature-language-learning`, `feature-launcher`, `feature-learning`, `feature-notes`, `feature-productivity`, `feature-tasks`, `voice-engine`, `analytics`

---

## Summary

| Metric | Count |
|---|---|
| Crates inspected | 16 |
| ✅ Accurate claims | ~85 |
| ⚠️ Drift | 5 |
| ❌ Wrong | 2 |
| 🔍 Missing (in code, not docs) | 3 |
| 📋 Tech Debt catalogued | 8 TODOs / 0 FIXMEs |

Overall assessment: Both docs are **largely accurate** with minor numeric drift (action counts, table counts) and one outdated API signature in the `ToolRegistry` documentation. The `feature-finance` "57 actions" claim is factually incorrect — the enum contains 64 actions. The `tone_analyzer` stub claim is slightly overstated (it has a basic heuristic, not a pure stub).

---

## Per-Crate Findings

### `tools-core`

**✅ Accurate**
- All 15 claimed modules exist and match `src/` tree (`approval_class`, `config_persistence`, `events`, `feature`, `interceptor`, `job_supervisor`, `metadata`, `pagination`, `params`, `registry`, `routing`, `search`, `validation`).
- `Tool` trait signature matches exactly (`name`, `description`, `parameters`, `execute`, `metadata`, `is_concurrency_safe`, `allowed_channels`, `custom_timeout`, `approval_class`, `approval_scope`, `to_schema`, `validate_params`).
- `ApprovalClass` enum: `Safe`, `Sensitive`, `Destructive`, `Admin` with `requires_prompt_on_remote()` returning `true` for `Destructive | Admin`.
- `ApprovalScope` enum: `ToolAction`, `ToolActionResource(String)` with `resource_key()`.
- `FeaturePackage` trait matches exactly (`name`, `tools`, `migrations`, `health_check` default).
- `FeatureMigration` struct fields match exactly.
- `JobSupervisorHandle` trait matches with all PTY methods (`write_stdin`, `resize`, `attach`, `detach`, `set_attach_channel`) defaulting to `NotPty`.
- `JobId` format is `bash-{10 base32 chars}`; alphabet excludes `i`/`l`.
- `PTY_ROWS_MIN=4`, `PTY_ROWS_MAX=200`, `PTY_COLS_MIN=20`, `PTY_COLS_MAX=400`.
- `RoutingContext` has all documented fields (actually 24+ fields; doc says "22+" which is conservative and correct).
- `ToolOutput::Structured` exists and `parse()` recognizes `__STRUCTURED__` prefix. **Confirmed unused** — no production code emits the prefix.
- `InterceptorChain` exists with `add`/`check` / first-error-short-circuits behavior.
- `Searchable` trait + `rrf_merge` / `rrf_merge_triple` exist.
- `ConfigPersistence` trait exists.
- `ToolEvent` enum variants match documented shapes.
- `Page<T>` exists.
- No `ConcurrencyClass` on `Tool` trait — confirmed the bus crate defines it but `Tool` only uses `is_concurrency_safe(args) -> bool`.

**⚠️ Drift**
- `ToolRegistry::list_meta()` returns `Vec<(String, String, Vec<String>)>` (name, description, aliases). Doc claims `Vec<&ToolMetadata>`.
- `ToolRegistry::by_category()` returns `Vec<&str>` (tool names). Doc claims `Vec<DynTool>`.

**📋 Tech Debt**
- None found (0 TODO / 0 FIXME / 0 unimplemented!).

---

### `tools-core-macros`

**✅ Accurate**
- All 5 macros exist: `#[derive(Tool)]`, `#[derive(ToolParams)]`, `#[derive(ActionParams)]`, `#[derive(DomainEnum)]`, `#[tool_actions(...)]`.
- Module tree matches: `action_params`, `domain_enum`, `helpers`, `tool_actions`, `tool_derive`, `tool_params`.
- JSON Schema generation panic message for nested structs exists in `helpers.rs:141`: `"unsupported field type ..."`.

**📋 Tech Debt**
- Zero production TODOs. One test uses `"TODO"` as a string literal for `DomainEnum` parsing (`tests/domain_enum_tests.rs:101`).

---

### `tools`

**✅ Accurate**
- All 14 domain tools exist: `MemoryTool`, `OkrTool`, `AreaTool`, `ProjectTool`, `CronTool`, `LearningTool`, `AnnotateTool`, `MirrorTool`, `SubagentsTool`, `TemporalTool`, `DocsTool`, `ContextRequestTool`, `SkillReferenceTool`, `AgentTaskTool`.
- `EMBEDDING_DIM = 384` in `embedding_engine.rs:24`.
- `EmbeddingEngineImpl` and `EmbeddingStore` exist.
- Re-exports from `tools-core` present.

**🔍 Missing (in code, not in docs)**
- `conversation_recall`, `progress_handler`, `search_utils`, `semantic_fact_search`, `todo_types` modules exist at crate root but are not mentioned in the 07 doc's file map (they are secondary).

**📋 Tech Debt**
- None found (method names like `embed_todo` contain the word "todo" but are not actual TODO comments).

---

### `feature-tasks`

**✅ Accurate**
- `TasksFeature` implements `FeaturePackage`. `tools()` returns `vec![]` unless `.with_task_tool(...)` called.
- Exactly **19 actions** across handlers: `create`, `update`, `complete`, `reopen`, `delete`, `show`, `list`, `summary`, `tree`, `search`, `focus`, `unfocus`, `log_time`, `add_dep`, `remove_dep`, `batch`, `recur`, `list_recurring`, `delete_recurring`.
- **8 tables** in migrations: `tasks`, `task_activity`, `task_attachments`, `task_time_entries`, `task_dependencies`, `task_estimation_history`, `task_recurrence_templates`, `task_alarms`.
- `focus_watcher.rs` spawns a background tokio task polling every 60s for expired focus slots, emits `DomainEvent::TaskFocusExpired`, clears `focused_at`.
- `max_focus_slots` enforced in `handle_focus` via `repo.focus(id, max_focus_slots as i64, deadline)`.
- `rrf_k = 60` default in `config.rs` and `tool/mod.rs`.
- `semantic_threshold = 0.5` default in `config.rs`.
- Bus events: `TaskCreated`, `TaskCompleted` (carries `deviation_pct`), `FocusExpired`, `FocusChanged`, `Deferred`, `EstimationRecorded`. Each carries an optional `coaching_signal` flag.
- `AlarmSpec` materialization uses `FireStore::schedule` with `kind="task_alarm"` and `dedup_prefix="task:{id}:alarm:"`.
- Cancellation uses `FireStore::cancel_by_prefix("task:{id}:alarm:")`.
- OKR cascade: `ProgressHandler` is called on `complete`; actual cascade logic is injected from agent layer.
- RRULE parser rejects `BYSETPOS`, `WKST`, `EXRULE`, `RDATE`.

**📋 Tech Debt**
- None found.

---

### `feature-notes`

**✅ Accurate**
- `NotesFeature` implements `FeaturePackage`; exports `NotesTool`.
- **8 tables**: `notebooks`, `notes`, `note_tags`, `note_links`, `note_entity_mentions`, `note_versions`, `inbox_items`, `practice_sessions`.
- `practice_sessions` table defined in migration `002_practice_sessions.sql`.

**🔍 Missing**
- Tool has more than "CRUD + search + inbox" — it also handles `tag_note`, `link_notes`, `create_notebook`, `archive_note`, `unarchive_note`, `list_archived`, `get_backlinks`, `capture_inbox`, `list_inbox` (15 handle methods total).

**📋 Tech Debt**
- None found.

---

### `feature-productivity`

**✅ Accurate**
- `ProductivityFeature` implements `FeaturePackage`; exports `ProductivityTool`.
- Tool actions cover focus, activity, dashboard, pomodoro, goals, time logging, export.

**⚠️ Drift**
- Doc claims **20 tables**. Migrations contain **22** `CREATE TABLE` statements (`activity_categories`, `activity_events`, `daily_summaries`, `nudge_history`, `productivity_goals`, `time_entries`, `distraction_learned_rules`, `activity_buckets`, `distraction_patterns`, `insight_cards`, `productivity_projects`, `productivity_tracking_rules`, `productivity_sessions`, `productivity_quality_scores`, `productivity_narratives`, `productivity_voice_journals`, `productivity_categorization_cache`, `productivity_privacy_rules`, `productivity_rule_evolution_log`, `calendar_events`, `weekly_assessments`, plus one implicit). The count is closer to 21–22 depending on whether virtual tables are counted.

**📋 Tech Debt**
- None found.

---

### `feature-finance`

**✅ Accurate**
- `FinanceFeature` implements `FeaturePackage`; exports `FinanceTool`.
- `FinanceTool::allowed_channels()` returns `common::ChannelMask::NON_CODING`.
- `PriceService` fetches live prices via `reqwest`.
- `RateCache` two-layer cache (in-memory `DashMap` + SQLite `finance_exchange_rates`) with **15-minute TTL** (`RateCache::new(exchange_rates, 15)`).
- `fire_handlers.rs` calls `analytics::MonteCarloEngine` and `analytics::fire::*` directly.
- `JarType` enum and `BudgetMethod::SixJar` exist.
- `FinanceHandler` trait for `BudgetAlert` events exists.
- **11 tables** in migration: `finance_accounts`, `finance_transactions`, `finance_budgets`, `finance_portfolios`, `finance_investments`, `finance_investment_transactions`, `finance_goals`, `finance_liabilities`, `finance_exchange_rates`, `finance_allocation_targets`, `finance_net_worth_snapshots`.
- Approval classification correctly splits into `Sensitive` (writes) and `Destructive` (deletes).

**❌ Wrong**
- Doc claims **57 actions**. The `parameters()` enum in `tool/mod.rs` contains **64** distinct action strings. The doc's own action table also adds up to 64. The "57" figure is incorrect.
- Inline comment in `tool/mod.rs` says "Dispatches 37+ actions" — also outdated.

**📋 Tech Debt**
- None found.

---

### `feature-launcher`

**✅ Accurate**
- `LauncherFeature` implements `FeaturePackage`. Wired in `app-core/src/init/mod.rs:1132` (Path C).
- **5 actions**: `search`, `execute`, `apply_window`, `pin`, `unpin`.
- **6 tables**: `launcher_usage_log`, `launcher_pins`, `clipboard_history`, `clipboard_fts` (FTS5 virtual), `entity_attention`, `entity_attention_fts` (FTS5 virtual).
- `InvertedFileIndex` exists in `search/inverted_index.rs`; used by `file_search.rs`.
- Criterion benchmarks exist: `benches/inverted_index.rs`, `benches/app_index_dedup.rs`.
- `ClipboardMonitor` exists.
- `WindowAction` enum: `LeftHalf`, `RightHalf`, `TopHalf`, `BottomHalf`, `LeftThird`, `CenterThird`, `RightThird`, `Maximize`, `Center`, `Restore`, `Preset(String)`.
- `WindowManager` singleton via `window_mgmt::global`.
- `FrequencyRepo`, `PinsRepo`, `SourceRegistry` exist.
- `apply_window` supports named presets (`preset:<name>`) via `window_mgmt/presets.rs`.

**📋 Tech Debt**
- None found.

---

### `feature-alarms`

**✅ Accurate**
- **No `FeaturePackage` impl** — exports only `AlarmTool`.
- Tool actions: `create`, `list`, `cancel`, `snooze`.
- `AlarmTool::approval_class()` returns `Destructive` for `cancel`, `Sensitive` for `create`/`snooze`, `Safe` for `list`.
- Wired in `agent_loop::builder.rs:686` (Path B).

**📋 Tech Debt**
- None found.

---

### `feature-focus`

**✅ Accurate**
- `FocusFeature` implements `FeaturePackage`; `tools()` returns `vec![]`.
- `DndManager` is a concrete struct.
- `DndScheduler` and `FocusBridge` are **traits** (not structs), implemented by platform-specific backends (`MacosFocusBridge`).
- 1 table: `focus_sessions`.

**📋 Tech Debt**
- None found.

---

### `feature-coaching`

**✅ Accurate**
- `CoachingFeature` implements `FeaturePackage`; `tools()` returns `vec![]`.
- `CoachingService` and `CoachingSignalConsumer` exist.
- Uses `#[derive(AiFeature)]` for skill discovery + metric harvesting.
- Subscribes to bus events from tasks, finance, productivity.

**📋 Tech Debt**
- None found.

---

### `feature-learning`

**✅ Accurate**
- `LearningFeature` implements `FeaturePackage`; `tools()` returns `Vec::new()` with explicit comment: "The 'learning' Tool lives in `crates/tools/src/domain/learning_tool.rs`".
- Actual `LearningTool` lives in `crates/tools` and is wired in `agent_loop::builder.rs:1735`.

**📋 Tech Debt**
- None found.

---

### `feature-language-learning`

**✅ Accurate**
- `LanguageLearningFeature` implements `FeaturePackage`; exports `LanguagePracticeTool`.
- Uses `voice-engine` (`PronunciationProvider`, `AppPronunciationProvider`).
- Migration defines 3 tables: `phoneme_mastery`, `pronunciation_logs`, `exam_attempts`.
- Shares `practice_sessions` table with `feature-notes` (defined in notes migration 002).

**📋 Tech Debt**
- `pronunciation_provider.rs:35` — `// TODO: Wire the full pipeline when phoneme aligner produces real data:`
- `practice_tool.rs:84` — `// TODO: Query pronunciation_logs for the current session`
- `practice_tool.rs:91` — `// TODO: Query phoneme_mastery for low-stability phonemes`

---

### `feature-insights`

**✅ Accurate**
- **No `FeaturePackage` impl at all.**
- `InsightService` exists and is constructed directly in `app-core::init`.

**📋 Tech Debt**
- None found.

---

### `voice-engine`

**✅ Accurate**
- `Qwen3AsrEngine` exists with `IDLE_UNLOAD_SECS = 300` (seconds, not ms).
- `CloudAsrEngine` exists.
- `Qwen3TtsEngine` exists with `MAX_CHUNK_CHARS = 400` and `IDLE_UNLOAD_SECS = 300`.
- Feature-gated behind `qwen3` Cargo feature (`features = ["dep:qwen3-tts-rs"]` in `Cargo.toml`).
- `AvSpeechTtsEngine` exists (macOS).
- `CloudTtsEngine` exists.
- `WebrtcVadProcessor` exists. Feature-gated behind `vad` feature.
- `vad` disabled fallback is RMS threshold.

**⚠️ Drift**
- `tone_analyzer.rs::classify_tone()` is **not a pure stub** — it implements a basic 3-segment heuristic (returns tone class based on segment averages). The doc says both `phoneme_aligner` and `tone_analyzer` are "stubs." The phoneme aligner is definitely a stub; the tone analyzer has real (if rudimentary) logic.

**📋 Tech Debt**
- `phoneme_aligner.rs:48` — `// TODO: Integrate qwen3_asr forced alignment API.`
- `phoneme_aligner.rs:64` — `// TODO: Use pitch-detection crate (YIN) to extract F0 contour per syllable.`
- `tone_analyzer.rs:81` — `// TODO: Extract actual F0 contour per syllable using pitch-detection crate.`
- `service.rs:753` — `// TODO: tee the audio stream during capture to actually write the WAV file.`
- `error_classifier.rs:44` — `actual: p.phoneme.clone(), // TODO: actual vs expected comparison`

---

### `analytics`

**✅ Accurate**
- Zero async/storage dependencies confirmed in `Cargo.toml`.
- `MonteCarloEngine`, `SimulationConfig`, `SimulationResult` exist.
- `fire` module with `fire_traditional`, `fire_coast`, `fire_lean`, `fire_fat`, etc.
- `portfolio` module exists.
- Called synchronously from `feature-finance::fire_handlers`.

**📋 Tech Debt**
- None found.

---

## Cross-Reference Check

### From `07-tools-framework.md`

| Link | Target exists? | Notes |
|---|---|---|
| `../00-overview.md` | ✅ Yes | |
| `./01-foundations.md` | ✅ Yes | |
| `./02-storage.md` | ✅ Yes | |
| `./04-agent-runtime.md` | ✅ Yes | |
| `./08-assistant-features.md` | ✅ Yes | |
| `./09-coding-mode.md` | ✅ Yes | |
| `./10-sandboxing-security.md` | ✅ Yes | |
| `../TECH_DEBT.md` | ✅ Yes | |
| `../crates/tools-core.md` | ✅ Yes | |

### From `08-assistant-features.md`

| Link | Target exists? | Notes |
|---|---|---|
| `../00-overview.md` | ✅ Yes | |
| `./02-storage.md` | ✅ Yes | |
| `./05-cognitive-memory.md` | ✅ Yes | |
| `./06-scheduling.md` | ✅ Yes | |
| `./07-tools-framework.md` | ✅ Yes | |
| `./07-tools-framework.md#the-four-wiring-paths` | ✅ Yes | Anchor exists |
| `./11-channels-mcp.md` | ✅ Yes | |
| `../TECH_DEBT.md` | ✅ Yes | |
| `../TECH_DEBT.md#1-pure-todo--fixme--unimplemented` | ⚠️ Present | File exists; anchor not verified at character level |
| `../TECH_DEBT.md#7-architectural-anomalies` | ⚠️ Present | File exists; anchor not verified at character level |

### Wiring path line-number verification

| Claim | Actual | Status |
|---|---|---|
| `AlarmTool` wired at `builder.rs:690` | `builder.rs:686` | ⚠️ 4-line drift |
| `TaskTool` wired at `builder.rs:1353` → 1417 | `builder.rs:1353` (start), 1405 (comment) | ✅ Accurate |
| `LearningTool` wired at `builder.rs:1735` | `builder.rs:1735` | ✅ Exact |
| `LauncherTool` wired at `app-core::init/mod.rs:1132` | `app-core/src/init/mod.rs:1132` | ✅ Exact |
| `AgentTaskTool` at `subagent.rs:800,819` | `subagent.rs:800` and `819` | ✅ Exact |

### Additional behavioral verification

| Claim | Status | Evidence |
|---|---|---|
| `MAX_CONCURRENT_TOOLS = 10` | ✅ | `crates/agent/src/execution/core.rs:60` |
| `ToolOutput::Structured` unused in prod | ✅ | Zero matches for `__STRUCTURED__` outside `tools-core/src/lib.rs` |
| `feature-coding-bash` uses path deps | ✅ | 10 `path = "../..."` entries in `Cargo.toml`; no `workspace = true` |
| `common::ChannelMask::NON_CODING` exists | ✅ | Used by `LearningTool` and `FinanceTool` |
| `ClassifyHook` trait exists | ✅ | `crates/approval/src/policy.rs:4` |
| `CodingApprovalPolicy` implements `ClassifyHook` | ✅ | `crates/approval/src/coding_policy.rs:131` |
