# Productivity Intelligence Layer — Design Document

**Date:** 2026-03-09
**Status:** Approved
**Approach:** Federated Services (Approach 2)

## Overview

Redesign `feature-productivity` into a two-tier architecture:
- **ProductivityEngine** (existing, trimmed): raw tracking layer — ActivityTracker 5s poll, BatchWriter, BucketAggregator, DistractionAnalyzer, DashboardEmitter, NudgeService. Remove: AutoFocusDetector (old FSM), old Categorizer, fixed `compute_productivity_score`.
- **ProductivityIntelligenceLayer** (new): brain orchestrator owning all 7 game-changers.

## Architecture

ProductivityEngine publishes raw `ActivityTick` via broadcast channel (cap=128). ProductivityIntelligenceLayer subscribes to this + DomainEventBus. Publishes high-level events (SessionCreated, QualityScored, PredictiveAlert, NarrativeGenerated, RuleEvolved, VoiceJournalProcessed) to DomainEventBus as single source of truth.

### Components

1. **TrackingRulesEngine** — O(1) HashMap lookup for app rules, prefix scan for URLs, compiled regex fallback. Sources: system, user, learned. Hot-reload on RuleEvolved events.
2. **CategorizationService** — rule match first, AI fallback for unknowns. Results cached in `productivity_categorization_cache` (7-day TTL).
3. **SessionAggregator** — 4-state FSM (Idle → Building → Active → Ending). Transitions: >=15min + >=75% category purity → FocusSession/MeetingSession/BreakSession.
4. **PredictiveEngine** — FSRS stability + historical patterns for energy forecasting, auto-protect deep work blocks.
5. **QualityScorer** — 0-100 semantic score with 5 components: focus_depth(0.30), okr_alignment(0.25), distraction_inv(0.20), task_completion(0.15), continuity(0.10). Weights auto-evolve via LearningService.
6. **ProductivityInterventionRouter** — real-time proactive interventions. Rate limited (3/hr). Coordinates with CoachingService.
7. **NarrativeGenerator** — end-of-day story via local LLM (ProductivityHandler trait).
8. **VoiceJournalProcessor** — audio → whisper transcript → semantic fact extraction → cognitive consolidation.

### Event Flow

```
ActivityTracker (5s poll) → broadcast::Sender<ActivityTick>
  → BatchWriter, BucketAggregator, DistractionAnalyzer, DashboardEmitter (existing)
  → ProductivityIntelligenceLayer (new subscriber)
    → CategorizationService → SessionAggregator → QualityScorer
    → PredictiveEngine, ProductivityInterventionRouter
    → publishes to DomainEventBus
      → CoachingService, BackgroundConsolidationService, ProductivityContextSource,
        LearningService, Weekly Reflection, Desktop UI
```

## DB Schema (Migration 004)

### New Tables
- `productivity_tracking_rules` — user-editable + auto-evolved rules (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source, confidence, hit_count)
- `productivity_sessions` — unified sessions (id, session_type [focus/meeting/break], started_at, ended_at, duration_secs, dominant_category, category_purity, quality_score, source [auto/manual/predicted], app_breakdown JSON, context_switches, distraction_count, predicted_energy, okr_alignment, notes, tags)
- `productivity_quality_scores` — daily + per-session scores (id, score_date, session_id FK, overall_score, focus_depth, okr_alignment, distraction_inv, task_completion, continuity, weights_json, explanation)
- `productivity_forecasts` — predictive forecasts (id, forecast_date, forecast_type, window_start/end, predicted_value, confidence, stability, auto_protected, user_overrode, actual_value, prediction_error)
- `productivity_narratives` — daily stories (id, narrative_date UNIQUE, narrative_text, key_moments JSON, sentiment, metrics snapshot)
- `productivity_voice_journals` — voice entries (id, recorded_at, duration_secs, transcript, extracted_facts JSON, sentiment, session_id, processed)
- `productivity_categorization_cache` — AI fallback cache (cache_key, category, session_type, confidence, source, expires_at)
- `productivity_privacy_rules` — user privacy controls (id, rule_type, pattern, match_mode)
- `productivity_rule_evolution_log` — audit trail (id, rule_id FK, action, old/new confidence, old/new category, trigger_source, evidence_count)

### Data Migration
Existing `focus_sessions` rows migrated to `productivity_sessions` via INSERT OR IGNORE. Old tables preserved (not dropped).

## New DomainEvent Variants
- `SessionCreated { session_id, session_type, dominant_category, predicted_energy }`
- `SessionEnded { session_id, session_type, duration_secs, quality_score, category_purity }`
- `QualityScored { score_date, session_id, overall_score, components }`
- `PredictiveAlert { forecast_type, window_start, window_end, predicted_value, suggested_action }`
- `NarrativeGenerated { date, sentiment, excerpt }`
- `RuleEvolved { rule_id, action, category, confidence, source }`
- `VoiceJournalProcessed { journal_id, extracted_fact_count, sentiment }`

## ContextEngine Update
ProductivityContextSource (priority 55) updated to 3-tier:
- Tier 1 (60s TTL): active session, quality score, energy prediction
- Tier 2 (600s TTL): 14-day patterns, peak windows, OKR alignment, active rules
- Tier 3 (event-driven): latest narrative excerpt

## Integration Points
- AppCore::init() wires ProductivityIntelligenceLayer after ProductivityEngine
- LearningService → TrackingRulesEngine (rule evolution after 3-day/5-observation threshold)
- Weekly Reflection gets productivity week summary as additional context
- CoachingService signal_accumulator maps new events to signals
- Bi-directional: reads TodoSource, CognitiveContextSource, feature-finance; publishes to all downstream consumers

## Constraints
- 100% on-device, no cloud
- Privacy-by-omission: titles redacted before reaching SessionAggregator
- Performance: rule matching O(1), AI only on cache miss
- Zero clippy warnings

## Files to Create/Modify
1. `feature-productivity/migrations/004-intelligence-layer.sql` — new tables + data migration + default rule seeding
2. `feature-productivity/src/lib.rs` — add intelligence module, update FeaturePackage
3. `feature-productivity/src/intelligence/mod.rs` — module declarations
4. `feature-productivity/src/intelligence/tracking_rules.rs` — TrackingRulesEngine
5. `feature-productivity/src/intelligence/categorization.rs` — CategorizationService
6. `feature-productivity/src/intelligence/session_aggregator.rs` — SessionAggregator FSM
7. `feature-productivity/src/intelligence/predictive_engine.rs` — PredictiveEngine
8. `feature-productivity/src/intelligence/quality_scorer.rs` — QualityScorer
9. `feature-productivity/src/intelligence/intervention_router.rs` — ProductivityInterventionRouter
10. `feature-productivity/src/intelligence/narrative_generator.rs` — NarrativeGenerator
11. `feature-productivity/src/intelligence/voice_journal.rs` — VoiceJournalProcessor
12. `feature-productivity/src/intelligence/layer.rs` — ProductivityIntelligenceLayer orchestrator
13. `feature-productivity/src/repos/mod.rs` — updated ProductivityRepos with new handles
14. `feature-productivity/src/repos/tracking_rule.rs` — TrackingRuleRepo
15. `feature-productivity/src/repos/session.rs` — SessionRepo
16. `feature-productivity/src/repos/quality_score.rs` — QualityScoreRepo
17. `feature-productivity/src/repos/forecast.rs` — ForecastRepo
18. `feature-productivity/src/repos/narrative.rs` — NarrativeRepo
19. `feature-productivity/src/repos/voice_journal.rs` — VoiceJournalRepo
20. `feature-productivity/src/repos/categorization_cache.rs` — CategorizationCacheRepo
21. `feature-productivity/src/repos/privacy_rule.rs` — PrivacyRuleRepo
22. `feature-productivity/src/repos/rule_evolution_log.rs` — RuleEvolutionLogRepo
23. `agent/src/context_sources/productivity.rs` — updated ProductivityContextSource
24. `bus/src/domain_events.rs` — 7 new event variants
25. `cognitive/src/salience.rs` — new match arms
26. `feature-coaching/src/signal_accumulator.rs` — new signal mappings
