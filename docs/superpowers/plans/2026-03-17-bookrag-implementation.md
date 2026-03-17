# BookRAG-Style Architecture Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add hierarchical structure-aware retrieval (BookRAG) to Klyntbot's RAG pipeline — tree index, entity graph integration, gradient-based entity resolution, composable operator library, and agent-based retrieval planner — all surfaced as a new DomainSearcher in InsightForge.

**Architecture:** BookIndex `B = (T, G, M)` lives in `context_engine` as traits, with concrete SQLite implementations in `cognitive`. A `BookRAGSearcher` (implementing `DomainSearcher`) wraps a `RetrievalPlanner` that classifies queries and generates operator pipelines. Results merge into InsightForge's existing RRF pipeline with zero changes to `ContextEngine` or `AgentRuntime`.

**Tech Stack:** Rust, SQLite (FTS5), LanceDB (384-dim fastembed), async_trait, tokio

**Spec:** `docs/superpowers/specs/2026-03-17-bookrag-architecture-design.md`

---

## File Structure

### New files to create

```
crates/context_engine/src/
    book_index/
        mod.rs              -- BookIndex orchestrator + BookEmbedder trait
        tree.rs             -- TreeNode types + BookTreeRepo trait
        gt_link.rs          -- GTLinkRepo trait + types
        entity_resolution.rs -- Gradient-based ER (Algorithm 1) + detect_gradient
        types.rs            -- TreeNodeType, SourceType, ScoredNode, configs
    operators/
        mod.rs              -- Operator trait, OperatorType, OperatorContext, OperatorLlm, pipeline runner
        formulator.rs       -- Decompose, Extract operators
        selector.rs         -- FilterModal, FilterRange, SelectByEntity, SelectBySection
        reasoner.rs         -- GraphReasoning (PageRank), TextRanker, SkylineRanker
        synthesizer.rs      -- Map, Reduce, SubQueryExecutor
    retrieval_planner/
        mod.rs              -- RetrievalPlanner, QueryCategory, RetrievalPlan
        classifier.rs       -- Heuristic + LLM-fallback query classification
    insight_forge/
        bookrag_searcher.rs -- BookRAGSearcher (DomainSearcher impl)

crates/cognitive/src/
    repos/
        book_tree.rs        -- Concrete BookTreeRepo (SQLite)
        gt_link.rs          -- Concrete GTLinkRepo (SQLite)
    migrations/
        002_book_index_tables.sql -- DDL for book_tree_nodes, entity_tree_links, FTS5, triggers

crates/agent/src/
    adapters/
        book_index_wiring.rs -- BookEmbedder adapter + BookIndex construction + searcher wiring
```

### Existing files to modify

```
crates/context_engine/src/lib.rs:1-10          -- Add pub mod book_index, operators, retrieval_planner
crates/context_engine/src/insight_forge/mod.rs:1-9 -- Add pub mod bookrag_searcher
crates/cognitive/src/repos/mod.rs:1-28         -- Add pub mod book_tree, gt_link + re-exports + migration
crates/cognitive/src/repos/entity.rs:380-441   -- Add GT-Link migration in merge_entities
crates/storage/src/vector_store/schemas.rs:82-93 -- Add entity_embedding_schema
crates/storage/src/vector_store/mod.rs:92-94   -- Add ensure_table("entity_embeddings")
crates/bus/src/domain_events.rs:208-215        -- Add NoteContentChanged, NoteDeleted, TaskHierarchyChanged
crates/config/src/schema/cognitive.rs:7-107    -- Add BookIndexConfig nested struct
crates/agent/src/agent_loop/builder.rs:689-691 -- Wire BookRAGSearcher via book_index_wiring
crates/agent/src/adapters/mod.rs:1-18          -- Add pub mod book_index_wiring
```

---

## Task 1: Storage Foundation — SQLite Tables + LanceDB Schema

**Files:**
- Create: `crates/cognitive/src/migrations/002_book_index_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs:52-60`
- Modify: `crates/storage/src/vector_store/schemas.rs:82-93`
- Modify: `crates/storage/src/vector_store/mod.rs:92-94`

- [ ] **Step 1: Write the migration SQL**

Create `crates/cognitive/src/migrations/002_book_index_tables.sql`:

```sql
-- BookIndex tree nodes (hierarchical document structure)
CREATE TABLE IF NOT EXISTS book_tree_nodes (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES book_tree_nodes(id),
    node_type TEXT NOT NULL,
    content TEXT NOT NULL,
    title TEXT,
    level INTEGER NOT NULL DEFAULT 0,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tree_nodes_parent ON book_tree_nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_tree_nodes_source ON book_tree_nodes(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_tree_nodes_level ON book_tree_nodes(level);

-- FTS5 for keyword search within tree nodes
CREATE VIRTUAL TABLE IF NOT EXISTS book_tree_nodes_fts USING fts5(
    title, content,
    content='book_tree_nodes',
    content_rowid='rowid',
    tokenize='porter'
);

-- FTS5 sync triggers
CREATE TRIGGER IF NOT EXISTS book_tree_nodes_ai AFTER INSERT ON book_tree_nodes BEGIN
    INSERT INTO book_tree_nodes_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS book_tree_nodes_ad AFTER DELETE ON book_tree_nodes BEGIN
    INSERT INTO book_tree_nodes_fts(book_tree_nodes_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

CREATE TRIGGER IF NOT EXISTS book_tree_nodes_au AFTER UPDATE ON book_tree_nodes BEGIN
    INSERT INTO book_tree_nodes_fts(book_tree_nodes_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO book_tree_nodes_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS book_tree_nodes_update_ts AFTER UPDATE ON book_tree_nodes BEGIN
    UPDATE book_tree_nodes SET updated_at = datetime('now') WHERE id = new.id;
END;

-- GT-Link: entity-to-tree-node mapping
CREATE TABLE IF NOT EXISTS entity_tree_links (
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    tree_node_id TEXT NOT NULL REFERENCES book_tree_nodes(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (entity_id, tree_node_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_tree_links_node ON entity_tree_links(tree_node_id);
```

- [ ] **Step 2: Register migration in cognitive_migrations**

In `crates/cognitive/src/repos/mod.rs`, add to the `cognitive_migrations()` vec (after the existing entry):

```rust
FeatureMigration {
    feature_name: "cognitive".to_string(),
    version: 5,
    description: "Add BookIndex tree nodes and GT-Link tables".to_string(),
    sql: include_str!("../migrations/002_book_index_tables.sql").to_string(),
}
```

- [ ] **Step 3: Add entity_embedding_schema to VectorStore**

In `crates/storage/src/vector_store/schemas.rs`, after `cognitive_fact_schema()` (~line 93):

```rust
pub(crate) fn entity_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("name", DataType::Utf8, false),
        Field::new("entity_type", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

In `crates/storage/src/vector_store/mod.rs`, after line 94 add:

```rust
store
    .ensure_table("entity_embeddings", schemas::entity_embedding_schema())
    .await?;
```

- [ ] **Step 4: Verify migration runs**

Run: `cargo nextest run -p cognitive -E 'test(migration)' 2>&1 | head -30`

If no migration test exists, write a quick smoke test in `crates/cognitive/src/repos/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn book_index_tables_created() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &cognitive_migrations(),
        ).await.unwrap();
        // Verify tables exist
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='book_tree_nodes'"
        ).fetch_one(pool.inner()).await.unwrap();
        assert_eq!(row.0, 1);

        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entity_tree_links'"
        ).fetch_one(pool.inner()).await.unwrap();
        assert_eq!(row.0, 1);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(book_index_tables)' -v`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/migrations/002_book_index_tables.sql \
  crates/cognitive/src/repos/mod.rs \
  crates/storage/src/vector_store/schemas.rs \
  crates/storage/src/vector_store/mod.rs
git commit -m "feat(bookrag): add storage foundation — SQLite tables, FTS5, entity embeddings"
```

---

## Task 2: Core Types + Traits in context_engine

**Files:**
- Create: `crates/context_engine/src/book_index/types.rs`
- Create: `crates/context_engine/src/book_index/tree.rs`
- Create: `crates/context_engine/src/book_index/gt_link.rs`
- Create: `crates/context_engine/src/book_index/mod.rs`
- Modify: `crates/context_engine/src/lib.rs:1-10`

- [ ] **Step 1: Create types.rs — all BookIndex value types**

```rust
// crates/context_engine/src/book_index/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeNodeType {
    Section,
    Text,
    Table,
    Code,
    Task,
    ListItem,
}

impl TreeNodeType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Section => "Section",
            Self::Text => "Text",
            Self::Table => "Table",
            Self::Code => "Code",
            Self::Task => "Task",
            Self::ListItem => "ListItem",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Section" => Self::Section,
            "Text" => Self::Text,
            "Table" => Self::Table,
            "Code" => Self::Code,
            "Task" => Self::Task,
            "ListItem" => Self::ListItem,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    Note,
    Task,
    Skill,
}

impl SourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Note => "Note",
            Self::Task => "Task",
            Self::Skill => "Skill",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Note" => Self::Note,
            "Task" => Self::Task,
            "Skill" => Self::Skill,
            _ => Self::Note,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub node_type: TreeNodeType,
    pub content: String,
    pub title: Option<String>,
    pub level: u32,
    pub source_type: SourceType,
    pub source_id: String,
    pub position: u32,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScoredNode {
    pub node: TreeNode,
    pub graph_score: f64,
    pub text_score: f64,
    pub combined: f64,
}

#[derive(Debug, Clone)]
pub struct EntityResolutionConfig {
    pub top_k: usize,
    pub gradient_threshold: f64,
    pub min_similarity: f64,
    pub use_llm_disambiguation: bool,
}

impl Default for EntityResolutionConfig {
    fn default() -> Self {
        Self {
            top_k: 10,
            gradient_threshold: 0.6,
            min_similarity: 0.3,
            use_llm_disambiguation: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BookRetrievalConfig {
    pub max_nodes: usize,
    pub max_map_nodes: usize,
    pub operator_timeout_ms: u64,
    pub pagerank_damping: f64,
    pub pagerank_iterations: u32,
}

impl Default for BookRetrievalConfig {
    fn default() -> Self {
        Self {
            max_nodes: 50,
            max_map_nodes: 10,
            operator_timeout_ms: 600,
            pagerank_damping: 0.85,
            pagerank_iterations: 20,
        }
    }
}
```

- [ ] **Step 2: Create tree.rs — BookTreeRepo trait**

```rust
// crates/context_engine/src/book_index/tree.rs

use async_trait::async_trait;
use common::Result;

use super::types::{SourceType, TreeNode};

/// Abstract repo for tree node CRUD. Concrete impl in cognitive crate (SQLite).
#[async_trait]
pub trait BookTreeRepo: Send + Sync {
    async fn insert_node(&self, node: &TreeNode) -> Result<()>;
    async fn insert_nodes(&self, nodes: &[TreeNode]) -> Result<()>;
    async fn get_node(&self, id: &str) -> Result<Option<TreeNode>>;
    async fn get_children(&self, parent_id: &str) -> Result<Vec<TreeNode>>;
    async fn get_subtree(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    async fn get_root_sections(&self, source_type: &SourceType) -> Result<Vec<TreeNode>>;
    async fn get_path_to_root(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    async fn delete_by_source(&self, source_type: &SourceType, source_id: &str) -> Result<u64>;
    async fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<TreeNode>>;
    async fn has_any_nodes(&self) -> Result<bool>;
}
```

- [ ] **Step 3: Create gt_link.rs — GTLinkRepo trait**

```rust
// crates/context_engine/src/book_index/gt_link.rs

use async_trait::async_trait;
use common::Result;

use super::types::TreeNode;

/// Abstract repo for entity ↔ tree node links.
#[async_trait]
pub trait GTLinkRepo: Send + Sync {
    async fn link(&self, entity_id: &str, tree_node_id: &str) -> Result<()>;
    async fn link_batch(&self, links: &[(String, String)]) -> Result<()>;
    async fn get_linked_nodes(&self, entity_id: &str) -> Result<Vec<TreeNode>>;
    async fn get_entities_in_subtree(&self, node_id: &str) -> Result<Vec<String>>;
    async fn delete_by_tree_node(&self, tree_node_id: &str) -> Result<u64>;
    async fn migrate_entity_links(&self, source_entity_id: &str, target_entity_id: &str) -> Result<()>;
}
```

- [ ] **Step 4: Create mod.rs — BookIndex orchestrator + BookEmbedder**

```rust
// crates/context_engine/src/book_index/mod.rs

pub mod gt_link;
pub mod tree;
pub mod types;
pub mod entity_resolution;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use common::Result;

pub use gt_link::GTLinkRepo;
pub use tree::BookTreeRepo;
pub use types::*;

/// Local embedding trait for context_engine (L3 cannot import cognitive L5).
#[async_trait]
pub trait BookEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

/// Trait wrapping the subset of EntityRepo operations BookIndex needs.
#[async_trait]
pub trait BookEntityRepo: Send + Sync {
    async fn find_by_name(&self, query: &str) -> Result<Vec<EntityInfo>>;
    async fn get_neighborhood_ids(&self, entity_id: &str, depth: u32) -> Result<Vec<(String, String, f64)>>;
}

/// Minimal entity info passed across the layer boundary.
#[derive(Debug, Clone)]
pub struct EntityInfo {
    pub id: String,
    pub name: String,
    pub entity_type: String,
}

pub struct BookIndex {
    tree_repo: Arc<dyn BookTreeRepo>,
    entity_repo: Arc<dyn BookEntityRepo>,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    embedder: Arc<dyn BookEmbedder>,
    has_content_flag: AtomicBool,
}

impl BookIndex {
    pub fn new(
        tree_repo: Arc<dyn BookTreeRepo>,
        entity_repo: Arc<dyn BookEntityRepo>,
        gt_link_repo: Arc<dyn GTLinkRepo>,
        embedder: Arc<dyn BookEmbedder>,
    ) -> Self {
        Self {
            tree_repo,
            entity_repo,
            gt_link_repo,
            embedder,
            has_content_flag: AtomicBool::new(false),
        }
    }

    pub fn has_content(&self) -> bool {
        self.has_content_flag.load(Ordering::Relaxed)
    }

    pub fn tree_repo(&self) -> &dyn BookTreeRepo {
        self.tree_repo.as_ref()
    }

    pub fn entity_repo(&self) -> &dyn BookEntityRepo {
        self.entity_repo.as_ref()
    }

    pub fn gt_link_repo(&self) -> &dyn GTLinkRepo {
        self.gt_link_repo.as_ref()
    }

    pub fn embedder(&self) -> &dyn BookEmbedder {
        self.embedder.as_ref()
    }

    pub async fn refresh_has_content(&self) -> Result<()> {
        let has = self.tree_repo.has_any_nodes().await?;
        self.has_content_flag.store(has, Ordering::Release);
        Ok(())
    }
}
```

- [ ] **Step 5: Register modules in context_engine/lib.rs**

In `crates/context_engine/src/lib.rs`, add at line 1:

```rust
pub mod book_index;
```

(Operators and retrieval_planner modules will be added in later tasks.)

- [ ] **Step 6: Verify it compiles**

Run: `cargo check -p context-engine 2>&1 | tail -10`
Expected: success (or only existing warnings)

- [ ] **Step 7: Commit**

```bash
git add crates/context_engine/src/book_index/ crates/context_engine/src/lib.rs
git commit -m "feat(bookrag): add core types and traits — BookIndex, BookTreeRepo, GTLinkRepo, BookEmbedder"
```

---

## Task 3: Concrete Repo Implementations in cognitive

**Files:**
- Create: `crates/cognitive/src/repos/book_tree.rs`
- Create: `crates/cognitive/src/repos/gt_link.rs`
- Modify: `crates/cognitive/src/repos/mod.rs:1-28`
- Modify: `crates/cognitive/src/repos/entity.rs:385-441`
- Test: inline `#[cfg(test)]` in each new file

- [ ] **Step 1: Write test for BookTreeRepo**

In `crates/cognitive/src/repos/book_tree.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> (SqliteBookTreeRepo, sqlx::SqlitePool) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(pool.inner(), &super::super::cognitive_migrations())
            .await.unwrap();
        (SqliteBookTreeRepo::new(pool.inner().clone()), pool.inner().clone())
    }

    #[tokio::test]
    async fn insert_and_get_subtree() {
        let (repo, _) = setup().await;
        let root = TreeNode {
            id: "root".into(), parent_id: None, node_type: TreeNodeType::Section,
            content: "Chapter 1".into(), title: Some("Chapter 1".into()),
            level: 0, source_type: SourceType::Note, source_id: "note-1".into(),
            position: 0, metadata: None,
        };
        let child = TreeNode {
            id: "child".into(), parent_id: Some("root".into()), node_type: TreeNodeType::Text,
            content: "Some paragraph".into(), title: None,
            level: 1, source_type: SourceType::Note, source_id: "note-1".into(),
            position: 0, metadata: None,
        };
        repo.insert_nodes(&[root, child]).await.unwrap();
        let subtree = repo.get_subtree("root").await.unwrap();
        assert_eq!(subtree.len(), 2); // root + child
    }

    #[tokio::test]
    async fn delete_by_source() {
        let (repo, _) = setup().await;
        let node = TreeNode {
            id: "n1".into(), parent_id: None, node_type: TreeNodeType::Text,
            content: "test".into(), title: None, level: 0,
            source_type: SourceType::Note, source_id: "note-1".into(),
            position: 0, metadata: None,
        };
        repo.insert_node(&node).await.unwrap();
        let deleted = repo.delete_by_source(&SourceType::Note, "note-1").await.unwrap();
        assert_eq!(deleted, 1);
        assert!(repo.get_node("n1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fts_search() {
        let (repo, _) = setup().await;
        let node = TreeNode {
            id: "n1".into(), parent_id: None, node_type: TreeNodeType::Text,
            content: "Rust programming language".into(), title: Some("Rust".into()),
            level: 0, source_type: SourceType::Note, source_id: "note-1".into(),
            position: 0, metadata: None,
        };
        repo.insert_node(&node).await.unwrap();
        let results = repo.search_fts("programming", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "n1");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(insert_and_get_subtree)' -v 2>&1 | tail -5`
Expected: FAIL (SqliteBookTreeRepo not defined)

- [ ] **Step 3: Implement SqliteBookTreeRepo**

Write the full `crates/cognitive/src/repos/book_tree.rs` implementation: `SqliteBookTreeRepo` struct with `new(pool)`, implement all `BookTreeRepo` trait methods using sqlx queries. Key implementation notes:

- `get_subtree` uses recursive CTE: `WITH RECURSIVE subtree AS (SELECT * FROM book_tree_nodes WHERE id = ? UNION ALL SELECT n.* FROM book_tree_nodes n JOIN subtree s ON n.parent_id = s.id) SELECT * FROM subtree`
- `get_path_to_root` uses upward recursive CTE
- `search_fts` joins `book_tree_nodes_fts` with base table via rowid
- `delete_by_source` deletes all nodes matching `(source_type, source_id)` — FTS5 triggers handle sync
- `has_any_nodes` is `SELECT EXISTS(SELECT 1 FROM book_tree_nodes LIMIT 1)`

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(insert_and_get|delete_by_source|fts_search)' -v`
Expected: all PASS

- [ ] **Step 5: Write test for GTLinkRepo**

In `crates/cognitive/src/repos/gt_link.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn link_and_query() {
        // Setup: create pool, run migrations, insert entity + tree node
        // Link entity to tree node
        // Query get_linked_nodes(entity_id) → should return the tree node
        // Query get_entities_in_subtree(node_id) → should return entity_id
    }

    #[tokio::test]
    async fn migrate_entity_links() {
        // Setup: create two entities, link entity-A to node-1
        // Migrate links from entity-A to entity-B
        // Verify: get_linked_nodes(entity-B) returns node-1
        // Verify: get_linked_nodes(entity-A) returns empty
    }
}
```

- [ ] **Step 6: Implement SqliteGTLinkRepo**

Write `crates/cognitive/src/repos/gt_link.rs`. Key: `link_batch` uses a transaction with `INSERT OR IGNORE` loop. `get_linked_nodes` joins `entity_tree_links` with `book_tree_nodes`. `migrate_entity_links` does `INSERT OR IGNORE ... SELECT` then `DELETE`.

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(link_and_query|migrate_entity_links)' -v`
Expected: PASS

- [ ] **Step 8: Add GT-Link migration to EntityRepo::merge_entities**

In `crates/cognitive/src/repos/entity.rs`, inside `merge_entities()` transaction (before the source entity DELETE at ~line 430), add:

```rust
// Migrate GT-Links before deleting source entity
sqlx::query(
    "INSERT OR IGNORE INTO entity_tree_links (entity_id, tree_node_id, created_at)
     SELECT ?, tree_node_id, created_at FROM entity_tree_links WHERE entity_id = ?"
)
.bind(target_id)
.bind(source_id)
.execute(&mut *tx)
.await?;

sqlx::query("DELETE FROM entity_tree_links WHERE entity_id = ?")
    .bind(source_id)
    .execute(&mut *tx)
    .await?;
```

- [ ] **Step 9: Register new modules in repos/mod.rs**

Add `pub mod book_tree;` and `pub mod gt_link;` to `crates/cognitive/src/repos/mod.rs`. Add re-exports for `SqliteBookTreeRepo` and `SqliteGTLinkRepo`.

- [ ] **Step 10: Verify all tests pass**

Run: `cargo nextest run -p cognitive -v 2>&1 | tail -10`
Expected: all existing + new tests PASS

- [ ] **Step 11: Commit**

```bash
git add crates/cognitive/src/repos/book_tree.rs crates/cognitive/src/repos/gt_link.rs \
  crates/cognitive/src/repos/mod.rs crates/cognitive/src/repos/entity.rs
git commit -m "feat(bookrag): add SqliteBookTreeRepo, SqliteGTLinkRepo, GT-Link merge safety"
```

---

## Task 4: Gradient-based Entity Resolution (Algorithm 1)

**Files:**
- Create: `crates/context_engine/src/book_index/entity_resolution.rs`
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write tests for detect_gradient**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_scores() {
        assert_eq!(detect_gradient(&[], 0.6), None);
    }

    #[test]
    fn single_score() {
        assert_eq!(detect_gradient(&[0.9], 0.6), None);
    }

    #[test]
    fn uniform_low_scores_no_gradient() {
        // All similar → Case A: new entity
        assert_eq!(detect_gradient(&[0.3, 0.28, 0.26, 0.25], 0.6), None);
    }

    #[test]
    fn clear_single_match() {
        // One high, then sharp drop → Case B, i=1
        assert_eq!(detect_gradient(&[0.95, 0.3, 0.28, 0.25], 0.6), Some(1));
    }

    #[test]
    fn multiple_matches_then_drop() {
        // Two high, then drop → Case B, i=2
        assert_eq!(detect_gradient(&[0.95, 0.90, 0.3, 0.28], 0.6), Some(2));
    }

    #[test]
    fn gradual_decline_no_gradient() {
        // Gentle slope, no sharp drop
        assert_eq!(detect_gradient(&[0.9, 0.85, 0.80, 0.76], 0.6), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context-engine -E 'test(detect_gradient)' -v 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement detect_gradient**

```rust
// crates/context_engine/src/book_index/entity_resolution.rs

/// Detect the gradient drop point in a descending-sorted score list.
/// Returns Some(i) where the sharp drop begins, or None if no gradient found.
pub fn detect_gradient(scores: &[f64], g: f64) -> Option<usize> {
    if scores.len() <= 1 {
        return None;
    }
    let mut prev = scores[0];
    for (i, &score) in scores.iter().enumerate().skip(1) {
        if score < prev / g {
            return Some(i);
        }
        prev = score;
    }
    None
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p context-engine -E 'test(detect_gradient)' -v`
Expected: all 6 PASS

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/book_index/entity_resolution.rs
git commit -m "feat(bookrag): add gradient-based entity resolution (detect_gradient)"
```

---

## Task 5: Operator Library — Traits + Context + Pure Operators

**Files:**
- Create: `crates/context_engine/src/operators/mod.rs`
- Create: `crates/context_engine/src/operators/reasoner.rs`
- Create: `crates/context_engine/src/operators/selector.rs`
- Create: `crates/context_engine/src/operators/formulator.rs`
- Create: `crates/context_engine/src/operators/synthesizer.rs`
- Modify: `crates/context_engine/src/lib.rs`

- [ ] **Step 1: Write tests for PageRank and Skyline**

In `crates/context_engine/src/operators/reasoner.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagerank_single_node() {
        let scores = pagerank_scores(
            &["a".into()],
            &[],       // no edges
            &["a".into()],
            0.85, 20,
        );
        assert!((scores["a"] - 1.0).abs() < 0.01);
    }

    #[test]
    fn pagerank_seeded() {
        // a -> b -> c, seed on a
        let scores = pagerank_scores(
            &["a".into(), "b".into(), "c".into()],
            &[("a".into(), "b".into(), 1.0), ("b".into(), "c".into(), 1.0)],
            &["a".into()],
            0.85, 20,
        );
        assert!(scores["a"] > scores["b"]);
        assert!(scores["b"] > scores["c"]);
    }

    #[test]
    fn skyline_basic() {
        let nodes = vec![
            scored_node("a", 0.9, 0.1),  // best graph
            scored_node("b", 0.1, 0.9),  // best text
            scored_node("c", 0.5, 0.5),  // dominated by neither
            scored_node("d", 0.1, 0.1),  // dominated by all
        ];
        let frontier = skyline_filter(&nodes);
        let ids: Vec<&str> = frontier.iter().map(|n| n.node.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
        assert!(!ids.contains(&"d"));
    }

    fn scored_node(id: &str, gs: f64, ts: f64) -> ScoredNode {
        ScoredNode {
            node: TreeNode {
                id: id.into(), parent_id: None,
                node_type: TreeNodeType::Text, content: String::new(),
                title: None, level: 0, source_type: SourceType::Note,
                source_id: String::new(), position: 0, metadata: None,
            },
            graph_score: gs, text_score: ts, combined: 0.0,
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context-engine -E 'test(pagerank|skyline)' -v 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement mod.rs — Operator trait + OperatorContext + OperatorLlm**

Write `crates/context_engine/src/operators/mod.rs` with the `Operator` trait, `OperatorType` enum, `OperatorLlm` trait, `OperatorContext` struct, and pipeline executor:

```rust
pub async fn execute_pipeline(
    operators: &[Box<dyn Operator>],
    ctx: &mut OperatorContext,
) -> Result<()> {
    for op in operators {
        match tokio::time::timeout(ctx.operator_timeout, op.execute(ctx)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                tracing::warn!("Operator '{}' failed: {e}", op.name());
                break;
            }
            Err(_) => {
                tracing::warn!("Operator '{}' timed out", op.name());
                break;
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Implement reasoner.rs — PageRank + Skyline + TextRanker**

Implement `pagerank_scores()` (iterative personalized PageRank), `skyline_filter()` (Pareto frontier), `GraphReasoning`, `TextRanker`, `SkylineRanker` operator structs.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p context-engine -E 'test(pagerank|skyline)' -v`
Expected: all PASS

- [ ] **Step 6: Implement selector.rs — FilterModal, FilterRange, SelectByEntity, SelectBySection**

These are simpler operators that filter `ctx.working_set` based on node type, source range, or GT-Link navigation.

- [ ] **Step 7: Implement formulator.rs — Decompose, Extract**

Both use `ctx.llm` for LLM calls. `Extract` also looks up entities in `ctx.book_index.entity_repo()`.

- [ ] **Step 8: Implement synthesizer.rs — Map, Reduce, SubQueryExecutor**

`Map` runs parallel LLM calls (capped at `ctx.max_map_nodes`). `Reduce` aggregates partials. `SubQueryExecutor` runs a sub-pipeline per sub-query via `tokio::join_all`.

- [ ] **Step 9: Register operators module**

Add `pub mod operators;` to `crates/context_engine/src/lib.rs`.

- [ ] **Step 10: Verify compilation + tests**

Run: `cargo nextest run -p context-engine -v 2>&1 | tail -10`
Expected: all PASS

- [ ] **Step 11: Commit**

```bash
git add crates/context_engine/src/operators/ crates/context_engine/src/lib.rs
git commit -m "feat(bookrag): add operator library — PageRank, Skyline, selectors, formulators, synthesizers"
```

---

## Task 6: Retrieval Planner + BookRAGSearcher

**Files:**
- Create: `crates/context_engine/src/retrieval_planner/mod.rs`
- Create: `crates/context_engine/src/retrieval_planner/classifier.rs`
- Create: `crates/context_engine/src/insight_forge/bookrag_searcher.rs`
- Modify: `crates/context_engine/src/insight_forge/mod.rs:1-9`
- Modify: `crates/context_engine/src/lib.rs`

- [ ] **Step 1: Write tests for query classification**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_single_hop() {
        assert_eq!(classify_heuristic("What is the deadline for Project Alpha?"), QueryCategory::SingleHop);
    }

    #[test]
    fn classify_multi_hop() {
        assert_eq!(classify_heuristic("How does my finance goal relate to work projects?"), QueryCategory::MultiHop);
    }

    #[test]
    fn classify_global() {
        assert_eq!(classify_heuristic("How many tasks are overdue across all projects?"), QueryCategory::GlobalAggregation);
    }

    #[test]
    fn classify_passthrough() {
        assert_eq!(classify_heuristic("hello"), QueryCategory::PassThrough);
        assert_eq!(classify_heuristic("hi there"), QueryCategory::PassThrough);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context-engine -E 'test(classify_)' -v 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement classifier.rs**

```rust
// crates/context_engine/src/retrieval_planner/classifier.rs

#[derive(Debug, Clone, PartialEq)]
pub enum QueryCategory {
    SingleHop,
    MultiHop,
    GlobalAggregation,
    PassThrough,
}

pub fn classify_heuristic(query: &str) -> QueryCategory {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();

    if words.len() <= 2 {
        return QueryCategory::PassThrough;
    }

    let global_keywords = ["how many", "count", "total", "list all", "sum of", "across all"];
    if global_keywords.iter().any(|kw| q.contains(kw)) {
        return QueryCategory::GlobalAggregation;
    }

    let multi_hop_keywords = ["compare", "differ", "relate", "between", "how does", "affect", "versus", "vs"];
    if multi_hop_keywords.iter().any(|kw| q.contains(kw)) {
        return QueryCategory::MultiHop;
    }

    QueryCategory::SingleHop
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p context-engine -E 'test(classify_)' -v`
Expected: all PASS

- [ ] **Step 5: Implement RetrievalPlanner (mod.rs)**

Write `crates/context_engine/src/retrieval_planner/mod.rs`: `RetrievalPlanner` with `plan()` method that calls `classify_heuristic`, then `generate_plan()` which returns operator vectors per category.

- [ ] **Step 6: Implement BookRAGSearcher**

Write `crates/context_engine/src/insight_forge/bookrag_searcher.rs`: `BookRAGSearcher` implementing `DomainSearcher`, wrapping `RetrievalPlanner`, with per-operator timeout and error handling as specified in the design.

- [ ] **Step 7: Register modules**

Add `pub mod retrieval_planner;` to `crates/context_engine/src/lib.rs`.
Add `pub mod bookrag_searcher;` to `crates/context_engine/src/insight_forge/mod.rs`.

- [ ] **Step 8: Verify compilation**

Run: `cargo check -p context-engine 2>&1 | tail -10`
Expected: success

- [ ] **Step 9: Commit**

```bash
git add crates/context_engine/src/retrieval_planner/ \
  crates/context_engine/src/insight_forge/bookrag_searcher.rs \
  crates/context_engine/src/insight_forge/mod.rs \
  crates/context_engine/src/lib.rs
git commit -m "feat(bookrag): add RetrievalPlanner + BookRAGSearcher (DomainSearcher)"
```

---

## Task 7: DomainEvent Extensions + Config

**Files:**
- Modify: `crates/bus/src/domain_events.rs:208-215`
- Modify: `crates/config/src/schema/cognitive.rs:7-107`

- [ ] **Step 1: Add new DomainEvent variants**

In `crates/bus/src/domain_events.rs`, after the existing note events (~line 215), add:

```rust
NoteContentChanged {
    note_id: String,
    content: String,
},
NoteDeleted {
    note_id: String,
},
TaskHierarchyChanged {
    project_id: String,
},
```

- [ ] **Step 2: Add BookIndexConfig to CognitiveConfig**

In `crates/config/src/schema/cognitive.rs`, add the struct and a field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookIndexConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub entity_resolution: EntityResolutionConfigSerde,
    #[serde(default)]
    pub retrieval: BookRetrievalConfigSerde,
}

// ... with Default impl and sub-structs for serde
```

Add `pub book_index: BookIndexConfig` to `CognitiveConfig` with `#[serde(default)]`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace 2>&1 | tail -10`
Expected: success (may have warnings from unused new event variants — that's fine)

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/config/src/schema/cognitive.rs
git commit -m "feat(bookrag): extend DomainEvent + add BookIndexConfig"
```

---

## Task 8: Agent Wiring — Connect Everything

**Files:**
- Create: `crates/agent/src/adapters/book_index_wiring.rs`
- Modify: `crates/agent/src/adapters/mod.rs:1-18`
- Modify: `crates/agent/src/agent_loop/builder.rs:689-691`

- [ ] **Step 1: Implement BookEmbedder adapter**

In `crates/agent/src/adapters/book_index_wiring.rs`:

```rust
use context_engine::book_index::BookEmbedder;
use tools::embedding::EmbeddingEngine;
// ...

pub struct BookEmbedderAdapter {
    engine: Arc<EmbeddingEngine>,
}

#[async_trait]
impl BookEmbedder for BookEmbedderAdapter {
    async fn embed(&self, text: &str) -> common::Result<Vec<f32>> {
        self.engine.embed(text).await
    }
}
```

- [ ] **Step 2: Implement BookEntityRepo adapter**

```rust
pub struct BookEntityRepoAdapter {
    entity_repo: cognitive::EntityRepo,
}

#[async_trait]
impl BookEntityRepo for BookEntityRepoAdapter {
    async fn find_by_name(&self, query: &str) -> common::Result<Vec<EntityInfo>> {
        let rows = self.entity_repo.find_by_name(query).await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|r| EntityInfo {
            id: r.id, name: r.name, entity_type: r.entity_type,
        }).collect())
    }
    // ... get_neighborhood_ids delegates to entity_repo.get_neighborhood
}
```

- [ ] **Step 3: Implement OperatorLlm adapter**

```rust
pub struct OperatorLlmAdapter {
    provider: providers::DynProvider,
}

#[async_trait]
impl OperatorLlm for OperatorLlmAdapter {
    async fn complete(&self, system: &str, prompt: &str) -> common::Result<String> {
        // Build messages, call provider.complete(), extract text
    }
}
```

- [ ] **Step 4: Write build_book_index + build_bookrag_searcher functions**

```rust
pub fn build_book_index(
    tree_repo: Arc<dyn BookTreeRepo>,
    entity_repo: cognitive::EntityRepo,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    engine: Arc<EmbeddingEngine>,
) -> Arc<BookIndex> {
    Arc::new(BookIndex::new(
        tree_repo,
        Arc::new(BookEntityRepoAdapter { entity_repo }),
        gt_link_repo,
        Arc::new(BookEmbedderAdapter { engine }),
    ))
}

pub fn build_bookrag_searcher(
    book_index: Arc<BookIndex>,
    provider: providers::DynProvider,
    config: &BookRetrievalConfig,
) -> Arc<BookRAGSearcher> {
    let llm = Arc::new(OperatorLlmAdapter { provider });
    let planner = Arc::new(RetrievalPlanner::new(book_index, llm, config.clone()));
    Arc::new(BookRAGSearcher::new(planner))
}
```

- [ ] **Step 5: Wire into builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, after the existing `forge.add_searcher` calls (~line 691):

```rust
// BookRAG integration
if config.cognitive.book_index.enabled {
    let tree_repo = Arc::new(SqliteBookTreeRepo::new(pool.inner().clone()));
    let gt_link_repo = Arc::new(SqliteGTLinkRepo::new(pool.inner().clone()));
    let book_index = book_index_wiring::build_book_index(
        tree_repo, entity_repo.clone(), gt_link_repo, embedding_engine.clone(),
    );
    let bookrag_searcher = book_index_wiring::build_bookrag_searcher(
        book_index, provider.clone(), &config.cognitive.book_index.retrieval.into(),
    );
    forge.add_searcher(bookrag_searcher);
}
```

- [ ] **Step 6: Register module**

Add `pub mod book_index_wiring;` to `crates/agent/src/adapters/mod.rs`.

- [ ] **Step 7: Verify full workspace compiles**

Run: `cargo check --workspace 2>&1 | tail -10`
Expected: success

- [ ] **Step 8: Run existing tests to verify no regression**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: all existing tests still PASS

- [ ] **Step 9: Commit**

```bash
git add crates/agent/src/adapters/book_index_wiring.rs \
  crates/agent/src/adapters/mod.rs \
  crates/agent/src/agent_loop/builder.rs
git commit -m "feat(bookrag): wire BookIndex + BookRAGSearcher into agent pipeline"
```

---

## Task 9: Markdown Tree Builder

**Files:**
- Add parsing logic to: `crates/cognitive/src/repos/book_tree.rs` (or a dedicated `parser.rs`)
- Test: inline

- [ ] **Step 1: Write tests for markdown → tree conversion**

```rust
#[test]
fn parse_simple_markdown() {
    let md = "# Chapter 1\nSome text.\n## Section 1.1\nMore text.\n## Section 1.2\nFinal.";
    let nodes = parse_markdown_to_tree("note-1", md);
    assert_eq!(nodes.len(), 5); // 2 sections + 3 text blocks
    assert_eq!(nodes[0].node_type.as_str(), "Section");
    assert_eq!(nodes[0].level, 1);
    assert_eq!(nodes[1].node_type.as_str(), "Text");
    assert_eq!(nodes[1].parent_id, Some(nodes[0].id.clone()));
}

#[test]
fn parse_code_blocks() {
    let md = "# Title\n```rust\nfn main() {}\n```\nAfter code.";
    let nodes = parse_markdown_to_tree("note-1", md);
    let code_nodes: Vec<_> = nodes.iter().filter(|n| matches!(n.node_type, TreeNodeType::Code)).collect();
    assert_eq!(code_nodes.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify failure**

- [ ] **Step 3: Implement parse_markdown_to_tree**

Line-by-line parser: detect heading lines (`^#{1,6}\s`), code fences (` ``` `), and treat everything else as Text. Track heading stack for parent assignment. Generate UUID for each node.

- [ ] **Step 4: Run tests**

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(bookrag): add markdown-to-tree parser"
```

---

## Task 10: Integration Test — End-to-End SingleHop Query

**Files:**
- Create: `crates/context_engine/src/book_index/tests.rs` (or inline in mod.rs)

- [ ] **Step 1: Write integration test**

Test that creates a BookIndex with mock repos, inserts a note tree with entities + GT-Links, runs a SingleHop query through BookRAGSearcher, and verifies relevant nodes are returned.

Use mock implementations of `BookTreeRepo`, `GTLinkRepo`, `BookEntityRepo`, `BookEmbedder`, and `OperatorLlm` that return canned data.

- [ ] **Step 2: Run test**

Run: `cargo nextest run -p context-engine -E 'test(single_hop_integration)' -v`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git commit -m "test(bookrag): add end-to-end SingleHop integration test"
```

---

## Task 11: Clippy + Format + Final Verification

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep warning | head -20`
Fix any new warnings.

- [ ] **Step 2: Run fmt**

Run: `cargo fmt --all --check`
Fix any formatting issues.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: all PASS

- [ ] **Step 4: Final commit if needed**

```bash
git commit -m "chore: fix clippy warnings and formatting for bookrag"
```
