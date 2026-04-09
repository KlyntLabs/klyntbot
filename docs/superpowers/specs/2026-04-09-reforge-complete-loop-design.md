# Reforge Complete Loop — Deep Feedback for Memory, Skills, and Parameters

**Date:** 2026-04-09
**Status:** Draft
**Depends on:** Reforge Cycle (implemented), Autotuner Phase 6 (implemented)
**Supersedes:** `2026-04-09-memory-retrieval-upgrade-design.md` (merged into this spec)

## Problem Statement

Reforge runs nightly and improves Memory (facts/rules), Skills (file edits), and Parameters (autotuner trials). But it operates on a **thin signal diet**: session scratchpads, episodic memories, routing snapshots, and aggregate retrieval precision. Meanwhile, the system produces rich per-message feedback that is either discarded or stored in tables Reforge never reads:

- **Tool calls fail** (`finance:tx_add` sends wrong `type` param) — Reforge doesn't see the error, can't fix the skill.
- **Users correct the bot** — but corrections aren't linked to the skill or the retrieved facts that caused the mistake.
- **Feature crates track behavioral data** (task estimation bias, coaching acceptance rates, focus quality scores) — none flows to Reforge.
- **Reforge itself generates suggestions** (`ContextPrioritySuggestions`, `CrossSessionPatterns`) then silently drops them.
- **Memory retrieval** is siloed: conversation embeddings and fact embeddings never blend; entity/community graphs exist but aren't used in retrieval.

The result: Reforge improves slowly because it can't see what's actually going wrong.

## Design Principles

1. **Feed every signal, waste nothing** — If the system computes it, Reforge should see it.
2. **Retrieval never gets worse** — New signals add to scoring; kill switches via weight=0.0.
3. **Tier by effort** — Phase A reads existing SQL tables (cheap). Phase B adds new infrastructure. Phase C adds persistence for signals currently discarded.
4. **Reforge self-improves** — Its own output (suggestions, patterns) feeds back into the next cycle.
5. **Cost proportional to value** — Real-time collection is cheap; batch enrichment is triggered by value signals.

---

## Architecture Overview

Three signal streams feed into an enhanced 8-phase Reforge cycle:

```
Real-time Collection (every message):
  Agent Signals:     tool outcomes, corrections, budget exhaustion, validation, loops, confidence
  Feature Signals:   task estimation, finance budgets, focus quality, coaching, suggestions, forecasts
  Cognitive Signals: retrieval scores, extraction yield, co-activation, value density

Reforge Nightly Cycle (enhanced):
  Phase 1:   Collect    ← reads 13+ new data sources
  Phase 2:   Synthesize ← correction→fact links, behavioral facts
  Phase 3:   Review     ← tool failure evidence, correction→skill targeting
  Phase 4:   Narrate    ← tool health + behavioral trends in narrative
  Phase 5:   Apply      ← persist cross-session patterns + context suggestions
  Phase 6:   Optimize   ← routing-outcome correlation for trials
  Phase 6.5: Graph      ← NEW: entity resolution, conversation promotion, temporal snapshot
  Phase 7:   Compact    ← feed CompactionResult to next cycle
```

---

## Phase A — Feedback Wiring

**Goal:** Connect existing SQL data to Reforge. No new tables, no new collection — just read what's already there.

### A1. Tool Failure Summaries into Phase 3 Review

**Source:** `outcome_records` table (written by `OutcomeRecorder` on every tool call)

**Collector addition:** New function `load_tool_failure_summaries(outcome_repo, since)` that queries:

```sql
SELECT tool_name, COUNT(*) as total, SUM(CASE WHEN success=0 THEN 1 ELSE 0 END) as failures,
       GROUP_CONCAT(DISTINCT error_category) as error_types
FROM outcome_records WHERE created_at > ?1
GROUP BY tool_name HAVING failures > 0
ORDER BY failures DESC LIMIT 20
```

**New type:**
```rust
pub struct ToolFailureSummary {
    pub tool_name: String,
    pub total_calls: u32,
    pub failure_count: u32,
    pub failure_rate: f64,
    pub error_types: Vec<String>,
}
```

**Added to:** `ReforgeCollected.tool_failures: Vec<ToolFailureSummary>`

**Consumed by:** Phase 3 Review prompt gets a new section:
```
## Tool Health (since last cycle)
- finance:tx_add — 5/8 calls failed (62.5%) — errors: InvalidParams
- tasks:list — 0/12 calls failed (0%)
```

The Review LLM uses this to propose targeted skill edits.

### A2. Skill Attribution on Corrections

**Current state:** `UserCorrectedAI` events have `active_skill: Option<String>` but it's always `None`.

**Fix:** In `agent_loop/mod.rs` where `DomainEvent::UserCorrectedAI` is published, populate `active_skill` from the current session's active skill (available via `SkillRouter::last_routed_skill()` or from the session's `RuntimeResult`).

**Collector addition:** `load_correction_summaries(event_log_repo, since)` groups corrections by skill:

```rust
pub struct CorrectionSummary {
    pub skill_name: String,
    pub correction_count: u32,
    pub sample_corrections: Vec<String>, // first 3 correction texts, truncated
}
```

**Consumed by:** Phase 3 Review sees "finance-management skill had 3 corrections" alongside tool failure data — both signals pointing at the same skill.

### A3. Feature Behavioral Metrics

Read existing tables to surface behavioral patterns. Each becomes a field on `ReforgeCollected`:

| Metric | Source Table | Query | Type |
|--------|-------------|-------|------|
| Task estimation bias | `task_estimation_history` | `AVG(deviation_pct)` since last run | `Option<f64>` |
| Coaching acceptance rate | `coaching_strategies` | `SUM(times_accepted)/SUM(times_used)` | `Option<f64>` |
| Focus quality trend | `daily_summaries` | `AVG(avg_session_quality)` last 7d vs 14d | `Option<f64>` |
| Suggestion dismiss rate | `task_suggestions` | `dismissed/(dismissed+applied)` since last run | `Option<f64>` |
| Forecast accuracy | `productivity_forecasts` | `AVG(ABS(prediction_error))` since last run | `Option<f64>` |
| Insight retention | `insight_progress_snapshots` | `AVG(composite_score)` per domain | `Vec<(String, f64)>` |
| Strategy tool stats | `strategy_records` via `get_tool_stats()` | Loop exhaustion, escalation rates | `ToolHealthStats` |

**Consumed by:** Phase 2 Synthesize extracts behavioral facts ("user underestimates tasks by 35%"). Phase 3 Review uses suggestion dismiss rates to determine if a skill's proactive features need adjustment.

### A4. Retrieval Quality Per-Domain

**Current:** `retrieval_feedback.avg_precision_since(days)` — single aggregate number.

**Enhancement:** `avg_precision_by_domain_since(days)` grouping by the domain of retrieved facts:

```sql
SELECT f.domain, AVG(rf.precision) as avg_precision, COUNT(*) as query_count
FROM retrieval_feedback rf
JOIN semantic_facts f ON f.id IN (SELECT value FROM json_each(rf.retrieved_fact_ids))
WHERE rf.created_at > ?1
GROUP BY f.domain
```

**Consumed by:** Phase 3 Review sees "finance domain retrieval precision: 45%, work domain: 82%" — identifies which knowledge domains need attention.

### A5. Persist Reforge's Own Output

**Fix 1:** `ContextPrioritySuggestions` — After Phase 3, persist to new table `reforge_suggestions`:
```sql
CREATE TABLE reforge_suggestions (
    id TEXT PRIMARY KEY,
    suggestion_type TEXT,  -- 'context_priority' or 'cross_session_pattern'
    content TEXT,
    reason TEXT,
    confidence REAL,
    cycle_run_at TEXT,
    acted_upon INTEGER DEFAULT 0
);
```

Phase 1 of the next cycle loads recent suggestions and feeds them back to the Review LLM: "Last cycle suggested X — should we act on it?"

**Fix 2:** `CrossSessionPatterns` — After Phase 2, persist high-confidence patterns (>0.7) as episodic memories with domain `reforge` and summary `cross-session pattern`.

**Fix 3:** `CompactionResult` — Phase 7 already returns `CompactionResult`. Store it in `reforge_state.last_compaction_stats` (JSON field) so Phase 1 of the next cycle can include "12 facts archived, 3 rules deactivated, co-activation graph 85% connected" in the synthesis context.

### A6. Knowledge Graph Health Metrics

**Source:** `CoActivationRepo.count_all()`, `SemanticFactRepo` counts, `CompactionResult`

**New struct:**
```rust
pub struct GraphHealthMetrics {
    pub active_facts: u32,
    pub active_rules: u32,
    pub co_activation_pairs: u32,
    pub facts_per_domain: Vec<(String, u32)>,
    pub avg_fact_stability: f64,
    pub prev_compaction: Option<CompactionStats>,
}
```

**Consumed by:** Phase 2 Synthesize sees graph health alongside session data. Phase 3 Review can identify underserved domains.

---

## Phase B — Memory Retrieval Upgrade

Merged from `2026-04-09-memory-retrieval-upgrade-design.md`. Three sub-phases:

### B1. Bridge (Cross-Retrieval + Entity Extraction + Temporal Log)

**B1a. Cross-Retrieval Layer** — After existing two-pass scoring, query `conv_embeddings` for supporting conversation evidence per top-15 facts. New signal: `recall_support` (0.0-1.0) as 11th relevance weight factor. Autotuner can tune this weight.

**B1b. Real-Time Entity Extraction** — Extend existing extraction prompt to also return entities and relationships. Weak strength (0.3-0.5), feeding existing `entity.rs` repo. No new infrastructure.

**B1c. Temporal Fact Log** — New `fact_changelog` table (append-only). Wired into `SemanticFactRepo` mutations. Records every create/update/supersede/archive with old/new values and source.

**B1d. RecallCollector Threshold Adjustment** — Lower gates from "3+ messages, 2+ sessions" to "2+ messages, 1+ session".

### B2. Enrich (Value-Density + Batch Graph + Phase 6.5)

**B2a. Value-Density Classifier** — Heuristic scoring (no LLM) per conversation turn:
- `entity_signal` (0.30) — named entities detected
- `action_signal` (0.25) — action verbs present
- `decision_signal` (0.25) — decision markers
- `novelty_signal` (0.20) — references to unknown entities

Three tiers: High (>0.7, immediate enrichment), Medium (0.4-0.7, queued for Reforge), Low (<0.4, cheap extraction only).

**B2b. Batch Graph Enrichment** — LLM-driven entity resolution, typed relationship extraction, entity merges. Two triggers: immediate (high density) and nightly (medium density via Reforge Phase 6.5).

**B2c. Reforge Phase 6.5: Graph Consolidation** — New phase between Optimize and Compact:
1. Collect medium-density turns since last cycle
2. Batch entity resolution (single LLM call)
3. Relationship refinement (upgrade weak edges)
4. Graph quality metrics (entity coverage, relationship density, orphan rate, merge rate)
5. Feed metrics to next cycle's synthesis prompt

**B2d. Temporal Snapshots** — Nightly snapshot: fact_count, entity_count, relationship_count, domain_summary, top_entities, graph_metrics. On-demand `facts_as_of(timestamp)` for state reconstruction.

### B3. Dissolve (Conversation Promotion + Graph-Aware Retrieval + Temporal Reasoning)

**B3a. Conversation Promoter** — `conv_embeddings` becomes staging area with promotion lifecycle based on value-density tiers. Promoted entries get `promoted_at` timestamp; cross-retrieval deprioritizes them.

**B3b. Graph-Aware Retrieval** — Graph-first with vector fallback: entity extraction from query → graph neighborhood traversal → temporal filter → vector search → merge with `graph_path_boost` (12th weight factor).

**B3c. Temporal Reasoning Queries** — `facts_as_of`, `first_mention`, `change_history`, `competing_truths`, `knowledge_diff`, `decision_points`. Exposed as `TemporalTool` (multi-action, read-only).

---

## Phase C — Deep Signal Integration

**Goal:** Persist signals currently discarded at runtime, enabling the deepest level of self-improvement.

### C1. Agent Runtime Signals

| Signal | Where to Persist | Change |
|--------|-----------------|--------|
| Budget exhaustion + turn count | `strategy_records.budget_exhausted`, `strategy_records.turns_used` | Thread `RuntimeResult` fields through `run_pipeline()` |
| Response validation warnings | New `response_warnings` table | Persist from `ResponseValidator` after content extraction |
| Fabricated tool responses | Counter in `outcome_records` or new `fabrication_log` | Log in `execution/core.rs` when detected |
| Tool oscillation loops | `strategy_records.loop_detected`, `strategy_records.loop_tools` | Persist from `LoopDetector` events |
| Per-message tokens at hook | `autotuner_shadow_log.tokens_used`, `autotuner_shadow_log.response_time_ms` | Remove `_` prefix in `on_message_completed()` |
| Context fill rate | `strategy_records.context_fill_pct` | Persist from `ContextAssembled` event |

### C2. Cognitive Pipeline Signals

| Signal | Where to Persist | Change |
|--------|-----------------|--------|
| Extraction yield per domain | Already in `pipeline_event_log` | Reforge collector reads it via `EventLogRepo` |
| Near-miss accumulations | New counter on `accumulated_observations` | Track patterns that hit `min_days - 1` before cleanup |
| Per-retrieval component scores | `retrieval_feedback.score_breakdown` (JSON) | Extend feedback recording in retrieval service |
| Salience verdict distribution | Aggregate from `domain_event_log.salience` | Collector groups by domain |

### C3. Feature Signal Persistence

| Signal | Current State | Fix |
|--------|--------------|-----|
| Coaching behavioral outcomes | In-memory `FeedbackTracker`, lost on restart | Persist `behavioral_positive/negative` to `coaching_interventions` |
| Distraction learned rules | `distraction_learned_rules` exists | Promote high-confidence rules to semantic memory facts |
| Phoneme mastery struggles | `phoneme_mastery.recent_errors` exists | Feed to Reforge as learning domain facts |
| Note link density | `note_links` exists | Aggregate as knowledge structure health metric |

---

## Data Flow: How Each Phase Uses New Signals

### Phase 1: Collect (Enhanced)

```rust
pub struct ReforgeCollected {
    // Existing
    pub sessions: Vec<SessionContext>,
    pub episodic_memories: Vec<EpisodicMemory>,
    pub user_model: UserModel,
    pub rules: Vec<ProceduralRule>,
    pub routing_summaries: Vec<RoutingSummary>,
    pub pending_meta_rules: Vec<String>,
    pub skill_files: HashMap<String, Vec<SkillFile>>,
    pub retrieval_precision: Option<f64>,
    pub is_bootstrap: bool,
    pub autotuner_ctx: Option<AutotunerContext>,

    // Phase A: Feedback wiring
    pub tool_failures: Vec<ToolFailureSummary>,
    pub correction_summaries: Vec<CorrectionSummary>,
    pub retrieval_precision_by_domain: Vec<(String, f64)>,
    pub behavioral_metrics: BehavioralMetrics,
    pub graph_health: GraphHealthMetrics,
    pub previous_suggestions: Vec<ReforgeSuggestion>,
    pub prev_compaction: Option<CompactionStats>,
    pub extraction_yield: Vec<(String, f64)>,

    // Phase B: Memory upgrade context
    pub pending_enrichment_turns: u32,
    pub graph_consolidation_needed: bool,
}
```

### Phase 2: Synthesize

The LLM now sees:
- Session scratchpads (existing)
- Episodic memories (existing)
- User model (existing)
- **Tool failure patterns** — "the agent consistently fails at X"
- **Behavioral metrics** — "user underestimates tasks, coaching works when gentle"
- **Graph health** — "finance domain has only 6 facts but 45 interactions"
- **Previous cross-session patterns** — "last cycle detected: user works on Klynt mornings, finance evenings"

Output changes:
- `cross_session_patterns` — now persisted (not dropped)
- New: `behavioral_facts` — facts derived from feature metrics ("user_estimation_bias is +35%")

### Phase 3: Review

The LLM now sees:
- Routing summaries + skill contents (existing)
- **Tool failure evidence per skill** — "finance:tx_add fails because the LLM sends account types instead of transaction types"
- **Correction patterns per skill** — "3 corrections this week in task-management, all about date parsing"
- **Previous context priority suggestions** — "last cycle suggested reducing temporal weight — not acted on"
- **Retrieval precision by domain** — "learning domain precision only 30%"
- **Suggestion dismiss rates** — "70% of proactive task suggestions dismissed"

Output changes:
- `context_priority_suggestions` — now persisted (not dropped)
- `skill_edits` — more targeted because evidence-driven
- `trial_suggestions` — can reference domain-specific precision data

### Phase 6.5: Graph Consolidation (New)

Runs between Phase 6 (Optimize) and Phase 7 (Compact):

1. Load medium-density conversation turns queued since last cycle
2. Single LLM call: batch entity resolution + relationship typing + merge decisions
3. Write resolved entities, typed relationships, merge duplicates
4. Promote high-value conversations into knowledge graph
5. Create temporal snapshot
6. Compute graph quality metrics → stored for next cycle's Phase 1

---

## Success Criteria

| # | Criterion | Metric |
|---|-----------|--------|
| 1 | Tool failures visible to Reforge | Phase 3 prompt includes tool failure section; skill edits reference failures |
| 2 | Corrections linked to skills | `active_skill` populated on >90% of `UserCorrectedAI` events |
| 3 | Behavioral facts extracted | Phase 2 produces facts like "user_estimation_bias" from feature metrics |
| 4 | Reforge output persisted | `ContextPrioritySuggestions` and `CrossSessionPatterns` stored and fed back |
| 5 | Cross-retrieval improves relevance | Retrieval precision +5% or no regression |
| 6 | Graph consolidation runs | Phase 6.5 completes without errors; entity count grows week-over-week |
| 7 | Temporal queries work | `facts_as_of(timestamp)` returns correct state on synthetic data |
| 8 | All existing tests pass | No regressions in 1226+ existing tests |
| 9 | Graceful degradation | Missing feature repos (coaching, tasks, etc.) don't crash the collector |
| 10 | Phase A ships independently | Tool failures + corrections improve skills without Phase B/C |

---

## Implementation Phases

### Phase A — Feedback Wiring (ship first, independent)
- A1: Tool failure summaries → Phase 3
- A2: Skill attribution on corrections
- A3: Feature behavioral metrics collection
- A4: Per-domain retrieval precision
- A5: Persist Reforge's own output (suggestions, patterns, compaction)
- A6: Knowledge graph health metrics

### Phase B — Memory Retrieval Upgrade (depends on nothing)
- B1: Cross-retrieval, entity extraction, temporal log, recall thresholds
- B2: Value-density classifier, batch graph enrichment, Phase 6.5, temporal snapshots
- B3: Conversation promoter, graph-aware retrieval, temporal reasoning tool

### Phase C — Deep Signal Integration (depends on A)
- C1: Agent runtime signal persistence (budget, validation, fabrication, loops, tokens, context fill)
- C2: Cognitive pipeline signals (extraction yield, near-miss accumulations, score breakdowns)
- C3: Feature signal persistence (coaching outcomes, distraction rules, phoneme mastery)

---

## Affected Crates

| Crate | Phase A Changes | Phase B Changes | Phase C Changes |
|-------|----------------|-----------------|-----------------|
| `cognitive` | Extended collector, persist suggestions/patterns, graph health | Cross-retrieval, value-density, graph enricher, temporal index, Phase 6.5 | Extraction yield, score breakdowns |
| `storage` | `reforge_suggestions` table, per-domain precision query | `fact_changelog` table, `knowledge_snapshots` table | `response_warnings` table, extended `strategy_records` |
| `agent` | Skill attribution on corrections, read `OutcomeRepo` in collector | Register TemporalTool, wire graph-aware retrieval | Persist budget/validation/loops/tokens |
| `tools` | — | New `TemporalTool` | — |
| `config` | — | New weights (`recall_support`, `graph_path_boost`), value-density thresholds | — |
| `feature-tasks` | Read-only queries on estimation/suggestions | — | — |
| `feature-productivity` | Read-only queries on quality/forecasts | — | Persist coaching behavioral outcomes |
| `feature-coaching` | Read-only queries on strategy acceptance | — | Persist behavioral feedback window |
| `feature-insights` | Read-only queries on progress scores | — | — |
| `autotuner` | — | — | Extended `MetricSnapshot`, per-trial token tracking |

## Non-Goals

- Replacing LanceDB or SQLite with a different storage engine
- Adding structured observability (OpenTelemetry, Prometheus)
- Multi-user or server-based deployment
- UI changes to Brain/Fabric views (separate spec)
- Real-time skill editing (Reforge is nightly batch only)
