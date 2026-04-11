# Query Enhancement Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-stage query rewriter with a two-pipeline enhancement system (`QueryPipeline` + `RankingPipeline`) gated by an explicit `EnhancementBudget` derived from `DepthMode`.

**Architecture:** Types and traits in `context_engine`, LLM-dependent implementations in `agent/src/adapters/`. PRF (heuristic) lives in `context_engine`. `CorrectionTracker` captures user corrections via `DomainEventBus`. Autotuner extended with Phase 5 params. Reforge gets enhancement trace signals.

**Tech Stack:** Rust async traits (`async_trait`), `tokio`, `sqlx`, existing `MemoryRetriever`/`InsightForge`/`DomainEventBus` infrastructure.

**Spec:** `docs/superpowers/specs/2026-04-10-query-enhancement-pipeline-design.md`

**Important:** Do NOT commit anything. The user will commit manually.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/context_engine/src/enhancement/mod.rs` | Module root, re-exports |
| `crates/context_engine/src/enhancement/types.rs` | `QueryBundle`, `EnhancementBudget`, `EnhancementOutput`, `EnhancementTrace`, `StageTrace`, `StageStatus`, `QuerySource` |
| `crates/context_engine/src/enhancement/traits.rs` | `QueryStage`, `RankingStage` traits |
| `crates/context_engine/src/enhancement/pipeline.rs` | `QueryPipeline`, `RankingPipeline` orchestrators |
| `crates/context_engine/src/enhancement/prf.rs` | `PrfStage` — pseudo-relevance feedback (heuristic, no LLM) |
| `crates/context_engine/src/enhancement/heuristic_rerank.rs` | `HeuristicRerankStage` — co-activation + recency + term overlap |
| `crates/agent/src/adapters/signal_enrichment.rs` | `SignalEnrichmentStage` — wraps existing `ContextualQueryRewriter` |
| `crates/agent/src/adapters/multi_query.rs` | `MultiQueryStage` — LLM generates query variants |
| `crates/agent/src/adapters/llm_rerank.rs` | `LlmRerankStage` — LLM pairwise reranking |
| `crates/agent/src/adapters/correction_tracker.rs` | `CorrectionTracker` — captures corrections via event bus |
| `crates/cognitive/src/repos/enhancement_trace.rs` | `EnhancementTraceRepo` — persists traces for Reforge |

### Modified files

| File | Changes |
|------|---------|
| `crates/context_engine/src/lib.rs` | Add `pub mod enhancement;` and re-exports |
| `crates/context_engine/src/assembler/mod.rs` | Replace `rewrite_or_spawn` dance with `QueryPipeline` call |
| `crates/context_engine/src/assembler/types.rs` | Add `depth_mode` field to `ContextRequest`, `enhancement_trace` to `AssembledContext` |
| `crates/context_engine/src/insight_forge/mod.rs` | Add `retrieve_with_bundle()` method |
| `crates/config/src/schema/cognitive.rs` | Add `QueryEnhancementConfig` and sub-configs |
| `crates/common/src/autotuner.rs` | Add Phase 5 `TrialParams` fields |
| `crates/agent/src/agent_loop/builder.rs` | Wire pipeline stages instead of bare rewriter |
| `crates/cognitive/src/repos/mod.rs` | Add `enhancement_trace` module |
| `crates/cognitive/src/services/reforge/feedback.rs` | Add `collect_enhancement_signals()` |
| `crates/cognitive/src/services/compaction.rs` | Add enhancement trace cleanup |

---

### Task 1: Core Types (`context_engine/src/enhancement/types.rs`)

**Files:**
- Create: `crates/context_engine/src/enhancement/types.rs`
- Create: `crates/context_engine/src/enhancement/mod.rs`
- Modify: `crates/context_engine/src/lib.rs`

- [ ] **Step 1: Create the enhancement module with core types**

Create `crates/context_engine/src/enhancement/types.rs`:

```rust
//! Core types for the query enhancement pipeline.

use serde::{Deserialize, Serialize};

/// Identifies which stage produced a contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuerySource {
    Passthrough,
    SignalEnrichment,
    PseudoRelevanceFeedback,
    MultiQuery,
    HeuristicRerank,
    LlmRerank,
}

impl std::fmt::Display for QuerySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passthrough => write!(f, "passthrough"),
            Self::SignalEnrichment => write!(f, "signal_enrichment"),
            Self::PseudoRelevanceFeedback => write!(f, "prf"),
            Self::MultiQuery => write!(f, "multi_query"),
            Self::HeuristicRerank => write!(f, "heuristic_rerank"),
            Self::LlmRerank => write!(f, "llm_rerank"),
        }
    }
}

/// A bundle of query variants produced by the QueryPipeline.
#[derive(Debug, Clone)]
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

impl QueryBundle {
    /// Create a passthrough bundle — no enrichment applied.
    pub fn passthrough(query: &str) -> Self {
        Self {
            original: query.to_string(),
            primary: query.to_string(),
            variants: vec![],
            confidence: 0.0,
            sources: vec![QuerySource::Passthrough],
        }
    }
}

/// Budget envelope for enhancement stages.
/// Replaces hardcoded DepthMode matching with an explicit cost model.
#[derive(Debug, Clone)]
pub struct EnhancementBudget {
    pub max_latency_ms: u64,
    pub max_llm_calls: u32,
    pub max_expansion_tokens: usize,
}

impl EnhancementBudget {
    pub fn normal() -> Self {
        Self {
            max_latency_ms: 100,
            max_llm_calls: 0,
            max_expansion_tokens: 0,
        }
    }

    pub fn deep_think() -> Self {
        Self {
            max_latency_ms: 500,
            max_llm_calls: 2,
            max_expansion_tokens: 200,
        }
    }

    pub fn ultra() -> Self {
        Self {
            max_latency_ms: 1000,
            max_llm_calls: 4,
            max_expansion_tokens: 400,
        }
    }
}

/// Status of a single pipeline stage execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    Ran,
    Skipped(String),
    Failed(String),
}

/// Trace for a single pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTrace {
    pub name: QuerySource,
    pub status: StageStatus,
    pub latency_ms: u64,
    pub llm_calls: u32,
    pub llm_tokens: u32,
    pub output_summary: String,
}

/// Full trace of the enhancement pipeline execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnhancementTrace {
    pub stages: Vec<StageTrace>,
    pub total_latency_ms: u64,
    pub total_llm_calls: u32,
    pub total_llm_tokens: u32,
}

impl EnhancementTrace {
    pub fn record_success(
        &mut self,
        name: QuerySource,
        latency_ms: u64,
        llm_calls: u32,
        llm_tokens: u32,
        summary: String,
    ) {
        self.stages.push(StageTrace {
            name,
            status: StageStatus::Ran,
            latency_ms,
            llm_calls,
            llm_tokens,
            output_summary: summary,
        });
        self.total_latency_ms += latency_ms;
        self.total_llm_calls += llm_calls;
        self.total_llm_tokens += llm_tokens;
    }

    pub fn record_skip(&mut self, name: QuerySource, reason: String) {
        self.stages.push(StageTrace {
            name,
            status: StageStatus::Skipped(reason),
            latency_ms: 0,
            llm_calls: 0,
            llm_tokens: 0,
            output_summary: String::new(),
        });
    }

    pub fn record_failure(&mut self, name: QuerySource, latency_ms: u64, error: String) {
        self.stages.push(StageTrace {
            name,
            status: StageStatus::Failed(error),
            latency_ms,
            llm_calls: 0,
            llm_tokens: 0,
            output_summary: String::new(),
        });
        self.total_latency_ms += latency_ms;
    }
}

/// Final output from the full enhancement flow.
#[derive(Debug, Clone)]
pub struct EnhancementOutput {
    pub query: QueryBundle,
    pub trace: EnhancementTrace,
}

impl EnhancementOutput {
    /// Passthrough — no enhancement, just wrap the original query.
    pub fn passthrough(query: &str) -> Self {
        Self {
            query: QueryBundle::passthrough(query),
            trace: EnhancementTrace::default(),
        }
    }
}
```

- [ ] **Step 2: Create the module root**

Create `crates/context_engine/src/enhancement/mod.rs`:

```rust
pub mod types;
pub mod traits;
pub mod pipeline;
pub mod prf;
pub mod heuristic_rerank;

pub use types::*;
pub use traits::*;
pub use pipeline::*;
```

- [ ] **Step 3: Register the module in context_engine's lib.rs**

In `crates/context_engine/src/lib.rs`, add after `pub mod rewriter;`:

```rust
pub mod enhancement;
pub use enhancement::{
    EnhancementBudget, EnhancementOutput, EnhancementTrace, QueryBundle, QuerySource,
    QueryStage, RankingStage, StageStatus, StageTrace,
};
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p context_engine 2>&1 | head -20`
Expected: Compile errors about missing `traits.rs`, `pipeline.rs`, `prf.rs`, `heuristic_rerank.rs` — that's fine, we'll create them next. If there are errors in `types.rs` itself, fix them.

---

### Task 2: Pipeline Traits (`context_engine/src/enhancement/traits.rs`)

**Files:**
- Create: `crates/context_engine/src/enhancement/traits.rs`

- [ ] **Step 1: Create trait definitions**

Create `crates/context_engine/src/enhancement/traits.rs`:

```rust
//! Traits for query enhancement pipeline stages.

use async_trait::async_trait;

use super::types::{EnhancementBudget, QueryBundle, QuerySource};
use crate::memory_retriever::MemoryEntry;
use crate::rewriter::RetrievalContext;

/// A query transformation stage: QueryBundle → QueryBundle.
///
/// Each stage may enrich the primary query, add variants, or adjust confidence.
/// Stages MUST gracefully degrade — return the input unchanged on error.
#[async_trait]
pub trait QueryStage: Send + Sync {
    /// Human-readable name for tracing.
    fn name(&self) -> QuerySource;

    /// Transform the query bundle. Returns the (possibly enriched) bundle.
    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle>;
}

/// A result reranking stage: Vec<MemoryEntry> → Vec<MemoryEntry>.
///
/// Reorders (and optionally prunes) retrieval results. The query bundle
/// is available for computing relevance signals.
#[async_trait]
pub trait RankingStage: Send + Sync {
    /// Human-readable name for tracing.
    fn name(&self) -> QuerySource;

    /// Rerank the candidates. Returns reordered entries.
    async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<MemoryEntry>,
        budget: &EnhancementBudget,
    ) -> common::Result<Vec<MemoryEntry>>;
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p context_engine 2>&1 | head -20`

---

### Task 3: Pipeline Orchestrators (`context_engine/src/enhancement/pipeline.rs`)

**Files:**
- Create: `crates/context_engine/src/enhancement/pipeline.rs`

- [ ] **Step 1: Implement QueryPipeline and RankingPipeline**

Create `crates/context_engine/src/enhancement/pipeline.rs`:

```rust
//! Pipeline orchestrators that run stages sequentially with graceful degradation.

use std::sync::Arc;

use tracing::warn;

use super::traits::{QueryStage, RankingStage};
use super::types::*;
use crate::memory_retriever::MemoryEntry;
use crate::rewriter::RetrievalContext;

/// Orchestrates query transformation stages.
///
/// Runs each stage sequentially. If a stage fails, the previous output
/// is passed through unchanged and the failure is recorded in the trace.
pub struct QueryPipeline {
    stages: Vec<Arc<dyn QueryStage>>,
}

impl QueryPipeline {
    pub fn new(stages: Vec<Arc<dyn QueryStage>>) -> Self {
        Self { stages }
    }

    /// Run all stages, producing an enhanced QueryBundle + trace.
    pub async fn enhance(
        &self,
        original_query: &str,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> EnhancementOutput {
        let mut bundle = QueryBundle::passthrough(original_query);
        let mut trace = EnhancementTrace::default();

        for stage in &self.stages {
            let start = std::time::Instant::now();
            match stage.transform(bundle.clone(), context, budget).await {
                Ok(output) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let variant_count = output.variants.len();
                    let summary = if output.sources != bundle.sources {
                        let new_sources: Vec<_> = output
                            .sources
                            .iter()
                            .filter(|s| !bundle.sources.contains(s))
                            .map(|s| s.to_string())
                            .collect();
                        if !new_sources.is_empty() {
                            format!("+{}, {} variants", new_sources.join(", "), variant_count)
                        } else {
                            "no change".to_string()
                        }
                    } else {
                        "no change".to_string()
                    };
                    trace.record_success(stage.name(), elapsed, 0, 0, summary);
                    bundle = output;
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    warn!(
                        stage = %stage.name(),
                        error = %e,
                        "Enhancement stage failed — passing through"
                    );
                    trace.record_failure(stage.name(), elapsed, e.to_string());
                    // bundle unchanged — continue with what we have
                }
            }
        }

        trace.total_latency_ms = trace.stages.iter().map(|s| s.latency_ms).sum();
        EnhancementOutput {
            query: bundle,
            trace,
        }
    }
}

/// Orchestrates result reranking stages.
///
/// Runs heuristic reranking always, then LLM reranking if budget allows.
pub struct RankingPipeline {
    stages: Vec<Arc<dyn RankingStage>>,
}

impl RankingPipeline {
    pub fn new(stages: Vec<Arc<dyn RankingStage>>) -> Self {
        Self { stages }
    }

    /// Rerank candidates through all stages.
    pub async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<MemoryEntry>,
        budget: &EnhancementBudget,
        trace: &mut EnhancementTrace,
    ) -> Vec<MemoryEntry> {
        let mut results = candidates;

        for stage in &self.stages {
            let start = std::time::Instant::now();
            match stage.rerank(query, results.clone(), budget).await {
                Ok(reranked) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let summary = format!("reranked {} entries", reranked.len());
                    trace.record_success(stage.name(), elapsed, 0, 0, summary);
                    results = reranked;
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    warn!(
                        stage = %stage.name(),
                        error = %e,
                        "Ranking stage failed — keeping previous order"
                    );
                    trace.record_failure(stage.name(), elapsed, e.to_string());
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewriter::RetrievalContext;

    /// A test stage that appends " enriched" to the primary query.
    struct MockEnrichStage;

    #[async_trait::async_trait]
    impl QueryStage for MockEnrichStage {
        fn name(&self) -> QuerySource {
            QuerySource::SignalEnrichment
        }
        async fn transform(
            &self,
            mut input: QueryBundle,
            _ctx: &RetrievalContext,
            _budget: &EnhancementBudget,
        ) -> common::Result<QueryBundle> {
            input.primary = format!("{} enriched", input.primary);
            input.confidence = 0.8;
            input.sources.push(QuerySource::SignalEnrichment);
            Ok(input)
        }
    }

    /// A test stage that always fails.
    struct FailingStage;

    #[async_trait::async_trait]
    impl QueryStage for FailingStage {
        fn name(&self) -> QuerySource {
            QuerySource::PseudoRelevanceFeedback
        }
        async fn transform(
            &self,
            _input: QueryBundle,
            _ctx: &RetrievalContext,
            _budget: &EnhancementBudget,
        ) -> common::Result<QueryBundle> {
            Err(common::KlyntbotError::Internal("test failure".into()))
        }
    }

    /// A test stage that adds a variant.
    struct MockVariantStage;

    #[async_trait::async_trait]
    impl QueryStage for MockVariantStage {
        fn name(&self) -> QuerySource {
            QuerySource::MultiQuery
        }
        async fn transform(
            &self,
            mut input: QueryBundle,
            _ctx: &RetrievalContext,
            _budget: &EnhancementBudget,
        ) -> common::Result<QueryBundle> {
            input.variants.push("variant query".to_string());
            input.sources.push(QuerySource::MultiQuery);
            Ok(input)
        }
    }

    #[tokio::test]
    async fn test_pipeline_runs_all_stages() {
        let pipeline = QueryPipeline::new(vec![
            Arc::new(MockEnrichStage),
            Arc::new(MockVariantStage),
        ]);
        let budget = EnhancementBudget::deep_think();
        let ctx = RetrievalContext::default();
        let output = pipeline.enhance("test query", &ctx, &budget).await;

        assert_eq!(output.query.original, "test query");
        assert_eq!(output.query.primary, "test query enriched");
        assert_eq!(output.query.variants.len(), 1);
        assert_eq!(output.query.variants[0], "variant query");
        assert!(output.query.sources.contains(&QuerySource::SignalEnrichment));
        assert!(output.query.sources.contains(&QuerySource::MultiQuery));
        assert_eq!(output.trace.stages.len(), 2);
    }

    #[tokio::test]
    async fn test_pipeline_degrades_gracefully_on_failure() {
        let pipeline = QueryPipeline::new(vec![
            Arc::new(MockEnrichStage),
            Arc::new(FailingStage),
            Arc::new(MockVariantStage),
        ]);
        let budget = EnhancementBudget::deep_think();
        let ctx = RetrievalContext::default();
        let output = pipeline.enhance("test query", &ctx, &budget).await;

        // Stage 1 succeeded, Stage 2 failed, Stage 3 should still run
        assert_eq!(output.query.primary, "test query enriched");
        assert_eq!(output.query.variants.len(), 1); // Stage 3 still ran
        assert_eq!(output.trace.stages.len(), 3);
        assert!(matches!(
            output.trace.stages[1].status,
            StageStatus::Failed(_)
        ));
    }

    #[tokio::test]
    async fn test_pipeline_passthrough_when_empty() {
        let pipeline = QueryPipeline::new(vec![]);
        let budget = EnhancementBudget::normal();
        let ctx = RetrievalContext::default();
        let output = pipeline.enhance("hello", &ctx, &budget).await;

        assert_eq!(output.query.original, "hello");
        assert_eq!(output.query.primary, "hello");
        assert!(output.query.variants.is_empty());
        assert_eq!(output.query.confidence, 0.0);
    }
}
```

- [ ] **Step 2: Verify it compiles and tests pass**

Run: `cargo nextest run -p context_engine -E 'test(pipeline)' 2>&1 | tail -20`

---

### Task 4: PRF Stage (`context_engine/src/enhancement/prf.rs`)

**Files:**
- Create: `crates/context_engine/src/enhancement/prf.rs`

- [ ] **Step 1: Write tests for PRF**

- [ ] **Step 2: Implement PrfStage**

Create `crates/context_engine/src/enhancement/prf.rs`:

```rust
//! Pseudo-Relevance Feedback (PRF) stage.
//!
//! Zero-LLM-cost technique: retrieve a small initial set, extract discriminative
//! terms from the results, add them as query variants.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;

use super::traits::QueryStage;
use super::types::{EnhancementBudget, QueryBundle, QuerySource};
use crate::memory_retriever::MemoryRetriever;
use crate::rewriter::RetrievalContext;

/// Configuration for pseudo-relevance feedback.
#[derive(Debug, Clone)]
pub struct PrfConfig {
    /// How many facts to fetch for term extraction (default: 3).
    pub initial_fetch_limit: usize,
    /// Minimum score for a fact to contribute expansion terms (default: 0.6).
    pub min_score_threshold: f64,
    /// Maximum terms to extract from initial results (default: 5).
    pub max_expansion_terms: usize,
}

impl Default for PrfConfig {
    fn default() -> Self {
        Self {
            initial_fetch_limit: 3,
            min_score_threshold: 0.6,
            max_expansion_terms: 5,
        }
    }
}

/// PRF stage — retrieves top-K results and extracts expansion terms.
pub struct PrfStage {
    retriever: Arc<dyn MemoryRetriever>,
    config: PrfConfig,
}

impl PrfStage {
    pub fn new(retriever: Arc<dyn MemoryRetriever>, config: PrfConfig) -> Self {
        Self { retriever, config }
    }
}

#[async_trait]
impl QueryStage for PrfStage {
    fn name(&self) -> QuerySource {
        QuerySource::PseudoRelevanceFeedback
    }

    async fn transform(
        &self,
        input: QueryBundle,
        _context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle> {
        // Budget guard: PRF needs at least 50ms of budget
        if budget.max_latency_ms < 50 {
            return Ok(input);
        }

        // 1. Quick retrieval
        let initial = self
            .retriever
            .retrieve(&input.primary, self.config.initial_fetch_limit)
            .await;

        // 2. Filter to high-confidence results
        let strong: Vec<_> = initial
            .iter()
            .filter(|e| e.score >= self.config.min_score_threshold)
            .collect();

        if strong.is_empty() {
            return Ok(input);
        }

        // 3. Extract discriminative terms
        let expansion_terms =
            extract_discriminative_terms(&strong, &input.primary, self.config.max_expansion_terms);

        if expansion_terms.is_empty() {
            return Ok(input);
        }

        // 4. Build expansion variant
        let variant = format!("{} {}", input.primary, expansion_terms.join(" "));
        let mut bundle = input;
        bundle.variants.push(variant);
        bundle.sources.push(QuerySource::PseudoRelevanceFeedback);
        Ok(bundle)
    }
}

/// Extract discriminative terms from retrieval results that aren't already
/// in the query. Ranks by cross-result frequency (terms in 2+ results score higher).
fn extract_discriminative_terms(
    entries: &[&crate::memory_retriever::MemoryEntry],
    query: &str,
    max_terms: usize,
) -> Vec<String> {
    let query_terms: HashSet<String> = tokenize(query).into_iter().collect();

    // Count term frequency across entries
    let mut term_freq: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        let entry_terms: HashSet<String> = tokenize(&entry.content).into_iter().collect();
        for term in entry_terms {
            if !query_terms.contains(&term) && !is_stopword(&term) && term.len() >= 3 {
                *term_freq.entry(term).or_insert(0) += 1;
            }
        }
    }

    // Sort by frequency (higher = appears in more results = more discriminative)
    let mut ranked: Vec<_> = term_freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.into_iter().take(max_terms).map(|(t, _)| t).collect()
}

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the" | "a" | "an" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for"
        | "of" | "with" | "by" | "from" | "is" | "it" | "this" | "that" | "are" | "was"
        | "were" | "be" | "been" | "has" | "have" | "had" | "do" | "does" | "did" | "will"
        | "would" | "could" | "should" | "may" | "might" | "can" | "not" | "no" | "so"
        | "if" | "then" | "than" | "too" | "very" | "just" | "about" | "up" | "out"
        | "all" | "also" | "how" | "what" | "when" | "where" | "who" | "which" | "why"
        | "there" | "here" | "some" | "any" | "each" | "every" | "both" | "few" | "more"
        | "most" | "other" | "into" | "over" | "after" | "before" | "between" | "under"
        | "through" | "during" | "without" | "within" | "along" | "following" | "across"
        | "behind" | "beyond" | "plus" | "except" | "its" | "my" | "your" | "his" | "her"
        | "our" | "their" | "me" | "him" | "them" | "you" | "she" | "he" | "we" | "they"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_retriever::{MemoryEntry, MemorySource};

    struct MockRetriever {
        entries: Vec<MemoryEntry>,
    }

    #[async_trait]
    impl MemoryRetriever for MockRetriever {
        async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
            self.entries.iter().take(limit).cloned().collect()
        }
    }

    fn make_entry(id: &str, content: &str, score: f64) -> MemoryEntry {
        MemoryEntry {
            id: id.into(),
            content: content.into(),
            score,
            source: MemorySource::CognitiveFact,
            raw_score: score,
        }
    }

    #[tokio::test]
    async fn test_prf_extracts_expansion_terms() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                make_entry("1", "budget projection quarterly finance", 0.8),
                make_entry("2", "quarterly review finance planning", 0.7),
                make_entry("3", "budget allocation department", 0.65),
            ],
        });
        let stage = PrfStage::new(retriever, PrfConfig::default());
        let input = QueryBundle::passthrough("show me budget");
        let budget = EnhancementBudget::normal();
        let ctx = RetrievalContext::default();

        let result = stage.transform(input, &ctx, &budget).await.unwrap();

        assert_eq!(result.variants.len(), 1);
        // "quarterly" appears in 2/3 results, should be top expansion term
        assert!(result.variants[0].contains("quarterly"));
        assert!(result.sources.contains(&QuerySource::PseudoRelevanceFeedback));
    }

    #[tokio::test]
    async fn test_prf_passthrough_on_low_scores() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                make_entry("1", "irrelevant content here", 0.3),
                make_entry("2", "also irrelevant stuff", 0.2),
            ],
        });
        let stage = PrfStage::new(retriever, PrfConfig::default());
        let input = QueryBundle::passthrough("test query");
        let budget = EnhancementBudget::normal();
        let ctx = RetrievalContext::default();

        let result = stage.transform(input, &ctx, &budget).await.unwrap();

        // All below 0.6 threshold — no expansion
        assert!(result.variants.is_empty());
        assert!(!result.sources.contains(&QuerySource::PseudoRelevanceFeedback));
    }

    #[tokio::test]
    async fn test_prf_skips_on_tight_budget() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![make_entry("1", "content", 0.9)],
        });
        let stage = PrfStage::new(retriever, PrfConfig::default());
        let input = QueryBundle::passthrough("test");
        let budget = EnhancementBudget {
            max_latency_ms: 30, // too tight
            max_llm_calls: 0,
            max_expansion_tokens: 0,
        };
        let ctx = RetrievalContext::default();

        let result = stage.transform(input, &ctx, &budget).await.unwrap();
        assert!(result.variants.is_empty());
    }

    #[tokio::test]
    async fn test_prf_no_duplicate_query_terms() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![make_entry(
                "1",
                "budget planning quarterly review",
                0.8,
            )],
        });
        let stage = PrfStage::new(retriever, PrfConfig::default());
        let input = QueryBundle::passthrough("budget planning");
        let budget = EnhancementBudget::normal();
        let ctx = RetrievalContext::default();

        let result = stage.transform(input, &ctx, &budget).await.unwrap();

        if !result.variants.is_empty() {
            // "budget" and "planning" should NOT appear as expansion terms
            let variant = &result.variants[0];
            let expansion_part = variant.strip_prefix("budget planning ").unwrap_or(variant);
            assert!(!expansion_part.contains("budget"));
            assert!(!expansion_part.contains("planning"));
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine -E 'test(prf)' 2>&1 | tail -20`

---

### Task 5: Heuristic Rerank Stage (`context_engine/src/enhancement/heuristic_rerank.rs`)

**Files:**
- Create: `crates/context_engine/src/enhancement/heuristic_rerank.rs`

- [ ] **Step 1: Implement HeuristicRerankStage**

Create `crates/context_engine/src/enhancement/heuristic_rerank.rs`:

```rust
//! Heuristic reranking stage — boosts results using co-activation,
//! query-term overlap, and recency signals. No LLM cost.

use std::collections::HashSet;

use async_trait::async_trait;

use super::prf::tokenize;
use super::traits::RankingStage;
use super::types::{EnhancementBudget, QueryBundle, QuerySource};
use crate::memory_retriever::MemoryEntry;

/// Configuration for heuristic reranking.
#[derive(Debug, Clone)]
pub struct HeuristicRerankConfig {
    /// Weight for query-term overlap boost (default: 0.05).
    pub term_overlap_weight: f64,
}

impl Default for HeuristicRerankConfig {
    fn default() -> Self {
        Self {
            term_overlap_weight: 0.05,
        }
    }
}

/// Heuristic reranking — boosts results using cheap signals.
///
/// Applied in all depth modes (no LLM cost). Boosts:
/// 1. Query-term overlap between enriched query and entry content
pub struct HeuristicRerankStage {
    config: HeuristicRerankConfig,
}

impl HeuristicRerankStage {
    pub fn new(config: HeuristicRerankConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl RankingStage for HeuristicRerankStage {
    fn name(&self) -> QuerySource {
        QuerySource::HeuristicRerank
    }

    async fn rerank(
        &self,
        query: &QueryBundle,
        mut candidates: Vec<MemoryEntry>,
        _budget: &EnhancementBudget,
    ) -> common::Result<Vec<MemoryEntry>> {
        let query_terms: HashSet<String> = tokenize(&query.primary).into_iter().collect();

        for entry in &mut candidates {
            let mut boost = 0.0_f64;

            // 1. Query-term overlap
            let entry_terms: HashSet<String> = tokenize(&entry.content).into_iter().collect();
            if !query_terms.is_empty() {
                let overlap = query_terms.intersection(&entry_terms).count() as f64
                    / query_terms.len() as f64;
                boost += overlap * self.config.term_overlap_weight;
            }

            entry.score = (entry.score + boost).min(1.0);
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(candidates)
    }
}

// Make tokenize available to this module from prf
pub use super::prf::tokenize as tokenize_text;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_retriever::MemorySource;

    fn make_entry(id: &str, content: &str, score: f64) -> MemoryEntry {
        MemoryEntry {
            id: id.into(),
            content: content.into(),
            score,
            source: MemorySource::CognitiveFact,
            raw_score: score,
        }
    }

    #[tokio::test]
    async fn test_heuristic_rerank_boosts_overlapping_terms() {
        let stage = HeuristicRerankStage::new(HeuristicRerankConfig::default());
        let query = QueryBundle {
            original: "budget forecast".into(),
            primary: "budget forecast quarterly".into(),
            variants: vec![],
            confidence: 0.8,
            sources: vec![QuerySource::SignalEnrichment],
        };

        let candidates = vec![
            make_entry("no-overlap", "random content here", 0.7),
            make_entry("overlap", "budget forecast for next quarter", 0.7),
        ];

        let budget = EnhancementBudget::normal();
        let result = stage.rerank(&query, candidates, &budget).await.unwrap();

        // "overlap" entry should be boosted and come first
        assert_eq!(result[0].id, "overlap");
        assert!(result[0].score > 0.7);
    }

    #[tokio::test]
    async fn test_heuristic_rerank_score_capped_at_1() {
        let stage = HeuristicRerankStage::new(HeuristicRerankConfig {
            term_overlap_weight: 0.5, // artificially high to test capping
        });
        let query = QueryBundle {
            original: "exact match".into(),
            primary: "exact match".into(),
            variants: vec![],
            confidence: 0.8,
            sources: vec![],
        };

        let candidates = vec![make_entry("high", "exact match content", 0.98)];
        let budget = EnhancementBudget::normal();
        let result = stage.rerank(&query, candidates, &budget).await.unwrap();

        assert!(result[0].score <= 1.0);
    }

    #[tokio::test]
    async fn test_heuristic_rerank_empty_candidates() {
        let stage = HeuristicRerankStage::new(HeuristicRerankConfig::default());
        let query = QueryBundle::passthrough("test");
        let budget = EnhancementBudget::normal();
        let result = stage.rerank(&query, vec![], &budget).await.unwrap();
        assert!(result.is_empty());
    }
}
```

- [ ] **Step 2: Make `tokenize` public in prf.rs**

In `crates/context_engine/src/enhancement/prf.rs`, change `fn tokenize` to `pub fn tokenize`.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine -E 'test(heuristic_rerank)' 2>&1 | tail -20`

---

### Task 6: InsightForge `retrieve_with_bundle` + Assembler Integration

**Files:**
- Modify: `crates/context_engine/src/insight_forge/mod.rs`
- Modify: `crates/context_engine/src/assembler/types.rs`
- Modify: `crates/context_engine/src/assembler/mod.rs`

This is the critical integration task — it connects the new pipeline to the existing retrieval path.

- [ ] **Step 1: Add `retrieve_with_bundle` to InsightForge**

In `crates/context_engine/src/insight_forge/mod.rs`, add this method to `impl InsightForge` (after the existing `retrieve_with_enrichment` method):

```rust
    /// Retrieve using a QueryBundle — feeds bundle.variants as additional sub-queries
    /// alongside the decomposed primary query.
    pub async fn retrieve_with_bundle(
        &self,
        bundle: &crate::enhancement::QueryBundle,
        total_limit: usize,
        session_key: Option<&str>,
    ) -> Vec<MemoryEntry> {
        let limit = total_limit.min(self.config.total_limit);

        // Circuit breaker check
        if let Some(sk) = session_key {
            if self.circuit_breaker.is_open(sk) {
                debug!("InsightForge circuit open for session, falling back");
                return self.fallback(&bundle.primary, limit).await;
            }
        }

        // Decompose the primary query
        let mut sub_queries = {
            let decompose_fut = self.decomposer.decompose(&bundle.primary, None);
            let timeout = tokio::time::timeout(
                std::time::Duration::from_millis(self.config.decomposer_timeout_ms),
                decompose_fut,
            );
            match timeout.await {
                Ok(subs) => {
                    let mut subs = subs;
                    subs.truncate(self.config.max_sub_queries);
                    subs
                }
                Err(_) => {
                    warn!("InsightForge decomposer timed out, falling back");
                    if let Some(sk) = session_key {
                        self.circuit_breaker.record_failure(sk);
                    }
                    return self.fallback(&bundle.primary, limit).await;
                }
            }
        };

        // Append bundle variants (from PRF, multi-query)
        for variant in &bundle.variants {
            if !sub_queries.contains(variant) {
                sub_queries.push(variant.clone());
            }
        }
        sub_queries.truncate(self.config.max_sub_queries + bundle.variants.len().min(3));

        tracing::debug!(
            sub_query_count = sub_queries.len(),
            variant_count = bundle.variants.len(),
            "InsightForge: retrieve_with_bundle sub-queries"
        );

        // Reuse existing fan-out + RRF merge logic
        self.fan_out_and_merge(&sub_queries, limit, session_key)
            .await
    }
```

- [ ] **Step 2: Extract fan-out logic into a shared method**

The existing `retrieve_with_enrichment` has the fan-out + RRF + budget allocation logic inline (lines ~177-295). Extract it into a `fan_out_and_merge` method that both `retrieve_with_enrichment` and `retrieve_with_bundle` can call. Read the full method first to determine exact extraction boundaries.

The new private method signature:

```rust
    async fn fan_out_and_merge(
        &self,
        sub_queries: &[String],
        limit: usize,
        session_key: Option<&str>,
    ) -> Vec<MemoryEntry> {
        // ... existing fan-out (lines ~177-294 from retrieve_with_enrichment) ...
    }
```

Then update `retrieve_with_enrichment` to call `fan_out_and_merge` after decomposition + enrichment injection.

- [ ] **Step 3: Add `depth_mode` to ContextRequest**

In `crates/context_engine/src/assembler/types.rs`, add to `ContextRequest`:

```rust
    /// Depth mode for query enhancement budget (default: Normal).
    pub depth_mode: crate::enhancement::EnhancementBudget,
```

And add `enhancement_trace` to `AssembledContext`:

```rust
    /// Enhancement pipeline trace (None if enhancement pipeline not configured).
    pub enhancement_trace: Option<crate::enhancement::EnhancementTrace>,
```

- [ ] **Step 4: Update the assembler to use QueryPipeline**

In `crates/context_engine/src/assembler/mod.rs`:

Add fields to `ContextEngine`:

```rust
    /// Optional query enhancement pipeline (replaces bare query_rewriter when set).
    query_pipeline: Option<Arc<crate::enhancement::QueryPipeline>>,
    /// Optional ranking pipeline for result reranking.
    ranking_pipeline: Option<Arc<crate::enhancement::RankingPipeline>>,
```

Add builder methods:

```rust
    pub fn with_query_pipeline(mut self, pipeline: Arc<crate::enhancement::QueryPipeline>) -> Self {
        self.query_pipeline = Some(pipeline);
        self
    }

    pub fn with_ranking_pipeline(mut self, pipeline: Arc<crate::enhancement::RankingPipeline>) -> Self {
        self.ranking_pipeline = Some(pipeline);
        self
    }
```

Replace the `retrieve_memory` method's rewrite section (lines ~442-533) to use the pipeline when available, falling back to the existing rewrite_or_spawn path when not configured.

- [ ] **Step 5: Run full context_engine tests**

Run: `cargo nextest run -p context_engine 2>&1 | tail -30`

---

### Task 7: Config Schema (`config/src/schema/cognitive.rs`)

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Add QueryEnhancementConfig**

Add after the existing `CognitiveConfig` struct:

```rust
/// Configuration for the query enhancement pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryEnhancementConfig {
    /// Master switch (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Pseudo-relevance feedback configuration.
    #[serde(default)]
    pub prf: PrfEnhancementConfig,

    /// Multi-query expansion configuration (LLM, Deep+ only).
    #[serde(default)]
    pub multi_query: MultiQueryEnhancementConfig,

    /// Reranking configuration.
    #[serde(default)]
    pub reranking: RerankingEnhancementConfig,
}

impl Default for QueryEnhancementConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prf: PrfEnhancementConfig::default(),
            multi_query: MultiQueryEnhancementConfig::default(),
            reranking: RerankingEnhancementConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrfEnhancementConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_prf_fetch_limit")]
    pub initial_fetch_limit: usize,
    #[serde(default = "default_prf_score_threshold")]
    pub min_score_threshold: f64,
    #[serde(default = "default_prf_max_terms")]
    pub max_expansion_terms: usize,
}

impl Default for PrfEnhancementConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_fetch_limit: 3,
            min_score_threshold: 0.6,
            max_expansion_terms: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiQueryEnhancementConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_multi_query_variants")]
    pub max_variants: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for MultiQueryEnhancementConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_variants: 3,
            model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RerankingEnhancementConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_rerank_top_n")]
    pub llm_rerank_top_n: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_rerank_model: Option<String>,
}

impl Default for RerankingEnhancementConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            llm_rerank_top_n: 10,
            llm_rerank_model: None,
        }
    }
}

fn default_prf_fetch_limit() -> usize { 3 }
fn default_prf_score_threshold() -> f64 { 0.6 }
fn default_prf_max_terms() -> usize { 5 }
fn default_multi_query_variants() -> usize { 3 }
fn default_rerank_top_n() -> usize { 10 }
```

Add to `CognitiveConfig`:

```rust
    /// Query enhancement pipeline configuration.
    #[serde(default)]
    pub query_enhancement: QueryEnhancementConfig,
```

- [ ] **Step 2: Verify config compiles**

Run: `cargo build -p config 2>&1 | head -20`

---

### Task 8: Signal Enrichment Stage (wraps existing rewriter)

**Files:**
- Create: `crates/agent/src/adapters/signal_enrichment.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

- [ ] **Step 1: Implement SignalEnrichmentStage**

Create `crates/agent/src/adapters/signal_enrichment.rs`:

```rust
//! Signal enrichment stage — wraps the existing ContextualQueryRewriter
//! heuristic logic as a QueryStage.

use async_trait::async_trait;

use context_engine::enhancement::{EnhancementBudget, QueryBundle, QuerySource, QueryStage};
use context_engine::rewriter::RetrievalContext;

use super::query_rewriter::ContextualQueryRewriter;

/// Wraps the existing heuristic query rewriter as a pipeline stage.
pub struct SignalEnrichmentStage {
    rewriter: ContextualQueryRewriter,
}

impl SignalEnrichmentStage {
    pub fn new(rewriter: ContextualQueryRewriter) -> Self {
        Self { rewriter }
    }
}

#[async_trait]
impl QueryStage for SignalEnrichmentStage {
    fn name(&self) -> QuerySource {
        QuerySource::SignalEnrichment
    }

    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        _budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle> {
        // Use the existing heuristic rewrite logic
        let result = self.rewriter.rewrite(&input.original, context).await;

        match result {
            Some(r) => Ok(QueryBundle {
                original: input.original,
                primary: r.enriched_query,
                variants: input.variants,
                confidence: r.confidence,
                sources: {
                    let mut s = input.sources;
                    s.push(QuerySource::SignalEnrichment);
                    s
                },
            }),
            None => Ok(input), // passthrough — no signals available
        }
    }
}
```

- [ ] **Step 2: Register in adapters/mod.rs**

Add `pub mod signal_enrichment;` to `crates/agent/src/adapters/mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -20`

---

### Task 9: Multi-Query Stage (LLM, Deep+)

**Files:**
- Create: `crates/agent/src/adapters/multi_query.rs`

- [ ] **Step 1: Implement MultiQueryStage**

Create `crates/agent/src/adapters/multi_query.rs`:

```rust
//! Multi-query expansion stage — LLM generates query variants (Deep+ only).

use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use context_engine::enhancement::{EnhancementBudget, QueryBundle, QuerySource, QueryStage};
use context_engine::rewriter::RetrievalContext;

/// Multi-query expansion via LLM.
pub struct MultiQueryStage {
    provider: Option<providers::DynProvider>,
    model: Option<String>,
}

impl MultiQueryStage {
    pub fn new(provider: Option<providers::DynProvider>, model: Option<String>) -> Self {
        Self { provider, model }
    }
}

#[async_trait]
impl QueryStage for MultiQueryStage {
    fn name(&self) -> QuerySource {
        QuerySource::MultiQuery
    }

    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle> {
        // Budget gate: skip in Normal mode (no LLM calls allowed)
        if budget.max_llm_calls == 0 {
            return Ok(input);
        }

        let provider = match &self.provider {
            Some(p) => p,
            None => return Ok(input),
        };

        let prompt = build_prompt(&input, context);
        let model = self
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model());

        let timeout_ms = budget.max_latency_ms.min(500);
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            provider.complete(
                &model,
                &[providers::Message::user(&prompt)],
                &providers::ChatParams {
                    max_tokens: Some(60),
                    temperature: Some(0.7),
                    ..Default::default()
                },
            ),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                let text = response.content_text();
                let variants: Vec<String> = text
                    .lines()
                    .map(|l| l.trim().trim_start_matches(|c: char| c == '-' || c == '*' || c.is_ascii_digit() || c == '.').trim().to_string())
                    .filter(|l| !l.is_empty() && l.len() > 5 && l.len() < 200)
                    .take(3)
                    .collect();

                if variants.is_empty() {
                    debug!("MultiQuery: LLM returned no valid variants");
                    return Ok(input);
                }

                debug!(variant_count = variants.len(), "MultiQuery: generated variants");
                let mut bundle = input;
                bundle.variants.extend(variants);
                bundle.sources.push(QuerySource::MultiQuery);
                Ok(bundle)
            }
            Ok(Err(e)) => {
                debug!("MultiQuery: LLM error: {e}");
                Ok(input) // graceful degradation
            }
            Err(_) => {
                debug!("MultiQuery: LLM timeout");
                Ok(input) // graceful degradation
            }
        }
    }
}

fn build_prompt(bundle: &QueryBundle, context: &RetrievalContext) -> String {
    let mut prompt = format!(
        "Generate 3 different search queries to find information relevant to: \"{}\"\n\
         Each query should approach from a different angle.\n\
         Return one query per line, nothing else.\n",
        bundle.primary
    );

    if let Some(ref skill) = context.active_skill {
        if skill != "general" {
            prompt.push_str(&format!("Context: user is in the {} domain.\n", skill));
        }
    }
    if let Some(ref task) = context.active_task {
        prompt.push_str(&format!("Current task: {}\n", task.title));
    }

    prompt
}
```

- [ ] **Step 2: Register in adapters/mod.rs**

Add `pub mod multi_query;` to `crates/agent/src/adapters/mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -20`

---

### Task 10: LLM Rerank Stage (Deep+)

**Files:**
- Create: `crates/agent/src/adapters/llm_rerank.rs`

- [ ] **Step 1: Implement LlmRerankStage**

Create `crates/agent/src/adapters/llm_rerank.rs`:

```rust
//! LLM reranking stage — pairwise relevance scoring (Deep+ only).

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use context_engine::enhancement::{EnhancementBudget, QueryBundle, QuerySource, RankingStage};
use context_engine::memory_retriever::MemoryEntry;

/// LLM-based reranking of top-N results.
pub struct LlmRerankStage {
    provider: Option<providers::DynProvider>,
    model: Option<String>,
    top_n: usize,
}

impl LlmRerankStage {
    pub fn new(provider: Option<providers::DynProvider>, model: Option<String>, top_n: usize) -> Self {
        Self {
            provider,
            model,
            top_n,
        }
    }
}

#[async_trait]
impl RankingStage for LlmRerankStage {
    fn name(&self) -> QuerySource {
        QuerySource::LlmRerank
    }

    async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<MemoryEntry>,
        budget: &EnhancementBudget,
    ) -> common::Result<Vec<MemoryEntry>> {
        // Budget gate
        if budget.max_llm_calls == 0 {
            return Ok(candidates);
        }

        let provider = match &self.provider {
            Some(p) => p,
            None => return Ok(candidates),
        };

        if candidates.is_empty() {
            return Ok(candidates);
        }

        let rerank_count = self.top_n.min(candidates.len());
        let (to_rerank, rest) = candidates.split_at(rerank_count);

        let prompt = build_rerank_prompt(&query.primary, to_rerank);
        let model = self
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model());

        let timeout_ms = budget.max_latency_ms.min(800);
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            provider.complete(
                &model,
                &[providers::Message::user(&prompt)],
                &providers::ChatParams {
                    max_tokens: Some(100),
                    temperature: Some(0.0),
                    ..Default::default()
                },
            ),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                let text = response.content_text();
                let scores = parse_scores(&text);

                let mut reranked: Vec<MemoryEntry> = to_rerank.to_vec();
                for entry in &mut reranked {
                    if let Some(&score) = scores.get(&entry.id) {
                        entry.score = score / 10.0; // normalize 0-10 to 0-1
                    }
                }
                reranked.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                reranked.extend_from_slice(rest);
                debug!(reranked_count = rerank_count, "LlmRerank: reranked results");
                Ok(reranked)
            }
            Ok(Err(e)) => {
                debug!("LlmRerank: LLM error: {e}");
                Ok(candidates) // graceful degradation
            }
            Err(_) => {
                debug!("LlmRerank: timeout");
                Ok(candidates) // graceful degradation
            }
        }
    }
}

fn build_rerank_prompt(query: &str, entries: &[MemoryEntry]) -> String {
    let mut prompt = format!(
        "Rate each item's relevance to the query on 0-10.\n\
         Query: \"{query}\"\n\n\
         Items:\n"
    );

    for entry in entries {
        let content_preview: String = entry.content.chars().take(120).collect();
        prompt.push_str(&format!("{}. {}\n", entry.id, content_preview));
    }

    prompt.push_str("\nReturn scores as: ID:SCORE (one per line, nothing else)");
    prompt
}

fn parse_scores(text: &str) -> HashMap<String, f64> {
    let mut scores = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some((id, score_str)) = line.split_once(':') {
            let id = id.trim().to_string();
            if let Ok(score) = score_str.trim().parse::<f64>() {
                scores.insert(id, score.clamp(0.0, 10.0));
            }
        }
    }
    scores
}
```

- [ ] **Step 2: Register in adapters/mod.rs**

Add `pub mod llm_rerank;` to `crates/agent/src/adapters/mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -20`

---

### Task 11: CorrectionTracker

**Files:**
- Create: `crates/agent/src/adapters/correction_tracker.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

- [ ] **Step 1: Implement CorrectionTracker**

Create `crates/agent/src/adapters/correction_tracker.rs`:

```rust
//! Tracks user corrections per session via DomainEventBus.
//!
//! Lightweight in-memory buffer — no DB, no persistence. Captures the most
//! recent correction per session key for injection into RetrievalContext.

use std::collections::HashMap;
use std::sync::Arc;

use bus::DomainEventBus;
use context_engine::rewriter::CorrectionContext;
use tokio::sync::RwLock;

/// In-memory correction tracker. Holds last correction per session.
#[derive(Clone)]
pub struct CorrectionTracker {
    corrections: Arc<RwLock<HashMap<String, CorrectionContext>>>,
}

impl CorrectionTracker {
    pub fn new() -> Self {
        Self {
            corrections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start listening for correction events on the event bus.
    /// Returns a JoinHandle for the listener task.
    pub fn start_listener(
        &self,
        bus: Arc<DomainEventBus>,
    ) -> tokio::task::JoinHandle<()> {
        let corrections = self.corrections.clone();
        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let bus::DomainEvent::UserCorrectedAI {
                    session_key,
                    rejected_topic,
                    corrected_to,
                    ..
                } = event
                {
                    let ctx = CorrectionContext {
                        rejected_topic,
                        corrected_to,
                    };
                    corrections.write().await.insert(session_key, ctx);
                }
            }
        })
    }

    /// Get the most recent correction for a session (if any).
    pub async fn latest_for_session(&self, session_key: &str) -> Option<CorrectionContext> {
        self.corrections.read().await.get(session_key).cloned()
    }

    /// Clear correction for a session (e.g., after it's been used).
    pub async fn clear(&self, session_key: &str) {
        self.corrections.write().await.remove(session_key);
    }
}
```

- [ ] **Step 2: Register in adapters/mod.rs**

Add `pub mod correction_tracker;` to `crates/agent/src/adapters/mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -20`

Note: The exact `DomainEvent::UserCorrectedAI` variant shape must match the bus crate. Read `crates/bus/src/lib.rs` to verify the variant fields before implementing. If the variant doesn't exist or has different fields, adapt accordingly.

---

### Task 12: Autotuner TrialParams Extension

**Files:**
- Modify: `crates/common/src/autotuner.rs`

- [ ] **Step 1: Add Phase 5 fields to TrialParams**

In `crates/common/src/autotuner.rs`, add after the Phase 4 fields (around line 60):

```rust
    // Phase 5: Query Enhancement Pipeline
    /// PRF minimum score threshold (bounds [0.3, 0.9]).
    pub prf_score_threshold: Option<f64>,
    /// PRF max expansion terms (bounds [2, 8]).
    pub prf_max_expansion_terms: Option<usize>,
    /// Heuristic rerank term-overlap weight (bounds [0.01, 0.15]).
    pub rerank_term_overlap_weight: Option<f64>,
    /// Multi-query max variants (bounds [1, 5]).
    pub multi_query_max_variants: Option<usize>,
    /// Override Normal mode latency budget in ms (bounds [50, 300]).
    pub enhancement_budget_latency_ms: Option<u64>,
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p common 2>&1 | head -20`

Since `TrialParams` derives `Default` and all fields are `Option`, this is backward-compatible.

---

### Task 13: Builder Wiring

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

This is the integration task that wires everything together.

- [ ] **Step 1: Replace bare rewriter with pipeline**

In `crates/agent/src/agent_loop/builder.rs`, replace the Phase 2/3 section (lines ~1092-1109) with:

```rust
        // Phase 2: Build query enhancement pipeline
        let rewriter_provider = self.cognitive_provider.clone();
        let rewriter_model = config.agents.rewriter_model.clone();
        let query_rewriter = crate::adapters::query_rewriter::ContextualQueryRewriter::new(
            rewriter_provider.clone(),
            rewriter_model.clone(),
            800,
        );

        // Stage 1: Signal enrichment (wraps existing heuristic rewriter)
        let signal_stage = crate::adapters::signal_enrichment::SignalEnrichmentStage::new(
            query_rewriter,
        );

        // Stage 2: PRF (heuristic, no LLM — needs memory_retriever)
        let prf_config = context_engine::enhancement::prf::PrfConfig {
            initial_fetch_limit: config.cognitive.query_enhancement.prf.initial_fetch_limit,
            min_score_threshold: config.cognitive.query_enhancement.prf.min_score_threshold,
            max_expansion_terms: config.cognitive.query_enhancement.prf.max_expansion_terms,
        };

        // Stage 3: Multi-query expansion (LLM, Deep+ only)
        let multi_query_stage = crate::adapters::multi_query::MultiQueryStage::new(
            rewriter_provider.clone(),
            rewriter_model.clone(),
        );

        // Build query pipeline (PRF needs retriever — add it if available)
        let mut query_stages: Vec<Arc<dyn context_engine::QueryStage>> = vec![
            Arc::new(signal_stage),
        ];

        // PRF stage added only if memory_retriever is available
        // (it's constructed during InsightForge setup above)
        // We'll add it conditionally after InsightForge construction

        query_stages.push(Arc::new(multi_query_stage));

        let query_pipeline = Arc::new(context_engine::enhancement::QueryPipeline::new(query_stages));

        // Stage 4: Heuristic reranking (always on)
        let heuristic_rerank = context_engine::enhancement::heuristic_rerank::HeuristicRerankStage::new(
            context_engine::enhancement::heuristic_rerank::HeuristicRerankConfig::default(),
        );

        // Stage 4b: LLM reranking (Deep+ only)
        let llm_rerank = crate::adapters::llm_rerank::LlmRerankStage::new(
            rewriter_provider,
            rewriter_model,
            config.cognitive.query_enhancement.reranking.llm_rerank_top_n,
        );

        let ranking_pipeline = Arc::new(context_engine::enhancement::RankingPipeline::new(vec![
            Arc::new(heuristic_rerank),
            Arc::new(llm_rerank),
        ]));

        let context_engine = context_engine
            .with_query_pipeline(query_pipeline)
            .with_ranking_pipeline(ranking_pipeline);
```

Note: The PRF stage needs `Arc<dyn MemoryRetriever>`, which is constructed during InsightForge setup (above this code). Read the builder to determine exact insertion point. If the retriever is available, insert PRF between signal enrichment and multi-query.

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -30`

- [ ] **Step 3: Run full workspace tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`

---

### Task 14: Enhancement Trace Repo + Reforge Integration

**Files:**
- Create: `crates/cognitive/src/repos/enhancement_trace.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/services/reforge/feedback.rs`
- Modify: `crates/cognitive/src/services/compaction.rs`

- [ ] **Step 1: Create EnhancementTraceRepo**

Create `crates/cognitive/src/repos/enhancement_trace.rs`:

```rust
//! Repository for persisting enhancement pipeline traces for Reforge analysis.

use sqlx::SqlitePool;

pub struct EnhancementTraceRepo {
    pool: SqlitePool,
}

const MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS enhancement_trace_log (
    id               TEXT PRIMARY KEY,
    session_key      TEXT NOT NULL,
    depth_mode       TEXT NOT NULL,
    stages_json      TEXT NOT NULL,
    total_latency_ms INTEGER NOT NULL,
    total_llm_calls  INTEGER NOT NULL,
    query_confidence REAL NOT NULL,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_enhancement_trace_created ON enhancement_trace_log(created_at);
";

impl EnhancementTraceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(MIGRATION).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn insert(
        &self,
        id: &str,
        session_key: &str,
        depth_mode: &str,
        stages_json: &str,
        total_latency_ms: i64,
        total_llm_calls: i64,
        query_confidence: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO enhancement_trace_log \
             (id, session_key, depth_mode, stages_json, total_latency_ms, total_llm_calls, query_confidence) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(id)
        .bind(session_key)
        .bind(depth_mode)
        .bind(stages_json)
        .bind(total_latency_ms)
        .bind(total_llm_calls)
        .bind(query_confidence)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete traces older than `days` days. Returns count deleted.
    pub async fn delete_older_than(&self, days: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM enhancement_trace_log \
             WHERE julianday('now') - julianday(created_at) > ?1",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Load aggregate metrics for Reforge analysis.
    pub async fn load_aggregates_since(
        &self,
        since: &str,
    ) -> Result<Vec<EnhancementAggregate>, sqlx::Error> {
        let rows: Vec<EnhancementAggregate> = sqlx::query_as(
            "SELECT depth_mode, \
                    COUNT(*) as total_runs, \
                    AVG(total_latency_ms) as avg_latency_ms, \
                    AVG(total_llm_calls) as avg_llm_calls, \
                    AVG(query_confidence) as avg_confidence \
             FROM enhancement_trace_log \
             WHERE created_at >= ?1 \
             GROUP BY depth_mode",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EnhancementAggregate {
    pub depth_mode: String,
    pub total_runs: i64,
    pub avg_latency_ms: f64,
    pub avg_llm_calls: f64,
    pub avg_confidence: f64,
}
```

- [ ] **Step 2: Register in repos/mod.rs**

Add `pub mod enhancement_trace;` and `pub use enhancement_trace::EnhancementTraceRepo;` to `crates/cognitive/src/repos/mod.rs`.

- [ ] **Step 3: Add collect_enhancement_signals to Reforge feedback**

In `crates/cognitive/src/services/reforge/feedback.rs`, add:

```rust
/// Load enhancement pipeline aggregate metrics for Reforge analysis.
pub async fn collect_enhancement_signals(
    trace_repo: &crate::repos::EnhancementTraceRepo,
    since: &str,
) -> Vec<super::types::EnhancementSignal> {
    match trace_repo.load_aggregates_since(since).await {
        Ok(aggregates) => aggregates
            .into_iter()
            .map(|a| super::types::EnhancementSignal {
                depth_mode: a.depth_mode,
                total_runs: a.total_runs as u32,
                avg_latency_ms: a.avg_latency_ms,
                avg_llm_calls: a.avg_llm_calls,
                avg_confidence: a.avg_confidence,
            })
            .collect(),
        Err(e) => {
            warn!("Reforge feedback: failed to load enhancement signals: {e}");
            vec![]
        }
    }
}
```

Add `EnhancementSignal` to `crates/cognitive/src/services/reforge/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementSignal {
    pub depth_mode: String,
    pub total_runs: u32,
    pub avg_latency_ms: f64,
    pub avg_llm_calls: f64,
    pub avg_confidence: f64,
}
```

- [ ] **Step 4: Add trace cleanup to compaction**

In `crates/cognitive/src/services/compaction.rs`, add an optional `EnhancementTraceRepo` parameter and a cleanup step (7-day TTL) similar to the existing cleanup steps. Add `enhancement_traces_deleted: u64` to `CompactionResult`.

- [ ] **Step 5: Run cognitive tests**

Run: `cargo nextest run -p cognitive 2>&1 | tail -30`

---

### Task 15: Final Integration Test + Clippy

**Files:**
- Modify: `tests/integration/cognitive.rs` (or create a new test)

- [ ] **Step 1: Write end-to-end integration test**

Add a test that constructs a `QueryPipeline` with a mock signal stage + mock PRF, runs `enhance()`, and verifies the output shape:

```rust
#[tokio::test]
async fn test_query_enhancement_pipeline_end_to_end() {
    // Build a pipeline with mock stages
    // Run enhance()
    // Verify QueryBundle has expected sources, variants
    // Verify EnhancementTrace records all stages
}
```

- [ ] **Step 2: Run full workspace tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30`
Expected: 0 warnings.

- [ ] **Step 4: Run fmt check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

---

## Dependency Order

```
Task 1 (types) → Task 2 (traits) → Task 3 (pipeline) → Task 4 (PRF) → Task 5 (heuristic rerank)
                                                                              ↓
Task 7 (config) ─────────────────────────────────────────────────→ Task 6 (InsightForge + assembler)
                                                                              ↓
Task 8 (signal enrichment) → Task 9 (multi-query) → Task 10 (llm rerank) → Task 13 (builder wiring)
                                                                              ↓
Task 11 (correction tracker) ────────────────────────────────────────────→ Task 13
                                                                              ↓
Task 12 (autotuner) ─────────────────────────────────────────────────────→ Task 13
                                                                              ↓
Task 14 (trace repo + reforge) ──────────────────────────────────────────→ Task 15 (final test)
```

Tasks 1-5 are sequential (each builds on the previous). Tasks 7-12 can be parallelized. Task 13 (builder wiring) depends on all stage implementations. Task 15 is the final verification.
