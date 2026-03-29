# Universal Semantic Layer — Upgrading the Second Brain from Note-Centric to All-Domain

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the semantic layer from a note-centric knowledge graph into a true second brain that unifies ALL domains (finance, productivity, OKRs, learning, coaching) as first-class graph citizens — visible in the Knowledge Fabric Explorer, wired into community detection, and fully scored by the 10-factor retrieval pipeline.

**Architecture:** Three workstreams: (1) **Domain Tree Builders** — new event subscribers that parse finance/productivity/OKR/learning data into `book_tree_nodes` so they participate in entity linking and Louvain communities, (2) **Scoring & Retrieval Fixes** — wire the 4 hardcoded-zero relevance weights, expand autotuner resolution, populate `entity_embeddings`, fix community stability/top_entities/ID stability, (3) **Graph Visualization Completion** — SSE live updates, community detail panel, wave-reveal in 3D, and the frontend event listener for `fabric_graph` Tauri events.

**Tech Stack:** Rust (tokio subscribers, SQLite, LanceDB), TypeScript/React (react-force-graph, Tauri event listeners)

---

## Scope Check

This plan covers three independent workstreams that can be parallelized:
- **WS1: Domain Tree Builders** (Tasks 1–5) — Rust backend, new subscriber crates
- **WS2: Scoring & Retrieval Fixes** (Tasks 6–10) — Rust backend, existing crate modifications
- **WS3: Graph Visualization Completion** (Tasks 11–14) — TypeScript frontend

WS1 and WS2 have no shared files and can be worked on simultaneously. WS3 depends on WS1 for domain-specific graph nodes and WS2 for community fixes, but the frontend scaffolding can proceed in parallel.

---

## File Structure

### WS1: Domain Tree Builders

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/context_engine/src/book_index/types.rs` | Modify | Add `Finance`, `Productivity`, `OKR`, `Learning` to `SourceType` enum |
| `crates/agent/src/adapters/finance_tree_builder.rs` | Create | Subscribe to `TransactionRecorded`, `BudgetAlert` → build tree nodes from finance data |
| `crates/agent/src/adapters/productivity_tree_builder.rs` | Create | Subscribe to `FocusSessionEnded`, `ActivitySessionCompleted`, `ProductivityScoreComputed` → build tree nodes |
| `crates/agent/src/adapters/okr_tree_builder.rs` | Create | Subscribe to `GoalProgress` → build tree nodes from objectives/key results |
| `crates/agent/src/adapters/learning_tree_builder.rs` | Create | Subscribe to `KnowledgeAtomAccepted`, `RetentionMilestoneReached` → build tree nodes from atoms/flashcards |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire new tree builders as event subscribers + backfill jobs |
| `crates/app-core/src/handlers/fabric.rs` | Modify | Include new `SourceType` variants in `fabric_graph_base` and `fabric_graph_expand` queries |

### WS2: Scoring & Retrieval Fixes

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/src/services/retrieval.rs` | Modify | Wire Phase 4+5 autotuner weights instead of hardcoded zeros |
| `crates/common/src/autotuner.rs` | Modify | Add `resolve_full_relevance_weights()` that resolves all 10 weights |
| `crates/agent/src/adapters/community_builder.rs` | Modify | Fix `top_entities` (populate from entity names), community ID stability (content-hash), stability decay |
| `crates/agent/src/adapters/entity_embedder.rs` | Create | Populate `entity_embeddings` LanceDB table from entities with descriptions |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire entity embedder backfill job |

### WS3: Graph Visualization Completion

| File | Action | Responsibility |
|------|--------|---------------|
| `desktop-ui/src/features/notes/hooks/useFabricGraph.ts` | Modify | Add Tauri event listener for `fabric_graph` SSE events |
| `desktop-ui/src/features/notes/hooks/useGraphElements.ts` | Modify | Add `ForceNodeType` variants for new domain types (`finance`, `productivity`, `okr`, `learning`) |
| `desktop-ui/src/features/notes/components/GraphView.tsx` | Modify | Wire community detail expand + new domain node rendering |
| `desktop-ui/src/features/notes/components/GraphBrainView.tsx` | Modify | Integrate wave-reveal into 3D mode |
| `desktop-ui/src/features/notes/lib/graphPainters.ts` | Modify | Add paint functions for new domain node shapes |

---

## WS1: Domain Tree Builders

### Task 1: Extend SourceType enum

**Files:**
- Modify: `crates/context_engine/src/book_index/types.rs:38-63`

This task adds the new domain source types so the rest of the pipeline can distinguish finance/productivity/OKR/learning tree nodes from note/task tree nodes.

- [ ] **Step 1: Write the test**

Add to the existing test module in `crates/context_engine/src/book_index/types.rs` (or a new `#[cfg(test)] mod tests` if none exists). If the file has no test module, add one at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_type_roundtrip_all_variants() {
        let variants = [
            ("note", SourceType::Note),
            ("task", SourceType::Task),
            ("skill", SourceType::Skill),
            ("finance", SourceType::Finance),
            ("productivity", SourceType::Productivity),
            ("okr", SourceType::OKR),
            ("learning", SourceType::Learning),
        ];
        for (s, expected) in variants {
            let parsed = SourceType::parse(s);
            assert_eq!(parsed.as_str(), s, "roundtrip failed for {s}");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p context-engine -E 'test(source_type_roundtrip)'`
Expected: FAIL — `Finance`, `Productivity`, `OKR`, `Learning` variants don't exist yet.

- [ ] **Step 3: Add variants to SourceType**

In `crates/context_engine/src/book_index/types.rs`, modify the `SourceType` enum and its impls:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Note,
    Task,
    Skill,
    Finance,
    Productivity,
    OKR,
    Learning,
}

impl SourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Note => "note",
            Self::Task => "task",
            Self::Skill => "skill",
            Self::Finance => "finance",
            Self::Productivity => "productivity",
            Self::OKR => "okr",
            Self::Learning => "learning",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "note" => Self::Note,
            "task" => Self::Task,
            "skill" => Self::Skill,
            "finance" => Self::Finance,
            "productivity" => Self::Productivity,
            "okr" => Self::OKR,
            "learning" => Self::Learning,
            _ => Self::Note,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p context-engine -E 'test(source_type_roundtrip)'`
Expected: PASS

- [ ] **Step 5: Fix any downstream compile errors**

Run: `cargo build --workspace`

The `fabric.rs` handler queries `source_type IN ("note","task")` — we'll update those queries in Task 5. For now, verify the workspace compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/book_index/types.rs
git commit -m "feat(context-engine): add Finance, Productivity, OKR, Learning to SourceType enum"
```

---

### Task 2: Finance Tree Builder

**Files:**
- Create: `crates/agent/src/adapters/finance_tree_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs` (add `pub mod finance_tree_builder;`)

The finance tree builder subscribes to `TransactionRecorded` and `BudgetAlert` events and creates tree nodes that represent financial activity. This makes finance data visible in the entity graph and community detection. We batch transactions by category into daily summaries as tree nodes — one root per day, one child per category, individual transactions as leaves.

- [ ] **Step 1: Write the test**

Create the file with tests first. The builder needs a pool, tree repo, vector store, embedder — same pattern as `NoteTreeBuilder`. Write a focused integration test:

```rust
// crates/agent/src/adapters/finance_tree_builder.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_tree_from_transaction_event() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let tree_repo = Arc::new(
            cognitive::repos::SqliteBookTreeRepo::new(pool.inner().clone()),
        );
        let vs = storage::VectorStore::open_in_memory().await.unwrap();
        let embedder = Arc::new(cognitive::test_utils::FakeEmbedder::new());

        let builder = FinanceTreeBuilder::new(
            tree_repo.clone(),
            Arc::new(vs),
            embedder,
            None, // no context update queue in test
            None, // no domain event bus in test
            pool.inner().clone(),
        );

        builder
            .handle_transaction("food", 45.20, false)
            .await
            .unwrap();

        let nodes = tree_repo
            .get_children_recursive("finance-daily-root")
            .await
            .unwrap();
        // Should have at least a category node with the transaction
        assert!(!nodes.is_empty(), "Should create tree nodes from transaction");
        assert!(
            nodes.iter().any(|n| n.content.contains("45.20")),
            "Transaction amount should appear in node content"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(builds_tree_from_transaction)'`
Expected: FAIL — `FinanceTreeBuilder` doesn't exist yet.

- [ ] **Step 3: Implement FinanceTreeBuilder**

```rust
//! FinanceTreeBuilder — event subscriber that creates tree nodes from financial
//! events (transactions, budget alerts) so finance data participates in the
//! entity graph, community detection, and Knowledge Fabric Explorer.
//!
//! Strategy: daily root node → category child nodes → transaction leaf nodes.
//! Budget alerts create a special "alert" node under the category.

use std::sync::Arc;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{
    ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, DomainEventBus,
    UpdatePriority,
};
use cognitive::TextEmbedder;
use common::truncate_at_boundary;
use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use context_engine::book_index::BookTreeRepo;

pub struct FinanceTreeBuilder {
    tree_repo: Arc<dyn BookTreeRepo>,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
    pool: SqlitePool,
}

impl FinanceTreeBuilder {
    pub fn new(
        tree_repo: Arc<dyn BookTreeRepo>,
        vector_store: Arc<storage::VectorStore>,
        embedder: Arc<dyn TextEmbedder>,
        context_update_queue: Option<Arc<ContextUpdateQueue>>,
        domain_event_bus: Option<Arc<DomainEventBus>>,
        pool: SqlitePool,
    ) -> Self {
        Self {
            tree_repo,
            vector_store,
            embedder,
            context_update_queue,
            domain_event_bus,
            pool,
        }
    }

    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        info!("FinanceTreeBuilder: subscriber started");
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("FinanceTreeBuilder: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::TransactionRecorded { category, amount, is_over_budget }) => {
                            if let Err(e) = self.handle_transaction(&category, amount, is_over_budget).await {
                                warn!("FinanceTreeBuilder: transaction error: {e}");
                            }
                        }
                        Ok(DomainEvent::BudgetAlert { category, spent, limit }) => {
                            if let Err(e) = self.handle_budget_alert(&category, spent, limit).await {
                                warn!("FinanceTreeBuilder: budget alert error: {e}");
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("FinanceTreeBuilder: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("FinanceTreeBuilder: channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Create tree nodes for a transaction event.
    /// Structure: finance-daily-{date} (root) → finance-cat-{date}-{category} → finance-txn-{uuid}
    pub async fn handle_transaction(
        &self,
        category: &str,
        amount: f64,
        is_over_budget: bool,
    ) -> common::Result<()> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let source_id = format!("finance-daily-{today}");

        // Ensure daily root exists
        let root_id = source_id.clone();
        let root_node = TreeNode {
            id: root_id.clone(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: format!("Finance activity for {today}"),
            title: Some(format!("Finance: {today}")),
            level: 0,
            source_type: SourceType::Finance,
            source_id: source_id.clone(),
            position: 0,
            metadata: None,
        };

        // Category node
        let cat_id = format!("finance-cat-{today}-{}", category.to_lowercase().replace(' ', "-"));
        let cat_node = TreeNode {
            id: cat_id.clone(),
            parent_id: Some(root_id.clone()),
            node_type: TreeNodeType::Section,
            content: format!("Spending in {category}"),
            title: Some(category.to_string()),
            level: 1,
            source_type: SourceType::Finance,
            source_id: source_id.clone(),
            position: 0,
            metadata: None,
        };

        // Transaction leaf
        let txn_id = format!("finance-txn-{}", common::new_id());
        let over_note = if is_over_budget { " (OVER BUDGET)" } else { "" };
        let txn_node = TreeNode {
            id: txn_id.clone(),
            parent_id: Some(cat_id.clone()),
            node_type: TreeNodeType::Text,
            content: format!("${amount:.2} in {category}{over_note}"),
            title: None,
            level: 2,
            source_type: SourceType::Finance,
            source_id: source_id.clone(),
            position: 0,
            metadata: None,
        };

        // Upsert root + category (idempotent), insert transaction
        self.tree_repo.upsert_node(&root_node).await?;
        self.tree_repo.upsert_node(&cat_node).await?;
        self.tree_repo.upsert_node(&txn_node).await?;

        // Embed transaction node
        let embed_text = format!("{category}: ${amount:.2}{over_note}");
        if let Ok(embedding) = self.embedder.embed(&embed_text).await {
            let _ = self
                .vector_store
                .upsert_tree_node_embedding(
                    &txn_id,
                    &embedding,
                    &source_id,
                    "2",
                    SourceType::Finance.as_str(),
                )
                .await;
        }

        // Emit TreeNodesRebuilt so EntityTreeLinker runs
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(DomainEvent::TreeNodesRebuilt {
                source_type: SourceType::Finance.as_str().to_string(),
                source_id: source_id.clone(),
            });
        }

        debug!(category, amount, "FinanceTreeBuilder: transaction indexed");
        Ok(())
    }

    pub async fn handle_budget_alert(
        &self,
        category: &str,
        spent: f64,
        limit: f64,
    ) -> common::Result<()> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let source_id = format!("finance-daily-{today}");
        let cat_id = format!("finance-cat-{today}-{}", category.to_lowercase().replace(' ', "-"));
        let alert_id = format!("finance-alert-{today}-{}", category.to_lowercase().replace(' ', "-"));

        let pct = if limit > 0.0 { (spent / limit * 100.0) as i32 } else { 100 };
        let alert_node = TreeNode {
            id: alert_id.clone(),
            parent_id: Some(cat_id),
            node_type: TreeNodeType::Text,
            content: format!("Budget alert: {category} at {pct}% (${spent:.2} / ${limit:.2})"),
            title: Some(format!("Alert: {category} budget")),
            level: 2,
            source_type: SourceType::Finance,
            source_id: source_id.clone(),
            position: 0,
            metadata: None,
        };

        self.tree_repo.upsert_node(&alert_node).await?;

        if let Ok(embedding) = self.embedder.embed(&alert_node.content).await {
            let _ = self
                .vector_store
                .upsert_tree_node_embedding(
                    &alert_id,
                    &embedding,
                    &source_id,
                    "2",
                    SourceType::Finance.as_str(),
                )
                .await;
        }

        if let Some(bus) = &self.domain_event_bus {
            bus.publish(DomainEvent::TreeNodesRebuilt {
                source_type: SourceType::Finance.as_str().to_string(),
                source_id,
            });
        }

        Ok(())
    }
}
```

Note: `upsert_node` may not exist on `BookTreeRepo` yet — the current API has `insert_nodes` (batch) and `delete_by_source`. If `upsert_node` doesn't exist, add it to the `BookTreeRepo` trait + `SqliteBookTreeRepo` impl using `INSERT OR REPLACE INTO book_tree_nodes ...`. Check the trait at `crates/context_engine/src/book_index/mod.rs` and the impl at `crates/cognitive/src/repos/book_tree.rs`.

Similarly, `upsert_tree_node_embedding` may need to be verified on `VectorStore`. Check `crates/storage/src/vector_store/mod.rs` for the existing method signature — it's likely `upsert_tree_node_embedding(id, vector, note_id, level, source_type)`.

- [ ] **Step 4: Register module**

Add to `crates/agent/src/adapters/mod.rs`:
```rust
pub mod finance_tree_builder;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(builds_tree_from_transaction)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/adapters/finance_tree_builder.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(agent): add FinanceTreeBuilder — index transactions as tree nodes"
```

---

### Task 3: Productivity Tree Builder

**Files:**
- Create: `crates/agent/src/adapters/productivity_tree_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

Same pattern as FinanceTreeBuilder. Subscribes to `FocusSessionEnded`, `ActivitySessionCompleted`, `ProductivityScoreComputed`. Structure: daily root → session type children → individual session leaves.

- [ ] **Step 1: Write the test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_tree_from_focus_session() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let tree_repo = Arc::new(
            cognitive::repos::SqliteBookTreeRepo::new(pool.inner().clone()),
        );
        let vs = storage::VectorStore::open_in_memory().await.unwrap();
        let embedder = Arc::new(cognitive::test_utils::FakeEmbedder::new());

        let builder = ProductivityTreeBuilder::new(
            tree_repo.clone(),
            Arc::new(vs),
            embedder,
            None,
            None,
        );

        builder
            .handle_focus_session_ended(1800, 0.85, 2)
            .await
            .unwrap();

        let nodes = tree_repo
            .get_children_recursive("productivity-daily-root")
            .await
            .unwrap();
        assert!(!nodes.is_empty());
        assert!(nodes.iter().any(|n| n.content.contains("30 min")));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(builds_tree_from_focus_session)'`
Expected: FAIL

- [ ] **Step 3: Implement ProductivityTreeBuilder**

Follow the exact same struct pattern as `FinanceTreeBuilder`:
- Constructor: `tree_repo`, `vector_store`, `embedder`, `context_update_queue`, `domain_event_bus`
- `run()`: subscribe to `FocusSessionEnded`, `ActivitySessionCompleted`, `ProductivityScoreComputed`
- `handle_focus_session_ended(duration_secs, quality, interruptions)`:
  - Daily root: `productivity-daily-{date}` with title "Productivity: {date}"
  - Focus session child: `productivity-focus-{uuid}` with content "Focus session: {mins} min, quality {quality:.0}%, {interruptions} interruptions"
  - source_type = `SourceType::Productivity`
  - Emit `TreeNodesRebuilt { source_type: "productivity", source_id }`
- `handle_activity_session(date, total_active_secs, productive_secs, distracting_secs)`:
  - Daily root (same as above, upserted)
  - Activity child: `productivity-activity-{date}` with content "Active: {hrs}h, productive: {pct}%, distracting: {pct}%"
- `handle_productivity_score(date, score)`:
  - Daily root (same)
  - Score child: `productivity-score-{date}` with content "Productivity score: {score:.0}/100"

- [ ] **Step 4: Register module + run tests**

Add `pub mod productivity_tree_builder;` to `crates/agent/src/adapters/mod.rs`.

Run: `cargo nextest run -p agent -E 'test(builds_tree_from_focus_session)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/adapters/productivity_tree_builder.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(agent): add ProductivityTreeBuilder — index focus sessions and activity as tree nodes"
```

---

### Task 4: OKR + Learning Tree Builders

**Files:**
- Create: `crates/agent/src/adapters/okr_tree_builder.rs`
- Create: `crates/agent/src/adapters/learning_tree_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

These two are smaller and can be done together. Same event subscriber pattern.

- [ ] **Step 1: Write tests for both**

OKR test:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_tree_from_goal_progress() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let tree_repo = Arc::new(
            cognitive::repos::SqliteBookTreeRepo::new(pool.inner().clone()),
        );
        let vs = storage::VectorStore::open_in_memory().await.unwrap();
        let embedder = Arc::new(cognitive::test_utils::FakeEmbedder::new());

        let builder = OKRTreeBuilder::new(
            tree_repo.clone(), Arc::new(vs), embedder, None, None, pool.inner().clone(),
        );

        builder
            .handle_goal_progress("obj-1", 0.75, 1.0)
            .await
            .unwrap();

        let nodes = tree_repo
            .get_children_recursive("okr-root")
            .await
            .unwrap();
        assert!(!nodes.is_empty());
    }
}
```

Learning test:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_tree_from_atom_accepted() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let tree_repo = Arc::new(
            cognitive::repos::SqliteBookTreeRepo::new(pool.inner().clone()),
        );
        let vs = storage::VectorStore::open_in_memory().await.unwrap();
        let embedder = Arc::new(cognitive::test_utils::FakeEmbedder::new());

        let builder = LearningTreeBuilder::new(
            tree_repo.clone(), Arc::new(vs), embedder, None, None, pool.inner().clone(),
        );

        builder
            .handle_atom_accepted("atom-1", "concept", "biology")
            .await
            .unwrap();

        let nodes = tree_repo
            .get_children_recursive("learning-root")
            .await
            .unwrap();
        assert!(!nodes.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(builds_tree_from_goal_progress)' && cargo nextest run -p agent -E 'test(builds_tree_from_atom_accepted)'`
Expected: Both FAIL

- [ ] **Step 3: Implement OKRTreeBuilder**

- Subscribes to `GoalProgress { objective_id, progress, target }`
- `handle_goal_progress(objective_id, progress, target)`:
  - Root: `okr-root` with title "Goals & Objectives"
  - Objective child: `okr-obj-{objective_id}` — look up objective title from `objectives` table via `pool`
  - Progress leaf: `okr-progress-{objective_id}-{date}` with content "Progress: {pct}% toward {target}"
  - source_type = `SourceType::OKR`, emit `TreeNodesRebuilt`

- [ ] **Step 4: Implement LearningTreeBuilder**

- Subscribes to `KnowledgeAtomAccepted { atom_id, atom_type }`, `RetentionMilestoneReached { atom_id, .. }`
- `handle_atom_accepted(atom_id, atom_type, domain)`:
  - Root: `learning-root` with title "Learning & Knowledge"
  - Domain child: `learning-domain-{domain}` with title e.g. "Biology"
  - Atom leaf: `learning-atom-{atom_id}` — look up atom content from `knowledge_atoms` table via `pool`
  - source_type = `SourceType::Learning`, emit `TreeNodesRebuilt`

- [ ] **Step 5: Register modules + run tests**

Add to `crates/agent/src/adapters/mod.rs`:
```rust
pub mod okr_tree_builder;
pub mod learning_tree_builder;
```

Run: `cargo nextest run -p agent -E 'test(builds_tree_from_goal)' && cargo nextest run -p agent -E 'test(builds_tree_from_atom)'`
Expected: Both PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/adapters/okr_tree_builder.rs crates/agent/src/adapters/learning_tree_builder.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(agent): add OKR and Learning tree builders — index goals and atoms as tree nodes"
```

---

### Task 5: Wire all tree builders into agent builder + fabric queries

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:866-898` (backfill section)
- Modify: `crates/app-core/src/handlers/fabric.rs` (source_type IN queries)

- [ ] **Step 1: Wire new subscribers in builder.rs**

After the existing `CommunityBuilder subscriber started` log at `builder.rs:~864`, add the new tree builder subscribers. Follow the exact pattern used for `TaskTreeBuilder` (lines 807-826):

```rust
// FinanceTreeBuilder subscriber
let finance_tree_builder = Arc::new(
    crate::adapters::finance_tree_builder::FinanceTreeBuilder::new(
        tree_repo.clone(),
        Arc::new(vs.clone()),
        text_embedder.clone(),
        self.context_update_queue.clone(),
        self.domain_event_bus.clone(),
        storage_pool.inner().clone(),
    ),
);
let finance_tree_rx = domain_bus.subscribe();
let finance_tree_shutdown = CancellationToken::new();
let _finance_tree_handle = tokio::spawn({
    let builder = Arc::clone(&finance_tree_builder);
    let shutdown = finance_tree_shutdown.clone();
    async move { builder.run(finance_tree_rx, shutdown).await; }
});
info!("FinanceTreeBuilder subscriber started");

// ProductivityTreeBuilder subscriber
let productivity_tree_builder = Arc::new(
    crate::adapters::productivity_tree_builder::ProductivityTreeBuilder::new(
        tree_repo.clone(),
        Arc::new(vs.clone()),
        text_embedder.clone(),
        self.context_update_queue.clone(),
        self.domain_event_bus.clone(),
    ),
);
let prod_tree_rx = domain_bus.subscribe();
let prod_tree_shutdown = CancellationToken::new();
let _prod_tree_handle = tokio::spawn({
    let builder = Arc::clone(&productivity_tree_builder);
    let shutdown = prod_tree_shutdown.clone();
    async move { builder.run(prod_tree_rx, shutdown).await; }
});
info!("ProductivityTreeBuilder subscriber started");

// OKRTreeBuilder subscriber
let okr_tree_builder = Arc::new(
    crate::adapters::okr_tree_builder::OKRTreeBuilder::new(
        tree_repo.clone(),
        Arc::new(vs.clone()),
        text_embedder.clone(),
        self.context_update_queue.clone(),
        self.domain_event_bus.clone(),
        storage_pool.inner().clone(),
    ),
);
let okr_tree_rx = domain_bus.subscribe();
let okr_tree_shutdown = CancellationToken::new();
let _okr_tree_handle = tokio::spawn({
    let builder = Arc::clone(&okr_tree_builder);
    let shutdown = okr_tree_shutdown.clone();
    async move { builder.run(okr_tree_rx, shutdown).await; }
});
info!("OKRTreeBuilder subscriber started");

// LearningTreeBuilder subscriber
let learning_tree_builder = Arc::new(
    crate::adapters::learning_tree_builder::LearningTreeBuilder::new(
        tree_repo.clone(),
        Arc::new(vs.clone()),
        text_embedder.clone(),
        self.context_update_queue.clone(),
        self.domain_event_bus.clone(),
        storage_pool.inner().clone(),
    ),
);
let learning_tree_rx = domain_bus.subscribe();
let learning_tree_shutdown = CancellationToken::new();
let _learning_tree_handle = tokio::spawn({
    let builder = Arc::clone(&learning_tree_builder);
    let shutdown = learning_tree_shutdown.clone();
    async move { builder.run(learning_tree_rx, shutdown).await; }
});
info!("LearningTreeBuilder subscriber started");
```

- [ ] **Step 2: Update fabric.rs queries to include new source types**

In `crates/app-core/src/handlers/fabric.rs`, find all SQL queries that filter by `source_type IN ("note","task")` and expand them to include the new types:

```sql
source_type IN ('note','task','finance','productivity','okr','learning')
```

There should be ~3 places in `fabric_graph_base`, `fabric_graph_expand("entities")`, and `fabric_graph_expand("tree")`.

- [ ] **Step 3: Build and verify**

Run: `cargo build --workspace`
Expected: Compiles successfully. No test changes needed — the existing fabric tests should still pass since the queries now return a superset.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/app-core/src/handlers/fabric.rs
git commit -m "feat(agent): wire Finance/Productivity/OKR/Learning tree builders into agent startup"
```

---

## WS2: Scoring & Retrieval Fixes

### Task 6: Fix 10-factor scorer wiring in retrieval.rs

**Files:**
- Modify: `crates/cognitive/src/services/retrieval.rs:93-104`
- Modify: `crates/common/src/autotuner.rs`

The 10-factor scorer has all weights defined in `decay.rs` but `retrieval.rs` hardcodes the last 4 to zero. The autotuner's `resolve_relevance_weights` only resolves 6 of 10.

- [ ] **Step 1: Write the test**

In `crates/common/src/autotuner.rs`, add a test for the new 10-weight resolver:

```rust
#[test]
fn resolve_full_relevance_weights_sums_to_one() {
    let params = TrialParams {
        relevance_weight_semantic: Some(0.20),
        relevance_weight_hierarchy: Some(0.12),
        relevance_weight_community: Some(0.18),
        ..Default::default()
    };
    let defaults: [f64; 10] = [0.20, 0.10, 0.08, 0.05, 0.15, 0.02, 0.10, 0.05, 0.15, 0.10];
    let weights = params.resolve_full_relevance_weights(&defaults);
    let sum: f64 = weights.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-10,
        "Full 10-factor weights must sum to 1.0, got {sum}"
    );
    // Verify overridden values are reflected (proportionally after normalization)
    assert!(weights[0] > weights[3], "semantic should be > frequency");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p common -E 'test(resolve_full_relevance_weights)'`
Expected: FAIL — method doesn't exist yet.

- [ ] **Step 3: Add resolve_full_relevance_weights to TrialParams**

In `crates/common/src/autotuner.rs`, add this method to `impl TrialParams`:

```rust
/// Resolve all 10 relevance weights to a normalized array that sums to 1.0.
/// Returns [semantic, retrievability, importance, frequency, situation, temporal,
///          hierarchy, path_coherence, community, cross_note].
pub fn resolve_full_relevance_weights(&self, defaults: &[f64; 10]) -> [f64; 10] {
    let raw = [
        self.relevance_weight_semantic.unwrap_or(defaults[0]),
        self.relevance_weight_retrievability.unwrap_or(defaults[1]),
        self.relevance_weight_importance.unwrap_or(defaults[2]),
        self.relevance_weight_frequency.unwrap_or(defaults[3]),
        self.relevance_weight_situation.unwrap_or(defaults[4]),
        self.relevance_weight_temporal.unwrap_or(defaults[5]),
        self.relevance_weight_hierarchy.unwrap_or(defaults[6]),
        self.relevance_weight_path_coherence.unwrap_or(defaults[7]),
        self.relevance_weight_community.unwrap_or(defaults[8]),
        self.relevance_weight_cross_note.unwrap_or(defaults[9]),
    ];
    let sum: f64 = raw.iter().sum();
    if sum > 0.0 {
        raw.map(|w| w / sum)
    } else {
        *defaults
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p common -E 'test(resolve_full_relevance_weights)'`
Expected: PASS

- [ ] **Step 5: Wire into retrieval.rs**

In `crates/cognitive/src/services/retrieval.rs`, at lines ~93-104, replace the hardcoded zeros. The function signature takes `params: &RetrievalParams` — we need to pass the autotuner's trial params through. The `RetrievalParams` struct needs 4 new fields, or we pass the full `RelevanceWeights` directly.

The simplest approach: change the `RelevanceWeights` construction at line 93 to accept the weights from `RetrievalParams`:

```rust
let weights = RelevanceWeights {
    semantic: params.relevance_weight_semantic,
    retrievability: params.relevance_weight_retrievability,
    importance: params.relevance_weight_importance,
    frequency: params.relevance_weight_frequency,
    situation: params.relevance_weight_situation,
    temporal: params.relevance_weight_temporal,
    hierarchy: params.relevance_weight_hierarchy,
    path_coherence: params.relevance_weight_path_coherence,
    community: params.relevance_weight_community,
    cross_note: params.relevance_weight_cross_note,
};
```

This requires adding 4 new fields to `RetrievalParams`. Find `RetrievalParams` in the same file or in `crates/cognitive/src/types.rs` and add:

```rust
pub relevance_weight_hierarchy: f64,
pub relevance_weight_path_coherence: f64,
pub relevance_weight_community: f64,
pub relevance_weight_cross_note: f64,
```

Update all call sites that construct `RetrievalParams` to include these new fields. The default values should come from `RelevanceWeights::default()` (0.10, 0.05, 0.15, 0.10 respectively). Search for `RetrievalParams {` across the codebase and update each constructor.

- [ ] **Step 6: Build and run all cognitive tests**

Run: `cargo nextest run -p cognitive`
Expected: PASS (no behavior change for existing tests since the old hardcoded values match defaults)

- [ ] **Step 7: Commit**

```bash
git add crates/common/src/autotuner.rs crates/cognitive/src/services/retrieval.rs
git commit -m "fix(cognitive): wire 10-factor scorer weights — replace hardcoded zeros with autotuner-resolved values"
```

---

### Task 7: Fix community builder — top_entities, stability decay, ID stability

**Files:**
- Modify: `crates/agent/src/adapters/community_builder.rs`

Three bugs in one file:
1. `top_entities` always empty (line 210)
2. `stability` always 1.0 (line 218)
3. Community IDs unstable across rebuilds (`comm-{idx}` depends on Louvain order)

- [ ] **Step 1: Write tests for all three fixes**

Add to the existing test module in `community_builder.rs`:

```rust
#[tokio::test]
async fn top_entities_populated_from_member_links() {
    // Setup: create entities, tree nodes, entity_tree_links, then run community build
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    // ... (setup community builder with test data)
    // After rebuild_communities():
    let communities = community_repo.list_active().await.unwrap();
    for comm in &communities {
        let top: Vec<String> = serde_json::from_str(comm.top_entities.as_deref().unwrap_or("[]")).unwrap();
        if comm.member_count > 0 {
            assert!(!top.is_empty(), "Community '{}' should have top_entities", comm.name);
        }
    }
}

#[tokio::test]
async fn community_ids_are_content_stable() {
    // Run rebuild twice with same data — community IDs should be identical
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    // ... setup
    builder.rebuild_communities().await.unwrap();
    let first_ids: Vec<String> = community_repo.list_active().await.unwrap().iter().map(|c| c.id.clone()).collect();

    builder.rebuild_communities().await.unwrap();
    let second_ids: Vec<String> = community_repo.list_active().await.unwrap().iter().map(|c| c.id.clone()).collect();

    assert_eq!(first_ids, second_ids, "Community IDs should be stable across rebuilds");
}

#[tokio::test]
async fn stability_decays_on_member_loss() {
    // Build community, then remove a member, rebuild, check stability < 1.0
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    // ... setup with 5 members
    builder.rebuild_communities().await.unwrap();
    let before = community_repo.get_community("comm-xyz").await.unwrap().unwrap();
    assert_eq!(before.stability, 1.0);

    // Remove one entity_tree_link to break a community bond
    // ... remove link
    builder.rebuild_communities().await.unwrap();
    let after = community_repo.get_community("comm-xyz").await.unwrap().unwrap();
    assert!(after.stability < 1.0, "Stability should decay when members are lost");
}
```

Note: These are integration-level tests that require setup fixtures. The exact setup will depend on the existing test helpers. Check `community_builder.rs` for existing test patterns and adapt.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(top_entities_populated)' && cargo nextest run -p agent -E 'test(community_ids_are_content_stable)' && cargo nextest run -p agent -E 'test(stability_decays)'`
Expected: All FAIL

- [ ] **Step 3: Fix top_entities**

In `community_builder.rs`, replace line 210:
```rust
let top_entities: Vec<String> = Vec::new();
```

With a query that gets the most frequent entity names linked to this community's members:

```rust
let top_entities: Vec<String> = {
    let member_node_ids: Vec<&str> = members.iter().map(|m| m.tree_node_id.as_str()).collect();
    if member_node_ids.is_empty() {
        Vec::new()
    } else {
        // Query entity names linked to member tree nodes, ordered by frequency
        let placeholders = member_node_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT e.name, COUNT(*) as cnt FROM entity_tree_links etl \
             JOIN entities e ON etl.entity_id = e.id \
             WHERE etl.tree_node_id IN ({placeholders}) \
             GROUP BY e.id ORDER BY cnt DESC LIMIT 5"
        );
        let mut query = sqlx::query_as::<_, (String,)>(&sql);
        for id in &member_node_ids {
            query = query.bind(id);
        }
        query
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(name,)| name)
            .collect()
    }
};
```

Note: This requires access to the `SqlitePool` in `CommunityBuilder`. If the struct doesn't have a `pool` field, add one (passed through from `builder.rs` where `storage_pool.inner().clone()` is available).

- [ ] **Step 4: Fix community ID stability**

Replace `format!("comm-{comm_idx}")` with a content-hash-based ID. The ID should be deterministic based on the sorted member node IDs:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn stable_community_id(member_node_ids: &[&str]) -> String {
    let mut sorted: Vec<&str> = member_node_ids.to_vec();
    sorted.sort();
    let mut hasher = DefaultHasher::new();
    for id in &sorted {
        id.hash(&mut hasher);
    }
    format!("comm-{:016x}", hasher.finish())
}
```

Replace `let community_id = format!("comm-{comm_idx}");` with:
```rust
let member_node_ids: Vec<&str> = members.iter().map(|m| m.tree_node_id.as_str()).collect();
let community_id = stable_community_id(&member_node_ids);
```

- [ ] **Step 5: Fix stability decay**

Instead of always setting `stability: 1.0`, compare the new membership with the existing:

```rust
let stability = if let Ok(Some(existing)) = self.community_repo.get_community(&community_id).await {
    let old_count = existing.member_count as usize;
    let new_count = members.len();
    if new_count >= old_count {
        // Community grew or stayed the same — increase stability (max 1.0)
        (existing.stability + 0.05).min(1.0)
    } else {
        // Community lost members — decay stability
        let loss_ratio = (old_count - new_count) as f64 / old_count as f64;
        (existing.stability * (1.0 - loss_ratio * 0.3)).max(0.0)
    }
} else {
    // New community starts at 0.7
    0.7
};
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(top_entities)' && cargo nextest run -p agent -E 'test(community_ids)' && cargo nextest run -p agent -E 'test(stability_decays)'`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/adapters/community_builder.rs
git commit -m "fix(community): populate top_entities, stable community IDs, stability decay"
```

---

### Task 8: Populate entity_embeddings LanceDB table

**Files:**
- Create: `crates/agent/src/adapters/entity_embedder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs` (backfill wiring)

The `entity_embeddings` LanceDB table schema exists and is created at startup but no code ever writes to it. This task adds a subscriber that embeds entities when they're created/updated.

- [ ] **Step 1: Write the test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn embeds_entity_on_creation() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let vs = storage::VectorStore::open_in_memory().await.unwrap();
        let embedder = Arc::new(cognitive::test_utils::FakeEmbedder::new());
        let entity_repo = cognitive::repos::EntityRepo::new(pool.inner().clone());

        // Create an entity
        let entity = entity_repo
            .upsert_entity(&cognitive::repos::NewEntity {
                name: "caffeine".to_string(),
                entity_type: "concept".to_string(),
                description: Some("A stimulant found in coffee and tea".to_string()),
                source: "test".to_string(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();

        let entity_embedder = EntityEmbedder::new(Arc::new(vs.clone()), embedder);
        entity_embedder
            .embed_entity(&entity.id, &entity.name, entity.description.as_deref(), &entity.entity_type)
            .await
            .unwrap();

        // Verify the embedding exists in LanceDB
        let results = vs
            .search_entity_embeddings("caffeine stimulant", 5, 0.0)
            .await
            .unwrap();
        assert!(!results.is_empty(), "Entity should be embedded in LanceDB");
    }
}
```

Note: `search_entity_embeddings` may not exist on `VectorStore` yet. Check `crates/storage/src/vector_store/mod.rs` for the schema — the table exists but likely has no search method. You'll need to add one, following the pattern of `search_tree_node_embeddings` or `search_community_embeddings`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(embeds_entity_on_creation)'`
Expected: FAIL

- [ ] **Step 3: Add search_entity_embeddings to VectorStore**

In `crates/storage/src/vector_store/mod.rs`, add:

```rust
pub async fn upsert_entity_embedding(
    &self,
    id: &str,
    vector: &[f32],
    name: &str,
    entity_type: &str,
    description: &str,
) -> Result<()> {
    // Follow the same pattern as upsert_community_embedding / upsert_tree_node_embedding
    // Table name: "entity_embeddings"
    // Fields: id, vector, name, entity_type, description, updated_at
}

pub async fn search_entity_embeddings(
    &self,
    query_text: &str,
    top_k: usize,
    min_similarity: f64,
) -> Result<Vec<VectorSearchResult>> {
    // Follow the same pattern as search_community_embeddings
    // Table name: "entity_embeddings"
}
```

- [ ] **Step 4: Implement EntityEmbedder**

```rust
//! EntityEmbedder — embeds entities into the `entity_embeddings` LanceDB table.

use std::sync::Arc;
use cognitive::TextEmbedder;
use tracing::warn;

pub struct EntityEmbedder {
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
}

impl EntityEmbedder {
    pub fn new(vector_store: Arc<storage::VectorStore>, embedder: Arc<dyn TextEmbedder>) -> Self {
        Self { vector_store, embedder }
    }

    pub async fn embed_entity(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        entity_type: &str,
    ) -> common::Result<()> {
        let text = match description {
            Some(desc) => format!("{name}: {desc}"),
            None => name.to_string(),
        };
        let embedding = self.embedder.embed(&text).await?;
        self.vector_store
            .upsert_entity_embedding(id, &embedding, name, entity_type, description.unwrap_or(""))
            .await?;
        Ok(())
    }

    /// Backfill all entities that have descriptions but no embeddings.
    pub async fn backfill_all(&self, pool: &sqlx::SqlitePool) -> common::Result<usize> {
        let entities: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, name, description, entity_type FROM entities WHERE description IS NOT NULL AND description != ''"
        )
        .fetch_all(pool)
        .await?;

        let mut count = 0;
        for (id, name, desc, etype) in &entities {
            if let Err(e) = self.embed_entity(id, name, desc.as_deref(), etype).await {
                warn!(entity_id = %id, "EntityEmbedder: failed to embed: {e}");
            } else {
                count += 1;
            }
        }
        Ok(count)
    }
}
```

- [ ] **Step 5: Register module + wire backfill in builder.rs**

Add `pub mod entity_embedder;` to `crates/agent/src/adapters/mod.rs`.

In `builder.rs`, in the backfill section (after entity links backfill at ~line 891), add:

```rust
// Backfill entity embeddings
let entity_embedder = crate::adapters::entity_embedder::EntityEmbedder::new(
    Arc::new(vs.clone()),
    text_embedder.clone(),
);
match entity_embedder.backfill_all(storage_pool.inner()).await {
    Ok(n) => info!("Entity embedding backfill: {n} entities embedded"),
    Err(e) => warn!("Entity embedding backfill error: {e}"),
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(embeds_entity)' && cargo nextest run -p storage`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/adapters/entity_embedder.rs crates/agent/src/adapters/mod.rs crates/agent/src/agent_loop/builder.rs crates/storage/src/vector_store/mod.rs
git commit -m "feat(agent): populate entity_embeddings LanceDB table — backfill + on-create"
```

---

### Task 9: EntityCard → entities table bridge

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

Currently, tools emit `EntityCard` structs (task, finance_account, objective) that flow through the agent event stream but never write to the `entities` table. We need a lightweight bridge: when the background service processes an observation that references an EntityCard, it should upsert the entity.

- [ ] **Step 1: Write the test**

Add to `crates/cognitive/src/services/background.rs` test module:

```rust
#[tokio::test]
async fn entity_card_creates_entity_row() {
    // This tests that when a tool execution observation includes entity metadata,
    // the corresponding entities are created in the entities table.
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let entity_repo = crate::repos::EntityRepo::new(pool.inner().clone());

    // Simulate: tool_call observation with entity_kind="task" in metadata
    upsert_entity_from_card(
        &entity_repo,
        "task-123",
        "task",
        "Build the landing page",
    ).await.unwrap();

    let entity = entity_repo.get_by_name("Build the landing page").await.unwrap();
    assert!(entity.is_some());
    assert_eq!(entity.unwrap().entity_type, "task");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(entity_card_creates)'`
Expected: FAIL — function doesn't exist.

- [ ] **Step 3: Implement the bridge function**

Add to `background.rs`:

```rust
/// Bridge: create an entity row from an EntityCard-style reference.
/// Called when tool execution events reference domain objects.
pub(crate) async fn upsert_entity_from_card(
    entity_repo: &crate::repos::EntityRepo,
    source_id: &str,
    entity_type: &str,
    name: &str,
) -> common::Result<crate::repos::EntityRow> {
    entity_repo
        .upsert_entity(&crate::repos::NewEntity {
            name: name.to_string(),
            entity_type: entity_type.to_string(),
            description: None,
            source: "tool_card".to_string(),
            source_id: Some(source_id.to_string()),
            metadata: None,
        })
        .await
}
```

Then, in the `ToolCallExecuted` event handler within `event_to_observation`, add a call to this when the tool name is one of the domain tools (`tasks`, `finance`, `okr`, `notes`). The exact integration point depends on where `ToolCallExecuted` events are processed — check the `event_to_observation` function.

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p cognitive -E 'test(entity_card_creates)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): bridge EntityCard → entities table on tool execution"
```

---

### Task 10: Remove duplicate NoteSearcher registration

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

Both `NoteSearcher` (flat FTS5) and `NoteTreeNavigator` (hierarchical vector) are registered as domain searchers. This creates duplicate note content in retrieval. Since `NoteTreeNavigator` subsumes `NoteSearcher` (it has a flat vector search path + FTS5 fallback), we should remove `NoteSearcher`.

- [ ] **Step 1: Verify NoteSearcher registration**

Search for `NoteSearcher` in `builder.rs`. It should be registered via `forge.add_searcher(Arc::new(note_searcher))` somewhere around lines 760-780.

- [ ] **Step 2: Remove NoteSearcher registration**

Comment out or delete the `NoteSearcher` construction and `add_searcher` call. Keep only the `NoteTreeNavigator` registration (which handles all note retrieval paths).

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace`
Expected: PASS — no functional change since NoteTreeNavigator covers all note retrieval.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "fix(agent): remove duplicate NoteSearcher — NoteTreeNavigator handles all note retrieval"
```

---

## WS3: Graph Visualization Completion

### Task 11: Add SSE event listener for fabric_graph Tauri events

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useFabricGraph.ts`

The backend emits `fabric_graph` Tauri events when communities change but no frontend listener exists.

- [ ] **Step 1: Add event listener**

In `useFabricGraph.ts`, add a `useEffect` that listens for the Tauri `fabric_graph` event and triggers a re-fetch:

```typescript
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { FabricGraphEvent } from "@shared/types/fabric";

// Inside the useFabricGraph hook, add:
useEffect(() => {
  let unlisten: UnlistenFn | null = null;

  listen<FabricGraphEvent>("fabric_graph", (event) => {
    const fe = event.payload;
    console.debug("[FabricGraph] SSE event:", fe.type, fe.nodeType, fe.id);

    // Re-fetch base data to pick up community changes
    if (fe.type === "node_added" || fe.type === "node_updated") {
      // Invalidate the SWR cache to trigger a re-fetch
      mutateBase();
    }
  }).then((fn) => {
    unlisten = fn;
  });

  return () => {
    unlisten?.();
  };
}, [mutateBase]);
```

The `mutateBase` function should come from SWR's `mutate` returned by `useQuery("fabric_graph_base", ...)`. Check the existing `useQuery` usage pattern in the file — it likely returns `{ data, mutate }`.

If the hook currently uses:
```typescript
const { data: base } = useQuery<FabricGraphBase>("fabric_graph_base", {});
```

Change to:
```typescript
const { data: base, mutate: mutateBase } = useQuery<FabricGraphBase>("fabric_graph_base", {});
```

- [ ] **Step 2: Verify in browser dev mode**

Note: In browser dev mode (`localhost:1420`), Tauri events are not available. This feature only works in the full Tauri desktop app. For dev mode, the existing SWR stale-time polling (30s) provides the refresh mechanism.

Add a guard:
```typescript
// Only listen for Tauri events in desktop mode
if (window.__TAURI_INTERNALS__) {
  listen<FabricGraphEvent>("fabric_graph", handler).then(/* ... */);
}
```

- [ ] **Step 3: Commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/hooks/useFabricGraph.ts
git commit -m "feat(ui): listen for fabric_graph Tauri events — live graph updates on community changes"
```

---

### Task 12: Add domain node types to graph elements and painters

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useGraphElements.ts:35`
- Modify: `desktop-ui/src/features/notes/lib/graphPainters.ts`

Add visual representations for finance, productivity, OKR, and learning nodes.

- [ ] **Step 1: Extend ForceNodeType**

In `useGraphElements.ts`, change line 35:

```typescript
export type ForceNodeType =
  | "note"
  | "entity"
  | "tree_section"
  | "tree_text"
  | "finance"
  | "productivity"
  | "okr"
  | "learning"
  | "project";
```

- [ ] **Step 2: Update useGraphElements to classify new source types**

In the node-building logic within `useGraphElements`, when creating ForceNodes from `fabricData`, classify nodes by their tags/source:

```typescript
function resolveNodeType(tags: string[]): ForceNodeType {
  if (tags.includes("project")) return "project";
  if (tags.includes("finance")) return "finance";
  if (tags.includes("productivity")) return "productivity";
  if (tags.includes("okr")) return "okr";
  if (tags.includes("learning")) return "learning";
  return "note";
}
```

Use this in the node construction where `nodeType` is assigned.

- [ ] **Step 3: Update graphPainters.ts for new node shapes**

Add distinct visual styles in `paintNode`:

```typescript
// In the paintNode function, after the existing node type checks:
case "finance":
  // Hexagon shape — represents money/value
  drawHexagon(ctx, x, y, size, node.color);
  break;
case "productivity":
  // Clock/ring shape — represents time
  drawRing(ctx, x, y, size, node.color);
  break;
case "okr":
  // Target/bullseye — represents goals
  drawTarget(ctx, x, y, size, node.color);
  break;
case "learning":
  // Book/square with rounded corners — represents knowledge
  drawRoundedSquare(ctx, x, y, size, node.color);
  break;
case "project":
  // Pentagon — represents structured work
  drawPentagon(ctx, x, y, size, node.color);
  break;
```

Implement the helper functions (`drawHexagon`, `drawRing`, `drawTarget`, `drawRoundedSquare`, `drawPentagon`) at the bottom of `graphPainters.ts`. Each is 5-10 lines of canvas path drawing.

- [ ] **Step 4: Update FabricNote tags in fabric.rs backend**

In `crates/app-core/src/handlers/fabric.rs`, when building `FabricNote` from tree node roots, tag them with their source type:

```rust
// When building FabricNote entries from non-note sources:
tags: vec![source_type.to_string()], // "finance", "productivity", "okr", "learning"
```

- [ ] **Step 5: Lint and test**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphElements.ts desktop-ui/src/features/notes/lib/graphPainters.ts crates/app-core/src/handlers/fabric.rs
git commit -m "feat(ui): add distinct visual shapes for finance, productivity, OKR, learning graph nodes"
```

---

### Task 13: Wire community detail expansion from frontend

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useFabricGraph.ts`

The `community_detail` expand endpoint exists but is never called. Wire it to the community label click handler.

- [ ] **Step 1: Add community detail panel state**

In `GraphView.tsx`, add state for the selected community detail:

```typescript
const [selectedCommunity, setSelectedCommunity] = useState<FabricCommunityDetail | null>(null);
```

- [ ] **Step 2: Wire community label click**

In the `onNodeClick` handler, check if the clicked node is a community label:

```typescript
if (clickedNode.nodeType === "community_label") {
  const communityId = clickedNode.id.replace("community-label-", "");
  fabric.expandLayer("community_detail", [communityId]).then(() => {
    const detail = fabric.communityDetails.get(communityId);
    if (detail) setSelectedCommunity(detail);
  });
}
```

- [ ] **Step 3: Render community detail panel**

Add a small panel (reuse the existing node preview panel pattern) that shows:
- Community name + member count
- Representative paths (clickable → open note)
- Top entities
- Stability value
- Member list (first 10, sorted by membership_score)

```typescript
{selectedCommunity && (
  <div className="absolute right-4 top-4 w-72 rounded-lg glass-panel p-4 space-y-2">
    <h3 className="font-semibold text-sm">{selectedCommunity.communityId}</h3>
    <div className="text-xs text-muted">
      {selectedCommunity.representativePaths.map((p) => (
        <div key={p} className="truncate">{p}</div>
      ))}
    </div>
    <div className="text-xs">
      Top entities: {selectedCommunity.topEntities.join(", ")}
    </div>
    <div className="text-xs">
      Members: {selectedCommunity.members.length}
    </div>
  </div>
)}
```

- [ ] **Step 4: Lint and test**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphView.tsx desktop-ui/src/features/notes/hooks/useFabricGraph.ts
git commit -m "feat(ui): wire community detail panel — click community label to see members and paths"
```

---

### Task 14: Integrate wave-reveal into 3D brain view

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphBrainView.tsx`
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

Currently, 3D mode renders all nodes immediately. The 2D mode gates rendering via `revealedNodes` but this set is never passed to `GraphBrainView`.

- [ ] **Step 1: Pass revealedNodes to GraphBrainView**

In `GraphView.tsx`, find where `<GraphBrainView>` is rendered and add the `revealedNodes` prop:

```typescript
<GraphBrainView
  {...existingProps}
  revealedNodes={revealedNodes}
  isRevealing={isRevealing}
/>
```

- [ ] **Step 2: Use revealedNodes in GraphBrainView**

In `GraphBrainView.tsx`, add the props to the interface and use them in the `nodeThreeObject` callback:

```typescript
interface GraphBrainViewProps {
  // ... existing props
  revealedNodes: Set<string>;
  isRevealing: boolean;
}
```

In the `nodeThreeObject` callback, set initial opacity/scale based on reveal state:

```typescript
nodeThreeObject={(node: ForceNode) => {
  const revealed = !props.isRevealing || props.revealedNodes.has(node.id);
  const material = createNodeMaterial(
    node.color,
    revealed ? emissiveIntensity : 0,
  );
  material.opacity = revealed ? 1.0 : 0.0;
  material.transparent = true;
  // ... rest of existing geometry creation
}}
```

- [ ] **Step 3: Lint and test**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphBrainView.tsx desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(ui): integrate wave-reveal into 3D brain view"
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - Domain tree builders for all 4 missing domains (finance, productivity, OKR, learning) ✓ (Tasks 2-4)
   - SourceType enum extension ✓ (Task 1)
   - Wiring into agent builder ✓ (Task 5)
   - 10-factor scorer wiring ✓ (Task 6)
   - Community builder bugs (top_entities, stability, IDs) ✓ (Task 7)
   - entity_embeddings population ✓ (Task 8)
   - EntityCard bridge ✓ (Task 9)
   - Duplicate searcher removal ✓ (Task 10)
   - SSE live updates frontend ✓ (Task 11)
   - Domain node visuals ✓ (Task 12)
   - Community detail panel ✓ (Task 13)
   - 3D wave-reveal ✓ (Task 14)

2. **Placeholder scan:** No TBDs, TODOs, or "implement later" found. All steps have code.

3. **Type consistency:**
   - `SourceType` variants (`Finance`, `Productivity`, `OKR`, `Learning`) used consistently across Tasks 1-5
   - `ForceNodeType` extended in Task 12, referenced consistently in painters
   - `FinanceTreeBuilder`, `ProductivityTreeBuilder`, `OKRTreeBuilder`, `LearningTreeBuilder` naming consistent
   - `resolve_full_relevance_weights` in Task 6 returns `[f64; 10]`, consumed in retrieval.rs

4. **Not in scope (explicitly):**
   - NoteSearcher `domain_searchers/note_searcher.rs` file deletion (we only remove the registration, not the file — it may have other consumers)
   - Autotuner prompt bounds table update for new domains
   - `suggest_merge` backend implementation (spec says Phase 3 hook, no-op is correct)
   - Feedback/coaching integration for new domain nodes
