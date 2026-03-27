# Hierarchical Note RAG Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace flat 500-char note embeddings with per-node tree embeddings and a purpose-built NoteTreeNavigator, enabling structural retrieval across note sections.

**Architecture:** Notes are parsed into tree nodes (headings, bullet sections, paragraphs) via Tiptap JSON (primary) or markdown (fallback). Each node gets its own 384-dim embedding in a new `tree_node_embeddings` LanceDB table. A new `NoteTreeNavigator` DomainSearcher provides 3-path retrieval (simple vector / hierarchical traversal / hybrid fusion). The existing `BookRAGSearcher` and its operator pipeline are deleted.

**Tech Stack:** Rust (storage/context_engine/cognitive/agent/app-core crates), LanceDB (vector store), fastembed (384-dim embeddings), SQLite (tree nodes, FTS5), TypeScript/React (desktop-ui), Tiptap (editor decorations)

**Spec:** `docs/superpowers/specs/2026-03-27-hierarchical-note-rag-design.md`

---

### Task 1: LanceDB `tree_node_embeddings` Table

**Files:**
- Modify: `crates/storage/src/vector_store/schemas.rs`
- Modify: `crates/storage/src/vector_store/mod.rs`
- Create: `crates/storage/src/vector_store/tree_node.rs`

- [ ] **Step 1: Write the schema function**

In `crates/storage/src/vector_store/schemas.rs`, add at the end of the file:

```rust
pub(crate) fn tree_node_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("note_id", DataType::Utf8, false),
        Field::new("level", DataType::Utf8, false),
        Field::new("source_type", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

- [ ] **Step 2: Register the table in VectorStore::connect()**

In `crates/storage/src/vector_store/mod.rs`, after the last `ensure_table` call (around L105), add:

```rust
store
    .ensure_table(
        "tree_node_embeddings",
        schemas::tree_node_embedding_schema(),
    )
    .await?;
```

- [ ] **Step 3: Create tree_node.rs with search and upsert helpers**

Create `crates/storage/src/vector_store/tree_node.rs`:

```rust
use crate::vector_store::VectorStore;
use common::Result;

pub struct TreeNodeSearchResult {
    pub node_id: String,
    pub note_id: String,
    pub level: String,
    pub score: f64,
}

impl VectorStore {
    pub async fn upsert_tree_node_embedding(
        &self,
        node_id: &str,
        embedding: &[f32],
        note_id: &str,
        level: &str,
        source_type: &str,
    ) -> Result<()> {
        self.upsert_embedding(
            "tree_node_embeddings",
            node_id,
            embedding,
            &[
                ("note_id", note_id),
                ("level", level),
                ("source_type", source_type),
            ],
        )
        .await
    }

    pub async fn search_tree_node_embeddings(
        &self,
        query_vector: &[f32],
        limit: usize,
        min_similarity: f64,
        note_id_filter: Option<&str>,
    ) -> Result<Vec<TreeNodeSearchResult>> {
        let filter = note_id_filter.map(|id| {
            format!("note_id = '{}'", crate::vector_store::sanitize_predicate_value(id))
        });

        let results = self
            .search_similar_with_filter(
                "tree_node_embeddings",
                query_vector,
                limit,
                filter.as_deref(),
            )
            .await?;

        Ok(results
            .into_iter()
            .filter(|r| r.score >= min_similarity)
            .map(|r| TreeNodeSearchResult {
                node_id: r.id,
                note_id: r.extra_fields.get("note_id").cloned().unwrap_or_default(),
                level: r.extra_fields.get("level").cloned().unwrap_or_default(),
                score: r.score,
            })
            .collect())
    }

    pub async fn delete_tree_node_embeddings_by_note(&self, note_id: &str) -> Result<()> {
        let filter = format!(
            "note_id = '{}'",
            crate::vector_store::sanitize_predicate_value(note_id)
        );
        self.delete_by_filter("tree_node_embeddings", &filter).await
    }
}
```

- [ ] **Step 4: Add module declaration**

In `crates/storage/src/vector_store/mod.rs`, add:

```rust
mod tree_node;
pub use tree_node::TreeNodeSearchResult;
```

- [ ] **Step 5: Build and verify**

Run: `cargo build -p storage`
Expected: Compiles successfully with no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/src/vector_store/
git commit -m "feat(storage): add tree_node_embeddings LanceDB table and helpers"
```

---

### Task 2: Bus Layer Extensions

**Files:**
- Modify: `crates/bus/src/context_updates.rs`
- Modify: `crates/context_engine/src/rewriter.rs`

- [ ] **Step 1: Add NoteStructureChanged to ContextUpdateReason**

In `crates/bus/src/context_updates.rs`, add a new variant to the `ContextUpdateReason` enum (around L16-L23):

```rust
NoteStructureChanged,
```

And update the `Display` impl to include it.

- [ ] **Step 2: Add hierarchical_intent to RetrievalContext**

In `crates/context_engine/src/rewriter.rs`, add to the `RetrievalContext` struct (around L27-L55):

```rust
pub hierarchical_intent: Option<HierarchicalIntent>,
```

And define the type above the struct:

```rust
#[derive(Debug, Clone)]
pub struct HierarchicalIntent {
    pub query_type: HierarchicalQueryType,
    pub target_note_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HierarchicalQueryType {
    Simple,
    Hierarchical,
    Hybrid,
}
```

- [ ] **Step 3: Build both crates**

Run: `cargo build -p bus -p context-engine`
Expected: Compiles. Downstream crates still compile because new fields have `Option`/`Default`.

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/context_updates.rs crates/context_engine/src/rewriter.rs
git commit -m "feat(bus,context-engine): add NoteStructureChanged reason and hierarchical intent types"
```

---

### Task 3: Extend Scoring Model (6-Factor → 8-Factor)

**Files:**
- Modify: `crates/cognitive/src/services/decay.rs`

- [ ] **Step 1: Write failing test for 8-factor scoring**

In `crates/cognitive/src/services/decay.rs`, add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_extended_relevance_score_hierarchy_and_coherence() {
    let weights = RelevanceWeights {
        semantic: 0.25,
        retrievability: 0.15,
        importance: 0.10,
        frequency: 0.05,
        situation: 0.20,
        temporal: 0.05,
        hierarchy: 0.10,
        path_coherence: 0.10,
    };
    let score = relevance_score(
        0.8,  // semantic
        0.9,  // retrievability
        0.7,  // importance
        0.5,  // frequency
        0.6,  // situation
        0.4,  // temporal
        1.0,  // hierarchy (root node, summary query)
        0.8,  // path_coherence (siblings scored well)
        &weights,
    );
    // 0.25*0.8 + 0.15*0.9 + 0.10*0.7 + 0.05*0.5 + 0.20*0.6 + 0.05*0.4 + 0.10*1.0 + 0.10*0.8
    // = 0.20 + 0.135 + 0.07 + 0.025 + 0.12 + 0.02 + 0.10 + 0.08 = 0.75
    assert!((score - 0.75).abs() < 0.001);
}

#[test]
fn test_extended_score_backward_compat_non_note() {
    let weights = RelevanceWeights::default();
    // With default hierarchy=0.0 and path_coherence=0.5, non-note results
    // should produce similar scores to the old 6-factor model
    let score = relevance_score(0.8, 0.9, 0.7, 0.5, 0.6, 0.4, 0.0, 0.5, &weights);
    assert!(score > 0.0 && score <= 1.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(extended_relevance_score)'`
Expected: FAIL — `relevance_score` doesn't accept 8 args yet, `RelevanceWeights` missing fields.

- [ ] **Step 3: Extend RelevanceWeights and relevance_score**

In `crates/cognitive/src/services/decay.rs`:

Add two fields to `RelevanceWeights`:

```rust
pub hierarchy: f64,
pub path_coherence: f64,
```

Update `Default` for `RelevanceWeights`:

```rust
impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            semantic: 0.25,
            retrievability: 0.15,
            importance: 0.10,
            frequency: 0.05,
            situation: 0.20,
            temporal: 0.05,
            hierarchy: 0.10,
            path_coherence: 0.10,
        }
    }
}
```

Update `relevance_score` signature and body to accept and use the two new factors:

```rust
pub fn relevance_score(
    semantic_similarity: f64,
    retrievability: f64,
    importance: f64,
    access_frequency: f64,
    situational_boost: f64,
    temporal_recency: f64,
    hierarchy_score: f64,
    path_coherence: f64,
    weights: &RelevanceWeights,
) -> f64 {
    let score = weights.semantic * semantic_similarity
        + weights.retrievability * retrievability
        + weights.importance * importance
        + weights.frequency * access_frequency
        + weights.situation * situational_boost
        + weights.temporal * temporal_recency
        + weights.hierarchy * hierarchy_score
        + weights.path_coherence * path_coherence;
    score.clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Fix all existing callers**

All existing call sites of `relevance_score` pass 6 positional args. Add `0.0, 0.5` (neutral hierarchy, neutral coherence) to each call site. Search with:

Run: `cargo build --workspace 2>&1 | head -50`

Fix each compiler error by adding the two new args. The main callers are in `crates/cognitive/src/services/retrieval.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(relevance_score)'`
Expected: All PASS including the two new tests.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/decay.rs crates/cognitive/src/services/retrieval.rs
git commit -m "feat(cognitive): extend relevance scorer from 6-factor to 8-factor (hierarchy + path_coherence)"
```

---

### Task 4: Autotuner Parameter Expansion (19D → 24D)

**Files:**
- Modify: `crates/common/src/autotuner.rs`
- Modify: `crates/autotuner/src/generator.rs`

- [ ] **Step 1: Add 5 new fields to TrialParams**

In `crates/common/src/autotuner.rs`, add after the Phase 3 fields:

```rust
// Phase 4: Hierarchical note retrieval (5 params)
pub relevance_weight_hierarchy: Option<f64>,
pub relevance_weight_path_coherence: Option<f64>,
pub tree_top_k: Option<usize>,
pub tree_min_similarity: Option<f64>,
pub hybrid_bias: Option<f64>,
```

- [ ] **Step 2: Add bounds to generator prompt**

In `crates/autotuner/src/generator.rs`, find the `build_generation_prompt` function's bounds table (around L171-L193) and add 5 rows:

```
| relevance_weight_hierarchy    | 0.0–0.25 | 0.10 | hierarchy_score weight in 8-factor model |
| relevance_weight_path_coherence | 0.0–0.20 | 0.10 | path_coherence weight in 8-factor model |
| tree_top_k                    | 5–30     | 15   | top-k for tree_node_embeddings search |
| tree_min_similarity           | 0.3–0.7  | 0.50 | min cosine similarity for tree nodes |
| hybrid_bias                   | 0.0–1.0  | 0.50 | RRF weight in hybrid retrieval path |
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p common -p autotuner`
Expected: Compiles. All `Option<T>` + `#[serde(default)]` ensures backward compat.

- [ ] **Step 4: Commit**

```bash
git add crates/common/src/autotuner.rs crates/autotuner/src/generator.rs
git commit -m "feat(autotuner): expand search space from 19D to 24D for hierarchical note retrieval"
```

---

### Task 5: Frontend — Add Tiptap JSON to Save Flow

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/EditorCore.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Add getJSON() to EditorCore onUpdate callback**

In `desktop-ui/src/features/notes/components/editor/EditorCore.tsx`, find the `onUpdate` callback (around L248-L249) that currently calls:

```typescript
onUpdate(ed.getHTML(), ed.storage.markdown.getMarkdown())
```

Change to:

```typescript
onUpdate(ed.getHTML(), ed.storage.markdown.getMarkdown(), JSON.stringify(ed.getJSON()))
```

Update the `onUpdate` prop type in the component to accept the third argument:

```typescript
onUpdate: (html: string, markdown: string, json: string) => void
```

- [ ] **Step 2: Pass bodyJson through NoteEditor save flow**

In `desktop-ui/src/features/notes/components/NoteEditor.tsx`, find the `handleUpdate` function (around L146) and update it to capture the JSON:

Update the `pendingRef` to store `{ html, markdown, json }`.

In `flushSave()` (around L128), update the `onSaveRef.current` call to include `bodyJson: pendingRef.current.json`.

- [ ] **Step 3: Update NoteUpdateParams type and mutation**

In the types file where `NoteUpdateParams` is defined, add:

```typescript
bodyJson?: string;
```

In `KnowledgeBasePage.tsx`, ensure the `onSave` callback passes `bodyJson` through to the mutation.

- [ ] **Step 4: Build frontend and verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles with no errors.

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(ui): add Tiptap JSON to note save flow for hierarchical tree parsing"
```

---

### Task 6: Backend — Store body_json Column

**Files:**
- Modify: `crates/feature-notes/src/models.rs`
- Modify: `crates/feature-notes/src/migrations/001_create_notes.sql`
- Modify: `crates/feature-notes/src/repos.rs` (or wherever NoteRepo insert/update SQL lives)
- Modify: `crates/app-core/src/handlers/notes/crud.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs` (NoteUpdateParams)

- [ ] **Step 1: Add body_json to NoteRow**

In `crates/feature-notes/src/models.rs`, add to `NoteRow` struct (around L66-L84):

```rust
pub body_json: Option<String>,
```

- [ ] **Step 2: Add column to migration SQL**

In `crates/feature-notes/src/migrations/001_create_notes.sql`, add to the `CREATE TABLE notes` statement:

```sql
body_json TEXT,
```

Bump the `FeatureMigration` version for `notes` in the crate's migration registration.

- [ ] **Step 3: Update NoteUpdateParams**

In `crates/desktop-shared/src/commands/notes.rs` (or wherever `NoteUpdateParams` is defined), add:

```rust
pub body_json: Option<String>,
```

- [ ] **Step 4: Update note_create and note_update handlers**

In `crates/app-core/src/handlers/notes/crud.rs`:

In `note_create` (around L102): include `body_json` in the `NoteRow` construction if provided in params.

In `note_update` (around L183): pass `body_json` to `repo.update_note()` if present in params.

Update the SQL INSERT/UPDATE queries in the NoteRepo to include the `body_json` column.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p feature-notes -p app-core -p desktop-shared`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-notes/ crates/app-core/src/handlers/notes/ crates/desktop-shared/
git commit -m "feat(notes): add body_json column for Tiptap JSON storage"
```

---

### Task 7: Tiptap JSON Parser

**Files:**
- Create: `crates/cognitive/src/services/tiptap_parser.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/cognitive/src/services/tiptap_parser.rs`:

```rust
use context_engine::book_index::types::TreeNode;

/// Parse a Tiptap JSON document into a tree of nodes.
/// Primary path for note tree building — richer than markdown because it preserves
/// bulletList, taskList, blockquote as distinct node types.
pub fn parse_tiptap_json_to_tree(source_id: &str, json_str: &str) -> Vec<TreeNode> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headings() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": { "level": 1 },
                    "content": [{ "type": "text", "text": "Introduction" }]
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Some intro text here." }]
                },
                {
                    "type": "heading",
                    "attrs": { "level": 2 },
                    "content": [{ "type": "text", "text": "Background" }]
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Background details." }]
                }
            ]
        }"#;

        let nodes = parse_tiptap_json_to_tree("note-1", json);
        // Root (level 0) + 2 headings (level 1, level 2)
        assert!(nodes.len() >= 3);
        // First real heading
        let h1 = nodes.iter().find(|n| n.level == 1).unwrap();
        assert_eq!(h1.title, "Introduction");
        assert!(h1.content.contains("Some intro text"));
        // Second heading is nested under first
        let h2 = nodes.iter().find(|n| n.level == 2).unwrap();
        assert_eq!(h2.title, "Background");
        assert_eq!(h2.parent_id, Some(h1.id.clone()));
    }

    #[test]
    fn test_parse_bullet_list_as_pseudo_section() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "bulletList",
                    "content": [
                        {
                            "type": "listItem",
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Item one" }] }
                            ]
                        },
                        {
                            "type": "listItem",
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Item two" }] }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let nodes = parse_tiptap_json_to_tree("note-2", json);
        // Should have at least a root and a bullet pseudo-section (level 7)
        let bullet_nodes: Vec<_> = nodes.iter().filter(|n| n.level == 7).collect();
        assert!(!bullet_nodes.is_empty());
        assert!(bullet_nodes[0].content.contains("Item one"));
    }

    #[test]
    fn test_parse_task_list() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "taskList",
                    "content": [
                        {
                            "type": "taskItem",
                            "attrs": { "checked": true },
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Done task" }] }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let nodes = parse_tiptap_json_to_tree("note-3", json);
        let task_nodes: Vec<_> = nodes.iter().filter(|n| n.level == 7).collect();
        assert!(!task_nodes.is_empty());
        assert!(task_nodes[0].content.contains("Done task"));
    }

    #[test]
    fn test_fallback_on_invalid_json() {
        let result = parse_tiptap_json_to_tree("note-4", "not valid json");
        assert!(result.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(parse_tiptap)'`
Expected: FAIL with `todo!()`.

- [ ] **Step 3: Implement the parser**

Replace `todo!()` in `parse_tiptap_json_to_tree` with the full implementation:

```rust
use serde_json::Value;
use uuid::Uuid;

pub fn parse_tiptap_json_to_tree(source_id: &str, json_str: &str) -> Vec<TreeNode> {
    let doc: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let content = match doc.get("content").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => return vec![],
    };

    let mut nodes = Vec::new();
    let root_id = Uuid::new_v4().to_string();

    // Root node (level 0)
    nodes.push(TreeNode {
        id: root_id.clone(),
        source_id: source_id.to_string(),
        source_type: "note".to_string(),
        parent_id: None,
        level: 0,
        title: source_id.to_string(), // Will be replaced with note title by caller
        content: String::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
    });

    let mut heading_stack: Vec<(u8, String)> = vec![(0, root_id.clone())];
    let mut current_content = String::new();
    let mut current_parent = root_id.clone();

    for block in content {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match block_type {
            "heading" => {
                // Flush accumulated content to current parent
                if !current_content.is_empty() {
                    if let Some(parent) = nodes.iter_mut().find(|n| n.id == current_parent) {
                        if parent.content.is_empty() {
                            parent.content = current_content.clone();
                        } else {
                            parent.content.push('\n');
                            parent.content.push_str(&current_content);
                        }
                    }
                    current_content.clear();
                }

                let level = block
                    .get("attrs")
                    .and_then(|a| a.get("level"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(1) as u8;

                let title = extract_text(block);
                let node_id = Uuid::new_v4().to_string();

                // Pop stack until we find a parent with lower level
                while heading_stack.last().is_some_and(|(l, _)| *l >= level) {
                    heading_stack.pop();
                }
                let parent_id = heading_stack.last().map(|(_, id)| id.clone()).unwrap_or(root_id.clone());

                nodes.push(TreeNode {
                    id: node_id.clone(),
                    source_id: source_id.to_string(),
                    source_type: "note".to_string(),
                    parent_id: Some(parent_id),
                    level,
                    title,
                    content: String::new(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                });

                heading_stack.push((level, node_id.clone()));
                current_parent = node_id;
            }
            "bulletList" | "orderedList" | "taskList" => {
                let list_content = extract_list_text(block);
                if !list_content.is_empty() {
                    let node_id = Uuid::new_v4().to_string();
                    nodes.push(TreeNode {
                        id: node_id,
                        source_id: source_id.to_string(),
                        source_type: "note".to_string(),
                        parent_id: Some(current_parent.clone()),
                        level: 7,
                        title: String::new(),
                        content: list_content,
                        created_at: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
            "paragraph" | "blockquote" => {
                let text = extract_text(block);
                if !text.is_empty() {
                    if !current_content.is_empty() {
                        current_content.push('\n');
                    }
                    current_content.push_str(&text);
                }
            }
            _ => {
                // codeBlock, image, etc. — extract any text content
                let text = extract_text(block);
                if !text.is_empty() {
                    if !current_content.is_empty() {
                        current_content.push('\n');
                    }
                    current_content.push_str(&text);
                }
            }
        }
    }

    // Flush remaining content
    if !current_content.is_empty() {
        if let Some(parent) = nodes.iter_mut().find(|n| n.id == current_parent) {
            if parent.content.is_empty() {
                parent.content = current_content;
            } else {
                parent.content.push('\n');
                parent.content.push_str(&current_content);
            }
        }
    }

    nodes
}

fn extract_text(node: &Value) -> String {
    let mut text = String::new();
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        for child in content {
            let child_type = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if child_type == "text" {
                if let Some(t) = child.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
            } else {
                // Recurse into nested nodes
                let nested = extract_text(child);
                if !nested.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&nested);
                }
            }
        }
    }
    text
}

fn extract_list_text(node: &Value) -> String {
    let mut items = Vec::new();
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        for item in content {
            let text = extract_text(item);
            if !text.is_empty() {
                items.push(text);
            }
        }
    }
    items.join("\n")
}
```

- [ ] **Step 4: Add module declaration**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod tiptap_parser;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(parse_tiptap)'`
Expected: All 4 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/tiptap_parser.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): add Tiptap JSON parser for hierarchical tree node extraction"
```

---

### Task 8: NoteTreeBuilder Event Subscriber

**Files:**
- Create: `crates/agent/src/adapters/note_tree_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Create NoteTreeBuilder**

Create `crates/agent/src/adapters/note_tree_builder.rs`:

```rust
use std::sync::Arc;

use bus::{DomainEvent, ContextUpdateQueue, ContextUpdate, ContextUpdateReason, UpdatePriority};
use cognitive::repos::book_tree::SqliteBookTreeRepo;
use cognitive::repos::gt_link::SqliteGTLinkRepo;
use cognitive::services::tiptap_parser::parse_tiptap_json_to_tree;
use common::Result;
use context_engine::book_index::types::TreeNode;
use storage::vector_store::VectorStore;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::adapters::cognitive_embedder::TextEmbedderImpl;

pub struct NoteTreeBuilder {
    tree_repo: Arc<SqliteBookTreeRepo>,
    gt_link_repo: Arc<SqliteGTLinkRepo>,
    vector_store: Arc<VectorStore>,
    embedder: Arc<TextEmbedderImpl>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
}

impl NoteTreeBuilder {
    pub fn new(
        tree_repo: Arc<SqliteBookTreeRepo>,
        gt_link_repo: Arc<SqliteGTLinkRepo>,
        vector_store: Arc<VectorStore>,
        embedder: Arc<TextEmbedderImpl>,
        context_update_queue: Option<Arc<ContextUpdateQueue>>,
    ) -> Self {
        Self {
            tree_repo,
            gt_link_repo,
            vector_store,
            embedder,
            context_update_queue,
        }
    }

    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::NoteContentChanged { note_id, content }) => {
                            if let Err(e) = self.handle_note_changed(&note_id, &content, None).await {
                                warn!("NoteTreeBuilder failed for note {note_id}: {e}");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("NoteTreeBuilder lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
            }
        }
        debug!("NoteTreeBuilder shut down");
    }

    pub async fn handle_note_changed(
        &self,
        note_id: &str,
        markdown_content: &str,
        tiptap_json: Option<&str>,
    ) -> Result<()> {
        // Parse tree: Tiptap JSON primary, markdown fallback
        let nodes = if let Some(json) = tiptap_json {
            let tiptap_nodes = parse_tiptap_json_to_tree(note_id, json);
            if tiptap_nodes.is_empty() {
                cognitive::repos::markdown_parser::parse_markdown_to_tree(note_id, markdown_content)
            } else {
                tiptap_nodes
            }
        } else {
            cognitive::repos::markdown_parser::parse_markdown_to_tree(note_id, markdown_content)
        };

        if nodes.is_empty() {
            return Ok(());
        }

        // Clear old tree nodes for this note
        self.tree_repo.delete_by_source(note_id).await?;
        self.vector_store.delete_tree_node_embeddings_by_note(note_id).await?;

        // Insert new tree nodes
        self.tree_repo.insert_nodes(&nodes).await?;

        // Embed each node
        let mut embedded_paths = Vec::new();
        for node in &nodes {
            let text = compose_node_text(node);
            if text.is_empty() {
                continue;
            }

            match self.embedder.embed(&text).await {
                Ok(embedding) => {
                    if let Err(e) = self.vector_store.upsert_tree_node_embedding(
                        &node.id,
                        &embedding,
                        note_id,
                        &node.level.to_string(),
                        &node.source_type,
                    ).await {
                        warn!("Failed to upsert tree node embedding {}: {e}", node.id);
                    }
                }
                Err(e) => {
                    warn!("Failed to embed tree node {}: {e}", node.id);
                }
            }

            if node.level > 0 && node.level <= 6 {
                embedded_paths.push(node.title.clone());
            }
        }

        // Push context update for live injection
        if let Some(queue) = &self.context_update_queue {
            let path_summary = if embedded_paths.is_empty() {
                format!("Note {note_id} tree structure updated ({} nodes)", nodes.len())
            } else {
                let preview = &embedded_paths[..embedded_paths.len().min(3)];
                format!(
                    "NoteStructureChanged: Sections updated: [{}] ({} total nodes)",
                    preview.join(" > "),
                    nodes.len()
                )
            };

            queue.push(ContextUpdate::new(
                ContextUpdateReason::NoteStructureChanged,
                Some(path_summary),
                None,
                UpdatePriority::Normal,
            ));
        }

        debug!("NoteTreeBuilder processed note {note_id}: {} tree nodes", nodes.len());
        Ok(())
    }
}

fn compose_node_text(node: &TreeNode) -> String {
    match node.level {
        0 => node.title.clone(),
        1..=6 => {
            let preview = if node.content.len() > 300 {
                &node.content[..300]
            } else {
                &node.content
            };
            if node.title.is_empty() {
                preview.to_string()
            } else {
                format!("{}\n{}", node.title, preview)
            }
        }
        _ => {
            if node.content.len() > 300 {
                node.content[..300].to_string()
            } else {
                node.content.clone()
            }
        }
    }
}
```

- [ ] **Step 2: Add module declaration**

In `crates/agent/src/adapters/mod.rs`, add:

```rust
pub mod note_tree_builder;
```

- [ ] **Step 3: Build**

Run: `cargo build -p agent`
Expected: Compiles. May need adjustments to imports based on actual module paths.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/adapters/note_tree_builder.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(agent): add NoteTreeBuilder event subscriber for tree node embedding pipeline"
```

---

### Task 9: Query Classifier + NoteTreeNavigator

**Files:**
- Create: `crates/context_engine/src/insight_forge/note_tree_navigator.rs`
- Modify: `crates/context_engine/src/insight_forge/mod.rs`

- [ ] **Step 1: Write failing test for query classification**

Create `crates/context_engine/src/insight_forge/note_tree_navigator.rs`:

```rust
use async_trait::async_trait;
use common::Result;

use crate::insight_forge::domain_searcher::{DomainSearcher, SearchContext};
use crate::insight_forge::types::MemoryEntry;
use crate::rewriter::HierarchicalQueryType;

/// Heuristic keywords that signal hierarchical intent.
const HIERARCHICAL_KEYWORDS: &[&str] = &[
    "section", "part", "chapter", "heading", "paragraph",
    "in my note", "in the note", "from the note", "note about",
    "notebook", "under the",
];

#[derive(Debug, Clone, PartialEq)]
enum QueryClass {
    Simple,
    Hierarchical,
    Hybrid,
}

fn classify_query(query: &str, has_active_task: bool) -> QueryClass {
    let lower = query.to_lowercase();
    let has_hierarchical_keyword = HIERARCHICAL_KEYWORDS
        .iter()
        .any(|kw| lower.contains(kw));

    if has_hierarchical_keyword && has_active_task {
        QueryClass::Hybrid
    } else if has_hierarchical_keyword {
        QueryClass::Hierarchical
    } else {
        QueryClass::Simple
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        assert_eq!(classify_query("what is photosynthesis", false), QueryClass::Simple);
    }

    #[test]
    fn test_hierarchical_query() {
        assert_eq!(
            classify_query("summarize the section about sleep in my health note", false),
            QueryClass::Hierarchical
        );
    }

    #[test]
    fn test_hybrid_query_with_active_task() {
        assert_eq!(
            classify_query("what does the section on habits say", true),
            QueryClass::Hybrid
        );
    }

    #[test]
    fn test_notebook_keyword() {
        assert_eq!(
            classify_query("find everything in notebook about finance", false),
            QueryClass::Hierarchical
        );
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p context-engine -E 'test(classify_query)'`
Expected: All 4 PASS.

- [ ] **Step 3: Implement NoteTreeNavigator struct**

Add the full `NoteTreeNavigator` implementation to the same file. This requires trait objects for the vector store and tree repo — use the traits already defined in `context_engine::book_index`:

```rust
use std::sync::Arc;

/// Trait for tree node embedding search — implemented by adapter in agent crate.
#[async_trait]
pub trait TreeNodeEmbeddingSearch: Send + Sync {
    async fn search_tree_nodes(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
        note_id_filter: Option<&str>,
    ) -> Result<Vec<TreeNodeHit>>;

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
}

#[derive(Debug, Clone)]
pub struct TreeNodeHit {
    pub node_id: String,
    pub note_id: String,
    pub level: u8,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct TreeSearchResult {
    pub node_id: String,
    pub note_id: String,
    pub title: String,
    pub content: String,
    pub path: Vec<PathSegment>,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub node_id: String,
    pub title: String,
    pub level: u8,
}

pub struct NoteTreeNavigator {
    embedding_search: Arc<dyn TreeNodeEmbeddingSearch>,
    tree_repo: Arc<dyn crate::book_index::tree::BookTreeRepo>,
    top_k: usize,
    min_similarity: f64,
}

impl NoteTreeNavigator {
    pub fn new(
        embedding_search: Arc<dyn TreeNodeEmbeddingSearch>,
        tree_repo: Arc<dyn crate::book_index::tree::BookTreeRepo>,
    ) -> Self {
        Self {
            embedding_search,
            tree_repo,
            top_k: 15,
            min_similarity: 0.50,
        }
    }

    async fn simple_search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let embedding = self.embedding_search.embed_query(query).await?;
        let hits = self
            .embedding_search
            .search_tree_nodes(&embedding, self.top_k, self.min_similarity, None)
            .await?;

        let mut results = Vec::new();
        for hit in hits.into_iter().take(limit) {
            if let Ok(Some(node)) = self.tree_repo.get(&hit.node_id).await {
                let ancestors = self.tree_repo.get_ancestors(&hit.node_id).await.unwrap_or_default();
                let path: Vec<PathSegment> = ancestors
                    .iter()
                    .map(|a| PathSegment {
                        node_id: a.id.clone(),
                        title: a.title.clone(),
                        level: a.level,
                    })
                    .collect();

                let path_str = path.iter().map(|p| p.title.as_str()).collect::<Vec<_>>().join(" > ");

                results.push(MemoryEntry {
                    content: format!("[{}] {}", path_str, node.content),
                    source: "note_tree".to_string(),
                    score: hit.score,
                    metadata: Some(serde_json::json!({
                        "node_id": hit.node_id,
                        "note_id": hit.note_id,
                        "level": hit.level,
                        "path": path.iter().map(|p| &p.title).collect::<Vec<_>>(),
                    })),
                });
            }
        }
        Ok(results)
    }

    async fn hierarchical_search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        // Coarse vector pass
        let embedding = self.embedding_search.embed_query(query).await?;
        let hits = self
            .embedding_search
            .search_tree_nodes(&embedding, 10, self.min_similarity, None)
            .await?;

        // Identify candidate notes
        let mut note_ids: Vec<String> = hits.iter().map(|h| h.note_id.clone()).collect();
        note_ids.sort();
        note_ids.dedup();

        let mut all_results = Vec::new();

        for note_id in note_ids.iter().take(3) {
            // Load full subtree for this note's root
            if let Ok(subtree) = self.tree_repo.get_by_source(note_id).await {
                for node in &subtree {
                    let vector_score = hits
                        .iter()
                        .find(|h| h.node_id == node.id)
                        .map(|h| h.score)
                        .unwrap_or(0.0);

                    // FTS boost — check if query terms appear in node title/content
                    let title_lower = node.title.to_lowercase();
                    let content_lower = node.content.to_lowercase();
                    let query_lower = query.to_lowercase();
                    let fts_boost = query_lower
                        .split_whitespace()
                        .filter(|w| w.len() > 2)
                        .filter(|w| title_lower.contains(w) || content_lower.contains(w))
                        .count() as f64
                        * 0.05;

                    let combined_score = vector_score + fts_boost;

                    if combined_score > 0.0 {
                        let ancestors = self.tree_repo.get_ancestors(&node.id).await.unwrap_or_default();
                        let path: Vec<PathSegment> = ancestors
                            .iter()
                            .map(|a| PathSegment {
                                node_id: a.id.clone(),
                                title: a.title.clone(),
                                level: a.level,
                            })
                            .collect();
                        let path_str = path.iter().map(|p| p.title.as_str()).collect::<Vec<_>>().join(" > ");

                        all_results.push(MemoryEntry {
                            content: format!("[{}] {}", path_str, node.content),
                            source: "note_tree".to_string(),
                            score: combined_score,
                            metadata: Some(serde_json::json!({
                                "node_id": node.id,
                                "note_id": note_id,
                                "level": node.level,
                                "path": path.iter().map(|p| &p.title).collect::<Vec<_>>(),
                            })),
                        });
                    }
                }
            }
        }

        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(limit);
        Ok(all_results)
    }
}

#[async_trait]
impl DomainSearcher for NoteTreeNavigator {
    fn name(&self) -> &str {
        "note_tree"
    }

    async fn search(&self, query: &str, context: &SearchContext) -> Result<Vec<MemoryEntry>> {
        let has_active_task = context.active_task.is_some();
        let query_class = classify_query(query, has_active_task);
        let limit = context.limit.unwrap_or(5);

        match query_class {
            QueryClass::Simple => self.simple_search(query, limit).await,
            QueryClass::Hierarchical => self.hierarchical_search(query, limit).await,
            QueryClass::Hybrid => {
                // Merge simple + hierarchical results
                let (simple, hierarchical) = tokio::join!(
                    self.simple_search(query, limit),
                    self.hierarchical_search(query, limit),
                );
                let mut merged = simple.unwrap_or_default();
                merged.extend(hierarchical.unwrap_or_default());
                // RRF merge
                merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                // Dedup by node_id
                let mut seen = std::collections::HashSet::new();
                merged.retain(|entry| {
                    entry.metadata.as_ref()
                        .and_then(|m| m.get("node_id"))
                        .and_then(|id| id.as_str())
                        .map(|id| seen.insert(id.to_string()))
                        .unwrap_or(true)
                });
                merged.truncate(limit);
                Ok(merged)
            }
        }
    }
}
```

- [ ] **Step 4: Export from mod.rs**

In `crates/context_engine/src/insight_forge/mod.rs`, add:

```rust
pub mod note_tree_navigator;
```

- [ ] **Step 5: Build**

Run: `cargo build -p context-engine`
Expected: Compiles. Adjust imports as needed based on actual trait definitions.

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/insight_forge/note_tree_navigator.rs crates/context_engine/src/insight_forge/mod.rs
git commit -m "feat(context-engine): add NoteTreeNavigator with 3-path retrieval (simple/hierarchical/hybrid)"
```

---

### Task 10: Delete BookRAGSearcher + Rewire Builder

**Files:**
- Delete: `crates/context_engine/src/insight_forge/bookrag_searcher.rs`
- Delete: All operator files under `crates/context_engine/src/insight_forge/operators/` (if they exist as separate files)
- Delete: `RetrievalPlanner` (find its file)
- Modify: `crates/context_engine/src/insight_forge/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Create: `crates/agent/src/adapters/tree_node_search.rs` (adapter implementing `TreeNodeEmbeddingSearch`)

- [ ] **Step 1: Create TreeNodeEmbeddingSearch adapter**

Create `crates/agent/src/adapters/tree_node_search.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use common::Result;
use context_engine::insight_forge::note_tree_navigator::{TreeNodeEmbeddingSearch, TreeNodeHit};
use storage::vector_store::VectorStore;

use super::cognitive_embedder::TextEmbedderImpl;

pub struct TreeNodeSearchAdapter {
    vector_store: Arc<VectorStore>,
    embedder: Arc<TextEmbedderImpl>,
}

impl TreeNodeSearchAdapter {
    pub fn new(vector_store: Arc<VectorStore>, embedder: Arc<TextEmbedderImpl>) -> Self {
        Self { vector_store, embedder }
    }
}

#[async_trait]
impl TreeNodeEmbeddingSearch for TreeNodeSearchAdapter {
    async fn search_tree_nodes(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
        note_id_filter: Option<&str>,
    ) -> Result<Vec<TreeNodeHit>> {
        let results = self
            .vector_store
            .search_tree_node_embeddings(query_embedding, limit, min_similarity, note_id_filter)
            .await?;

        Ok(results
            .into_iter()
            .map(|r| TreeNodeHit {
                node_id: r.node_id,
                note_id: r.note_id,
                level: r.level.parse().unwrap_or(0),
                score: r.score,
            })
            .collect())
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embedder.embed(query).await
    }
}
```

- [ ] **Step 2: Add module declaration**

In `crates/agent/src/adapters/mod.rs`, add:

```rust
pub mod tree_node_search;
```

- [ ] **Step 3: Delete BookRAGSearcher and related files**

Remove `crates/context_engine/src/insight_forge/bookrag_searcher.rs` and any operator files that are only used by BookRAGSearcher. Remove the `pub mod bookrag_searcher;` line from `insight_forge/mod.rs`.

Run: `cargo build -p context-engine 2>&1 | head -30`

Fix any remaining references to deleted types. Remove unused imports.

- [ ] **Step 4: Rewire builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, find the BookRAG wiring section (around L753-L838). Replace it with NoteTreeNavigator wiring:

```rust
// NoteTreeNavigator (replaces BookRAGSearcher)
let tree_node_search = Arc::new(TreeNodeSearchAdapter::new(
    vector_store.clone(),
    text_embedder.clone(),
));
let note_tree_navigator = NoteTreeNavigator::new(
    tree_node_search,
    tree_repo.clone(),
);
forge.add_searcher(Arc::new(note_tree_navigator));

// NoteTreeBuilder subscriber
let note_tree_builder = Arc::new(NoteTreeBuilder::new(
    tree_repo.clone(),
    gt_link_repo.clone(),
    vector_store.clone(),
    text_embedder.clone(),
    context_update_queue.clone(),
));
let tree_builder_rx = bus.subscribe();
let tree_builder_shutdown = shutdown_token.clone();
let tree_builder_handle = tokio::spawn(async move {
    note_tree_builder.run(tree_builder_rx, tree_builder_shutdown).await;
});
```

Remove all BookRAGSearcher, BookIndex, BookIndexUpdater, RetrievalPlanner, operator imports and construction code.

- [ ] **Step 5: Build full workspace**

Run: `cargo build --workspace`
Expected: Compiles with no errors. Fix any remaining references to deleted types.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (or only pre-existing desktop exceptions).

- [ ] **Step 7: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass. Some tests that referenced BookRAGSearcher may need updates or removal.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(agent): replace BookRAGSearcher with NoteTreeNavigator, wire tree builder subscriber"
```

---

### Task 11: Background Migration Job + Feature Flag

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/config/src/schema/` (add feature flag)

- [ ] **Step 1: Add feature flag to config**

In the appropriate config schema file (likely `crates/config/src/schema/cognitive.rs` or similar), add:

```rust
#[serde(default = "default_true")]
pub hierarchical_notes: bool,
```

Where `default_true` returns `true`.

- [ ] **Step 2: Add migration job to app-core init**

In `crates/app-core/src/init/mod.rs`, add a background task that runs after startup (similar to the existing note embedding catch-up job around L649):

```rust
// Hierarchical note tree migration
if config.cognitive.hierarchical_notes {
    let repos = repos.clone();
    let tree_builder = note_tree_builder.clone();
    tokio::spawn(async move {
        if let Err(e) = migrate_notes_to_tree_nodes(&repos, &tree_builder).await {
            tracing::warn!("Note tree migration error: {e}");
        }
    });
}

async fn migrate_notes_to_tree_nodes(
    repos: &Repos,
    tree_builder: &NoteTreeBuilder,
) -> common::Result<()> {
    let batch_size = 100;
    let mut offset = 0;
    loop {
        let notes = repos.notes().list_notes_needing_tree_index(batch_size, offset).await?;
        if notes.is_empty() {
            break;
        }
        for note in &notes {
            tree_builder
                .handle_note_changed(&note.id, &note.body, note.body_json.as_deref())
                .await?;
        }
        offset += notes.len();
        tracing::info!("Tree migration progress: {offset} notes processed");
        // Yield to other tasks
        tokio::task::yield_now().await;
    }
    tracing::info!("Note tree migration complete: {offset} total notes");
    Ok(())
}
```

- [ ] **Step 3: Add list_notes_needing_tree_index to NoteRepo**

In the NoteRepo, add a query that returns notes without corresponding `book_tree_nodes` rows:

```sql
SELECT n.* FROM notes n
WHERE NOT EXISTS (
    SELECT 1 FROM book_tree_nodes btn
    WHERE btn.source_id = n.id AND btn.source_type = 'note'
)
AND n.body != ''
ORDER BY n.updated_at DESC
LIMIT ? OFFSET ?
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p app-core`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/ crates/config/ crates/feature-notes/
git commit -m "feat(app-core): add background migration job for hierarchical note tree indexing"
```

---

### Task 12: Chat Breadcrumb UI

**Files:**
- Modify: `desktop-ui/src/features/chat/components/MessageList.tsx` (or the relevant message component)
- Create: `desktop-ui/src/features/chat/components/TreePathBreadcrumb.tsx`
- Create: `desktop-ui/src/shared/types/tree-path.ts`

- [ ] **Step 1: Define TreePathRef type**

Create `desktop-ui/src/shared/types/tree-path.ts`:

```typescript
export interface TreePathRef {
  noteId: string;
  noteName: string;
  path: PathSegment[];
  nodeId: string;
  similarity: number;
}

export interface PathSegment {
  nodeId: string;
  title: string;
  level: number;
}
```

- [ ] **Step 2: Create TreePathBreadcrumb component**

Create `desktop-ui/src/features/chat/components/TreePathBreadcrumb.tsx`:

```tsx
import type { TreePathRef } from "@shared/types/tree-path";
import { ipc } from "@shared/hooks/useIpc";

interface Props {
  paths: TreePathRef[];
}

export function TreePathBreadcrumb({ paths }: Props) {
  if (paths.length === 0) return null;

  const handleClick = (path: TreePathRef) => {
    // Navigate to note and scroll to section
    ipc("navigate_to_note_section", {
      noteId: path.noteId,
      nodeId: path.nodeId,
    });
  };

  return (
    <div className="flex flex-wrap gap-1.5 mt-1.5">
      {paths.map((path) => (
        <button
          key={path.nodeId}
          type="button"
          onClick={() => handleClick(path)}
          className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-md
                     bg-surface-raised/50 text-muted hover:text-foreground hover:bg-surface-raised
                     transition-colors cursor-pointer border border-border/50"
        >
          <span className="opacity-60">📄</span>
          {path.path.map((seg, i) => (
            <span key={seg.nodeId}>
              {i > 0 && <span className="opacity-40 mx-0.5">›</span>}
              <span>{seg.title}</span>
            </span>
          ))}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Integrate into message rendering**

In the chat message component, check if the message metadata contains tree path references and render the breadcrumb below the message content:

```tsx
import { TreePathBreadcrumb } from "./TreePathBreadcrumb";

// Inside the assistant message rendering:
{message.metadata?.treePaths && (
  <TreePathBreadcrumb paths={message.metadata.treePaths} />
)}
```

- [ ] **Step 4: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(ui): add tree path breadcrumb in chat messages for note section references"
```

---

### Task 13: AI Highlight Decoration in Tiptap Editor

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/extensions/AiHighlightPlugin.ts`
- Modify: `desktop-ui/src/features/notes/components/editor/EditorCore.tsx`
- Modify: `desktop-ui/src/styles/editor.css`

- [ ] **Step 1: Create AiHighlightPlugin**

Create `desktop-ui/src/features/notes/components/editor/extensions/AiHighlightPlugin.ts`:

```typescript
import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export interface AiHighlight {
  from: number;
  to: number;
  similarity: number;
  nodeId: string;
}

const aiHighlightPluginKey = new PluginKey("aiHighlight");

export const AiHighlightExtension = Extension.create({
  name: "aiHighlight",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: aiHighlightPluginKey,
        state: {
          init() {
            return DecorationSet.empty;
          },
          apply(tr, decorationSet) {
            const meta = tr.getMeta(aiHighlightPluginKey);
            if (meta?.highlights) {
              const decorations = meta.highlights.map((h: AiHighlight) =>
                Decoration.inline(h.from, h.to, {
                  class: "ai-highlight",
                  "data-similarity": String(h.similarity),
                  "data-node-id": h.nodeId,
                  title: `AI matched this section (${(h.similarity * 100).toFixed(0)}% similarity)`,
                }),
              );
              return DecorationSet.create(tr.doc, decorations);
            }
            if (meta?.clear) {
              return DecorationSet.empty;
            }
            return decorationSet.map(tr.mapping, tr.doc);
          },
        },
        props: {
          decorations(state) {
            return this.getState(state);
          },
        },
      }),
    ];
  },
});

/** Set AI highlights on the editor. Call with empty array to clear. */
export function setAiHighlights(
  editor: { view: { dispatch: (tr: unknown) => void; state: { tr: unknown } } },
  highlights: AiHighlight[],
) {
  const { tr } = editor.view.state;
  if (highlights.length === 0) {
    (tr as { setMeta: (key: PluginKey, value: unknown) => unknown }).setMeta(
      aiHighlightPluginKey,
      { clear: true },
    );
  } else {
    (tr as { setMeta: (key: PluginKey, value: unknown) => unknown }).setMeta(
      aiHighlightPluginKey,
      { highlights },
    );
  }
  editor.view.dispatch(tr);
}
```

- [ ] **Step 2: Add CSS styles**

In `desktop-ui/src/styles/editor.css`, add:

```css
.ai-highlight {
  background: oklch(0.6962 0.1942 23.6149 / 0.12);
  border-radius: 3px;
  transition: background 0.3s ease;
  cursor: pointer;
}

.ai-highlight:hover {
  background: oklch(0.6962 0.1942 23.6149 / 0.22);
}
```

- [ ] **Step 3: Register extension in EditorCore**

In `desktop-ui/src/features/notes/components/editor/EditorCore.tsx`, add `AiHighlightExtension` to the extensions list in `getEditorExtensions()`:

```typescript
import { AiHighlightExtension } from "./extensions/AiHighlightPlugin";

// In getEditorExtensions():
AiHighlightExtension,
```

- [ ] **Step 4: Auto-fade after 10 seconds**

In the component that receives `TreePathRef` from chat responses, add an effect that sets highlights and clears them after 10 seconds:

```typescript
import { setAiHighlights } from "./extensions/AiHighlightPlugin";

// When a chat response references a note section:
useEffect(() => {
  if (!editor || !treePaths?.length) return;
  // Resolve node positions and set highlights
  // ... (position resolution depends on how nodeId maps to document positions)
  const timer = setTimeout(() => setAiHighlights(editor, []), 10_000);
  return () => clearTimeout(timer);
}, [editor, treePaths]);
```

- [ ] **Step 5: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(ui): add AI highlight decoration in Tiptap editor for matched note sections"
```

---

### Task 14: InsightForge Collapsible Tree View

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/StructureTreeView.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx`

- [ ] **Step 1: Create StructureTreeView component**

Create `desktop-ui/src/features/notes/components/insight/StructureTreeView.tsx`:

```tsx
import { useState } from "react";
import type { TreePathRef, PathSegment } from "@shared/types/tree-path";

interface TreeViewNode {
  segment: PathSegment;
  children: TreeViewNode[];
  matched: boolean;
  similarity?: number;
}

interface Props {
  paths: TreePathRef[];
}

export function StructureTreeView({ paths }: Props) {
  if (paths.length === 0) return null;

  // Build tree from flat paths
  const root = buildTree(paths);

  return (
    <div className="mt-3 rounded-lg border border-border/50 bg-surface-base/50 p-2.5 text-xs">
      <p className="text-muted mb-1.5 font-medium">Structure</p>
      {root.children.map((child) => (
        <TreeNodeView key={child.segment.nodeId} node={child} depth={0} />
      ))}
    </div>
  );
}

function TreeNodeView({ node, depth }: { node: TreeViewNode; depth: number }) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <button
        type="button"
        onClick={() => hasChildren && setExpanded(!expanded)}
        className={`flex items-center gap-1 py-0.5 w-full text-left hover:bg-surface-raised/50 rounded px-1
                    ${node.matched ? "text-foreground" : "text-muted"}`}
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
      >
        {hasChildren ? (
          <span className="opacity-50 text-[10px]">{expanded ? "▼" : "▶"}</span>
        ) : (
          <span className={`text-[10px] ${node.matched ? "text-brand" : "opacity-30"}`}>
            {node.matched ? "●" : "○"}
          </span>
        )}
        <span>{node.segment.title || "(untitled)"}</span>
        {node.matched && node.similarity != null && (
          <span className="ml-auto text-muted opacity-60">
            {(node.similarity * 100).toFixed(0)}%
          </span>
        )}
      </button>
      {expanded &&
        node.children.map((child) => (
          <TreeNodeView key={child.segment.nodeId} node={child} depth={depth + 1} />
        ))}
    </div>
  );
}

function buildTree(paths: TreePathRef[]): TreeViewNode {
  const root: TreeViewNode = {
    segment: { nodeId: "root", title: "Root", level: 0 },
    children: [],
    matched: false,
  };

  for (const path of paths) {
    let current = root;
    for (let i = 0; i < path.path.length; i++) {
      const seg = path.path[i];
      let child = current.children.find((c) => c.segment.nodeId === seg.nodeId);
      if (!child) {
        child = {
          segment: seg,
          children: [],
          matched: i === path.path.length - 1,
          similarity: i === path.path.length - 1 ? path.similarity : undefined,
        };
        current.children.push(child);
      }
      if (i === path.path.length - 1) {
        child.matched = true;
        child.similarity = path.similarity;
      }
      current = child;
    }
  }

  return root;
}
```

- [ ] **Step 2: Integrate into SynthesisTab**

In `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx`, add the tree view below the synthesis content when tree path data is available:

```tsx
import { StructureTreeView } from "./StructureTreeView";

// Below the MarkdownContent:
{synthesisResult?.treePaths && (
  <StructureTreeView paths={synthesisResult.treePaths} />
)}
```

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`
Expected: Clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(ui): add collapsible structure tree view in InsightForge synthesis tab"
```

---

### Task 15: Integration Test + Final Verification

**Files:**
- Create: `tests/e2e/hierarchical_notes.rs`

- [ ] **Step 1: Write integration test**

Create `tests/e2e/hierarchical_notes.rs`:

```rust
use klyntbot::*;
use common::Result;

#[tokio::test]
async fn test_note_tree_node_embedding_pipeline() -> Result<()> {
    let pool = storage::StoragePool::connect_in_memory().await?;
    // Run all migrations
    pool.run_feature_migrations(&cognitive::cognitive_migrations()).await?;
    pool.run_feature_migrations(&feature_notes::migrations()).await?;

    let repos = storage::Repos::from_pool(&pool);

    // Create a note with markdown content
    let note_id = "test-note-1";
    let markdown = "# Introduction\n\nSome intro text.\n\n## Background\n\nBackground info.\n\n## Methods\n\n- Step one\n- Step two\n- Step three";

    // Parse to tree
    let nodes = cognitive::repos::markdown_parser::parse_markdown_to_tree(note_id, markdown);

    // Should have: root + Introduction + Background + Methods + bullet pseudo-section
    assert!(nodes.len() >= 4, "Expected at least 4 nodes, got {}", nodes.len());

    // Verify hierarchy
    let root = nodes.iter().find(|n| n.level == 0).expect("root node");
    let h1 = nodes.iter().find(|n| n.title == "Introduction").expect("h1");
    assert_eq!(h1.level, 1);

    let h2_background = nodes.iter().find(|n| n.title == "Background").expect("h2");
    assert_eq!(h2_background.level, 2);

    Ok(())
}

#[tokio::test]
async fn test_tiptap_json_parser() -> Result<()> {
    let json = r#"{
        "type": "doc",
        "content": [
            {
                "type": "heading",
                "attrs": { "level": 1 },
                "content": [{ "type": "text", "text": "My Note" }]
            },
            {
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Content under heading" }]
            },
            {
                "type": "bulletList",
                "content": [
                    {
                        "type": "listItem",
                        "content": [
                            { "type": "paragraph", "content": [{ "type": "text", "text": "Bullet point" }] }
                        ]
                    }
                ]
            }
        ]
    }"#;

    let nodes = cognitive::services::tiptap_parser::parse_tiptap_json_to_tree("note-1", json);

    assert!(nodes.len() >= 2, "Expected at least root + heading, got {}", nodes.len());

    let heading = nodes.iter().find(|n| n.title == "My Note").expect("heading");
    assert_eq!(heading.level, 1);
    assert!(heading.content.contains("Content under heading"));

    let bullet = nodes.iter().find(|n| n.level == 7);
    assert!(bullet.is_some(), "Expected bullet pseudo-section");

    Ok(())
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo nextest run -E 'test(hierarchical_notes)'`
Expected: All PASS.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

- [ ] **Step 5: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add tests/e2e/hierarchical_notes.rs
git commit -m "test(e2e): add integration tests for hierarchical note tree parsing and embedding"
```

---

### Follow-up (not blocking Phase 1)

- **A/B metrics instrumentation:** Add `tree_retrieval_accuracy` analytics event to track structural recall@3, path accuracy, and latency delta. Wire into existing `enrichment_feedback` mechanism. This is needed for validating Phase 1 results but can be added after the core pipeline is functional.
- **Autotuner consumer wiring:** Wire `TrialParams.tree_top_k`, `tree_min_similarity`, `hybrid_bias` into `NoteTreeNavigator` via the `RwLock<Option<TrialParams>>` pattern (same as `UnifiedMemoryService`).
- **Entity tree linking:** The `NoteTreeBuilder` should call `SqliteGTLinkRepo::link_batch()` after parsing to connect entity mentions to tree nodes. This requires entity extraction output — integrate with existing `KnowledgeGraphService` entity extractor.
