# Gap Fill (Phases 0-3) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete all outstanding Phase 0-3 items: DomainSearcher implementations (NoteSearcher, TaskSearcher, GraphSearcher), InsightForge config schema, entity extraction in the consolidation pipeline, and a post-RRF budget allocator — making the InsightForge context retrieval system fully production-ready.

**Architecture:** DomainSearcher implementations live in the `agent` crate (L5) following the dependency inversion pattern — the trait is in context_engine (L3), implementations in agent (L5) which can access feature crates (L4). Searchers are registered into InsightForge during agent builder construction. InsightForge config adds a nested `insightForge` section to `CognitiveConfig`. Entity extraction adds a post-consolidation hook that upserts entities from newly learned facts.

**Tech Stack:** Rust (SQLite, tokio, async_trait)

**Spec:** `docs/superpowers/specs/2026-03-16-mirofish-integration-architecture.md` (§2: InsightForge, §1: Knowledge Graph, §4: Temporal)

**Note:** Entity backfill (converting existing SPO facts into graph entries) is explicitly deferred per CLAUDE.md: "Pre-release — no user data to migrate." The entities table populates going forward via the entity extraction hook.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/agent/src/domain_searchers/mod.rs` | Module registration |
| `crates/agent/src/domain_searchers/note_searcher.rs` | NoteSearcher: FTS search over notes |
| `crates/agent/src/domain_searchers/task_searcher.rs` | TaskSearcher: keyword search over tasks |
| `crates/agent/src/domain_searchers/graph_searcher.rs` | GraphSearcher: entity name + relationship search |

### Modified files

| File | Change |
|------|--------|
| `crates/agent/src/lib.rs` | Register domain_searchers module |
| `crates/agent/src/agent_loop/builder.rs` | Register searchers into InsightForge, use config |
| `crates/config/src/schema/cognitive.rs` | Add InsightForge config fields |
| `crates/cognitive/src/services/background.rs` | Add entity extraction after consolidation |
| `crates/context_engine/src/insight_forge/mod.rs` | Add budget_allocator pass after RRF merge |

---

## Chunk 1: DomainSearchers + Config

### Task 1: InsightForge Config Fields

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Add InsightForge config fields to CognitiveConfig**

In `crates/config/src/schema/cognitive.rs`, add after the `relevance_weight_temporal` field:

```rust
    /// Whether InsightForge multi-dimensional retrieval is enabled (default: true).
    #[serde(default = "default_insight_forge_enabled")]
    pub insight_forge_enabled: bool,

    /// Max sub-queries for InsightForge decomposer (default: 5).
    #[serde(default = "default_insight_forge_max_sub_queries")]
    pub insight_forge_max_sub_queries: usize,

    /// Max results per source per sub-query (default: 5).
    #[serde(default = "default_insight_forge_per_source_limit")]
    pub insight_forge_per_source_limit: usize,

    /// Hard cap on total InsightForge results (default: 15).
    #[serde(default = "default_insight_forge_total_limit")]
    pub insight_forge_total_limit: usize,

    /// Timeout ms for each domain searcher (default: 800).
    #[serde(default = "default_insight_forge_per_source_timeout_ms")]
    pub insight_forge_per_source_timeout_ms: u64,
```

Add to `Default` impl and add default functions:

```rust
fn default_insight_forge_enabled() -> bool { true }
fn default_insight_forge_max_sub_queries() -> usize { 5 }
fn default_insight_forge_per_source_limit() -> usize { 5 }
fn default_insight_forge_total_limit() -> usize { 15 }
fn default_insight_forge_per_source_timeout_ms() -> u64 { 800 }
```

- [ ] **Step 2: Build**

Run: `cargo build -p config`

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/schema/cognitive.rs
git commit -m "feat(config): add InsightForge configuration fields"
```

---

### Task 2: NoteSearcher

**Files:**
- Create: `crates/agent/src/domain_searchers/mod.rs`
- Create: `crates/agent/src/domain_searchers/note_searcher.rs`
- Modify: `crates/agent/src/lib.rs`

NoteSearcher wraps `NoteRepo::search_fts()` (which returns `Vec<NoteSearchResult>` with id, title, body, rank) and converts results into `MemoryEntry` values.

- [ ] **Step 1: Create module registration**

Create `crates/agent/src/domain_searchers/mod.rs`:

```rust
pub mod note_searcher;
pub mod task_searcher;
pub mod graph_searcher;

pub use note_searcher::NoteSearcher;
pub use task_searcher::TaskSearcher;
pub use graph_searcher::GraphSearcher;
```

- [ ] **Step 2: Create NoteSearcher**

Create `crates/agent/src/domain_searchers/note_searcher.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::MemoryEntry;
use context_engine::MemorySource;
use feature_notes::repo::NoteRepo;

/// Searches notes via FTS5 full-text search.
pub struct NoteSearcher {
    repo: NoteRepo,
}

impl NoteSearcher {
    pub fn new(repo: NoteRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl DomainSearcher for NoteSearcher {
    fn domain_name(&self) -> &str {
        "notes"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let results = match self.repo.search_fts(query).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        results
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, note)| {
                let body_preview = if note.body.len() > 500 {
                    format!("{}...", &note.body[..500])
                } else {
                    note.body.clone()
                };
                MemoryEntry {
                    id: note.id.clone(),
                    content: format!("[Note: {}] {}", note.title, body_preview),
                    score: 1.0 / (1.0 + i as f64), // decay by rank
                    source: MemorySource::Domain { name: "notes".into() },
                    raw_score: note.rank.unwrap_or(0.0),
                }
            })
            .collect()
    }
}
```

**Note:** Check the actual `NoteSearchResult` fields by reading `crates/feature-notes/src/models.rs`. It may have a `rank` field (from FTS5 BM25). Adapt the code accordingly.

- [ ] **Step 3: Register module in lib.rs**

In `crates/agent/src/lib.rs`, add:
```rust
pub mod domain_searchers;
```

- [ ] **Step 4: Build**

Run: `cargo build -p agent`

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/domain_searchers/ crates/agent/src/lib.rs
git commit -m "feat(agent): add NoteSearcher domain searcher for InsightForge"
```

---

### Task 3: TaskSearcher

**Files:**
- Create: `crates/agent/src/domain_searchers/task_searcher.rs`

TaskSearcher wraps the task repo's `search_by_keyword()` method.

- [ ] **Step 1: Create TaskSearcher**

Create `crates/agent/src/domain_searchers/task_searcher.rs`:

```rust
use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::MemoryEntry;
use context_engine::MemorySource;
use storage::Repos;

/// Searches tasks via keyword matching.
pub struct TaskSearcher {
    repos: Repos,
}

impl TaskSearcher {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl DomainSearcher for TaskSearcher {
    fn domain_name(&self) -> &str {
        "tasks"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let rows = match self.repos.task_repo().search_by_keyword(query, Some(limit as i64)).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        rows.into_iter()
            .enumerate()
            .map(|(i, task)| {
                let status = &task.status;
                let title = &task.title;
                MemoryEntry {
                    id: task.id.clone(),
                    content: format!("[Task: {} ({})] {}", title, status, task.description.as_deref().unwrap_or("")),
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain { name: "tasks".into() },
                    raw_score: 0.0,
                }
            })
            .collect()
    }
}
```

**Note:** Check the actual `TaskRow` fields and `Repos::task_repo()` method. The task repo is accessed via `Repos` (L2 storage crate). Adapt field names to match the actual struct.

- [ ] **Step 2: Build**

Run: `cargo build -p agent`

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/domain_searchers/task_searcher.rs
git commit -m "feat(agent): add TaskSearcher domain searcher for InsightForge"
```

---

### Task 4: GraphSearcher

**Files:**
- Create: `crates/agent/src/domain_searchers/graph_searcher.rs`

GraphSearcher wraps `EntityRepo::find_by_name()` to search the knowledge graph by entity name, returning entity descriptions + relationship context.

- [ ] **Step 1: Create GraphSearcher**

Create `crates/agent/src/domain_searchers/graph_searcher.rs`:

```rust
use async_trait::async_trait;
use cognitive::repos::EntityRepo;
use context_engine::insight_forge::DomainSearcher;
use context_engine::MemoryEntry;
use context_engine::MemorySource;

/// Searches the unified knowledge graph by entity name.
pub struct GraphSearcher {
    entity_repo: EntityRepo,
}

impl GraphSearcher {
    pub fn new(entity_repo: EntityRepo) -> Self {
        Self { entity_repo }
    }
}

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

        entities
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, entity)| {
                let desc = entity.description.as_deref().unwrap_or("no description");
                MemoryEntry {
                    id: entity.id.clone(),
                    content: format!(
                        "[Entity: {} ({})] {} (seen {} times)",
                        entity.name, entity.entity_type, desc, entity.mention_count
                    ),
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain { name: "graph".into() },
                    raw_score: entity.mention_count as f64,
                }
            })
            .collect()
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p agent`

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/domain_searchers/graph_searcher.rs
git commit -m "feat(agent): add GraphSearcher domain searcher for InsightForge"
```

---

### Task 5: Register Searchers in Agent Builder + Wire Config

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

This is the critical wiring task. The builder currently creates `InsightForge` with `InsightForgeConfig::default()` (line 623-627) but never registers searchers. We need to:
1. Build `InsightForgeConfig` from the config schema
2. Create and register all three searchers

- [ ] **Step 1: Build InsightForgeConfig from config**

In `crates/agent/src/agent_loop/builder.rs`, replace the InsightForge creation block (around lines 622-631):

```rust
// OLD:
let forge = context_engine::InsightForge::new(
    context_engine::InsightForgeConfig::default(),
    Arc::new(context_engine::HeuristicDecomposer),
    Arc::clone(&retriever),
);

// NEW:
let forge_config = context_engine::InsightForgeConfig {
    enabled: config.cognitive.insight_forge_enabled,
    max_sub_queries: config.cognitive.insight_forge_max_sub_queries,
    per_source_limit: config.cognitive.insight_forge_per_source_limit,
    total_limit: config.cognitive.insight_forge_total_limit,
    per_source_timeout_ms: config.cognitive.insight_forge_per_source_timeout_ms,
    ..context_engine::InsightForgeConfig::default()
};
let mut forge = context_engine::InsightForge::new(
    forge_config,
    Arc::new(context_engine::HeuristicDecomposer),
    Arc::clone(&retriever),
);

// Register domain searchers
forge.add_searcher(Arc::new(
    crate::domain_searchers::NoteSearcher::new(self.note_repo.clone()),
));
forge.add_searcher(Arc::new(
    crate::domain_searchers::TaskSearcher::new(self.repos.clone()),
));
if let Some(ref pool) = cognitive_pool {
    forge.add_searcher(Arc::new(
        crate::domain_searchers::GraphSearcher::new(
            cognitive::repos::EntityRepo::new(pool.clone()),
        ),
    ));
}
```

**Note:** `self.note_repo` and `self.repos` should be accessible in this scope — check the builder's fields. `cognitive_pool` is the SQLite pool used for cognitive repos. Adapt variable names to match what's actually in scope.

- [ ] **Step 2: Build**

Run: `cargo build --workspace`

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): register DomainSearchers into InsightForge + wire config"
```

---

## Chunk 2: Entity Extraction + Budget Allocator

### Task 6: Entity Extraction in Consolidation Pipeline

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

After `execute_memory_ops()` completes (and after contradiction detection), extract entities from newly added/updated facts and upsert them into the entities table. This keeps the knowledge graph growing automatically without a separate LLM call — we just parse the subject and object of each new fact as entities.

- [ ] **Step 1: Add entity extraction after consolidation**

In `crates/cognitive/src/services/background.rs`, after the contradiction detection block (which is after `execute_memory_ops().await`), add:

```rust
                        // ── Entity extraction from new facts ──────────────────
                        // For each Add or Update op, upsert the subject and object
                        // as entities in the knowledge graph. No LLM needed — the
                        // SPO triple structure gives us entities directly.
                        let entity_repo = crate::repos::EntityRepo::new(repo.pool().clone());
                        for (candidate, op) in candidates.iter().zip(ops.iter()) {
                            match op {
                                crate::types::MemoryOp::Add { .. }
                                | crate::types::MemoryOp::Update { .. } => {
                                    let fact = &candidate.candidate;
                                    // Upsert subject as entity
                                    let _ = entity_repo
                                        .upsert_entity(crate::repos::NewEntity {
                                            name: fact.subject.clone(),
                                            entity_type: "concept".to_string(),
                                            description: None,
                                            source: "extracted".to_string(),
                                            source_id: Some(fact.id.clone()),
                                            metadata: None,
                                        })
                                        .await;
                                    // Upsert object as entity (skip if it looks like a value, not a name)
                                    if fact.object.len() > 2
                                        && fact.object.len() < 100
                                        && !fact.object.chars().all(|c| c.is_ascii_digit() || c == '.')
                                    {
                                        let _ = entity_repo
                                            .upsert_entity(crate::repos::NewEntity {
                                                name: fact.object.clone(),
                                                entity_type: "concept".to_string(),
                                                description: None,
                                                source: "extracted".to_string(),
                                                source_id: Some(fact.id.clone()),
                                                metadata: None,
                                            })
                                            .await;
                                    }
                                }
                                _ => {}
                            }
                        }
```

**Note:** `repo.pool()` should give access to the underlying `SqlitePool`. Check if `SemanticFactRepo` exposes a `pool()` method. If not, the pool is available from the config's `repo` field. `EntityRepo::new()` takes a `SqlitePool`. `NewEntity` is from `crate::repos`.

- [ ] **Step 2: Build**

Run: `cargo build -p cognitive`

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): auto-extract entities from consolidated facts"
```

---

### Task 7: Post-RRF Budget Allocator

**Files:**
- Modify: `crates/context_engine/src/insight_forge/mod.rs`

After the RRF merge and deduplication in the `retrieve` method, add a budget allocation pass that ensures results don't exceed a configurable total limit and that no single source dominates.

- [ ] **Step 1: Add budget allocation after RRF merge**

In `crates/context_engine/src/insight_forge/mod.rs`, find the `retrieve` method. After the deduplication and score normalization step (where results are sorted and truncated to `limit`), add a diversity pass:

```rust
        // Budget allocation: ensure no single source provides more than 60% of results.
        // This prevents a single high-scoring domain from drowning out others.
        let max_per_source = (limit as f64 * 0.6).ceil() as usize;
        let mut source_counts: HashMap<String, usize> = HashMap::new();
        let mut budgeted = Vec::new();
        let mut overflow = Vec::new();

        for entry in merged {
            let source_key = match &entry.source {
                crate::MemorySource::CognitiveFact => "cognitive".to_string(),
                crate::MemorySource::ConversationRecall => "recall".to_string(),
                crate::MemorySource::Domain { name } => name.clone(),
            };
            let count = source_counts.entry(source_key).or_insert(0);
            if *count < max_per_source {
                *count += 1;
                budgeted.push(entry);
            } else {
                overflow.push(entry);
            }
        }

        // Fill remaining slots from overflow (highest score first, already sorted)
        let remaining = limit.saturating_sub(budgeted.len());
        budgeted.extend(overflow.into_iter().take(remaining));
        budgeted.truncate(limit);

        budgeted
```

Replace the existing final truncation with this budget-aware version.

- [ ] **Step 2: Build**

Run: `cargo build -p context_engine`

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine`

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/insight_forge/mod.rs
git commit -m "feat(context_engine): add post-RRF budget allocator for source diversity"
```

---

### Task 8: Final Verification

- [ ] **Step 1: Full workspace build + tests**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Commit if needed**

```bash
cargo fmt --all
git add -A && git commit -m "style: format gap-fill implementation"
```
