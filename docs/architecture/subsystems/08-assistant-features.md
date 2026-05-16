# Subsystem 08 — Assistant-Mode Features

> **Status:** 🟢 Stable *(mostly)* — `feature-coaching`/`feature-focus`/`feature-insights` register no tools; voice pronunciation pipeline is 🔴 stubbed
> **Status last verified:** 2026-05-16
> **Crates:** `feature-tasks`, `feature-notes`, `feature-productivity`, `feature-finance`, `feature-focus`, `feature-coaching`, `feature-learning`, `feature-language-learning`, `feature-insights`, `feature-alarms`, `feature-launcher`, `voice-engine`, `analytics` *(13 crates)*
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

The 13 crates that comprise assistant-mode user-facing functionality. They span four shapes: **tool-bearing features** (tasks, notes, finance, productivity, language-learning, alarms, launcher — register LLM-visible tools), **service-only features** (coaching, focus, insights — pipeline/bridge code with no LLM surface), **shells** (learning — the tool actually lives in `crates/tools/`), and **support libraries** (voice-engine, analytics — provide capability to other features). The naming convention `feature-*` implies a uniform shape that the code doesn't deliver — see [Open questions](#open-questions--debt).

The two architectural standouts are **`feature-finance`** (64 actions in 12 sub-modules — the largest multi-action tool in the workspace) and **`feature-launcher`** (a substantial standalone system: inverted file index, clipboard FTS, attention-decay aggregator, window manager, calendar source — wired in `app-core`, not the agent builder).

---

## Architecture diagram

```mermaid
flowchart LR
    classDef tooled fill:#e0f2f1,stroke:#00796b,color:#004d40
    classDef service fill:#fff8e1,stroke:#f9a825,color:#f57f17
    classDef shell fill:#fce4ec,stroke:#c2185b,color:#880e4f
    classDef lib fill:#e3f2fd,stroke:#1976d2,color:#0d47a1

    T[feature-tasks<br/><i>19 actions · RRULE · alarms · OKR<br/>focus slots · LanceDB hybrid search</i>]:::tooled
    N[feature-notes<br/><i>notebooks · YAML front matter · FTS<br/>entity mentions · practice sessions</i>]:::tooled
    P[feature-productivity<br/><i>20 tables · activity FSM<br/>auto-focus · nudges</i>]:::tooled
    F[feature-finance<br/><i>64 actions · 11 tables<br/>price service · FIRE · Six-Jar</i>]:::tooled
    A[feature-alarms<br/><i>standalone reminders<br/>via scheduled_fires</i>]:::tooled
    LP[feature-language-learning<br/><i>pronunciation · practice<br/>exam tracking</i>]:::tooled
    LCH[feature-launcher<br/><i>inverted index · clipboard<br/>attention · windows · calendar</i>]:::tooled

    C[feature-coaching<br/><i>signal consumer<br/>NO tools</i>]:::service
    FC[feature-focus<br/><i>DND bridge<br/>NO tools</i>]:::service
    I[feature-insights<br/><i>nightly insight review<br/>NO tools · NO FeaturePackage</i>]:::service

    L[feature-learning<br/><i>shell · tools() returns vec![]<br/>tool in crates/tools</i>]:::shell

    V[voice-engine<br/><i>STT · TTS · VAD<br/>pronunciation pipeline (stub)</i>]:::lib
    AN[analytics<br/><i>zero-dep pure compute<br/>Monte Carlo · FIRE · portfolio</i>]:::lib

    T <-.signals.-> C
    F <-.signals.-> C
    P <-.signals.-> C
    P <-.auto_focus.-> FC
    T <-.uses.-> FC
    LP <-.uses.-> V
    F <-.uses.-> AN
    LP <-.shared table:practice_sessions.-> N
    I -.reads.-> N
```

---

## Mental model

Four shapes of "feature crate":

| Shape | Crates | Pattern |
|---|---|---|
| **Tool-bearing** | tasks, notes, productivity, finance, language-learning, alarms, launcher | Has a `FeaturePackage` (or is wired directly) + registers ≥1 tool |
| **Service-only** | coaching, focus, insights | Background services or bridges; tools `vec![]` or no `FeaturePackage` |
| **Shell** | learning | Has `FeaturePackage` but `tools()` returns `vec![]`; actual tool in `crates/tools/` |
| **Support library** | voice-engine, analytics | Provides capabilities consumed by other features |

The naming convention `feature-*` implies "user-visible feature with tools." Several crates violate that — and there's no convention or trait that distinguishes them. The doc here normalizes by shape.

---

## Reference: per-crate tool inventory

| Crate | Tool | Actions | ApprovalClass | Channels | Tables |
|---|---|--:|---|---|--:|
| `feature-tasks` | `tasks` | 19 | Destructive (deletes), Sensitive (writes/focus/recur), Safe (reads) | NON_CODING | 8 |
| `feature-notes` | `notes` | CRUD + search + inbox | (default) | (default) | 8 |
| `feature-productivity` | `productivity` | focus + activity + dashboard | Sensitive (writes), Safe (reads) | (default) | 20 |
| `feature-finance` | `finance` | **64** | Destructive (deletes), Sensitive (writes), Safe (reads) | NON_CODING | 11 |
| `feature-language-learning` | `language_practice` | pronunciation + practice + exam | (default) | (default) | 3 |
| `feature-launcher` | `launcher` | search/execute/apply_window/pin/unpin | (sensitive on execute/window) | (default) | 6 |
| `feature-alarms` | `alarm` | create/cancel/snooze/list | Destructive (cancel), Sensitive (create/snooze) | (default) | 0 (uses `scheduled_fires`) |
| `feature-coaching` | — | — | — | — | 0 |
| `feature-focus` | — | — | — | — | 1 (`focus_sessions`) |
| `feature-insights` | — | — | — | — | 0 |
| `feature-learning` | — *(in `tools` crate)* | — | — | NON_CODING | 0 (tables in `cognitive`) |
| `voice-engine` | — | — | — | — | 0 |
| `analytics` | — | — | — | — | 0 |

### `feature-tasks` — 19 actions

`create`, `update`, `complete`, `reopen`, `delete`, `show`, `list`, `summary`, `tree`, `search`, `focus`, `unfocus`, `log_time`, `add_dep`, `remove_dep`, `batch`, `recur`, `list_recurring`, `delete_recurring`.

**Tables:** `tasks`, `task_activity`, `task_attachments`, `task_time_entries`, `task_dependencies`, `task_estimation_history`, `task_recurrence_templates`, `task_alarms`.

### `feature-finance` — 64 actions across 12 sub-modules

| Group | Actions |
|---|---|
| Accounts | `account_add`, `account_list`, `account_update`, `account_delete` |
| Transactions | `tx_add`, `tx_list`, `tx_update`, `tx_delete`, `tx_search`, `tx_recurring_add` |
| Budgets | `budget_create`, `budget_list`, `budget_status`, `budget_update`, `budget_delete` |
| Investments | `portfolio_create/list/delete`, `investment_add/update/delete/tx/summary`, `price_fetch/refresh`, `portfolio_drift/rebalance/returns/correlation` |
| Goals/Liabilities/Net Worth | `goal_create/list/update/delete/fire/whatif`, `liability_add/list/update/delete`, `net_worth` |
| Reports | `report_spending`, `report_income`, `report_trends`, `report_net_worth_history`, `daily_review` |
| Analytics | `analyze_spending_anomalies`, `analyze_spending_trends`, `analyze_recurring_charges`, `analyze_category_correlation` |
| FIRE | `fire_traditional`, `fire_coast`, `fire_lean`, `fire_fat`, `fire_withdrawal_sim`, `fire_backtest`, `fire_sensitivity` |
| Allocations | `allocation_target_set/list/delete` |
| Snapshots | `snapshot_record`, `snapshot_history` |
| Settings | `settings_get`, `settings_update` |
| Health | `finance_health_check` |

**Tables:** `finance_accounts`, `finance_transactions`, `finance_budgets`, `finance_portfolios`, `finance_investments`, `finance_investment_transactions`, `finance_goals`, `finance_liabilities`, `finance_exchange_rates`, `finance_allocation_targets`, `finance_net_worth_snapshots`.

### `feature-launcher` — 5 actions, 6 tables

Actions: `search`, `execute`, `apply_window`, `pin`, `unpin`.

**Tables:** `launcher_usage_log`, `launcher_pins`, `clipboard_history`, `clipboard_fts` *(FTS5 virtual)*, `entity_attention`, `entity_attention_fts` *(FTS5 virtual)*.

### Non-tool-bearing crates explained

| Crate | Why no tools |
|---|---|
| `feature-coaching` | Pipeline-only. `FeaturePackage::tools()` returns `vec![]`. `CoachingService` + `CoachingSignalConsumer` run as background async tasks. Uses `#[derive(AiFeature)]` purely for skill discovery + metric harvesting. |
| `feature-focus` | DND bridge. `tools()` returns `vec![]`. `DndManager`/`DndScheduler`/`FocusBridge` are programmatic APIs consumed by `feature-tasks` (alarms) and `feature-productivity` (auto-focus). |
| `feature-insights` | **No `FeaturePackage` impl at all.** `InsightService` constructed directly in `app-core::init`. Pure backend service. |
| `feature-learning` | `FeaturePackage::tools()` explicitly returns `vec![]` with comment: "the 'learning' Tool lives in `crates/tools/src/domain/learning_tool.rs`." Wired in `agent_loop::builder.rs:1735`. |
| `feature-alarms` | **No `FeaturePackage` impl.** `AlarmTool` exported and wired in `agent_loop::builder.rs:690`. No tables — uses `scheduled_fires` with `kind="standalone_alarm"`, `dedup_prefix="standalone:{id}:"`. |
| `voice-engine` | Support library. Provides STT/TTS/VAD/pronunciation capabilities to `feature-language-learning` and to the dictation feature in `app-core`. |
| `analytics` | Zero-dep pure-compute library (no async, no storage). Called synchronously from `feature-finance::fire_handlers`. |

---

## Spotlight: `feature-tasks`

**RRULE handling.** Custom parser in `rrule_utils.rs`. Supports `FREQ`, `INTERVAL`, `BYDAY`, `BYHOUR`, `BYMINUTE`, `BYMONTHDAY`, `COUNT`, `UNTIL`, `EXDATE`. **Explicitly rejects** `BYSETPOS`, `WKST`, `EXRULE`, `RDATE` — a hidden contract worth knowing before writing complex RRULEs that silently produce empty results.

**Alarm wiring.** `AlarmSpec` has 3 variants (`RelativeBefore`, `CivilTime`, `Absolute`), mirroring `scheduling::AlarmRule`. `materialize_for_task` calls `scheduling::AlarmRule::compute_fire_at`, persists `TaskAlarmRow` to `task_alarms`, then `FireStore::schedule(FireSpec)` writes to `scheduled_fires`. Cancellation: `FireStore::cancel_by_prefix("task:{id}:alarm:")`.

**OKR cascade.** Tasks FK to `key_result_id` and `objective_id`. On `complete`, optional `ProgressHandler` (re-exported from `tools_core`) is called. The cascade logic itself is injected — `feature-tasks` provides the hook, doesn't implement the cascade.

**Semantic search.** `handle_search` runs keyword query against `TaskRepo`, then vector cosine search against LanceDB (if `embedding_store` wired). Results merged with hybrid `rrf_merge` (Reciprocal Rank Fusion, `rrf_k = 60` default). Semantic threshold default 0.5.

**Focus slot mechanism.** `tasks.focused_at` + `tasks.focus_deadline` columns. `max_focus_slots` enforced in `handle_focus`. `focus_watcher.rs` runs a background loop polling for expired focus slots and clearing them; emits `FocusChanged` bus event with deadline.

**Bus events emitted.** `TaskCreated`, `TaskCompleted` (with `deviation_pct`), `FocusExpired`, `FocusChanged`, `Deferred`, `EstimationRecorded`. Each can carry a `coaching_signal` flag consumed by `feature-coaching`.

**The `with_task_tool` footgun.** `TasksFeature::new()` followed by `.tools()` returns `vec![]`. You must call `.with_task_tool(tool)` first, or no tools are registered. Same pattern in `FinanceFeature`.

---

## Spotlight: `feature-finance`

**The 64-action surface** is the largest multi-action tool in the workspace. Organized by 12 functional groups (see table above).

**Live market data.** `PriceService` fetches prices via `reqwest`. `RateCache` provides two-layer caching: in-memory `DashMap` + SQLite `finance_exchange_rates` with **15-minute TTL**.

**FIRE / Monte Carlo.** `fire_handlers.rs` calls `analytics::MonteCarloEngine` + `analytics::fire::*` directly. Actions like `fire_traditional` build a `SimulationConfig` (runs, return model, inflation model, withdrawal strategy, seed) and call `MonteCarloEngine::run`. `fire_sensitivity` sweeps withdrawal rates with `runs_per_point`.

**Six Jars allocation.** `BudgetMethod::SixJar` (aliased `sixjar`, `6jar`) parsed in `budget_create`. `JarType` enum assigns each budget to one of 6 jar categories. Validation enforces `jar_type` present when method is `SixJar`.

**Budget alerts.** `FinanceHandler` trait (impl injected from `app-core`) fires `BudgetAlert` events when spending exceeds threshold. `FinanceFeature`'s `mirror_snapshot` is keyed on `BudgetAlert` with 1-hour flush interval.

---

## Spotlight: `feature-launcher`

The most architecturally interesting crate in this subsystem. Substantial standalone systems:

**Inverted file index.** `InvertedFileIndex` (in `search/inverted_index.rs`). Stores `IndexEntry { path, name, kind, depth }` in a `Vec` with parallel `FxHashMap<PathBuf, u32>` for fast incremental updates. Name tokenized on whitespace / dot / dash / underscore / slash. Uses `fst` crate for FST-based indexing and `roaring` for bitmap operations. **Dedicated criterion benchmarks**: `benches/inverted_index.rs`, `benches/app_index_dedup.rs`.

**Clipboard history.** `ClipboardMonitor` polls the system clipboard. Entries stored in `clipboard_history` with FTS5 full-text index (`clipboard_fts`).

**Window manager.** `window_mgmt/` uses macOS Accessibility API via `platform-macos`. Actions: `leftHalf`/`rightHalf`/`topHalf`/`bottomHalf`/`leftThird`/`centerThird`/`rightThird`/`maximize`/`center`/`restore` + named presets (`preset:<name>`). `WindowManager` singleton via `window_mgmt::global`.

**Calendar source.** `search/calendar.rs` implements `SearchSource` trait as `CalendarSource`. Reads events from injected fetcher (`app-core` provides `CalendarFetcherImpl`). Default window: 1 day past, 7 days forward.

**Attention aggregator.** `services/attention_aggregator.rs`. Reads `activity_events` from productivity tables, computes **exponential decay-weighted attention seconds** (14-day half-life, λ = ln2/14), groups by `(canonical_id, kind)` (app or site), upserts into `entity_attention`. **Target: 90-day × 50k events in under 2s on Apple Silicon.**

**Frequency tracking.** `FrequencyRepo` tracks per-item launch counts in `launcher_usage_log`. Used for frecency ranking alongside attention scores.

**Wiring.** Not in `agent_loop::builder.rs` like other tools. `app-core/src/init/launcher.rs` constructs `LauncherFeature` and registers via `agent.tool_registry().write().await`. Architectural anomaly — see [`07-tools-framework.md`](./07-tools-framework.md#the-four-wiring-paths).

---

## Spotlight: `voice-engine`

**STT engines** (`TranscriptionEngine` trait):
- `Qwen3AsrEngine` — local on-device, MLX backend. Lazy model load. `IDLE_UNLOAD_SECS = 300s` (drops model from memory after idle).
- `CloudAsrEngine` — HTTP cloud transcription via `reqwest`.

**TTS engines** (`TtsEngine` trait):
- `Qwen3TtsEngine` — local MLX. **Feature-gated behind `qwen3` Cargo feature** (`features = ["dep:qwen3-tts-rs"]`). Chunks input at `MAX_CHUNK_CHARS = 400` to stay within 2048 max codes. Idle-unload after 300s. Fails at construction if feature not enabled (no silent no-op).
- `AvSpeechTtsEngine` — macOS `AVSpeechSynthesizer` via `platform-macos`.
- `CloudTtsEngine` — HTTP cloud synthesis.

**VAD.** `WebrtcVadProcessor` (GMM-based, 480-sample 30ms frames @ 16kHz). Feature-gated behind `vad` feature (`features = ["dep:webrtc-vad", "dep:nnnoiseless"]`). `nnnoiseless` provides RNNoise-based denoising. Fallback when `vad` disabled: RMS threshold. Documented `unsafe impl Send` with safety justification (no thread-local state in libfvad).

**Pronunciation pipeline — 🔴 STUB.** Both `phoneme_aligner.rs::Qwen3PhonemeAligner::align()` and `tone_analyzer.rs::classify_tone()` are stubs. Pronunciation scoring runs without real alignment data. The end-to-end pipeline is unconnected. See [`TECH_DEBT.md`](../TECH_DEBT.md#1-pure-todo--fixme--unimplemented) §1.

---

## Inter-feature dependencies

```
feature-coaching      ← subscribes to TaskEvent (feature-tasks), FinanceEvent (feature-finance),
                        ProductivityEvent (feature-productivity); reads UserSituation + flashcards
                        from cognitive

feature-language-learning ← uses voice-engine (PronunciationProvider, VoiceService);
                            shares practice_sessions table with feature-notes (migration 002)

feature-productivity  ← uses activity-log, platform-macos (OS activity capture);
                        auto_focus fires ProductivityEvent consumed by feature-coaching

feature-insights      ← reads notes via feature-notes; cognitive for cross-domain scope

feature-tasks         ← uses scheduling (FireStore, AlarmRule);
                        storage (TaskRepo, VectorStore for LanceDB);
                        bus (DomainEventBus for TaskCreated/TaskCompleted/TaskDeferred)

feature-focus         → provides DndManager/FocusBridge consumed by feature-tasks (focus_alarms)
                        and feature-productivity (focus session gating)

feature-launcher      ← uses platform-macos (running apps, window manager)
                        wired in app-core, NOT in agent builder
```

---

## Workflows

### A user adds a recurring task with an alarm

```
1. tasks.create({ title: "Standup", recur: "FREQ=DAILY;BYHOUR=9", alarms: [{ kind: "relative_before", offset: "PT15M" }] })
   ↓
2. feature-tasks::handle_create:
   - Parse RRULE → reject if uses BYSETPOS/WKST/EXRULE/RDATE
   - INSERT INTO tasks
   - INSERT INTO task_recurrence_templates (rrule, iana_tz, materialize_ahead=3, next_instance_at)
   - INSERT INTO scheduled_fires (kind="recurrence_spawn", ref_id=template_id, ...)
   ↓
3. materialize_for_task:
   - For each AlarmSpec: scheduling::AlarmRule::compute_fire_at(due, tz)
   - INSERT INTO task_alarms
   - FireStore::schedule(FireSpec { kind="task_alarm", ref_id=task_id, dedup_prefix="task:{id}:alarm:" })
   ↓
4. Publish DomainEvent::TaskCreated on bus
   ↓
5. feature-coaching::CoachingSignalConsumer receives TaskCreated → may emit nudge
```

### A finance FIRE simulation runs

```
1. finance.fire_traditional({ runs: 10000, withdrawal_rate: 0.04, ... })
   ↓
2. feature-finance::fire_handlers::handle_fire_traditional:
   - Build SimulationConfig
   - analytics::MonteCarloEngine::run(config)
     - Pure compute, no I/O, deterministic given seed
     - Returns SimulationResult { success_rate, percentile_paths, ... }
   ↓
3. Return formatted result as JSON string
```

### Launcher search

```
1. launcher.search({ query: "ter" })
   ↓
2. feature-launcher::handle_search:
   - InvertedFileIndex.search(query) → file/app candidates
   - SourceRegistry.search(query) → calendar, clipboard, browser-history, ssh hosts
   - AttentionAggregator scoring (14-day half-life)
   - FrequencyRepo scoring (launch counts)
   - Frecency rank
   ↓
3. Return ranked top-N (typically 8)
```

---

## Internals

### `feature-coaching` uses `#[derive(AiFeature)]` for skill discovery only

The crate has no LLM-visible tools but still uses the `ai-core` derive macro because that's the registration hook for skills + metric specs. The coaching itself is purely event-driven via `CoachingSignalConsumer` subscribing to the bus. The `AiFeature` registration is vestigial-looking but does real work (registers `MetricSpec`s into `MetricRegistry`).

### `feature-alarms` has no `FeaturePackage`

The crate exports only `AlarmTool`. There is no `AlarmsFeature` struct, no migrations, no health check. Scheduling lives entirely in the `scheduling` crate (`scheduled_fires` table). The standalone-alarm storage convention is:
- `kind = "standalone_alarm"`
- `dedup_prefix = "standalone:{id}:"`
- Cancellation: `FireStore::cancel_by_prefix("standalone:{id}:")`

### `analytics` is intentionally zero-dependency

No async, no storage, no side effects. All functions accept pre-fetched data + a seed for reproducibility. Called synchronously from `feature-finance::fire_handlers` after data is fetched via `FinanceStorage`. This makes the math testable in isolation and reproducible across runs.

### `voice-engine` Qwen3 engines unload after idle

Both `Qwen3AsrEngine` and `Qwen3TtsEngine` use the same pattern: `Arc<Mutex<InnerState { last_used: Instant, model: Option<Model> }>>`. A background check (or guard drop) tests `last_used.elapsed() >= 300s` and calls `model.take()` to drop the model from memory. This matters on MLX backends where model weights are multi-GB.

### `analytics` + `feature-finance` boundary

`analytics::MonteCarloEngine`, `analytics::fire::*`, `analytics::portfolio::*` are called *directly* from `feature-finance::fire_handlers`. There's no service layer between them. The split exists because:
- `analytics` can be unit-tested without any DB.
- `feature-finance` can swap to a different sim engine without changing actions.

### Cross-feature shared tables

- `practice_sessions` — defined in `feature-notes` migration 002, also used by `feature-language-learning`.
- `activity_events` (in `feature-productivity` migrations) is read by `feature-launcher::AttentionAggregator`.

Cross-feature table sharing isn't documented anywhere except this doc — historically a source of subtle migration bugs (one crate alters the schema, the other crate's queries break).

---

## Dependencies & extension points

### Upstream deps (selected)

Each tool-bearing feature consumes: `tools-core` + `tools-core-macros` (for `#[derive(Tool)]`), `storage` (for repos), `bus` (for events), `common`, `config`. Tools needing LLM access pull `providers`. Tools needing scheduling pull `scheduling`.

### Adding a new feature crate

1. Decide shape (tool-bearing / service-only / shell / library).
2. For tool-bearing: implement `FeaturePackage`. For service-only: skip `FeaturePackage`; expose via `app-core::init`.
3. Pick the registration path — Path A (FeaturePackage) is default; if your tool needs heavy injected deps, use Path B (`agent_loop::builder.rs`); if it depends on app-core state, use Path C.
4. Add `FeatureMigration`s if you own SQL tables.
5. **If you reuse a table from another crate, document it loudly.** No mechanism currently prevents two crates from owning the same schema.

### Adding a finance action

1. Add a variant to the `finance` tool's `Action` enum.
2. Implement the handler in the relevant `fire_handlers.rs` / `budget_handlers.rs` / etc.
3. Add to the action dispatch in `tool.rs`.
4. Bump the schema version if the action needs a new column.

---

## Open questions & debt

- **The `feature-*` naming convention is misleading.** Some crates are services, some are shells, some have no `FeaturePackage`. Either rename (e.g., `service-coaching`, `service-insights`) or add a `FeatureShape` trait.
- **`feature-alarms` and `feature-insights` have no `FeaturePackage` impl.** Inconsistent with the naming convention. Either add stubs or rename.
- **`feature-learning::FeaturePackage::tools()` returns `vec![]`** with a comment pointing to `crates/tools/src/domain/learning_tool.rs`. Confusing for anyone looking for the implementation.
- **`TasksFeature::tools()` returns `vec![]` unless `.with_task_tool(...)` is called.** Same for `FinanceFeature`. Footgun for plugins/tests constructing `TasksFeature::new()`.
- **`feature-productivity` has 20 tables**, almost all undocumented at the project level.
- **Voice pronunciation pipeline** is stubbed end-to-end (phoneme_aligner + tone_analyzer). Listed in [`TECH_DEBT.md`](../TECH_DEBT.md) §1.
- **Cross-feature shared tables** (`practice_sessions`, `activity_events`) have no enforcement mechanism. Add a runtime ownership registry or move shared tables to a dedicated "shared" crate.
- **`feature-coding-bash` Cargo.toml uses path deps** instead of `workspace = true` (a [`TECH_DEBT.md`](../TECH_DEBT.md#7-architectural-anomalies) item). It's actually a coding-mode crate, not assistant-mode — but worth noting here because it sits in `crates/feature-*`.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #1, #7, #8 for specifics.

---

## Cross-references

- [`02-storage.md`](./02-storage.md) — feature repos via `Repos`
- [`05-cognitive-memory.md`](./05-cognitive-memory.md) — `feature-coaching` signals + `feature-learning` tables live here
- [`06-scheduling.md`](./06-scheduling.md) — `feature-tasks`/`feature-alarms` schedule via `FireStore`
- [`07-tools-framework.md`](./07-tools-framework.md) — `#[derive(Tool)]` + `FeaturePackage` traits
- [`11-channels-mcp.md`](./11-channels-mcp.md) — assistant-mode features are exposed to MCP clients
