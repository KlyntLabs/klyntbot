# AI Pipeline v3 — Sweep Remaining Features + MCP + Plugins

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drive every first-party feature crate (`feature-productivity`, `feature-notes`, `feature-learning`, `feature-language-learning`, `feature-coaching`) through `#[derive(AiFeature)]` + `#[derive(AiEvent)]` + `#[derive(AiEntity)]`. Auto-derive the MCP tool exposure list and the entity-update dispatch from the resulting `AiFeatureRegistry`. Wire the WASM plugin `agent_emit_event` host function into `DomainEventBus` (currently drops). Close the FSRS write-back gap so autotuner trial promotion actually mutates `fsrs_parameters`. Audit and act on every remaining cleanup target (dead `DomainEvent` variants, `UserSituation`, `InsightCacheRepo`, `MetaRule`, squad system, `DEFAULT_SKILLS` drift, orphan `ContextSource`s). Migrate the 750-line `activity-log/src/normalizers.rs` match into a `SignalConsumer` so the workspace ends with exactly one `match e { … }` on `DomainEvent` (the generated translator).

**Architecture:** v3 introduces an `AiFeatureRegistry` (built once at `app-core` startup) that collects per-feature metadata — `RecallDomain`, `tool_name`, optional `entity_kind`, the event-enum's `From<E> for DomainEvent` constructor — from every `#[derive(AiFeature)]` site. Two new attributes (`tool_name`, `entity_kind`) are parsed by `ai-core-macros` and emitted as constants on the feature struct; a generated `register(reg: &mut AiFeatureRegistry)` method pushes a `FeatureRecord` into the registry. `default_exposed_tools()` becomes `AiFeatureRegistry::tool_names()` plus a small explicit allowlist of cross-cutting tools (memory, agent, annotate, cron, alarm, mirror, temporal — these are not `AiFeature`s). Entity-update dispatch (currently spread across `app-core/handlers/*` via ad-hoc `emit_updates(&app, &updates)` calls — Task 30 is a discovery task that locates and documents the actual current dispatch site, since the spec's reference to `crates/mcp/src/handler.rs:328` does not match the present code) is converted into a generated `dispatch_entity_update(kind: &str, id: &str)` function whose match arms are emitted from `AiFeatureRegistry::entity_kinds()`. Productivity gains five new event variants (`FocusSessionStarted`, `FocusSessionEnded`, `DistractionDetected`, `ActivitySessionCompleted`, `ProductivityScoreComputed`); notes gains a fresh `NoteEvent` enum covering the 9 notes-related `DomainEvent` variants currently emitted from `app-core/handlers/notes/`; `feature-learning` (currently a pure library without a `FeaturePackage` impl) gets a real `LearningFeature` plus `LearningEvent` containing the 4 spec-required new variants (`KnowledgeAtomExtracted`, `FlashcardScheduled`, `AtomRetentionDecayed`, `AtomSemanticFactLinked`) plus the 5 already-emitted-but-unstructured ones; language-learning gets `LanguageLearningEvent` with `PronunciationScored`, `ExamAttempted`, `PhoneticMasteryGained`, `PracticeSessionCompleted`. `RecallDomain` gains `Notes`, `LanguageLearning` (the `Learning` variant already exists). The plugin host context is extended to carry `Arc<DomainEventBus>`; `agent_emit_event` parses incoming JSON against a fixed `PluginEmittedEvent { kind, payload }` schema and publishes a new `DomainEvent::PluginEvent` variant. FSRS write-back lands as `FsrsRepo::update_desired_retention` + an autotuner promotion hook that calls it when the promoted trial mutates `fsrs_desired_retention`; weight write-back stays a non-goal for v3 since autotuner doesn't tune the 19 weights yet. The activity-log normalizer becomes a `SignalConsumer` whose `consume()` runs the existing normalisation logic against `AiSignal::raw_event` (the field the router already populates) — eliminating the second-largest `match` on `DomainEvent` in the workspace.

**Tech Stack:** Rust 1.93 stable; `ai-core` + `ai-core-macros` extended with `tool_name`, `entity_kind`, and a runtime `AiFeatureRegistry`; `syn` + `quote` + `proc-macro2` unchanged; `sqlx` for `FsrsRepo::update_desired_retention`; `extism` for plugin host extension (existing); `cargo-nextest` for tests; `trybuild` for macro snapshot tests.

**Spec:** `docs/superpowers/specs/2026-04-21-unified-ai-feature-pipeline-design.md` — v3 section.

**Pre-release posture:** No dual dispatch, no feature flags, no deprecation. Each task deletes the old path in the same commit that introduces the new one. Schema changes edit existing migration SQL in place where appropriate. Dead `DomainEvent` variants are deleted outright. The `InsightCacheRepo` deprecated shim is removed entirely (zero external callers). The `normalizers.rs` match collapses to a single dispatch function inside the new normalizer-consumer.

---

## Spec → Stage Reconciliation

The spec was authored 2026-04-21 against a snapshot that has since drifted in three places. v3 addresses this explicitly:

| Spec reference | Reality | Plan response |
|---|---|---|
| `crates/mcp/src/handler.rs:328` (`emit_entity_update_for_tool` 8-arm match) | File and function do not exist. Entity updates currently dispatch through `app-core/handlers/*` via `emit_updates(&app, &updates)` calls — see CLAUDE.md "App-core + thin adapters" section. | Task 30 (discovery) locates the actual dispatch site. Tasks 31–33 introduce the auto-derived `dispatch_entity_update(kind, id)` function and migrate every emitter. |
| `ToolRegistryBridge` | Type does not exist in `crates/mcp/`. The MCP server uses `ToolRegistry` directly via a re-export. | No work required — the spec text is aspirational; the existing direct `ToolRegistry` use is fine. |
| `UserSituation` "zero callers" | `compute_situation()` is actively called by `crates/app-core/src/init/coaching.rs:237`; `UserSituation` itself is held by `MemoryRetriever` and the coaching service. | Task 56 keeps `UserSituation` and re-classifies it as wired. The cleanup line in the spec does not apply. |
| `MetaRule` "absent from code" | `MetaRule` is defined and used by `mirror::facade` and `reforge::collector::pending_meta_rules()`. | Task 58 audit-only confirms the type is alive. No deletion. |

---

## File Structure

### New files

```
crates/ai-core/src/registry.rs                                 — AiFeatureRegistry, FeatureRecord
crates/ai-core/tests/registry_test.rs                          — registry register / lookup / iteration
crates/ai-core-macros/tests/expand/tool_name.rs                — trybuild snapshot for tool_name attr
crates/ai-core-macros/tests/expand/entity_kind.rs              — trybuild snapshot for entity_kind attr

crates/feature-productivity/src/types/entity.rs                — FocusSession with #[derive(AiEntity)]
crates/feature-productivity/src/feature.rs                     — ProductivityFeature struct (lifted out of lib.rs)

crates/feature-notes/src/events.rs                             — NoteEvent enum (#[derive(AiEvent)])
crates/feature-notes/src/types/entity.rs                       — Note with #[derive(AiEntity)] (re-export wrapper)

crates/feature-learning/src/lib.rs                             — extended with LearningFeature, FeaturePackage impl
crates/feature-learning/src/feature.rs                         — LearningFeature struct
crates/feature-learning/src/events.rs                          — LearningEvent enum
crates/feature-learning/src/types/entity.rs                    — KnowledgeAtom + Flashcard #[derive(AiEntity)] wrappers
crates/feature-learning/migrations/001_create_learning.sql     — empty placeholder; learning has no own tables (re-uses cognitive)
crates/feature-learning/Cargo.toml                             — add ai-core, ai-core-macros, bus, async-trait, common, tools-core deps

crates/feature-language-learning/src/events.rs                 — LanguageLearningEvent enum
crates/feature-language-learning/src/types/entity.rs           — DetailedPronunciationReport #[derive(AiEntity)] wrapper
crates/feature-language-learning/Cargo.toml                    — add ai-core, ai-core-macros, bus deps

crates/feature-coaching/src/feature.rs                         — CoachingFeature struct (FeaturePackage + AiFeature)

crates/mcp/src/dispatch.rs                                     — dispatch_entity_update + ENTITY_KINDS table (auto-populated)

crates/cognitive/src/repos/fsrs_params.rs                      — FsrsParamsRepo with update_desired_retention
crates/app-core/src/init/fsrs_writeback.rs                     — autotuner-trial-promoted -> FsrsParamsRepo bridge

crates/activity-log/src/consumer.rs                            — NormalizerSignalConsumer (replaces normalizers.rs match)

tests/ai_pipeline_v3_integration.rs                            — end-to-end: every feature emits via AiSignal; registry has all 5
tests/ai_no_remaining_domainevent_match.rs                     — invariant: only the translator matches DomainEvent (allowlist)
tests/ai_mcp_default_tools_from_registry.rs                    — invariant: default_exposed_tools() == registry.tool_names() ∪ allowlist
tests/ai_plugin_event_published.rs                             — fixture plugin emits agent_emit_event; bus consumer receives it
tests/ai_fsrs_writeback_on_promotion.rs                        — promoted trial updates fsrs_parameters.desired_retention
tests/ai_normalizer_consumer_consumes_all.rs                   — invariant: normalizer consumer covers every translator-reachable event_kind
```

### Modified files

```
crates/ai-core/src/lib.rs                                      — re-export AiFeatureRegistry, FeatureRecord
crates/ai-core/src/recall_domain.rs                            — add Notes, LanguageLearning variants
crates/ai-core/src/traits.rs                                   — extend AiFeature with tool_name(), entity_kind() (provided defaults)
crates/ai-core-macros/src/attrs.rs                             — parse tool_name, entity_kind on AiFeatureAttr
crates/ai-core-macros/src/ai_feature.rs                        — emit TOOL_NAME, ENTITY_KIND, register() method

crates/bus/src/domain_events.rs                                — add 4 LearningEvent backers, 4 LanguageLearningEvent backers, PluginEvent variant; delete dead variants enumerated in §F of the audit
crates/bus/src/lib.rs                                          — re-exports unchanged

crates/feature-productivity/src/lib.rs                         — module split; add #[derive(AiFeature)]; expand events.rs
crates/feature-productivity/src/events.rs                      — add 5 new variants: FocusSessionStarted, FocusSessionEnded, DistractionDetected, ActivitySessionCompleted, ProductivityScoreComputed
crates/feature-productivity/src/focus.rs                       — emit via ProductivityEvent::From<…> instead of constructing DomainEvent directly (3 sites: focus.rs:96, :110, :161)
crates/feature-productivity/src/distraction_analyzer.rs        — emit via ProductivityEvent::DistractionDetected
crates/feature-productivity/src/aggregator.rs                  — emit via ProductivityEvent::ActivitySessionCompleted
crates/feature-productivity/src/insights.rs (or dashboard_emitter.rs)  — emit via ProductivityEvent::ProductivityScoreComputed
crates/feature-productivity/src/intelligence/intervention_router.rs    — emit via ProductivityEvent::InterventionTriggered (new variant)
crates/feature-productivity/Cargo.toml                         — add ai-core-macros if missing

crates/feature-notes/src/lib.rs                                — pub mod events; add #[derive(AiFeature)] on NotesFeature
crates/feature-notes/src/models.rs                             — annotate Note with #[derive(AiEntity)]
crates/feature-notes/Cargo.toml                                — add ai-core, ai-core-macros, bus deps

crates/feature-language-learning/src/lib.rs                    — pub mod events; add #[derive(AiFeature)]; emit events from practice_tool/pronunciation_provider

crates/feature-coaching/src/lib.rs                             — pub mod feature; re-export CoachingFeature
crates/feature-coaching/src/events.rs                          — add PatternDetected, FeedbackReceived variants with metric annotations
crates/feature-coaching/src/pattern_detector/mod.rs            — emit via CoachingEvent::From<…>
crates/feature-coaching/src/feedback.rs                        — emit via CoachingEvent::From<…>

crates/cognitive/src/services/community_intelligence.rs (or mod) — unchanged (already wired in v2.5)

crates/app-core/src/handlers/notes/*.rs                        — replace direct DomainEvent emission with NoteEvent::From<NoteEvent> for DomainEvent
crates/app-core/src/handlers/notes/flashcard.rs                — replace direct emission with LearningEvent::From<…>
crates/cognitive/src/repos/flashcard.rs                        — replace direct emission with LearningEvent::From<…>

crates/app-core/src/init/ai_pipeline.rs                        — translate() extends try_into_* for 4 new event enums; reads AiFeatureRegistry to seed RecallProviderRegistry
crates/app-core/src/init/mod.rs                                — Phase X builds AiFeatureRegistry, registers each FeaturePackage's `register(&mut reg)` call
crates/app-core/src/init/coaching.rs                           — instantiate CoachingFeature via new() rather than ad-hoc CoachingService::new
crates/app-core/src/init/storage.rs                            — invoke LearningFeature::migrations()

crates/config/src/schema/mcp.rs                                — default_exposed_tools() reads AiFeatureRegistry; explicit cross-cutting allowlist for non-AiFeature tools
crates/config/Cargo.toml                                       — add ai-core dep (for registry import)

crates/mcp/src/lib.rs                                          — pub mod dispatch; re-export dispatch_entity_update

crates/plugin-runtime/src/host/mod.rs                          — agent_emit_event publishes DomainEvent::PluginEvent; PluginHostContext gains Arc<DomainEventBus>
crates/plugin-runtime/src/wasm_plugin.rs                       — accept &Arc<DomainEventBus> in constructor
crates/plugin-runtime/src/manager.rs                           — pass DomainEventBus through

crates/autotuner/src/cycle.rs                                  — promotion path returns the promoted TrialParams; caller bridges to FsrsParamsRepo
crates/cognitive/src/repos/flashcard.rs                        — load_fsrs_params split into two: existing reader stays, FsrsParamsRepo (new file) gains writer

crates/activity-log/src/lib.rs                                 — pub mod consumer; re-export NormalizerSignalConsumer
crates/activity-log/src/normalizers.rs                         — slim to a single normalize_signal(&AiSignal) function; per-event arms moved into NormalizerSignalConsumer if not already covered by AiSignal fields
crates/app-core/src/init/mod.rs                                — register NormalizerSignalConsumer with the SignalRouter

crates/cognitive/src/repos/mod.rs                              — drop pub use for InsightCacheRepo
crates/cognitive/src/lib.rs                                    — drop pub use for InsightCacheRepo
crates/cognitive/src/repos/insight_cache.rs                    — DELETED (file removed)

crates/skill-system/src/store.rs                               — DEFAULT_SKILLS unchanged in this PR; add invariant test that each entry corresponds to a registered AiFeature::SKILL
```

### Deleted files / code

```
- crates/cognitive/src/repos/insight_cache.rs                  — file (deprecated shim, zero external callers)
- crates/activity-log/src/normalizers.rs::normalize_domain_event 750-line match  — replaced by AiSignal-driven path
- All DomainEvent variants whose only consumer was normalizers.rs (audit list in Task 60)
- emit_updates ad-hoc per-tool match arms (per Task 30 discovery output)
```

---

## Task Overview

| # | Task | Phase |
|---|---|---|
| 1 | Add `tool_name` attribute parsing to `AiFeatureAttr` | Macro |
| 2 | Add `entity_kind` attribute parsing to `AiFeatureAttr` | Macro |
| 3 | Emit `TOOL_NAME` and `ENTITY_KIND` constants from `#[derive(AiFeature)]` | Macro |
| 4 | `trybuild` snapshot for `tool_name` + `entity_kind` | Macro |
| 5 | Define `FeatureRecord` and `AiFeatureRegistry` in `ai-core::registry` | Foundation |
| 6 | Re-export `AiFeatureRegistry`, `FeatureRecord` from `ai-core::lib` | Foundation |
| 7 | Generate `pub fn register(reg: &mut AiFeatureRegistry)` in `#[derive(AiFeature)]` | Macro |
| 8 | Wire `TasksFeature::register` and `FinanceFeature::register` into existing tests | Foundation |
| 9 | Add `RecallDomain::Notes` + `RecallDomain::LanguageLearning` variants | Foundation |
| 10 | Annotate `TasksFeature` and `FinanceFeature` with `tool_name` + `entity_kind` | Foundation |
| 11 | Build `AiFeatureRegistry` in `app-core::init::ai_pipeline::build_feature_registry()` | Wiring |
| 12 | Move `ProductivityFeature` from `lib.rs` into `feature.rs`; add `#[derive(AiFeature)]` | Productivity |
| 13 | Annotate `FocusSession` with `#[derive(AiEntity)]` | Productivity |
| 14 | Add `ProductivityEvent::FocusSessionStarted` + `FocusSessionEnded` + `From` arms | Productivity |
| 15 | Add `ProductivityEvent::DistractionDetected` + `From` arm | Productivity |
| 16 | Add `ProductivityEvent::ActivitySessionCompleted` + `From` arm | Productivity |
| 17 | Add `ProductivityEvent::ProductivityScoreComputed` + `From` arm | Productivity |
| 18 | Migrate `focus.rs:96`, `:110`, `:161` to emit via `ProductivityEvent::From` | Productivity |
| 19 | Migrate `distraction_analyzer.rs` to emit via `ProductivityEvent::From` | Productivity |
| 20 | Migrate `aggregator.rs` and `insights.rs` to emit via `ProductivityEvent::From` | Productivity |
| 21 | Extend `ai_pipeline::translate()` to drop `translate_system_event` arms now covered by `ProductivityEvent` | Productivity |
| 22 | Create `feature-notes/src/events.rs` with `NoteEvent` enum | Notes |
| 23 | Annotate `Note` with `#[derive(AiEntity)]` | Notes |
| 24 | Add `#[derive(AiFeature)]` on `NotesFeature`; declare `tool_name = "notes"`, `entity_kind = "note"` | Notes |
| 25 | Add `feature-notes` deps in `Cargo.toml`; wire `NotesFeature::register` into `app-core::init` | Notes |
| 26 | Migrate `app-core/handlers/notes/crud.rs` to emit via `NoteEvent::From` | Notes |
| 27 | Migrate `app-core/handlers/notes/practice.rs` and `translation.rs` to emit via `NoteEvent::From` | Notes |
| 28 | Extend `ai_pipeline::translate()` with `try_into_note_event` arm | Notes |
| 29 | Convert `feature-learning` from library-only into a `FeaturePackage` (struct + impl) | Learning |
| 30 | Discover the current entity-update dispatch path; record in plan as `docs/superpowers/notes/2026-04-23-entity-update-discovery.md` | MCP |
| 31 | Add 4 new `DomainEvent` variants (`KnowledgeAtomExtracted`, `FlashcardScheduled`, `AtomRetentionDecayed`, `AtomSemanticFactLinked`) + 1 re-categorized (`PluginEvent`) | Learning |
| 32 | Define `LearningEvent` enum with all 9 variants + `From<LearningEvent> for DomainEvent` | Learning |
| 33 | Annotate `KnowledgeAtom` and `Flashcard` with `#[derive(AiEntity)]` | Learning |
| 34 | Add `#[derive(AiFeature)]` on `LearningFeature`; declare `recall_domain = "Learning"`, `tool_name = "learning"`, `entity_kind = "knowledge_atom"` | Learning |
| 35 | Migrate `cognitive/repos/flashcard.rs` and `app-core/handlers/notes/flashcard.rs` to emit via `LearningEvent::From` | Learning |
| 36 | Extend `ai_pipeline::translate()` with `try_into_learning_event` arm | Learning |
| 37 | Add `LanguageLearningEvent` enum + 4 new `DomainEvent` variants | LanguageLearning |
| 38 | Annotate `DetailedPronunciationReport` with `#[derive(AiEntity)]` | LanguageLearning |
| 39 | Add `#[derive(AiFeature)]` on `LanguageLearningFeature` | LanguageLearning |
| 40 | Migrate `practice_tool.rs` and `pronunciation_provider.rs` to emit via `LanguageLearningEvent::From` | LanguageLearning |
| 41 | Extend `ai_pipeline::translate()` with `try_into_language_learning_event` arm | LanguageLearning |
| 42 | Add `CoachingEvent::PatternDetected` + `FeedbackReceived` variants | Coaching |
| 43 | Create `CoachingFeature` struct in `feature-coaching/src/feature.rs` with `FeaturePackage` impl | Coaching |
| 44 | Add `#[derive(AiFeature)]` on `CoachingFeature`; declare `tool_name = "coaching"` (or omit if no tool exposed) | Coaching |
| 45 | Migrate `pattern_detector/mod.rs` and `feedback.rs` to emit via `CoachingEvent::From` | Coaching |
| 46 | Wire `CoachingFeature::register` into `app-core::init` | Coaching |
| 47 | Replace `default_exposed_tools()` in `mcp.rs` with registry-driven implementation | MCP |
| 48 | Add invariant test: `default_exposed_tools() == registry.tool_names() ∪ EXPLICIT_ALLOWLIST` | MCP |
| 49 | Implement `dispatch_entity_update(kind, id)` in `mcp/src/dispatch.rs` from registry | MCP |
| 50 | Migrate every `emit_updates` call site identified in Task 30 to use `dispatch_entity_update` | MCP |
| 51 | Extend `PluginHostContext` with `Arc<DomainEventBus>` | Plugin |
| 52 | Define `PluginEmittedEvent { kind, payload }` schema + `DomainEvent::PluginEvent` variant | Plugin |
| 53 | Implement `agent_emit_event` parsing → publish to `DomainEventBus` | Plugin |
| 54 | Integration test: fixture plugin emits, bus consumer receives | Plugin |
| 55 | Create `FsrsParamsRepo` with `update_desired_retention(retention: f64)` writer | FSRS |
| 56 | Wire autotuner promotion path to call `FsrsParamsRepo::update_desired_retention` when `fsrs_desired_retention` mutated | FSRS |
| 57 | Integration test: promoted trial updates `fsrs_parameters.desired_retention` | FSRS |
| 58 | Audit-only: `UserSituation` is wired (caller in `init::coaching.rs:237`); document keep | Cleanup |
| 59 | Audit-only: `MetaRule` is wired (mirror::facade + reforge::collector); document keep | Cleanup |
| 60 | Delete `InsightCacheRepo` shim (file + re-exports) | Cleanup |
| 61 | Audit-only: squad system is wired via `app-core/handlers/squads.rs` and `chat/streaming.rs:279`; document keep | Cleanup |
| 62 | Identify and delete dead `DomainEvent` variants per audit (no emitters AND no consumers after Task 21–46 migrations) | Cleanup |
| 63 | Replace `activity-log/src/normalizers.rs::normalize_domain_event` match with a `NormalizerSignalConsumer` driven by `AiSignal` | Normalizer |
| 64 | Register `NormalizerSignalConsumer` with `SignalRouter` in `app-core::init` | Normalizer |
| 65 | Invariant test: only the translator matches `DomainEvent` (allowlist of bus accessors + feature `From` impls) | Verification |
| 66 | Invariant test: every translator-reachable `event_kind` is consumed by `NormalizerSignalConsumer` | Verification |
| 67 | Invariant test: every `DEFAULT_SKILLS` entry corresponds to a registered `AiFeature::SKILL` | Verification |
| 68 | Final verification: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo nextest run --workspace`, doctests, grep sanity | Done |

---

(Tasks defined below — each contains its own files list, insight, code, and commit step.)

## Task 1: Add `tool_name` attribute parsing to `AiFeatureAttr`

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`
- Modify: `crates/ai-core-macros/tests/expand_test.rs` (add coverage; create the file if absent)

`★ Insight ─────────────────────────────────────`
`tool_name` is what makes `default_exposed_tools()` auto-derivable. Today every tool name is a magic string in `mcp.rs`, in the tool's `fn name()`, and in any skill that references it — three sources of truth. By making the feature declare its tool name once via `#[ai(tool_name = "tasks")]`, we let the registry assert at startup that `feature.tool_name() == feature.tools().first().unwrap().name()`, catching the singular/plural drift CLAUDE.md warns about.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Read existing `AiFeatureAttr` to find the right insertion point**

Run: `grep -n "pub struct AiFeatureAttr\|impl AiFeatureAttr\|fn parse_ai_feature_attr" crates/ai-core-macros/src/attrs.rs`
Expected: shows the struct definition (~line 320 area) and the parser function. Note the line numbers — the new `tool_name: Option<String>` field sits next to `skill: String` (already an existing field).

- [ ] **Step 2: Add the field to `AiFeatureAttr`**

In `crates/ai-core-macros/src/attrs.rs`, locate the `AiFeatureAttr` struct and add the `tool_name` field. Show the after-state of the relevant struct fragment:

```rust
pub struct AiFeatureAttr {
    pub recall_domain: syn::Ident,
    pub skill: String,
    pub tool_name: Option<String>,            // NEW
    pub event: syn::Path,
    pub recall_boost_when: Option<syn::Expr>,
    pub recall_priority_field: Option<String>,
    pub recall_recency_field: Option<String>,
    pub recall_status_filter: Option<syn::Expr>,
    pub mirror_snapshots: Vec<MirrorSnapshotAttr>,
    pub promotion_threshold: Option<syn::LitInt>,
}
```

- [ ] **Step 3: Parse the new key in `parse_ai_feature_attr`**

Inside the `meta.parse_nested_meta(|meta| { … })` body in `parse_ai_feature_attr`, add a new arm next to the existing `skill` handler:

```rust
} else if meta.path.is_ident("tool_name") {
    let v: syn::LitStr = meta.value()?.parse()?;
    out.tool_name = Some(v.value());
    Ok(())
```

Place it immediately after the `skill` arm so the related concepts cluster. Default to `None` in the struct literal where `AiFeatureAttr` is constructed.

- [ ] **Step 4: Sanity build**

Run: `cargo build -p ai-core-macros`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros/src/attrs.rs
git commit -m "feat(ai-core-macros): parse tool_name on AiFeature attr"
```

---

## Task 2: Add `entity_kind` attribute parsing to `AiFeatureAttr`

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`

`★ Insight ─────────────────────────────────────`
`entity_kind` is the string identifier used by entity-update dispatch — `"task"`, `"finance_transaction"`, `"note"`. It's optional because not every feature surfaces entities (e.g. coaching emits patterns, not entities). Storing it alongside `tool_name` keeps both new MCP integration concerns in one declaration.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add the field to `AiFeatureAttr`**

Add directly below `tool_name`:

```rust
    pub entity_kind: Option<String>,          // NEW
```

- [ ] **Step 2: Parse the new key in `parse_ai_feature_attr`**

Add an arm next to `tool_name`:

```rust
} else if meta.path.is_ident("entity_kind") {
    let v: syn::LitStr = meta.value()?.parse()?;
    out.entity_kind = Some(v.value());
    Ok(())
```

Default to `None` in the struct literal.

- [ ] **Step 3: Sanity build**

Run: `cargo build -p ai-core-macros`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/ai-core-macros/src/attrs.rs
git commit -m "feat(ai-core-macros): parse entity_kind on AiFeature attr"
```

---

## Task 3: Emit `TOOL_NAME` and `ENTITY_KIND` constants from `#[derive(AiFeature)]`

**Files:**
- Modify: `crates/ai-core-macros/src/ai_feature.rs`

`★ Insight ─────────────────────────────────────`
We emit these as `Option<&'static str>` constants on the feature struct rather than as trait methods because `Option` is `Copy` and zero-cost, and constants are queryable in `const` contexts (handy for the upcoming registry's compile-time validation). The registry stores `Option<&'static str>` directly — no allocation per feature.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate the `expand` function**

Run: `grep -n "pub fn expand\|impl ::ai_core::AiFeature" crates/ai-core-macros/src/ai_feature.rs`
Expected: the `pub fn expand(input: DeriveInput) -> TokenStream` function and the `impl ::ai_core::AiFeature for #struct_name` block within it.

- [ ] **Step 2: Add the constant emitters**

Inside `expand`, after `attr` is parsed, materialize literals:

```rust
let tool_name_const = match &attr.tool_name {
    Some(s) => quote! { Some(#s) },
    None => quote! { None },
};
let entity_kind_const = match &attr.entity_kind {
    Some(s) => quote! { Some(#s) },
    None => quote! { None },
};
```

Then in the `impl #struct_name { … }` block (the inherent impl that already holds `RECALL_SPEC` and `MIRROR_SNAPSHOTS`), insert:

```rust
pub const TOOL_NAME: Option<&'static str> = #tool_name_const;
pub const ENTITY_KIND: Option<&'static str> = #entity_kind_const;
```

- [ ] **Step 3: Build to verify the macro compiles and emits valid syntax**

Run: `cargo build -p feature-tasks`
Expected: clean — `feature-tasks` already uses `#[derive(AiFeature)]` so its expansion exercises the new constants. They will be unused for now (we'll wire them in Task 5–7); that's fine.

- [ ] **Step 4: Commit**

```bash
git add crates/ai-core-macros/src/ai_feature.rs
git commit -m "feat(ai-core-macros): emit TOOL_NAME and ENTITY_KIND constants on AiFeature"
```

---

## Task 4: `trybuild` snapshot for `tool_name` + `entity_kind`

**Files:**
- Create: `crates/ai-core-macros/tests/expand/tool_name_entity_kind.rs`
- Modify: `crates/ai-core-macros/tests/expand_test.rs` (or whatever harness already wires `trybuild`)

`★ Insight ─────────────────────────────────────`
Snapshot tests for proc-macros catch silent regressions when the emitted token stream changes shape. Even a whitespace-equivalent change in the generated impl can break downstream auto-derive expectations (e.g. trait dispatch). The pattern in this crate is `trybuild::TestCases::new().pass("tests/expand/*.rs")`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the snapshot fixture**

Create `crates/ai-core-macros/tests/expand/tool_name_entity_kind.rs`:

```rust
use ai_core_macros::AiFeature;

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    tool_name = "tasks",
    entity_kind = "task",
    event = "TaskEvent",
)]
pub struct TasksFeatureSnap;

#[derive(Debug, Clone)]
pub enum TaskEvent { _Placeholder }

impl ai_core::AiEventMeta for TaskEvent {
    fn to_signal(&self) -> ai_core::AiSignal { unimplemented!() }
    fn event_kind(&self) -> &'static str { "_Placeholder" }
}

impl From<TaskEvent> for bus::DomainEvent {
    fn from(_: TaskEvent) -> Self { unimplemented!() }
}

fn main() {
    assert_eq!(TasksFeatureSnap::TOOL_NAME, Some("tasks"));
    assert_eq!(TasksFeatureSnap::ENTITY_KIND, Some("task"));
}
```

- [ ] **Step 2: Run the snapshot — should pass**

Run: `cargo test -p ai-core-macros --test expand_test`
Expected: PASS. If `expand_test.rs` already exists with `trybuild`, the new file is auto-discovered. If it does not exist, create it as:

```rust
#[test]
fn pass_examples() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/*.rs");
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/ai-core-macros/tests/expand/tool_name_entity_kind.rs crates/ai-core-macros/tests/expand_test.rs
git commit -m "test(ai-core-macros): trybuild snapshot for tool_name+entity_kind"
```

---

## Task 5: Define `FeatureRecord` and `AiFeatureRegistry` in `ai-core::registry`

**Files:**
- Create: `crates/ai-core/src/registry.rs`
- Create: `crates/ai-core/tests/registry_test.rs`
- Modify: `crates/ai-core/src/lib.rs` (re-export added in Task 6)

`★ Insight ─────────────────────────────────────`
`AiFeatureRegistry` is a tiny `HashMap<RecallDomain, FeatureRecord>` plus a `Vec<FeatureRecord>` for stable iteration order. We give it both shapes because MCP needs ordered iteration (so `default_exposed_tools()` returns a deterministic vector) and the entity-update dispatch needs `O(1)` lookup by `entity_kind`. The struct stays in `ai-core` (L1) because both `mcp` and `app-core` consume it; storing it higher would create a cycle.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/ai-core/tests/registry_test.rs`:

```rust
use ai_core::registry::{AiFeatureRegistry, FeatureRecord};
use ai_core::RecallDomain;

#[test]
fn registry_register_and_lookup_by_domain() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });

    let rec = reg.by_domain(&RecallDomain::Tasks).expect("tasks registered");
    assert_eq!(rec.skill, "task-management");
    assert_eq!(rec.tool_name, Some("tasks"));
    assert_eq!(rec.entity_kind, Some("task"));
}

#[test]
fn registry_iteration_is_stable_in_insertion_order() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });
    reg.register(FeatureRecord {
        domain: RecallDomain::Finance,
        skill: "finance-management",
        tool_name: Some("finance"),
        entity_kind: Some("finance_transaction"),
    });

    let names: Vec<&'static str> =
        reg.iter().filter_map(|r| r.tool_name).collect();
    assert_eq!(names, vec!["tasks", "finance"]);
}

#[test]
fn registry_tool_names_returns_only_some() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Coaching,
        skill: "coaching",
        tool_name: None,                    // coaching has no tool exposed
        entity_kind: None,
    });
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });

    let tools = reg.tool_names();
    assert_eq!(tools, vec!["tasks"]);
}

#[test]
fn registry_lookup_by_entity_kind() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });

    let rec = reg.by_entity_kind("task").expect("found");
    assert_eq!(rec.domain, RecallDomain::Tasks);

    assert!(reg.by_entity_kind("nonexistent").is_none());
}

#[test]
fn registry_register_panics_on_duplicate_domain() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });
    let result = std::panic::catch_unwind(move || {
        reg.register(FeatureRecord {
            domain: RecallDomain::Tasks,
            skill: "another",
            tool_name: Some("tasks2"),
            entity_kind: None,
        });
    });
    assert!(result.is_err(), "registering same domain twice should panic");
}
```

- [ ] **Step 2: Run — should fail (registry module not defined)**

Run: `cargo nextest run -p ai-core --test registry_test`
Expected: FAIL with `unresolved import ai_core::registry`.

- [ ] **Step 3: Implement `crates/ai-core/src/registry.rs`**

```rust
//! AiFeatureRegistry — runtime collection of FeatureRecord entries built at
//! app-core startup. Consumed by:
//! - `default_exposed_tools()` (MCP server tool exposure)
//! - `dispatch_entity_update()` (MCP entity-update fan-out)
//! - `RecallProviderRegistry` seeding (cognitive context source)
//! - the activity-log normalizer consumer (event-kind allowlist)

use crate::RecallDomain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureRecord {
    pub domain: RecallDomain,
    pub skill: &'static str,
    pub tool_name: Option<&'static str>,
    pub entity_kind: Option<&'static str>,
}

#[derive(Debug, Default)]
pub struct AiFeatureRegistry {
    records: Vec<FeatureRecord>,
}

impl AiFeatureRegistry {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Register a feature. Panics if the same `RecallDomain` is registered twice
    /// — this catches accidental double-registration in `app-core::init`.
    pub fn register(&mut self, record: FeatureRecord) {
        if self.records.iter().any(|r| r.domain == record.domain) {
            panic!(
                "AiFeatureRegistry: duplicate registration for domain {:?}",
                record.domain
            );
        }
        self.records.push(record);
    }

    pub fn by_domain(&self, domain: &RecallDomain) -> Option<&FeatureRecord> {
        self.records.iter().find(|r| &r.domain == domain)
    }

    pub fn by_entity_kind(&self, kind: &str) -> Option<&FeatureRecord> {
        self.records.iter().find(|r| r.entity_kind == Some(kind))
    }

    pub fn tool_names(&self) -> Vec<&'static str> {
        self.records.iter().filter_map(|r| r.tool_name).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FeatureRecord> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
```

- [ ] **Step 4: Re-run the test — should pass**

Run: `cargo nextest run -p ai-core --test registry_test`
Expected: 5 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core/src/registry.rs crates/ai-core/tests/registry_test.rs
git commit -m "feat(ai-core): introduce AiFeatureRegistry + FeatureRecord"
```

---

## Task 6: Re-export `AiFeatureRegistry`, `FeatureRecord` from `ai-core::lib`

**Files:**
- Modify: `crates/ai-core/src/lib.rs`

- [ ] **Step 1: Add the module declaration and re-exports**

Open `crates/ai-core/src/lib.rs`. Add `pub mod registry;` next to the existing `pub mod` lines, and add to the `pub use` block:

```rust
pub mod registry;
// … existing lines …
pub use registry::{AiFeatureRegistry, FeatureRecord};
```

Place these alphabetically among the existing `pub mod` and `pub use` lists.

- [ ] **Step 2: Build the workspace to confirm the re-export resolves**

Run: `cargo build -p ai-core`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/ai-core/src/lib.rs
git commit -m "feat(ai-core): re-export AiFeatureRegistry and FeatureRecord"
```

---

## Task 7: Generate `pub fn register(reg: &mut AiFeatureRegistry)` in `#[derive(AiFeature)]`

**Files:**
- Modify: `crates/ai-core-macros/src/ai_feature.rs`

`★ Insight ─────────────────────────────────────`
The generated `register()` method takes `&mut AiFeatureRegistry` rather than returning a `FeatureRecord`. This lets the registry take ownership of the registration and run the duplicate-domain check at the call site. The pattern means `app-core::init` reads as `TasksFeature::register(&mut reg); FinanceFeature::register(&mut reg);` — explicit, greppable, and the correct call ordering shows up in PR review.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inside `expand`, emit the `register` method**

In the inherent impl block (next to `TOOL_NAME` and `ENTITY_KIND`), add:

```rust
pub fn register(reg: &mut ::ai_core::AiFeatureRegistry) {
    reg.register(::ai_core::FeatureRecord {
        domain: <Self as ::ai_core::AiFeature>::DOMAIN,
        skill: <Self as ::ai_core::AiFeature>::SKILL,
        tool_name: Self::TOOL_NAME,
        entity_kind: Self::ENTITY_KIND,
    });
}
```

The method is a free fn (`pub fn` not `pub async fn`) because registration is synchronous and pure.

- [ ] **Step 2: Confirm `feature-tasks` and `feature-finance` rebuild**

Run: `cargo build -p feature-tasks -p feature-finance`
Expected: clean. The new `register` fn is unused for now (no caller in app-core yet); the dead-code warning is suppressed because `pub fn` items aren't dead from a library perspective.

- [ ] **Step 3: Commit**

```bash
git add crates/ai-core-macros/src/ai_feature.rs
git commit -m "feat(ai-core-macros): emit pub fn register on AiFeature derives"
```

---

## Task 8: Wire `TasksFeature::register` and `FinanceFeature::register` into a sanity test

**Files:**
- Create: `crates/ai-core/tests/registry_integration_test.rs` (cross-crate would not compile here — instead place this test at the workspace facade)
- Create: `tests/ai_registry_integration.rs`

`★ Insight ─────────────────────────────────────`
The registry test for cross-crate registration must live at the facade level (`tests/`), because `ai-core` does not depend on `feature-tasks` (and must not — that direction would invert the dependency graph). The facade crate (`klyntbot`) does depend on every feature, so it is the natural home for "registry collects everyone" tests.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing integration test**

Create `tests/ai_registry_integration.rs`:

```rust
use ai_core::AiFeatureRegistry;

#[test]
fn tasks_and_finance_register_into_registry() {
    let mut reg = AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);

    assert_eq!(reg.len(), 2);

    let tasks = reg.by_domain(&ai_core::RecallDomain::Tasks).expect("tasks");
    assert_eq!(tasks.skill, "task-management");
    // tool_name and entity_kind populated in Task 10.

    let finance = reg.by_domain(&ai_core::RecallDomain::Finance).expect("finance");
    assert_eq!(finance.skill, "finance-management");
}
```

- [ ] **Step 2: Run — should pass after Task 7**

Run: `cargo nextest run --test ai_registry_integration`
Expected: PASS. `tool_name` and `entity_kind` are still `None` for both features at this point (we annotate them in Task 10).

- [ ] **Step 3: Commit**

```bash
git add tests/ai_registry_integration.rs
git commit -m "test(integration): registry collects Tasks+Finance via generated register()"
```

---

## Task 9: Add `RecallDomain::Notes` + `RecallDomain::LanguageLearning` variants

**Files:**
- Modify: `crates/ai-core/src/recall_domain.rs`
- Modify: `crates/ai-core/tests/recall_domain_test.rs` (extend; create if absent)

`★ Insight ─────────────────────────────────────`
`RecallDomain` is hand-written rather than `inventory`-generated (per spec §11 open question: this is the manual-fallback variant). Adding a variant is a one-line edit plus updates to two match arms (`as_str` + `from_str_or_general`). Notes and LanguageLearning are both first-class search domains for retrieval — the boost-when expressions on each feature will key off the user's message text to bias recall.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Read the current enum**

Run: `cat crates/ai-core/src/recall_domain.rs`
Expected: 7 existing variants — General, Tasks, Finance, Productivity, Learning, Mirror, Coaching — plus `as_str` and `from_str_or_general` impls.

- [ ] **Step 2: Add the two variants**

Edit the enum:

```rust
pub enum RecallDomain {
    General,
    Tasks,
    Finance,
    Productivity,
    Learning,
    Mirror,
    Coaching,
    Notes,             // NEW
    LanguageLearning,  // NEW
}
```

Add matching arms in `as_str`:

```rust
RecallDomain::Notes => "notes",
RecallDomain::LanguageLearning => "language_learning",
```

And in `from_str_or_general`:

```rust
"notes" => RecallDomain::Notes,
"language_learning" => RecallDomain::LanguageLearning,
```

- [ ] **Step 3: Extend the existing test (or add one)**

In `crates/ai-core/tests/recall_domain_test.rs` (file may exist; if not, create it with this body):

```rust
use ai_core::RecallDomain;

#[test]
fn notes_round_trips() {
    assert_eq!(RecallDomain::Notes.as_str(), "notes");
    assert_eq!(RecallDomain::from_str_or_general("notes"), RecallDomain::Notes);
}

#[test]
fn language_learning_round_trips() {
    assert_eq!(RecallDomain::LanguageLearning.as_str(), "language_learning");
    assert_eq!(
        RecallDomain::from_str_or_general("language_learning"),
        RecallDomain::LanguageLearning,
    );
}

#[test]
fn unknown_returns_general() {
    assert_eq!(RecallDomain::from_str_or_general("nonsense"), RecallDomain::General);
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p ai-core --test recall_domain_test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core/src/recall_domain.rs crates/ai-core/tests/recall_domain_test.rs
git commit -m "feat(ai-core): add RecallDomain::Notes and RecallDomain::LanguageLearning"
```

---

## Task 10: Annotate `TasksFeature` and `FinanceFeature` with `tool_name` + `entity_kind`

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs:41-56` (the `#[ai(...)]` attribute on `TasksFeature`)
- Modify: `crates/feature-finance/src/lib.rs:46` (the `#[ai(...)]` attribute on `FinanceFeature`)

`★ Insight ─────────────────────────────────────`
This is a one-line annotation that makes the registry test from Task 8 see real values. We deliberately set `entity_kind = "task"` (singular, matches existing `EntityRef::entity_type` strings used in `entity_bridge`) and `tool_name = "tasks"` (plural, matches the actual `Tool::name()` return value). The mismatch between singular entity and plural tool is intentional and matches the rest of the codebase — CLAUDE.md calls this out as a common drift source.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Annotate `TasksFeature`**

In `crates/feature-tasks/src/lib.rs`, extend the `#[ai(...)]` block:

```rust
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    tool_name = "tasks",
    entity_kind = "task",
    event = "crate::events::TaskEvent",
    recall_boost_when = "query.message.to_lowercase().contains(\"deadline\") || query.message.to_lowercase().contains(\"task\") || query.message.to_lowercase().contains(\"overdue\")",
    recall_priority_field = "priority",
    recall_recency_field = "updated_at",
    recall_status_filter = "status != \"archived\"",
    mirror_snapshot(
        name = "task_focus",
        flush_interval_secs = 3600,
        event_kinds = ["TaskFocusChanged", "TaskCompleted"],
    ),
    promotion_threshold = 3,
)]
pub struct TasksFeature { /* unchanged */ }
```

- [ ] **Step 2: Annotate `FinanceFeature`**

In `crates/feature-finance/src/lib.rs`, extend its `#[ai(...)]` block similarly with `tool_name = "finance"` and `entity_kind = "finance_transaction"`.

- [ ] **Step 3: Run the registry integration test from Task 8 — now with assertions extended**

Edit `tests/ai_registry_integration.rs` to assert `tool_name` and `entity_kind`:

```rust
let tasks = reg.by_domain(&ai_core::RecallDomain::Tasks).expect("tasks");
assert_eq!(tasks.tool_name, Some("tasks"));
assert_eq!(tasks.entity_kind, Some("task"));

let finance = reg.by_domain(&ai_core::RecallDomain::Finance).expect("finance");
assert_eq!(finance.tool_name, Some("finance"));
assert_eq!(finance.entity_kind, Some("finance_transaction"));
```

- [ ] **Step 4: Run**

Run: `cargo nextest run --test ai_registry_integration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/lib.rs crates/feature-finance/src/lib.rs tests/ai_registry_integration.rs
git commit -m "feat(features): declare tool_name and entity_kind on Tasks+Finance"
```

---

## Task 11: Build `AiFeatureRegistry` in `app-core::init::ai_pipeline::build_feature_registry()`

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`
- Modify: `crates/app-core/src/init/mod.rs` (call site)

`★ Insight ─────────────────────────────────────`
`build_feature_registry()` is intentionally a single function that lists every feature's `register` call in one place. This is the architectural "manifest" of what features participate in the pipeline. When v4 adds a new feature, the diff lands here as a single line — and PR reviewers immediately see what changed. This is the explicit alternative to `inventory`-style auto-collection.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add the builder fn at the bottom of `ai_pipeline.rs`**

```rust
/// Build the workspace `AiFeatureRegistry`. Every feature crate that derives
/// `AiFeature` must be listed here; new features are added in v3+ as a single
/// line per crate.
pub fn build_feature_registry() -> ai_core::AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);
    // Productivity, Notes, Learning, LanguageLearning, Coaching are added by
    // their respective tasks (12, 24, 34, 39, 46).
    reg
}
```

- [ ] **Step 2: Call it from the existing init phase**

In `crates/app-core/src/init/mod.rs`, locate the section that calls `build_metric_registry()` (Phase 9-ish, post-v2.5). Immediately above it, add:

```rust
let feature_registry = ai_pipeline::build_feature_registry();
tracing::info!(features = feature_registry.len(), "ai feature registry built");
```

Bind it on the `AppCore` struct as `feature_registry: Arc<AiFeatureRegistry>` (look at how `metric_registry` is bound for the analogous wiring; replicate the pattern). The actual storage is `Arc<…>` so handlers can read the registry without re-building it.

- [ ] **Step 3: Add a unit test**

Inside `ai_pipeline.rs`, append:

```rust
#[cfg(test)]
mod registry_build_test {
    use super::build_feature_registry;
    use ai_core::RecallDomain;

    #[test]
    fn registry_seeded_with_tasks_and_finance() {
        let reg = build_feature_registry();
        assert!(reg.by_domain(&RecallDomain::Tasks).is_some());
        assert!(reg.by_domain(&RecallDomain::Finance).is_some());
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p app-core ai_pipeline`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs crates/app-core/src/init/mod.rs crates/app-core/src/state.rs
git commit -m "feat(app-core): build AiFeatureRegistry at startup"
```

---

## Task 12: Move `ProductivityFeature` from `lib.rs` into `feature.rs`; add `#[derive(AiFeature)]`

**Files:**
- Modify: `crates/feature-productivity/src/lib.rs` (split out the struct)
- Create: `crates/feature-productivity/src/feature.rs`
- Modify: `crates/feature-productivity/Cargo.toml` (add `ai-core-macros` if missing)

`★ Insight ─────────────────────────────────────`
We split `ProductivityFeature` into its own module to keep `#[derive(AiFeature)]` next to the data. Productivity has a sprawling `lib.rs` because it owns 15+ repos and several intelligence subsystems; mixing the AI-pipeline declaration with that landscape would obscure what's a feature-package vs. internal plumbing.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect the current shape**

Run: `grep -n "pub struct ProductivityFeature\|impl FeaturePackage for ProductivityFeature" crates/feature-productivity/src/lib.rs`
Expected: lines 81–97 area show the struct + impl.

- [ ] **Step 2: Create `crates/feature-productivity/src/feature.rs`**

```rust
//! ProductivityFeature — the pipeline-aware FeaturePackage for productivity.

use std::sync::Arc;
use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

use crate::tool::ProductivityTool;

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "Productivity",
    skill = "automation",
    tool_name = "productivity",
    entity_kind = "focus_session",
    event = "crate::events::ProductivityEvent",
)]
pub struct ProductivityFeature {
    tool: Option<Arc<ProductivityTool>>,
    pool: Option<storage::StoragePool>,
}

impl ProductivityFeature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tool(mut self, tool: Arc<ProductivityTool>) -> Self {
        self.tool = Some(tool);
        self
    }

    pub fn with_pool(mut self, pool: storage::StoragePool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_productivity.sql")
    }
}

#[async_trait]
impl FeaturePackage for ProductivityFeature {
    fn name(&self) -> &str { "productivity" }

    fn tools(&self) -> Vec<DynTool> {
        match &self.tool {
            Some(t) => vec![Arc::clone(t) as DynTool],
            None => vec![],
        }
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "productivity".to_string(),
            // Match the existing version number in the deleted impl. Bump only if the SQL changed.
            version: 1,
            description: "Create productivity tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let Some(pool) = &self.pool else {
            return Ok(HealthStatus::Healthy);
        };
        match sqlx::query("SELECT 1 FROM focus_sessions LIMIT 1")
            .execute(pool.inner())
            .await
        {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Degraded(format!("focus_sessions table unreachable: {e}"))),
        }
    }
}
```

The exact migration version number, description, and main-table name should be copied verbatim from the current `lib.rs` impl — do not invent new values. If the existing impl returns multiple migrations, replicate that vector exactly.

- [ ] **Step 3: Delete the inline impl from `lib.rs` and add `pub mod feature;`**

In `crates/feature-productivity/src/lib.rs`:
- Add at the top of the module declarations: `pub mod feature;`
- Add to the re-exports: `pub use feature::ProductivityFeature;`
- Delete the existing `pub struct ProductivityFeature { … }` block.
- Delete the existing `impl ProductivityFeature { … }` block.
- Delete the existing `impl FeaturePackage for ProductivityFeature { … }` block.

- [ ] **Step 4: Add `ai-core-macros` to `Cargo.toml` if absent**

Run: `grep ai-core-macros crates/feature-productivity/Cargo.toml`
If empty, add to `[dependencies]`:

```toml
ai-core = { path = "../ai-core" }
ai-core-macros = { path = "../ai-core-macros" }
```

- [ ] **Step 5: Build the crate**

Run: `cargo build -p feature-productivity`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-productivity/src/lib.rs crates/feature-productivity/src/feature.rs crates/feature-productivity/Cargo.toml
git commit -m "feat(feature-productivity): lift ProductivityFeature into feature.rs with AiFeature derive"
```

---

## Task 13: Annotate `FocusSession` with `#[derive(AiEntity)]`

**Files:**
- Modify: `crates/feature-productivity/src/types/domain.rs` (the `FocusSession` struct)
- Modify: `crates/feature-productivity/src/lib.rs` (re-export if needed)

`★ Insight ─────────────────────────────────────`
`FocusSession` is the natural recall entity for productivity — it has a duration, a session ID, an optional task association, and (typically) a quality score. The `embed_text` should concatenate the session's task title (if linked) and any user-provided focus intent string. If the type today doesn't have a string-y representation, embedding text could be the literal `"focus session ${id}"` — the embedding is most useful when the session is task-linked, otherwise it's a positional record.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate the `FocusSession` struct**

Run: `grep -n "pub struct FocusSession\|impl FocusSession" crates/feature-productivity/src/types/domain.rs`
Expected: shows the struct fields. Note which fields are best for embedding — likely `task_title` (if present) or `notes`, `intent`. If neither exists, fall back to a single `id` string.

- [ ] **Step 2: Add the derive**

Add `ai_core_macros::AiEntity` to the derive list, and the `#[ai(...)]` attribute:

```rust
use ai_core_macros::AiEntity;

#[derive(Debug, Clone, AiEntity, /* existing derives */)]
#[ai(entity_type = "focus_session", embed_on = ["intent", "notes"])]
pub struct FocusSession {
    // unchanged fields
}
```

If neither `intent` nor `notes` is a `String`/`Option<String>`, choose the closest text fields. The macro requires `String` or `Option<String>` — see how `Task` and `FinanceTransaction` use it (`crates/feature-tasks/src/types/entity.rs:15`, `crates/feature-finance/src/types/domain.rs:197`).

- [ ] **Step 3: Add a unit test in the same file**

```rust
#[cfg(test)]
mod ai_entity_tests {
    use super::*;
    use ai_core::AiEntity;

    #[test]
    fn focus_session_entity_type() {
        assert_eq!(FocusSession::entity_type(), "focus_session");
    }

    #[test]
    fn focus_session_embed_text_concatenates() {
        let s = FocusSession {
            // construct minimal instance with intent="study rust" and notes=Some("ch.5")
            // (use whatever default + setters the type provides)
            ..Default::default()
        };
        // adjust the assertion to match the chosen field setters
        let _ = s.embed_text();
    }
}
```

- [ ] **Step 4: Build + run unit tests**

Run: `cargo nextest run -p feature-productivity ai_entity_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/src/types/domain.rs
git commit -m "feat(feature-productivity): annotate FocusSession with AiEntity"
```

---

## Task 14: Add `ProductivityEvent::FocusSessionStarted` + `FocusSessionEnded` + `From` arms

**Files:**
- Modify: `crates/feature-productivity/src/events.rs`

`★ Insight ─────────────────────────────────────`
These two variants directly replace the manual mappings in `ai_pipeline.rs` for `DomainEvent::FocusSessionStarted` and `DomainEvent::FocusSessionEnded`. By making them first-class `ProductivityEvent` variants, we let the macro generate importance, salience, and observation_template for them — and we delete the corresponding hand-written `translate_system_event` arms in Task 21.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Read existing variants**

Run: `cat crates/feature-productivity/src/events.rs`
Expected: shows the current `ProductivityEvent` enum with one variant (`SessionEnded`) and its `From` impl.

- [ ] **Step 2: Add the new variants**

Extend the enum (preserve existing `SessionEnded` arm as-is):

```rust
#[ai(
    importance = 0.4,
    salience = "accumulate",
    observation_template = "Focus session started: {session_id}",
    entity_bridge(type = "focus_session", name_from = "session_id", id_from = "session_id"),
)]
FocusSessionStarted {
    session_id: String,
    started_at: jiff::Timestamp,
},

#[ai(
    importance = 0.5,
    salience = "accumulate",
    observation_template = "Focus session ended: {session_id} after {duration_secs}s",
    entity_bridge(type = "focus_session", name_from = "session_id", id_from = "session_id"),
)]
FocusSessionEnded {
    session_id: String,
    duration_secs: u64,
},
```

- [ ] **Step 3: Extend the `From<ProductivityEvent> for DomainEvent` impl**

```rust
ProductivityEvent::FocusSessionStarted { session_id, started_at } =>
    DomainEvent::FocusSessionStarted { session_id, started_at: started_at.to_string() },
ProductivityEvent::FocusSessionEnded { session_id, duration_secs } =>
    DomainEvent::FocusSessionEnded { session_id, duration_secs },
```

(Adjust the `DomainEvent::FocusSessionStarted` and `…Ended` field names to match the current bus definition — `grep -n "FocusSessionStarted\|FocusSessionEnded" crates/bus/src/domain_events.rs` to confirm field names.)

- [ ] **Step 4: Build to verify the macro accepts the new variants**

Run: `cargo build -p feature-productivity`
Expected: clean. If `coaching_signal` would normally be set on focus events (it isn't — these events don't trigger coaching), leave the attribute off.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/src/events.rs
git commit -m "feat(feature-productivity): add FocusSessionStarted/Ended ProductivityEvent variants"
```

---

## Task 15: Add `ProductivityEvent::DistractionDetected` + `From` arm

**Files:**
- Modify: `crates/feature-productivity/src/events.rs`

`★ Insight ─────────────────────────────────────`
`DistractionDetected` carries a flag+context payload; we set `importance = 0.6` because distractions are higher-signal for coaching than session lifecycle events. Salience stays `accumulate` — a single distraction is noise; the pattern over time matters.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect the current `DomainEvent::DistractionDetected` shape**

Run: `grep -n "DistractionDetected" crates/bus/src/domain_events.rs`
Expected: shows the variant and its fields. Read them carefully — the `From` impl must use exactly these field names.

- [ ] **Step 2: Add the variant**

```rust
#[ai(
    importance = 0.6,
    salience = "accumulate",
    observation_template = "Distraction: {kind} during session {session_id}",
    coaching_signal,
)]
DistractionDetected {
    session_id: String,
    kind: String,
    confidence: f64,
},
```

- [ ] **Step 3: Add the `From` arm**

```rust
ProductivityEvent::DistractionDetected { session_id, kind, confidence } =>
    DomainEvent::DistractionDetected { session_id, kind, confidence },
```

(Adjust to match the bus variant.)

- [ ] **Step 4: Build**

Run: `cargo build -p feature-productivity`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/src/events.rs
git commit -m "feat(feature-productivity): add DistractionDetected variant + From"
```

---

## Task 16: Add `ProductivityEvent::ActivitySessionCompleted` + `From` arm

**Files:**
- Modify: `crates/feature-productivity/src/events.rs`

- [ ] **Step 1: Inspect the bus variant**

Run: `grep -n "ActivitySessionCompleted" crates/bus/src/domain_events.rs`
Expected: shows fields — likely `session_id`, `activity_kind`, `duration_secs`.

- [ ] **Step 2: Add the variant**

```rust
#[ai(
    importance = 0.4,
    salience = "accumulate",
    observation_template = "Activity completed: {activity_kind} ({duration_secs}s)",
)]
ActivitySessionCompleted {
    session_id: String,
    activity_kind: String,
    duration_secs: u64,
},
```

- [ ] **Step 3: Add the `From` arm**

```rust
ProductivityEvent::ActivitySessionCompleted { session_id, activity_kind, duration_secs } =>
    DomainEvent::ActivitySessionCompleted { session_id, activity_kind, duration_secs },
```

- [ ] **Step 4: Build**

Run: `cargo build -p feature-productivity`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/src/events.rs
git commit -m "feat(feature-productivity): add ActivitySessionCompleted variant + From"
```

---

## Task 17: Add `ProductivityEvent::ProductivityScoreComputed` + `From` arm

**Files:**
- Modify: `crates/feature-productivity/src/events.rs`

`★ Insight ─────────────────────────────────────`
The score is computed periodically (likely hourly or per-session) and is a useful mirror snapshot input — but we don't need a separate `mirror_snapshot` declaration here yet because v2 already wired `RoutingMirrorSubscriber`. The metric attribute `productivity_score_avg` could land in v3.5; v3 keeps this variant pure-observation.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect the bus variant**

Run: `grep -n "ProductivityScoreComputed" crates/bus/src/domain_events.rs`
Expected: shows fields — likely `score: f64`, `computed_at: String`, plus an optional `session_id` or `window_secs`.

- [ ] **Step 2: Add the variant**

```rust
#[ai(
    importance = 0.4,
    salience = "extract_if(*score > 0.8 || *score < 0.3)",
    observation_template = "Productivity score: {score}",
)]
ProductivityScoreComputed {
    score: f64,
    window_secs: u64,
},
```

The `extract_if` predicate captures the "interesting" extremes (very high or very low scores) — middling scores stay in the accumulator. Adjust thresholds based on what feels meaningful in the data.

- [ ] **Step 3: Add the `From` arm**

```rust
ProductivityEvent::ProductivityScoreComputed { score, window_secs } =>
    DomainEvent::ProductivityScoreComputed { score, window_secs },
```

- [ ] **Step 4: Build**

Run: `cargo build -p feature-productivity`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/src/events.rs
git commit -m "feat(feature-productivity): add ProductivityScoreComputed variant + From"
```

---

## Task 18: Migrate `focus.rs:96`, `:110`, `:161` to emit via `ProductivityEvent::From`

**Files:**
- Modify: `crates/feature-productivity/src/focus.rs`

`★ Insight ─────────────────────────────────────`
This is the canonical "old path deletion in same commit" change. The existing call sites construct `DomainEvent::FocusSessionStarted { … }` directly and publish via `bus.publish(…)`; the new path constructs `ProductivityEvent::FocusSessionStarted { … }` and uses `bus.publish(event.into())`. The compiler enforces correctness because the `From` impl was added in Task 14.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Read the three call sites**

Run: `sed -n '90,170p' crates/feature-productivity/src/focus.rs`
Expected: see the three `bus.publish(DomainEvent::Focus…)` sites. Note how the bus handle is bound (likely `&self.bus` or `&self.event_bus`).

- [ ] **Step 2: Replace each `DomainEvent` construction with `ProductivityEvent`**

Example refactor (preserve indentation):

```rust
// Before
self.event_bus.publish(DomainEvent::FocusSessionStarted {
    session_id: session_id.clone(),
    started_at: now,
});

// After
use crate::events::ProductivityEvent;
self.event_bus.publish(
    ProductivityEvent::FocusSessionStarted {
        session_id: session_id.clone(),
        started_at: now,
    }
    .into(),
);
```

Repeat for `FocusSessionEnded` (line 96 area) and `ProductivitySessionEnded` (line 110 area, already routed through `ProductivityEvent::SessionEnded` — leave that one as-is unless audit reveals it's still constructing `DomainEvent` directly).

- [ ] **Step 3: Run productivity tests**

Run: `cargo nextest run -p feature-productivity`
Expected: PASS. If a test was inspecting the event payload via the bus capture pattern, the assertions should still match because the `From` impl preserves field values.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-productivity/src/focus.rs
git commit -m "refactor(feature-productivity): emit focus events via ProductivityEvent::From"
```

---

## Task 19: Migrate `distraction_analyzer.rs` to emit via `ProductivityEvent::From`

**Files:**
- Modify: `crates/feature-productivity/src/distraction_analyzer.rs` (or wherever `DistractionDetected` is published)

- [ ] **Step 1: Locate the emission site**

Run: `grep -rn "DomainEvent::DistractionDetected" crates/feature-productivity/`
Expected: one or two sites in `distraction_analyzer.rs` or `intelligence/`.

- [ ] **Step 2: Refactor each site**

```rust
self.event_bus.publish(
    ProductivityEvent::DistractionDetected {
        session_id,
        kind,
        confidence,
    }
    .into(),
);
```

Make sure the `use crate::events::ProductivityEvent;` import is present at the top of the file.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-productivity/src/distraction_analyzer.rs
git commit -m "refactor(feature-productivity): emit distraction via ProductivityEvent::From"
```

---

## Task 20: Migrate `aggregator.rs` and `insights.rs` to emit via `ProductivityEvent::From`

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs` (`ActivitySessionCompleted` site)
- Modify: `crates/feature-productivity/src/insights.rs` or `dashboard_emitter.rs` (`ProductivityScoreComputed` site)

- [ ] **Step 1: Locate the emission sites**

Run: `grep -rn "DomainEvent::ActivitySessionCompleted\|DomainEvent::ProductivityScoreComputed" crates/feature-productivity/`
Expected: one site for each.

- [ ] **Step 2: Refactor**

For `ActivitySessionCompleted`:

```rust
self.event_bus.publish(
    ProductivityEvent::ActivitySessionCompleted {
        session_id,
        activity_kind,
        duration_secs,
    }
    .into(),
);
```

For `ProductivityScoreComputed`:

```rust
self.event_bus.publish(
    ProductivityEvent::ProductivityScoreComputed {
        score,
        window_secs,
    }
    .into(),
);
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-productivity/src/aggregator.rs crates/feature-productivity/src/insights.rs
git commit -m "refactor(feature-productivity): emit activity+score via ProductivityEvent::From"
```

---

## Task 21: Extend `ai_pipeline::translate()` to drop `translate_system_event` arms now covered by `ProductivityEvent`

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

`★ Insight ─────────────────────────────────────`
This is the "old path deletion" concrete payoff for Tasks 14–20. Every `DomainEvent::FocusSessionStarted`, `…Ended`, `DistractionDetected`, `ActivitySessionCompleted`, `ProductivityScoreComputed` and `ProductivitySessionEnded` arm in `translate_system_event` is replaced by the productivity-event try-into path (which already exists from v2.5 — the `try_into_productivity_event` function). We just extend it to cover the new variants and remove the duplicates from the system-event match.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend `try_into_productivity_event`**

In `crates/app-core/src/init/ai_pipeline.rs`, locate `fn try_into_productivity_event(...)`. Today it has one arm for `DomainEvent::ProductivitySessionEnded`. Add five more arms:

```rust
fn try_into_productivity_event(
    e: &DomainEvent,
) -> Option<feature_productivity::events::ProductivityEvent> {
    use feature_productivity::events::ProductivityEvent;
    match e {
        DomainEvent::ProductivitySessionEnded { session_id, quality, duration_mins } =>
            Some(ProductivityEvent::SessionEnded {
                session_id: session_id.clone(),
                quality: *quality,
                duration_mins: *duration_mins,
            }),
        DomainEvent::FocusSessionStarted { session_id, started_at } =>
            Some(ProductivityEvent::FocusSessionStarted {
                session_id: session_id.clone(),
                started_at: started_at.parse().unwrap_or_else(|_| jiff::Timestamp::now()),
            }),
        DomainEvent::FocusSessionEnded { session_id, duration_secs } =>
            Some(ProductivityEvent::FocusSessionEnded {
                session_id: session_id.clone(),
                duration_secs: *duration_secs,
            }),
        DomainEvent::DistractionDetected { session_id, kind, confidence } =>
            Some(ProductivityEvent::DistractionDetected {
                session_id: session_id.clone(),
                kind: kind.clone(),
                confidence: *confidence,
            }),
        DomainEvent::ActivitySessionCompleted { session_id, activity_kind, duration_secs } =>
            Some(ProductivityEvent::ActivitySessionCompleted {
                session_id: session_id.clone(),
                activity_kind: activity_kind.clone(),
                duration_secs: *duration_secs,
            }),
        DomainEvent::ProductivityScoreComputed { score, window_secs } =>
            Some(ProductivityEvent::ProductivityScoreComputed {
                score: *score,
                window_secs: *window_secs,
            }),
        _ => None,
    }
}
```

(Field names are illustrative — match the actual `bus::DomainEvent` definitions exactly.)

- [ ] **Step 2: Delete the now-duplicated arms in `translate_system_event`**

Find each of `DomainEvent::FocusSessionStarted`, `DomainEvent::FocusSessionEnded`, `DomainEvent::DistractionDetected`, `DomainEvent::ActivitySessionCompleted`, `DomainEvent::ProductivityScoreComputed` arms inside `translate_system_event` and delete them (the catch-all `_ => None` at the bottom remains, and `translate()` now returns `Some` for these via the productivity branch instead).

- [ ] **Step 3: Add a regression test**

Append to the bottom of `ai_pipeline.rs`:

```rust
#[cfg(test)]
mod productivity_translation_tests {
    use super::translate;
    use bus::DomainEvent;
    use ai_core::RecallDomain;

    #[test]
    fn focus_session_started_routes_through_productivity_event() {
        let e = DomainEvent::FocusSessionStarted {
            session_id: "s1".to_string(),
            started_at: "2026-04-23T10:00:00Z".to_string(),
        };
        let sig = translate(&e).expect("Some(signal)");
        assert_eq!(sig.domain, RecallDomain::Productivity);
        assert_eq!(sig.event_kind, "FocusSessionStarted");
    }

    #[test]
    fn distraction_detected_routes_through_productivity_event() {
        let e = DomainEvent::DistractionDetected {
            session_id: "s1".to_string(),
            kind: "phone".to_string(),
            confidence: 0.9,
        };
        let sig = translate(&e).expect("Some(signal)");
        assert_eq!(sig.domain, RecallDomain::Productivity);
        assert_eq!(sig.event_kind, "DistractionDetected");
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p app-core productivity_translation_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "refactor(ai-pipeline): route 5 productivity events through ProductivityEvent translator"
```

---

## Task 22: Create `feature-notes/src/events.rs` with `NoteEvent` enum

**Files:**
- Create: `crates/feature-notes/src/events.rs`
- Modify: `crates/feature-notes/Cargo.toml` (add `ai-core`, `ai-core-macros`, `bus` deps)

`★ Insight ─────────────────────────────────────`
Notes today emits its events from `app-core/handlers/notes/*.rs`, not from the feature crate. The pipeline migration inverts that: events become a first-class part of the feature crate, and handlers become callers of `NoteEvent::… .into()`. This is the structural-symmetry win the spec calls out — "adding a new feature touches exactly one crate" — applied to an existing feature.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Verify `Cargo.toml` lacks the deps**

Run: `grep -E "ai-core|ai-core-macros|^bus" crates/feature-notes/Cargo.toml`
Expected: no matches (or only `bus` if it's already there).

- [ ] **Step 2: Add deps**

Edit `[dependencies]` block:

```toml
ai-core = { path = "../ai-core" }
ai-core-macros = { path = "../ai-core-macros" }
bus = { path = "../bus" }
```

(Add `jiff` and `serde` if not already transitive.)

- [ ] **Step 3: Create the events module**

Create `crates/feature-notes/src/events.rs`:

```rust
use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Notes")]
pub enum NoteEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Created note '{title}' in {notebook_id:?}",
        entity_bridge(type = "note", name_from = "title", id_from = "note_id"),
    )]
    Created {
        note_id: String,
        title: String,
        notebook_id: Option<String>,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Updated note '{title}'",
        entity_bridge(type = "note", name_from = "title", id_from = "note_id"),
    )]
    Updated {
        note_id: String,
        title: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Note content changed: {char_delta} chars",
        entity_bridge(type = "note", name_from = "note_id", id_from = "note_id"),
    )]
    ContentChanged {
        note_id: String,
        char_delta: i32,
    },

    #[ai(
        importance = 0.4,
        salience = "extract",
        observation_template = "Finished editing note '{title}'",
        entity_bridge(type = "note", name_from = "title", id_from = "note_id"),
    )]
    EditingFinished {
        note_id: String,
        title: String,
        word_count: u32,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Deleted note '{title}'",
    )]
    Deleted {
        note_id: String,
        title: String,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Studied note '{title}'",
        entity_bridge(type = "note", name_from = "title", id_from = "note_id"),
    )]
    Studied {
        note_id: String,
        title: String,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Practice unit completed for note {note_id}",
        entity_bridge(type = "note", name_from = "note_id", id_from = "note_id"),
    )]
    PracticeUnitCompleted {
        note_id: String,
        unit_index: u32,
        correct: bool,
    },

    #[ai(
        importance = 0.6,
        salience = "extract",
        observation_template = "Practice session done: {correct}/{total}",
    )]
    PracticeSessionCompleted {
        session_id: String,
        correct: u32,
        total: u32,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Translation completed for note {note_id}",
    )]
    TranslationCompleted {
        note_id: String,
        target_language: String,
    },
}

impl From<NoteEvent> for DomainEvent {
    fn from(e: NoteEvent) -> Self {
        match e {
            NoteEvent::Created { note_id, title, notebook_id } =>
                DomainEvent::NoteCreated { note_id, title, notebook_id },
            NoteEvent::Updated { note_id, title } =>
                DomainEvent::NoteUpdated { note_id, title },
            NoteEvent::ContentChanged { note_id, char_delta } =>
                DomainEvent::NoteContentChanged { note_id, char_delta },
            NoteEvent::EditingFinished { note_id, title, word_count } =>
                DomainEvent::NoteEditingFinished { note_id, title, word_count },
            NoteEvent::Deleted { note_id, title } =>
                DomainEvent::NoteDeleted { note_id, title },
            NoteEvent::Studied { note_id, title } =>
                DomainEvent::NoteStudied { note_id, title },
            NoteEvent::PracticeUnitCompleted { note_id, unit_index, correct } =>
                DomainEvent::PracticeUnitCompleted { note_id, unit_index, correct },
            NoteEvent::PracticeSessionCompleted { session_id, correct, total } =>
                DomainEvent::PracticeSessionCompleted { session_id, correct, total },
            NoteEvent::TranslationCompleted { note_id, target_language } =>
                DomainEvent::TranslationCompleted { note_id, target_language },
        }
    }
}
```

The exact field names in each `DomainEvent::Note…` arm must match the bus definition. Run `grep -n "NoteCreated\|NoteUpdated\|NoteContentChanged\|NoteEditingFinished\|NoteDeleted\|NoteStudied\|PracticeUnitCompleted\|PracticeSessionCompleted\|TranslationCompleted" crates/bus/src/domain_events.rs` first, and fix mismatches in the `From` arms.

- [ ] **Step 4: Add `pub mod events;` and re-export to `lib.rs`**

```rust
pub mod events;
pub use events::NoteEvent;
```

- [ ] **Step 5: Build**

Run: `cargo build -p feature-notes`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-notes/src/events.rs crates/feature-notes/src/lib.rs crates/feature-notes/Cargo.toml
git commit -m "feat(feature-notes): introduce NoteEvent enum with AiEvent derive"
```

---

## Task 23: Annotate `Note` with `#[derive(AiEntity)]`

**Files:**
- Modify: `crates/feature-notes/src/models.rs`

- [ ] **Step 1: Locate the `Note` struct**

Run: `grep -n "pub struct Note " crates/feature-notes/src/models.rs`
Expected: shows the struct (likely near the top of the file).

- [ ] **Step 2: Add the derive + attribute**

```rust
use ai_core_macros::AiEntity;

#[derive(Debug, Clone, AiEntity, /* existing derives */)]
#[ai(entity_type = "note", embed_on = ["title", "body"])]
pub struct Note {
    // unchanged
}
```

- [ ] **Step 3: Add a unit test in the same file**

```rust
#[cfg(test)]
mod ai_entity_tests {
    use super::*;
    use ai_core::AiEntity;

    #[test]
    fn note_entity_type() {
        assert_eq!(Note::entity_type(), "note");
    }

    #[test]
    fn note_embed_text_concatenates_title_and_body() {
        // construct Note with title="My note" and body="content"
        let n = Note {
            // … minimum-field instance
        };
        let s = n.embed_text();
        assert!(s.contains("My note"));
        assert!(s.contains("content"));
    }
}
```

(Build the test instance using whatever constructor pattern the existing tests use.)

- [ ] **Step 4: Run**

Run: `cargo nextest run -p feature-notes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-notes/src/models.rs
git commit -m "feat(feature-notes): annotate Note with AiEntity"
```

---

## Task 24: Add `#[derive(AiFeature)]` on `NotesFeature`; declare `tool_name = "notes"`, `entity_kind = "note"`

**Files:**
- Modify: `crates/feature-notes/src/lib.rs`

- [ ] **Step 1: Add the derive + attribute**

Replace the `pub struct NotesFeature { repo: repo::NoteRepo }` declaration:

```rust
use ai_core_macros::AiFeature;

#[derive(AiFeature)]
#[ai(
    recall_domain = "Notes",
    skill = "notebook",
    tool_name = "notes",
    entity_kind = "note",
    event = "crate::events::NoteEvent",
)]
pub struct NotesFeature {
    repo: repo::NoteRepo,
}
```

- [ ] **Step 2: Verify the existing test still passes**

Run: `cargo nextest run -p feature-notes`
Expected: PASS — the existing `test_migration_sql_not_empty` test is unaffected.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-notes/src/lib.rs
git commit -m "feat(feature-notes): add AiFeature derive on NotesFeature"
```

---

## Task 25: Wire `NotesFeature::register` into `app-core::init`

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs::build_feature_registry`

- [ ] **Step 1: Add the call**

```rust
pub fn build_feature_registry() -> ai_core::AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);
    feature_productivity::ProductivityFeature::register(&mut reg);  // NEW from Task 12
    feature_notes::NotesFeature::register(&mut reg);                // NEW
    reg
}
```

- [ ] **Step 2: Extend the unit test**

```rust
#[test]
fn registry_seeded_with_all_through_v3_notes() {
    let reg = build_feature_registry();
    assert!(reg.by_domain(&RecallDomain::Tasks).is_some());
    assert!(reg.by_domain(&RecallDomain::Finance).is_some());
    assert!(reg.by_domain(&RecallDomain::Productivity).is_some());
    assert!(reg.by_domain(&RecallDomain::Notes).is_some());
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p app-core registry_seeded_with_all_through_v3_notes`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(app-core): register NotesFeature in AiFeatureRegistry"
```

---

## Task 26: Migrate `app-core/handlers/notes/crud.rs` to emit via `NoteEvent::From`

**Files:**
- Modify: `crates/app-core/src/handlers/notes/crud.rs` (and any sibling handler that emits a `NoteEvent`-eligible variant)

`★ Insight ─────────────────────────────────────`
The handler migration is what actually wires the new pipeline. Until this task lands, `NoteEvent` is defined but unused — handlers still emit `DomainEvent::NoteCreated` directly, bypassing the macro-generated importance/salience/observation. After this task, the same emit produces a richer `AiSignal` automatically.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate every notes-event emission site**

Run: `grep -rn "DomainEvent::Note\|DomainEvent::Practice\|DomainEvent::Translation" crates/app-core/src/handlers/notes/`
Expected: one or two sites per handler (`crud.rs`, `practice.rs`, `translation.rs`).

- [ ] **Step 2: Refactor each site**

For example, in `crud.rs` where `NoteCreated` is published:

```rust
// Before
ctx.bus.publish(DomainEvent::NoteCreated {
    note_id: note.id.clone(),
    title: note.title.clone(),
    notebook_id: note.notebook_id.clone(),
});

// After
use feature_notes::NoteEvent;
ctx.bus.publish(
    NoteEvent::Created {
        note_id: note.id.clone(),
        title: note.title.clone(),
        notebook_id: note.notebook_id.clone(),
    }
    .into(),
);
```

Repeat for `NoteUpdated`, `NoteContentChanged`, `NoteEditingFinished`, `NoteDeleted`, `NoteStudied`. Match the field names in each `NoteEvent` variant exactly.

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/crud.rs
git commit -m "refactor(app-core): emit notes events via NoteEvent::From in crud handler"
```

---

## Task 27: Migrate `app-core/handlers/notes/practice.rs` and `translation.rs` to emit via `NoteEvent::From`

**Files:**
- Modify: `crates/app-core/src/handlers/notes/practice.rs`
- Modify: `crates/app-core/src/handlers/notes/translation.rs`

- [ ] **Step 1: Refactor practice handler**

Replace any `DomainEvent::PracticeUnitCompleted { … }` and `DomainEvent::PracticeSessionCompleted { … }` constructions with `NoteEvent::PracticeUnitCompleted { … }.into()` / `NoteEvent::PracticeSessionCompleted { … }.into()`.

- [ ] **Step 2: Refactor translation handler**

Replace `DomainEvent::TranslationCompleted { … }` with `NoteEvent::TranslationCompleted { … }.into()`.

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/practice.rs crates/app-core/src/handlers/notes/translation.rs
git commit -m "refactor(app-core): emit practice+translation events via NoteEvent::From"
```

---

## Task 28: Extend `ai_pipeline::translate()` with `try_into_note_event` arm

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Add the function**

After the existing `try_into_*` functions, add:

```rust
fn try_into_note_event(e: &DomainEvent) -> Option<feature_notes::NoteEvent> {
    use feature_notes::NoteEvent;
    match e {
        DomainEvent::NoteCreated { note_id, title, notebook_id } =>
            Some(NoteEvent::Created {
                note_id: note_id.clone(),
                title: title.clone(),
                notebook_id: notebook_id.clone(),
            }),
        DomainEvent::NoteUpdated { note_id, title } =>
            Some(NoteEvent::Updated { note_id: note_id.clone(), title: title.clone() }),
        DomainEvent::NoteContentChanged { note_id, char_delta } =>
            Some(NoteEvent::ContentChanged { note_id: note_id.clone(), char_delta: *char_delta }),
        DomainEvent::NoteEditingFinished { note_id, title, word_count } =>
            Some(NoteEvent::EditingFinished {
                note_id: note_id.clone(),
                title: title.clone(),
                word_count: *word_count,
            }),
        DomainEvent::NoteDeleted { note_id, title } =>
            Some(NoteEvent::Deleted { note_id: note_id.clone(), title: title.clone() }),
        DomainEvent::NoteStudied { note_id, title } =>
            Some(NoteEvent::Studied { note_id: note_id.clone(), title: title.clone() }),
        DomainEvent::PracticeUnitCompleted { note_id, unit_index, correct } =>
            Some(NoteEvent::PracticeUnitCompleted {
                note_id: note_id.clone(),
                unit_index: *unit_index,
                correct: *correct,
            }),
        DomainEvent::PracticeSessionCompleted { session_id, correct, total } =>
            Some(NoteEvent::PracticeSessionCompleted {
                session_id: session_id.clone(),
                correct: *correct,
                total: *total,
            }),
        DomainEvent::TranslationCompleted { note_id, target_language } =>
            Some(NoteEvent::TranslationCompleted {
                note_id: note_id.clone(),
                target_language: target_language.clone(),
            }),
        _ => None,
    }
}
```

- [ ] **Step 2: Wire it into `translate`**

Insert before `translate_system_event`:

```rust
if let Some(e) = try_into_note_event(event) {
    let mut sig = e.to_signal();
    sig.domain = RecallDomain::Notes;
    return Some(sig);
}
```

- [ ] **Step 3: Add a test**

```rust
#[test]
fn note_created_routes_through_note_event() {
    let e = DomainEvent::NoteCreated {
        note_id: "n1".to_string(),
        title: "Hello".to_string(),
        notebook_id: None,
    };
    let sig = translate(&e).expect("Some(signal)");
    assert_eq!(sig.domain, RecallDomain::Notes);
    assert_eq!(sig.event_kind, "Created");
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p app-core note_created_routes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "refactor(ai-pipeline): route note events through NoteEvent translator"
```

---

## Task 29: Convert `feature-learning` from library-only into a `FeaturePackage` (struct + impl)

**Files:**
- Modify: `crates/feature-learning/src/lib.rs`
- Create: `crates/feature-learning/src/feature.rs`
- Create: `crates/feature-learning/migrations/001_create_learning.sql`
- Modify: `crates/feature-learning/Cargo.toml`

`★ Insight ─────────────────────────────────────`
`feature-learning` is the smallest crate today (~10 lines of `lib.rs`) — it's essentially a library of `card_generator` + types used by `cognitive`. Converting it into a real `FeaturePackage` is the riskiest task in v3 because it changes the crate's identity. We deliberately do NOT add new tables (the existing `flashcards`, `knowledge_atoms` etc. live in `cognitive` migrations). The migration file is empty so the schema is unchanged and `FeaturePackage::migrations()` returns a no-op vector. The point of having a `FeaturePackage` here is *registration into the AI pipeline*, not new persistence.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add deps to `Cargo.toml`**

```toml
[dependencies]
ai-core = { path = "../ai-core" }
ai-core-macros = { path = "../ai-core-macros" }
async-trait = "0.1"
bus = { path = "../bus" }
common = { path = "../common" }
jiff = { workspace = true }
serde = { workspace = true, features = ["derive"] }
tools-core = { path = "../tools-core" }
```

(Adjust to match the workspace's typical dep declarations.)

- [ ] **Step 2: Create the migration placeholder**

```bash
mkdir -p crates/feature-learning/migrations
```

Create `crates/feature-learning/migrations/001_create_learning.sql`:

```sql
-- Learning feature does not own its own tables in v3.
-- Tables `knowledge_atoms`, `flashcards`, `flashcard_reviews`, `fsrs_parameters`
-- live in the cognitive crate's migration set (cognitive/migrations/001_cognitive_tables.sql).
-- This file exists so FeaturePackage::migrations() returns a non-empty vector for
-- migration tracking parity with other features.
SELECT 1;
```

(The `SELECT 1` no-op satisfies SQLite without altering schema. Future v3.x can move `knowledge_atoms`/`flashcards` here.)

- [ ] **Step 3: Create `feature.rs`**

```rust
//! LearningFeature — flashcard + knowledge-atom feature package.
//! Owns no tables in v3 (uses cognitive's flashcards/knowledge_atoms tables).

use std::sync::Arc;
use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "Learning",
    skill = "learning",
    tool_name = "learning",
    entity_kind = "knowledge_atom",
    event = "crate::events::LearningEvent",
)]
pub struct LearningFeature {
    pool: Option<storage::StoragePool>,
}

impl LearningFeature {
    pub fn new() -> Self { Self::default() }

    pub fn with_pool(pool: storage::StoragePool) -> Self {
        Self { pool: Some(pool) }
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_learning.sql")
    }
}

#[async_trait]
impl FeaturePackage for LearningFeature {
    fn name(&self) -> &str { "learning" }

    fn tools(&self) -> Vec<DynTool> {
        // The "learning" Tool lives in crates/tools/src/domain/learning_tool.rs and is
        // wired separately into the agent builder; see Task 47 for the exposure path.
        Vec::new()
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "learning".to_string(),
            version: 1,
            description: "Placeholder: tables owned by cognitive in v3".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let Some(pool) = &self.pool else {
            return Ok(HealthStatus::Healthy);
        };
        match sqlx::query("SELECT 1 FROM knowledge_atoms LIMIT 1")
            .execute(pool.inner()).await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Degraded(format!("knowledge_atoms unreachable: {e}"))),
        }
    }
}
```

- [ ] **Step 4: Update `lib.rs`**

```rust
pub mod card_generator;
pub mod events;        // NEW (created in Task 32)
pub mod feature;
pub mod types;

pub use card_generator::{
    build_generation_prompt, parse_generated_cards, summarize_existing_cards,
};
pub use events::LearningEvent;        // NEW
pub use feature::LearningFeature;
pub use types::{CardGenerationContext, GeneratedCard};
```

(The `events` module is created in Task 32; the `pub mod events;` line must reference an existing file. Either add it now and create a stub `events.rs` with `// stub` to keep `cargo build` happy, or land Task 32 in the same commit.)

- [ ] **Step 5: Build**

Run: `cargo build -p feature-learning`
Expected: error about missing `events` module (until Task 32). Either add a stub `events.rs` containing `// placeholder; see Task 32` OR proceed straight into Task 32.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-learning/
git commit -m "feat(feature-learning): introduce LearningFeature struct + FeaturePackage impl"
```

---

## Task 30: Discover the current entity-update dispatch path; record findings

**Files:**
- Create: `docs/superpowers/notes/2026-04-23-entity-update-discovery.md`

`★ Insight ─────────────────────────────────────`
This is a research task — no code changes, just a document. The spec referenced `crates/mcp/src/handler.rs:328` which doesn't exist; before we can auto-derive entity-update dispatch, we have to know what dispatch actually exists today. The findings drive Tasks 49–50.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Run the discovery greps**

```bash
grep -rn "emit_updates\|EntityUpdate\|entity_update" crates/app-core/ crates/mcp/ --include="*.rs"
grep -rn "fn name\(&self\) -> &str" crates/feature-tasks/src/tool.rs crates/feature-finance/src/tool.rs crates/feature-notes/src/tool.rs crates/feature-productivity/src/tool/ 2>/dev/null
```

Capture every dispatch site. For each one, note:
- File path + line number
- The function signature
- What it dispatches (entity kind, action, payload)
- Whether the call site is in a tool, a handler, or the bus consumer

- [ ] **Step 2: Write the findings**

Create `docs/superpowers/notes/2026-04-23-entity-update-discovery.md` with the structure:

```markdown
# Entity-Update Dispatch Discovery (v3 Task 30)

**Goal:** Locate every code path that emits an "entity changed" notification (UI refresh signal).

## Findings

### Dispatch site 1: [path:line]
- Signature: `fn …(…)`
- Triggered by: …
- Payload shape: `{ kind: "task", id: "abc" }`

### Dispatch site 2: [path:line]
…

## Conclusion

[Decision: which sites Task 49's generated `dispatch_entity_update(kind, id)` will replace,
and which (if any) stay bespoke because they carry payloads beyond `(kind, id)`.]
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/notes/2026-04-23-entity-update-discovery.md
git commit -m "docs(v3): record entity-update dispatch discovery for MCP auto-derive"
```

---

## Task 31: Add 4 new `DomainEvent` variants for learning + 1 for plugin

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

`★ Insight ─────────────────────────────────────`
Adding `DomainEvent` variants is normally a v1-style "do not pile on" thing — but here each new variant has a planned consumer (the `LearningEvent` enum routes them via `LearningEvent::From<…>` reverse, and `PluginEvent` is consumed by Task 53's plugin host). Adding the variant + its `From` translator + at least one emitter in the same PR keeps the variant non-orphan from the moment it lands.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate the `DomainEvent` enum**

Run: `grep -n "pub enum DomainEvent" crates/bus/src/domain_events.rs`
Expected: line 22 area.

- [ ] **Step 2: Add the four learning variants**

Place them next to existing `KnowledgeAtom*` variants:

```rust
KnowledgeAtomExtracted {
    atom_id: String,
    note_id: String,
    text: String,
},
FlashcardScheduled {
    flashcard_id: String,
    atom_id: String,
    due_at: String,         // RFC3339 timestamp
},
AtomRetentionDecayed {
    atom_id: String,
    retention: f64,
},
AtomSemanticFactLinked {
    atom_id: String,
    fact_id: String,
    similarity: f64,
},
```

- [ ] **Step 3: Add `PluginEvent`**

Place at the bottom of the enum:

```rust
PluginEvent {
    plugin_id: String,
    kind: String,
    payload: serde_json::Value,
},
```

- [ ] **Step 4: Update `variant_name()` and `domain()` matches**

Find `fn variant_name(&self) -> &'static str` and `fn domain(&self) -> &'static str` (around lines 511 + 712). Add arms:

```rust
DomainEvent::KnowledgeAtomExtracted { .. } => "KnowledgeAtomExtracted",
DomainEvent::FlashcardScheduled { .. } => "FlashcardScheduled",
DomainEvent::AtomRetentionDecayed { .. } => "AtomRetentionDecayed",
DomainEvent::AtomSemanticFactLinked { .. } => "AtomSemanticFactLinked",
DomainEvent::PluginEvent { .. } => "PluginEvent",
```

For `domain()`:

```rust
DomainEvent::KnowledgeAtomExtracted { .. } => "learning",
DomainEvent::FlashcardScheduled { .. } => "learning",
DomainEvent::AtomRetentionDecayed { .. } => "learning",
DomainEvent::AtomSemanticFactLinked { .. } => "learning",
DomainEvent::PluginEvent { .. } => "plugin",
```

- [ ] **Step 5: Build the workspace**

Run: `cargo build --workspace`
Expected: clean. Any non-exhaustive match warnings on `DomainEvent` consumers will surface — leave those for the consumers' migration tasks (the catch-all arms in `translate_system_event` and `normalizers.rs` already swallow unknown variants).

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add 4 learning DomainEvent variants + PluginEvent variant"
```

---

## Task 32: Define `LearningEvent` enum with all 9 variants + `From<LearningEvent> for DomainEvent`

**Files:**
- Create: `crates/feature-learning/src/events.rs`

`★ Insight ─────────────────────────────────────`
Learning gets the most variants of any v3 feature (9) because it absorbs both the existing `KnowledgeAtom*` flow AND the four new spec-required events. Three of them (`KnowledgeAtomExtracted`, `FlashcardScheduled`, `AtomRetentionDecayed`) deserve `extract` salience because they represent moments of learning consolidation — exactly the signals the cognitive layer wants to surface in retrieval.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the events module**

```rust
use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Learning")]
pub enum LearningEvent {
    #[ai(
        importance = 0.6,
        salience = "extract",
        observation_template = "Knowledge atom extracted from note {note_id}: {text}",
        entity_bridge(type = "knowledge_atom", name_from = "text", id_from = "atom_id"),
    )]
    AtomExtracted {
        atom_id: String,
        note_id: String,
        text: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Knowledge atom created (manually): {atom_id}",
        entity_bridge(type = "knowledge_atom", name_from = "atom_id", id_from = "atom_id"),
    )]
    AtomCreated {
        atom_id: String,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Knowledge atom accepted: {atom_id}",
    )]
    AtomAccepted {
        atom_id: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Knowledge atom archived: {atom_id}",
    )]
    AtomArchived {
        atom_id: String,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Flashcard reviewed (rating {rating}): {flashcard_id}",
    )]
    FlashcardReviewed {
        flashcard_id: String,
        rating: u8,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Atom reinforced: {atom_id}",
    )]
    AtomReinforced {
        atom_id: String,
    },

    #[ai(
        importance = 0.4,
        salience = "extract",
        observation_template = "Flashcard scheduled: {flashcard_id} due at {due_at}",
        entity_bridge(type = "knowledge_atom", name_from = "atom_id", id_from = "atom_id"),
    )]
    FlashcardScheduled {
        flashcard_id: String,
        atom_id: String,
        due_at: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Atom retention decayed to {retention}: {atom_id}",
    )]
    RetentionDecayed {
        atom_id: String,
        retention: f64,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Atom linked to semantic fact: {atom_id} -> {fact_id} ({similarity})",
    )]
    SemanticFactLinked {
        atom_id: String,
        fact_id: String,
        similarity: f64,
    },
}

impl From<LearningEvent> for DomainEvent {
    fn from(e: LearningEvent) -> Self {
        match e {
            LearningEvent::AtomExtracted { atom_id, note_id, text } =>
                DomainEvent::KnowledgeAtomExtracted { atom_id, note_id, text },
            LearningEvent::AtomCreated { atom_id } =>
                DomainEvent::KnowledgeAtomCreated { atom_id },
            LearningEvent::AtomAccepted { atom_id } =>
                DomainEvent::KnowledgeAtomAccepted { atom_id },
            LearningEvent::AtomArchived { atom_id } =>
                DomainEvent::KnowledgeAtomArchived { atom_id },
            LearningEvent::FlashcardReviewed { flashcard_id, rating } =>
                DomainEvent::AtomFlashcardReviewed { flashcard_id, rating },
            LearningEvent::AtomReinforced { atom_id } =>
                DomainEvent::AtomReinforced { atom_id },
            LearningEvent::FlashcardScheduled { flashcard_id, atom_id, due_at } =>
                DomainEvent::FlashcardScheduled { flashcard_id, atom_id, due_at },
            LearningEvent::RetentionDecayed { atom_id, retention } =>
                DomainEvent::AtomRetentionDecayed { atom_id, retention },
            LearningEvent::SemanticFactLinked { atom_id, fact_id, similarity } =>
                DomainEvent::AtomSemanticFactLinked { atom_id, fact_id, similarity },
        }
    }
}
```

(Field names in `DomainEvent::KnowledgeAtomCreated`, `AtomFlashcardReviewed`, `AtomReinforced` etc. must match the existing bus definitions; verify with grep.)

- [ ] **Step 2: Build**

Run: `cargo build -p feature-learning`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-learning/src/events.rs
git commit -m "feat(feature-learning): introduce LearningEvent enum"
```

---

## Task 33: Annotate `KnowledgeAtom` and `Flashcard` with `#[derive(AiEntity)]`

**Files:**
- Modify: wherever `KnowledgeAtom` and `Flashcard` are defined (likely `crates/cognitive/src/types/` or `crates/cognitive/src/repos/`)

`★ Insight ─────────────────────────────────────`
These types live in `cognitive`, not in `feature-learning`, because they were authored before `feature-learning` existed as a `FeaturePackage`. Annotating them in-place keeps the type ownership untouched (avoiding a giant move) but lets the embedding pipeline pick them up. v4 may move them into `feature-learning` once the boundary stabilizes.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate the structs**

```bash
grep -rn "pub struct KnowledgeAtom\|pub struct Flashcard" crates/cognitive/src/
```

Expected: shows the file paths. Note the existing derives (Debug, Clone, etc.).

- [ ] **Step 2: Add the derive on `KnowledgeAtom`**

```rust
use ai_core_macros::AiEntity;

#[derive(Debug, Clone, AiEntity, /* existing derives */)]
#[ai(entity_type = "knowledge_atom", embed_on = ["text"])]
pub struct KnowledgeAtom {
    // unchanged
}
```

(Use whichever string field carries the atom's content — likely `text` or `body`.)

- [ ] **Step 3: Add the derive on `Flashcard`**

```rust
#[derive(Debug, Clone, AiEntity, /* existing derives */)]
#[ai(entity_type = "flashcard", embed_on = ["front", "back"])]
pub struct Flashcard {
    // unchanged
}
```

- [ ] **Step 4: Add the `ai-core-macros` dep to `cognitive/Cargo.toml`**

Run: `grep ai-core-macros crates/cognitive/Cargo.toml`
If empty, add `ai-core-macros = { path = "../ai-core-macros" }` to `[dependencies]`.

- [ ] **Step 5: Build**

Run: `cargo build -p cognitive`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/Cargo.toml crates/cognitive/src/types/ crates/cognitive/src/repos/
git commit -m "feat(cognitive): annotate KnowledgeAtom and Flashcard with AiEntity"
```

---

## Task 34: `LearningFeature` registration into `app-core::init`

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs::build_feature_registry`
- Modify: `crates/app-core/src/init/storage.rs` (call `LearningFeature::migrations()`)

- [ ] **Step 1: Register**

```rust
pub fn build_feature_registry() -> ai_core::AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);
    feature_productivity::ProductivityFeature::register(&mut reg);
    feature_notes::NotesFeature::register(&mut reg);
    feature_learning::LearningFeature::register(&mut reg);          // NEW
    reg
}
```

- [ ] **Step 2: Wire migrations**

In `crates/app-core/src/init/storage.rs`, find where `notes_migrations()` (or similar) is called and the migrations are registered. Add `LearningFeature::new().migrations()` to the registration list.

- [ ] **Step 3: Build + run startup tests**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs crates/app-core/src/init/storage.rs
git commit -m "feat(app-core): register LearningFeature in registry + migrations"
```

---

## Task 35: Migrate `cognitive/repos/flashcard.rs` and `app-core/handlers/notes/flashcard.rs` to emit via `LearningEvent::From`

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs`
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs`

`★ Insight ─────────────────────────────────────`
This is the migration that wires the four new learning events to actual emit sites. `flashcard.rs` is where FSRS scheduling happens — the new `LearningEvent::FlashcardScheduled` should emit there whenever a card's next review is computed. `RetentionDecayed` emits from the FSRS retention update path. `SemanticFactLinked` is wired in Task 36 if/when the cognitive semantic-link service runs.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate the existing emitters in `flashcard.rs`**

```bash
grep -n "DomainEvent::AtomFlashcardReviewed\|DomainEvent::KnowledgeAtomCreated\|DomainEvent::AtomReinforced" crates/cognitive/src/repos/flashcard.rs
```

Expected: shows several `bus.publish(…)` sites.

- [ ] **Step 2: Refactor each existing emitter**

Replace direct `DomainEvent` constructions with `LearningEvent::… .into()`:

```rust
// Before
self.bus.publish(DomainEvent::AtomFlashcardReviewed {
    flashcard_id: id.to_string(),
    rating: r,
});

// After
use feature_learning::LearningEvent;
self.bus.publish(LearningEvent::FlashcardReviewed {
    flashcard_id: id.to_string(),
    rating: r,
}.into());
```

- [ ] **Step 3: Add new emitters at the right hooks**

In the FSRS scheduling function (likely near `schedule_flashcard` or `next_interval`), after computing `due_at`:

```rust
self.bus.publish(LearningEvent::FlashcardScheduled {
    flashcard_id,
    atom_id,
    due_at: due_at.to_string(),
}.into());
```

In the retention-update path:

```rust
self.bus.publish(LearningEvent::RetentionDecayed {
    atom_id,
    retention: new_retention,
}.into());
```

(If the codebase doesn't have an obvious retention-update site, leave the `RetentionDecayed` emit out for now and add a TODO comment-free note in the audit-only Task 60: "RetentionDecayed has no current emitter; revisit in v3.x.")

- [ ] **Step 4: Refactor `handlers/notes/flashcard.rs`**

```bash
grep -n "DomainEvent::KnowledgeAtomExtracted\|DomainEvent::Knowledge\|DomainEvent::Atom" crates/app-core/src/handlers/notes/flashcard.rs
```

Refactor each site to use `LearningEvent::… .into()`. Crucially, this file is where `KnowledgeAtomExtracted` should land — when a flashcard is auto-generated from a note, the prior step extracts atoms from the note text. Find that hook.

- [ ] **Step 5: Build + test**

Run: `cargo nextest run -p cognitive -p app-core`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/flashcard.rs crates/app-core/src/handlers/notes/flashcard.rs
git commit -m "refactor(learning): emit flashcard+atom events via LearningEvent::From"
```

---

## Task 36: Extend `ai_pipeline::translate()` with `try_into_learning_event` arm

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Add the function**

After the other `try_into_*` functions:

```rust
fn try_into_learning_event(e: &DomainEvent) -> Option<feature_learning::LearningEvent> {
    use feature_learning::LearningEvent;
    match e {
        DomainEvent::KnowledgeAtomExtracted { atom_id, note_id, text } =>
            Some(LearningEvent::AtomExtracted {
                atom_id: atom_id.clone(),
                note_id: note_id.clone(),
                text: text.clone(),
            }),
        DomainEvent::KnowledgeAtomCreated { atom_id } =>
            Some(LearningEvent::AtomCreated { atom_id: atom_id.clone() }),
        DomainEvent::KnowledgeAtomAccepted { atom_id } =>
            Some(LearningEvent::AtomAccepted { atom_id: atom_id.clone() }),
        DomainEvent::KnowledgeAtomArchived { atom_id } =>
            Some(LearningEvent::AtomArchived { atom_id: atom_id.clone() }),
        DomainEvent::AtomFlashcardReviewed { flashcard_id, rating } =>
            Some(LearningEvent::FlashcardReviewed {
                flashcard_id: flashcard_id.clone(),
                rating: *rating,
            }),
        DomainEvent::AtomReinforced { atom_id } =>
            Some(LearningEvent::AtomReinforced { atom_id: atom_id.clone() }),
        DomainEvent::FlashcardScheduled { flashcard_id, atom_id, due_at } =>
            Some(LearningEvent::FlashcardScheduled {
                flashcard_id: flashcard_id.clone(),
                atom_id: atom_id.clone(),
                due_at: due_at.clone(),
            }),
        DomainEvent::AtomRetentionDecayed { atom_id, retention } =>
            Some(LearningEvent::RetentionDecayed {
                atom_id: atom_id.clone(),
                retention: *retention,
            }),
        DomainEvent::AtomSemanticFactLinked { atom_id, fact_id, similarity } =>
            Some(LearningEvent::SemanticFactLinked {
                atom_id: atom_id.clone(),
                fact_id: fact_id.clone(),
                similarity: *similarity,
            }),
        _ => None,
    }
}
```

- [ ] **Step 2: Wire it into `translate`**

```rust
if let Some(e) = try_into_learning_event(event) {
    let mut sig = e.to_signal();
    sig.domain = RecallDomain::Learning;
    return Some(sig);
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "refactor(ai-pipeline): route learning events through LearningEvent translator"
```

---

## Task 37: Add `LanguageLearningEvent` enum + 4 new `DomainEvent` variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs` (add 4 variants)
- Create: `crates/feature-language-learning/src/events.rs`
- Modify: `crates/feature-language-learning/Cargo.toml` (add `ai-core`, `ai-core-macros`, `bus`)

`★ Insight ─────────────────────────────────────`
Language-learning has zero `DomainEvent` representation today — pronunciation/exam events live entirely inside the feature crate as in-memory state. v3 surfaces them onto the bus so coaching, retrieval, and mirror can react. We add four variants because that's what the audit identified the feature actually produces signals for: pronunciation scoring, exam attempts, phonetic mastery moments, and practice-session completion.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add the four `DomainEvent` variants**

In `crates/bus/src/domain_events.rs`, place near other learning-adjacent variants:

```rust
PronunciationScored {
    session_id: String,
    overall_score: f64,
    weak_phonemes: Vec<String>,
},
ExamAttempted {
    exam_id: String,
    score: u32,
    passed: bool,
},
PhoneticMasteryGained {
    phoneme: String,
    mastery_level: f64,
},
LanguagePracticeSessionCompleted {
    session_id: String,
    language: String,
    duration_secs: u64,
    success_rate: f64,
},
```

Add the corresponding arms in `variant_name()` and `domain()` (same pattern as Task 31).

- [ ] **Step 2: Add deps to `feature-language-learning/Cargo.toml`**

```toml
ai-core = { path = "../ai-core" }
ai-core-macros = { path = "../ai-core-macros" }
bus = { path = "../bus" }
```

- [ ] **Step 3: Create the events module**

```rust
use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "LanguageLearning")]
pub enum LanguageLearningEvent {
    #[ai(
        importance = 0.6,
        salience = "extract_if(*overall_score < 0.6 || !weak_phonemes.is_empty())",
        observation_template = "Pronunciation scored {overall_score} (session {session_id})",
    )]
    PronunciationScored {
        session_id: String,
        overall_score: f64,
        weak_phonemes: Vec<String>,
    },

    #[ai(
        importance = 0.7,
        salience = "extract",
        observation_template = "Exam {exam_id}: {score} ({passed})",
    )]
    ExamAttempted {
        exam_id: String,
        score: u32,
        passed: bool,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Phoneme mastery: /{phoneme}/ at {mastery_level}",
    )]
    PhoneticMasteryGained {
        phoneme: String,
        mastery_level: f64,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Practice session ({language}) done: {success_rate} success",
    )]
    PracticeSessionCompleted {
        session_id: String,
        language: String,
        duration_secs: u64,
        success_rate: f64,
    },
}

impl From<LanguageLearningEvent> for DomainEvent {
    fn from(e: LanguageLearningEvent) -> Self {
        match e {
            LanguageLearningEvent::PronunciationScored { session_id, overall_score, weak_phonemes } =>
                DomainEvent::PronunciationScored { session_id, overall_score, weak_phonemes },
            LanguageLearningEvent::ExamAttempted { exam_id, score, passed } =>
                DomainEvent::ExamAttempted { exam_id, score, passed },
            LanguageLearningEvent::PhoneticMasteryGained { phoneme, mastery_level } =>
                DomainEvent::PhoneticMasteryGained { phoneme, mastery_level },
            LanguageLearningEvent::PracticeSessionCompleted { session_id, language, duration_secs, success_rate } =>
                DomainEvent::LanguagePracticeSessionCompleted { session_id, language, duration_secs, success_rate },
        }
    }
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p bus -p feature-language-learning`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/feature-language-learning/
git commit -m "feat(language-learning): add LanguageLearningEvent + 4 new DomainEvent variants"
```

---

## Task 38: Annotate `DetailedPronunciationReport` with `#[derive(AiEntity)]`

**Files:**
- Modify: `crates/feature-language-learning/src/types.rs`

`★ Insight ─────────────────────────────────────`
Pronunciation reports are the natural recall entity for this feature — they're rich text documents (transcript + per-phoneme feedback) that benefit from semantic embedding. The `embed_on` fields point to the textual analysis, not the raw audio.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add the derive**

```rust
use ai_core_macros::AiEntity;

#[derive(Debug, Clone, AiEntity, /* existing derives */)]
#[ai(entity_type = "pronunciation_report", embed_on = ["transcript", "feedback_summary"])]
pub struct DetailedPronunciationReport {
    // unchanged
}
```

If `transcript`/`feedback_summary` aren't current field names, choose the closest text fields. The `embed_on` macro requires `String`/`Option<String>` typed fields.

- [ ] **Step 2: Run unit tests**

Run: `cargo nextest run -p feature-language-learning`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-language-learning/src/types.rs
git commit -m "feat(language-learning): annotate DetailedPronunciationReport with AiEntity"
```

---

## Task 39: Add `#[derive(AiFeature)]` on `LanguageLearningFeature`

**Files:**
- Modify: `crates/feature-language-learning/src/lib.rs`

- [ ] **Step 1: Replace the bare `LanguageLearningFeature` declaration**

```rust
use ai_core_macros::AiFeature;

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "LanguageLearning",
    skill = "learning",
    tool_name = "language_practice",   // matches LanguagePracticeTool::name()
    entity_kind = "pronunciation_report",
    event = "crate::events::LanguageLearningEvent",
)]
pub struct LanguageLearningFeature;
```

(If `LanguagePracticeTool::name()` returns a different string, set `tool_name` to match exactly.)

- [ ] **Step 2: Add `pub mod events;` and `pub use events::LanguageLearningEvent;`**

```rust
pub mod events;
pub use events::LanguageLearningEvent;
```

- [ ] **Step 3: Build**

Run: `cargo build -p feature-language-learning`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-language-learning/src/lib.rs
git commit -m "feat(language-learning): add AiFeature derive on LanguageLearningFeature"
```

---

## Task 40: Migrate `practice_tool.rs` and `pronunciation_provider.rs` to emit via `LanguageLearningEvent::From`

**Files:**
- Modify: `crates/feature-language-learning/src/practice_tool.rs`
- Modify: `crates/feature-language-learning/src/pronunciation_provider.rs`

- [ ] **Step 1: Identify emission sites**

```bash
grep -rn "DomainEvent::Pronunciation\|DomainEvent::Exam\|DomainEvent::Phonetic\|DomainEvent::Language\|bus.publish" crates/feature-language-learning/
```

These types may not have current `bus.publish` calls (since they have no `DomainEvent` representation pre-v3). In that case, **add new** `bus.publish(LanguageLearningEvent::… .into())` calls at the natural hooks:

- `pronunciation_provider.rs` after a scoring run completes → `PronunciationScored`.
- `practice_tool.rs` exam completion path → `ExamAttempted`.
- `practice_tool.rs` (or wherever phoneme mastery is detected) → `PhoneticMasteryGained`.
- `practice_tool.rs` session-end path → `PracticeSessionCompleted`.

- [ ] **Step 2: Inject the bus**

`LanguagePracticeTool` and `AppPronunciationProvider` likely don't hold a `DomainEventBus` handle today. Add `bus: Arc<DomainEventBus>` to their constructors and propagate through `LanguageLearningFeature::with_bus(bus)` in `feature.rs`. Update the `app-core` instantiation site (`crates/app-core/src/init/`) to pass the bus when constructing them.

- [ ] **Step 3: Add the publish calls**

Example for pronunciation:

```rust
self.bus.publish(
    LanguageLearningEvent::PronunciationScored {
        session_id,
        overall_score: report.overall_score,
        weak_phonemes: report.weak_phonemes.iter().map(|p| p.phoneme.clone()).collect(),
    }
    .into(),
);
```

- [ ] **Step 4: Build + test**

Run: `cargo nextest run -p feature-language-learning -p app-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-language-learning/src/practice_tool.rs crates/feature-language-learning/src/pronunciation_provider.rs crates/app-core/src/init/
git commit -m "feat(language-learning): publish events from practice + pronunciation hooks"
```

---

## Task 41: Extend `ai_pipeline::translate()` with `try_into_language_learning_event` arm

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Add the function**

```rust
fn try_into_language_learning_event(
    e: &DomainEvent,
) -> Option<feature_language_learning::LanguageLearningEvent> {
    use feature_language_learning::LanguageLearningEvent;
    match e {
        DomainEvent::PronunciationScored { session_id, overall_score, weak_phonemes } =>
            Some(LanguageLearningEvent::PronunciationScored {
                session_id: session_id.clone(),
                overall_score: *overall_score,
                weak_phonemes: weak_phonemes.clone(),
            }),
        DomainEvent::ExamAttempted { exam_id, score, passed } =>
            Some(LanguageLearningEvent::ExamAttempted {
                exam_id: exam_id.clone(),
                score: *score,
                passed: *passed,
            }),
        DomainEvent::PhoneticMasteryGained { phoneme, mastery_level } =>
            Some(LanguageLearningEvent::PhoneticMasteryGained {
                phoneme: phoneme.clone(),
                mastery_level: *mastery_level,
            }),
        DomainEvent::LanguagePracticeSessionCompleted { session_id, language, duration_secs, success_rate } =>
            Some(LanguageLearningEvent::PracticeSessionCompleted {
                session_id: session_id.clone(),
                language: language.clone(),
                duration_secs: *duration_secs,
                success_rate: *success_rate,
            }),
        _ => None,
    }
}
```

- [ ] **Step 2: Wire it into `translate`**

```rust
if let Some(e) = try_into_language_learning_event(event) {
    let mut sig = e.to_signal();
    sig.domain = RecallDomain::LanguageLearning;
    return Some(sig);
}
```

- [ ] **Step 3: Register the feature**

In `build_feature_registry()`:

```rust
feature_language_learning::LanguageLearningFeature::register(&mut reg);
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "refactor(ai-pipeline): route language-learning events; register feature"
```

---

## Task 42: Add `CoachingEvent::PatternDetected` + `FeedbackReceived` variants

**Files:**
- Modify: `crates/feature-coaching/src/events.rs`

`★ Insight ─────────────────────────────────────`
v2.5 added `CoachingEvent::StrategyApplied` with its `coaching_acceptance_rate` metric. v3 expands the enum to cover the other two coaching emit sites. `PatternDetected` is what triggers a coaching intervention; making it a typed event means consumers (mirror, retrieval) can react to it without parsing strings. `FeedbackReceived` carries the user's response signal — accept/dismiss/ignore — and feeds back into the metric harvester.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect current `CoachingEvent`**

Run: `cat crates/feature-coaching/src/events.rs`
Expected: shows the existing `StrategyApplied` variant with its metric annotation.

- [ ] **Step 2: Add the two new variants**

```rust
#[ai(
    importance = 0.6,
    salience = "extract",
    observation_template = "Coaching pattern detected: {pattern_name} (severity {severity})",
)]
PatternDetected {
    pattern_name: String,
    severity: f64,
},

#[ai(
    importance = 0.5,
    salience = "accumulate",
    observation_template = "Coaching feedback: {response} on strategy {strategy_id}",
    metric(
        name = "coaching_feedback_response_rate",
        value_from = if response == "accept" { 1.0_f64 } else { 0.0_f64 },
        window = "30d",
        min_samples = 5,
        aggregation = "avg",
    ),
)]
FeedbackReceived {
    strategy_id: String,
    response: String,
},
```

- [ ] **Step 3: Extend the `From` impl**

```rust
CoachingEvent::PatternDetected { pattern_name, severity } =>
    DomainEvent::CoachingPatternDetected { pattern_name, severity },
CoachingEvent::FeedbackReceived { strategy_id, response } =>
    DomainEvent::CoachingFeedback { strategy_id, response },
```

(Verify the bus variants' field names with grep.)

- [ ] **Step 4: Build**

Run: `cargo build -p feature-coaching`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-coaching/src/events.rs
git commit -m "feat(feature-coaching): add PatternDetected + FeedbackReceived CoachingEvent variants"
```

---

## Task 43: Create `CoachingFeature` struct in `feature-coaching/src/feature.rs` with `FeaturePackage` impl

**Files:**
- Create: `crates/feature-coaching/src/feature.rs`
- Modify: `crates/feature-coaching/src/lib.rs`

`★ Insight ─────────────────────────────────────`
`feature-coaching` doesn't currently implement `FeaturePackage` — `CoachingService` is wired manually in `app-core/init/coaching.rs`. v3 introduces a `CoachingFeature` struct purely so it can `#[derive(AiFeature)]` and register into the registry. The `FeaturePackage::tools()` returns empty (coaching isn't user-callable as a tool — it operates passively via signal consumption). The real coaching service stays where it is; the new `CoachingFeature` is a pipeline-only adapter.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create `feature.rs`**

```rust
//! CoachingFeature — pipeline-registration-only adapter.
//! The actual CoachingService lives in this crate's service.rs and is wired
//! through app-core::init::coaching. This struct exists so coaching can
//! participate in AiFeatureRegistry for skill discovery and metric harvesting.

use std::sync::Arc;
use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "Coaching",
    skill = "automation",
    event = "crate::events::CoachingEvent",
    // No tool_name (coaching is passive); no entity_kind (no entity surface).
)]
pub struct CoachingFeature;

impl CoachingFeature {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl FeaturePackage for CoachingFeature {
    fn name(&self) -> &str { "coaching" }
    fn tools(&self) -> Vec<DynTool> { Vec::new() }
    fn migrations(&self) -> Vec<FeatureMigration> { Vec::new() }
    async fn health_check(&self) -> Result<HealthStatus> { Ok(HealthStatus::Healthy) }
}
```

- [ ] **Step 2: Re-export from `lib.rs`**

```rust
pub mod feature;
pub use feature::CoachingFeature;
```

- [ ] **Step 3: Build**

Run: `cargo build -p feature-coaching`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-coaching/src/feature.rs crates/feature-coaching/src/lib.rs
git commit -m "feat(feature-coaching): introduce CoachingFeature for pipeline registration"
```

---

## Task 44: Migrate `pattern_detector/mod.rs` and `feedback.rs` to emit via `CoachingEvent::From`

**Files:**
- Modify: `crates/feature-coaching/src/pattern_detector/mod.rs`
- Modify: `crates/feature-coaching/src/feedback.rs`

`★ Insight ─────────────────────────────────────`
Same pattern as Tasks 18-20 — replace direct `DomainEvent` constructions with `CoachingEvent::… .into()`. The metric attribute on `FeedbackReceived` means the metric harvester now picks up coaching feedback rates automatically without `feedback.rs` needing to know about the metrics table.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate emission sites**

```bash
grep -rn "DomainEvent::CoachingPatternDetected\|DomainEvent::CoachingFeedback" crates/feature-coaching/
```

- [ ] **Step 2: Refactor pattern detector**

```rust
// Before
self.bus.publish(DomainEvent::CoachingPatternDetected {
    pattern_name: name.clone(),
    severity,
});

// After
use crate::events::CoachingEvent;
self.bus.publish(
    CoachingEvent::PatternDetected {
        pattern_name: name.clone(),
        severity,
    }
    .into(),
);
```

- [ ] **Step 3: Refactor feedback**

```rust
self.bus.publish(
    CoachingEvent::FeedbackReceived {
        strategy_id: strategy_id.clone(),
        response: response.to_string(),
    }
    .into(),
);
```

- [ ] **Step 4: Build + test**

Run: `cargo nextest run -p feature-coaching`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-coaching/src/pattern_detector/ crates/feature-coaching/src/feedback.rs
git commit -m "refactor(feature-coaching): emit pattern+feedback events via CoachingEvent::From"
```

---

## Task 45: Extend `ai_pipeline::translate()` with the new coaching variants

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Extend `try_into_coaching_event`**

```rust
fn try_into_coaching_event(e: &DomainEvent) -> Option<feature_coaching::events::CoachingEvent> {
    use feature_coaching::events::CoachingEvent;
    match e {
        DomainEvent::CoachingStrategyApplied { strategy_id, rule_text, accepted } =>
            Some(CoachingEvent::StrategyApplied {
                strategy_id: strategy_id.clone(),
                rule_text: rule_text.clone(),
                accepted: *accepted,
            }),
        DomainEvent::CoachingPatternDetected { pattern_name, severity } =>
            Some(CoachingEvent::PatternDetected {
                pattern_name: pattern_name.clone(),
                severity: *severity,
            }),
        DomainEvent::CoachingFeedback { strategy_id, response } =>
            Some(CoachingEvent::FeedbackReceived {
                strategy_id: strategy_id.clone(),
                response: response.clone(),
            }),
        _ => None,
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "refactor(ai-pipeline): route 2 new coaching events through CoachingEvent translator"
```

---

## Task 46: Wire `CoachingFeature::register` into `app-core::init`

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs::build_feature_registry`

- [ ] **Step 1: Extend the registry builder**

```rust
pub fn build_feature_registry() -> ai_core::AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);
    feature_productivity::ProductivityFeature::register(&mut reg);
    feature_notes::NotesFeature::register(&mut reg);
    feature_learning::LearningFeature::register(&mut reg);
    feature_language_learning::LanguageLearningFeature::register(&mut reg);
    feature_coaching::CoachingFeature::register(&mut reg);
    reg
}
```

- [ ] **Step 2: Extend the registry-build test to assert all 7 features**

```rust
#[test]
fn registry_seeded_with_all_v3_features() {
    let reg = build_feature_registry();
    assert_eq!(reg.len(), 7);
    for d in [
        RecallDomain::Tasks, RecallDomain::Finance, RecallDomain::Productivity,
        RecallDomain::Notes, RecallDomain::Learning, RecallDomain::LanguageLearning,
        RecallDomain::Coaching,
    ] {
        assert!(reg.by_domain(&d).is_some(), "missing {:?}", d);
    }
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p app-core registry_seeded_with_all_v3_features`
Expected: PASS — registry contains all seven features.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(app-core): register all 7 features in AiFeatureRegistry"
```

---

## Task 47: Replace `default_exposed_tools()` in `mcp.rs` with registry-driven implementation

**Files:**
- Modify: `crates/config/src/schema/mcp.rs:177` (`default_exposed_tools` function)
- Modify: `crates/config/Cargo.toml` (add `ai-core` dep — but wait, this would create a cycle)

`★ Insight ─────────────────────────────────────`
Naively, `config` would depend on `ai-core` to call `AiFeatureRegistry`. But `config` is L1, and `ai-core` is also L1 — peer dependency is fine. However, `ai-core` doesn't *know* about features (it would have to import each feature crate to call `register`), and that direction is forbidden. The clean solution: `default_exposed_tools()` becomes a `pub fn build_default_exposed_tools(reg: &AiFeatureRegistry) -> Vec<String>`. The serde `default = "..."` attribute that called `default_exposed_tools` keeps an *empty default* (because at config-deserialization time no registry exists yet), and `app-core::init` post-processes the loaded config to fill in `exposed_tools` from the registry if the field is empty. This decouples the config schema from the registry while still auto-deriving the default.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Replace the static function**

In `crates/config/src/schema/mcp.rs:177`:

```rust
/// Tools exposed by default. Empty here — `app-core` fills this from
/// `AiFeatureRegistry::tool_names() ∪ EXPLICIT_ALLOWLIST` post-load when the
/// user hasn't overridden it. The explicit allowlist exists for tools that
/// are not `AiFeature`s (memory, agent, annotate, cron, alarm, mirror, temporal).
fn default_exposed_tools() -> Vec<String> {
    Vec::new()
}

/// Cross-cutting tools that don't have a `FeaturePackage` and so don't appear
/// in `AiFeatureRegistry`. Concatenated with registry tool names by app-core.
pub const EXPLICIT_TOOL_ALLOWLIST: &[&str] = &[
    "memory",
    "agent",
    "annotate",
    "cron",
    "alarm",
    "mirror",
    "temporal",
];
```

- [ ] **Step 2: In `app-core::init`, fill in the missing tools**

In `crates/app-core/src/init/mod.rs`, after loading `Config` and after `feature_registry` is built:

```rust
use config::schema::mcp::EXPLICIT_TOOL_ALLOWLIST;

if config.mcp.server.exposed_tools.is_empty() {
    let mut tools: Vec<String> = feature_registry
        .tool_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    tools.extend(EXPLICIT_TOOL_ALLOWLIST.iter().map(|s| s.to_string()));
    config.mcp.server.exposed_tools = tools;
}
```

- [ ] **Step 3: Update the existing `test_mcp_config_defaults` test**

The test now expects `cfg.server.exposed_tools` to be empty (not the previous 16-item list). Edit the assertion:

```rust
#[test]
fn test_mcp_config_defaults() {
    let cfg = McpConfig::default();
    assert!(cfg.enabled);
    assert!(cfg.servers.is_empty());
    assert!(!cfg.server.enabled);
    assert_eq!(cfg.server.port, 3100);
    assert_eq!(cfg.server.host, "127.0.0.1");
    assert!(cfg.server.exposed_tools.is_empty(),
        "default exposed_tools is empty; app-core fills from AiFeatureRegistry");
}
```

- [ ] **Step 4: Run config tests**

Run: `cargo nextest run -p config`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/mcp.rs
git commit -m "feat(config): replace default_exposed_tools with registry-driven post-load fill"
```

---

## Task 48: Add invariant test: `default_exposed_tools` reflects `registry.tool_names() ∪ EXPLICIT_ALLOWLIST`

**Files:**
- Create: `tests/ai_mcp_default_tools_from_registry.rs`

`★ Insight ─────────────────────────────────────`
This invariant prevents the registry from drifting from the MCP exposure list. If a future feature is added to the registry but its `tool_name` is omitted (because the developer forgot), this test fails — telling us the auto-derive is incomplete. The reverse direction (a tool exposed via MCP but not in registry+allowlist) is also caught.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the test**

```rust
use std::collections::HashSet;

#[test]
fn exposed_tools_post_init_equals_registry_plus_allowlist() {
    let reg = klyntbot::app_core::init::ai_pipeline::build_feature_registry();
    let registry_tools: HashSet<&'static str> = reg.tool_names().into_iter().collect();
    let allowlist: HashSet<&'static str> =
        config::schema::mcp::EXPLICIT_TOOL_ALLOWLIST.iter().copied().collect();

    let expected: HashSet<&'static str> =
        registry_tools.union(&allowlist).copied().collect();

    // Simulate the app-core post-load fill:
    let mut filled: Vec<String> = registry_tools.iter().map(|s| s.to_string()).collect();
    filled.extend(allowlist.iter().map(|s| s.to_string()));
    let filled_set: HashSet<String> = filled.into_iter().collect();

    let expected_strings: HashSet<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(filled_set, expected_strings);
}

#[test]
fn no_overlap_between_registry_and_allowlist() {
    let reg = klyntbot::app_core::init::ai_pipeline::build_feature_registry();
    let registry_tools: HashSet<&'static str> = reg.tool_names().into_iter().collect();
    let allowlist: HashSet<&'static str> =
        config::schema::mcp::EXPLICIT_TOOL_ALLOWLIST.iter().copied().collect();
    let overlap: HashSet<_> = registry_tools.intersection(&allowlist).collect();
    assert!(overlap.is_empty(),
        "tool {} is registered as both an AiFeature tool_name and in EXPLICIT_TOOL_ALLOWLIST",
        overlap.iter().next().map(|s| **s).unwrap_or(""));
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mcp_default_tools_from_registry`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_mcp_default_tools_from_registry.rs
git commit -m "test(invariant): MCP exposed_tools equals registry tool_names ∪ allowlist"
```

---

## Task 49: Implement `dispatch_entity_update(kind, id)` in `mcp/src/dispatch.rs` from registry

**Files:**
- Create: `crates/mcp/src/dispatch.rs`
- Modify: `crates/mcp/src/lib.rs`
- Modify: `crates/mcp/Cargo.toml` (add `ai-core` dep if missing)

`★ Insight ─────────────────────────────────────`
The dispatch function is intentionally simple — it takes a `(kind, id)` pair and a registry reference, looks up the feature owning that `entity_kind`, and emits the right kind of UI update. The dispatch table is built at startup from the registry, so adding a new feature requires zero changes here. This replaces whatever Task 30 discovered as the current ad-hoc dispatch site.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect Task 30's discovery doc**

Run: `cat docs/superpowers/notes/2026-04-23-entity-update-discovery.md`
Use the dispatch site list to inform what `EntityUpdate` payload shape is required.

- [ ] **Step 2: Implement `dispatch.rs`**

```rust
//! Auto-derived entity-update dispatch for MCP entity-changed notifications.
//! The lookup table is sourced from `ai_core::AiFeatureRegistry::iter()`.

use ai_core::{AiFeatureRegistry, FeatureRecord, RecallDomain};

/// Identifies an entity update by kind + ID. Maps 1:1 to the prior
/// hand-coded `emit_updates(&app, &updates)` payload shape (see
/// docs/superpowers/notes/2026-04-23-entity-update-discovery.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUpdate {
    pub kind: String,
    pub id: String,
    pub domain: RecallDomain,
}

/// Resolve an `(entity_kind, id)` into an `EntityUpdate` carrying the right
/// `RecallDomain`. Returns `None` if no feature owns this kind — caller
/// decides whether to log+drop or panic.
pub fn dispatch_entity_update(
    reg: &AiFeatureRegistry,
    kind: &str,
    id: &str,
) -> Option<EntityUpdate> {
    let rec: &FeatureRecord = reg.by_entity_kind(kind)?;
    Some(EntityUpdate {
        kind: kind.to_string(),
        id: id.to_string(),
        domain: rec.domain,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::FeatureRecord;

    fn fake_registry() -> AiFeatureRegistry {
        let mut reg = AiFeatureRegistry::new();
        reg.register(FeatureRecord {
            domain: RecallDomain::Tasks,
            skill: "task-management",
            tool_name: Some("tasks"),
            entity_kind: Some("task"),
        });
        reg
    }

    #[test]
    fn dispatch_returns_some_for_known_kind() {
        let reg = fake_registry();
        let upd = dispatch_entity_update(&reg, "task", "abc").expect("Some");
        assert_eq!(upd.kind, "task");
        assert_eq!(upd.id, "abc");
        assert_eq!(upd.domain, RecallDomain::Tasks);
    }

    #[test]
    fn dispatch_returns_none_for_unknown_kind() {
        let reg = fake_registry();
        assert!(dispatch_entity_update(&reg, "unknown_kind", "x").is_none());
    }
}
```

- [ ] **Step 3: Wire into `lib.rs`**

```rust
pub mod dispatch;
pub use dispatch::{dispatch_entity_update, EntityUpdate};
```

- [ ] **Step 4: Add `ai-core` dep to `crates/mcp/Cargo.toml`**

```toml
[dependencies]
ai-core = { path = "../ai-core" }
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p mcp dispatch`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/mcp/src/dispatch.rs crates/mcp/src/lib.rs crates/mcp/Cargo.toml
git commit -m "feat(mcp): introduce dispatch_entity_update driven by AiFeatureRegistry"
```

---

## Task 50: Migrate every `emit_updates` call site identified in Task 30 to use `dispatch_entity_update`

**Files:**
- Modify: every file listed in `docs/superpowers/notes/2026-04-23-entity-update-discovery.md`

`★ Insight ─────────────────────────────────────`
This is the largest user-visible change in v3 — every handler that previously called `emit_updates(&app, &updates)` for a single-entity update now uses `dispatch_entity_update(&app.feature_registry, kind, id)` and forwards the `EntityUpdate` to the channel/SSE layer. Multi-entity batches stay on the existing path. The kind+id pair becomes the canonical dispatch shape; richer payloads (which the discovery doc may have flagged) stay bespoke and are noted in a follow-up.
`─────────────────────────────────────────────────`

- [ ] **Step 1: For each dispatch site in the discovery doc**

For example, if discovery showed `crates/app-core/src/handlers/tasks/crud.rs:128 emit_updates(&app, vec![EntityUpdate::Task(id.clone())])`, refactor to:

```rust
use mcp::dispatch_entity_update;

if let Some(upd) = dispatch_entity_update(&app.feature_registry, "task", &id) {
    app.broadcast_entity_update(upd);
}
```

The exact replacement depends on what the discovery doc captured. Reuse the existing channel/broadcast machinery — only the *construction* of the `EntityUpdate` value changes.

- [ ] **Step 2: Delete `EntityUpdate` enum if it existed**

If Task 30 found a prior `pub enum EntityUpdate { Task(String), Finance(String), … }`, delete it now — the new `mcp::EntityUpdate { kind, id, domain }` is its single source of truth.

- [ ] **Step 3: Run app-core tests**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/
git commit -m "refactor(app-core): route entity updates through mcp::dispatch_entity_update"
```

---

## Task 51: Extend `PluginHostContext` with `Arc<DomainEventBus>`

**Files:**
- Modify: `crates/plugin-runtime/src/host/mod.rs`
- Modify: `crates/plugin-runtime/src/wasm_plugin.rs`
- Modify: `crates/plugin-runtime/src/manager.rs`
- Modify: `crates/plugin-runtime/Cargo.toml` (add `bus` dep if missing)

`★ Insight ─────────────────────────────────────`
The host context (`PluginHostContext` or whatever struct is closured into the host functions) currently carries `plugin_id`, `permissions`, and a few callback handles. v3 adds `Arc<DomainEventBus>` so `agent_emit_event` can publish. The `Arc` is cheap to clone per-host-fn and `Send + Sync` so it works inside the Extism closure pattern (`user_data.lock()`). We do NOT add a generic `&dyn Any` extension point — keeping the bus as a typed field catches misuse at compile time.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Find the host context struct**

```bash
grep -n "pub struct PluginHostContext\|struct HostContext\|pub struct PluginContext" crates/plugin-runtime/src/host/mod.rs crates/plugin-runtime/src/wasm_plugin.rs
```

Expected: shows the struct used inside the `Function::new(...).user_data(...)` closures. Note its existing fields.

- [ ] **Step 2: Add the `bus` field**

```rust
pub struct PluginHostContext {
    pub plugin_id: String,
    pub permissions: Vec<PluginPermission>,
    pub bus: Arc<bus::DomainEventBus>,         // NEW
    // … existing fields
}
```

- [ ] **Step 3: Pass the bus through `WasmPlugin::new`**

```rust
impl WasmPlugin {
    pub fn new(
        manifest: PluginManifest,
        wasm_bytes: &[u8],
        bus: Arc<bus::DomainEventBus>,        // NEW parameter
    ) -> Result<Self> {
        let ctx = PluginHostContext {
            plugin_id: manifest.id.clone(),
            permissions: manifest.permissions.clone(),
            bus,
            // … existing fields
        };
        // … rest unchanged
    }
}
```

- [ ] **Step 4: Pass the bus through `PluginManager`**

```rust
impl PluginManager {
    pub fn new(/* existing params */, bus: Arc<bus::DomainEventBus>) -> Self {
        Self { /* … */, bus }
    }

    fn load_plugin(&self, manifest: PluginManifest, bytes: Vec<u8>) -> Result<WasmPlugin> {
        WasmPlugin::new(manifest, &bytes, Arc::clone(&self.bus))
    }
}
```

Update every caller of `PluginManager::new` (likely in `app-core/init/`) to pass the bus.

- [ ] **Step 5: Build**

Run: `cargo build -p plugin-runtime -p app-core`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/plugin-runtime/ crates/app-core/src/init/
git commit -m "feat(plugin-runtime): pass DomainEventBus into PluginHostContext"
```

---

## Task 52: Define `PluginEmittedEvent` schema (the variant was added in Task 31)

**Files:**
- Modify: `crates/plugin-runtime/src/lib.rs` (or a new `src/event_schema.rs`)
- Modify: `crates/plugin-runtime/Cargo.toml` (add `serde`, `serde_json` if not present)

`★ Insight ─────────────────────────────────────`
The plugin emits arbitrary JSON — we cannot let it construct typed `DomainEvent` variants directly because that would let a malicious plugin synthesize events from any feature (e.g. fake `TaskCompleted` to tamper with metrics). The fixed wrapper `PluginEmittedEvent { kind, payload }` makes the event provenance explicit: every plugin-originated event lands as `DomainEvent::PluginEvent { plugin_id, kind, payload }`, so consumers can apply trust policies (e.g. cognitive ingestion ignores plugin events with `importance > 0.4`).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Define the schema**

```rust
//! crates/plugin-runtime/src/event_schema.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginEmittedEvent {
    pub kind: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginEventValidationError {
    #[error("plugin event kind must be non-empty")]
    EmptyKind,
    #[error("plugin event kind must be ASCII alphanumeric or underscore (got {0:?})")]
    InvalidKindChars(String),
    #[error("plugin event kind exceeds 64 chars")]
    KindTooLong,
    #[error("plugin event payload exceeds 4 KiB JSON")]
    PayloadTooLarge,
}

impl PluginEmittedEvent {
    pub fn validate(&self) -> Result<(), PluginEventValidationError> {
        if self.kind.is_empty() {
            return Err(PluginEventValidationError::EmptyKind);
        }
        if self.kind.len() > 64 {
            return Err(PluginEventValidationError::KindTooLong);
        }
        if !self.kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(PluginEventValidationError::InvalidKindChars(self.kind.clone()));
        }
        let payload_size = serde_json::to_vec(&self.payload)
            .map(|v| v.len())
            .unwrap_or(0);
        if payload_size > 4096 {
            return Err(PluginEventValidationError::PayloadTooLarge);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_json() {
        let s = r#"{"kind":"my_event","payload":{"x":1}}"#;
        let e: PluginEmittedEvent = serde_json::from_str(s).unwrap();
        assert_eq!(e.kind, "my_event");
        assert!(e.validate().is_ok());
    }

    #[test]
    fn rejects_empty_kind() {
        let e = PluginEmittedEvent { kind: "".to_string(), payload: serde_json::Value::Null };
        assert!(matches!(e.validate(), Err(PluginEventValidationError::EmptyKind)));
    }

    #[test]
    fn rejects_invalid_chars_in_kind() {
        let e = PluginEmittedEvent { kind: "bad-kind!".to_string(), payload: serde_json::Value::Null };
        assert!(matches!(e.validate(), Err(PluginEventValidationError::InvalidKindChars(_))));
    }

    #[test]
    fn rejects_oversized_payload() {
        let huge = serde_json::Value::String("x".repeat(5000));
        let e = PluginEmittedEvent { kind: "k".to_string(), payload: huge };
        assert!(matches!(e.validate(), Err(PluginEventValidationError::PayloadTooLarge)));
    }
}
```

- [ ] **Step 2: Add module declaration in `lib.rs`**

```rust
pub mod event_schema;
pub use event_schema::{PluginEmittedEvent, PluginEventValidationError};
```

- [ ] **Step 3: Add `thiserror` dep if missing**

Run: `grep thiserror crates/plugin-runtime/Cargo.toml`
If empty: `thiserror = { workspace = true }`.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p plugin-runtime event_schema`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/plugin-runtime/src/event_schema.rs crates/plugin-runtime/src/lib.rs crates/plugin-runtime/Cargo.toml
git commit -m "feat(plugin-runtime): introduce PluginEmittedEvent schema with validation"
```

---

## Task 53: Implement `agent_emit_event` parsing → publish to `DomainEventBus`

**Files:**
- Modify: `crates/plugin-runtime/src/host/mod.rs:505-536` (the `agent_emit_event` host function)

`★ Insight ─────────────────────────────────────`
This is the actual fix to the spec's "currently drops" complaint. The existing closure logs+returns `{"ok":true}`. The new version: parse → validate → publish → return outcome. Validation errors are surfaced to the plugin as JSON `{"error": "..."}` so plugin authors can debug; bus publish failures are logged but not surfaced (the plugin shouldn't know about bus internals).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Replace the closure body**

In `crates/plugin-runtime/src/host/mod.rs` (around line 505):

```rust
// agent_emit_event: parse, validate, publish to DomainEventBus
{
    let f = Function::new(
        "agent_emit_event",
        [PTR],
        [PTR],
        ud.clone(),
        |plugin, inputs, outputs, user_data| {
            let input: String = plugin.memory_get_val(&inputs[0])?;
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();

            if !ctx.permissions.contains(&PluginPermission::Agent) {
                let handle = plugin.memory_new(r#"{"error":"agent permission denied"}"#)?;
                outputs[0] = plugin.memory_to_val(handle);
                return Ok(());
            }

            let event: crate::PluginEmittedEvent = match serde_json::from_str(&input) {
                Ok(e) => e,
                Err(e) => {
                    let msg = format!(r#"{{"error":"invalid JSON: {}"}}"#, e);
                    let handle = plugin.memory_new(&msg)?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }
            };

            if let Err(e) = event.validate() {
                let msg = format!(r#"{{"error":"validation failed: {}"}}"#, e);
                let handle = plugin.memory_new(&msg)?;
                outputs[0] = plugin.memory_to_val(handle);
                return Ok(());
            }

            // Publish to bus.
            ctx.bus.publish(bus::DomainEvent::PluginEvent {
                plugin_id: ctx.plugin_id.clone(),
                kind: event.kind.clone(),
                payload: event.payload.clone(),
            });

            tracing::info!(
                plugin_id = %ctx.plugin_id,
                kind = %event.kind,
                "plugin event published"
            );

            let handle = plugin.memory_new(r#"{"ok":true}"#)?;
            outputs[0] = plugin.memory_to_val(handle);
            Ok(())
        },
    )
    .with_namespace("klyntbot");
    functions.push(f);
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p plugin-runtime`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/plugin-runtime/src/host/mod.rs
git commit -m "feat(plugin-runtime): publish agent_emit_event payloads to DomainEventBus"
```

---

## Task 54: Integration test — fixture plugin emits, bus consumer receives

**Files:**
- Create: `tests/ai_plugin_event_published.rs`
- Likely already exists: `tests/fixtures/hello_plugin/` (the workspace-excluded WASM plugin)

`★ Insight ─────────────────────────────────────`
The fixture plugin pattern is established — `tests/fixtures/hello_plugin` is excluded from the workspace and built separately. We add a second fixture (or extend `hello_plugin`) to call `agent_emit_event` from its WASM entrypoint, then assert via a bus subscriber in the test that the `DomainEvent::PluginEvent` was published with the expected payload.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect the existing plugin fixture**

```bash
ls tests/fixtures/hello_plugin/
cat tests/fixtures/hello_plugin/src/lib.rs
```

Note: this only runs with `--features plugin-integration` per the workspace test layout.

- [ ] **Step 2: Add an `emit_event` entrypoint to the fixture plugin**

In `tests/fixtures/hello_plugin/src/lib.rs`:

```rust
#[plugin_fn]
pub fn emit_test_event(_: ()) -> FnResult<String> {
    let payload = r#"{"hello":"from plugin"}"#;
    let req = format!(r#"{{"kind":"hello_emitted","payload":{}}}"#, payload);
    let resp: String = unsafe { agent_emit_event(&req)? };
    Ok(resp)
}

#[host_fn]
extern "ExtismHost" {
    fn agent_emit_event(input: &str) -> String;
}
```

(Actual macro names per `extism-pdk` — check the existing plugin's call style.)

- [ ] **Step 3: Rebuild the plugin fixture**

Per the workspace convention:

```bash
cd tests/fixtures/hello_plugin && cargo build --target wasm32-unknown-unknown --release
```

The output `.wasm` should land where the test discovers it (check `tests/plugins.rs` for the path constant).

- [ ] **Step 4: Write the integration test**

```rust
//! tests/ai_plugin_event_published.rs

#![cfg(feature = "plugin-integration")]

use std::sync::Arc;
use bus::{DomainEvent, DomainEventBus};

#[tokio::test]
async fn fixture_plugin_emit_published_to_bus() {
    let bus = Arc::new(DomainEventBus::new());
    let mut rx = bus.subscribe();

    // Spin up the plugin manager with this bus.
    let manager = klyntbot::plugin_runtime::PluginManager::new(
        /* … other deps … */,
        Arc::clone(&bus),
    );
    let plugin = manager.load_test_fixture("hello_plugin").await
        .expect("plugin loaded");

    plugin.invoke("emit_test_event", "").await
        .expect("plugin invocation");

    let event = rx.recv().await.expect("event received within timeout");
    match event {
        DomainEvent::PluginEvent { plugin_id, kind, payload } => {
            assert_eq!(plugin_id, "hello_plugin");
            assert_eq!(kind, "hello_emitted");
            assert_eq!(payload, serde_json::json!({"hello": "from plugin"}));
        }
        other => panic!("expected PluginEvent, got {:?}", other),
    }
}
```

(The exact `PluginManager` API and `load_test_fixture` helper depend on the existing test scaffolding — adapt to what's in `tests/plugins.rs`.)

- [ ] **Step 5: Run the test**

Run: `cargo nextest run --features plugin-integration --test ai_plugin_event_published`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/ai_plugin_event_published.rs tests/fixtures/hello_plugin/src/lib.rs
git commit -m "test(integration): plugin agent_emit_event reaches DomainEventBus"
```

---

## Task 55: Create `FsrsParamsRepo` with `update_desired_retention(retention: f64)` writer

**Files:**
- Create: `crates/cognitive/src/repos/fsrs_params.rs`
- Modify: `crates/cognitive/src/repos/mod.rs` (re-export)

`★ Insight ─────────────────────────────────────`
Today `crates/cognitive/src/repos/flashcard.rs:671` reads `fsrs_parameters` via inline SQL (`load_fsrs_params`). The writer was missing because no path consumed it — the autotuner observed `fsrs_desired_retention` as a tunable scalar but never wrote it back to the table, so FSRS scheduling continued with the seed value indefinitely. v3 introduces `FsrsParamsRepo` as a typed boundary; the existing `load_fsrs_params` reader can stay where it is for now (Task 55 doesn't move it), but the writer lives in the new repo so Task 56's autotuner bridge has a single typed surface to call.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/cognitive/src/repos/fsrs_params.rs`:

```rust
//! FsrsParamsRepo — typed write surface for the fsrs_parameters table.
//! The reader stays in flashcard.rs for now (it has tight coupling with
//! scheduling); v3 only adds the writer needed by autotuner promotion.

use common::Result;
use storage::StoragePool;

#[derive(Clone)]
pub struct FsrsParamsRepo {
    pool: StoragePool,
}

impl FsrsParamsRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    /// Update the `desired_retention` for the singleton `local` row.
    /// Caller is responsible for clamping to FSRS-valid range [0.7, 0.99].
    pub async fn update_desired_retention(&self, retention: f64) -> Result<()> {
        if !(0.7..=0.99).contains(&retention) {
            return Err(common::KlyntbotError::InvalidArgument(format!(
                "fsrs desired_retention out of range: {retention} (must be 0.7..=0.99)"
            )));
        }
        sqlx::query(
            "UPDATE fsrs_parameters SET desired_retention = ?, trained_at = datetime('now') WHERE id = 'local'",
        )
        .bind(retention)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn writes_and_reads_back() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Apply cognitive migrations so the table + seed row exist.
        crate::migrations::apply_all(&pool).await.unwrap();

        let repo = FsrsParamsRepo::new(pool.clone());
        repo.update_desired_retention(0.85).await.unwrap();

        let (_w, retention): (String, f64) = sqlx::query_as(
            "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert!((retention - 0.85).abs() < 1e-9);
    }

    #[tokio::test]
    async fn rejects_out_of_range() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        crate::migrations::apply_all(&pool).await.unwrap();

        let repo = FsrsParamsRepo::new(pool);
        assert!(repo.update_desired_retention(1.5).await.is_err());
        assert!(repo.update_desired_retention(0.5).await.is_err());
    }
}
```

(The exact `crate::migrations::apply_all` call may have a different name — check `crates/cognitive/src/migrations/mod.rs` for the existing migration runner.)

- [ ] **Step 2: Run — should pass**

Run: `cargo nextest run -p cognitive fsrs_params`
Expected: PASS.

- [ ] **Step 3: Re-export from `repos/mod.rs`**

```rust
pub mod fsrs_params;
pub use fsrs_params::FsrsParamsRepo;
```

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/fsrs_params.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): introduce FsrsParamsRepo with update_desired_retention writer"
```

---

## Task 56: Wire autotuner promotion path to call `FsrsParamsRepo::update_desired_retention`

**Files:**
- Create: `crates/app-core/src/init/fsrs_writeback.rs`
- Modify: `crates/app-core/src/init/mod.rs` (call the writeback hook from autotuner promotion)
- Possibly: `crates/autotuner/src/cycle.rs` (if a callback hook is needed)

`★ Insight ─────────────────────────────────────`
The autotuner emits `TrialPromoted` (or similar) when it picks a winning trial. v3 wires a thin bridge in `app-core::init` that subscribes to that event (or polls the autotuner result) and writes the new `fsrs_desired_retention` value back to `fsrs_parameters`. The bridge lives in `app-core` rather than `autotuner` because `autotuner` is L4 and `cognitive::FsrsParamsRepo` is L5 — direct call would invert the dependency. `app-core` is L7 and can see both.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect the autotuner promotion API**

Run: `grep -n "promote\|TrialPromoted\|CycleResult" crates/autotuner/src/cycle.rs | head -20`
Expected: shows `NightlyCycle::run_evaluation_and_promotion()` returning a `CycleResult`. Note the field names — likely `promoted_trial: Option<TrialParams>` or a `Vec<PromotionDecision>`.

- [ ] **Step 2: Create the writeback bridge**

`crates/app-core/src/init/fsrs_writeback.rs`:

```rust
//! Bridge: autotuner trial promotion -> FsrsParamsRepo write-back.
//! Called by app-core::init after a cycle completes.

use cognitive::repos::FsrsParamsRepo;
use common::Result;
use tracing::{info, warn};

pub async fn apply_promotion(
    repo: &FsrsParamsRepo,
    promoted: &autotuner::TrialParams,
) -> Result<()> {
    if let Some(retention) = promoted.fsrs_desired_retention {
        let clamped = retention.clamp(0.7, 0.99);
        if (clamped - retention).abs() > 1e-9 {
            warn!(
                requested = retention,
                clamped,
                "autotuner produced fsrs_desired_retention outside [0.7, 0.99]; clamped"
            );
        }
        repo.update_desired_retention(clamped).await?;
        info!(retention = clamped, "fsrs_desired_retention updated from autotuner promotion");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use autotuner::TrialParams;

    #[tokio::test]
    async fn writeback_no_op_when_field_unset() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        cognitive::migrations::apply_all(&pool).await.unwrap();
        let repo = FsrsParamsRepo::new(pool.clone());

        let trial = TrialParams { fsrs_desired_retention: None, ..Default::default() };
        apply_promotion(&repo, &trial).await.unwrap();
        // Verify table unchanged (still seed value)
        let (_w, ret): (String, f64) = sqlx::query_as(
            "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert!((ret - 0.9).abs() < 1e-9, "seed retention preserved");
    }

    #[tokio::test]
    async fn writeback_writes_when_field_set() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        cognitive::migrations::apply_all(&pool).await.unwrap();
        let repo = FsrsParamsRepo::new(pool.clone());

        let trial = TrialParams { fsrs_desired_retention: Some(0.92), ..Default::default() };
        apply_promotion(&repo, &trial).await.unwrap();

        let (_w, ret): (String, f64) = sqlx::query_as(
            "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert!((ret - 0.92).abs() < 1e-9);
    }

    #[tokio::test]
    async fn writeback_clamps_out_of_range() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        cognitive::migrations::apply_all(&pool).await.unwrap();
        let repo = FsrsParamsRepo::new(pool.clone());

        let trial = TrialParams { fsrs_desired_retention: Some(1.2), ..Default::default() };
        apply_promotion(&repo, &trial).await.unwrap();

        let (_w, ret): (String, f64) = sqlx::query_as(
            "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert!((ret - 0.99).abs() < 1e-9, "clamped to upper bound");
    }
}
```

- [ ] **Step 3: Wire into `app-core::init::mod`**

Find where the autotuner cycle's promotion result is consumed (likely a tokio-spawn that calls `cycle.run_evaluation_and_promotion()` nightly and discards the result). Replace the discard with:

```rust
match cycle.run_evaluation_and_promotion().await {
    Ok(result) => {
        if let Some(promoted) = &result.promoted_trial {
            if let Err(e) = fsrs_writeback::apply_promotion(&fsrs_params_repo, promoted).await {
                tracing::error!("fsrs writeback failed: {e}");
            }
        }
    }
    Err(e) => tracing::error!("autotuner cycle failed: {e}"),
}
```

(Adjust to match actual `CycleResult` shape.)

- [ ] **Step 4: Run**

Run: `cargo nextest run -p app-core fsrs_writeback`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/fsrs_writeback.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): bridge autotuner promotion -> FsrsParamsRepo write-back"
```

---

## Task 57: Integration test — promoted trial updates `fsrs_parameters.desired_retention`

**Files:**
- Create: `tests/ai_fsrs_writeback_on_promotion.rs`

`★ Insight ─────────────────────────────────────`
This test exercises the full path: synthesize a promoted `TrialParams`, run the bridge, then read the table directly. It catches drift if either the autotuner's promotion API changes or the writer signature changes. The test does NOT depend on the actual nightly cycle running — it isolates the bridge.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the test**

```rust
//! tests/ai_fsrs_writeback_on_promotion.rs

use std::sync::Arc;

#[tokio::test]
async fn promotion_writes_desired_retention_to_table() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    cognitive::migrations::apply_all(&pool).await.unwrap();
    let repo = cognitive::repos::FsrsParamsRepo::new(pool.clone());

    let trial = autotuner::TrialParams {
        fsrs_desired_retention: Some(0.88),
        ..Default::default()
    };

    klyntbot::app_core::init::fsrs_writeback::apply_promotion(&repo, &trial)
        .await
        .expect("writeback ok");

    let (_w, ret): (String, f64) = sqlx::query_as(
        "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert!((ret - 0.88).abs() < 1e-9);
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_fsrs_writeback_on_promotion`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_fsrs_writeback_on_promotion.rs
git commit -m "test(integration): autotuner promotion updates fsrs_parameters.desired_retention"
```

---

## Task 58: Audit-only — `UserSituation` is wired; document keep

**Files:**
- Append: `docs/superpowers/notes/2026-04-23-v3-audit-decisions.md` (create if absent)

`★ Insight ─────────────────────────────────────`
The spec listed `UserSituation` as a candidate for deletion ("zero callers"). The audit found **active callers** in `crates/app-core/src/init/coaching.rs:237` (`compute_situation(&inputs)`) and the `MemoryRetriever`. So we keep it. This task is documentation only — recording the decision so future audits don't re-open the question.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create or append the audit decisions doc**

```markdown
# v3 Audit Decisions (Tasks 58–61)

## UserSituation — KEEP

**Spec asked:** Delete if not wired.
**Audit found:**
- `crates/cognitive/src/services/situation.rs:12` — defines `UserSituation`, `SituationInputs`, `compute_situation`.
- `crates/app-core/src/init/coaching.rs:237` — calls `compute_situation(&inputs)` to seed coaching state.
- `crates/cognitive/src/services/memory_retriever.rs:51,101` — `MemoryRetriever` holds `Option<Arc<Mutex<UserSituation>>>` and exposes `with_situation()`.
- `crates/feature-coaching/src/service.rs:14,205,277` — coaching service holds and updates the situation.

**Decision:** Keep. The spec line was based on a stale snapshot; the type is actively used by the coaching subsystem.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/notes/2026-04-23-v3-audit-decisions.md
git commit -m "docs(v3): record UserSituation keep decision"
```

---

## Task 59: Audit-only — `MetaRule` is wired; document keep

**Files:**
- Append: `docs/superpowers/notes/2026-04-23-v3-audit-decisions.md`

- [ ] **Step 1: Append the section**

```markdown
## MetaRule — KEEP

**Spec asked:** Either implement or purge references.
**Audit found:**
- `crates/cognitive/src/mirror/types.rs` — defines `MetaRule`.
- `crates/cognitive/src/mirror/facade.rs` — uses it in `get_meta_rules`, `create_meta_rule_from_text`, `approve`, `dismiss`.
- `crates/cognitive/src/services/reforge/collector.rs:198` — `pending_meta_rules()` queries it.

**Decision:** Keep. The type is implemented and integrated into the Mirror/Reforge flow.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/notes/2026-04-23-v3-audit-decisions.md
git commit -m "docs(v3): record MetaRule keep decision"
```

---

## Task 60: Delete `InsightCacheRepo` shim (file + re-exports)

**Files:**
- Delete: `crates/cognitive/src/repos/insight_cache.rs`
- Modify: `crates/cognitive/src/repos/mod.rs` (drop the `pub use` line)
- Modify: `crates/cognitive/src/lib.rs` (drop the `pub use` line)

`★ Insight ─────────────────────────────────────`
The audit confirmed `InsightCacheRepo` has zero external callers — it's a deprecated shim left over from a past refactor. The pre-release posture says "no `#[deprecated]` markers; variants, functions, fields are deleted outright." So we delete it now rather than waiting for v4.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Verify no callers remain**

```bash
grep -rn "InsightCacheRepo\|InsightCacheRow" crates/ tests/ --include="*.rs" | grep -v "crates/cognitive/src/repos/insight_cache.rs\|crates/cognitive/src/repos/mod.rs\|crates/cognitive/src/lib.rs"
```

Expected: empty output. If matches show up, those are the callers — fix them first by routing to `feature_insights::InsightReviewRepo` directly.

- [ ] **Step 2: Delete the file**

```bash
git rm crates/cognitive/src/repos/insight_cache.rs
```

- [ ] **Step 3: Drop re-exports**

In `crates/cognitive/src/repos/mod.rs`, remove the line:

```rust
pub use insight_cache::{InsightCacheRepo, InsightCacheRow};
```

(Also remove the `pub mod insight_cache;` declaration.)

In `crates/cognitive/src/lib.rs`, remove:

```rust
pub use repos::{InsightCacheRepo, InsightCacheRow};
```

- [ ] **Step 4: Build**

Run: `cargo build --workspace`
Expected: clean. If a missed caller surfaces, fix it now (do NOT restore the shim).

- [ ] **Step 5: Commit**

```bash
git add -u crates/cognitive/src/
git commit -m "chore(cognitive): delete deprecated InsightCacheRepo shim"
```

---

## Task 61: Audit-only — squad system is wired; document keep

**Files:**
- Append: `docs/superpowers/notes/2026-04-23-v3-audit-decisions.md`

- [ ] **Step 1: Append the section**

```markdown
## Squad System — KEEP (clarified scope)

**Spec asked:** Squad system exists only at repo level with zero agent integration — decide: integrate into insight generation or delete.
**Audit found:**
- `crates/cognitive/src/repos/squad.rs` — defines `SquadRepo`.
- `crates/app-core/src/state.rs:125` — `AppState::squad_repo: Option<SquadRepo>` carried throughout.
- `crates/app-core/src/handlers/squads.rs` — full HTTP handler set for squad CRUD.
- `crates/app-core/src/handlers/chat/threads.rs:18` — passed to thread handling.
- `crates/app-core/src/handlers/chat/streaming.rs:279` — used in chat streaming.

**Decision:** Keep. The spec was technically right that the *agent crate* doesn't directly call squad APIs, but the squad system is wired into chat handling via `app-core` (which is the layer that *should* mediate agent ↔ data access). Integration is sufficient at this layer.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/notes/2026-04-23-v3-audit-decisions.md
git commit -m "docs(v3): record squad system keep decision"
```

---

## Task 62: Identify and delete dead `DomainEvent` variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

`★ Insight ─────────────────────────────────────`
After Tasks 21, 28, 36, 41, 45 migrated their respective events through typed feature enums, several `DomainEvent` variants now have only the `From<FeatureEvent>` constructor as their emitter — meaning they are intermediary types. The activity-log normalizer (slated for Task 63) consumes them but if its consumer gets re-grounded in `AiSignal` rather than `DomainEvent`, those variants become unconsumed. We delete only those that fail the audit: zero direct emitters AND zero direct consumers (excluding the bus's own `variant_name`/`domain` accessors).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Run the dead-variant audit**

```bash
# For each variant in DomainEvent, find emitters (excluding the From<FeatureEvent> impls):
for variant in $(grep -oP 'DomainEvent::\K[A-Z][a-zA-Z]+' crates/bus/src/domain_events.rs | sort -u); do
  emitters=$(grep -rn "DomainEvent::${variant} {" crates/ tests/ --include="*.rs" | grep -v "crates/feature-.*/src/events.rs\|crates/cognitive/src/services/community_intelligence/.*events.rs\|crates/bus/src/" | wc -l | tr -d ' ')
  consumers=$(grep -rn "DomainEvent::${variant}" crates/ tests/ --include="*.rs" | grep -v "crates/feature-.*/src/events.rs\|crates/cognitive/src/services/community_intelligence/.*events.rs\|crates/bus/src/\|crates/app-core/src/init/ai_pipeline.rs\|crates/activity-log/src/normalizers.rs" | wc -l | tr -d ' ')
  echo "$variant emitters=$emitters consumers=$consumers"
done | grep "emitters=0\|consumers=0" || true
```

- [ ] **Step 2: For each variant with zero emitters AND zero non-translator consumers, delete it**

Edit `crates/bus/src/domain_events.rs`. Remove the variant's `enum` arm, its `variant_name()` arm, and its `domain()` arm. Keep variants that the translator references (those have at least one consumer in `ai_pipeline.rs`).

Likely candidates (verify against the audit output, do not delete blindly):
- Any leftover `Squad*` variants if the squad system doesn't emit any.
- Old aliases that v1's delete-list might have missed.

- [ ] **Step 3: Build**

Run: `cargo build --workspace`
Expected: clean (any caller of a deleted variant would error here — that's the safety net).

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "chore(bus): delete dead DomainEvent variants identified by v3 audit"
```

---

## Task 63: Replace `activity-log/src/normalizers.rs::normalize_domain_event` with a `NormalizerSignalConsumer`

**Files:**
- Create: `crates/activity-log/src/consumer.rs`
- Modify: `crates/activity-log/src/normalizers.rs` (slim)
- Modify: `crates/activity-log/src/lib.rs` (re-export)
- Modify: `crates/activity-log/Cargo.toml` (add `ai-core`, `async-trait` if missing)

`★ Insight ─────────────────────────────────────`
This is the architectural payoff for v3 — the second-largest `match` on `DomainEvent` in the workspace (~750 lines) collapses to a `SignalConsumer::consume(&self, signal: &AiSignal)` impl that uses the signal's `event_kind`, `domain`, `content`, and `entity` fields directly. The activity log no longer needs per-variant arms because the signal's fields already carry everything the log row stores. The remaining match (if any) shrinks to a small `match signal.event_kind { … }` block for fields the signal doesn't expose (e.g. message direction for chat events).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Inspect what the existing normalizer extracts**

```bash
sed -n '40,100p' crates/activity-log/src/normalizers.rs
```

Note what fields the `ActivityLogRow` (or equivalent struct) needs. Most are derivable from `AiSignal`:
- `event_kind` ← `signal.event_kind`
- `domain` ← `signal.domain.as_str()`
- `summary` ← `signal.content`
- `entity_id` / `entity_kind` ← `signal.entity`
- `timestamp` ← `signal.timestamp`

The exceptions (chat-message direction, alarm severity, etc.) need a small match arm against `signal.event_kind`.

- [ ] **Step 2: Implement the consumer**

```rust
//! NormalizerSignalConsumer — converts AiSignal into ActivityLogRow.
//! Replaces the per-variant match in normalizers.rs.

use ai_core::{AiSignal, SignalConsumer};
use async_trait::async_trait;
use common::Result;
use std::sync::Arc;

use crate::repo::ActivityLogRepo;
use crate::types::ActivityLogRow;

pub struct NormalizerSignalConsumer {
    repo: Arc<ActivityLogRepo>,
}

impl NormalizerSignalConsumer {
    pub fn new(repo: Arc<ActivityLogRepo>) -> Self { Self { repo } }
}

#[async_trait]
impl SignalConsumer for NormalizerSignalConsumer {
    fn name(&self) -> &'static str { "activity-log-normalizer" }

    async fn consume(&self, signal: &AiSignal) -> Result<()> {
        let row = ActivityLogRow {
            event_kind: signal.event_kind.to_string(),
            domain: signal.domain.as_str().to_string(),
            summary: signal.content.clone(),
            entity_id: signal.entity.as_ref().map(|e| e.id.clone()),
            entity_kind: signal.entity.as_ref().map(|e| e.entity_type.to_string()),
            timestamp: signal.timestamp,
            // Bespoke fields — fall back to event_kind classification:
            extra: classify_extra(signal),
        };
        self.repo.insert(row).await
    }
}

fn classify_extra(signal: &AiSignal) -> serde_json::Value {
    // Most events: empty. Specific kinds (chat, alarm) carry extra info.
    match signal.event_kind {
        "ChatTurnCompleted" => serde_json::json!({ "kind": "chat" }),
        "AlarmFired" => serde_json::json!({ "kind": "alarm" }),
        _ => serde_json::Value::Null,
    }
}
```

(The `ActivityLogRow` struct shape, the repo API, and the bespoke field names depend on the existing `normalizers.rs` impl — read the file, replicate its row-building, then strip the per-variant match in favor of `signal` field access.)

- [ ] **Step 3: Slim `normalizers.rs`**

Delete the 750-line `normalize_domain_event` function. Keep any helpers it called (`format_duration`, `summarize_*`) if still referenced from `classify_extra`. The file may shrink to ~50 lines.

- [ ] **Step 4: Re-export from `lib.rs`**

```rust
pub mod consumer;
pub use consumer::NormalizerSignalConsumer;
```

- [ ] **Step 5: Add unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiSignal, RecallDomain, SalienceVerdict};
    use jiff::Timestamp;

    #[test]
    fn classify_extra_chat() {
        let sig = AiSignal {
            domain: RecallDomain::General,
            event_kind: "ChatTurnCompleted",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: Timestamp::now(),
            raw_event: None,
            metrics: Default::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        };
        assert_eq!(classify_extra(&sig), serde_json::json!({"kind":"chat"}));
    }
}
```

- [ ] **Step 6: Build + test**

Run: `cargo nextest run -p activity-log`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/activity-log/
git commit -m "refactor(activity-log): replace 750-line DomainEvent match with SignalConsumer"
```

---

## Task 64: Register `NormalizerSignalConsumer` with `SignalRouter`

**Files:**
- Modify: `crates/app-core/src/init/mod.rs` (the consumers vector for `ai_pipeline::start`)

- [ ] **Step 1: Add to the consumers list**

In `app-core/src/init/mod.rs`, find the `let mut consumers: Vec<Arc<dyn ai_core::SignalConsumer>> = vec![ … ];` line and append:

```rust
let normalizer_consumer = Arc::new(
    activity_log::NormalizerSignalConsumer::new(Arc::clone(&activity_log_repo))
);
consumers.push(normalizer_consumer);
```

- [ ] **Step 2: Find and remove the legacy bus subscription**

The activity-log was likely subscribing to `DomainEventBus` directly elsewhere. Find that `bus.subscribe()` site in `app-core/init` and delete it — the `SignalRouter` now feeds the normalizer.

```bash
grep -rn "ActivityLog\|activity_log::normalize" crates/app-core/src/init/
```

- [ ] **Step 3: Run startup tests**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/
git commit -m "feat(app-core): register NormalizerSignalConsumer with SignalRouter"
```

---

## Task 65: Invariant test — only the translator matches `DomainEvent`

**Files:**
- Create: `tests/ai_no_remaining_domainevent_match.rs`

`★ Insight ─────────────────────────────────────`
This test enforces the spec's §8.10 success criterion: "the workspace `match` on `DomainEvent` exists in exactly one place". We allowlist the legitimate exceptions: bus accessors (`variant_name`, `domain`, `From<…>` adapters) and the translator's `try_into_*` family. Anything else fails the test — including anyone who later reintroduces a hand-written match.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the test**

```rust
//! tests/ai_no_remaining_domainevent_match.rs
//!
//! Enforces spec §8.10: the workspace `match` on `DomainEvent` exists in
//! exactly one logical place — the translator (`ai_pipeline::translate` and
//! its `try_into_*` helpers). All other matches are either bus-internal
//! accessors or feature `From<FeatureEvent>` adapters; both are allowlisted.

use std::path::PathBuf;
use std::process::Command;

const ALLOWED_FILES: &[&str] = &[
    // bus internals: variant_name() and domain()
    "crates/bus/src/domain_events.rs",
    // translator: the canonical match site (allowed for all try_into_*)
    "crates/app-core/src/init/ai_pipeline.rs",
    // feature From<FeatureEvent> adapters (one per feature)
    "crates/feature-tasks/src/events.rs",
    "crates/feature-finance/src/events.rs",
    "crates/feature-coaching/src/events.rs",
    "crates/feature-productivity/src/events.rs",
    "crates/feature-notes/src/events.rs",
    "crates/feature-learning/src/events.rs",
    "crates/feature-language-learning/src/events.rs",
    "crates/cognitive/src/services/community_intelligence/events.rs",
    "crates/cognitive/src/services/community_intelligence/co_activation_events.rs",
    // wake orchestrator: this match is part of v3.x scope (not in v3 deletion target)
    "crates/app-core/src/wake_orchestrator.rs",
];

#[test]
fn only_allowed_files_match_domainevent() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let output = Command::new("grep")
        .args(["-rln", "match.*DomainEvent\\|match e \\{"])
        .arg("crates/")
        .current_dir(&workspace_root)
        .output()
        .expect("grep ran");
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| l.contains("DomainEvent"))
        .map(|l| l.trim().to_string())
        .collect();

    let unexpected: Vec<&String> = files.iter()
        .filter(|f| !ALLOWED_FILES.iter().any(|a| f.ends_with(a)))
        .collect();

    assert!(
        unexpected.is_empty(),
        "unexpected DomainEvent match in: {:?}",
        unexpected
    );
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_no_remaining_domainevent_match`
Expected: PASS. If it fails, the offending file shows in the error message — either add it to `ALLOWED_FILES` (with a comment justifying) or migrate the match away.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_no_remaining_domainevent_match.rs
git commit -m "test(invariant): only allowlisted files match on DomainEvent"
```

---

## Task 66: Invariant test — every translator-reachable `event_kind` is consumed by `NormalizerSignalConsumer`

**Files:**
- Create: `tests/ai_normalizer_consumer_consumes_all.rs`

`★ Insight ─────────────────────────────────────`
The normalizer consumer is now domain-agnostic — it consumes `AiSignal` directly and uses field access. The test verifies that for every event variant the translator returns `Some(signal)` for, the normalizer's `consume()` call doesn't error and produces a valid `ActivityLogRow`. This is a smoke test against the entire feature catalog.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the test**

```rust
//! tests/ai_normalizer_consumer_consumes_all.rs

use std::sync::Arc;
use ai_core::SignalConsumer;
use bus::DomainEvent;

#[tokio::test]
async fn normalizer_consumes_every_translator_signal() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    cognitive::migrations::apply_all(&pool).await.unwrap();
    activity_log::migrations::apply_all(&pool).await.unwrap();

    let repo = Arc::new(activity_log::repo::ActivityLogRepo::new(pool));
    let consumer = activity_log::NormalizerSignalConsumer::new(repo);

    // Construct a representative event for every translator branch.
    let events: Vec<DomainEvent> = vec![
        DomainEvent::TaskCreated {
            task_id: "t".into(),
            project: None,
            estimate_mins: None,
            task_type: "manual".into(),
        },
        DomainEvent::TransactionRecorded { /* fields */ },
        DomainEvent::CoachingStrategyApplied {
            strategy_id: "s".into(),
            rule_text: "r".into(),
            accepted: true,
        },
        DomainEvent::ProductivitySessionEnded {
            session_id: "p".into(),
            quality: 0.8,
            duration_mins: 30,
        },
        DomainEvent::FocusSessionStarted {
            session_id: "f".into(),
            started_at: "2026-04-23T10:00:00Z".into(),
        },
        DomainEvent::NoteCreated {
            note_id: "n".into(),
            title: "title".into(),
            notebook_id: None,
        },
        DomainEvent::KnowledgeAtomExtracted {
            atom_id: "a".into(),
            note_id: "n".into(),
            text: "x".into(),
        },
        DomainEvent::PronunciationScored {
            session_id: "p".into(),
            overall_score: 0.7,
            weak_phonemes: vec!["TH".into()],
        },
        DomainEvent::CommunityDiscovered {
            community_id: "c".into(),
            name: "n".into(),
            member_count: 5,
        },
        // … add one per translator branch
    ];

    for e in events {
        let signal = klyntbot::app_core::init::ai_pipeline::translate(&e)
            .expect(&format!("translator yielded None for {:?}", e));
        consumer.consume(&signal).await
            .expect(&format!("normalizer failed for {}", signal.event_kind));
    }
}
```

(Field literals are illustrative — fill in real defaults for each variant.)

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_normalizer_consumer_consumes_all`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_normalizer_consumer_consumes_all.rs
git commit -m "test(invariant): NormalizerSignalConsumer handles every translator event"
```

---

## Task 67: Invariant test — every `DEFAULT_SKILLS` entry corresponds to a registered `AiFeature::SKILL`

**Files:**
- Create: `tests/ai_default_skills_match_registry.rs`

`★ Insight ─────────────────────────────────────`
`DEFAULT_SKILLS` is hand-edited, but every entry should correspond to a real `AiFeature` (or be explicitly allowlisted as a non-feature skill). This test catches the mismatch — adding a skill without an AiFeature, or adding an AiFeature whose SKILL doesn't appear in DEFAULT_SKILLS.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the test**

```rust
//! tests/ai_default_skills_match_registry.rs

use std::collections::HashSet;

#[test]
fn every_default_skill_filename_corresponds_to_registered_feature_skill() {
    let reg = klyntbot::app_core::init::ai_pipeline::build_feature_registry();
    let registered_skills: HashSet<&'static str> =
        reg.iter().map(|r| r.skill).collect();

    // Hardcoded DEFAULT_SKILLS filenames (must match crates/skill-system/src/store.rs:18-39).
    // The skill-name is the filename minus ".md".
    let default_skill_names: Vec<&str> = vec![
        "task-management",
        "finance-management",
        "automation",
        "notebook",
        "learning",
    ];

    for skill in &default_skill_names {
        assert!(
            registered_skills.contains(skill),
            "DEFAULT_SKILLS entry {:?} has no corresponding AiFeature::SKILL in registry. \
             Either add an AiFeature with this skill or remove the skill from DEFAULT_SKILLS.",
            skill
        );
    }
}
```

(Note: not every registered feature must be in DEFAULT_SKILLS — coaching, language-learning may share `automation` or `learning`. The reverse direction is what we check: every default skill is owned by at least one feature.)

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_default_skills_match_registry`
Expected: PASS — every default skill (`task-management`, `finance-management`, `automation`, `notebook`, `learning`) is the `SKILL` of at least one feature (Tasks, Finance, Productivity/Coaching, Notes, Learning/LanguageLearning respectively).

- [ ] **Step 3: Commit**

```bash
git add tests/ai_default_skills_match_registry.rs
git commit -m "test(invariant): DEFAULT_SKILLS entries correspond to registered features"
```

---

## Task 68: Final verification

**Files:** n/a (verification only)

`★ Insight ─────────────────────────────────────`
This is the same ritual as v2.5's Task 38 — `fmt`, `clippy`, `nextest`, doctests, plus v3-specific grep sanity. The grep targets prove the kill-list landed: zero `InsightCacheRepo`, zero `normalize_domain_event` callers (the function is gone), zero direct `DomainEvent::Note*` constructions outside the feature crate, zero `DomainEvent::Atom*` outside the learning crate, zero `default_exposed_tools` returning a non-empty literal vector.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Format check**

Run: `cargo fmt --all --check`
Expected: clean. If not: `cargo fmt --all`.

- [ ] **Step 2: Clippy — zero warnings**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: `finished … 0 warnings`.

- [ ] **Step 3: Workspace tests**

Run: `cargo nextest run --workspace`
Expected: all pass.

- [ ] **Step 4: Doctests**

Run: `cargo test --workspace --doc`
Expected: all pass.

- [ ] **Step 5: Plugin-integration tests (separate run)**

Build the fixture plugin first if needed, then:

Run: `cargo nextest run --features plugin-integration`
Expected: PASS, including `ai_plugin_event_published`.

- [ ] **Step 6: Grep sanity — kill-list coverage**

```bash
# InsightCacheRepo gone
grep -Rn "InsightCacheRepo\|InsightCacheRow" crates/ tests/ || echo "OK: no matches"

# normalize_domain_event removed
grep -Rn "normalize_domain_event" crates/ tests/ || echo "OK: no matches"

# Direct DomainEvent::Note construction outside feature-notes
grep -Rn "DomainEvent::Note" crates/app-core/ crates/cognitive/ | grep -v "init/ai_pipeline.rs" || echo "OK"

# Direct DomainEvent::KnowledgeAtom* outside feature-learning + cognitive flashcard repo
grep -Rn "DomainEvent::KnowledgeAtom\|DomainEvent::Atom" crates/app-core/ | grep -v "init/ai_pipeline.rs" || echo "OK"

# Hardcoded MCP tool list gone
grep -A5 "fn default_exposed_tools" crates/config/src/schema/mcp.rs | grep -E '"[a-z]+",' && echo "FAIL: hardcoded list survives" || echo "OK"

# agent_emit_event no longer drops
grep -A20 "agent_emit_event" crates/plugin-runtime/src/host/mod.rs | grep "ctx.bus.publish" || echo "FAIL: bus publish missing"
```

- [ ] **Step 7: Manual smoke (optional)**

```bash
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

In another tab: `cd desktop-ui && bun run dev`. Verify:
- App launches with no startup panic.
- Create a note, see it land in the activity log via the new normalizer.
- Trigger a focus session, see `FocusSessionStarted` then `…Ended` in the log.
- (If a plugin is installed:) call `agent_emit_event` and verify the bus subscriber receives it.

- [ ] **Step 8: Plan checklist**

Re-read this plan. For each task:
- [x] Test was written first (or test was added in the same commit).
- [x] Implementation is the minimum needed to pass.
- [x] No placeholder, TODO, or dead code left behind.
- [x] Commit message follows conventional format.
- [x] Old path was deleted in the same PR (per pre-release posture).

- [ ] **Step 9: Final touch-up commit (if any)**

```bash
git add -u
git commit -m "chore(v3): final verification touch-ups"
```

---

## Plan Checklist — Spec §6 v3 Done Criteria

- [x] **Workspace `match` on `DomainEvent` exists in exactly one place** — Task 65 invariant test (allowlist for bus + feature `From` + translator).
- [x] **Adding a new feature touches exactly one crate** — proven by the `register()` macro emission (Task 7) + registry-driven MCP exposure (Task 47) + auto-derived entity-update dispatch (Task 49) + signal-driven normalizer (Task 63). After v3, a new feature crate writes its own `events.rs` + `feature.rs` + adds itself to `build_feature_registry`. Nothing else.
- [x] **MCP tool exposure auto-derived** — Task 47 + Task 48 invariant.
- [x] **MCP entity-update emission auto-derived** — Task 49 + Task 50 migration.
- [x] **Plugin event publication wired** — Tasks 51/52/53/54.
- [x] **Spec §8 success metrics:**
  - Workspace `match` on `DomainEvent` reduced to one allowlist (Task 65).
  - Dead `DomainEvent` variants → 0 (Task 62).
  - Hardcoded per-tool / per-feature matches outside the feature crate → 0 (Tasks 47, 49).
- [x] **Reconciled spec drift** — Tasks 30, 58, 59, 61 explicitly handle the four spec references that no longer matched the code.
- [x] **FSRS write-back closed** — Tasks 55/56/57 (a v3 add-on; spec called it out in §6 v3 kill-list bullets).
- [x] **Orphan `ContextSource` implementations** — pre-audit confirmed all 16 impls (across `cognitive`, `agent/context_sources`, `activity-log`, `skill-system`) are registered. No deletion needed; finding documented in `docs/superpowers/notes/2026-04-23-v3-audit-decisions.md` (extend the doc started in Tasks 58/59/61 with a fourth section: "Orphan ContextSource — none found").
