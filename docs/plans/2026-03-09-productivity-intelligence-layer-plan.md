# Productivity Intelligence Layer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the old AutoFocusDetector/Categorizer/fixed-score system with a full ProductivityIntelligenceLayer — the brain that creates high-level sessions, predicts energy, scores quality, and generates narrative insights.

**Architecture:** Two-tier federated services. ProductivityEngine (existing, trimmed) handles raw 5s polling + storage. New ProductivityIntelligenceLayer subscribes to the ActivityTick broadcast, classifies via TrackingRulesEngine (O(1)), aggregates into sessions via 4-state FSM, scores quality 0-100, predicts energy windows, and publishes high-level DomainEvents.

**Tech Stack:** Rust, SQLite (sqlx), tokio broadcast/mpsc, FSRS decay (from cognitive crate), DomainEventBus, CancellationToken

**Design doc:** `docs/plans/2026-03-09-productivity-intelligence-layer-design.md`

---

### Task 1: Migration File + New Types

**Files:**
- Create: `crates/feature-productivity/migrations/004_intelligence_layer.sql`
- Modify: `crates/feature-productivity/src/types.rs` (add new types at bottom)

**Step 1: Create migration SQL**

Create `crates/feature-productivity/migrations/004_intelligence_layer.sql` with all new tables:
- `productivity_tracking_rules` (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source, confidence, hit_count, last_hit_at, is_active, created_at, updated_at)
- `productivity_sessions` (id, session_type CHECK focus/meeting/break, started_at, ended_at, duration_secs, dominant_category, category_purity, quality_score, source CHECK auto/manual/predicted, app_breakdown JSON, context_switches, distraction_count, predicted_energy, okr_alignment, notes, tags, created_at, updated_at)
- `productivity_quality_scores` (id, score_date, session_id FK → productivity_sessions ON DELETE CASCADE, overall_score, focus_depth, okr_alignment, distraction_inv, task_completion, continuity, weights_json, explanation, created_at) with UNIQUE index on score_date WHERE session_id IS NULL
- `productivity_forecasts` (id, forecast_date, forecast_type CHECK energy/focus_window/meeting_load/burnout_risk, window_start, window_end, predicted_value, confidence, stability, auto_protected, user_overrode, actual_value, prediction_error, created_at)
- `productivity_narratives` (id, narrative_date UNIQUE, narrative_text, key_moments JSON, sentiment CHECK, total_focus_mins, total_meeting_mins, total_break_mins, quality_score, top_categories JSON, created_at)
- `productivity_voice_journals` (id, recorded_at, duration_secs, transcript, extracted_facts JSON, sentiment, session_id, processed, created_at)
- `productivity_categorization_cache` (cache_key PK, category, session_type, confidence, source CHECK rule/ai/user_override, created_at, expires_at)
- `productivity_privacy_rules` (id, rule_type CHECK exclude_app/redact_title/exclude_url, pattern, match_mode, created_at)
- `productivity_rule_evolution_log` (id, rule_id FK, action CHECK, old_confidence, new_confidence, old_category, new_category, trigger_source, evidence_count, created_at)
- Data migration: INSERT OR IGNORE from focus_sessions → productivity_sessions
- Seed default rules: translate existing 20 activity_categories into productivity_tracking_rules rows

**Step 2: Add new types to types.rs**

Add at bottom of `crates/feature-productivity/src/types.rs`:
- `IntelligenceSessionType` enum: Focus, Meeting, Break (with Display/FromStr/Serialize/Deserialize)
- `RuleType` enum: App, Url, Title, Compound
- `MatchMode` enum: Exact, Prefix, Contains, Regex
- `RuleSource` enum: System, User, Learned
- `ClassificationSource` enum: Rule, AiFallback, Default
- `TrackingRule` struct (all DB fields)
- `ProductivitySession` struct (all DB fields, sqlx::FromRow)
- `QualityScore` struct (all DB fields)
- `Forecast` struct + `ForecastType` enum
- `Narrative` struct + `KeyMoment` struct + `MomentType` enum
- `VoiceJournalEntry` struct
- `PrivacyRule` struct
- `RuleEvolutionEntry` struct
- `ClassificationResult` struct (category, session_type, confidence, rule_id, source)
- `PrivacyFilterResult` struct (excluded, title: Option, url: Option)
- `SessionEvent` enum (Created, Ended, Updated)
- `ProductivityIntervention` struct + `InterventionType` + `SuggestedAction` + `Urgency` enums
- `ScoreWeights` struct (focus_depth, okr_alignment, distraction_inv, task_completion, continuity) with Default impl
- `ProtectedBlock` struct
- `AccuracyReport` struct
- `NarrativeMetrics` struct

**Step 3: Run test to verify migration works**

Run: `cargo nextest run -p feature-productivity -E 'test(migration)' --no-capture` or create a quick test:
```rust
#[tokio::test]
async fn test_migration_004_runs() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    pool.run_feature_migrations(&ProductivityFeature::migrations_static()).await.unwrap();
}
```

**Step 4: Commit**
```bash
git add crates/feature-productivity/migrations/004_intelligence_layer.sql crates/feature-productivity/src/types.rs
git commit -m "feat(productivity): add intelligence layer migration and types"
```

---

### Task 2: New Repo Files (Data Access Layer)

**Files:**
- Create: `crates/feature-productivity/src/repos/tracking_rule.rs`
- Create: `crates/feature-productivity/src/repos/session.rs`
- Create: `crates/feature-productivity/src/repos/quality_score.rs`
- Create: `crates/feature-productivity/src/repos/forecast.rs`
- Create: `crates/feature-productivity/src/repos/narrative.rs`
- Create: `crates/feature-productivity/src/repos/voice_journal.rs`
- Create: `crates/feature-productivity/src/repos/categorization_cache.rs`
- Create: `crates/feature-productivity/src/repos/privacy_rule.rs`
- Create: `crates/feature-productivity/src/repos/rule_evolution_log.rs`
- Modify: `crates/feature-productivity/src/repos/mod.rs` (add modules + extend ProductivityRepos)

All repos follow existing pattern: struct wrapping `SqlitePool`, `new(pool)` constructor, methods return `common::Result<T>`, errors mapped via `common::KlyntbotError::Storage(e.to_string())`.

**Step 1: Write tests first**

Each repo needs at minimum: insert, get_by_id, list (with date range where applicable), update, delete tests. Write inline `#[cfg(test)] mod tests` in each file. Tests use `StoragePool::connect_in_memory()` + `run_feature_migrations()`.

**Step 2: Implement repos**

Key methods per repo:
- `TrackingRuleRepo`: `list_active()`, `get(id)`, `create(rule)`, `update(rule)`, `increment_hit(id)`, `list_by_source(source)`, `deactivate(id)`
- `SessionRepo`: `create(session)`, `get(id)`, `get_active()` (ended_at IS NULL), `end_session(id, ended_at, duration, quality)`, `list_range(start, end)`, `list_by_type(type, start, end)`, `today_summary()` (aggregate query)
- `QualityScoreRepo`: `upsert(score)`, `get_daily(date)`, `get_for_session(session_id)`, `list_range(start, end)`, `average_range(start, end)`
- `ForecastRepo`: `create(forecast)`, `list_for_date(date)`, `update_actual(id, actual_value)`, `list_pending_evaluation()`
- `NarrativeRepo`: `upsert(narrative)`, `get_by_date(date)`, `list_range(start, end)`
- `VoiceJournalRepo`: `create(entry)`, `get(id)`, `list_range(start, end)`, `mark_processed(id)`
- `CategorizationCacheRepo`: `get(cache_key)`, `upsert(entry)`, `purge_expired()`
- `PrivacyRuleRepo`: `list_all()`, `create(rule)`, `delete(id)`
- `RuleEvolutionLogRepo`: `create(entry)`, `list_for_rule(rule_id)`

**Step 3: Update repos/mod.rs**

Add all new module declarations, re-exports, and extend `ProductivityRepos` struct with new handles:
```rust
pub struct ProductivityRepos {
    // ... existing 12 fields ...
    pub tracking_rules: TrackingRuleRepo,
    pub intelligence_sessions: SessionRepo,
    pub quality_scores: QualityScoreRepo,
    pub forecasts: ForecastRepo,
    pub narratives: NarrativeRepo,
    pub voice_journals: VoiceJournalRepo,
    pub categorization_cache: CategorizationCacheRepo,
    pub privacy_rules: PrivacyRuleRepo,
    pub rule_evolution_log: RuleEvolutionLogRepo,
}
```

**Step 4: Run all tests**

Run: `cargo nextest run -p feature-productivity`

**Step 5: Commit**
```bash
git add crates/feature-productivity/src/repos/
git commit -m "feat(productivity): add intelligence layer repos"
```

---

### Task 3: DomainEvent Updates + Salience

**Files:**
- Modify: `crates/bus/src/domain_events.rs` (add 7 new variants)
- Modify: `crates/cognitive/src/salience.rs` (add match arms)
- Modify: `crates/cognitive/src/background.rs` (add event_to_observation arms)

**Step 1: Add new DomainEvent variants**

Add after existing productivity variants in `DomainEvent` enum:
- `SessionCreated { session_id: String, session_type: String, dominant_category: String, predicted_energy: Option<f64> }`
- `SessionEnded { session_id: String, session_type: String, duration_secs: i64, quality_score: Option<f64>, category_purity: f64 }`
- `QualityScored { score_date: String, session_id: Option<String>, overall_score: f64, components: String }`
- `PredictiveAlert { forecast_type: String, window_start: String, window_end: String, predicted_value: f64, suggested_action: Option<String> }`
- `NarrativeGenerated { date: String, sentiment: String, excerpt: String }`
- `RuleEvolved { rule_id: String, action: String, category: String, confidence: f64, source: String }`
- `VoiceJournalProcessed { journal_id: String, extracted_fact_count: usize, sentiment: Option<String> }`

**Step 2: Update salience.rs**

Add match arms in `evaluate_salience()`:
- `SessionCreated` → Discard (only end matters)
- `SessionEnded` with quality >= 80 → Extract, else Accumulate
- `QualityScored` with score >= 85 or <= 30 → Extract, else Accumulate
- `NarrativeGenerated` → Extract
- `PredictiveAlert` → Accumulate
- `RuleEvolved` → Discard (internal bookkeeping)
- `VoiceJournalProcessed` → Extract

**Step 3: Update background.rs event_to_observation()**

Add match arms for new variants to generate appropriate `Observation` structs with domain="productivity".

**Step 4: Run tests**

Run: `cargo nextest run -p bus && cargo nextest run -p cognitive`

**Step 5: Commit**
```bash
git add crates/bus/src/domain_events.rs crates/cognitive/src/salience.rs crates/cognitive/src/background.rs
git commit -m "feat(bus): add intelligence layer domain events and salience"
```

---

### Task 4: TrackingRulesEngine + CategorizationService

**Files:**
- Create: `crates/feature-productivity/src/intelligence/mod.rs`
- Create: `crates/feature-productivity/src/intelligence/tracking_rules.rs`
- Create: `crates/feature-productivity/src/intelligence/categorization.rs`
- Modify: `crates/feature-productivity/src/lib.rs` (add intelligence module)

**Step 1: Create intelligence/mod.rs**

Module declarations for all intelligence submodules.

**Step 2: Write TrackingRulesEngine tests**

Test cases:
- `test_classify_exact_app_match` — known app returns correct category
- `test_classify_url_prefix_match` — URL pattern matches
- `test_classify_regex_fallback` — regex rule matches when exact/prefix miss
- `test_classify_priority_ordering` — lower priority number wins
- `test_classify_no_match_returns_default` — unknown app returns Default source
- `test_reload_picks_up_new_rules` — after evolve_rule, classify finds new rule
- `test_privacy_filter_excludes_app` — excluded app returns excluded=true
- `test_privacy_filter_redacts_title` — redacted title returns None

**Step 3: Implement TrackingRulesEngine**

Key implementation details:
- `load()`: SELECT all active rules, partition into `app_rules` (HashMap), `url_rules` (sorted Vec), `regex_rules` (compiled). Load privacy rules into `PrivacyFilter`.
- `classify()`: (1) exact app lookup in HashMap O(1), (2) binary search prefix scan for URL, (3) linear regex scan, (4) return Default if nothing matches.
- `evolve_rule()`: INSERT into tracking_rules + INSERT into rule_evolution_log, then `reload()`.
- `apply_privacy()`: check excluded_apps set, then check redaction rules for title/url.

**Step 4: Write CategorizationService tests**

- `test_categorize_uses_rules_first` — rule match returns without AI call
- `test_categorize_falls_back_to_cache` — cache hit on second call
- `test_categorize_default_when_no_handler` — no AI handler returns Default

**Step 5: Implement CategorizationService**

- `categorize()`: (1) `rules_engine.classify()` → if source != Default, return. (2) Check in-memory cache. (3) Check DB cache via `categorization_cache.get()`. (4) If AI handler available, call it with batched context. (5) Cache result.

**Step 6: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(tracking_rules) | test(categoriz)'`

**Step 7: Commit**
```bash
git add crates/feature-productivity/src/intelligence/
git commit -m "feat(productivity): add TrackingRulesEngine and CategorizationService"
```

---

### Task 5: SessionAggregator (4-state FSM)

**Files:**
- Create: `crates/feature-productivity/src/intelligence/session_aggregator.rs`

**Step 1: Write FSM tests**

This is the most critical component. Test cases:
- `test_idle_to_building_on_focus_tick` — first focus tick starts Building state
- `test_building_to_active_on_threshold` — 15 min of >=75% focus → Active state + SessionEvent::Created
- `test_building_to_idle_on_timeout` — 20 min without meeting purity → back to Idle
- `test_building_to_idle_on_category_change` — category changes during building → reset
- `test_active_to_ending_on_purity_drop` — purity < 50% for > 2 min → Ending state
- `test_ending_to_idle_emits_session_ended` — Ending completes → SessionEvent::Ended
- `test_ending_recovery_back_to_active` — purity recovers within 2 min → back to Active
- `test_active_emits_updates_every_tick` — each tick in Active emits SessionEvent::Updated
- `test_flush_ends_active_session` — flush() on shutdown ends any active session
- `test_meeting_session_detection` — meeting-category ticks create meeting session
- `test_break_session_detection` — break-category ticks create break session

**Step 2: Implement SessionAggregator**

Key implementation:
- `SessionState` enum: Idle, Building { category, session_type, since, tick_count, matching_ticks }, Active { session_id, session_type }, Ending { session_id, since }
- `process_tick()`: match on current state, evaluate transitions, write to SessionRepo on create/end, publish DomainEvents.
- `VecDeque<ClassifiedTick>` window capped at 240 entries (20 min at 5s intervals).
- Purity calculation: `matching_ticks as f64 / total_ticks as f64`.

**Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(session_agg)'`

**Step 4: Commit**
```bash
git add crates/feature-productivity/src/intelligence/session_aggregator.rs
git commit -m "feat(productivity): add SessionAggregator 4-state FSM"
```

---

### Task 6: QualityScorer + PredictiveEngine

**Files:**
- Create: `crates/feature-productivity/src/intelligence/quality_scorer.rs`
- Create: `crates/feature-productivity/src/intelligence/predictive_engine.rs`

**Step 1: Write QualityScorer tests**

- `test_score_session_computes_components` — verify each component calculation
- `test_score_day_aggregates_sessions` — daily score averages session scores
- `test_weights_default` — verify default weights sum to 1.0
- `test_update_weights` — new weights persist and affect scoring

**Step 2: Implement QualityScorer**

- `score_session()`: load session from SessionRepo, compute 5 components, weighted sum, store in QualityScoreRepo, publish DomainEvent::QualityScored.
- Components: focus_depth = (total_non_idle_secs - distraction_secs) / total_secs, okr_alignment from session's okr_alignment field, distraction_inv = 1.0 - (distraction_count / baseline), task_completion from todo repo, continuity = 1.0 - (context_switches / baseline).

**Step 3: Write PredictiveEngine tests**

- `test_forecast_next_day_uses_history` — with 7 days of data, produces forecasts
- `test_current_energy_interpolates` — returns energy for current time
- `test_evaluate_accuracy_updates_stability` — FSRS stability adjusted based on error
- `test_suggest_protected_blocks` — returns blocks during peak energy windows

**Step 4: Implement PredictiveEngine**

- `forecast_next_day()`: query last 14 days of sessions, compute hourly energy curves, find peaks/valleys, generate Forecast rows with FSRS stability from cognitive::decay.
- `evaluate_accuracy()`: compare predicted vs actual for past date, compute error, update stability using `cognitive::decay::update_stability()`.
- `suggest_protected_blocks()`: find top-2 peak energy windows, create ProtectedBlock suggestions.

**Step 5: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(quality_scor) | test(predictive)'`

**Step 6: Commit**
```bash
git add crates/feature-productivity/src/intelligence/quality_scorer.rs crates/feature-productivity/src/intelligence/predictive_engine.rs
git commit -m "feat(productivity): add QualityScorer and PredictiveEngine"
```

---

### Task 7: InterventionRouter + NarrativeGenerator + VoiceJournal

**Files:**
- Create: `crates/feature-productivity/src/intelligence/intervention_router.rs`
- Create: `crates/feature-productivity/src/intelligence/narrative_generator.rs`
- Create: `crates/feature-productivity/src/intelligence/voice_journal.rs`
- Modify: `crates/feature-productivity/src/handler.rs` (extend ProductivityHandler trait)

**Step 1: Extend ProductivityHandler trait**

Add to `crates/feature-productivity/src/handler.rs`:
```rust
#[async_trait]
pub trait ProductivityHandler: Send + Sync {
    async fn generate_daily_summary(&self, context: &str) -> common::Result<String>;
    async fn generate_narrative(&self, context: &str) -> common::Result<String>;
    async fn classify_activity(&self, app: &str, title: &str, url: Option<&str>) -> common::Result<String>;
}
```

**Step 2: Write InterventionRouter tests**

- `test_context_switch_storm_triggers_focus` — >5 switches in 3 min → FocusModeActivation
- `test_rate_limiting` — max 3 interventions per hour
- `test_break_reminder_after_90min` — continuous focus > 90 min → BreakReminder
- `test_no_intervention_during_meeting` — meeting session suppresses nudges

**Step 3: Implement ProductivityInterventionRouter**

- `evaluate()`: check recent tick history for context-switch storms, check active session quality, check break timing, check predictions for upcoming energy windows. Rate-limit via `VecDeque<InterventionRecord>` with 1-hour window.

**Step 4: Write NarrativeGenerator tests**

- `test_generate_with_no_handler_returns_fallback` — without LLM, returns template-based narrative
- `test_get_or_generate_caches` — second call returns cached narrative

**Step 5: Implement NarrativeGenerator**

- `generate()`: load today's sessions, quality score, top categories from repos. Build context string. If handler available, call `generate_narrative()`. Otherwise, generate template-based narrative. Store in NarrativeRepo. Publish DomainEvent::NarrativeGenerated.

**Step 6: Implement VoiceJournalProcessor**

- `process()`: store audio metadata in VoiceJournalRepo. If transcription handler available, transcribe. Extract facts via ExtractionHandler (reuses existing cognitive trait). Publish DomainEvent::VoiceJournalProcessed. Comment: `// Reuses existing WhisperTool via ToolRegistry for transcription`.

**Step 7: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(intervention) | test(narrative) | test(voice)'`

**Step 8: Commit**
```bash
git add crates/feature-productivity/src/intelligence/intervention_router.rs crates/feature-productivity/src/intelligence/narrative_generator.rs crates/feature-productivity/src/intelligence/voice_journal.rs crates/feature-productivity/src/handler.rs
git commit -m "feat(productivity): add InterventionRouter, NarrativeGenerator, VoiceJournalProcessor"
```

---

### Task 8: ProductivityIntelligenceLayer Orchestrator

**Files:**
- Create: `crates/feature-productivity/src/intelligence/layer.rs`
- Modify: `crates/feature-productivity/src/lib.rs` (add module + re-exports + migration v4)
- Modify: `crates/feature-productivity/src/engine.rs` (expose tick_sender, remove AutoFocusDetector)

**Step 1: Write orchestrator tests**

- `test_intelligence_layer_processes_tick` — send ActivityTick, verify categorization runs
- `test_intelligence_layer_creates_session` — 15 min of focus ticks → session created in DB
- `test_intelligence_layer_scores_on_session_end` — session end triggers quality scoring
- `test_intelligence_layer_respects_cancellation` — cancel token stops the loop

**Step 2: Implement ProductivityIntelligenceLayer**

```rust
pub struct ProductivityIntelligenceLayer { /* all components as Arc<...> */ }
```

The `run()` loop:
```rust
// Subscribes to ActivityTick broadcast (NOT DomainEventBus) because:
// 1. Performance: broadcast is direct, no serialization overhead
// 2. Separation: raw ticks are pre-event (before they become domain events)
// 3. Backpressure: dedicated channel won't be affected by other event consumers
async fn run(mut self) {
    loop {
        tokio::select! {
            _ = self.cancel.cancelled() => {
                self.session_agg.write().await.flush().await;
                break;
            }
            result = self.tick_rx.recv() => {
                match result {
                    Ok(tick) => self.process_tick(tick).await,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Intelligence layer lagged {n} ticks");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}
```

**Step 3: Update engine.rs**

- Remove `auto_focus` field, `AutoFocusDetector` import, `take_auto_focus_rx()` method.
- Add `pub fn tick_sender(&self) -> &broadcast::Sender<ActivityTick>` to expose for IntelligenceLayer subscription.
- Keep Categorizer in tracker for backward compat during transition (IntelligenceLayer uses its own TrackingRulesEngine).

**Step 4: Update lib.rs**

- Add `pub mod intelligence;` module declaration
- Add re-exports for all intelligence types
- Update `ProductivityFeature::migrations_static()` to include v4
- Update `ProductivityFeature::migration_v4_sql()` → `include_str!("../migrations/004_intelligence_layer.sql")`

**Step 5: Run all tests**

Run: `cargo nextest run -p feature-productivity`

**Step 6: Commit**
```bash
git add crates/feature-productivity/src/intelligence/layer.rs crates/feature-productivity/src/lib.rs crates/feature-productivity/src/engine.rs
git commit -m "feat(productivity): add ProductivityIntelligenceLayer orchestrator"
```

---

### Task 9: Update ProductivityContextSource

**Files:**
- Modify: `crates/agent/src/context_sources/productivity.rs`

**Step 1: Update ProductivityContextSource**

Rewrite `provide()` to read from new high-level data:
- Tier 1 (60s TTL): active session from `SessionRepo::get_active()`, quality from `QualityScoreRepo::get_daily(today)`, energy from `ForecastRepo::list_for_date(today)` + interpolate.
- Tier 2 (600s TTL): 14-day patterns from `SessionRepo::list_range()` aggregated, peak windows from `ForecastRepo`, tracking rules count from `TrackingRuleRepo`.
- Tier 3 (event-driven): latest narrative from `NarrativeRepo::get_by_date(yesterday_or_today)`.

**Step 2: Run tests**

Run: `cargo nextest run -p agent -E 'test(productivity_context)'`

**Step 3: Commit**
```bash
git add crates/agent/src/context_sources/productivity.rs
git commit -m "feat(agent): update ProductivityContextSource for intelligence layer"
```

---

### Task 10: Coaching Signal Mapping + Integration Wiring

**Files:**
- Modify: `crates/feature-coaching/src/signal_accumulator.rs` (add signal mappings for new events)
- Modify: `crates/app-core/src/init.rs` (wire ProductivityIntelligenceLayer)

**Step 1: Update signal_accumulator.rs**

Add match arms for new DomainEvent variants:
- `SessionCreated` → `Signal::new(SignalType::FocusSessionStarted, 0.7)`
- `SessionEnded` → `Signal::new(SignalType::FocusSessionEnded, importance)` where importance scales with duration
- `QualityScored` with low score → `Signal::new(SignalType::LowProductivity, 0.8)`
- `PredictiveAlert` → `Signal::new(SignalType::PredictiveWarning, 0.6)`

**Step 2: Wire in app-core/init.rs**

After `ProductivityEngine::start()`:
```rust
let intelligence_layer = ProductivityIntelligenceLayer::new(
    productivity_engine.tick_sender(),
    domain_event_bus.clone(),
    productivity_repos.clone(),
    productivity_handler.clone(),
    extraction_handler.clone(),
    cancel_token.child_token(),
).await?;
let intelligence_handle = intelligence_layer.start();
```

Store handle in AppCore for shutdown.

**Step 3: Run full workspace build + test**

Run: `cargo build --workspace && cargo nextest run --workspace`

**Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

**Step 5: Commit**
```bash
git add crates/feature-coaching/src/signal_accumulator.rs crates/app-core/src/init.rs
git commit -m "feat(productivity): wire ProductivityIntelligenceLayer into app-core"
```

---

### Task 11: Final Verification + Cleanup

**Step 1: Run full test suite**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

**Step 2: Run clippy + fmt**

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

**Step 3: Fix any remaining issues**

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat(productivity): complete ProductivityIntelligenceLayer implementation"
```
