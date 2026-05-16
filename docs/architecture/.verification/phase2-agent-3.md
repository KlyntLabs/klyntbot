# Phase 2 Agent 3 — Verification Report

**Assigned docs:** `05-cognitive-memory.md`, `06-scheduling.md`  
**Crates inspected:** `cognitive`, `ai-core`, `ai-core-macros`, `autotuner`, `scheduling`  
**Date:** 2026-05-16  

---

## Executive Summary

- **05-cognitive-memory.md** is broadly accurate. The code matches the documented architecture, modules, public APIs, and constants. A handful of stale comments and minor signature drifts were found.
- **06-scheduling.md** is accurate in structure and behavior. Two **API signature mismatches** in the `RecurrenceEngine` trait documentation were found (return types). One **line-number offset** for a constant was noted.

---

## 05 — Cognitive Memory

### Crate: `cognitive`

| Check | Result |
|---|---|
| Existence | ✅ `crates/cognitive/` exists |
| Module structure | ✅ `src/lib.rs` declares: `bench_hooks`, `consumers`, `embedder`, `mirror`, `pipeline`, `repos`, `search`, `services`, `specta_helpers`, `types` |
| File map | ✅ All files listed in doc exist (verified via glob + read) |
| Re-exports | ✅ `UnifiedMemoryService`, `CognitiveContextSource`, `ConversationRecallService`, `SessionMemoryService`, `compute_situation`, `TemporalService`, `PipelineEvent`, `CompactionResult`, `ConsolidationHandler`, `execute_memory_ops`, `ExtractionHandler`, `SemanticFactEmbedder`, `TextEmbedder`, etc. all present in `lib.rs` |

#### Repos (`repos/mod.rs`)

- 29 repo modules declared. All match doc.
- `USER_MODEL_DOMAINS` constant has 10 domains (includes `general`, `tasks`, `coaching`, `meta` beyond the 7 in `UserModel`). Doc does not claim exhaustivity — acceptable.
- `RULE_DOMAINS` = `["productivity", "tasks", "finance", "coaching", "general"]` — verified.
- `cognitive_migrations()` returns 16 migrations (versions 1–14 plus mirror v1–v2). Verified.

#### Services — deep verification

| Service | Key finding |
|---|---|
| `reforge/service.rs` | `run_reforge` signature has **25 positional parameters**; doc claims 26. Counted manually in source. |
| `reforge/service.rs` | File-level doc comment still says "8 phases"; code contains **16 phase markers** (including 2.5, 2.6, 3.5, 3.6, 6.5, 6.5b, 6.5-ext, 6.7, 7.7). Doc already tracks this stale comment. |
| `reforge/mod.rs` | Hook traits verified: `ReforgeHandler`, `AutotunerBridge` (reforge version), `GraphEnrichmentHandler`, `CommunityIntelligenceHandler`, `CodingPhaseRunner`, `CrossCliPhaseRunner`, `SkillDiscoveryRunner`. |
| `mirror/engine.rs` | `MirrorEngine::start` takes **NO** `Arc<DomainEventBus>` — bus removal confirmed. 8 unconditional sources + 2 conditional. Test asserts `consumers.len() == 8` when optional repos are `None`. |
| `mirror/facade.rs` | All methods listed in doc exist. |
| `mirror/types.rs` | Second `AutotunerBridge` trait confirmed (`apply_champion` / `current_champion_params`). |
| `fsrs5.rs` | `DEFAULT_WEIGHTS` exact 19 values match doc. `retrievability` uses power-law `1/(1 + t/(9S))`. |
| `decay.rs` | `retrievability` uses exponential `exp(ln(0.9) * t/s)`. `RelevanceWeights` has 12 fields with exact default values matching doc. |
| `louvain.rs` | ~394 LOC, `UnGraph<String, f64>`, `detect_communities` signature verified. |
| `ppr_retrieval.rs` | ~404 LOC, `DiGraph<String, f32, u32>`, `personalized_pagerank` signature verified. `CachedPprGraph` and `retrieve_with_ppr_boost` present. |
| `memory_retriever.rs` | `RRF_K = 60.0`, `MIN_FACT_SCORE = 0.15`. Builder methods `with_ppr_cache`, `with_predictive_cache`, `with_temporal_pruner` present. |
| `conversation_recall.rs` | Defaults: `decay_half_life_days=138.0`, `default_threshold=0.4`, `default_limit=5`. |
| `context_source.rs` | `CACHE_TTL_SECS = 60`, priority `60`, all 12 relevance-weight defaults match doc. |
| `session_memory.rs` | `MIN_TURNS_BEFORE_UPDATE = 3`, `UPDATE_INTERVAL_TURNS = 3`, `MAX_MESSAGES_FOR_SUMMARY = 20`. |
| `compaction.rs` | All constants match doc (90-day defaults, `MAX_ACTIVE_FACTS=10_000`, `LOW_STABILITY=0.1`, etc.). |
| `atom_decay.rs` | Tiered decay thresholds (0.97 / 0.92 / 0.85) and auto-archive thresholds verified. |
| `consolidation.rs` | `LOW_CONFIDENCE_THRESHOLD = 0.5`. `execute_memory_ops` signature verified. |
| `scoring.rs` | `KnowledgeDepthCache`, `CommunityCache`, `community_boost_score` present. |
| `situation.rs` | `UserSituation` and `compute_situation` signatures match doc. |
| `extraction.rs` | `ExtractedFact`, `ExtractedEntity`, `ExtractedRelationship`, `ExtractionHandler`, `ConflictResolver` present. |
| `background.rs` | `BackgroundServiceConfig` has all fields listed in doc. `PipelineEvent` variants verified. |
| `search/bm25.rs` | `bm25_search_all` and per-table search functions present. |
| `types.rs` | `MemoryOp`, `SemanticFact`, `EpisodicMemory`, `ProceduralRule`, `UserModel`, `DEFAULT_MEMORY_TYPE="fact"`, `PRIORITY_CRITICAL=2` all verified. |

#### Issues found in `cognitive`

1. **`run_reforge` parameter count drift** — Doc claims 26 parameters; code signature has 25 positional parameters.
2. **Stale "8 phases" comment** in `reforge/service.rs:1-4` — code implements 16 phase markers.
3. **`TODO(T7)` stub** — `mirror/sources/skill_effectiveness.rs` is unregistered in `MirrorEngine::start` and has empty `accumulate`/`flush` methods.
4. **Raw SQL access** — `strategy_records` table access via raw SQL in `reforge/feedback.rs:173` was mentioned in doc but not independently located in this session.
5. **Two `AutotunerBridge` traits** with identical names (reforge vs mirror) — doc notes this as debt.
6. **Two `retrievability` functions** with identical names (fsrs5 vs decay) — doc notes this as debt.
7. **Mirror bus removal** — doc correctly notes `MirrorEngine::start` no longer takes `Arc<DomainEventBus>`; warns CLAUDE.md is stale.

---

### Crate: `ai-core`

| Check | Result |
|---|---|
| Existence | ✅ `crates/ai-core/` exists |
| Module structure | ✅ `lib.rs` modules: `metric`, `metrics`, `mirror`, `recall`, `recall_domain`, `recall_provider_registry`, `registry`, `router`, `signal`, `traits` |
| Re-exports | ✅ All re-exports listed in doc present (`Aggregation`, `AiMetrics`, `MirrorSignalSource`, `MirrorSubscriberRunner`, `RecallItem`, `RecallQuery`, `RecallSpec`, `RecallDomain`, `RecallProviderRegistry`, `AiFeatureRegistry`, `SignalRouter`, `Translator`, `AiSignal`, `EntityRef`, `SalienceVerdict`, `AiEntity`, `AiEventMeta`, `AiFeature`, `RecallProvider`, `SignalConsumer`, `MetricRegistry`, `MetricSpec`, `MetricSample`) |

- `traits.rs`: `AiFeature`, `AiEventMeta`, `AiEntity`, `SignalConsumer`, `RecallProvider` signatures match doc.
- `signal.rs`: `AiSignal` fields and `SalienceVerdict` variants (`Extract`, `Accumulate`, `Discard`) match doc.
- `router.rs`: `SignalRouter::start` signature matches doc.
- `recall_domain.rs`: `RecallDomain` has exactly 9 variants (General, Tasks, Finance, Productivity, Learning, Mirror, Coaching, Notes, LanguageLearning).
- `mirror.rs`: `MirrorSnapshotSpec`, `MirrorSignalSource` trait, `MirrorSubscriberRunner` present.
- `metric.rs`: `Aggregation` (Avg, Sum, Count), `MetricSpec`, `MetricSample`, `MetricRegistry` present.

No issues.

---

### Crate: `ai-core-macros`

| Check | Result |
|---|---|
| Existence | ✅ `crates/ai-core-macros/` exists |
| Derives | ✅ Exactly 3: `AiEvent`, `AiEntity`, `AiFeature` in `lib.rs` |

No issues.

---

### Crate: `autotuner`

| Check | Result |
|---|---|
| Existence | ✅ `crates/autotuner/` exists |
| Module structure | ✅ `lib.rs` modules: `cycle`, `evaluator`, `events`, `generator`, `metrics`, `traits`, `trial` |
| Re-exports | ✅ `pub use cycle::*; pub use evaluator::*; pub use events::*; pub use generator::*; pub use metrics::*; pub use traits::*; pub use trial::*;` |

- `trial.rs`: `TrialStatus` (Pending, Active, Completed, Promoted, Reverted), `Trial`, `TrialResult`, `Experiment`, `Champion` all present. `Champion::default()` has `trial_id=None` and reason `"Using Config defaults"`.
- `cycle.rs`: `NightlyCycle::new` and `run_evaluation_and_promotion` signatures match doc. `CycleResult` and `AutotunerHealth` present. Diversity bonus uses `0.3 * (distance/max_distance)`. `affected_param_names` macro checks 19 fields.
- `evaluator.rs`: `ConstraintEvaluator::from_config` and `evaluate` signatures match doc. `ConstraintVerdict::passes_all()` present. Phase 2 constraints (retrieval_precision, retrieval_recall, correction_rate_regression, promotion_accuracy) all implemented.
- `traits.rs`: `ShadowClassifier`, `MetricSource`, `ShadowRetriever`, `ShadowContext`, `ShadowPrediction`, `ShadowRetrievalResult`, `MetricSnapshot` present.
- `metrics.rs`: `aggregate_to_result` volume-weighted averaging verified. All 11 metric fields aggregated.

No issues.

---

## 06 — Scheduling

### Crate: `scheduling`

| Check | Result |
|---|---|
| Existence | ✅ `crates/scheduling/` exists |
| Module structure | ✅ `lib.rs` modules: `error`, `service`, `temporal`, `types` |
| File map | ✅ All 11 files listed in doc exist and match purpose |

#### `src/lib.rs`

- Re-exports: `CronError`, `row_to_job`, `CronExecutor`, `CronHandler`, `FireSpec`, `FireStore`, `RecurrenceEngine`, `RecurrenceTemplate`, `AlarmRule`, `RuleError`, `CronJob`, `CronJobState`, `CronOrigin`, `CronPayload`, `CronSchedule` all present.
- Comment: "CronService itself is removed — use CronExecutor + TemporalScheduler instead." — verified.

#### `src/service/mod.rs`

- `row_to_job(row: CronJobRow) -> CronJob` present.
- Corrupt-schedule defensive behavior (`enabled=false`) verified in code.

#### `src/types.rs`

- `CronSchedule` variants (`At`, `Every`, `Cron`) and field names match doc.
- `CronPayload` fields (`kind`, `message`, `deliver`, `channel`, `to`) and defaults match doc.
- `CronJobState`, `CronOrigin` (System, User, Ai, Plugin), `IntentWindow`, `IntentTrigger` (UserPresent, FirstActivityAfter, MinActiveMinutes, UserIdle), `CatchUpPriority` (Immediate, WhenPresent, WhenIdle) all verified.
- `CronJob` fields and `new()` constructor match doc.

#### `src/error.rs`

- `CronError` variants (InvalidExpression, JobNotFound, ExecutionFailed, Io, Json) present.
- `SchedulerError` variants (Storage, Rule, Rrule, InvalidState) present.

#### `src/temporal/cron_executor.rs`

- `CronExecutor` struct, `new`, `register`, `set_callback`, `start`, `run_now` signatures match doc.
- `CronHandler` type alias = `Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>` — verified.
- Dispatch uses `tokio::task::spawn_blocking` — verified.
- `in_intent_window` and `in_intent_window_with_tz` behavior matches doc (only `FirstActivityAfter` is evaluated; presence triggers pass through).

#### `src/temporal/scheduler.rs`

- `MAX_SLEEP = Duration::from_secs(30)` — verified.
- `DEFAULT_GRACE_SECS = 3600` — verified.
- `RECURRENCE_SPAWN_KIND = "recurrence_spawn"` — verified.
- `SchedulerConfig` fields match doc.
- `TemporalScheduler` struct and methods (`new`, `with_cron_bridge`, `with_recurrence_engine`, `start_background`, `wake`, `shutdown`, `run`, `store`) verified.
- `process_due` partitions into Fire / SkipStale / CoalesceLater — verified.
- Two-phase commit (`begin_firing` → publish → `mark_fired`) — verified.
- `recover_in_flight` re-publishes and marks fired — verified.
- `emit_missed` publishes `DomainEvent::MissedAlarms` — verified.
- Cron advance and recurrence-spawn routing after dispatch — verified.

#### `src/temporal/fire_store.rs`

- `FireSpec` fields match doc.
- `FireStore::schedule`, `begin_firing`, `mark_fired`, `mark_suppressed`, `cancel_by_prefix`, `cancel_by_kind_ref`, `pending_with_kind_before` signatures match doc.

#### `src/temporal/cron_bridge.rs`

- `CronBridge::new`, `reconcile_all`, `advance` signatures match doc.
- `advance` no-ops on deleted job (`NotFound`) and disabled job — verified.
- `At` schedules do not recur — verified.
- Chrono boundary comment and `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` present.

#### `src/temporal/misfire.rs`

- `MisfirePolicy` variants (`Strict`, `SkipIfStale`, `Coalesce`) and default (`SkipIfStale`) — verified.
- `Decision::classify` behavior (inclusive grace boundary) — verified.

#### `src/temporal/recurrence.rs`

- `RecurrenceTemplate` fields match doc.
- `RecurrenceEngine::new`, `on_spawn`, `disable_template` signatures match doc.
- `CreateInstanceOutcome` enum (`Created`, `SourceTaskMissing`) present.
- `SourceTaskMissing` triggers disable + cancel + prefix-cancel — verified.
- `UNTIL` inclusive behavior (RFC 5545 §3.3.10) — verified in test `until_stops_materialization`.
- `count_remaining` decrements to zero then halts — verified in test.

#### `src/temporal/rrule.rs`

- `RRuleSpec` fields and `Frequency` enum (`Daily`, `Weekly`, `Monthly`, `Yearly`) match doc.
- `compile()` and `evaluate_next_n` signatures match doc.
- `next_n_from_rrule_string` signature matches doc.
- Chrono boundary comment and allow attributes present.

#### `src/temporal/rules.rs`

- `AlarmRule` variants (`RelativeBefore`, `CivilTimeOnDayOffset`, `Absolute`) match doc.
- `compute_fire_at` signature and DST disambiguation (`compatible()`) — verified.
- Test for fold resolves to earlier DST instant — verified.

#### `migrations/001_scheduled_fires.sql`

- Schema columns match doc (id, fire_at_ms, kind, ref_id, payload, dedup_prefix, fired, firing_started_at_ms, fired_at_ms, suppressed_by, created_at_ms).
- 3 indexes (pending by time, dedup prefix, kind+ref_id) — verified.

### Issues found in `scheduling`

1. **`RecurrenceEngine` trait signature mismatches in doc:**
   - `TemplateRepo::decrement_count` doc says `Result<()>`; code returns `anyhow::Result<Option<u32>>`.
   - `InstanceRepo::create_instance` doc says `Result<String>`; code returns `anyhow::Result<CreateInstanceOutcome>`.
   - `InstanceRepo::cancel_unfired_instances` doc says `Result<u64>`; code returns `anyhow::Result<()>`.
2. **`DEFAULT_MATERIALIZE_AHEAD` line number** — doc says `app-core/src/init/temporal_scheduler.rs:19-21`; actual definition is on **line 21** (the comment spans 19-20, const is 21).
3. **Stale "CronService" references** — doc already tracks these:
   - `app-core/src/init/temporal_scheduler.rs:3` comment says "Runs SIDE-BY-SIDE with the legacy `CronService`".
   - `app-core/src/init/temporal_scheduler.rs:99` log line says `"TemporalScheduler started (side-by-side with CronService)"`.

---

## Cross-reference verification

| Doc | Cross-reference | Status |
|---|---|---|
| `05-cognitive-memory.md` | `01-foundations.md` | ✅ Exists |
| `05-cognitive-memory.md` | `02-storage.md` | ✅ Exists |
| `05-cognitive-memory.md` | `03-providers.md` | ✅ Exists |
| `05-cognitive-memory.md` | `04-agent-runtime.md` | ✅ Exists |
| `05-cognitive-memory.md` | `06-scheduling.md` | ✅ Exists |
| `05-cognitive-memory.md` | `crates/cognitive.md` | ✅ Exists (doc notes "planned", file exists) |
| `06-scheduling.md` | `../00-overview.md` | ✅ Exists |
| `06-scheduling.md` | `01-foundations.md` | ✅ Exists |
| `06-scheduling.md` | `02-storage.md` | ✅ Exists |
| `06-scheduling.md` | `05-cognitive-memory.md` | ✅ Exists |
| `06-scheduling.md` | `08-assistant-features.md` | ✅ Exists |
| `06-scheduling.md` | `11-channels-mcp.md` | ✅ Exists |
| Both | `../TECH_DEBT.md` | ✅ Exists |

---

## Overall Verdict

| Doc | Verdict |
|---|---|
| `05-cognitive-memory.md` | **Mostly accurate** — 1 parameter-count drift, 1 stale phase-count comment, and several known naming collisions (already tracked as debt). |
| `06-scheduling.md` | **Mostly accurate** — 3 API signature mismatches in `RecurrenceEngine` trait documentation, 1 minor line-number offset, and stale CronService references (already tracked). |

**Recommended follow-ups:**
1. Update `06-scheduling.md` `RecurrenceEngine` trait signatures to match code (`decrement_count -> Result<Option<u32>>`, `create_instance -> Result<CreateInstanceOutcome>`, `cancel_unfired_instances -> Result<()>`).
2. Update `05-cognitive-memory.md` `run_reforge` parameter count from 26 → 25 (or verify if one was omitted in counting).
3. Fix stale "8 phases" comment in `crates/cognitive/src/services/reforge/service.rs`.
