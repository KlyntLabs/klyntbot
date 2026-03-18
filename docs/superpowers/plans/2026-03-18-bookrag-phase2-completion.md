# BookRAG Phase 2 Completion Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the remaining ~22% of the BookRAG spec — entity extraction + GT-Link population during tree builds, task/skill tree construction, rebuild_all backfill, caching, LLM fallback classifier, and remaining test scenarios.

**Architecture:** Extends the existing BookRAG infrastructure (Phase 1) with three focus areas: (1) entity extraction pipeline that runs after tree node insertion in BookIndexUpdater, extracting entities via LLM and creating GT-Links, (2) task and skill tree builders that convert project hierarchies and compiled skill content into tree nodes, (3) operational polish — caching, backfill, LLM classifier fallback.

**Tech Stack:** Rust, SQLite, async_trait, tokio, serde_json

**Spec:** `docs/superpowers/specs/2026-03-17-bookrag-architecture-design.md`
**Phase 1 plan:** `docs/superpowers/plans/2026-03-17-bookrag-implementation.md`

---

## File Structure

### New files to create

```
crates/agent/src/adapters/
    book_index_entity_extractor.rs  -- Entity extraction from tree nodes + GT-Link creation
    book_index_task_builder.rs      -- Task hierarchy → tree node conversion
    book_index_skill_builder.rs     -- Skill content → tree node conversion
    book_index_backfill.rs          -- One-time backfill of existing notes/tasks/skills
```

### Existing files to modify

```
crates/agent/src/adapters/mod.rs:1-4                   -- Add new module declarations
crates/agent/src/adapters/book_index_updater.rs:67-105  -- Wire entity extraction after tree insert, handle task events
crates/agent/src/adapters/book_index_wiring.rs:108-145  -- Add entity extractor construction, pass to updater
crates/agent/src/agent_loop/builder.rs:693-727          -- Wire backfill, pass task/note repos to updater
crates/context_engine/src/retrieval_planner/classifier.rs:9-37  -- Add LLM fallback after heuristic
crates/context_engine/src/retrieval_planner/mod.rs:42-53        -- Pass LLM to classifier when heuristic returns SingleHop for long queries
crates/context_engine/src/book_index/mod.rs:43-91       -- Add cached tree query methods with TtlCache
crates/context_engine/src/book_index/tests.rs            -- Add MultiHop + Global integration tests
```

---

## Task 1: Entity Extraction from Tree Nodes

Extract entities from tree node content after tree insertion in BookIndexUpdater. For each leaf node (Text, Code), call the LLM to extract entity names, upsert them via EntityRepo, and create GT-Links.

**Files:**
- Create: `crates/agent/src/adapters/book_index_entity_extractor.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/adapters/book_index_updater.rs`
- Modify: `crates/agent/src/adapters/book_index_wiring.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Write the entity extractor module**

Create `crates/agent/src/adapters/book_index_entity_extractor.rs`:

```rust
use std::sync::Arc;

use context_engine::book_index::types::{TreeNode, TreeNodeType};
use context_engine::book_index::GTLinkRepo;
use cognitive::repos::{EntityRepo, NewEntity};
use context_engine::operators::OperatorLlm;
use tracing::{debug, warn};

/// Extracts entities from tree nodes and creates GT-Links.
/// Runs in background after tree insertion — does NOT block note save.
pub struct BookIndexEntityExtractor {
    entity_repo: EntityRepo,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    llm: Arc<dyn OperatorLlm>,
}

impl BookIndexEntityExtractor {
    pub fn new(
        entity_repo: EntityRepo,
        gt_link_repo: Arc<dyn GTLinkRepo>,
        llm: Arc<dyn OperatorLlm>,
    ) -> Self {
        Self {
            entity_repo,
            gt_link_repo,
            llm,
        }
    }

    /// Extract entities from all leaf nodes and create GT-Links.
    /// Call this after tree nodes have been inserted.
    pub async fn extract_and_link(&self, nodes: &[TreeNode]) -> common::Result<usize> {
        let leaf_nodes: Vec<&TreeNode> = nodes
            .iter()
            .filter(|n| matches!(n.node_type, TreeNodeType::Text | TreeNodeType::Code | TreeNodeType::Task))
            .filter(|n| n.content.len() > 20) // Skip trivially short content
            .collect();

        if leaf_nodes.is_empty() {
            return Ok(0);
        }

        let mut total_links = 0;

        for node in &leaf_nodes {
            match self.extract_entities_from_node(node).await {
                Ok(count) => total_links += count,
                Err(e) => {
                    warn!("Entity extraction failed for node {}: {e}", node.id);
                    // Continue with other nodes — don't fail entire batch
                }
            }
        }

        Ok(total_links)
    }

    async fn extract_entities_from_node(&self, node: &TreeNode) -> common::Result<usize> {
        let prompt = format!(
            "Extract named entities from this text. Return one entity per line in format: NAME|TYPE\n\
             Types: Person, Project, Tool, Concept, Organization, Location\n\
             Only extract proper nouns and specific named things. Skip generic words.\n\n\
             Text:\n{}",
            &node.content[..node.content.len().min(500)]
        );

        let response = self
            .llm
            .complete(
                "You extract named entities. Return NAME|TYPE per line, nothing else.",
                &prompt,
            )
            .await?;

        let mut links = Vec::new();

        for line in response.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() != 2 {
                continue;
            }
            let name = parts[0].trim();
            let entity_type = parts[1].trim();
            if name.is_empty() || entity_type.is_empty() || name.len() < 2 {
                continue;
            }

            // Upsert the entity (increments mention_count if exists)
            // Note: upsert_entity returns Result<EntityRow, sqlx::Error> — convert error
            match self
                .entity_repo
                .upsert_entity(&NewEntity {
                    name: name.to_string(),
                    entity_type: entity_type.to_string(),
                    description: None,
                    source: "bookindex".to_string(),
                    source_id: Some(node.source_id.clone()),
                    metadata: None,
                })
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
            {
                Ok(entity) => {
                    links.push((entity.id, node.id.clone()));
                }
                Err(e) => {
                    debug!("Entity upsert failed for '{name}': {e}");
                }
            }
        }

        if !links.is_empty() {
            self.gt_link_repo.link_batch(&links).await?;
        }

        Ok(links.len())
    }
}
```

- [ ] **Step 2: Register module**

In `crates/agent/src/adapters/mod.rs`, add:

```rust
pub mod book_index_entity_extractor;
```

- [ ] **Step 3: Wire entity extractor into BookIndexUpdater**

In `crates/agent/src/adapters/book_index_updater.rs`, update the `start()` signature and `handle_event()` to accept an optional `BookIndexEntityExtractor`. After tree nodes are inserted, spawn entity extraction in background:

Change `handle_event` signature to accept an `Option<&BookIndexEntityExtractor>`.

After the `tree_repo.insert_nodes(&nodes).await?;` call in the `NoteContentChanged` handler, add:

```rust
if let Some(ref extractor) = entity_extractor {
    let extractor = extractor.clone();
    let nodes_for_extraction = nodes.clone();
    tokio::spawn(async move {
        match extractor.extract_and_link(&nodes_for_extraction).await {
            Ok(n) => {
                if n > 0 {
                    debug!("BookIndex: linked {n} entities for note {note_id}");
                }
            }
            Err(e) => warn!("BookIndex entity extraction failed: {e}"),
        }
    });
}
```

**Important:** The `BookIndexEntityExtractor` needs to be wrapped in `Arc` so it can be cloned into the spawned task. Update the struct field in BookIndexUpdater to `Option<Arc<BookIndexEntityExtractor>>`.

- [ ] **Step 4: Update book_index_wiring to build the extractor**

In `crates/agent/src/adapters/book_index_wiring.rs`, add a builder function:

```rust
pub fn build_entity_extractor(
    entity_repo: cognitive::repos::EntityRepo,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    provider: providers::DynProvider,
    config: &config::Config,
) -> Arc<BookIndexEntityExtractor> {
    let params = providers::cognitive_chat_params(config, 256);
    let llm: Arc<dyn OperatorLlm> = Arc::new(OperatorLlmAdapter { provider, params });
    Arc::new(BookIndexEntityExtractor::new(entity_repo, gt_link_repo, llm))
}
```

- [ ] **Step 5: Wire in builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, within the `if config.cognitive.book_index.enabled` block, construct the entity extractor and pass to BookIndexUpdater:

```rust
let entity_extractor = crate::adapters::book_index_wiring::build_entity_extractor(
    cognitive::repos::EntityRepo::new(storage_pool.inner().clone()),
    gt_link_repo.clone(),
    provider.clone(),
);
```

Pass `Some(entity_extractor)` to `BookIndexUpdater::start()`.

- [ ] **Step 6: Verify compilation**

Run: `cargo check --workspace 2>&1 | tail -5`
Expected: success

- [ ] **Step 7: Test entity extraction manually**

Run: `cargo nextest run -p agent -E 'test(book_index)' -v 2>&1 | tail -10`
Then manually verify via SQLite:
```bash
sqlite3 ~/.klyntbot-dev/data.db "SELECT COUNT(*) FROM entity_tree_links;"
```

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/adapters/book_index_entity_extractor.rs \
  crates/agent/src/adapters/mod.rs \
  crates/agent/src/adapters/book_index_updater.rs \
  crates/agent/src/adapters/book_index_wiring.rs \
  crates/agent/src/agent_loop/builder.rs
git commit -m "feat(bookrag): add entity extraction + GT-Link population during tree build"
```

---

## Task 2: Task Tree Builder

Build tree nodes from task hierarchies: Area (level 0) → Project (level 1) → Task (level 2) → Subtask (level 3). The task description becomes a Text child node.

**Files:**
- Create: `crates/agent/src/adapters/book_index_task_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/adapters/book_index_updater.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Write test for task tree building**

In `crates/agent/src/adapters/book_index_task_builder.rs`, add the builder and inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_project_tree() {
        let tasks = vec![
            mock_task("t1", "Build API", None, Some("proj-1"), Some("Design the REST API")),
            mock_task("t2", "Write tests", Some("t1"), Some("proj-1"), Some("Unit + integration tests")),
            mock_task("t3", "Deploy", Some("t1"), Some("proj-1"), None),
        ];
        let nodes = build_task_tree("proj-1", "Project Alpha", &tasks);
        assert!(nodes.len() >= 4); // project section + 3 tasks (some with description children)
        assert_eq!(nodes[0].node_type.as_str(), "Section"); // Project root
        assert_eq!(nodes[0].title.as_deref(), Some("Project Alpha"));
        assert!(matches!(nodes[0].source_type, SourceType::Task));
    }

    fn mock_task(id: &str, title: &str, parent_id: Option<&str>, project_id: Option<&str>, desc: Option<&str>) -> TaskRow {
        // ... construct minimal TaskRow
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(build_project_tree)' -v 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement build_task_tree**

```rust
use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use storage::TaskRow;
use uuid::Uuid;

/// Build tree nodes from a project's task hierarchy.
///
/// Creates: Project Section (level 1) → Task nodes (level 2) → Subtask nodes (level 3).
/// Task descriptions become Text children.
pub fn build_task_tree(project_id: &str, project_name: &str, tasks: &[TaskRow]) -> Vec<TreeNode> {
    let mut nodes = Vec::new();

    // Root: project section
    let project_node_id = Uuid::new_v4().to_string();
    nodes.push(TreeNode {
        id: project_node_id.clone(),
        parent_id: None,
        node_type: TreeNodeType::Section,
        content: project_name.to_string(),
        title: Some(project_name.to_string()),
        level: 1,
        source_type: SourceType::Task,
        source_id: project_id.to_string(),
        position: 0,
        metadata: None,
    });

    // Build ID → node_id mapping for parent references
    let mut task_node_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // Root tasks (no parent_id or parent is outside this project)
    let root_tasks: Vec<&TaskRow> = tasks
        .iter()
        .filter(|t| t.parent_id.is_none() || !tasks.iter().any(|other| Some(&other.id) == t.parent_id.as_ref()))
        .collect();

    let mut position: u32 = 1;
    for task in &root_tasks {
        add_task_node(&mut nodes, &mut task_node_ids, task, &project_node_id, 2, &mut position, project_id, tasks);
    }

    nodes
}

fn add_task_node(
    nodes: &mut Vec<TreeNode>,
    task_node_ids: &mut std::collections::HashMap<String, String>,
    task: &TaskRow,
    parent_node_id: &str,
    level: u32,
    position: &mut u32,
    project_id: &str,
    all_tasks: &[TaskRow],
) {
    let node_id = Uuid::new_v4().to_string();
    task_node_ids.insert(task.id.clone(), node_id.clone());

    nodes.push(TreeNode {
        id: node_id.clone(),
        parent_id: Some(parent_node_id.to_string()),
        node_type: TreeNodeType::Task,
        content: task.title.clone(),
        title: Some(task.title.clone()),
        level,
        source_type: SourceType::Task,
        source_id: project_id.to_string(),
        position: *position,
        metadata: None,
    });
    *position += 1;

    // Add description as Text child if present
    if let Some(ref desc) = task.description {
        if !desc.is_empty() {
            nodes.push(TreeNode {
                id: Uuid::new_v4().to_string(),
                parent_id: Some(node_id.clone()),
                node_type: TreeNodeType::Text,
                content: desc.clone(),
                title: None,
                level: level + 1,
                source_type: SourceType::Task,
                source_id: project_id.to_string(),
                position: *position,
                metadata: None,
            });
            *position += 1;
        }
    }

    // Recurse into children
    let children: Vec<&TaskRow> = all_tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == Some(&task.id))
        .collect();
    for child in children {
        add_task_node(nodes, task_node_ids, child, &node_id, level + 1, position, project_id, all_tasks);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(build_project_tree)' -v`
Expected: PASS

- [ ] **Step 5: Wire task tree building into BookIndexUpdater**

In `crates/agent/src/adapters/book_index_updater.rs`, update the `TaskHierarchyChanged` handler. The updater needs access to `TaskRepo` and `ProjectRepo` (or just query tasks by project_id). Add `task_repo: Option<storage::TaskRepo>` to the updater's start params.

In the `TaskHierarchyChanged` handler:

```rust
bus::DomainEvent::TaskHierarchyChanged { project_id } => {
    debug!("BookIndex: rebuilding task tree for project {project_id}");
    if let Some(ref task_repo) = task_repo {
        // Delete existing task tree for this project
        tree_repo.delete_by_source(&SourceType::Task, &project_id).await?;

        // Get project name (StorageError → KlyntbotError conversion)
        let project_name = if let Some(ref project_repo) = project_repo {
            project_repo.get(&project_id).await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
                .ok()
                .flatten()
                .map(|p| p.name)
                .unwrap_or_else(|| project_id.clone())
        } else {
            project_id.clone()
        };

        // Get all tasks for this project (StorageError → KlyntbotError conversion)
        let tasks = task_repo.list(&storage::TaskFilter {
            project_id: Some(project_id.clone()),
            ..Default::default()
        }).await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
            .unwrap_or_default();

        if !tasks.is_empty() {
            let nodes = crate::adapters::book_index_task_builder::build_task_tree(
                &project_id, &project_name, &tasks,
            );
            tree_repo.insert_nodes(&nodes).await?;
            book_index.refresh_has_content().await?;
            debug!("BookIndex: inserted {} task tree nodes for project {project_id}", nodes.len());
        }
    }
}
```

- [ ] **Step 6: Pass TaskRepo and ProjectRepo to BookIndexUpdater in builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, pass the repos:

```rust
let task_repo_for_updater = storage::TaskRepo::new(storage_pool.inner().clone());
let project_repo_for_updater = storage::ProjectRepo::new(storage_pool.inner().clone());
```

Pass them to `BookIndexUpdater::start()`.

- [ ] **Step 7: Verify compilation**

Run: `cargo check --workspace 2>&1 | tail -5`

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/adapters/book_index_task_builder.rs \
  crates/agent/src/adapters/mod.rs \
  crates/agent/src/adapters/book_index_updater.rs \
  crates/agent/src/agent_loop/builder.rs
git commit -m "feat(bookrag): add task hierarchy tree builder"
```

---

## Task 3: Skill Tree Builder (Static, Boot-time)

Build tree nodes from compiled skill YAML content at startup. Uses SHA-256 checksum to avoid unnecessary rebuilds.

**Files:**
- Create: `crates/agent/src/adapters/book_index_skill_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Write test for skill parsing**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skill_to_tree() {
        let content = "---\nname: test-skill\n---\n# Test Skill\n\n## Overview\nA test skill.\n\n## Instructions\nDo this thing.";
        let nodes = build_skill_tree("test-skill", content);
        assert!(nodes.len() >= 3); // root + 2 sections + text
        assert_eq!(nodes[0].source_type.as_str(), "Skill");
    }
}
```

- [ ] **Step 2: Add sha2 to agent Cargo.toml**

In `crates/agent/Cargo.toml`, add: `sha2 = { workspace = true }`

- [ ] **Step 3: Implement skill tree builder**

```rust
use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Build tree nodes from a skill's SKILL.md content.
/// Reuses the same heading-based parser as notes but with SourceType::Skill.
pub fn build_skill_tree(skill_name: &str, content: &str) -> Vec<TreeNode> {
    // Strip YAML frontmatter
    let body = strip_frontmatter(content);
    // Reuse markdown parser with SourceType::Skill
    let mut nodes = cognitive::repos::parse_markdown_to_tree(skill_name, &body);
    // Override source_type to Skill
    for node in &mut nodes {
        node.source_type = SourceType::Skill;
    }
    nodes
}

/// Build all skill trees from compiled skill content.
/// Returns the SHA-256 checksum of all content for change detection.
pub async fn build_all_skill_trees(
    tree_repo: &dyn context_engine::book_index::BookTreeRepo,
) -> common::Result<String> {
    let mut hasher = Sha256::new();

    for (name, content) in skill_system::BUILTIN_SKILLS {
        hasher.update(content.as_bytes());

        // Delete old tree for this skill
        tree_repo.delete_by_source(&SourceType::Skill, name).await?;

        let nodes = build_skill_tree(name, content);
        if !nodes.is_empty() {
            tree_repo.insert_nodes(&nodes).await?;
            tracing::debug!("BookIndex: built {} tree nodes for skill '{name}'", nodes.len());
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn strip_frontmatter(content: &str) -> &str {
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            return content[end + 6..].trim_start();
        }
    }
    content
}
```

- [ ] **Step 4: Wire into builder.rs at startup**

In `crates/agent/src/agent_loop/builder.rs`, after the BookIndexUpdater start, add a one-time skill tree build:

```rust
// Build skill trees at startup (non-blocking)
let tree_repo_for_skills = tree_repo.clone();
tokio::spawn(async move {
    match crate::adapters::book_index_skill_builder::build_all_skill_trees(
        tree_repo_for_skills.as_ref(),
    ).await {
        Ok(checksum) => tracing::info!("BookIndex: skill trees built (checksum: {checksum})"),
        Err(e) => tracing::warn!("BookIndex: skill tree build failed: {e}"),
    }
});
```

- [ ] **Step 5: Verify compilation + run tests**

Run: `cargo check --workspace 2>&1 | tail -5`
Run: `cargo nextest run -p agent -E 'test(skill)' -v`

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/adapters/book_index_skill_builder.rs \
  crates/agent/src/adapters/mod.rs \
  crates/agent/src/agent_loop/builder.rs
git commit -m "feat(bookrag): add skill tree builder at boot with SHA-256 checksum"
```

---

## Task 4: Rebuild All (Backfill Existing Notes)

One-time backfill that indexes all existing notes that don't have tree nodes yet. Runs at startup, non-blocking.

**Files:**
- Create: `crates/agent/src/adapters/book_index_backfill.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Implement backfill**

```rust
use std::sync::Arc;

use context_engine::book_index::{BookIndex, BookTreeRepo};
use context_engine::book_index::types::SourceType;
use cognitive::repos::parse_markdown_to_tree;
use tracing::{debug, info, warn};

/// Backfill tree nodes for all existing notes that don't have trees yet.
/// Runs once at startup, non-blocking.
pub async fn backfill_existing_notes(
    note_repo: &feature_notes::repo::NoteRepo,
    tree_repo: &dyn BookTreeRepo,
    book_index: &BookIndex,
    entity_extractor: Option<&Arc<crate::adapters::book_index_entity_extractor::BookIndexEntityExtractor>>,
) -> common::Result<u32> {
    // Get all notes
    let notes = note_repo.list_notes(None).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    let mut indexed = 0u32;
    for note in &notes {
        if note.body.is_empty() {
            continue;
        }

        // Delete-then-rebuild is idempotent — safe to run on already-indexed notes
        tree_repo.delete_by_source(&SourceType::Note, &note.id).await?;

        let nodes = parse_markdown_to_tree(&note.id, &note.body);
        if nodes.is_empty() {
            continue;
        }

        tree_repo.insert_nodes(&nodes).await?;
        indexed += 1;

        // Fire-and-forget entity extraction
        if let Some(extractor) = entity_extractor {
            let extractor = extractor.clone();
            let nodes_clone = nodes.clone();
            tokio::spawn(async move {
                if let Err(e) = extractor.extract_and_link(&nodes_clone).await {
                    debug!("Backfill entity extraction failed: {e}");
                }
            });
        }
    }

    if indexed > 0 {
        book_index.refresh_has_content().await?;
        info!("BookIndex: backfilled {indexed} notes into tree index");
    }

    Ok(indexed)
}
```

- [ ] **Step 2: Wire into builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, after skill tree build, add:

```rust
// Backfill existing notes (non-blocking)
let note_repo_for_backfill = feature_notes::repo::NoteRepo::new(storage_pool.inner().clone());
let tree_repo_for_backfill = tree_repo.clone();
let book_index_for_backfill = book_index.clone();
let extractor_for_backfill = entity_extractor.clone();
tokio::spawn(async move {
    match crate::adapters::book_index_backfill::backfill_existing_notes(
        &note_repo_for_backfill,
        tree_repo_for_backfill.as_ref(),
        &book_index_for_backfill,
        extractor_for_backfill.as_ref(),
    ).await {
        Ok(0) => {}
        Ok(n) => tracing::info!("BookIndex: backfilled {n} existing notes"),
        Err(e) => tracing::warn!("BookIndex backfill failed: {e}"),
    }
});
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace 2>&1 | tail -5`

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/adapters/book_index_backfill.rs \
  crates/agent/src/adapters/mod.rs \
  crates/agent/src/agent_loop/builder.rs
git commit -m "feat(bookrag): add one-time backfill for existing notes at startup"
```

---

## Task 5: LLM Fallback Classifier

When heuristic classification returns SingleHop for longer queries (6+ words), use LLM to refine the classification. Short queries and clear heuristic matches skip the LLM call.

**Files:**
- Modify: `crates/context_engine/src/retrieval_planner/classifier.rs`
- Modify: `crates/context_engine/src/retrieval_planner/mod.rs`
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write test for LLM classifier**

In `crates/context_engine/src/retrieval_planner/classifier.rs`:

```rust
#[tokio::test]
async fn llm_fallback_refines_ambiguous() {
    use async_trait::async_trait;

    struct MockLlm;
    #[async_trait]
    impl crate::operators::OperatorLlm for MockLlm {
        async fn complete(&self, _system: &str, _prompt: &str) -> common::Result<String> {
            Ok("MultiHop".to_string())
        }
    }

    let result = classify_with_llm_fallback(
        "What is the connection between my finance goals and the project timeline",
        &MockLlm,
    ).await;
    // Heuristic would say SingleHop, but LLM refines to MultiHop
    assert_eq!(result, QueryCategory::MultiHop);
}
```

- [ ] **Step 2: Implement classify_with_llm_fallback**

```rust
/// Classify with heuristic first, then LLM fallback for ambiguous long queries.
pub async fn classify_with_llm_fallback(
    query: &str,
    llm: &dyn crate::operators::OperatorLlm,
) -> QueryCategory {
    let heuristic_result = classify_heuristic(query);

    // Only use LLM fallback for ambiguous cases:
    // - Heuristic returned SingleHop (default/fallback)
    // - Query is long enough to be potentially complex (6+ words)
    let words = query.split_whitespace().count();
    if heuristic_result != QueryCategory::SingleHop || words < 6 {
        return heuristic_result;
    }

    // LLM refinement
    let prompt = format!(
        "Classify this query into exactly one category:\n\
         - SingleHop: simple lookup, one entity, direct answer\n\
         - MultiHop: requires connecting info across multiple topics\n\
         - GlobalAggregation: counting, listing all, summarizing across everything\n\
         - PassThrough: greeting, chitchat, not a knowledge question\n\n\
         Reply with ONLY the category name.\n\n\
         Query: \"{}\"",
        query
    );

    match llm.complete("You classify queries. Reply with one word: SingleHop, MultiHop, GlobalAggregation, or PassThrough.", &prompt).await {
        Ok(response) => {
            let trimmed = response.trim();
            match trimmed {
                "MultiHop" => QueryCategory::MultiHop,
                "GlobalAggregation" => QueryCategory::GlobalAggregation,
                "PassThrough" => QueryCategory::PassThrough,
                _ => QueryCategory::SingleHop, // Default to heuristic result
            }
        }
        Err(_) => heuristic_result, // LLM failed, use heuristic
    }
}
```

- [ ] **Step 3: Update RetrievalPlanner to use LLM fallback**

In `crates/context_engine/src/retrieval_planner/mod.rs`, change `plan()`:

```rust
pub async fn plan(&self, query: &str) -> Result<RetrievalPlan> {
    let category = classify_with_llm_fallback(query, self.llm.as_ref()).await;
    let operators = self.generate_plan(query, &category);
    Ok(RetrievalPlan { category, operators })
}
```

Update the **module-level** `pub use` at line 7 of `mod.rs`:

```rust
pub use classifier::{classify_heuristic, classify_with_llm_fallback, QueryCategory};
```

(Replace the existing `pub use classifier::{classify_heuristic, QueryCategory};` line.)

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p context_engine -E 'test(classify)' -v`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/retrieval_planner/classifier.rs \
  crates/context_engine/src/retrieval_planner/mod.rs
git commit -m "feat(bookrag): add LLM fallback classifier for ambiguous queries"
```

---

## Task 6: Caching — TTL Cache on Tree Queries

Add TTL caching to frequently called tree queries (get_root_sections, get_subtree) to avoid repeated SQLite calls during the same retrieval pipeline.

**Files:**
- Modify: `crates/context_engine/src/book_index/mod.rs`

- [ ] **Step 1: Add cached query methods to BookIndex**

In `crates/context_engine/src/book_index/mod.rs`, add a `DashMap`-based cache (matches existing patterns in the codebase — `dashmap` is already a workspace dependency in context_engine's `Cargo.toml`):

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

// Add to BookIndex struct:
root_sections_cache: DashMap<String, (Vec<TreeNode>, Instant)>,
cache_ttl: Duration,
```

Add cached accessor methods:

```rust
/// Get root sections with 60s TTL cache.
pub async fn get_root_sections_cached(&self, source_type: &SourceType) -> Result<Vec<TreeNode>> {
    let key = source_type.as_str().to_string();
    if let Some(entry) = self.root_sections_cache.get(&key) {
        let (nodes, inserted_at) = entry.value();
        if inserted_at.elapsed() < self.cache_ttl {
            return Ok(nodes.clone());
        }
    }
    let nodes = self.tree_repo.get_root_sections(source_type).await?;
    self.root_sections_cache.insert(key, (nodes.clone(), Instant::now()));
    Ok(nodes)
}

/// Invalidate all caches (call after tree modifications).
pub fn invalidate_caches(&self) {
    self.root_sections_cache.clear();
}
```

Update `new()` to initialize the cache and `refresh_has_content()` to invalidate.

- [ ] **Step 2: Update operators to use cached methods**

In `crates/context_engine/src/operators/selector.rs`, update `SelectBySection::execute()` to call `get_root_sections_cached()` instead of `get_root_sections()`.

- [ ] **Step 3: Verify compilation + tests**

Run: `cargo nextest run -p context_engine -v 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/book_index/mod.rs \
  crates/context_engine/src/operators/selector.rs
git commit -m "feat(bookrag): add DashMap TTL cache for tree queries"
```

---

## Task 7: Remaining Test Scenarios

Add MultiHop integration test, Global aggregation test, and regression test.

**Files:**
- Modify: `crates/context_engine/src/book_index/tests.rs`

- [ ] **Step 1: Write MultiHop integration test**

```rust
#[tokio::test]
async fn multi_hop_integration() {
    // Create two notes with shared entities
    let nodes = vec![
        // Note 1: Project Alpha
        make_section("s1", None, "Project Alpha", "note-1", 1),
        make_text("t1", "s1", "Project Alpha uses Rust and has deadline March 30.", "note-1", 2),
        // Note 2: Finance Goals
        make_section("s2", None, "Finance Goals", "note-2", 1),
        make_text("t2", "s2", "Budget for Project Alpha is $15,000.", "note-2", 2),
    ];
    let tree_repo = Arc::new(MockBookTreeRepo::new(nodes));
    let gt_links = vec![
        ("entity-alpha".to_string(), "s1".to_string()),
        ("entity-alpha".to_string(), "t1".to_string()),
        ("entity-alpha".to_string(), "t2".to_string()), // shared entity across notes
    ];
    let gt_link_repo = Arc::new(MockGTLinkRepo::new(gt_links, tree_repo.clone()));
    // ... build BookIndex, Planner, Searcher
    // Query: "How does Project Alpha relate to my finance goals?"
    // Expect: MultiHop classification, results from both notes
    let results = searcher.search("How does Project Alpha relate to my finance goals?", 10).await;
    assert!(!results.is_empty());
}
```

- [ ] **Step 2: Write Global aggregation test**

```rust
#[tokio::test]
async fn global_aggregation_integration() {
    // Create multiple notes
    let nodes = vec![
        make_section("s1", None, "Note 1", "note-1", 1),
        make_text("t1", "s1", "Task A is overdue.", "note-1", 2),
        make_section("s2", None, "Note 2", "note-2", 1),
        make_text("t2", "s2", "Task B is overdue.", "note-2", 2),
    ];
    let tree_repo = Arc::new(MockBookTreeRepo::new(nodes));
    let gt_link_repo = Arc::new(MockGTLinkRepo::new(vec![], tree_repo.clone()));
    // ... build searcher
    // Query: "How many tasks are overdue across all projects?"
    // Expect: GlobalAggregation classification
    let results = searcher.search("How many tasks are overdue across all projects?", 10).await;
    // May be empty (no FTS results from mock) but should NOT crash
}
```

- [ ] **Step 3: Write regression test (InsightForge with empty BookRAG)**

```rust
#[tokio::test]
async fn insight_forge_with_empty_bookrag_regression() {
    // Test that InsightForge works correctly with an empty BookRAGSearcher
    use crate::insight_forge::*;

    let retriever = Arc::new(MockRetriever {
        entries: vec![make_entry("m1", 0.9)],
    });
    let decomposer = Arc::new(HeuristicDecomposer);
    let mut forge = InsightForge::new(InsightForgeConfig::default(), decomposer, retriever);

    // Add BookRAGSearcher with empty index (should return empty, not crash)
    let empty_tree = Arc::new(MockBookTreeRepo::new(vec![]));
    let empty_gt = Arc::new(MockGTLinkRepo::new(vec![], empty_tree.clone()));
    let book_index = Arc::new(BookIndex::new(
        empty_tree, Arc::new(MockBookEntityRepo), empty_gt, Arc::new(MockBookEmbedder),
    ));
    let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
    let planner = Arc::new(RetrievalPlanner::new(book_index, llm, BookRetrievalConfig::default()));
    forge.add_searcher(Arc::new(BookRAGSearcher::new(planner, 50, 10, 5000)));

    let results = forge.retrieve("Tell me about something complex and interesting", 10, Some("test")).await;
    assert!(!results.is_empty(), "Memory retriever results should still work");
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run -p context_engine -v 2>&1 | tail -10`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/book_index/tests.rs
git commit -m "test(bookrag): add MultiHop, GlobalAggregation, and regression tests"
```

---

## Task 8: Clippy + Format + Full Verification

- [ ] **Step 1: Run fmt**

Run: `cargo fmt --all --check`
Fix any issues.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep warning | head -20`
Fix any new warnings.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: all PASS

- [ ] **Step 4: Build desktop**

Run: `cargo build -p desktop 2>&1 | tail -5`
Expected: success

- [ ] **Step 5: Live verification — create note and check GT-Links**

```bash
# Create a note via API
curl -s -X POST http://127.0.0.1:3456/api/note_create \
  -H 'Content-Type: application/json' \
  -d '{"title":"Test Entity Extraction","body":"# Test\n\nJayden is working on Project Alpha using Rust and Axum."}'

# Wait for tree build + entity extraction
sleep 5

# Verify tree nodes
sqlite3 ~/.klyntbot-dev/data.db "SELECT COUNT(*) || ' tree nodes' FROM book_tree_nodes;"

# Verify GT-Links were created
sqlite3 ~/.klyntbot-dev/data.db "SELECT COUNT(*) || ' GT-Links' FROM entity_tree_links;"

# Verify entities were extracted
sqlite3 ~/.klyntbot-dev/data.db "SELECT name, entity_type FROM entities WHERE source = 'bookindex' ORDER BY created_at DESC LIMIT 10;"
```

- [ ] **Step 6: Commit if needed**

```bash
git commit -m "chore: fix clippy warnings and formatting for bookrag phase 2"
```
