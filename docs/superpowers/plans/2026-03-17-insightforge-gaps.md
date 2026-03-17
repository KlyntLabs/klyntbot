# InsightForge Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining gaps in the InsightForge multi-dimensional retrieval pipeline — FinanceSearcher, LLM-backed decomposer, session_key passthrough, entity backfill, and GraphSearcher neighborhood expansion.

**Architecture:** InsightForge core is already implemented and wired at startup (`agent/src/agent_loop/builder.rs:619-664`). Three domain searchers exist (Notes FTS, Tasks keyword, Graph name-match). This plan adds the missing FinanceSearcher, an LLM decomposer fallback, fixes the circuit breaker session_key, adds entity backfill on startup, and enhances the GraphSearcher to traverse neighborhoods.

**Tech Stack:** Rust (async-trait, tokio, sqlx), feature-finance/feature-tasks/cognitive crates, context_engine InsightForge module

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/agent/src/domain_searchers/finance_searcher.rs` | DomainSearcher impl wrapping `FinanceTransactionRepo::list` |
| `crates/context_engine/src/insight_forge/llm_decomposer.rs` | LLM-backed QueryDecomposer using cheap model (Gemini Flash) |

### Modified files

| File | Change |
|------|--------|
| `crates/agent/src/domain_searchers/mod.rs` | Register `finance_searcher` module + re-export |
| `crates/agent/src/domain_searchers/graph_searcher.rs` | Expand from name-match to neighborhood traversal |
| `crates/agent/src/agent_loop/builder.rs:646-657` | Register FinanceSearcher + swap HeuristicDecomposer for `FallbackDecomposer(heuristic, llm)` |
| `crates/context_engine/src/insight_forge/mod.rs` | Pass session_key from `retrieve_memory` call |
| `crates/context_engine/src/insight_forge/decomposer.rs` | Add `FallbackDecomposer` that tries heuristic first, then LLM |
| `crates/context_engine/src/assembler/mod.rs:364` | Pass session_key from `ContextRequest` instead of `None` |
| `crates/context_engine/src/assembler/types.rs` | Add `session_key: Option<String>` to `ContextRequest` |
| `crates/context_engine/src/lib.rs` | Re-export `LlmDecomposer`, `FallbackDecomposer` |
| `crates/app-core/src/init/agent.rs` or `crates/agent/src/agent_loop/builder.rs` | Entity backfill startup job |
| `crates/cognitive/src/repos/entity.rs` | Add `backfill_from_facts` method |

---

## Chunk 1: FinanceSearcher

### Task 1: Create FinanceSearcher

**Files:**
- Create: `crates/agent/src/domain_searchers/finance_searcher.rs`
- Modify: `crates/agent/src/domain_searchers/mod.rs`

- [ ] **Step 1: Create the searcher**

```rust
// crates/agent/src/domain_searchers/finance_searcher.rs
use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use storage::Repos;

pub struct FinanceSearcher {
    repos: Repos,
}

impl FinanceSearcher {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl DomainSearcher for FinanceSearcher {
    fn domain_name(&self) -> &str {
        "finance"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        use storage::rows::finance::FinanceTransactionFilter;
        let filter = FinanceTransactionFilter {
            query: Some(query.to_string()),
            limit: Some(limit as i64),
            ..Default::default()
        };

        let rows = match self.repos.finance.transactions.list(&filter).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        rows.into_iter()
            .enumerate()
            .map(|(i, tx)| {
                let amount_display = format!("{:.2}", tx.amount as f64 / 100.0);
                let desc = format!(
                    "[Transaction: {} {} {} on {}] {}",
                    tx.tx_type,
                    amount_display,
                    tx.currency.as_deref().unwrap_or(""),
                    tx.tx_date,
                    tx.notes.as_deref().unwrap_or(""),
                );
                MemoryEntry {
                    id: tx.id.clone(),
                    content: desc,
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain {
                        name: "finance".into(),
                    },
                    raw_score: 0.0,
                }
            })
            .collect()
    }
}
```

- [ ] **Step 2: Register in mod.rs**

In `crates/agent/src/domain_searchers/mod.rs`, add:

```rust
pub mod finance_searcher;

pub use finance_searcher::FinanceSearcher;
```

- [ ] **Step 3: Wire in builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, after the GraphSearcher registration (~line 657), add:

```rust
forge.add_searcher(Arc::new(crate::domain_searchers::FinanceSearcher::new(
    repos.clone(),
)));
```

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p agent -E 'test(insight_forge)'`

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/domain_searchers/
git commit -m "feat(agent): add FinanceSearcher domain searcher for InsightForge"
```

---

## Chunk 2: Session Key Passthrough

### Task 2: Wire session_key through ContextRequest to InsightForge

The circuit breaker tracks failures per-session, but `session_key` is always passed as `None` at `assembler/mod.rs:364`. This means the circuit breaker never actually trips.

**Files:**
- Modify: `crates/context_engine/src/assembler/types.rs`
- Modify: `crates/context_engine/src/assembler/mod.rs:364`

- [ ] **Step 1: Add session_key to ContextRequest**

In `crates/context_engine/src/assembler/types.rs`, add the field to `ContextRequest`:

```rust
pub struct ContextRequest {
    pub message_text: String,
    pub history: Vec<Message>,
    pub system_prompt: String,
    pub strategy: ExecutionStrategy,
    pub tool_definitions: Vec<serde_json::Value>,
    pub context_window: usize,
    /// Optional session key for circuit-breaker tracking.
    pub session_key: Option<String>,
}
```

Then `cargo build --workspace` to find all construction sites that need updating — add `session_key: None` to each (or `session_key: Some(session_key.to_string())` where session key is available). The main call site is in `agent/src/agent_runtime/runtime.rs` where `ContextRequest` is built before calling `context_engine.assemble()`.

- [ ] **Step 2: Pass session_key in retrieve_memory**

In `crates/context_engine/src/assembler/mod.rs`, change line ~364 from:

```rust
forge.retrieve(&request.message_text, self.memory_retrieval_limit, None).await
```

to:

```rust
forge.retrieve(&request.message_text, self.memory_retrieval_limit, request.session_key.as_deref()).await
```

- [ ] **Step 3: Populate session_key at call sites**

Search for `ContextRequest` construction sites (likely in `agent/src/agent_runtime/runtime.rs` or `agent/src/agent_loop/`). Pass the session key from the agent's session context into `ContextRequest::session_key`.

Run: `cargo build --workspace` to find any compilation errors from the new field.

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/ crates/agent/
git commit -m "fix(context-engine): pass session_key to InsightForge circuit breaker"
```

---

## Chunk 3: GraphSearcher Neighborhood Expansion

### Task 3: Enhance GraphSearcher to traverse neighborhoods

Currently `GraphSearcher` only does `find_by_name` — it finds entities matching the query but doesn't traverse the graph. When it finds a matching entity, it should also expand 1-hop neighbors for richer context.

**Files:**
- Modify: `crates/agent/src/domain_searchers/graph_searcher.rs`

- [ ] **Step 1: Expand search to include neighborhood**

Replace the current `search` implementation:

```rust
#[async_trait]
impl DomainSearcher for GraphSearcher {
    fn domain_name(&self) -> &str {
        "graph"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let entities = match self.entity_repo.find_by_name(query).await {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut entries = Vec::new();

        // Add matched entities
        for (i, entity) in entities.iter().take(limit / 2).enumerate() {
            let desc = entity.description.as_deref().unwrap_or("no description");
            entries.push(MemoryEntry {
                id: entity.id.clone(),
                content: format!(
                    "[Entity: {} ({})] {} (seen {} times)",
                    entity.name, entity.entity_type, desc, entity.mention_count
                ),
                score: 1.0 / (1.0 + i as f64),
                source: MemorySource::Domain {
                    name: "graph".into(),
                },
                raw_score: entity.mention_count as f64,
            });

            // Expand 1-hop neighborhood for the top match only
            if i == 0 {
                if let Ok(Some(hood)) = self.entity_repo.get_neighborhood(&entity.id, 1).await {
                    for (j, neighbor) in hood.neighbors.iter().enumerate() {
                        if entries.len() >= limit {
                            break;
                        }
                        // Find the relationship connecting this neighbor
                        let rel_type = hood
                            .relationships
                            .iter()
                            .find(|r| {
                                r.target_entity_id == neighbor.id
                                    || r.source_entity_id == neighbor.id
                            })
                            .map(|r| r.relationship_type.as_str())
                            .unwrap_or("related");

                        let n_desc = neighbor.description.as_deref().unwrap_or("");
                        entries.push(MemoryEntry {
                            id: neighbor.id.clone(),
                            content: format!(
                                "[Connected: {} ({}) — {} → {}] {}",
                                neighbor.name,
                                neighbor.entity_type,
                                rel_type,
                                entity.name,
                                n_desc
                            ),
                            score: 0.5 / (1.0 + j as f64),
                            source: MemorySource::Domain {
                                name: "graph".into(),
                            },
                            raw_score: neighbor.mention_count as f64,
                        });
                    }
                }
            }
        }

        entries.truncate(limit);
        entries
    }
}
```

- [ ] **Step 2: Build + test**

Run: `cargo build --workspace`

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/domain_searchers/graph_searcher.rs
git commit -m "feat(agent): expand GraphSearcher to traverse 1-hop neighborhoods"
```

---

## Chunk 4: LLM-Backed Decomposer with Fallback

### Task 4: Add LlmDecomposer

The spec calls for a cheap LLM (Gemini Flash) decomposer as fallback when the heuristic produces insufficient sub-queries. The `FallbackDecomposer` tries heuristic first; if it produces < 3 sub-queries, falls back to LLM.

**Files:**
- Create: `crates/context_engine/src/insight_forge/llm_decomposer.rs`
- Modify: `crates/context_engine/src/insight_forge/decomposer.rs`
- Modify: `crates/context_engine/src/insight_forge/mod.rs`
- Modify: `crates/context_engine/src/lib.rs`

- [ ] **Step 1: Create LlmDecomposer**

```rust
// crates/context_engine/src/insight_forge/llm_decomposer.rs
use async_trait::async_trait;

use super::decomposer::QueryDecomposer;

/// Trait for LLM providers used by the decomposer.
/// Lives here (L3) to avoid depending on the `providers` crate directly.
#[async_trait]
pub trait DecomposerLlm: Send + Sync {
    /// Single-turn chat returning the response text.
    async fn generate(&self, prompt: &str) -> Result<String, String>;
}

/// LLM-backed query decomposer.
///
/// Asks a cheap model to break a query into sub-queries.
/// The caller wraps this in a timeout via `InsightForge::retrieve`.
pub struct LlmDecomposer {
    llm: std::sync::Arc<dyn DecomposerLlm>,
}

impl LlmDecomposer {
    pub fn new(llm: std::sync::Arc<dyn DecomposerLlm>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl QueryDecomposer for LlmDecomposer {
    async fn decompose(&self, query: &str, _context_hint: Option<&str>) -> Vec<String> {
        let prompt = format!(
            r#"Break the following user message into 3-5 distinct search queries that would help find all relevant information. Each query should focus on a different aspect (facts, relationships, timeline, context, risks).

User message: {query}

Respond with ONLY a JSON array of strings, no explanation:
["query 1", "query 2", "query 3"]"#
        );

        match self.llm.generate(&prompt).await {
            Ok(response) => {
                // Try to parse as JSON array
                let trimmed = response.trim();
                if let Ok(queries) = serde_json::from_str::<Vec<String>>(trimmed) {
                    let mut result = vec![query.to_string()];
                    result.extend(queries);
                    result.truncate(5);
                    result
                } else {
                    vec![query.to_string()]
                }
            }
            Err(_) => vec![query.to_string()],
        }
    }
}
```

- [ ] **Step 2: Add FallbackDecomposer to decomposer.rs**

Append to `crates/context_engine/src/insight_forge/decomposer.rs`:

```rust
/// Tries the heuristic decomposer first; if it produces fewer than
/// `min_heuristic_queries` sub-queries, falls back to the LLM decomposer.
pub struct FallbackDecomposer {
    heuristic: HeuristicDecomposer,
    llm: Arc<dyn QueryDecomposer>,
    min_heuristic_queries: usize,
}

impl FallbackDecomposer {
    pub fn new(llm: Arc<dyn QueryDecomposer>, min_heuristic_queries: usize) -> Self {
        Self {
            heuristic: HeuristicDecomposer,
            llm,
            min_heuristic_queries,
        }
    }
}

#[async_trait]
impl QueryDecomposer for FallbackDecomposer {
    async fn decompose(&self, query: &str, context_hint: Option<&str>) -> Vec<String> {
        let heuristic_result = self.heuristic.decompose(query, context_hint).await;
        if heuristic_result.len() >= self.min_heuristic_queries {
            return heuristic_result;
        }
        // Heuristic insufficient — try LLM
        self.llm.decompose(query, context_hint).await
    }
}
```

Add `use std::sync::Arc;` to the imports at the top of the file.

- [ ] **Step 3: Register in mod.rs and lib.rs**

In `crates/context_engine/src/insight_forge/mod.rs`, add:
```rust
pub mod llm_decomposer;
```
And in the re-exports:
```rust
pub use decomposer::FallbackDecomposer;
pub use llm_decomposer::{DecomposerLlm, LlmDecomposer};
```

In `crates/context_engine/src/lib.rs`, add to the InsightForge re-exports:
```rust
pub use insight_forge::{
    CircuitBreaker, DecomposerLlm, DomainSearcher, FallbackDecomposer, HeuristicDecomposer,
    InsightForge, InsightForgeConfig, LlmDecomposer, QueryDecomposer,
};
```

- [ ] **Step 4: Wire FallbackDecomposer in builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, create a `DecomposerLlm` adapter from the existing cheap provider, then wire the `FallbackDecomposer`:

First, create the adapter in the builder file (above the `build` method):

```rust
struct DecomposerLlmAdapter {
    provider: providers::DynProvider,
}

#[async_trait::async_trait]
impl context_engine::DecomposerLlm for DecomposerLlmAdapter {
    async fn generate(&self, prompt: &str) -> Result<String, String> {
        let params = providers::ChatParams::new("default".to_string())
            .with_max_tokens(256);
        let messages = vec![providers::Message::User {
            content: providers::UserContent::Text(prompt.to_string()),
        }];
        let response = self.provider.chat(&messages, None, &params).await.map_err(|e| e.to_string())?;
        response.content.ok_or_else(|| "empty response".to_string())
    }
}
```

Then replace the `HeuristicDecomposer` in the InsightForge construction (~line 640-643). Use the existing `cognitive_provider` (already available in the builder context from `self.cognitive_provider`):

```rust
// Use FallbackDecomposer if a cognitive provider is available
let decomposer: Arc<dyn context_engine::QueryDecomposer> =
    if let Some(ref provider) = self.cognitive_provider {
        let llm_adapter = Arc::new(DecomposerLlmAdapter {
            provider: provider.clone(),
        });
        let llm_decomposer = Arc::new(context_engine::LlmDecomposer::new(llm_adapter));
        Arc::new(context_engine::FallbackDecomposer::new(llm_decomposer, 3))
    } else {
        Arc::new(context_engine::HeuristicDecomposer)
    };

let mut forge = context_engine::InsightForge::new(
    forge_config,
    decomposer,
    Arc::clone(&retriever),
);
```

**Note:** The HeuristicDecomposer currently produces ≥3 sub-queries for any message ≥20 chars (which is the InsightForge activation threshold). This means the LLM fallback will rarely fire with the current threshold of 3. This is by design — the heuristic handles the common case at zero cost, and the LLM path exists for future refinement (e.g., raising the heuristic quality threshold or adding semantic sub-query evaluation). If you want the LLM to fire more often during development/testing, temporarily set `min_heuristic_queries` to 6.

- [ ] **Step 5: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p context_engine -E 'test(decomposer)'`

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/insight_forge/ crates/agent/
git commit -m "feat(context-engine): add LLM-backed FallbackDecomposer for InsightForge"
```

---

## Chunk 5: Entity Backfill

### Task 5: Add entity backfill on startup

Pre-existing SPO facts from before EntityRepo was wired aren't reflected in the entities table. Add a one-time backfill that converts unique subjects from `semantic_facts` into entity entries.

**Files:**
- Modify: `crates/cognitive/src/repos/entity.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs` (or `crates/app-core/src/init/mod.rs`)

- [ ] **Step 1: Add backfill_from_facts to EntityRepo**

```rust
/// Backfill entities from existing SPO facts.
///
/// For each unique subject in `semantic_facts` that doesn't already have
/// an entity, creates a 'concept' entity. Runs once at startup; idempotent.
pub async fn backfill_from_facts(&self) -> Result<u32, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO entities (id, name, entity_type, description, source, first_seen_at, last_seen_at, mention_count, created_at, updated_at)
        SELECT
            lower(hex(randomblob(16))),
            TRIM(subject),
            CASE
                WHEN GROUP_CONCAT(predicate) LIKE '%works_on%' OR GROUP_CONCAT(predicate) LIKE '%project%' THEN 'project'
                WHEN GROUP_CONCAT(predicate) LIKE '%uses%' OR GROUP_CONCAT(predicate) LIKE '%tool%' THEN 'technology'
                WHEN GROUP_CONCAT(predicate) LIKE '%knows%' OR GROUP_CONCAT(predicate) LIKE '%person%' THEN 'person'
                ELSE 'concept'
            END,
            NULL,
            'backfill',
            MIN(created_at),
            MAX(created_at),
            COUNT(*),
            MIN(created_at),
            MIN(created_at)
        FROM semantic_facts
        WHERE subject != 'user'
          AND LOWER(TRIM(subject)) NOT IN (SELECT LOWER(name) FROM entities)
        GROUP BY LOWER(TRIM(subject))
        "#,
    )
    .execute(&*self.pool)
    .await?;

    Ok(result.rows_affected() as u32)
}
```

- [ ] **Step 2: Call during agent init**

In `crates/agent/src/agent_loop/builder.rs`, after the InsightForge block (after ~line 665), inside the existing `if let Some(pool) = self.pool.clone()` block where `storage_pool` is bound, add:

```rust
// One-time entity backfill from pre-existing SPO facts
let backfill_repo = cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
match backfill_repo.backfill_from_facts().await {
    Ok(0) => {} // Nothing to backfill
    Ok(n) => tracing::info!("Backfilled {n} entities from SPO facts"),
    Err(e) => tracing::debug!("Entity backfill error (non-fatal): {e}"),
}
```

- [ ] **Step 3: Build + test**

Run: `cargo build --workspace`

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/entity.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(cognitive): add entity backfill from pre-existing SPO facts"
```

---

## Chunk 6: Verification

### Task 6: Full verification

- [ ] **Step 1: Backend tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Frontend build (ensure no breakage)**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 5: Manual verification**

Start: `cargo tauri dev`

1. Open a chat conversation
2. Send a complex message like "Help me plan the API migration considering all the related tasks and budget"
3. Check backend logs for `InsightForge` debug output — verify decomposer activates and multiple sources are searched
4. Verify finance-related queries include transaction context
5. Verify entity graph neighbors appear in context for known entities
