# Simulator Completeness Analysis

## Current State (34 metrics, 7 tiers)

The simulator measures system behavior across 7 tiers with baseline regression detection, checkpoint assertions, and per-epoch accumulation. After the signal completeness work, 4 new signal categories were wired in: cost economics, cache utilization, retention distribution, and estimation accuracy.

### Metrics by Tier

**Tier 1 — Memory Fidelity (6 metrics)**
- `knowledge_retention` — fraction of known facts found in DB
- `retrieval_precision` — mean per-query precision
- `retrieval_recall` — mean per-query recall
- `fact_extraction_accuracy` — facts_extracted / facts_introduced
- `contradiction_detection_rate` — cumulative contradictions / facts
- `correction_rate` — user corrections / messages

**Tier 2 — Behavioral Quality (9 metrics)**
- `token_efficiency` — total_tokens / messages
- `personalization_score` — weighted: 0.4x retention + 0.3x precision + 0.3x recall
- `task_completion_rate` — cumulative completed / created
- `routing_stability` — routing_matches / messages
- `routing_accuracy` — agent or heuristic routing correctness
- `response_quality` — embedding cosine similarity against expected
- `salience_extract_rate` — Extract / (Extract + Accumulate + Discard)
- `insight_usefulness` — insight_count / day, capped at 1.0
- `estimation_deviation_avg` — mean |actual - estimated| / estimated ratio for completed tasks

**Tier 3 — System Health (3 metrics)**
- `autotuner_promotion_success` — promoted / terminal trials
- `community_stability` — avg stability from communities table
- `brain_version_velocity` — promoted versions per epoch

**Tier 4 — Cognitive Depth (4 metrics)**
- `memory_retrievability` — FSRS-5 average retrievability across active facts
- `retrievability_min` — worst-case fact retrievability (tail detection)
- `retrievability_p25` — 25th percentile retrievability
- `meta_rule_count` — mirror meta-rules proposed

**Tier 5 — Agent Path (5 metrics)**
- `agent_routing_accuracy`, `agent_tool_selection`, `agent_mode_distribution`
- `react_convergence_rate`, `agent_response_quality`

**Tier 6 — Multi-Turn / Adversarial (4 metrics)**
- `multi_turn_coherence`, `cross_feature_chain_success`
- `adversarial_resilience`, `error_recovery_rate`

**Tier 7 — Cost Economics (2 metrics)**
- `cost_per_outcome_usd` — total cost / (tasks_completed + facts_extracted)
- `cache_hit_rate` — cache_read_tokens / prompt_tokens from usage_records

**Plus:** `wall_time_per_epoch_ms`

### What Was Added (signal completeness work)

| Metric | Source | How It Works |
|---|---|---|
| `cost_per_outcome_usd` | `metrics/cost.rs` queries `usage_records` | `SUM(estimated_cost_usd) / outcomes`; regresses when cost increases |
| `cache_hit_rate` | `metrics/cost.rs` queries `usage_records` | `SUM(cache_read_tokens) / SUM(prompt_tokens)`; included in baseline regression |
| `retrievability_min` / `retrievability_p25` | `metrics/cognitive.rs` distribution function | Sorts all FSRS-5 scores, returns percentiles; catches "average is fine but tail is dying" |
| `estimation_deviation_avg` | Accumulated in `EpochAccumulator` | `abs(actual - estimated) / estimated` per task completion, averaged per epoch |

Supporting changes:
- **SimulationProvider** generates realistic `cache_read_tokens` / `cache_write_tokens` (first call writes ~40% of prompt tokens to cache, subsequent calls read)
- **CompleteTask** variant now carries `estimated_duration_mins` and `actual_duration_mins`; persona generates random estimates (15-120 min) and actuals (10-150 min)
- **ActionExecutor** emits `DomainEvent::EstimationRecorded` alongside `TaskCompleted` when durations are present
- Heuristic-mode `usage_records` INSERT now includes cache tokens and estimated cost
- Single `tokio::join!` consolidates all per-epoch DB queries (cost, cache, retrievability distribution, meta-rules)

---

## Remaining Hardcoded / Stubbed Values

| Location | What's hardcoded | Impact |
|---|---|---|
| `sim_metric_source.rs:85-93` | 9 autotuner metrics return `0.0` | Autotuner evaluation runs on fabricated data |
| `harness.rs` tool_usage INSERT | Tool duration always `10` ms | Real tool latency variance is invisible |
| `harness.rs` Trial B params | Confidence scores `0.85`, `0.92/0.88/0.78` per topic | Trials can't reveal real routing quality |
| `harness.rs` correction flag | `msg_idx % 2 == 0` for Trial B | 50% correction rate is arbitrary |
| `harness.rs` mirror snapshot | `fallback_rate: 0.0`, `avg_routing_confidence: 0.85` | Fabricated mirror data |
| `harness.rs` coaching multipliers | `0.15/0.9/-0.2/1.0/0.2` | Fixed coaching dynamics |
| `actions.rs` flashcard review | `recall_speed_ms: 2000`, `new_retention_pct: rating * 20.0` | Synthetic review quality |

---

## Missing Signal Categories

Signals the production system produces but the simulator doesn't observe:

### High Priority (next to wire)

5. **Coaching acceptance rate** — `Helpful / (Helpful + Dismissed + StopSuggesting)` from `CoachingFeedback` events. The simulator already runs a coaching subscriber but never measures outcomes.
6. **Context compression ratio** — `after_tokens / before_tokens` from `ContextCompressed` agent events. Would need agent-mode capture.

### Medium Priority

7. **Work context confidence** — `work_contexts.confidence` averages
8. **Cross-domain insight rate** — `CrossDomainDotReady` events per epoch
9. **Budget adherence** — `BudgetWarning` event tracking
10. **Focus quality trend** — `FocusSessionEnded.quality` averages
11. **Tool latency distribution** — replace fixed 10ms with realistic variance

### Lower Priority

12. **Delegation success rate** — from `DelegationCompleted` events
13. **MCP tool availability** — from `McpStartupComplete` events
14. **Note community health** — from `CommunityDiscovered/Updated/Weakened` events
15. **Debate consensus quality** — from `SquadDebateCompleted` events

### Entire Subsystems Not Simulated

- **Voice interaction** — `PronunciationReport`, `RoutingSuggestion`, `ToneContour`
- **Activity monitoring** — `ActivitySessionCompleted`, `DistractionDetected`
- **Squad debates** — `SquadDebateCompleted` with `persona_accuracies`

---

## Structural Gaps

- **Time granularity** — epochs are 1-day steps; sub-hour dynamics (focus sessions, context compression) are invisible
- **Conversation depth** — persona generates mostly independent messages; multi-turn context building is shallow
- **Error cascades** — 4 fixed error types injected randomly; no cascade testing (e.g., storage timeout → extraction failure → retrieval miss)
- **Concurrent sessions** — production runs multiple sessions across channels; simulator tests one at a time

---

## Metrics That Don't Discriminate

| Metric | Score | Problem |
|---|---|---|
| `routing_accuracy: 1.000` | Flat skill architecture → all messages → "klyntbot"; no real routing decision tested |
| `salience_extraction: 1.000` | Simulator classifies its own synthetic events — self-confirming |
| `insight_usefulness: 1.000` | Counts existence, not quality (formula: `count / day`, capped at 1.0) |
| `chain_success: 0.000` | Only 2 hardcoded chain patterns in mock provider |
| `adversarial_resilience: 0.000` | Only 4 trivial adversarial patterns (typo, null args, empty ID, fake tool) |

Goal: get every metric into the 0.3-0.9 range where it actually discriminates between good and bad behavior.
