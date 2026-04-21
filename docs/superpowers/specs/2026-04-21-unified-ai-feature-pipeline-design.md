# Unified AI Feature Pipeline — Design

**Date:** 2026-04-21
**Status:** Draft (pre-implementation)
**Scope:** Single design. Implementation plan will be derived from this doc via `writing-plans`.
**Pre-release policy:** No fallback, no backward compatibility, no parallel paths, no deprecation grace period. Each stage deletes the old path in the same PR that introduces the new one.

---

## 1. Problem Statement

The codebase has two user-facing features (Tasks, Finance) and six AI subsystems that consume feature signals (cognitive ingestion, salience, mirror, reforge, coaching, retrieval). Today, every consumer dispatches on the global `DomainEvent` enum through hand-written match arms, and every feature has to touch multiple unrelated crates to participate in the AI layer.

**Concrete failures this causes:**

- 20+ `DomainEvent` variants are emitted but have no subscriber anywhere (`TaskBlocked`, `TaskStatusChanged`, `TaskFieldUpdated`, `GoalProgress`, `EstimationRecorded`, `RecurringTemplateAdvanced`, all 3 `Community*` events, etc.) — silent data loss.
- `TaskCreated` falls into cognitive's catch-all path and receives importance `0.3` instead of a meaningful value.
- Mirror has zero task/finance awareness despite being the self-reflection subsystem.
- Reforge's behavioral metrics are 5 hand-written SQL queries in `feedback.rs`; adding a metric takes 3 file edits.
- Coaching's signal converter has a 14-event whitelist; everything else falls to `"Other"`.
- `TaskTool` is hand-wired into `agent_loop/builder.rs` bypassing `FeaturePackage::tools()`, creating asymmetry with Finance.
- Retrieval domains are magic strings (`"tasks"`, `"finance"`, `"general"`) with no enum or validation, so precision tracking in `RetrievalFeedbackRepo` is convention-coupled.

**What we want:** a feature declares its AI contract once (events, entities, retrieval hints, metrics) via derive macros and a trait, and every downstream AI subsystem consumes that declaration automatically. Adding a new feature or a new field becomes a one-file change. Adding a new AI consumer becomes a one-subscriber change. Neither requires edits to the other.

---

## 2. Goals & Non-Goals

### Goals

- Single feature→AI declaration surface via derive macros and one trait.
- Tasks and Finance adopt the pipeline and become structurally symmetric.
- Cognitive, mirror, reforge, coaching, retrieval all consume a common signal type.
- Eliminate all dead `DomainEvent` variants.
- Eliminate all hand-written per-feature SQL in reforge feedback.
- Eliminate all hardcoded domain strings; replace with a generated `RecallDomain` enum.
- Each migration PR deletes the old path entirely (no dual dispatch).

### Non-Goals

- Observability infrastructure (OpenTelemetry, Prometheus, metric dashboards) — CLAUDE.md explicitly calls this a non-goal.
- LLM-driven task automations (`plan_day`, `decompose`, `execute`, `forecast`, etc.) — removed 2026-04-20 and not returning.
- Changing storage engines or embedding models.
- Redesigning the skill system itself (only its registration surface).
- Rebuilding analytics (FIRE/Monte Carlo stays in the `analytics` crate, out of the pipeline).
- User-visible UI for the new pipeline (the frontend consumes existing tool APIs; the pipeline is purely internal).

---

## 3. Architecture

```
Feature Crate (L4)
 │
 │  #[derive(AiFeature)]   on the feature struct
 │  #[derive(AiEvent)]     on the feature's event enum
 │  #[derive(AiEntity)]    on the feature's primary struct(s)
 │
 ▼
Generated code:
  - impl AiFeature for TasksFeature
  - impl AiEventMeta for TaskEvent
  - impl From<TaskEvent> for DomainEvent
  - fn TaskEvent::to_signal(&self) -> AiSignal
  - fn Task::embed_text(&self) -> String
 │
 ▼
ai-core crate (L1)
  Types:   AiSignal, RecallDomain (generated), SalienceVerdict, Importance
  Traits:  AiFeature, AiEventMeta, AiEntity, SignalConsumer, RecallProvider
  Runtime: SignalRouter (broadcasts AiSignal to all SignalConsumers)
 │
 ├────────► CognitiveIngestionConsumer  (writes Observations, salience, entities)
 ├────────► MirrorSignalConsumer        (writes mirror_*_snapshots)
 ├────────► ReforgeMetricHarvester      (maintains metric tables declaratively)
 ├────────► CoachingSignalConsumer      (replaces conversion.rs whitelist)
 └────────► RetrievalIndexer            (feeds CognitiveContextSource / recall)
```

**Key dependency direction:** features depend on `ai-core`; AI subsystems depend on `ai-core`. No AI subsystem depends on any feature. No feature depends on any AI subsystem. `ai-core` depends on `bus` only for the `DomainEvent` enum definition.

---

## 4. The `ai-core` Contract (v1)

### 4.1 Crate Layout

| Crate | Layer | Kind | Purpose |
|---|---|---|---|
| `ai-core` | L1 | library | Runtime types, traits, signal router |
| `ai-core-macros` | L1 | proc-macro | `AiFeature`, `AiEvent`, `AiEntity` derives |

Both are new. `ai-core-macros` follows the same pattern as the existing `tools-core-macros` crate (derive macros at L1 with no runtime dependencies).

### 4.2 Core Types

```rust
// ai-core/src/signal.rs

pub struct AiSignal {
    pub domain: RecallDomain,          // generated enum, see §4.5
    pub event_kind: &'static str,      // e.g. "TaskCreated"
    pub importance: f64,               // 0.0–1.0
    pub salience: SalienceVerdict,     // Extract | Accumulate | Discard
    pub content: String,               // rendered from observation_template
    pub entity: Option<EntityRef>,     // Some if entity_bridge declared
    pub timestamp: jiff::Timestamp,
    pub raw_event: DomainEvent,        // for consumers needing full payload
}

pub enum SalienceVerdict { Extract, Accumulate, Discard }

pub struct EntityRef {
    pub entity_type: &'static str,     // "task" | "finance_category" | ...
    pub id: String,
    pub name: String,
}
```

### 4.3 v1 Attribute Vocabulary

The "cognitive ingestion sextet" (five ingestion attrs + one retrieval attr).

| Attribute | Scope | Purpose |
|---|---|---|
| `importance = f64` or `importance_fn = "path"` | Event variant | Observation importance 0.0–1.0. `importance_fn` for dynamic cases (e.g., high when `deviation_pct > 50`). |
| `salience = "accumulate" / "extract" / "extract_if(expr)"` | Event variant | Routes event to cognitive memory lane. |
| `entity_bridge = (type = ..., name_from = ..., id_from = ...)` | Event variant | Auto-upsert into `entities` table. |
| `observation_template = "literal {field}"` | Event variant | Observation content rendering. |
| `embed_on = [fields]` | Entity struct | Fields concatenated into embedding text. |
| `recall_domain = "..."` | Feature struct | Generates a variant in the `RecallDomain` enum. |

v1.5 / v2 / v2.5 extensions land with their consumer migrations (§6) and include: `recall_boost_when`, `recall_priority_field`, `recall_recency_field`, `recall_status_filter`, `coaching_signal`, `mirror_snapshot`, `metric`, `context_priority`, `promotion_threshold`.

### 4.4 Traits

```rust
// ai-core/src/traits.rs

pub trait AiFeature: Send + Sync + 'static {
    const DOMAIN: RecallDomain;
    const SKILL: &'static str;
    type Event: AiEventMeta + Into<DomainEvent>;
}

pub trait AiEventMeta {
    fn to_signal(&self) -> AiSignal;
    fn event_kind(&self) -> &'static str;
}

pub trait AiEntity {
    fn embed_text(&self) -> String;
    fn entity_type() -> &'static str;
    fn recall_filter(&self) -> bool;     // returns true if entity should appear in recall
}

#[async_trait]
pub trait SignalConsumer: Send + Sync {
    fn name(&self) -> &'static str;
    async fn consume(&self, signal: &AiSignal) -> common::Result<()>;
}

pub trait RecallProvider: Send + Sync {
    fn domain(&self) -> RecallDomain;
    fn score_query(&self, query: &RecallQuery) -> f64;       // 0.0 = irrelevant
    fn candidates(&self, query: &RecallQuery) -> Vec<RecallItem>;
}
```

### 4.5 Generated `RecallDomain` Enum

At compile time, every crate declaring `#[derive(AiFeature)] #[ai(recall_domain = "X")]` contributes a variant to a workspace-global `RecallDomain` enum. This replaces all `domain: String` fields across cognitive, reforge, and retrieval with a typed enum.

Implementation: `ai-core` exposes a `ai_core::recall_domains!()` macro invoked once in `ai-core/src/lib.rs` that collects variants via `inventory` crate registration (each feature's derive registers itself). The resulting enum is exported from `ai-core`.

### 4.6 SignalRouter

`ai-core::SignalRouter` is a thin runtime that subscribes to the existing `DomainEventBus`, calls `AiEventMeta::to_signal()` on each event via a dispatch table generated at startup, and broadcasts the resulting `AiSignal` to all registered `SignalConsumer` impls. Consumers are registered in `app-core` during initialization.

---

## 5. Feature-Side Changes

### 5.1 Tasks

- Introduce `crates/feature-tasks/src/events.rs` with a `TaskEvent` enum, annotated via `#[derive(DomainEvent, AiEvent)]`.
- Generated `From<TaskEvent> for DomainEvent` replaces direct emission of the global enum.
- Delete the hand-wiring in `crates/agent/src/agent_loop/builder.rs:1257-1318`; `TasksFeature::tools()` returns the `TaskTool`.
- Delete `feature-tasks/src/cognitive_bridge.rs::extract_agentic_success_rate` (orphan).
- Delete `agentic` and `hybrid` variants from `TaskType::from_str` and the tool JSON schema.
- Delete the dead summary branch at `query.rs:202-208` that counts historical `agentic` rows.
- Annotate `Task` with `#[derive(AiEntity)]`, `embed_on = ["title", "description"]`.

### 5.2 Finance

- Introduce `crates/feature-finance/src/events.rs` with a `FinanceEvent` enum.
- Add missing lifecycle events: `AccountCreated`, `BudgetCreated`, `GoalCreated`, `GoalAchieved`. (Today Finance emits only `TransactionRecorded` and `BudgetAlert`.)
- Annotate `FinanceTransaction` with `#[derive(AiEntity)]`, `embed_on = ["counterparty", "category", "subcategory"]`.
- Wire embedding — Finance has no embedding layer today; add one via the generic pipeline.

### 5.3 Bus Crate

- Delete these `DomainEvent` variants (emitted nowhere or matched nowhere):
  `TaskBlocked`, `TaskUnblocked`, `TaskStatusChanged`, `TaskPriorityChanged`, `TaskFieldUpdated`, `TaskDueDateChanged`, `TaskHierarchyChanged`, `TreeNodesRebuilt`, `RecurringTemplateAdvanced`, `GoalProgress`, `RuleEvolved`, `VoiceJournalProcessed`, `VoiceCapture`, `NarrativeGenerated`, `PredictiveAlert`, `SquadDebateCompleted`, `SquadInteractionPattern`, `MemoryPromoted`, `MessageDeferred`, `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened`, `MirrorTrialKilled`, `MirrorSnippetCreated`, `TrialActivated`.
  (Any variant a later stage actually needs gets re-added via its feature's `AiEvent` — cleaner than keeping dead weight.)
- Keep `EstimationRecorded` — it has a real consumer in reforge.

### 5.4 Pre-existing Bugs Exposed by the Migration

The audit surfaced three latent bugs that the v1 migration will touch. Fix in v1, not deferred.

| Bug | Location | Fix |
|---|---|---|
| `domain_event_log.payload` stores the variant name string, not JSON. Timeline field extraction (`task_id`, `title`, etc.) always returns `None`. | `app-core/src/init/cognitive.rs:210-213` (writer); `app-core/src/handlers/timeline.rs:211-215` (reader) | Writer serializes full `DomainEvent` to JSON via serde. Reader deserializes properly. Timeline UI starts showing real payload fields for the first time. |
| `event_log.rs::count_by_event_type_and_data` references nonexistent column `data`; will panic if called. | `cognitive/src/repos/event_log.rs:205` | Either rename to `payload` or delete the method entirely once the pipeline's declarative metrics replace its call sites. |
| Tasks migration version mismatch — trait `migrations()` returns version `2`, bootstrap in `storage.rs:107-113` manually inserts version `1`. | `feature-tasks/src/lib.rs` vs `app-core/src/init/storage.rs:107-113` | Unify on the `FeaturePackage::migrations()` trait path. Drop all `FooFeature::migrations_static()` helpers workspace-wide. |

### 5.5 `FeaturePackage` Trait Cleanup

The `FeaturePackage` trait has orphan surface area that v1 cleans up.

- Delete `FeaturePackage::config_key()` and `FeaturePackage::default_config()` — declared on the trait but never called. The config loader uses `#[serde(default)]` on sub-structs directly.
- Delete all `FooFeature::migrations_static()` helpers (Notes, Finance, Language-Learning). Bootstrap calls `FeaturePackage::migrations()` uniformly via the trait.
- Add runtime collision detection in `StoragePool::run_feature_migrations` — panic on duplicate `(feature_name, version)` pairs. Silent skip today lets version drift go unnoticed.
- `FeaturePackage::health_check()` is a no-op stub in both Tasks and Finance despite owning 8+ tables each. Both implementations must probe their tables (simple `SELECT 1 FROM <main_table> LIMIT 1`) in v1.

### 5.6 String-Literal `event_type` Queries (Pipeline Exposure)

Renaming or deleting `DomainEvent` variants silently breaks string-literal queries. The pipeline's generated `event_kind()` constants replace these.

| Location | Current | After migration |
|---|---|---|
| `reforge/feedback.rs:50-53` | `event_type = "UserCorrectedAI"` literal | Typed constant generated by `#[derive(AiEvent)]` |
| `autotuner/metric_collector.rs:67-69` | `count_by_event_type("...")` calls | Typed constants |
| `activity-log/src/normalizers.rs:46-415` | 25-arm match on variant names | Updated per stage as variants migrate (compile error catches drift — safe) |
| `simulator/src/actions.rs` + `harness.rs` | Direct `DomainEvent` construction | Constructs through typed feature event enum; `From<TaskEvent> for DomainEvent` conversion is generated |

No string comparisons on variant names remain after v1. This is tracked as a success metric in §8.

---

## 6. Staged Rollout

Each stage is **one PR** comprising pipeline migration + AI consumer improvement + dead-code kill-list. The old path is deleted in the same PR. No dual dispatch ever exists.

### Stage v1 — Foundation + Tasks + Finance + Cognitive (2 weeks)

**Pipeline work:**
- Create `ai-core` and `ai-core-macros` crates with the v1 sextet attributes.
- Migrate Tasks and Finance to the pipeline.
- Replace cognitive's `event_to_observation` and `evaluate_salience` match arms with a `SignalConsumer` that reads `AiSignal::importance` and `AiSignal::salience` directly.

**AI improvement audit:**
- Port every magic importance literal from `background.rs:899-1213` to declared attributes. Review each value; tighten any that look wrong (`TaskCreated` specifically moves from 0.3 catch-all to an intentional value).
- Add Finance embedding (previously missing entirely).
- Port `entity_bridge` logic for Tasks and Finance.

**Kill-list:**
- `background.rs`: the entire `event_to_observation` match (lines 899-1213).
- `salience.rs`: all hardcoded match arms.
- `agent_loop/builder.rs:1257-1318`: TaskTool hand-wiring.
- `feature-tasks/src/cognitive_bridge.rs::extract_agentic_success_rate`.
- `TaskType::from_str`: `agentic`/`hybrid` arms.
- `TaskTool` JSON schema: `agentic`, `hybrid`, `acceptance_criteria`, `agent_config`.
- All 25 dead `DomainEvent` variants listed in §5.3.
- `FeaturePackage::config_key()` and `FeaturePackage::default_config()` — orphan trait methods.
- All `FooFeature::migrations_static()` helpers (Notes, Finance, Language-Learning) — replaced by uniform trait path.
- `app-core/src/init/cognitive.rs:210-213` payload-storage bug — fixed in place.
- `cognitive/src/repos/event_log.rs:205` `count_by_event_type_and_data` nonexistent-column bug — fixed or method removed.
- String-literal `event_type` queries in `reforge/feedback.rs:50-53` and `autotuner/metric_collector.rs:67-69` — replaced by generated typed constants.

**Done criteria:** every Task and Finance event produces an `AiSignal` with declared importance; cognitive ingestion has no feature-specific match arms; `FeaturePackage` trait has no orphan methods; no string-literal `event_type` comparisons anywhere; `cargo clippy --workspace --all-targets` is clean; one integration test per feature proves the end-to-end pipeline.

### Stage v1.5 — Coaching + Retrieval Boost + Collector Merge (1 week)

**Pipeline work:**
- Migrate `feature-coaching/src/conversion.rs` to a `SignalConsumer`.
- Add `recall_boost_when`, `recall_priority_field`, `recall_recency_field`, `recall_status_filter` attributes.
- Wire `CognitiveContextSource` to query all registered `AiFeature` impls via `RecallProvider`.
- Merge cognitive's `pipeline/` collectors with the new pipeline. The audit confirmed `CognitiveSignal` is a complementary convergence layer, not a competing dispatch. Collectors (`AtomCollector`, `ChatTurnCollector`, `CoachingCollector`, `RecallCollector`, `SessionCollector`) change their subscription source from `DomainEventBus` → `SignalRouter`. `consolidator.rs` and `writer.rs` stay as the convergence+persistence stage (well-factored, no changes needed).

**AI improvement audit:**
- Review the 14-event whitelist in `conversion.rs`. Each event becomes a declared `coaching_signal` or is removed.
- Audit `default_conditions()`. Remove any `TriggerCondition` with zero fires in the last 30 days (if logs exist) or justify keeping it.
- Replace all string-domain usage in `RetrievalFeedbackRepo` with `RecallDomain` enum.
- Replace `coaching_collector.rs` 8 hardcoded `pattern_name` strings with declared `coaching_signal` attributes on coaching events.
- Replace each collector's `domain: "general".into()` (in `chat_turn_collector.rs:39`, `recall_collector.rs:106`, `session_collector.rs:82`) with typed `RecallDomain::General`.

**Kill-list:**
- `conversion.rs`: hardcoded 14-event match.
- `CognitiveContextSource`: any hardcoded per-feature logic.
- `RetrievalFeedbackRepo`: string domain columns (migrated in-place per pre-release policy).
- 8 hardcoded `pattern_name` strings in `coaching_collector.rs:57-68`.
- `"general"` string literals in all five cognitive collectors.

**Done criteria:** no string domain literals in AI subsystems; coaching signal whitelist is empty (everything declarative); all cognitive collectors consume `AiSignal` via `SignalRouter`, not raw `DomainEvent`.

### Stage v2 — Mirror Redesign (2 weeks)

**Pipeline work:**
- Introduce `MirrorSignalSource` trait with shared accumulator + flush scheduler base.
- Add `mirror_snapshot = "..."` attribute.
- Migrate all 4 mirror subscribers to the new trait.
- Task and Finance gain mirror snapshots (focus patterns, spending drift) via declarations.

**AI improvement audit:**
- Review the hourly-flush cadence in `RoutingMirrorSubscriber`. Validate it against real data volume.
- Review whether the 4-hour trial preview window is correct.
- Add at least 2 new snapshot types (task focus patterns, finance spending drift).

**Kill-list:**
- `subscribers/routing.rs`, `meta_rule.rs`, `version.rs`, `trial.rs`: bespoke accumulator code replaced by the trait.
- Any dead `MirrorAlert` variant.

**Done criteria:** adding a new mirror concern requires one `#[ai(mirror_snapshot = ...)]` attribute and one SQL table migration.

### Stage v2.5 — Reforge Metrics (2 weeks)

**Pipeline work:**
- Add `metric = (name, value_from, window, min_samples, aggregation)` attribute.
- Auto-generate SQL for `BehavioralMetrics` population from declared metrics.
- Replace `feedback.rs::load_behavioral_metrics` with derive-generated queries.

**AI improvement audit:**
- Audit all 5 existing metrics for correctness (thresholds, windows, sample minimums).
- Add new metrics newly cheap to declare: `focus_expiration_rate`, `budget_overrun_frequency`, `task_deferral_rate`, `goal_progress_velocity`.
- Add per-feature `promotion_threshold` attribute to replace the global `accumulate_promote_threshold` for features that need faster/slower promotion (finance budget alerts promote fast; casual chat stays slow).

**Kill-list:**
- `feedback.rs::load_behavioral_metrics`: all raw SQL.
- `BehavioralMetrics` manual struct: regenerated from declarations.

**Community & CoActivation (new in v2.5):**
- Communities and CoActivation are real Reforge Phase 6.5 outputs. Currently their `DomainEvent` variants (`CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened`) are in v1's delete list because Phase 6.5 never publishes them. v2.5 re-introduces them via `#[derive(AiEvent)]` on a new `community_intelligence::CommunityEvent` enum, cleanly typed.
- `CoActivationRepo` gains a corresponding `CoActivationStrengthened` event if coaching or retrieval consumers want it.

**Done criteria:** `BehavioralMetrics` fields match the set of `#[ai(metric)]` declarations; zero hand-written SQL in reforge feedback; community restructuring publishes typed events.

### Stage v3 — Sweep Remaining Features + MCP + Plugins (2 weeks)

**Pipeline work:**
- Migrate productivity, notes, learning atoms (4 new events: `KnowledgeAtomExtracted`, `FlashcardScheduled`, `AtomRetentionDecayed`, `AtomSemanticFactLinked`), language-learning to the pipeline.
- Each declares its event enum, entity embeddings, and retrieval hints.
- Auto-generate MCP's `default_exposed_tools()` from registered `AiFeature` implementations — replaces the hardcoded list at `config/src/schema/mcp.rs:177`.
- Auto-generate `emit_entity_update_for_tool()` dispatch from `AiFeature::entity_kind()` — replaces the hardcoded 8-tool match at `handler.rs:328`.
- WASM plugin `agent_emit_event` host function: translate plugin-emitted JSON into a typed `DomainEvent` and publish to the bus (currently drops the event). Requires sandboxed schema validation before publish.
- Wire FSRS weight write-back from autotuner into `fsrs_parameters` table (closes the current gap where `desired_retention` is observed but the 19 weights never update).

**AI improvement audit:**
- Every `DomainEvent` variant either has a declared signal shape or is deleted.
- Every `ContextSource` registration is justified or removed.
- Every skill in `skills/*/SKILL.md` is validated against the tools it declares.
- `UserSituation` / `compute_situation()` has zero callers — either wire it to `feature-coaching` or delete it.
- `InsightCacheRepo` is marked `#[deprecated]` and shims to `InsightReviewRepo` — delete.
- `MetaRule` type referenced in mirror design but absent from code — either implement or purge references.
- Squad system exists only at repo level with zero agent integration — decide: integrate into insight generation or delete.

**Kill-list:**
- All remaining dead `DomainEvent` variants across the workspace.
- Orphan `ContextSource` implementations.
- Unused `DEFAULT_SKILLS` entries.
- `config/src/schema/mcp.rs:177` hardcoded tool list.
- `handler.rs:328` hardcoded entity-kind match.
- `UserSituation` (if not wired).
- `InsightCacheRepo` (deprecated shim).
- `MetaRule` dangling references.
- Squad system (if not integrated).

**Done criteria:** the workspace `match` on `DomainEvent` exists in exactly one place (the generated `SignalRouter` dispatch); adding a new feature touches exactly one crate; MCP tool exposure, MCP entity-update emission, and plugin event publication are all auto-derived from `AiFeature` declarations.

---

## 7. Testing Strategy

- **Macro output tests** (`trybuild` / `expect-test` in `ai-core-macros/tests/`): snapshot the generated code for representative attribute combinations. Added to per stage as new attributes ship.
- **Contract integration test** (`tests/ai_pipeline_integration.rs`): publishes one of every feature event; asserts the resulting `AiSignal` shape matches expectation and every registered `SignalConsumer` receives it.
- **Invariant test** (`tests/ai_no_missed_data.rs`): iterates every `AiFeature` impl via `inventory`; asserts every declared event variant emits at least one signal when constructed with sample data. Catches silently-broken attributes.
- **Regression harness per migration**: before deleting an old path, snapshot its output for a fixture event stream; replay through the new path; diff.

---

## 8. Success Metrics

Measured at each stage merge:

1. **Hardcoded dispatch lines deleted** — `background.rs` + `salience.rs` + `conversion.rs` combined: target from ~800 lines to < 100 lines of non-generic code after v1.5.
2. **Dead `DomainEvent` variants** — current 25 → 0 after v3.
3. **"New feature" file touches** — current ~7 files (bus, background, salience, coaching conversion, mirror engine, subscriber, skill listing) → 1 file (feature crate).
4. **Cognitive importance coverage** — 0 events fall to the 0.3 catch-all after v1.
5. **Mirror feature coverage** — current 0 task/finance snapshot types → at least 4 after v2.
6. **Reforge metric count** — current 5 → 10+ after v2.5.
7. **String domain literals in AI crates** — current ~40 occurrences → 0 after v1.5.
8. **String-literal `event_type` queries** — current ~12 occurrences across reforge, autotuner, activity-log → 0 after v1 (all typed via generated constants).
9. **Orphan `FeaturePackage` trait methods called at runtime** — current 2 (`config_key`, `default_config`) → 0 after v1.
10. **Hardcoded per-tool / per-feature matches outside the feature crate** — target 0 after v3 (covers MCP tool list, entity-update dispatch, plugin event translation).

---

## 9. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Macro errors are hard to debug | `cargo expand` snapshot tests in CI; every attribute has a documented example output |
| Importance porting causes cognitive quality regression | Pre-migration snapshot of `event_to_observation` output; post-migration diff must match semantically |
| A stage's scope creeps past 2 weeks | Stage-done checklist is a hard gate; incomplete stages split, not extended |
| `RecallDomain` enum generation via `inventory` surprises Rust compiler | Prototype in v1 before committing to the pattern; fall back to manual enum if `inventory` cross-crate collection proves brittle (this is the one permitted mechanical fallback — not a backward-compat fallback) |
| Per-feature promotion thresholds conflict with autotuner's global params | v2.5 decides: autotuner global params become defaults; per-feature attributes override. Autotuner experiments on the defaults only. |

---

## 10. Pre-Release Cleanup Posture

Applied to every stage:

- No feature flags gating old vs new paths.
- No `#[deprecated]` markers; variants, functions, fields are deleted outright.
- Schema changes edit migration SQL in-place per CLAUDE.md's pre-release policy; no incremental migration files.
- No escape-hatch `extra = {...}` attribute in the macro. If a concept is needed, it becomes a first-class attribute.
- No compatibility shim types or re-exports.
- Tests that exercised deleted behavior are deleted, not skipped.

---

## 11. Open Questions

- **Cross-crate `inventory` collection** for `RecallDomain` enum generation: confirm this works with the workspace's crate graph in a v1 prototype before committing to the approach. If it does not work cleanly, switch to a workspace-global `ai-core::recall_domains!` macro invoked once at the crate root that takes the list of features as an argument. This is a mechanical choice, not a design change.
- **Autotuner interaction with per-feature promotion thresholds** — resolved in v2.5 (autotuner global = default; attribute overrides). Flagging here so v2.5 has it pre-decided.
- **Mirror snapshot retention** — currently no policy. v2 should define retention per snapshot type (e.g., routing snapshots keep 90 days; brain versions keep forever).

---

## 12. Out-of-Scope

The following are explicitly not part of this design and will not be added even if the pipeline makes them cheap:

- Observability stack (OpenTelemetry, Prometheus, external metric export).
- Reintroducing LLM-driven task automations.
- A plugin runtime for user-authored features (plugins already exist via `plugin-runtime`; the pipeline is for first-party features only).
- A new UI for inspecting the pipeline (existing MCP `MirrorTool` is sufficient for now).
