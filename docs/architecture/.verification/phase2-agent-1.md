# Architecture Docs Verification — Phase 2, Agent 1

**Docs verified:** `01-foundations.md`, `02-storage.md`
**Crates verified:** `common`, `config`, `bus`, `storage`, `session`
**Date:** 2026-05-16

---

## Summary

| Crate     | Status   | Issues |
|-----------|----------|--------|
| `common`  | ⚠️ Drift | 3 drift, 2 wrong, 2 missing |
| `config`  | ⚠️ Drift | 2 drift |
| `bus`     | ⚠️ Drift | 1 drift, 1 missing |
| `storage` | ❌ Wrong | 1 wrong, 2 drift, 1 missing |
| `session` | ✅ Accurate | 0 issues |

**Cross-reference health:** 1 broken link (`00-overview.md` missing). All other links resolve.

**Tech debt catalog:** 1 `TODO` found (common::notify.rs).

---

## Per-Crate Findings

### `common`

#### ✅ Accurate
- **Module existence:** All 16 declared modules in `src/lib.rs` exist. File map lists 17 entries (counting `time/` and `ports/` directories); 17 items exist in `src/`.
- **`KlyntbotError`:** Enum variants, `From<sqlx::Error>` impl, and sub-error enums (`ToolError`, `ProviderError`, `ChannelError`, `SessionError`, `ConfigError`) match exactly.
- **`types.rs`:** `SessionKey`, `ChannelName`, `ChatId`, `MessageRole`, `AppMode`, and all string constants (`SYSTEM_CHANNEL`, `CLI_CHANNEL`, `MCP_CHANNEL`, `CODING_CHANNEL`, `TELEGRAM_RESET_SENDER`, `MIRROR_ALERT_COST_THRESHOLD_CROSSED`) verified with correct values.
- **`SessionKey::split()`** returns `Option<(ChannelName, ChatId)>` — signature matches docs.
- **`http.rs`:** `build_http_client`, `build_http_client_with_builder`, `shared_http_client` all present with correct signatures.
- **`helpers.rs`:** `truncate_at_boundary`, `truncate_chars` present.
- **`pricing.rs`:** `MODEL_PRICING` table, `lookup()`, `cost_for()`, `cost_with_cache_for()` all present.
- **`date.rs`:** `parse_datetime_jiff` present with `fallback_tz` parameter.
- **`session_mode.rs`:** `SessionMode` enum has `Assistant`, `Coding`, `Subagent` variants (doc only mentions first two, but `Subagent` is present and valid).
- **`prompts.rs`:** `InteractionRequest`, `Question`, `Answer`, `AnswerOption`, `AnswerType`, `AnswerValue`, `FormResponse` all present.
- **`tool_channel.rs`:** `Channel` enum and `ChannelMask` bitfield present.
- **`entity_card.rs`:** `EntityCard` present.
- **`autotuner.rs`:** `TrialParams` present.
- **`coverage.rs`:** `CoverageDelta`, `FileCoverage` present.

#### ⚠️ Drift
- **`src/ports.rs` vs `src/ports/`:** Doc lists `src/ports.rs` but it is a directory (`src/ports/mod.rs` + `src/ports/notification.rs`). The `NotificationSender` trait and `OsNotificationSender` impl exist, just under a directory module.
- **Module count in diagram:** Diagram says "17 modules". `lib.rs` declares 16 `pub mod` items. If `lib.rs` itself is counted, the total is 17 source files/modules — acceptable ambiguity.
- **`ChannelMask` naming in docs:** Doc says "`Channel::All` = visible in every channel", "`Channel::NonCoding`", "`Channel::CodingOnly`". These are actually `ChannelMask::ALL`, `ChannelMask::NON_CODING`, `ChannelMask::CODING_ONLY`. The `Channel` enum only has `Coding`, `Desktop`, `Other`. This is imprecise naming but the behavioral description is correct.

#### ❌ Wrong
- **`memory.rs` purpose:** Doc claims `src/memory.rs` contains "FSRS5 + salience helpers used by cognitive". **Actual content:** `set_purge_hook` and `purge_freed_memory` — allocator-specific memory purge hooks. No FSRS5 or salience code exists in this file. This is factually incorrect.
- **`MessageBus` channel type:** Doc claims `MessageBus` is "MPMC via Clone of sender". The actual `MessageBus` uses `tokio::sync::mpsc` (multiple-producer, single-consumer). The sender can be cloned for multiple producers, but `take_inbound_rx()` / `take_outbound_rx()` can only be called **once**, giving a single consumer per direction. It is not MPMC.
- **`ContextUpdateQueue` backing:** Doc claims "MPSC; drained at iteration boundaries". Actual implementation uses `std::sync::Mutex<VecDeque<ContextUpdate>>` — no tokio channel at all.

#### 🔍 Missing
- **`time/` submodule detail:** Doc mentions `src/time/` as "Time-zone-aware utilities (Jiff-based)" but doesn't list the sub-files (`mod.rs`, `convert.rs`, `helpers.rs`) or the `now_utc`, `now_in_tz`, `system_tz` functions.
- **`NonUiPolicy`:** Present in `tool_channel.rs` but not mentioned in docs.

#### 📋 Tech Debt
- `crates/common/src/notify.rs:195` — `// TODO(priority-toast): consider adding <audio src="ms-winsoundevent:Notification.Looping.Alarm"/>`

---

### `config`

#### ✅ Accurate
- **File map:** `src/lib.rs`, `src/loader.rs`, `src/env.rs`, `src/schema/mod.rs`, `src/schema/hot.rs` all exist.
- **`loader.rs`:** `init`, `load`, `load_sync`, `save`, `save_sync`, `reload_if_changed`, `config_dir`, `config_path` all present with correct signatures.
- **`env.rs`:** `load_with_env_overrides` present. Env override semantics (double-underscore nesting, `KLYNTBOT_` prefix) match implementation.
- **`Secret<T>`:** Defined in `src/schema/core.rs:61` as `pub struct Secret<T>(T);` with `.expose()` method. Accurate.
- **Config camelCase JSON:** Verified in tests and `loader.rs` serialization.
- **Hot-reload mtime check:** `reload_if_changed` checks mtime before full parse — matches workflow.
- **Schema modules:** 40 `.rs` files in `src/schema/`, confirming "30+ sub-config modules" claim.

#### ⚠️ Drift
- **`HotConfig` fields:** Doc says hot-reloadable fields include "model, temperature, max_tokens, max_iterations, pipeline_timeout, monthly_budget". Actual `HotConfig` fields:
  - `model` ✅
  - `temperature` ✅
  - `max_tokens` ✅
  - `max_tool_iterations` (doc says `max_iterations`) ⚠️
  - `safety_timeout_secs` (doc says `pipeline_timeout`) ⚠️
  - `monthly_budget_usd` (doc says `monthly_budget`) ⚠️
  - `per_thread_cost_ceiling_usd` — **missing from doc**
  - `cost_alert_at_percent` — **missing from doc**
- **`HotConfigDiff` fields:** Doc describes diffing but doesn't enumerate fields. Actual struct has `cost_ceiling_changed` in addition to the other change flags; doc omits this.

#### ❌ Wrong
- None.

#### 🔍 Missing
- None significant.

#### 📋 Tech Debt
- None found.

---

### `bus`

#### ✅ Accurate
- **File map:** All 8 modules listed exist exactly: `queue.rs`, `events.rs`, `domain_events.rs`, `typed_broker.rs`, `event_domain.rs`, `context_updates.rs`, `injection.rs`, `learning_events.rs`.
- **`DomainEventBus`:** Present at `domain_events.rs:1226` with `new`, `publish`, `subscribe`, `publish_todo`, `publish_bash_job`, `subscriber_count` methods. Uses `tokio::sync::broadcast`.
- **`TypedBroker<T>`:** Present at `typed_broker.rs` with `new`, `publish`, `subscribe`, `receiver_count`, `sender_clone`. Uses `tokio::sync::broadcast`.
- **`LearningEventBus`:** Present at `learning_events.rs` with `new`, `publish`, `subscribe`.
- **`MessageBus`:** Present at `queue.rs` with `new`, `take_inbound_rx`, `take_outbound_rx`, `publish_inbound`, `publish_outbound`, `inbound_sender`, `outbound_sender`.
- **`events.rs`:** `InboundMessage`, `OutboundMessage`, `MessageKind` all present. `MAX_MESSAGE_SIZE = 65536`.
- **`domain_events.rs`:** `DomainEvent` enum has all claimed variants (`TaskCompleted`, `TodoEvent`, `BashJobEvent`, `CodingMemoryKind`, `FeedbackResponse`, `CorrectionKind`, `ConcurrencyClass`, etc.). `variant_name()` and `domain()` methods present.
- **`event_domain.rs`:** `EventDomain` enum with all claimed variants + `Custom(String)`.
- **`context_updates.rs`:** `ContextUpdate`, `ContextUpdateQueue`, `UpdatePriority`, `ContextUpdateReason` all present.
- **`injection.rs`:** `DynamicInjector`, `InjectorContext`, `InjectorRegistry` all present.

#### ⚠️ Drift
- **`MessageBus` description:** Doc says "async MPMC queue for InboundMessage / OutboundMessage". As noted under `common`, this is `tokio::sync::mpsc` (single consumer per direction). The `Clone` applies to senders only.

#### ❌ Wrong
- None.

#### 🔍 Missing
- **`context_updates.rs` implementation detail:** Doc doesn't mention the 30-second deduplication window (`DEDUP_WINDOW_SECS`) or the 200-item max pending limit (`MAX_PENDING`).

#### 📋 Tech Debt
- None found.

---

### `storage`

#### ✅ Accurate
- **File map:** `src/lib.rs`, `src/pool.rs`, `src/error.rs`, `src/macros.rs`, `src/sqlite_types.rs`, `src/circuit_breaker.rs`, `src/finance_storage.rs`, `src/messages/`, `src/repos/`, `src/rows/`, `src/vector_store/`, `migrations/` all exist.
- **`StoragePool` API:** `connect`, `connect_in_memory`, `from_existing`, `run_feature_migrations`, `inner`, `optimize` all present with correct signatures.
- **PRAGMAs:** `foreign_keys=ON`, `busy_timeout=5000`, `cache_size=-2000`, `journal_mode=WAL`, `wal_autocheckpoint=1000` all match.
- **Max connections:** `max_connections: 5` — verified.
- **`Repos` aggregate:** Has 30 public repo fields plus `pool`. Doc says "30+ per-domain repositories" — accurate.
- **`FinanceStorage`:** Wraps exactly 9 finance repos as claimed.
- **`DataVersionWatcherHandle`:** Present with `CancellationToken` and `Drop` impl that cancels the watcher.
- **`vector_store/mod.rs`:** `VectorStore::connect`, `get_table`, table cache, `CognitiveFactParams` re-export, `sanitize_predicate_value` re-export all present.
- **`sqlite_types.rs`:** `SqlTs` (INTEGER epoch ms) and `SqlDate` (TEXT `YYYY-MM-DD`) match docs.
- **`error.rs`:** `StorageError` (Sqlx, Migration, NotFound, Conflict, Vector, Serialization) and `OptionExt` trait present.
- **`messages/`:** `MessagePart` enum exported from `messages/mod.rs`.

#### ⚠️ Drift
- **Repo directory table:** Doc lists `NoteRepo`, `NotebookRepo`, `EntityMentionRepo` under "Notes / knowledge". These do **not** exist in `storage/src/repos/`. Notes repos appear to live in a `feature-notes` crate (as the doc itself notes with "consumed via `feature-notes` repo set"). This is acceptable since the doc qualifies it, but the table lists them alongside storage-native repos.
- **`VectorStore` search/insert API:** Doc references `VectorStore::search` and `VectorStore::insert` generically. These methods exist in submodules (`crud.rs`, `cognitive.rs`) but are not directly on the `VectorStore` struct in `mod.rs`. The doc is slightly hand-wavy about the exact API surface.

#### ❌ Wrong
- **`circuit_breaker.rs`:** Doc claims "Per-repo circuit breaker state (degrades writes after consecutive failures)" and describes "After N consecutive write failures, the breaker opens — subsequent writes return StorageError immediately without hitting SQLite. Periodic half-open probes attempt to close."
  - **Actual code:** `circuit_breaker.rs` only has three simple functions: `ensure_table`, `load`, `save`. It persists a single global `open_until_utc` deadline to a `circuit_breaker_state` table. There is **no per-repo tracking**, **no failure counting**, **no half-open probe logic**, and **no automatic degradation of writes**. This is a significant factual error.

#### 🔍 Missing
- **`test_util.rs`:** Present in `storage/src/` but not mentioned in file map.
- **`messages/render.rs`:** Present but not mentioned.
- **Vector store submodules:** `vector_store/` contains `cognitive.rs`, `community.rs`, `conv.rs`, `crud.rs`, `entity_embedding.rs`, `maintenance.rs`, `schemas.rs`, `tree_node.rs` — none mentioned in docs.

#### 📋 Tech Debt
- None found.

---

### `session`

#### ✅ Accurate
- **File map:** `src/lib.rs` and `src/manager.rs` are the only files — matches "intentionally tiny" claim.
- **`SessionManager`:** Present with `from_repo`, `get_or_create`, `save`, `save_by_key`, `reset_session`, `delete`, `list`, `has_session`, `save_compressed_prefix`, `load_compressed_prefix`, `clear_compressed_prefix`.
- **`Session`:** Present with `new`, `add_message`, `add_message_with_request_id`, `add_structured_message`, `get_history`, `clear`, `validate_and_repair`.
- **`SessionMessage`:** Present with all claimed fields (`id`, `role`, `content`, `timestamp`, `request_id`, `tool_calls`, `metadata`).
- **`SessionInfo`:** Present with `key`, `created_at`, `updated_at`, `message_count`.
- **LRU + DashMap cache:** Implementation matches doc description (concurrent per-session access, LRU eviction).
- **Compaction constants:** `COMPACTION_THRESHOLD = 200`, `COMPACTION_KEEP = 100`, `IN_MEMORY_TRIM_THRESHOLD = 60`, `IN_MEMORY_TRIM_KEEP = 40` — all present.

#### ⚠️ Drift
- None.

#### ❌ Wrong
- None.

#### 🔍 Missing
- None.

#### 📋 Tech Debt
- None found.

---

## Cross-Reference Check

### From `01-foundations.md`
| Link | Status |
|------|--------|
| `../00-overview.md` | ❌ **MISSING** — file does not exist |
| `./02-storage.md` | ✅ Exists |
| `./03-providers.md` | ✅ Exists |
| `./04-agent-runtime.md` | ✅ Exists |
| `./05-cognitive-memory.md` | ✅ Exists |
| `../TECH_DEBT.md` | ✅ Exists |

### From `02-storage.md`
| Link | Status |
|------|--------|
| `../00-overview.md` | ❌ **MISSING** — file does not exist |
| `./01-foundations.md` | ✅ Exists |
| `./04-agent-runtime.md` | ✅ Exists |
| `./05-cognitive-memory.md` | ✅ Exists |
| `./07-tools-framework.md` | ✅ Exists |
| `./08-assistant-features.md` | ✅ Exists |
| `../TECH_DEBT.md` | ✅ Exists |

### `TECH_DEBT.md` references
Both docs reference `TECH_DEBT.md` categories. The file exists; spot-checking category numbers against the doc claims was not performed in this pass (would require reading `TECH_DEBT.md` in full).

---

## Recommendations

1. **Fix `common/src/memory.rs` description** — Update doc to describe the actual memory-purge hook API, or move FSRS5 docs to the correct crate/file.
2. **Fix `storage/src/circuit_breaker.rs` description** — Either implement the claimed per-repo circuit breaker logic, or rewrite the doc to match the simple global deadline persistence that exists.
3. **Update `HotConfig` field list** — Align with actual fields (`max_tool_iterations`, `safety_timeout_secs`, `per_thread_cost_ceiling_usd`, `cost_alert_at_percent`).
4. **Fix `ports.rs` file map entry** — Change to `src/ports/` or `src/ports/mod.rs`.
5. **Create `00-overview.md`** — Both subsystem docs link to it; it is the only broken cross-reference.
6. **Clarify `MessageBus` / `ContextUpdateQueue` channel types** — Use precise terms (`mpsc` for MessageBus, `Mutex<VecDeque>` for ContextUpdateQueue) to avoid misleading readers about the concurrency model.
