# Phase 2 Verification — Agent 3a (cognitive-memory.md)

**Doc:** `docs/architecture/subsystems/05-cognitive-memory.md`  
**Crates verified:** `cognitive`, `ai-core`, `ai-core-macros`, `autotuner`  
**Date:** 2026-05-16

---

## Summary

| Crate            | ✅ Accurate | ⚠️ Drift | ❌ Wrong | 🔍 Missing | 📋 Tech Debt |
|------------------|------------|----------|----------|-----------|--------------|
| `cognitive`      | 35+        | 3        | 0        | 6         | 5            |
| `ai-core`        | 12         | 0        | 0        | 1         | 0            |
| `ai-core-macros` | 8          | 2        | 0        | 0         | 0            |
| `autotuner`      | 10         | 1        | 0        | 2         | 0            |

**Cross-reference issues:** 1 broken link (`00-overview.md` missing).

---

## Per-Crate Findings

### `cognitive`

#### ✅ Accurate
- **Module / file existence:** All claimed top-level modules exist (`embedder`, `types`, `pipeline`, `consumers`, `repos`, `search`, `services`, `mirror`).
- **Pipeline files:** `signal.rs`, `collector.rs`, `*_collector.rs` (atom, chat_turn, coaching, recall, session), `consolidator.rs`, `writer.rs` all present.
- **Consumer files:** `ingestion.rs`, `metric.rs`, `retrieval.rs` present.
- **Repo files:** 30+ repo modules under `src/repos/`; all named repos in the doc (`SemanticFactRepo`, `EpisodicMemoryRepo`, `FlashcardRepo`, `FsrsParamsRepo`, `EntityRepo`, `CommunityRepo`, `CoActivationRepo`, `ProceduralRuleRepo`, `AccumulatedObservationRepo`, `FactChangelogRepo`, `RetentionHistoryRepo`, `ReviewStatsRepo`, `ReviewSessionRepo`, `AiSignalIndexRepo`, `MetricRepo`, `EventLogRepo`, etc.) exist and are re-exported from `lib.rs`.
- **Services:** `fsrs5.rs`, `decay.rs`, `fsrs_optimizer.rs`, `atom_decay.rs`, `extraction.rs`, `extraction_critic.rs`, `consolidation.rs`, `compaction.rs`, `louvain.rs`, `ppr_retrieval.rs`, `community_intelligence/`, `community_membership_online.rs`, `memory_retriever.rs`, `conversation_recall.rs`, `context_source.rs`, `graph_enrichment.rs`, `graph_linker.rs`, `hierarchical_compressor.rs`, `micro_reforge.rs`, `predictive_cache.rs`, `scoring.rs`, `session_memory.rs`, `situation.rs`, `temporal.rs`, `temporal_pruner.rs`, `tiptap_parser.rs`, `value_density.rs`, `background.rs`, `reforge/service.rs`, `reforge/{mod,collector,feedback,skill_discovery,skill_files,types}.rs` all exist.
- **Mirror files:** `engine.rs`, `facade.rs`, `narratives.rs`, `repo.rs`, `retention.rs`, `types.rs`, `sources/*.rs` all exist.
- **Public API re-exports** in `lib.rs` match the documented surface (`UnifiedMemoryService`, `ConversationRecallService`, `SessionMemoryService`, `CognitiveContextSource`, `compute_situation`, `SituationInputs`, `UserSituation`, `ConsolidationHandler`, `execute_memory_ops`, `ExtractionHandler`, `ExtractedFact`, `ExtractedEntity`, `ExtractedRelationship`, `PipelineEvent`, etc.).
- **FSRS-5 constants & functions:** `DEFAULT_WEIGHTS: [f64; 19]` and `retrievability`, `initial_stability`, `initial_difficulty`, `next_difficulty`, `next_stability_success`, `next_stability_failure` all present with documented signatures.
- **Decay:** `retrievability` (exponential) and `RelevanceWeights` (12 factors) present exactly as documented.
- **Louvain:** 394 LOC, `UnGraph<String, f64>`, `detect_communities` — verified.
- **PPR:** 404 LOC, `DiGraph<String, f32, u32>`, `personalized_pagerank`, `CachedPprGraph`, `retrieve_with_ppr_boost` — verified.
- **Two `AutotunerBridge` traits:** One in `services/reforge/mod.rs` (Phase 6 bridge: `run_evaluation`, `create_trials`) and one in `mirror/types.rs` (`apply_champion`, `current_champion_params`) — confirmed.
- **MirrorEngine::start** does **not** take `Arc<DomainEventBus>`; comment at `engine.rs:101` explicitly notes the bus was dropped.
- **Mirror signal source registration:** 8 unconditional sources + 2 conditional sources registered; `SkillEffectivenessSource` is a stub and not registered.
- **Reforge env gates:** `KCA_COMMUNITY_SUMMARIES=1` (Phase 6.7) and `KCA_REFORGE_COMPRESS=1` (Phase 7.7) both gated via `std::env::var` in `service.rs`.
- **`strategy_records` raw SQL:** `reforge/feedback.rs:167-173` queries `strategy_records` via raw SQL — confirmed.
- **Reforge hook traits:** `ReforgeHandler`, `GraphEnrichmentHandler`, `CommunityIntelligenceHandler`, `CodingPhaseRunner`, `CrossCliPhaseRunner`, `SkillDiscoveryRunner` all declared in `services/reforge/mod.rs`.

#### ⚠️ Drift
1. **`run_reforge` parameter count:** Doc claims 26 parameters. Actual signature (`service.rs:30-56`) has **25** positional parameters.
2. **Mirror source file count:** Doc says "Signal sources (10 files)". `src/mirror/sources/` contains **11 `.rs` files** (including the unregistered `skill_effectiveness.rs` stub). There are 10 *registered* sources, but 11 source files.
3. **LLM call count nuance:** Doc says "3 LLM calls at the handler level". The core `ReforgeHandler` indeed makes 3 LLM calls (Synthesize, Review, Narrate), but several extension hooks (`CodingPhaseRunner`, `GraphEnrichmentHandler`, `CommunityIntelligenceHandler`) may issue additional LLM calls. The phase table correctly marks these as "(via hook)", so the summary is acceptable but worth noting.

#### ❌ Wrong
- None.

#### 🔍 Missing (present in code, not in doc file map)
1. `src/services/graph_retrieval.rs` — used by `memory_retriever.rs` for graph-path boosts.
2. `src/services/atom_extraction.rs` — atom-level extraction helper.
3. `src/services/extraction_critic_types.rs` — types for the critic.
4. `src/services/graph_linker_types.rs` — types for graph linker.
5. `src/services/micro_reforge_types.rs` — types for micro-reforge.
6. `src/mirror/sources/coding_bash.rs` — the file backing `BackgroundJobSignalSource` (doc names the struct but not the file).

#### 📋 Tech Debt Found
1. **Stale file-level doc:** `services/reforge/service.rs:3` says "`run_reforge` drives all **8 phases**" — actual code has 16 phase markers.
2. **`SkillEffectivenessSource` stub:** `mirror/sources/skill_effectiveness.rs:77,84` contain `TODO(T7)` comments; `accumulate` and `flush` are no-ops.
3. **`run_reforge` 25-parameter signature** — still a code smell; doc already calls this out.
4. **Two `AutotunerBridge` traits with identical names** — doc already calls this out.
5. **Two `retrievability` functions** (FSRS-5 power-law vs decay exponential) — doc already calls this out.

---

### `ai-core`

#### ✅ Accurate
- **Traits:** `AiFeature`, `AiEventMeta`, `AiEntity`, `SignalConsumer`, `RecallProvider` all present in `src/traits.rs` with documented signatures.
- **Enums:** `SalienceVerdict` (`Extract`, `Accumulate`, `Discard`), `RecallDomain` (9 variants exactly), `Aggregation` (`Avg`, `Sum`, `Count`) all present.
- **SignalRouter:** `SignalRouter::start(bus, consumers, translator)` subscribes to `DomainEventBus`, applies the translator closure, and fans out to consumers in parallel tokio tasks — confirmed in `src/router.rs`.
- **Supporting types:** `AiSignal`, `EntityRef`, `RecallQuery`, `RecallItem`, `RecallSpec`, `MetricSpec`, `MetricSample`, `MetricRegistry`, `AiFeatureRegistry`, `RecallProviderRegistry`, `MirrorSignalSource`, `MirrorSubscriberRunner`, `MirrorSnapshotSpec` all present.

#### ⚠️ Drift
- None significant.

#### ❌ Wrong
- None.

#### 🔍 Missing
- `src/recall_provider_registry.rs` exists and is used but not mentioned in the doc's trait-surface summary.

#### 📋 Tech Debt
- No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` found in `ai-core/src/`.

---

### `ai-core-macros`

#### ✅ Accurate
- **Derive macros:** `#[derive(AiEvent)]`, `#[derive(AiEntity)]`, `#[derive(AiFeature)]` all declared in `src/lib.rs`.
- **`AiEvent` generated impl:** Produces `impl AiEventMeta` (`to_signal`, `event_kind`) and `const FEATURE_METRICS: &'static [&'static MetricSpec]` — confirmed in `src/ai_event.rs`.
- **Per-variant attributes parsed:** `importance`, `importance_fn`, `salience` (`extract` / `accumulate` / `discard` / `extract_if(expr)`), `observation_template`, `metric` (`name`, `window`, `min_samples`, `aggregation`, `value_from`) — all parsed in `src/attrs.rs`.
- **Tuple variant rejection:** `ai_event.rs:138-143` emits a compile error for unnamed fields.
- **`AiEntity` generated impl:** Produces `impl AiEntity` (`entity_type`, `embed_text`) and requires `#[ai(entity_type = "...", embed_on = [...])]` — confirmed.
- **`AiFeature` generated impl:** Produces `impl AiFeature`, `impl RecallProvider`, plus constants `RECALL_SPEC`, `MIRROR_SNAPSHOTS`, `PROMOTE_THRESHOLD_OVERRIDE`, `TOOL_NAME`, `ENTITY_KIND`, and `register(reg)` — confirmed in `src/ai_feature.rs`.

#### ⚠️ Drift
1. **`entity` attribute name:** Doc shorthand says `entity(type, name_from, id_from)`. The actual attribute expected by the macro is **`entity_bridge`** (e.g. `#[ai(entity_bridge(type = "...", name_from = "...", id_from = "..."))]`).
2. **`coaching` attribute name:** Doc shorthand says `coaching(rule, app_from, amount_from, category_from)`. The actual attribute expected is **`coaching_signal`** (e.g. `#[ai(coaching_signal(rule = "...", app_from = "..."))]`).

#### ❌ Wrong
- None.

#### 🔍 Missing
- None.

#### 📋 Tech Debt
- No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` found in `ai-core-macros/src/`.

---

### `autotuner`

#### ✅ Accurate
- **Trial types:** `Trial`, `TrialResult`, `TrialStatus` (`Pending`, `Active`, `Completed`, `Promoted`, `Reverted`), `Experiment`, `Champion` all present in `src/trial.rs`.
- **13 metrics:** `TrialResult` contains exactly 13 metric fields (`messages_scored`, `correction_rate`, `classification_accuracy`, `avg_tokens_per_message`, `avg_response_time_ms`, `routing_stability`, `memory_relevance`, `user_satisfaction`, `retrieval_precision`, `retrieval_recall`, `memory_freshness`, `promotion_accuracy`, `knowledge_retention_score`).
- **Cycle result:** `CycleResult` in `src/cycle.rs` has `promotion`, `regression`, `completed_count`, `failed_constraints`, `evaluated_trials`, and `health`.
- **ConstraintEvaluator:** `src/evaluator.rs` implements `ConstraintEvaluator::from_config(&AutoTunerConfig)` and evaluates all documented constraints (correction improvement, token cost, response time, routing stability, memory relevance, retrieval precision, retrieval recall, correction rate regression, promotion accuracy).
- **Traits:** `ShadowClassifier`, `MetricSource`, `ShadowRetriever` all present in `src/traits.rs`.
- **Nightly cycle workflow:** `NightlyCycle::run_evaluation_and_promotion` fetches active trials, aggregates metrics, skips insufficient messages, evaluates constraints, picks best candidate with diversity bonus, and checks champion regression — matches documented behavior.

#### ⚠️ Drift
1. **`CycleResult.health` type:** Doc shows `health` as a plain field. In code it is `Option<AutotunerHealth>` (can be `None` when health diagnosis is skipped).

#### ❌ Wrong
- None.

#### 🔍 Missing
1. `AutotunerHealth` diagnostic struct and `HealthWarning` enum (in `src/cycle.rs`) are not mentioned in the doc.

#### 📋 Tech Debt
- No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` found in `autotuner/src/`.

---

## Cross-Reference Check

| Link in doc | Target | Status |
|-------------|--------|--------|
| `../00-overview.md` | `docs/architecture/subsystems/00-overview.md` | ❌ **Missing** |
| `./01-foundations.md` | `docs/architecture/subsystems/01-foundations.md` | ✅ Exists |
| `./02-storage.md` | `docs/architecture/subsystems/02-storage.md` | ✅ Exists |
| `./03-providers.md` | `docs/architecture/subsystems/03-providers.md` | ✅ Exists |
| `./04-agent-runtime.md` | `docs/architecture/subsystems/04-agent-runtime.md` | ✅ Exists |
| `./06-scheduling.md` | `docs/architecture/subsystems/06-scheduling.md` | ✅ Exists |
| `../TECH_DEBT.md` | `docs/architecture/TECH_DEBT.md` | ✅ Exists |
| `../crates/cognitive.md` | `docs/architecture/crates/cognitive.md` | ✅ Exists |

**Note:** The broken `00-overview.md` link is inherited from the subsystem doc template; the actual overview file may reside at a different path (e.g., `docs/architecture/00-overview.md` without the `./subsystems/` prefix), but as written the relative link `../00-overview.md` from `subsystems/05-cognitive-memory.md` resolves to a non-existent file.
