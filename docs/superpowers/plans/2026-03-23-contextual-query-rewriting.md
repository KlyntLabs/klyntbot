# Contextual Query Rewriting — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `QueryRewriter` component that enriches vague user queries with contextual signals (active skill, task, energy, corrections) before vector search, improving retrieval relevance across all features.

**Architecture:** Standalone `QueryRewriter` trait in `context_engine` (L3), implemented as `ContextualQueryRewriter` in `agent` (L5). Produces an enriched query injected as an extra sub-query into InsightForge's existing fan-out. Original query always preserved. Heuristic-first with background LLM fallback.

**Tech Stack:** Rust, async_trait, tokio (spawn, timeout, select), serde, existing `DynProvider` for LLM fallback.

**Spec:** `docs/superpowers/specs/2026-03-23-contextual-query-rewriting-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/context_engine/src/rewriter.rs` | Create | `QueryRewriter` trait, `RewriteResult`, `RewriteSource`, `RetrievalContext`, `ActiveTaskContext`, `ActiveView`, `CorrectionContext` |
| `crates/context_engine/src/lib.rs` | Modify | Export new `rewriter` module |
| `crates/context_engine/src/assembler/types.rs` | Modify | Add `retrieval_context` field to `ContextRequest` |
| `crates/context_engine/src/assembler/mod.rs` | Modify | Add `query_rewriter` field + builder method to `ContextEngine`. Call rewriter in `retrieve_memory()`. Update `compute_cache_key()` |
| `crates/context_engine/src/insight_forge/mod.rs` | Modify | Add `retrieve_with_enrichment()` method. Refactor `retrieve()` to delegate |
| `crates/agent/src/adapters/query_rewriter.rs` | Create | `ContextualQueryRewriter` — specificity check, heuristic templates, LLM fallback |
| `crates/agent/src/adapters/mod.rs` | Modify | Export `query_rewriter` module |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Add `user_situation` field. Build `RetrievalContext` at Step 5.5 |
| `crates/agent/src/agent_loop/mod.rs` | Modify | Add `last_correction` field, set on detection, pass through `run_pipeline` |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `QueryRewriter` into `ContextEngine`. Thread situation + task state to runtime |

---

## Task 1: Define types and trait in `context_engine`

**Files:**
- Create: `crates/context_engine/src/rewriter.rs`
- Modify: `crates/context_engine/src/lib.rs:17-32` (add export)

- [ ] **Step 1: Create `rewriter.rs` with all types and the trait**

```rust
// crates/context_engine/src/rewriter.rs
//
// IMPORTANT: context_engine is L3, cognitive is L5. We CANNOT import
// cognitive::UserSituation here. Instead we define a local snapshot struct
// with the fields the rewriter needs. The agent crate (L5) maps from
// cognitive::UserSituation → UserSituationSnapshot when building RetrievalContext.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Result of a query rewrite attempt.
#[derive(Debug, Clone)]
pub struct RewriteResult {
    pub enriched_query: String,
    pub confidence: f32,
    pub source: RewriteSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteSource {
    Heuristic,
    Llm,
}

/// Snapshot of user situation signals relevant to query rewriting.
/// Mirrors a subset of cognitive::UserSituation fields — populated
/// by the agent crate when building RetrievalContext.
#[derive(Debug, Clone, Default)]
pub struct UserSituationSnapshot {
    pub energy_level: f64,
    pub focus_state: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
}

/// Rich context available at retrieval time for query enrichment.
#[derive(Debug, Clone, Default)]
pub struct RetrievalContext {
    pub active_skill: Option<String>,
    pub active_task: Option<ActiveTaskContext>,
    pub recent_user_messages: Vec<String>,
    pub situation: Option<UserSituationSnapshot>,
    pub active_view: Option<ActiveView>,
    pub recent_correction: Option<CorrectionContext>,
}

#[derive(Debug, Clone)]
pub struct ActiveTaskContext {
    pub title: String,
    pub project_name: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveView {
    pub dashboard: String,
    pub focused_entity: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CorrectionContext {
    pub rejected_topic: String,
    pub corrected_to: String,
}

#[async_trait]
pub trait QueryRewriter: Send + Sync {
    async fn rewrite(
        &self,
        original: &str,
        context: &RetrievalContext,
    ) -> Option<RewriteResult>;
}
```

- [ ] **Step 2: Add module export to `lib.rs`**

In `crates/context_engine/src/lib.rs`, add after the existing module declarations:

```rust
pub mod rewriter;
pub use rewriter::{
    ActiveTaskContext, ActiveView, CorrectionContext, QueryRewriter, RetrievalContext,
    RewriteResult, RewriteSource, UserSituationSnapshot,
};
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p context-engine`
Expected: SUCCESS (no downstream consumers yet)

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/rewriter.rs crates/context_engine/src/lib.rs
git commit -m "feat(context-engine): add QueryRewriter trait and RetrievalContext types"
```

---

## Task 2: Add `retrieval_context` to `ContextRequest`

**Files:**
- Modify: `crates/context_engine/src/assembler/types.rs:22-37`
- Modify: `crates/context_engine/src/assembler/mod.rs` (all ContextRequest struct literals in tests)

- [ ] **Step 1: Add the field to ContextRequest**

In `crates/context_engine/src/assembler/types.rs`, add to the `ContextRequest` struct after `session_key`:

```rust
    /// Contextual signals for query rewriting (active skill, task, situation, etc.)
    pub retrieval_context: Option<crate::rewriter::RetrievalContext>,
```

- [ ] **Step 2: Fix all struct literal instantiations**

Every place that constructs `ContextRequest` as a struct literal will fail to compile. Add `retrieval_context: None` to each. Run `cargo check -p context-engine` and fix each error. There are ~9 sites in `assembler/mod.rs` tests and any call sites in the `agent` crate.

Run: `cargo check --workspace 2>&1 | head -40`

Fix each error by adding `retrieval_context: None` to the struct literal.

- [ ] **Step 3: Verify full workspace compiles**

Run: `cargo check --workspace`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/assembler/
git commit -m "feat(context-engine): add retrieval_context field to ContextRequest"
```

---

## Task 3: Wire `QueryRewriter` into `ContextEngine` and update cache key

**Files:**
- Modify: `crates/context_engine/src/assembler/mod.rs:28-40` (struct), `65-131` (builders), `194-221` (cache key), `365-434` (retrieve_memory)

- [ ] **Step 1: Write tests for cache key behavior**

Add to the test module in `crates/context_engine/src/assembler/mod.rs`:

```rust
#[test]
fn cache_key_varies_by_skill() {
    let mut req1 = test_request();
    req1.retrieval_context = Some(crate::rewriter::RetrievalContext {
        active_skill: Some("finance-management".into()),
        ..Default::default()
    });
    let mut req2 = test_request();
    req2.retrieval_context = Some(crate::rewriter::RetrievalContext {
        active_skill: Some("task-management".into()),
        ..Default::default()
    });
    assert_ne!(
        ContextEngine::compute_cache_key(&req1),
        ContextEngine::compute_cache_key(&req2),
    );
}

#[test]
fn cache_key_varies_by_task() {
    let mut req1 = test_request();
    req1.retrieval_context = Some(crate::rewriter::RetrievalContext {
        active_task: Some(crate::rewriter::ActiveTaskContext {
            title: "March budget review".into(),
            project_name: None,
            domain: None,
        }),
        ..Default::default()
    });
    let mut req2 = test_request();
    req2.retrieval_context = Some(crate::rewriter::RetrievalContext {
        active_task: Some(crate::rewriter::ActiveTaskContext {
            title: "API migration".into(),
            project_name: None,
            domain: None,
        }),
        ..Default::default()
    });
    assert_ne!(
        ContextEngine::compute_cache_key(&req1),
        ContextEngine::compute_cache_key(&req2),
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context-engine -E 'test(cache_key_varies)'`
Expected: FAIL (cache key doesn't include retrieval context yet)

- [ ] **Step 3: Add `query_rewriter` field and builder to `ContextEngine`**

In the `ContextEngine` struct (line ~28), add:

```rust
    query_rewriter: Option<Arc<dyn crate::rewriter::QueryRewriter>>,
```

Add the `Default` init:

```rust
    query_rewriter: None,
```

Add builder method after existing `with_*` methods:

```rust
    pub fn with_query_rewriter(mut self, rewriter: Arc<dyn crate::rewriter::QueryRewriter>) -> Self {
        self.query_rewriter = Some(rewriter);
        self
    }
```

- [ ] **Step 4: Update `compute_cache_key` to include retrieval context**

In `compute_cache_key` (line ~194), after `hasher.update(request.context_window.to_le_bytes());` add:

```rust
        // Hash retrieval context signals that affect enrichment
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
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p context-engine -E 'test(cache_key_varies)'`
Expected: PASS

- [ ] **Step 6: Add rewriter call in `retrieve_memory`**

In `retrieve_memory` (line ~365), before the InsightForge/retriever calls, add:

```rust
        // Query rewriting: enrich vague queries with contextual signals
        let enriched = match (&self.query_rewriter, &request.retrieval_context) {
            (Some(rewriter), Some(ctx)) => rewriter.rewrite(&request.message_text, ctx).await,
            _ => None,
        };
```

The `enriched` variable isn't consumed until Task 4 adds `retrieve_with_enrichment`. For now, add a temporary suppression to avoid unused-variable warnings:

```rust
        let _ = &enriched; // Used in Task 4 when retrieve_with_enrichment is added
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p context-engine`
Expected: SUCCESS

- [ ] **Step 8: Commit**

```bash
git add crates/context_engine/src/assembler/mod.rs
git commit -m "feat(context-engine): wire QueryRewriter into ContextEngine with cache key"
```

---

## Task 4: Add `retrieve_with_enrichment` to InsightForge

**Files:**
- Modify: `crates/context_engine/src/insight_forge/mod.rs:114-231` (retrieve method)

- [ ] **Step 1: Write test for backward compatibility**

Add to the InsightForge test module:

```rust
#[tokio::test]
async fn retrieve_delegates_to_enrichment_with_none() {
    // Verify that retrieve() produces identical results to retrieve_with_enrichment(None)
    // Construct InsightForge manually (no build_test_forge helper — construct inline
    // following the pattern used in existing InsightForge tests in this file)
    let retriever: Arc<dyn MemoryRetriever> = Arc::new(MockRetriever::new(/* test entries */));
    let decomposer: Arc<dyn QueryDecomposer> = Arc::new(HeuristicDecomposer);
    let forge = InsightForge::new(InsightForgeConfig::default(), decomposer, retriever);

    let result_old = forge.retrieve("test query", 10, None).await;
    let result_new = forge.retrieve_with_enrichment("test query", None, 10, None).await;
    assert_eq!(result_old.len(), result_new.len());
    for (a, b) in result_old.iter().zip(result_new.iter()) {
        assert_eq!(a.id, b.id);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p context-engine -E 'test(retrieve_delegates)'`
Expected: FAIL (method doesn't exist yet)

- [ ] **Step 3: Implement `retrieve_with_enrichment`**

Refactor the existing `retrieve` method body into `retrieve_with_enrichment`, adding the enriched query injection:

```rust
    /// Retrieve with an optional enriched query injected as an extra sub-query.
    pub async fn retrieve_with_enrichment(
        &self,
        query: &str,
        enriched: Option<&crate::rewriter::RewriteResult>,
        total_limit: usize,
        session_key: Option<&str>,
    ) -> Vec<MemoryEntry> {
        // ... existing circuit breaker check ...
        // ... existing decomposition with timeout ...

        // Inject enriched query at position 1 if present
        if let Some(result) = enriched {
            sub_queries.insert(1.min(sub_queries.len()), result.enriched_query.clone());
        }

        // ... existing fan-out, RRF merge, budget capping (unchanged) ...
    }

    /// Original method — delegates to enrichment with None.
    pub async fn retrieve(
        &self,
        query: &str,
        total_limit: usize,
        session_key: Option<&str>,
    ) -> Vec<MemoryEntry> {
        self.retrieve_with_enrichment(query, None, total_limit, session_key).await
    }
```

Move the entire body of the old `retrieve` into `retrieve_with_enrichment`. The old `retrieve` becomes a 1-line delegate. Preserve all timeout, circuit-breaker, and fallback logic.

- [ ] **Step 4: Update `retrieve_memory` in assembler to pass enrichment**

In `crates/context_engine/src/assembler/mod.rs`, in `retrieve_memory`, update the InsightForge call:

```rust
        // Was: forge.retrieve(&request.message_text, self.memory_retrieval_limit, ...)
        // Now:
        let entries = forge.retrieve_with_enrichment(
            &request.message_text,
            enriched.as_ref(),
            self.memory_retrieval_limit,
            request.session_key.as_deref(),
        ).await;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p context-engine`
Expected: ALL PASS (backward compat + existing tests)

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/insight_forge/mod.rs crates/context_engine/src/assembler/mod.rs
git commit -m "feat(context-engine): add retrieve_with_enrichment to InsightForge"
```

---

## Task 5: Implement `ContextualQueryRewriter` — specificity check + heuristic templates

**Files:**
- Create: `crates/agent/src/adapters/query_rewriter.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

This is the core implementation. Phase 1 = heuristic only, no LLM.

- [ ] **Step 1: Write unit tests for specificity check**

Create `crates/agent/src/adapters/query_rewriter.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_specificity_no_pronouns_with_entities() {
        let s = query_specificity("show me March FIRE projection");
        assert_eq!(s, Specificity::High);
    }

    #[test]
    fn low_specificity_pronouns() {
        let s = query_specificity("what was that thing?");
        assert_eq!(s, Specificity::Low);
    }

    #[test]
    fn low_specificity_short_no_entities() {
        let s = query_specificity("how are we");
        assert_eq!(s, Specificity::Low);
    }

    #[test]
    fn medium_specificity_no_pronouns_no_entities_long() {
        let s = query_specificity("tell me about the current status of everything");
        assert_eq!(s, Specificity::Medium);
    }

    #[test]
    fn pronouns_override_entities() {
        let s = query_specificity("what did John say about that auth thing?");
        assert_eq!(s, Specificity::Low);
    }
}
```

- [ ] **Step 2: Implement the specificity check**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Specificity {
    High,
    Medium,
    Low,
}

const PRONOUNS: &[&str] = &["that", "this", "it", "those", "these", "there", "them"];

fn contains_pronouns(query: &str) -> bool {
    let lower = query.to_lowercase();
    PRONOUNS.iter().any(|p| {
        lower.split_whitespace().any(|w| {
            let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
            trimmed == *p
        })
    })
}

fn has_domain_keywords(query: &str) -> bool {
    let lower = query.to_lowercase();
    // Named entities: month names, domain terms, proper nouns (capitalized words in original)
    let domain_terms = [
        "fire", "budget", "graphql", "rest", "api", "auth", "sprint", "okr",
        "january", "february", "march", "april", "may", "june", "july",
        "august", "september", "october", "november", "december",
    ];
    domain_terms.iter().any(|t| lower.contains(t))
        || query.split_whitespace().any(|w| {
            w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && w.len() > 1
                && !["What", "When", "Where", "How", "Why", "Who", "Can", "Could",
                     "Would", "Should", "Is", "Are", "Do", "Does", "Did", "The", "A"].contains(&w)
        })
}

fn query_specificity(query: &str) -> Specificity {
    let word_count = query.split_whitespace().count();
    let has_pronouns = contains_pronouns(query);
    let has_entities = has_domain_keywords(query);

    if has_pronouns {
        return Specificity::Low;
    }

    match (word_count, has_entities) {
        (_, true) if word_count >= 4 => Specificity::High,
        (1..=3, false) => Specificity::Low,
        _ => Specificity::Medium,
    }
}
```

- [ ] **Step 3: Run specificity tests**

Run: `cargo nextest run -p agent -E 'test(specificity)'`
Expected: ALL PASS

- [ ] **Step 4: Write tests for heuristic rewriter**

Add to the test module:

```rust
    use context_engine::rewriter::{
        ActiveTaskContext, CorrectionContext, RetrievalContext, RewriteSource,
        UserSituationSnapshot,
    };

    fn finance_context() -> RetrievalContext {
        RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "March budget review".into(),
                project_name: None,
                domain: Some("finance".into()),
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn heuristic_enriches_vague_query_with_skill_and_task() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = finance_context();
        let result = rewriter.rewrite("how are we doing?", &ctx).await;
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.enriched_query.to_lowercase().contains("march budget"));
        assert_eq!(r.source, RewriteSource::Heuristic);
        assert!(r.confidence >= 0.7);
    }

    #[tokio::test]
    async fn high_specificity_returns_none() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = finance_context();
        let result = rewriter.rewrite("show me March FIRE projection", &ctx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn correction_is_highest_priority() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext {
            active_skill: Some("task-management".into()),
            recent_correction: Some(CorrectionContext {
                rejected_topic: "wrong project".into(),
                corrected_to: "no, the GraphQL migration".into(),
            }),
            ..Default::default()
        };
        let result = rewriter.rewrite("any blockers?", &ctx).await;
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.enriched_query.to_lowercase().contains("graphql migration"));
    }

    #[tokio::test]
    async fn no_context_medium_specificity_returns_none() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext::default();
        let result = rewriter.rewrite("tell me about the progress on things", &ctx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn low_energy_includes_more_signals() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let situation = UserSituationSnapshot {
            energy_level: 0.2,
            ..Default::default()
        };
        let ctx = RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "March budget review".into(),
                project_name: Some("Q1 Finance".into()),
                domain: Some("finance".into()),
            }),
            situation: Some(situation),
            recent_user_messages: vec!["I was checking the spending breakdown".into()],
            ..Default::default()
        };
        let result = rewriter.rewrite("what about that?", &ctx).await;
        assert!(result.is_some());
        let query = result.unwrap().enriched_query.to_lowercase();
        // Low energy should include more signals (task + project + recent keywords)
        assert!(query.contains("march budget") || query.contains("spending"));
    }
```

- [ ] **Step 5: Implement `ContextualQueryRewriter` with heuristic templates**

```rust
use async_trait::async_trait;
use context_engine::rewriter::{
    QueryRewriter, RetrievalContext, RewriteResult, RewriteSource,
};

pub struct ContextualQueryRewriter {
    llm_provider: Option<providers::DynProvider>,
    rewriter_model: Option<String>,
    timeout_ms: u64,
}

impl ContextualQueryRewriter {
    pub fn new(
        llm_provider: Option<providers::DynProvider>,
        rewriter_model: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        Self { llm_provider, rewriter_model, timeout_ms }
    }

    /// Phase 1 constructor — heuristic only, no LLM.
    pub fn heuristic_only() -> Self {
        Self { llm_provider: None, rewriter_model: None, timeout_ms: 800 }
    }

    fn is_aggressive(&self, ctx: &RetrievalContext) -> bool {
        ctx.situation.as_ref().map_or(false, |s| {
            s.energy_level < 0.4 || s.deadline_pressure > 0.7
        })
    }

    fn max_signals(&self, ctx: &RetrievalContext) -> usize {
        if self.is_aggressive(ctx) { 4 } else { 2 }
    }

    fn heuristic_rewrite(&self, original: &str, ctx: &RetrievalContext) -> Option<RewriteResult> {
        let max = self.max_signals(ctx);
        let mut signals: Vec<String> = Vec::new();

        // Priority 1: Correction
        if let Some(ref corr) = ctx.recent_correction {
            signals.push(extract_key_terms_from(&corr.corrected_to));
        }
        // Priority 2: Active view
        if let Some(ref view) = ctx.active_view {
            if let Some(ref desc) = view.description {
                signals.push(desc.clone());
            }
        }
        // Priority 3: Active task
        if let Some(ref task) = ctx.active_task {
            let mut s = task.title.clone();
            if let Some(ref proj) = task.project_name {
                s = format!("{s} ({proj})");
            }
            signals.push(s);
        }
        // Priority 4: Skill domain
        if let Some(ref skill) = ctx.active_skill {
            let domain = skill.replace("-management", "").replace('-', " ");
            signals.push(domain);
        }
        // Priority 5: Recent message keywords
        for msg in ctx.recent_user_messages.iter().take(2) {
            let terms = extract_key_terms_from(msg);
            if !terms.is_empty() {
                signals.push(terms);
            }
        }

        signals.truncate(max);
        if signals.is_empty() {
            return None;
        }

        let enriched = build_template(original, &signals, ctx);
        let confidence = if ctx.recent_correction.is_some() { 0.9 } else { 0.75 };

        Some(RewriteResult {
            enriched_query: enriched,
            confidence,
            source: RewriteSource::Heuristic,
        })
    }
}

/// Extract key terms from text, filtering stop words. Public so AgentLoop
/// can use it for correction topic extraction (Task 7).
pub fn extract_key_terms_from(text: &str) -> String {
    let stop_words: std::collections::HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "shall", "can", "need", "dare", "ought",
        "used", "to", "of", "in", "for", "on", "with", "at", "by", "from",
        "as", "into", "through", "during", "before", "after", "above", "below",
        "between", "out", "off", "over", "under", "again", "further", "then",
        "once", "and", "but", "or", "nor", "not", "so", "yet", "both",
        "either", "neither", "each", "every", "all", "any", "few", "more",
        "most", "other", "some", "such", "no", "only", "own", "same", "than",
        "too", "very", "just", "about", "up", "i", "me", "my", "we", "our",
        "you", "your", "he", "him", "his", "she", "her", "it", "its", "they",
        "them", "their", "what", "which", "who", "whom", "this", "that",
        "these", "those", "am", "if", "how", "when", "where", "why",
        "because", "until", "while", "although", "though", "even",
        "no", "the", "one", "other", "don't", "didn't", "won't",
    ].into_iter().collect();

    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2 && !stop_words.contains(w.to_lowercase().as_str()))
        .take(5)
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_template(original: &str, signals: &[String], ctx: &RetrievalContext) -> String {
    let signal_text = signals.join(" — ");
    let original_terms = extract_key_terms_from(original);

    if ctx.recent_correction.is_some() {
        // Correction template: "What are the current {original_topic} on {corrected}?"
        if original_terms.is_empty() {
            format!("{signal_text} — current status and details")
        } else {
            format!("{original_terms} regarding {signal_text}")
        }
    } else if original_terms.is_empty() {
        // Pure context template: "{signals} — overview and current status"
        format!("{signal_text} — overview and current status")
    } else {
        // Fusion template: "{signals} — {original key terms}"
        format!("{signal_text} — {original_terms}")
    }
}

#[async_trait]
impl QueryRewriter for ContextualQueryRewriter {
    async fn rewrite(
        &self,
        original: &str,
        context: &RetrievalContext,
    ) -> Option<RewriteResult> {
        let specificity = query_specificity(original);

        match specificity {
            Specificity::High => None,
            Specificity::Medium => self.heuristic_rewrite(original, context),
            Specificity::Low => {
                // Try heuristic first
                if let Some(result) = self.heuristic_rewrite(original, context) {
                    return Some(result);
                }
                // LLM fallback (Phase 2 — currently returns None)
                // TODO: Implement llm_rewrite when Phase 2 is enabled
                None
            }
        }
    }
}
```

- [ ] **Step 6: Export the module**

In `crates/agent/src/adapters/mod.rs`, add:

```rust
pub mod query_rewriter;
```

- [ ] **Step 7: Run all tests**

Run: `cargo nextest run -p agent -E 'test(query_rewriter) + test(specificity)'`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/adapters/query_rewriter.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(agent): implement ContextualQueryRewriter with heuristic templates"
```

---

## Task 6: Build `RetrievalContext` in `AgentRuntime` (Step 5.5)

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:71-98` (struct), `407-420` (ContextRequest build)

- [ ] **Step 1: Add `user_situation` field to `AgentRuntime`**

In the `AgentRuntime` struct (line ~71), add:

```rust
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
```

Update the constructor (`new()`) to accept and store it. Add a builder method:

```rust
    pub fn with_user_situation(
        mut self,
        situation: Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>,
    ) -> Self {
        self.user_situation = Some(situation);
        self
    }
```

- [ ] **Step 2: Build `RetrievalContext` at Step 5.5**

In `process_message()`, after the intent analysis step and before ContextRequest construction (line ~407), add:

```rust
        // Step 5.5: Build retrieval context for query rewriting
        let retrieval_context = {
            let active_skill = profile.as_ref().map(|p| p.name().to_string());

            // Message is an enum (User/Assistant/System/Tool) — use .role() method
            let recent_user_messages: Vec<String> = history.iter()
                .rev()
                .filter(|m| m.role() == providers::MessageRole::User)
                .take(2)
                .map(|m| m.content_text().unwrap_or_default().chars().take(200).collect())
                .collect();

            // Map cognitive::UserSituation → context_engine::UserSituationSnapshot
            // (context_engine cannot depend on cognitive directly — L3 vs L5)
            let situation = if let Some(ref sit) = self.user_situation {
                let s = sit.lock().await;
                Some(context_engine::UserSituationSnapshot {
                    energy_level: s.energy_level,
                    focus_state: s.focus_state,
                    deadline_pressure: s.deadline_pressure,
                    distraction_risk: s.distraction_risk,
                })
            } else {
                None
            };

            // active_task and recent_correction are wired in later tasks
            Some(context_engine::RetrievalContext {
                active_skill,
                active_task: None,  // TODO: Wire from focused task (Task 8)
                recent_user_messages,
                situation,
                active_view: None,  // Phase 4
                recent_correction: None,  // TODO: Wire from AgentLoop (Task 7)
            })
        };
```

Then include it in the ContextRequest construction:

```rust
        retrieval_context,
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): build RetrievalContext at Step 5.5 in AgentRuntime"
```

---

## Task 7: Wire correction forwarding from `AgentLoop` to `AgentRuntime`

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs:45-91` (struct), `744-785` (run_pipeline), `851-915` (correction detection)

- [ ] **Step 1: Add `last_correction` field to `AgentLoop`**

In the `AgentLoop` struct, add:

```rust
    last_correction: Option<context_engine::CorrectionContext>,
```

Initialize to `None` in the constructor.

- [ ] **Step 2: Set correction in detection code**

In the correction detection block (~line 851-915), after `emit_correction_signal`, add:

```rust
        self.last_correction = Some(context_engine::CorrectionContext {
            rejected_topic: crate::adapters::query_rewriter::extract_key_terms_from(
                &last_assistant.chars().take(200).collect::<String>()
            ),
            corrected_to: content.to_string(),
        });
```

Note: `extract_key_terms_from` is `pub` in `adapters::query_rewriter` (made public in Task 5 for this purpose).

- [ ] **Step 3: Thread correction through `run_pipeline` to runtime**

Add `correction: Option<context_engine::CorrectionContext>` as a parameter to `run_pipeline()`. Update all call sites (there are ~4) to pass `self.last_correction.take()` at the primary call site and `None` at the others.

In `run_pipeline`, include the correction in the `process_message` call so it reaches the `RetrievalContext`. This requires the runtime's `process_message` to accept correction as a parameter, or pass it via the existing parameter set.

The cleanest approach: add `correction: Option<CorrectionContext>` to the tuple of arguments to `runtime.process_message()`, then use it when building `RetrievalContext` at Step 5.5.

- [ ] **Step 4: Clear after use**

After `run_pipeline` reads `last_correction`, it's consumed by `.take()` — automatically cleared for the next message.

- [ ] **Step 5: Verify compilation + existing tests pass**

Run: `cargo nextest run -p agent`
Expected: ALL PASS (no behavioral change yet — correction is `None` in RetrievalContext until set)

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire correction forwarding from AgentLoop to RetrievalContext"
```

---

## Task 8: Wire `QueryRewriter` in `AgentLoopBuilder` and thread situation to runtime

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:640-815` (wiring section)

- [ ] **Step 1: Construct and wire `ContextualQueryRewriter`**

In the builder's `build()` method, after the ContextEngine is constructed but before it's finalized (around line 810), add:

```rust
        // Wire query rewriter (Phase 1: heuristic only)
        let query_rewriter = Arc::new(
            crate::adapters::query_rewriter::ContextualQueryRewriter::heuristic_only()
        );
        context_engine = context_engine.with_query_rewriter(query_rewriter as Arc<dyn context_engine::QueryRewriter>);
```

- [ ] **Step 2: Thread `user_situation` to `AgentRuntime`**

The builder already holds `self.user_situation: Option<Arc<Mutex<UserSituation>>>`. Pass it to the runtime:

```rust
        let mut runtime = AgentRuntime::new(/* existing args */);
        if let Some(ref sit) = self.user_situation {
            runtime = runtime.with_user_situation(Arc::clone(sit));
        }
```

- [ ] **Step 3: Verify full workspace compiles and tests pass**

Run: `cargo check --workspace && cargo nextest run --workspace`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire QueryRewriter and UserSituation into runtime via builder"
```

---

## Task 9: Integration test — end-to-end rewrite improves retrieval

**Files:**
- Create or modify: integration test in `tests/` or `crates/agent/src/adapters/query_rewriter.rs`

- [ ] **Step 1: Write integration test**

```rust
#[tokio::test]
async fn rewrite_augments_not_replaces_in_insight_forge() {
    // Setup: InsightForge with a mock retriever
    // Insert facts that match "GraphQL migration" but not "any blockers?"
    //
    // Test: Call retrieve_with_enrichment with:
    //   original = "any blockers?"
    //   enriched = RewriteResult { enriched_query: "GraphQL migration blockers", ... }
    //
    // Assert: Results include GraphQL-related entries that wouldn't appear
    //         from "any blockers?" alone
    //
    // Also assert: Both the original and enriched sub-queries are in the fan-out
}
```

- [ ] **Step 2: Write integration test for graceful degradation**

```rust
#[tokio::test]
async fn no_rewriter_degrades_gracefully() {
    // Setup: ContextEngine with query_rewriter: None
    // Process a vague query
    // Assert: Works exactly as before — no panics, no errors
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo nextest run -E 'test(rewrite_augments)' -E 'test(no_rewriter_degrades)'`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add tests/ crates/
git commit -m "test(agent): add integration tests for contextual query rewriting"
```

---

## Task 10: Final verification — clippy, fmt, full test suite

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 2: Run fmt check**

Run: `cargo fmt --all --check`
Expected: SUCCESS

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: ALL PASS

- [ ] **Step 4: Run doc tests**

Run: `cargo test --workspace --doc`
Expected: ALL PASS

- [ ] **Step 5: Final commit if any fixups needed**

```bash
git add -A && git commit -m "chore: clippy and fmt fixes for contextual query rewriting"
```

---

## Summary

| Task | Description | Estimated Lines | Dependencies |
|------|-------------|-----------------|-------------|
| 1 | Types + trait in context_engine | ~65 | None |
| 2 | Add `retrieval_context` to ContextRequest | ~15 + ~9 fixups | Task 1 |
| 3 | Wire QueryRewriter into ContextEngine + cache key | ~50 | Tasks 1, 2 |
| 4 | `retrieve_with_enrichment` in InsightForge | ~40 | Task 3 |
| 5 | `ContextualQueryRewriter` (specificity + heuristic) | ~280 | Tasks 1, 4 |
| 6 | Build RetrievalContext in AgentRuntime | ~40 | Tasks 2, 5 |
| 7 | Wire correction forwarding | ~25 | Task 6 |
| 8 | Wire rewriter + situation in builder | ~20 | Tasks 5, 6, 7 |
| 9 | Integration tests | ~60 | Task 8 |
| 10 | Final verification | ~5 | Task 9 |

**Total: ~610 lines** (slightly above spec estimate of ~505 due to test code)

**Phase 1 delivers:** Heuristic-only rewriting covering Moments 2, 3, 4, 6, 7. Zero latency overhead. Zero cost. LLM fallback (Phase 2) requires adding the provider call in Task 5's `Specificity::Low` branch.
