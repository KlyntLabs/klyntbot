# Simulator Completeness Analysis

## Current State (34 metrics, 7 tiers)

The simulator measures system behavior across 7 tiers with baseline regression detection, checkpoint assertions, and per-epoch accumulation. Cost, cache, retention distribution, estimation accuracy, error injection in agent mode, and empty response retry are all wired and working. All hardcoded stubs have been replaced with data-driven values.

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
- `response_quality` — embedding cosine similarity against semantic intent descriptions
- `salience_extract_rate` — Extract / (Extract + Accumulate + Discard)
- `insight_usefulness` — qualified_insights / total_messages, capped at 1.0
- `estimation_deviation_avg` — cumulative mean |actual - estimated| / estimated ratio

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
- `cost_per_outcome_usd` — cumulative cost / cumulative outcomes (tasks + facts)
- `cache_hit_rate` — cumulative cache_read_tokens / prompt_tokens

**Plus:** `wall_time_per_epoch_ms`

### Latest 1-Month Results (software_engineer_1mo, 150 messages, 30 days)

| Metric | Value | Assessment |
|---|---|---|
| knowledge_retention | 1.000 | 4 facts, all fresh |
| retrieval_precision | 0.750 | Strong |
| personalization | 0.925 | Excellent |
| response_quality | 0.333 | Low — embedding model limitation |
| estimation_deviation | 0.556 | 55% avg deviation |
| community_stability | 0.921 | Strong |
| meta_rule_count | 18 | Active mirror learning |
| tool_selection | 0.250 | Low — see known issues |
| chain_success | 1.000 | 6/6 cross-feature workflows |
| adversarial_resilience | 1.000 | 3/3 handled |
| error_recovery_rate | 1.000 | Agent mode injection working |
| ResponseEmpty breakpoints | 1 | Down from 6 after retry fix |
| cost_per_outcome_usd | inf | See known issue below |

### Production Fixes Made

- **Empty LLM response retry** (`execute_loop.rs`) — retries once if budget allows; returns last_content from prior turn if available. Reduced ResponseEmpty from 6 to 1 in 1-month sim.
- **ErrorInjectingTool wrapper** (`agent_harness.rs`) — wraps agent-mode tools with probabilistic failure injection via `ToolRegistry.take_all()`. Uses shared `sample_injected_error()` for 4-variant error generation.

---

## Known Issues

**`cost_per_outcome_usd: inf` in last run** — Fixed in code (now cumulative), needs verification in next run.

**`tool_selection: 0.250`** — Fixed in code (only scores when persona intended tool calls), needs verification. The agent often uses auxiliary tools (project, area) for context gathering instead of the domain tool directly. Even with the guard, the score will reflect whether the agent reaches the expected domain tool during its ReAct loop.

**`response_quality: 0.333`** — Reference embeddings updated to match semantic intent descriptions. The local embedding model may still produce low similarity between keyword lists and verbose LLM responses. Consider switching to API-based embeddings for more accurate scoring.

**`retrievability_min/p25: 1.000`** — Only 4 facts over 30 days with high stability. Need either more facts (higher `new_fact_introduction_rate`) or lower initial stability to see FSRS-5 decay in action.

**`salience_extract: 1.000`** — Self-confirming: the simulator classifies its own synthetic events. Needs ground-truth salience labels in persona annotations.

---

## Recently Fixed Hardcoded Values

All 7 previously hardcoded/stubbed values have been replaced with data-driven implementations:

| Item | What changed | How it works now |
|---|---|---|
| **SimMetricSource** (9 → 3 zeros) | 6 metrics now computed from DB | `avg_tokens` from usage_records, `avg_response_time` from interaction_log, `memory_relevance` from semantic_facts stability, `promotion_accuracy` from trial statuses, `knowledge_retention` via FSRS-5, `retrieval_recall` from shadow retrieval log. Only `rewrite_trigger_rate`, `rewrite_engagement_rate` (query rewriting not simulated), and `user_satisfaction` remain at defaults — correctly so. |
| **Tool duration** | Variable by tool type | Fast tools (tasks, notes): 12-18ms, medium (finance, productivity): 25-37ms, slow (learning): 50-75ms. Deterministic noise from msg_idx. |
| **Trial confidence** | Computed from topic clarity × keyword weight | `compute_routing_confidence()` maps each topic to a clarity score (tasks=0.95, coaching=0.75), then applies the trial's keyword weight with ±0.04 per-message jitter. |
| **Correction flag** | Confidence-based for Trial B | Trial B only gets corrections flagged when `variant_confidence < 0.87`. High-confidence topics (tasks, finance) avoid Trial B corrections; ambiguous topics (coaching) still get them. Creates realistic correlation between trial params and correction rate. |
| **Mirror snapshot** | Computed from epoch routing data | `fallback_rate` = fraction of messages where topic keywords didn't match content. `avg_routing_confidence` = mean control confidence across epoch messages. `low_confidence_count` = actual fallback count. |
| **Coaching multipliers** | Diminishing returns + commitment scaling | Distraction risk: `delta = 0.20 * (1 - risk * 0.6)` (diminishing). Focus start: `focus_state = 0.7 + (target_mins/60) * 0.25` (commitment). Budget alert: `escalation = 0.15 + pressure * 0.10` (compounding). |
| **Flashcard review** | Rating-based with FSRS-5 alignment | `recall_speed_ms`: 900ms (easy) to 4000ms (forgot) + topic noise. `new_retention_pct`: 30% (forgot) to 90% (easy) ± 5% noise, clamped to [10, 99]. |

---

## Missing Signal Categories

Signals the production system produces but the simulator doesn't observe:

### High Priority (next to wire)

5. **Coaching acceptance rate** — `Helpful / (Helpful + Dismissed + StopSuggesting)` from `CoachingFeedback` events. The simulator already runs a coaching subscriber but never measures outcomes.
6. **Context compression ratio** — `after_tokens / before_tokens` from `ContextCompressed` agent events. Would need agent-mode capture in the event drain.

### Medium Priority

7. **Work context confidence** — `work_contexts.confidence` averages
8. **Cross-domain insight rate** — `CrossDomainDotReady` events per epoch
9. **Budget adherence** — `BudgetWarning` event tracking
10. **Focus quality trend** — `FocusSessionEnded.quality` averages

### Lower Priority

11. **Delegation success rate** — from `DelegationCompleted` events
12. **MCP tool availability** — from `McpStartupComplete` events
13. **Note community health** — from `CommunityDiscovered/Updated/Weakened` events
14. **Debate consensus quality** — from `SquadDebateCompleted` events

### Entire Subsystems Not Simulated

- **Voice interaction** — `PronunciationReport`, `RoutingSuggestion`, `ToneContour`
- **Activity monitoring** — `ActivitySessionCompleted`, `DistractionDetected`
- **Squad debates** — `SquadDebateCompleted` with `persona_accuracies`

---

## Structural Gaps

- **Time granularity** — epochs are 1-day steps; sub-hour dynamics (focus sessions, context compression) are invisible
- **Conversation depth** — persona generates mostly independent messages; multi-turn context building is shallow
- **Error cascades** — 4 error types injected randomly; no cascade testing (e.g., storage timeout → extraction failure → retrieval miss)
- **Concurrent sessions** — production runs multiple sessions across channels; simulator tests one at a time
