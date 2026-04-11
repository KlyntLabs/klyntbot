# Query Enhancement Pipeline — Design Spec

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this spec task-by-task.

**Goal:** Replace the single-stage query rewriter with a multi-stage enhancement pipeline that separates query transformation from result reranking, gated by an explicit cost budget derived from execution depth mode.

**Architecture:** Two distinct pipelines — `QueryPipeline` (query text → `QueryBundle`) and `RankingPipeline` (`Vec<ScoredFact>` → reordered `Vec<ScoredFact>`) — with retrieval as the explicit boundary between them. An `EnhancementBudget` struct controls how much intelligence each depth mode spends. Stages degrade gracefully on failure.

**Tech Stack:** Rust async traits, existing `MemoryRetriever` / `InsightForge` infrastructure, existing 12-factor scorer, LLM calls via `DynProvider`.

---

## 1. Problem Statement

The current `ContextualQueryRewriter` (1399 lines, `crates/agent/src/adapters/query_rewriter.rs`) is functional — it has heuristic signal enrichment with 5-priority hierarchy, LLM fallback for low-specificity queries, and autotuner integration. However:

1. **No reranking stage** — retrieved results are never re-scored after initial retrieval. Co-activation and recency signals are underweighted.
2. **No pseudo-relevance feedback** — the system never examines initial results to improve the query. This is a zero-cost technique that consistently improves recall.
3. **No multi-query expansion** — ambiguous queries produce a single retrieval pass. Generating variants and merging via RRF dramatically improves recall.
4. **`CorrectionContext` is defined but never populated** — Priority 1 signal (confidence 0.9) is dead code.
5. **Mixed concerns** — query enrichment and the background race pattern (`rewrite_or_spawn`, `late_rx`) are tangled together in the assembler.
6. **No cost control** — the LLM fallback has a hardcoded timeout but no explicit budget model for latency or token spend.

## 2. Architecture

### 2.1 Two-Pipeline Design

```
User Query + RetrievalContext + DepthMode
    │
    ▼
┌─────────────────────────────────────┐
│         QueryPipeline               │  Query → QueryBundle
│  ┌───────────────────────────────┐  │
│  │ Stage 1: Signal Enrichment    │  │  existing rewriter logic + CorrectionContext
│  │ Stage 2: PRF Expansion        │  │  heuristic, top-3, 0.6 threshold
│  │ Stage 3: Multi-Query (Deep+)  │  │  LLM generates 3 variants
│  └───────────────────────────────┘  │
│  Returns: QueryBundle               │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│         Retrieval                   │  QueryBundle → Vec<ScoredFact>
│  (InsightForge / MemoryRetriever)   │  existing infrastructure
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│         RankingPipeline             │  Vec<ScoredFact> → Vec<ScoredFact>
│  ┌───────────────────────────────┐  │
│  │ Normal: Heuristic rescore     │  │  co-activation + recency + term overlap
│  │ Deep:   LLM cross-encoder     │  │  pairwise relevance scoring (top-10)
│  └───────────────────────────────┘  │
│  Returns: Vec<ScoredFact> (reordered)│
└──────────────┬──────────────────────┘
               │
               ▼
           EnhancementOutput
```

**Key boundary:** `QueryPipeline` only transforms query text. `RankingPipeline` only reorders results. Retrieval sits between them as an explicit boundary. This separation makes each pipeline independently testable, cacheable, and extensible.

### 2.2 Core Types

All types live in `context_engine/src/enhancement/types.rs`:

```rust
/// A bundle of query variants produced by the QueryPipeline.
pub struct QueryBundle {
    /// The original user query, always preserved for debugging/fallback.
    pub original: String,
    /// The primary query after enrichment (may equal original if no enrichment).
    pub primary: String,
    /// Additional query variants from PRF and multi-query expansion.
    pub variants: Vec<String>,
    /// Confidence in the enrichment (0.0 = passthrough, 1.0 = high).
    pub confidence: f32,
    /// Which stages contributed to this bundle.
    pub sources: Vec<QuerySource>,
}

pub enum QuerySource {
    Passthrough,
    SignalEnrichment,
    PseudoRelevanceFeedback,
    MultiQuery,
    HeuristicRerank,
    LlmRerank,
}

/// Budget envelope for enhancement stages.
/// Replaces hardcoded DepthMode matching with an explicit cost model.
pub struct EnhancementBudget {
    pub max_latency_ms: u64,
    pub max_llm_calls: u32,
    pub max_expansion_tokens: usize,
}

impl EnhancementBudget {
    pub fn normal() -> Self {
        Self { max_latency_ms: 100, max_llm_calls: 0, max_expansion_tokens: 0 }
    }
    pub fn deep_think() -> Self {
        Self { max_latency_ms: 500, max_llm_calls: 2, max_expansion_tokens: 200 }
    }
    pub fn ultra() -> Self {
        Self { max_latency_ms: 1000, max_llm_calls: 4, max_expansion_tokens: 400 }
    }
}

impl From<DepthMode> for EnhancementBudget {
    fn from(mode: DepthMode) -> Self {
        match mode {
            DepthMode::Normal => Self::normal(),
            DepthMode::DeepThink => Self::deep_think(),
            DepthMode::Ultra => Self::ultra(),
        }
    }
}

/// Final output from the full enhancement flow.
pub struct EnhancementOutput {
    pub query: QueryBundle,
    pub candidates: Option<Vec<ScoredFact>>,
    pub trace: EnhancementTrace,
}

impl EnhancementOutput {
    /// Passthrough — no enhancement, just wrap the original query.
    pub fn passthrough(query: &str) -> Self {
        Self {
            query: QueryBundle {
                original: query.to_string(),
                primary: query.to_string(),
                variants: vec![],
                confidence: 0.0,
                sources: vec![QuerySource::Passthrough],
            },
            candidates: None,
            trace: EnhancementTrace::default(),
        }
    }
}
```

### 2.3 Pipeline Traits

```rust
/// Query transformation stage: QueryBundle → QueryBundle
pub trait QueryStage: Send + Sync {
    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> Result<QueryBundle>;
}

/// Result reranking stage: Vec<ScoredFact> → Vec<ScoredFact>
pub trait RankingStage: Send + Sync {
    async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<ScoredFact>,
        budget: &EnhancementBudget,
    ) -> Result<Vec<ScoredFact>>;
}
```

### 2.4 Graceful Degradation

Every stage follows the same error handling pattern — failure passes through the previous output unchanged:

```rust
// In QueryPipeline::enhance():
for stage in &self.stages {
    match stage.transform(bundle.clone(), ctx, budget).await {
        Ok(output) => {
            trace.record_success(stage_name, elapsed);
            bundle = output;
        }
        Err(e) => {
            tracing::warn!("Enhancement stage {stage_name} failed: {e}");
            trace.record_failure(stage_name, elapsed, e.to_string());
            // bundle unchanged — continue with what we have
        }
    }
}
```

No stage failure stops the pipeline. The trace records what ran and what degraded.

## 3. Stage Implementations

### 3.1 Signal Enrichment (Stage 1) — always on

**Location:** `agent/src/adapters/signal_enrichment.rs`

Thin adapter wrapping the existing `ContextualQueryRewriter::heuristic_rewrite()`. No new logic — just bridges the old `RewriteResult` into the new `QueryBundle`.

Behavior:
- Classifies query specificity (High → passthrough, Medium/Low → enrich)
- Priority hierarchy: correction > view > task > skill > recent messages
- Returns enriched `primary` query with confidence and source

The existing LLM fallback (`llm_rewrite`) is retired from this stage — its job is now handled by Stage 3 (multi-query expansion), which does the same thing but better (3 variants instead of 1).

### 3.2 Pseudo-Relevance Feedback (Stage 2) — Normal+

**Location:** `context_engine/src/enhancement/prf.rs`

Zero-LLM-cost technique. Retrieves a small initial set, extracts discriminative terms, adds them as query variants.

**Algorithm:**
1. Quick retrieval: top-3 facts via `MemoryRetriever`, score >= 0.6 threshold
2. Tokenize each fact's (subject, predicate, object) triple
3. Filter stopwords (reuse existing 289-word list from query_rewriter.rs)
4. Remove terms already present in `input.primary`
5. Rank by cross-result frequency (terms in 2+ of 3 results score higher)
6. Take top-5 terms
7. Build expansion variant: `"{primary} {expansion_terms}"`
8. Append to `QueryBundle.variants`

**Guardrails (PrfConfig):**
- `initial_fetch_limit: 3` — tight, not 5
- `min_score_threshold: 0.6` — only expand from high-confidence results
- `max_expansion_terms: 5`
- `stopword_filter: true` — always
- Budget check: skip if `budget.max_latency_ms < 50`

**Failure mode protection:** If initial retrieval returns low-scoring results (all below 0.6), PRF produces nothing — no query drift.

**Dependency:** Takes `Arc<dyn MemoryRetriever>` at construction (same pattern InsightForge uses). Constructed in agent builder.

### 3.3 Multi-Query Expansion (Stage 3) — Deep+ only

**Location:** `agent/src/adapters/multi_query.rs`

LLM generates 3 query variants approaching the question from different angles.

**Algorithm:**
1. Check `budget.max_llm_calls > 0` — skip in Normal mode
2. Build prompt with original query + available context (skill, task, view)
3. LLM generates 3 search queries, one per line (max 60 tokens, temperature 0.7)
4. Filter: non-empty, 5-200 chars, take first 3
5. Append to `QueryBundle.variants`

**Budget enforcement:** Checks `max_llm_calls` before calling. Timeout capped at `min(budget.max_latency_ms, 500ms)`.

**Graceful degradation:** Timeout or LLM error → passthrough, trace records failure.

### 3.4 Heuristic Reranking (Stage 4a) — always on

**Location:** `context_engine/src/enhancement/heuristic_rerank.rs`

Reorders results using signals the initial 12-factor scorer underweights:

1. **Co-activation boost** (max +0.1): `sigmoid(strength, center=3) * weight`. Facts retrieved together historically cluster together again.
2. **Query-term overlap** (max +0.05): Direct BM25-style term match between enriched primary query and fact object text.
3. **Recency tiebreaker** (+0.02): Among similar scores, prefer facts accessed within last 7 days.

Re-sorts by updated score. Cheap — just DB lookups for co-activation, no LLM.

### 3.5 LLM Reranking (Stage 4b) — Deep+ only

**Location:** `agent/src/adapters/llm_rerank.rs`

Pairwise relevance scoring — the LLM scores each candidate against the query.

**Algorithm:**
1. Check `budget.max_llm_calls > 0` — skip in Normal mode
2. Take top-10 candidates only (cost control)
3. Build prompt: "Rate each fact's relevance to the query on 0-10. Return ID:SCORE per line."
4. Parse scores, apply as new relevance scores
5. Merge with remaining candidates, re-sort

**Budget enforcement:** Single LLM call, max 100 tokens output. Timeout capped at `min(budget.max_latency_ms, 800ms)`.

## 4. Data Flow and Integration

### 4.1 Assembler Changes

**File:** `context_engine/src/assembler/mod.rs`

Current code calls `self.query_rewriter.rewrite_or_spawn()` with the background race pattern (`late_rx`, `try_recv`). This is replaced with:

```rust
let budget = EnhancementBudget::from(request.depth_mode)
    .with_config_overrides(&self.config.query_enhancement);

let enhancement = match &self.query_pipeline {
    Some(pipeline) => {
        let bundle = pipeline.enhance(&request.message_text, ctx, &budget).await;
        let results = self.insight_forge.retrieve_with_bundle(&bundle.query, limit).await?;
        let ranked = self.ranking_pipeline.rerank(&bundle.query, results, &budget).await;
        EnhancementOutput { query: bundle.query, candidates: Some(ranked), trace: bundle.trace }
    }
    None => EnhancementOutput::passthrough(&request.message_text),
};
```

The entire `late_rx` / `try_recv` / background spawn dance is removed. The pipeline is linear and predictable.

### 4.2 InsightForge Extension

**File:** `context_engine/src/insight_forge/mod.rs`

New method alongside existing `retrieve()`:

```rust
pub async fn retrieve_with_bundle(&self, bundle: &QueryBundle, limit: usize) -> Result<Vec<ScoredFact>> {
    let mut sub_queries = self.decompose(&bundle.primary).await?;
    sub_queries.extend(bundle.variants.iter().cloned());
    sub_queries.dedup();
    self.fan_out_and_merge(sub_queries, limit).await
}
```

Existing `retrieve()` stays for backward compatibility.

### 4.3 CorrectionContext Wiring

**New:** `CorrectionTracker` — lightweight in-memory buffer.

- Listens to `DomainEvent::UserCorrectedAI` on the event bus
- Keeps last correction per session in `Arc<RwLock<HashMap<SessionKey, CorrectionContext>>>`
- Injected into `RetrievalContext` during `build_retrieval_context()` in `agent_runtime/runtime.rs`
- No DB, no persistence — corrections are session-scoped and transient

### 4.4 Builder Wiring

**File:** `agent/src/agent_loop/builder.rs`

```rust
// Build query stages
let signal_stage = SignalEnrichmentStage::new(existing_rewriter_logic);
let prf_stage = PrfStage::new(memory_retriever.clone(), PrfConfig::from(&config));
let multi_query_stage = MultiQueryStage::new(provider.clone(), model.clone());

let query_pipeline = QueryPipeline::new(vec![
    Arc::new(signal_stage),
    Arc::new(prf_stage),
    Arc::new(multi_query_stage),
]);

// Build ranking stages
let heuristic_rerank = HeuristicRerankStage::new(co_activation_repo.clone());
let llm_rerank = LlmRerankStage::new(provider.clone(), model.clone());
let ranking_pipeline = RankingPipeline::new(heuristic_rerank, Some(llm_rerank));

// Wire into context engine
let context_engine = context_engine
    .with_query_pipeline(query_pipeline)
    .with_ranking_pipeline(ranking_pipeline);
```

### 4.5 Type Consistency

`InsightForge::retrieve_with_bundle()` returns `Vec<ScoredFact>` directly (not `Vec<MemoryEntry>`), matching what `RankingPipeline::rerank()` expects. No conversion step needed.

## 5. Configuration

### 5.1 Config Schema

**File:** `crates/config/src/schema/cognitive.rs`

New `QueryEnhancementConfig` section under the existing `CognitiveConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryEnhancementConfig {
    /// Master switch. Default: true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub prf: PrfConfig,
    pub multi_query: MultiQueryConfig,
    pub reranking: RerankingConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_overrides: Option<BudgetOverrides>,
}
```

Sub-configs:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prf.enabled` | bool | true | Enable pseudo-relevance feedback |
| `prf.initialFetchLimit` | usize | 3 | How many facts to fetch for term extraction |
| `prf.minScoreThreshold` | f64 | 0.6 | Minimum score for a fact to contribute terms |
| `prf.maxExpansionTerms` | usize | 5 | Maximum terms to extract from initial results |
| `multiQuery.enabled` | bool | true | Enable LLM multi-query expansion (Deep+ only) |
| `multiQuery.maxVariants` | usize | 3 | Number of query variants to generate |
| `multiQuery.model` | Option | None | LLM model override (falls back to `rewriter_model`) |
| `reranking.enabled` | bool | true | Enable result reranking |
| `reranking.heuristicCoActivationWeight` | f64 | 0.1 | Co-activation boost weight in heuristic rerank |
| `reranking.heuristicRecencyWeight` | f64 | 0.02 | Recency boost weight |
| `reranking.llmRerankTopN` | usize | 10 | How many top results to LLM-rerank |
| `reranking.llmRerankModel` | Option | None | LLM model override |
| `budgetOverrides.normal.maxLatencyMs` | u64 | 100 | Override Normal mode latency budget |
| `budgetOverrides.deepThink.maxLlmCalls` | u32 | 2 | Override Deep mode LLM call budget |

**Hot-reload:** Pipeline holds `Arc<RwLock<QueryEnhancementConfig>>`. Reads at the start of each `enhance()` call. Stage enable/disable takes effect within 5 seconds (matching existing hot-reload behavior).

### 5.2 Example config.json

```json
{
  "cognitive": {
    "queryEnhancement": {
      "enabled": true,
      "prf": {
        "enabled": true,
        "minScoreThreshold": 0.7
      },
      "multiQuery": {
        "enabled": true,
        "model": "anthropic/claude-haiku-4-5"
      },
      "reranking": {
        "enabled": true,
        "llmRerankTopN": 8
      },
      "budgetOverrides": {
        "normal": { "maxLatencyMs": 150 },
        "deepThink": { "maxLlmCalls": 3 }
      }
    }
  }
}
```

### 5.3 Desktop UI

Query Enhancement settings are added to the existing `/settings` page under the existing Cognitive/Memory tab — not a new page or tab.

**Settings group:** "Query Enhancement"

Controls:
- Master toggle: enabled/disabled
- Per-stage toggles: PRF, Multi-Query, Reranking
- PRF: score threshold slider (0.3-0.9), max expansion terms (3/5/7)
- Multi-Query: model dropdown (from available providers), max variants
- Reranking: LLM rerank toggle, rerank top-N slider (5-20)
- Budget overrides per depth mode: latency (ms), LLM calls

Uses existing `get_config` / `update_config` Tauri commands — no new commands needed. The UI reads/writes `cognitive.queryEnhancement` through the existing config API.

### 5.4 Debug Trace UI

`EnhancementTrace` is surfaced in the existing message debug panel (expandable section alongside token counts and retrieval scores):

```
Enhancement Trace
  Depth: Normal  |  Total: 87ms  |  LLM: 0
  
  [ok] SignalEnrichment    12ms   +task, +skill
  [ok] PRF                 71ms   +2 terms
  [--] MultiQuery          —      skipped (Normal)
  [ok] HeuristicRerank     4ms    reordered top-15
  [--] LlmRerank           —      skipped (Normal)
  
  QueryBundle: 1 primary + 1 variant
  Confidence: 0.75
```

Rendered from `AgentEvent::RetrievalEnhanced(EnhancementTrace)` in the SSE stream. New event variant + small React component.

## 6. Observability

### 6.1 EnhancementTrace

```rust
pub struct EnhancementTrace {
    pub stages: Vec<StageTrace>,
    pub total_latency_ms: u64,
    pub total_llm_calls: u32,
    pub total_llm_tokens: u32,
    pub depth_mode: DepthMode,
}

pub struct StageTrace {
    pub name: QuerySource,
    pub status: StageStatus,
    pub latency_ms: u64,
    pub llm_calls: u32,
    pub llm_tokens: u32,
    pub output_summary: String,
}

pub enum StageStatus {
    Ran,
    Skipped(String),   // "budget_exceeded", "no_signals", "depth_normal"
    Failed(String),    // error message
}
```

### 6.2 Event Emission

Emitted as `AgentEvent::RetrievalEnhanced(EnhancementTrace)` — same pattern as existing `AgentEvent::ContextCompressed`. Consumed by:
- Frontend debug panel (display)
- Autotuner (correlate enhancement quality with response quality for A/B testing)

### 6.3 MCP Exposure

New action on the existing `memory` MCP tool: `get_last_enhancement_trace`. Returns the most recent `EnhancementTrace` for debugging via Claude Code or other MCP clients.

## 7. Code Organization

| Component | Crate | Path |
|-----------|-------|------|
| `QueryBundle`, `EnhancementBudget`, `EnhancementOutput`, `EnhancementTrace` | `context_engine` | `src/enhancement/types.rs` |
| `QueryStage`, `RankingStage` traits | `context_engine` | `src/enhancement/traits.rs` |
| `QueryPipeline`, `RankingPipeline` orchestrators | `context_engine` | `src/enhancement/pipeline.rs` |
| `PrfStage` (heuristic) | `context_engine` | `src/enhancement/prf.rs` |
| `HeuristicRerankStage` | `context_engine` | `src/enhancement/heuristic_rerank.rs` |
| `SignalEnrichmentStage` (wraps existing rewriter) | `agent` | `src/adapters/signal_enrichment.rs` |
| `MultiQueryStage` (LLM) | `agent` | `src/adapters/multi_query.rs` |
| `LlmRerankStage` (LLM) | `agent` | `src/adapters/llm_rerank.rs` |
| `CorrectionTracker` | `agent` | `src/adapters/correction_tracker.rs` |
| `QueryEnhancementConfig` | `config` | `src/schema/cognitive.rs` |
| Settings UI component | `desktop-ui` | `src/features/settings/` (existing) |
| Debug trace component | `desktop-ui` | `src/features/chat/` (existing debug panel) |
| `EnhancementTraceRepo` | `cognitive` | `src/repos/enhancement_trace.rs` |
| Reforge feedback collector | `cognitive` | `src/services/reforge/feedback.rs` (extend existing) |
| Autotuner `TrialParams` extension | `common` | `src/autotuner.rs` (extend existing) |

## 8. What Doesn't Change

- InsightForge's decomposer, circuit breaker, and RRF merge logic — untouched
- The 12-factor relevance scorer in `decay.rs` — untouched (heuristic rerank adds on top, doesn't replace)
- Autotuner integration — extended (not replaced) with new `TrialParams` fields for pipeline params
- Reforge Collect → Synthesize → Review flow — extended (not replaced) with enhancement trace signals
- The `QueryRewriter` trait — kept for backward compatibility; assembler prefers `QueryPipeline` when available
- Existing `ContextualQueryRewriter` tests — adapted to test `SignalEnrichmentStage` wrapper
- `rewrite_or_spawn` and the background race pattern — removed from the assembler, but the method stays on the struct for any other callers

## 9. Testing Strategy

### Unit Tests (per stage)

| Stage | Key test cases |
|-------|---------------|
| SignalEnrichment | correction priority, skill/task injection, High specificity passthrough, empty context passthrough |
| PRF | mock retriever returns 3 facts → expansion terms extracted; all facts below threshold → passthrough; stopword filtering; empty retriever → passthrough |
| MultiQuery | mock LLM returns 3 lines → variants added; timeout → passthrough; budget.max_llm_calls=0 → skip; malformed output → passthrough |
| HeuristicRerank | co-activation boost reorders results; recency tiebreaker; score capped at 1.0; empty co-activation repo → no crash |
| LlmRerank | mock LLM scores → reorder; budget=0 → skip; parse failure → passthrough; only top-N sent to LLM |

### Integration Tests (pipeline-level)

- Full `QueryPipeline` with all 3 stages → `QueryBundle` accumulates sources correctly
- Full `RankingPipeline` with heuristic + LLM → final ordering correct
- End-to-end: assembler → pipeline → InsightForge → ranking → `EnhancementOutput` shape
- Budget enforcement: `max_llm_calls=0` → no LLM stages fire
- Graceful degradation: Stage 2 errors → Stage 3 still runs with Stage 1's output
- Config hot-reload: disable PRF mid-test → next call skips it

### CorrectionTracker Tests

- Emit `UserCorrectedAI` → tracker captures it
- Session scoping: correction from session A doesn't leak to session B
- Only latest correction retained per session
- No correction → `None` in `RetrievalContext`

## 10. Depth Mode Behavior Summary

| Stage | Normal | DeepThink | Ultra |
|-------|--------|-----------|-------|
| Signal Enrichment | always | always | always |
| PRF | on (heuristic, ~50-100ms) | on | on |
| Multi-Query | **off** | on (1 LLM call, ~300ms) | on |
| Heuristic Rerank | on (no LLM) | on | on |
| LLM Rerank | **off** | on (1 LLM call, ~200ms) | on |
| **Total extra latency** | ~50-100ms | ~300-500ms | ~500-1000ms |
| **Total LLM calls** | 0 | 2 | 4 |

Normal mode adds zero LLM calls — pure heuristic improvement. Deep mode adds the two highest-ROI LLM techniques (multi-query + reranking) within a 500ms budget.

## 11. Self-Optimization Integration (Autotuner + Reforge)

The `EnhancementTrace` is the connective tissue — it feeds both optimization systems without either needing to know about the other. The autotuner optimizes parameters; Reforge optimizes strategy.

### 11.1 Autotuner — Parameter-Level A/B Testing

The autotuner already tunes 3 rewriter params via `TrialParams`. We extend with pipeline-specific params:

```rust
// crates/common/src/autotuner.rs — extend TrialParams
pub struct TrialParams {
    // ... existing fields ...

    // Phase 4: Query Enhancement Pipeline params
    /// PRF minimum score threshold (bounds [0.3, 0.9]).
    pub prf_score_threshold: Option<f64>,
    /// PRF max expansion terms (bounds [2, 8]).
    pub prf_max_expansion_terms: Option<usize>,
    /// Heuristic rerank co-activation weight (bounds [0.02, 0.2]).
    pub rerank_co_activation_weight: Option<f64>,
    /// Multi-query max variants (bounds [1, 5]).
    pub multi_query_max_variants: Option<usize>,
    /// Override Normal mode latency budget (bounds [50, 300]).
    pub enhancement_budget_latency_ms: Option<u64>,
}
```

**How it works:**
1. Autotuner generates trial parameter sets with varied enhancement pipeline values
2. `QueryPipeline::enhance()` reads champion overrides at the start of each call (same pattern as existing rewriter)
3. `EnhancementTrace` provides the evaluation signal — autotuner correlates:
   - Which stages ran and their latency
   - Query confidence scores
   - Downstream response quality (existing satisfaction signal)
4. After enough observations, the winning parameter set is promoted to champion

**Wiring:** `QueryPipeline` takes `champion_overrides: Option<Arc<RwLock<Option<TrialParams>>>>` at construction (same pattern as `ContextualQueryRewriter`). In `enhance()`, resolved params override config defaults:

```rust
fn resolve_params(&self) -> ResolvedEnhancementParams {
    let champion = self.champion_overrides.as_ref()
        .and_then(|lock| lock.read().ok())
        .and_then(|opt| opt.clone());
    
    ResolvedEnhancementParams {
        prf_score_threshold: champion.as_ref()
            .and_then(|c| c.prf_score_threshold)
            .unwrap_or(self.config.prf.min_score_threshold),
        // ... same pattern for other params
    }
}
```

### 11.2 Reforge — Strategy-Level Nightly Reflection

Reforge's existing "Collect" phase gathers retrieval precision signals. We add `EnhancementTrace` history as a new feedback signal source.

**New feedback signal:** `EnhancementTraceSignal`

The cognitive background service persists a rolling window of `EnhancementTrace` summaries (last 7 days) to a new `enhancement_trace_log` table:

```sql
CREATE TABLE enhancement_trace_log (
    id           TEXT PRIMARY KEY,
    session_key  TEXT NOT NULL,
    depth_mode   TEXT NOT NULL,
    stages_json  TEXT NOT NULL,      -- serialized Vec<StageTrace>
    total_latency_ms INTEGER NOT NULL,
    total_llm_calls  INTEGER NOT NULL,
    query_confidence REAL NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Reforge analysis patterns** (in the existing Collect → Synthesize flow):

| Pattern detected | Reforge suggestion |
|-----------------|-------------------|
| PRF expansion terms rarely appear in final selected facts (>70% miss rate) | "PRF is adding noise — raise `prf_score_threshold` to 0.75 or disable" |
| Multi-query variants return >80% same results as primary query | "Multi-query expansion is redundant — reduce `max_variants` to 1 or adjust prompt diversity" |
| LLM reranking changes top-3 order >60% of the time | "LLM reranking is high-value — consider enabling in Normal mode" |
| Specific domain (e.g., finance) consistently has low query confidence in Normal mode | "Auto-upgrade to DeepThink for finance-domain queries" |
| Stage X fails >20% of calls | "Stage X is unreliable — investigate provider/model stability" |
| Normal mode latency consistently hits budget ceiling | "Budget too tight — raise `enhancement_budget_latency_ms` to 150ms" |

**Implementation:** Add `collect_enhancement_signals()` to `crates/cognitive/src/services/reforge/feedback.rs` alongside the existing `collect_retrieval_signals()`. This function queries the `enhancement_trace_log` table, computes aggregate metrics, and returns structured feedback that Reforge's Synthesize phase can reason about.

**No new Reforge phase needed** — this feeds into the existing Collect → Synthesize → Review flow. The LLM prompt in the Synthesize phase is extended with an "Enhancement Pipeline Health" section containing the aggregated metrics.

### 11.3 Compaction

The `enhancement_trace_log` table is cleaned up in the existing compaction cycle:

```rust
// In run_compaction():
// 9. Clean old enhancement traces (7-day rolling window)
if let Some(trace_repo) = enhancement_trace_repo {
    let deleted = trace_repo.delete_older_than(7).await?;
    // ...
}
```

### 11.4 Integration Summary

```
Per-message:
  QueryPipeline → reads champion_overrides → emits EnhancementTrace
                                                    │
                                                    ▼
                                           enhancement_trace_log (SQLite)
                                                    │
                              ┌──────────────────────┼──────────────────────┐
                              ▼                      ▼                      ▼
                         Autotuner              Reforge                Debug UI
                    (A/B test params)    (nightly strategy)     (message trace panel)
                              │                      │
                              ▼                      ▼
                     champion_overrides      reforge_suggestions
                    (live param tuning)   (strategy recommendations)
```

The feedback loop is:
1. **Per-message:** Pipeline runs with current params, emits trace
2. **Continuous:** Autotuner evaluates trial params using trace + quality signals, promotes winners
3. **Nightly:** Reforge analyzes 7-day trace history, generates strategy suggestions (enable/disable stages, per-domain depth, budget adjustments)
4. **On approval:** Reforge suggestions become config changes or new autotuner trial seeds
