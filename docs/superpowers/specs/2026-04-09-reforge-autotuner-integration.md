# Reforge Phase 6 — Autotuner Integration

## Summary

Wire the Autotuner's evaluation + promotion logic into Reforge Phase 6, and merge trial generation into the existing Phase 3 Review LLM call. The orchestrator owns evaluation math and champion state; Reforge owns LLM-driven trial generation with full context awareness.

**Key decisions:**
- **No separate LLM call** — trial generation merges into Phase 3 Review (stays at 3 LLM calls total)
- **Orchestrator owns evaluation** — Phase 6 calls `orchestrator.run_evaluation()` for promotion/rollback
- **Reforge owns generation** — Phase 3 LLM suggests trial params alongside skill edits
- **5 guardrails** protect against LLM-generated trial risks

---

## Architecture

```
Phase 3 (Review LLM call — extended):
  Input:  corrections + routing + skills + champion params + trial history + metrics
  Output: skill_edits + routing_insights + trial_suggestions[0-3]

Phase 6 (Optimize — two steps):
  Step 1: EVALUATE
    orchestrator.run_evaluation(&champion)
    ├─ Evaluate completed trials (50+ messages) against 10 constraints
    ├─ Promote winner → update champion + memory_param_sink
    ├─ Detect regression → rollback if 3+ consecutive days
    └─ Emit AutotunerDecision event → ConfigArchiver creates BrainVersion

  Step 2: CREATE TRIALS
    Take trial_suggestions from Phase 3
    ├─ Validate param ranges (clamp or reject)
    ├─ Check diversity gate (min distance between trials + vs champion)
    ├─ Check active trial cap (max 6)
    ├─ orchestrator.create_trials(validated_params)
    └─ Log experiment creation
```

---

## Phase 3 Review Prompt Extension

### Additional Input

Add to the Review LLM call input:

```
## Autotuner Context

### Current Champion Parameters
routing: skill_keyword_weight=0.60, skill_semantic_weight=0.40, activation_threshold=0.65
retrieval: semantic=0.30, retrievability=0.20, situation=0.25, importance=0.15
memory: vector_top_k=30, min_similarity=0.55, fsrs_desired_retention=0.90
[... grouped by category]

### Performance Metrics
Metrics (last 24h): correction_rate=0.12, retrieval_precision=0.78, avg_response_time=1200ms
Metrics (7-day avg): correction_rate=0.09, retrieval_precision=0.81, avg_response_time=1100ms
Trend: correction_rate ↑3% (worsening), retrieval_precision ↓2% (slight decline)

### Recent Experiment History
Experiment #4 (2 days ago):
  Trial A: relevance_weight_semantic=0.35 → PROMOTED (correction_rate improved 8%)
  Trial B: vector_top_k=60 → FAILED (response_time exceeded +15% threshold)
  Trial C: min_similarity=0.40 → COMPLETED (no improvement over champion)

Experiment #3 (5 days ago):
  Trial A: skill_keyword_weight=0.70 → COMPLETED (routing stability decreased 12%)
  Trial B: fsrs_desired_retention=0.95 → COMPLETED (marginal improvement, not enough)
  Trial C: accumulate_promote_threshold=5 → PROMOTED (convergence_score improved 6%)
```

### Additional Output Field

```json
{
  "skill_edits": [...],
  "routing_insights": [...],
  "context_priority_suggestions": [...],
  "trial_suggestions": [
    {
      "hypothesis": "Increasing semantic weight should improve retrieval for this technical user",
      "pace": "conservative",
      "param_overrides": {
        "relevance_weight_semantic": 0.35,
        "relevance_weight_situation": 0.20
      }
    },
    {
      "hypothesis": "Lower activation threshold may catch more finance-related queries",
      "pace": "balanced",
      "param_overrides": {
        "skill_activation_threshold": 0.55,
        "relevance_weight_community": 0.20
      }
    },
    {
      "hypothesis": "Higher vector_top_k with stricter similarity for precision-focused retrieval",
      "pace": "bold",
      "param_overrides": {
        "vector_top_k": 50,
        "min_similarity": 0.65
      }
    }
  ]
}
```

Only overridden params are specified — the rest inherit from the current champion. The `pace` field is informational (conservative/balanced/bold) for logging.

---

## Phase 6 Implementation

### Step 1: Evaluate

```
1. Get orchestrator reference (Arc<AutoTunerOrchestrator> captured in cron closure)
2. Call orchestrator.run_evaluation() which internally:
   a. Loads all Active trials
   b. Collects 24h metrics via MetricSource
   c. Evaluates each completed trial (50+ messages) against 10 constraints
   d. Selects best passing candidate (improvement score + diversity bonus)
   e. If winner: promote → update champion → update memory_param_sink → emit event
   f. If regression: increment counter → rollback at 3 days
3. Log CycleResult: promotions, regressions, failures, health
```

### Step 2: Create Trials from Phase 3

```
1. Extract trial_suggestions from ReviewOutput (may be empty)
2. For each suggestion:
   a. Validate all param_overrides against defined (min, max) ranges
   b. Clamp out-of-range values, warn if >50% invalid → reject suggestion
3. Diversity gate:
   a. Compute pairwise Euclidean distance in 27-D param space
   b. Reject if any two trials within 5% distance of each other
   c. Reject if all three within 10% distance of champion
   d. Fallback: use random perturbation generator
4. Active trial cap:
   a. Count active trials in DB
   b. If >= 6: skip generation, log "too many active trials"
5. Auto-expire stale trials: deactivate trials > 7 days old with < 20 messages
6. Create remaining valid trials via orchestrator.create_pending_trials()
7. Activate them for shadow scoring
```

---

## Guardrails

| Risk | Guardrail | Implementation | Fallback |
|------|-----------|---------------|----------|
| **Invalid params** | Range validation on all 27 params | Clamp to (min, max); reject if >50% overrides invalid | Skip that suggestion |
| **Mode collapse** | Diversity gate | Normalized Euclidean distance (0-1 scale): min 0.05 between trials, min 0.10 from champion | Random perturbation |
| **Stale context** | 7-day rolling metrics in prompt | Feed both 24h and 7-day averages + trend direction | 24h evaluation unchanged |
| **Trial spam** | Max 6 active + 7-day expiry | Count before creation; expire old low-traffic trials | Skip generation |
| **Unstable reasoning** | Experiment history in prompt | Last 3-5 experiments with outcomes and constraint failures | Champion stays unchanged |

**Defense in depth:** All guardrails are pre-creation filters. The existing 10 constraint evaluators still protect against any trial that slips through — a bad config can only become champion if it actually improves metrics over 50+ real messages.

---

## Data Flow

### Collector (Phase 1) — New Data

Add to `ReforgeCollected`:
```rust
pub champion_summary: Option<String>,        // formatted champion params
pub trial_history: Vec<TrialHistoryEntry>,   // last 5 experiments with outcomes
pub metrics_24h: Option<MetricsSnapshot>,    // current 24h metrics
pub metrics_7d: Option<MetricsSnapshot>,     // 7-day rolling average
pub active_trial_count: u32,                 // for cap check
```

These come from the `AutoTunerOrchestrator` — the collector needs access to it (or its repos).

### Types — New

```rust
pub struct TrialSuggestion {
    pub hypothesis: String,
    pub pace: String,                              // "conservative", "balanced", "bold"
    pub param_overrides: HashMap<String, f64>,     // only overridden params
}

pub struct TrialHistoryEntry {
    pub experiment_id: String,
    pub days_ago: u32,
    pub trials: Vec<TrialOutcome>,
}

pub struct TrialOutcome {
    pub params_summary: String,
    pub result: String,                            // "PROMOTED", "FAILED", "COMPLETED", "EXPIRED"
    pub constraint_failures: Vec<String>,
    pub improvement: Option<f64>,
}

pub struct MetricsSnapshot {
    pub correction_rate: f64,
    pub retrieval_precision: f64,
    pub avg_response_time_ms: f64,
    pub avg_tokens_per_message: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
}
```

### ReviewOutput — Extended

Add `trial_suggestions: Vec<TrialSuggestion>` with `#[serde(default)]`.

---

## Cron Handler Changes

The Reforge cron handler needs `Arc<AutoTunerOrchestrator>` added to its closure captures:

```rust
// In register_cron_callbacks():
let orchestrator = orchestrator.clone(); // Arc<AutoTunerOrchestrator>

// In JOB_REFORGE_NIGHTLY handler:
// Phase 6: call orchestrator.run_evaluation()
// Phase 6b: create trials from phase 3 suggestions
```

The orchestrator is already constructed during app init. Just thread the Arc into the closure.

---

## What Changes vs What Stays

### Changes
| Component | Change |
|-----------|--------|
| Phase 3 Review prompt | Extended with champion params, metrics, experiment history |
| Phase 3 ReviewOutput type | New `trial_suggestions` field |
| Phase 6 in service.rs | Implemented: evaluation + trial creation |
| Reforge cron handler | Captures `Arc<AutoTunerOrchestrator>` |
| ReforgeCollected type | New autotuner context fields |
| Collector | Loads autotuner data when orchestrator available |

### Stays the Same
| Component | Why |
|-----------|-----|
| Shadow scoring hooks | Still collect metrics on every message |
| AutoTunerOrchestrator | Still owns champion state, param sink, evaluation |
| 10 constraint evaluators | Same safety gates |
| Rollback logic | Same 3-day regression threshold |
| ConfigArchiver | Still listens for AutotunerDecision events |
| TrialRepo | Same storage |
| MetricSource | Same metric collection |

---

## Success Criteria

1. **Phase 6 runs evaluation** — promotions and rollbacks work as before
2. **Phase 3 generates trial suggestions** — LLM sees full context (routing + skills + autotuner)
3. **Guardrails prevent bad trials** — validation, diversity, cap, expiry all enforced
4. **No separate autotuner LLM call** — stays at 3 LLM calls total
5. **Champion params propagate live** — memory_param_sink updated on promotion
6. **BrainVersion created** — ConfigArchiver records promotions
7. **Experiment history in prompt** — LLM builds on past results
8. **7-day rolling metrics** — prevents overreaction to single-day noise
9. **All existing tests pass** — shadow scoring, constraint evaluation unchanged
10. **Graceful degradation** — if orchestrator unavailable, Phase 6 skips cleanly
