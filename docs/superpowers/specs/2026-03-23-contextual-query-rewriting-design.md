# Contextual Query Rewriting — Design Spec

**Date:** 2026-03-23
**Status:** Approved
**Scope:** New `QueryRewriter` component in the retrieval pipeline that enriches vague queries with contextual signals before vector search, improving retrieval relevance across all features.

---

## Problem

The retrieval pipeline currently passes the user's raw message string — unmodified — through every layer from `AgentRuntime` to `LanceDB` vector search. Contextual signals (active skill, current task, user energy/focus, desktop view state, recent conversation) are only applied *after* search during scoring (6-factor FSRS formula, situational boost). This means the vector search itself has no awareness of what the user is doing, looking at, or referring to.

For a personal agent that should feel like a second brain, this creates friction:
- Pronoun-heavy queries ("what was that thing?") retrieve irrelevant results
- Vague follow-ups ("how are we doing?") lack domain grounding
- Cross-domain references ("finish that") miss the active task context
- Desktop dashboard context is invisible to retrieval

The retrieval architecture is strong *post-search* (hybrid vector+BM25, FSRS decay, situational scoring, RRF fusion). The gap is *pre-search* — the query itself carries none of the agent's understanding of the user's current state.

## Solution

A standalone `QueryRewriter` component that sits between intent analysis and InsightForge. It produces an **enriched query** that augments (never replaces) the original, injected as an extra sub-query into InsightForge's existing fan-out. The original query is always preserved as `sub_queries[0]`.

### Design Principles

1. **Augment, never replace.** The original query is the user's voice; the enriched version is the agent's understanding. Both are searched. RRF naturally handles overlap and divergence.
2. **Respectfully lazy.** Explicit, specific queries skip enrichment entirely. The agent doesn't try too hard on clear requests.
3. **Energy-adaptive.** Low energy (< 0.4) or high deadline pressure (> 0.7) triggers more aggressive enrichment — the second brain carries heavier cognitive load when the user is drained.
4. **100% invisible.** No UI hints, no "I enriched your query" messages. Ever. The delight comes from results being eerily good.
5. **Graceful degradation.** Every contextual signal is optional. Missing signals are skipped, not faked.

---

## User Moments (Success Criteria)

### Moment 1 — Pronoun resolution, low-energy afternoon
> *4:15pm, energy=0.3. Last 2 messages about auth middleware refactoring.*
> User: "what was that thing we discussed about the auth layer?"
>
> **Expected:** LLM fallback resolves "that thing" using recent messages → enriched query: "Recall our recent discussion on auth middleware refactoring and session token changes." Both original and enriched searched. RRF surfaces the exact conversation + compliance fact.

### Moment 2 — Ambiguous query in finance skill
> *finance-management skill active. Task: "March budget review." deadline_pressure=0.8.*
> User: "how are we doing?"
>
> **Expected:** Heuristic enrichment (skill + task + deadline) → "March budget review — current spending progress and status." Finance facts surface immediately.

### Moment 3 — Cross-domain follow-up during task planning
> *task-management skill. Previous messages about API migration project subtasks.*
> User: "what's left on that?"
>
> **Expected:** LLM fallback (pronoun + history) → "What are the remaining subtasks on the API migration project?" TaskSearcher + cognitive facts both find the right project.

### Moment 4 — Explicit query (respectfully lazy)
> User: "show me March FIRE projection"
>
> **Expected:** Specificity check returns High → `None`. Zero enrichment. Zero overhead.

### Moment 5 — Time-anchored recall
> *Yesterday's conversation: REST→GraphQL decision.*
> User: "what did I say about that yesterday?"
>
> **Expected:** LLM fallback → "REST to GraphQL API migration decision rationale yesterday." Time reference preserved in original; semantic content in enriched. RRF merges conversation recall (time-favored) with architectural decision fact (semantically matched).

### Moment 6 — Dashboard + chat synergy (desktop differentiator)
> *Finance dashboard open. March FIRE projection widget visible. No chat history.*
> User: "break this down"
>
> **Expected:** Heuristic (active_view description) → "Explain the March 2026 FIRE projection, focusing on variance, risks and opportunities." Desktop view becomes retrieval context.
> **Note:** Requires Phase 4 frontend wiring. Until then, `active_view` is `None` and rewriter skips this signal.

### Moment 7 — Post-correction learning
> *User corrected "no, the other one — the GraphQL migration." Next message:*
> User: "any blockers?"
>
> **Expected:** Heuristic (correction is #1 priority) → "What are the current blockers on the GraphQL migration project?" Correction signal dominates template.

### Moment 8 — Spaced-repetition coaching recall
> *During coaching session. Referring to a 3-month-old decision flagged by procedural memory.*
> User: "remind me why we decided that"
>
> **Expected:** LLM fallback (coaching context + "why we decided") → enriched query surfaces the exact rationale from semantic/procedural layers using salience decay + current learning goal.

---

## Architecture

### New Types (defined in `context_engine`)

```rust
/// Rich context available at retrieval time for query enrichment.
/// Each field is optional — the rewriter gracefully degrades when signals are missing.
pub struct RetrievalContext {
    /// Active orchestrator skill (e.g., "finance-management", "task-management")
    pub active_skill: Option<String>,
    /// Current task or project the user is working on
    pub active_task: Option<ActiveTaskContext>,
    /// Last N user messages (not assistant) for pronoun/reference resolution
    pub recent_user_messages: Vec<String>,
    /// User's current cognitive state
    pub situation: Option<UserSituationSnapshot>,
    /// Desktop UI state — what the user is looking at right now
    pub active_view: Option<ActiveView>,
    /// Recent correction: if the user just rejected a result, what did they correct to?
    pub recent_correction: Option<CorrectionContext>,
}

pub struct ActiveTaskContext {
    pub title: String,
    pub project_name: Option<String>,
    pub domain: Option<String>,
}

pub struct UserSituationSnapshot {
    pub energy: f64,
    pub focus: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
}

pub struct ActiveView {
    pub dashboard: String,
    pub focused_entity: Option<String>,
    /// High-level semantic description of what the user is focused on,
    /// e.g. "March 2026 FIRE projection with variance highlighted"
    pub description: Option<String>,
}

pub struct CorrectionContext {
    pub rejected_topic: String,
    pub corrected_to: String,
}
```

### QueryRewriter Trait (defined in `context_engine`)

```rust
pub struct RewriteResult {
    pub enriched_query: String,
    pub confidence: f32,
    pub source: RewriteSource,
}

pub enum RewriteSource {
    Heuristic,
    Llm,
}

#[async_trait]
pub trait QueryRewriter: Send + Sync {
    /// Attempt to produce an enriched query given the original and available context.
    /// Returns None if the original is already specific enough (respectfully lazy).
    async fn rewrite(
        &self,
        original: &str,
        context: &RetrievalContext,
    ) -> Option<RewriteResult>;
}
```

### Implementation: `ContextualQueryRewriter` (in `agent`)

**Decision cascade:**

```
rewrite(original, context)
  │
  ├─ specificity_check(original)
  │    High → return None (respectfully lazy)
  │    Medium → heuristic only, skip LLM
  │    Low → heuristic then LLM fallback
  │
  ├─ heuristic_rewrite(original, context)
  │    → semantic fusion templates using available signals
  │    → if produced enrichment: return Some(RewriteResult { confidence: 0.7-0.9, source: Heuristic })
  │
  └─ llm_rewrite(original, context)  (only if specificity=Low and heuristic returned None)
       → small/fast model, 800ms timeout
       → return Some(RewriteResult { confidence: 0.6-0.85, source: Llm }) or None
```

**Specificity check:**

```rust
fn query_specificity(query: &str, context: &RetrievalContext) -> Specificity {
    let word_count = query.split_whitespace().count();
    let has_pronouns = contains_pronouns(query);
    let has_named_entities = has_domain_keywords(query);

    match (word_count, has_pronouns, has_named_entities) {
        (_, false, true) if word_count >= 4 => Specificity::High,
        (1..=3, _, false) => Specificity::Low,
        (_, true, _) => Specificity::Low,
        _ => Specificity::Medium,
    }
}
```

**Heuristic rewriter — signal priority:**

1. **Recent correction** (highest) — directly injects the corrected topic
2. **Active view description** — desktop UI context
3. **Active task title + project** — current work focus
4. **Active skill domain** — domain bias
5. **Recent user message keywords** — top 3 non-stopword terms from last 1-2 messages
6. **Energy-adaptive aggressiveness:**
   - `energy < 0.4 OR deadline_pressure > 0.7` → include up to 4 signals
   - `energy > 0.7 AND deadline_pressure < 0.3` → include top 2 signals only

**Assembly uses semantic fusion templates** (not keyword concatenation):

| Query pattern | Template example |
|---------------|-----------------|
| Question + skill context | "{task_title} — current {domain} progress and status" |
| Pronoun + recent messages | "Recall our recent discussion on {recent_keywords}" |
| Command + active view | "Explain the {view_description}, focusing on {query_verb} aspects" |
| Follow-up + correction | "What are the current {query_topic} on the {corrected_to} project?" |
| Time reference + history | "{resolved_topic} {time_reference}" |

Templates produce natural-language strings optimized for embedding similarity. ~8 templates covering the major query patterns.

**LLM fallback prompt:**

```
You are a query rewriter for a personal AI assistant. Given the user's
vague query and their current context, produce a single enriched search
query that captures what they likely mean.

Rules:
- Output ONLY the rewritten query, nothing else
- Keep it under 20 words
- Preserve any time references from the original
- If the query is already clear enough, output "SKIP"

User's query: "{original}"

Context:
- Active skill: {skill or "none"}
- Current task: {task_title or "none"}
- Recent messages: {last 2 user messages, truncated to 100 chars each}
- Current view: {active_view.description or "none"}

Rewritten query:
```

Timeout: **800ms hard cap.** On timeout → return `None`.

---

## Pipeline Integration

### Data flow

```
AgentRuntime::process_message()
  ├─ Step 1: SkillRouter → active_skill
  ├─ Step 4: IntentAnalyzer → analysis
  │
  ├─ Step 5.5 (NEW): Build RetrievalContext
  │    - active_skill from SkillRouter
  │    - active_task from session/RoutingContext
  │    - recent_user_messages: last 2 user-role msgs from history
  │    - situation: snapshot from shared UserSituation
  │    - active_view: from shared desktop state (None until Phase 4)
  │    - recent_correction: from AgentLoop correction detection
  │
  ├─ Step 6: ContextEngine::assemble(ContextRequest)
  │    ContextRequest now carries: retrieval_context: Option<RetrievalContext>
  │
  │    └─ retrieve_memory(request)
  │         ├─ QueryRewriter::rewrite(message_text, &retrieval_context)
  │         │    → Option<RewriteResult>
  │         │
  │         └─ InsightForge::retrieve(
  │              original_query,
  │              enriched_query,      // new parameter
  │              limit, session_key
  │            )
  │            ├─ decomposer.decompose(original) → [orig, dim1, dim2, ...]
  │            ├─ if enriched: insert at position 1
  │            │   → [orig, enriched, dim1, dim2, ...]
  │            └─ fan-out all sub-queries (no changes downstream)
```

### Files changed

| File | Change | Lines |
|------|--------|-------|
| `context_engine/src/types.rs` | Add `RetrievalContext`, `ActiveTaskContext`, `UserSituationSnapshot`, `ActiveView`, `CorrectionContext`, `RewriteResult`, `RewriteSource` | ~60 |
| `context_engine/src/rewriter.rs` | New: `QueryRewriter` trait | ~20 |
| `context_engine/src/assembler/mod.rs` | `ContextRequest` gains `retrieval_context`. `retrieve_memory()` calls rewriter | ~20 |
| `context_engine/src/insight_forge/mod.rs` | `retrieve()` gains `enriched_query: Option<&str>`, inserts at sub_queries[1] | ~10 |
| `agent/src/adapters/query_rewriter.rs` | New: `ContextualQueryRewriter` (specificity check, heuristic templates, LLM fallback) | ~250 |
| `agent/src/agent_runtime/runtime.rs` | Step 5.5: build `RetrievalContext`, pass into `ContextRequest` | ~30 |
| `agent/src/agent_loop/mod.rs` | Expose `recent_correction` to runtime | ~10 |

**Total: ~400 lines of new/changed code.**

### What does NOT change

- `UnifiedMemoryService` — receives sub-queries from InsightForge as today
- `retrieve_relevant_facts` — scores and ranks as today
- `ConversationRecallService` — searches as today
- All `DomainSearcher` implementations — unchanged
- RRF merge logic — unchanged
- BudgetAllocator, HistoryCompressor — unchanged
- 6-factor FSRS scoring formula — unchanged
- Response validation — unchanged

### Cache behavior

The rewrite happens inside `assemble_uncached()` (after cache miss). Same message + same history = same cache key = rewritten results reused. No caching changes needed.

### Dependency inversion

```
context_engine (L3): defines QueryRewriter trait + RetrievalContext types
agent (L5): implements ContextualQueryRewriter (needs DynProvider for LLM fallback)
```

Follows the same pattern as `ExtractionHandler`, `ConsolidationHandler`, `ReflectionHandler`, `SemanticFactEmbedder`.

---

## Testing

### Unit tests (~15)

| Test | Validates |
|------|-----------|
| `high_specificity_returns_none` | "show me March FIRE projection" → None (Moment 4) |
| `low_energy_increases_signals` | energy=0.3 → more context injected than energy=0.9 |
| `deadline_pressure_increases_signals` | deadline_pressure=0.8 → same as low energy |
| `pronouns_trigger_low_specificity` | "what was that thing?" → Specificity::Low |
| `correction_is_highest_priority` | With correction + skill + task, correction dominates |
| `active_view_enriches_query` | "break this down" + view → enriched includes view description |
| `no_context_returns_none` | Empty RetrievalContext + Medium specificity → None |
| `heuristic_uses_semantic_templates` | Enriched reads as natural language, not keyword soup |
| `llm_fallback_on_pronoun_no_context` | Pronouns + empty heuristic → LLM triggered |
| `llm_skip_response_returns_none` | LLM returns "SKIP" → None |
| `llm_timeout_returns_none` | LLM exceeds 800ms → None |
| `recent_messages_extract_keywords` | History about "auth middleware" → keywords injected |
| `skill_domain_injected` | finance-management → "finance" in enriched |
| `task_title_injected` | "March budget review" → appears in enriched |
| `confidence_reflects_source` | Heuristic: 0.7-0.9; LLM: 0.6-0.85 |

### Integration tests (~5)

| Test | Validates |
|------|-----------|
| `rewrite_improves_fact_retrieval` | "what about that project?" + task context → retrieves correct facts |
| `rewrite_augments_not_replaces` | Both original and enriched appear in InsightForge fan-out |
| `no_rewriter_degrades_gracefully` | `query_rewriter: None` → system works as today |
| `enriched_deduplicates_via_rrf` | Same fact from both queries → single entry in results |
| `end_to_end_moment_2` | "how are we doing?" + finance skill → finance facts in top 3 |

---

## Metrics

### Primary: Mind-Reading Rate
% of rewritten queries where the user's next action is natural continuation (no clarification asked, stays in flow).
- **Baseline:** Measure for 2 weeks pre-rollout
- **Target:** +25% in Phase 2

### Secondary: Retrieval Engagement Rate
`referenced_memory_count / retrieved_memory_count` per message.
- **Target:** +15%

### Operational

| Metric | Target |
|--------|--------|
| Clarification drop rate | -30% vs baseline |
| Rewrite trigger rate | 25-35% of messages |
| Respectfully-lazy skip rate | 40-50% (High specificity) |
| LLM fallback rate | <30% of rewritten queries |
| Enriched RRF contribution | >50% when triggered |
| Correction effectiveness (Moment 7) | >80% improvement on next query |
| Latency P50 | <1ms (heuristic path) |
| Latency P99 | <800ms (LLM path) |

---

## Rollout Phases

### Phase 1 — Heuristic-only
- Deploy `ContextualQueryRewriter` with heuristic path only, LLM disabled
- Covers Moments 2, 3, 4, 6, 7 (skill/task/correction/view context)
- Zero latency overhead, zero cost increase
- Measure baselines for all metrics

### Phase 2 — LLM fallback enabled
- Enable LLM fallback for Low-specificity queries (Moments 1, 5, 8)
- Small model (Haiku-class), 800ms timeout
- Monitor latency impact and LLM fallback rate
- Tune trigger conditions from Phase 1 data

### Phase 3 — Autotuner integration
- Rewrite confidence, trigger thresholds, template aggressiveness as tunable params
- Per-skill/energy experiments via champion overrides
- Track which `RewriteSource` produces better retrieval engagement

### Phase 4 — Desktop view wiring
- Wire `ActiveView` from Tauri frontend → agent runtime → RetrievalContext
- Enables Moment 6 (dashboard + chat synergy)
- Rewriter already handles it — just needs the data path
