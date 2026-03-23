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
    /// User's current cognitive state. Reuses the existing `UserSituation` from
    /// `cognitive::situation` (fields: energy_level, focus_state, deadline_pressure,
    /// distraction_risk). Passed as Option since situation may not be computed yet.
    pub situation: Option<cognitive::UserSituation>,
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

// NOTE: No UserSituationSnapshot — we reuse cognitive::UserSituation directly.
// Field names are energy_level, focus_state, deadline_pressure, distraction_risk.
// The rewriter reads these fields directly; no translation layer needed.

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

**Note on `cognitive` dependency:** `context_engine` already depends on `cognitive` types
(via `MemoryRetriever`, `MemoryEntry`). Importing `UserSituation` adds no new dependency edge.

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

    // Pronouns are checked first — they always indicate ambiguity,
    // even when named entities are present ("what did John say about that?")
    if has_pronouns {
        return Specificity::Low;
    }

    // After this point, has_pronouns is false. The 1..=3 branch only reaches
    // short non-pronoun queries like "summarize costs" or "check budget".
    match (word_count, has_named_entities) {
        (_, true) if word_count >= 4 => Specificity::High,  // "show me March FIRE projection"
        (1..=3, false) => Specificity::Low,                   // "how are we doing?"
        _ => Specificity::Medium,
    }
}
```

**Precedence rule:** Pronouns always force `Low` regardless of other signals. This prevents
a query like "what did John say about that auth thing?" from being classified as `High`
just because it has named entities and is long enough.

**Heuristic rewriter — signal priority:**

1. **Recent correction** (highest) — directly injects the corrected topic
2. **Active view description** — desktop UI context
3. **Active task title + project** — current work focus
4. **Active skill domain** — domain bias
5. **Recent user message keywords** — top 3 non-stopword terms from last 1-2 messages
6. **Energy-adaptive aggressiveness** (uses `UserSituation` field names):
   - `energy_level < 0.4 OR deadline_pressure > 0.7` → include up to 4 signals
   - `energy_level > 0.7 AND deadline_pressure < 0.3` → include top 2 signals only

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
  │    - active_skill: from SkillRouter result (profile.name())
  │    - active_task: from focused task query (see "Data path for active_task" below)
  │    - recent_user_messages: filter history for user-role msgs, take last 2
  │    - situation: from shared Arc<Mutex<UserSituation>> (see "Data path for situation" below)
  │    - active_view: None (until Phase 4 frontend wiring)
  │    - recent_correction: from new per-session field (see "Data path for corrections" below)
  │
  ├─ Step 6: ContextEngine::assemble(ContextRequest)
  │    ContextRequest now carries: retrieval_context: Option<RetrievalContext>
  │
  │    └─ retrieve_memory(request)
  │         ├─ self.query_rewriter.rewrite(message_text, &retrieval_context)
  │         │    → Option<RewriteResult>
  │         │
  │         └─ InsightForge::retrieve_with_enrichment(
  │              original_query,
  │              enriched_query,      // new parameter
  │              limit, session_key
  │            )
  │            ├─ decomposer.decompose(original) → [orig, dim1, dim2, ...]
  │            ├─ if enriched: insert at position 1
  │            │   → [orig, enriched, dim1, dim2, ...]
  │            └─ fan-out all sub-queries (no changes downstream)
```

### Wiring the QueryRewriter into ContextEngine

`ContextEngine` gets a new optional field and builder method:

```rust
// In ContextEngine struct:
query_rewriter: Option<Arc<dyn QueryRewriter>>,

// Builder method:
pub fn with_query_rewriter(mut self, rewriter: Arc<dyn QueryRewriter>) -> Self {
    self.query_rewriter = Some(rewriter);
    self
}
```

In `retrieve_memory()`, before calling InsightForge:
```rust
let enriched = match (&self.query_rewriter, &request.retrieval_context) {
    (Some(rewriter), Some(ctx)) => rewriter.rewrite(&request.message_text, ctx).await,
    _ => None,
};
```

In `AgentLoopBuilder::build()`, construct `ContextualQueryRewriter` and pass it:
```rust
let query_rewriter = Arc::new(ContextualQueryRewriter::new(rewrite_provider, config));
context_engine = context_engine.with_query_rewriter(query_rewriter);
```

### InsightForge API change

To avoid breaking the existing `retrieve()` signature (used in tests and the fallback path),
add a **new method** rather than modifying the existing one:

```rust
/// Retrieve with an optional enriched query injected as sub_queries[1].
pub async fn retrieve_with_enrichment(
    &self,
    query: &str,
    enriched: Option<&RewriteResult>,
    total_limit: usize,
    session_key: Option<&str>,
) -> Vec<MemoryEntry> {
    // Decompose original
    let mut sub_queries = self.decomposer.decompose(query, None).await;
    // Inject enriched at position 1 if present
    if let Some(result) = enriched {
        sub_queries.insert(1.min(sub_queries.len()), result.enriched_query.clone());
    }
    // Fan-out as today...
}

/// Original method — unchanged, delegates internally.
pub async fn retrieve(&self, query: &str, total_limit: usize, session_key: Option<&str>) -> Vec<MemoryEntry> {
    self.retrieve_with_enrichment(query, None, total_limit, session_key).await
}
```

**Important:** `retrieve_with_enrichment` must preserve the existing timeout and circuit-breaker
logic from `retrieve()`. The implementation should call into the existing timeout-wrapped
decomposition path (not bypass it). The enriched query is inserted into the sub-query list
*after* decomposition completes but *before* the fan-out loop — so the timeout covers
decomposition only, and the enriched query participates in the same fan-out with the same
per-source timeout as all other sub-queries.

```
```

This preserves backward compatibility — existing tests and the fallback path continue to
call `retrieve()` with no changes.

### Data path for `situation` (existing infrastructure, minor extension)

A shared `Arc<Mutex<UserSituation>>` already exists:

1. **Created in `app-core/src/init/agent.rs:64`:** `let user_situation = Arc::new(Mutex::new(UserSituation::default()));`
2. **Passed to `AgentLoopBuilder` at `init/agent.rs:84`:** `.with_user_situation(user_situation.clone())`
3. **Written by the coaching engine at `init/coaching.rs:66`:** `*user_situation.lock().await = real_situation;`
4. **Already wired into `UnifiedMemoryService` at `builder.rs:649-651`** for situational boost scoring.

The `Arc<Mutex<UserSituation>>` is already threaded through the agent builder. To make it
available at Step 5.5 in `AgentRuntime`, we need one extension:

- **Add the `Arc<Mutex<UserSituation>>` as an optional field on `AgentRuntime`** (not just
  on `UnifiedMemoryService`). The builder already has it — just pass it through.
- **In Step 5.5**, read `situation.lock().await.clone()` → `RetrievalContext.situation`.
- **Fallback:** On cold start (before coaching engine computes the first situation),
  `UserSituation::default()` has all fields at 0.0. The rewriter treats this as "no strong
  signal" and uses default aggressiveness (same as medium energy).

Estimated: ~10 lines (field on `AgentRuntime` + builder wire + read in Step 5.5).
The coaching engine writes periodically; no new write path needed.

### Data path for `active_task` (new lightweight query)

No existing field carries "active task." We add a lightweight query:

Promote `active_task` to **hot shared state** (same pattern as `UserSituation`) rather than
querying the DB on every message:

1. **Add `Arc<Mutex<Option<ActiveTaskContext>>>` to `AgentLoop` / `AgentRuntime`.**
   Initialize as `None`.
2. **Update it reactively** — when the focus slot changes (via `TaskTool::focus()` or
   `TaskTool::unfocus()`), publish a `DomainEvent::TaskFocusChanged` and have the
   `AgentLoop` update the shared state. This avoids per-message DB queries and makes
   "what's left on that?" feel instant during deep work sessions.
3. **Fallback on cold start:** If the shared state is `None` (no focus event yet), do a
   one-time `TaskRepo::list_focused().await?.into_iter().next()` to hydrate. After that,
   the shared state is authoritative.
4. **Map to `ActiveTaskContext`**: `title = task.title`, `project_name` from task's project
   relation (if loaded), `domain` inferred from the active skill name.

Estimated: ~30 lines (shared state field + event handler + cold-start hydration + mapping).
Slightly more than a per-message query but eliminates DB round-trips on every message and
ensures the agent always has instant access to the current focus.

### Data path for `recent_correction` (new per-session state)

Correction detection in `AgentLoop` currently fires a `DomainEvent` and doesn't store the
result. We add a lightweight forwarding mechanism:

1. **Add `last_correction: Option<CorrectionContext>` as a field on `AgentLoop` itself.**
   This is session-scoped state (one `AgentLoop` per active session).
2. **Set it in `process_message()` / `process_direct_streaming()`** where correction
   detection already runs (alongside the existing `DomainEvent` publish). The detection
   has access to `original` (prior assistant message) and `correction` (the user's new
   message text). Map directly:
   - `rejected_topic`: extract key terms from `original` (the assistant response that was wrong)
   - `corrected_to`: use the **raw user correction text** as-is (e.g., "no, the GraphQL migration")

   The heuristic rewriter treats `corrected_to` as a keyword source — it extracts domain
   terms from the raw string (e.g., "GraphQL migration" from "no, the other one — the
   GraphQL migration"). No topic-resolution step is needed for Phase 1; the raw text
   provides sufficient signal for template injection. Phase 2's LLM fallback can do
   more sophisticated extraction if the raw text is too noisy.
3. **Thread it through `run_pipeline()`** — `run_pipeline` is the intermediary between
   `AgentLoop::process_message()` and `AgentRuntime::process_message()`. Add
   `correction: Option<CorrectionContext>` as a parameter to `run_pipeline()`. The runtime
   receives it and includes it in `RetrievalContext`.
4. **NOT via `RoutingContext`** — `RoutingContext` is defined in `tools-core` (L1) and
   is used for routing decisions, not transient session state. Adding correction context
   there would pollute a widely-used shared type. Instead, `run_pipeline()` carries it
   as a separate parameter.
5. **Clear after one use** — after building `RetrievalContext`, set `self.last_correction = None`.
   The correction signal applies only to the immediately next message.

Estimated: ~20 lines (field on AgentLoop + set in correction detect + parameter on
run_pipeline + read in runtime + clear).

### Files changed (revised estimates)

| File | Change | Lines |
|------|--------|-------|
| `context_engine/src/rewriter.rs` | New: `QueryRewriter` trait, `RewriteResult`, `RewriteSource` | ~25 |
| `context_engine/src/types.rs` | Add `RetrievalContext`, `ActiveTaskContext`, `ActiveView`, `CorrectionContext` | ~45 |
| `context_engine/src/assembler/types.rs` | `ContextRequest` gains `retrieval_context: Option<RetrievalContext>`. All existing struct literal instantiations (tests, call sites) need `retrieval_context: None` added — there are ~9 such sites in `assembler/mod.rs` tests alone | ~15 |
| `context_engine/src/assembler/mod.rs` | Add `query_rewriter` field + `with_query_rewriter()`. `retrieve_memory()` calls rewriter. Update `compute_cache_key()` to include retrieval context | ~35 |
| `context_engine/src/insight_forge/mod.rs` | Add `retrieve_with_enrichment()` method. Refactor existing `retrieve()` to delegate. Update fallback path | ~40 |
| `agent/src/adapters/query_rewriter.rs` | New: `ContextualQueryRewriter` (specificity, heuristic templates, LLM fallback) | ~280 |
| `agent/src/agent_runtime/runtime.rs` | Step 5.5: build `RetrievalContext`, pass into `ContextRequest` | ~40 |
| `agent/src/agent_loop/mod.rs` | Add `last_correction` field, set on detection, pass to runtime, clear after use | ~15 |
| `agent/src/agent_loop/builder.rs` | Wire `QueryRewriter` into `ContextEngine`, thread `TaskRepo` + situation state | ~20 |

**Total: ~505 lines of new/changed code** (revised from ~400 after accounting for data path
infrastructure, InsightForge backward compat, and cache key updates).

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

The `compute_cache_key()` hash is updated to include retrieval context signals:

```rust
// Add to compute_cache_key():
if let Some(ref ctx) = request.retrieval_context {
    if let Some(ref skill) = ctx.active_skill {
        hasher.update(skill.as_bytes());
    }
    if let Some(ref task) = ctx.active_task {
        hasher.update(task.title.as_bytes());
    }
    if let Some(ref correction) = ctx.recent_correction {
        hasher.update(correction.corrected_to.as_bytes());
    }
    // active_view.description and situation are NOT included in cache key:
    // - situation changes continuously (energy, focus); including it would
    //   defeat caching entirely. Accepted tradeoff: two messages <60s apart
    //   with different energy states get the same enrichment. This is acceptable
    //   because rewrite confidence ranges overlap across energy levels, and the
    //   6-factor FSRS scoring (which DOES use live situation) compensates at
    //   ranking time.
    // - active_view changes rarely mid-conversation. The 60s cache TTL provides
    //   sufficient freshness.
}
```

This ensures that switching skills or tasks mid-session produces different cache keys,
preventing stale enriched results. Situation and view are excluded to avoid cache thrashing
(they change continuously).

**Adaptive TTL:** When a rewrite was produced (enriched_query is Some), the cache TTL for
that entry is shortened to **30 seconds** (instead of the default 60s). This ensures the
agent stays tuned to the user's current energy/view during the moments that matter most —
low-energy afternoons, dashboard synergy. When no rewrite happened (specificity=High),
the standard 60s TTL applies. Implementation: tag cached entries with a `rewritten: bool`
flag and check it on TTL expiry.

### Dependency inversion

```
context_engine (L3): defines QueryRewriter trait + RetrievalContext types
                     holds Option<Arc<dyn QueryRewriter>> in ContextEngine
                     calls rewriter.rewrite() in retrieve_memory()

agent (L5):          implements ContextualQueryRewriter
                     constructed with: DynProvider (for LLM fallback, configurable model),
                     RewriterConfig (timeout, aggressiveness, template set)
                     wired via AgentLoopBuilder → context_engine.with_query_rewriter()
```

Follows the same pattern as `ExtractionHandler`, `ConsolidationHandler`, `ReflectionHandler`,
`SemanticFactEmbedder`. The LLM provider for the rewriter is a **separate config entry**
(`config.agents.rewriterModel` — camelCase per Config convention) defaulting to the
cheapest/fastest available model — NOT the primary conversation model.

### LLM fallback — background race pattern (zero perceived latency)

The rewriter uses a **heuristic-first, background-LLM race** pattern:

1. **Heuristic always runs first** (instant, ~0ms). If it produces an enrichment,
   return it immediately. InsightForge begins fan-out with the heuristic enrichment.
2. **If heuristic returns None AND specificity=Low**, spawn the LLM call as a background
   task (`tokio::spawn`) with an 800ms timeout. Meanwhile, InsightForge begins fan-out
   with the original query only (no enriched sub-query yet).
3. **If the LLM finishes before InsightForge fan-out completes**, inject the LLM-enriched
   query as a late sub-query into the still-running fan-out. InsightForge's parallel
   architecture supports this — sub-queries are independent tasks joined at the end.
4. **If InsightForge finishes first**, discard the pending LLM result. The response uses
   original-only retrieval. No delay added.

This ensures **zero perceived latency on 100% of queries**:
- Heuristic path: 0ms (70% of rewrites)
- LLM path: 0ms perceived (LLM races with InsightForge in background)
- Worst case: InsightForge finishes before LLM → same as no rewrite (graceful)
- Best case: LLM finishes during fan-out → enriched results merged in at no cost

The 800ms timeout on the LLM call is still applied via `tokio::time::timeout` to prevent
resource leaks on slow models. The `DynProvider::chat()` call is the same as other LLM
fallback paths in the codebase.

**Implementation note:** InsightForge's fan-out uses `join_all()` on sub-query tasks. To
support late injection, the enriched query task is added to the `join_all` set if the LLM
completes before the join begins. If the join is already in progress, the result is discarded.
This requires a small refactor of the fan-out to use `tokio::select!` between the join and
the LLM channel, or to pre-allocate a slot for the enriched query that resolves to empty
if the LLM doesn't finish in time.

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
| `llm_background_race_discards_late` | If InsightForge finishes before LLM → LLM result discarded, no error |
| `recent_messages_extract_keywords` | History about "auth middleware" → keywords injected |
| `skill_domain_injected` | finance-management → "finance" in enriched |
| `task_title_injected` | "March budget review" → appears in enriched |
| `confidence_reflects_source` | Heuristic: 0.7-0.9; LLM: 0.6-0.85 |

### Integration tests (~8)

| Test | Validates |
|------|-----------|
| `rewrite_improves_fact_retrieval` | "what about that project?" + task context → retrieves correct facts |
| `rewrite_augments_not_replaces` | Both original and enriched appear in InsightForge fan-out |
| `no_rewriter_degrades_gracefully` | `query_rewriter: None` → system works as today |
| `enriched_deduplicates_via_rrf` | Same fact from both queries → single entry in results |
| `end_to_end_moment_2` | "how are we doing?" + finance skill → finance facts in top 3 |
| `cache_key_varies_by_skill` | Same message with different `active_skill` → different cache keys |
| `cache_key_varies_by_task` | Same message with different `active_task` → different cache keys |
| `retrieve_with_enrichment_backward_compat` | `InsightForge::retrieve()` still works with no enrichment arg |

---

## Metrics

### Primary: Mind-Reading Rate
% of rewritten queries where the user's next action is natural continuation (no clarification asked, stays in flow).
- **Baseline:** Measure for 2 weeks pre-rollout
- **Target:** +25% in Phase 2

### Secondary: Retrieval Engagement Rate
`referenced_memory_count / retrieved_memory_count` per message.
- **Target:** +15%

### Tertiary: Memory Flywheel Strength
Average salience/consolidation score of memories retrieved via enriched queries vs
original-only queries, measured over a rolling 2-week window. This directly measures the
virtuous loop: better retrieval → richer conversation history → smarter consolidation →
higher-quality facts → even better future retrieval. If this metric trends upward over
weeks, the second brain is genuinely *growing* with the user, not just getting faster.
- **Target:** Positive trend over first 4 weeks of Phase 2

### Operational

| Metric | Target |
|--------|--------|
| Clarification drop rate | -30% vs baseline |
| Rewrite trigger rate | 25-35% of messages |
| Respectfully-lazy skip rate | 40-50% (High specificity) — **measure actual distribution in Phase 1 before setting Phase 2 targets** |
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
