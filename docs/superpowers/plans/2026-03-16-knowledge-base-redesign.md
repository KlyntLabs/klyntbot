# Knowledge Base Redesign Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Evolve the notes feature into a graph-first knowledge management system with AI suggestions, hybrid FTS5+semantic search, and deep entity integration.

**Architecture:** Two phases. **Phase 1** (this plan, 25 tasks): backend foundation, FTS5 search, frontend layout restructure, context panel, graph mode, editor improvements, note creation, inbox, version history. **Phase 2** (deferred, 6 tasks): embedding service, semantic search, AI suggestions backend, connected AI panel, unlinked mentions, global hotkey. Phase 1 delivers a fully functional knowledge base; Phase 2 adds the AI intelligence layer.

**Phase 1 delivers:** Three-panel layout, focus/graph modes, FTS5 search, backlinks panel, entity references, graph minimap, smart graph views, wiki-link creation, editable title, AI suggestions stub, inbox, version history overlay, tag explorer.

**Phase 2 delivers (deferred):** LanceDB note embeddings, semantic search, hybrid FTS5+semantic ranking, AI suggestions computation (4 signals), unlinked mentions, connected AI action buttons, global quick capture hotkey.

**Tech Stack:** Rust (SQLite FTS5, LanceDB embeddings), TypeScript/React (TipTap, D3-force), Tauri IPC, `diff` npm package for version diffs.

**Spec:** `docs/superpowers/specs/2026-03-16-knowledge-base-redesign.md`

---

## File Structure

### Backend — New/Modified Files

| File | Responsibility |
|------|---------------|
| `crates/feature-notes/migrations/001_create_notes.sql` | **Modify:** Consolidated schema with FTS5, inbox_items, embedding_updated_at |
| `crates/feature-notes/src/models.rs` | **Modify:** Add `InboxItem`, `InboxItemRow` structs |
| `crates/feature-notes/src/repo/notes.rs` | **Modify:** Replace LIKE search with FTS5, add pagination, tag filter |
| `crates/feature-notes/src/repo/links.rs` | **Modify:** Add `get_backlinks_with_context()`, suggestion signal queries |
| `crates/feature-notes/src/repo/tags.rs` | **Modify:** Add `get_all_tags()` for tag explorer |
| `crates/feature-notes/src/repo/inbox.rs` | **Create:** Inbox CRUD (create, list, delete) |
| `crates/feature-notes/src/repo/suggestions.rs` | **Create:** `find_structural_holes()`, `find_entity_cooccurrences()`, `find_tag_overlaps()` |
| `crates/feature-notes/src/tool.rs` | **Modify:** Add archive, backlinks, inbox, notebook hierarchy actions |
| `crates/feature-notes/src/lib.rs` | **Modify:** Bump migration version, register new repo modules |
| `crates/app-core/src/handlers/notes/crud.rs` | **Modify:** Add archive/unarchive, backlinks, unlinked mentions handlers |
| `crates/app-core/src/handlers/notes/inbox.rs` | **Create:** Inbox handlers (create, list, delete) |
| `crates/app-core/src/handlers/notes/suggestions.rs` | **Create:** `NoteSuggestionsService`, 4-signal computation |
| `crates/app-core/src/handlers/notes/embeddings.rs` | **Create:** `NoteEmbeddingService` using `Arc<dyn TextEmbedder>` |
| `crates/app-core/src/state.rs` | **Modify:** Add embedding service field to `AppCore` |
| `crates/desktop/src/commands/notes.rs` | **Modify:** Add new Tauri commands, update DEV_COMMANDS |
| `crates/desktop-shared/src/commands/notes.rs` | **Modify:** Add response/param types for new commands |

### Frontend — New Files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Top-level layout manager, three-mode switching |
| `desktop-ui/src/features/notes/components/NavigationSidebar.tsx` | Left panel: search, quick access, tags, notebooks |
| `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx` | Center panel wrapper, editable title, metadata line |
| `desktop-ui/src/features/notes/components/ContextPanel.tsx` | Right panel: orchestrates 4 sections + secondary accordion |
| `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx` | AI suggestions section with action buttons |
| `desktop-ui/src/features/notes/components/BacklinksPanel.tsx` | Backlinks + unlinked mentions section |
| `desktop-ui/src/features/notes/components/EntityReferencesPanel.tsx` | Cross-domain entity cards section |
| `desktop-ui/src/features/notes/components/GraphMinimap.tsx` | Small neighborhood graph for context panel |
| `desktop-ui/src/features/notes/components/TagsExplorer.tsx` | Tag cloud with click-to-filter |
| `desktop-ui/src/features/notes/components/QuickAccessList.tsx` | Pinned + recent notes list |
| `desktop-ui/src/features/notes/components/NoteCreationDialog.tsx` | AI-assisted creation modal |
| `desktop-ui/src/features/notes/components/InboxSection.tsx` | Inbox items list + triage UI |
| `desktop-ui/src/features/notes/components/VersionHistoryOverlay.tsx` | Redesigned version history with diff |
| `desktop-ui/src/features/notes/components/GraphToolbar.tsx` | Smart view selector, graph search, filters |
| `desktop-ui/src/features/notes/components/GraphNodeTooltip.tsx` | Hover tooltip for graph nodes |
| `desktop-ui/src/features/notes/components/LinkInsertDialog.tsx` | Custom link/image insertion dialog (replaces window.prompt) |
| `desktop-ui/src/shared/lib/tagColor.ts` | Deterministic tag-to-color mapping utility |
| `desktop-ui/src/features/notes/hooks/useNoteSuggestions.ts` | Fetches AI suggestions, refetches on save |
| `desktop-ui/src/features/notes/hooks/useBacklinks.ts` | Fetches backlinks + unlinked mentions |
| `desktop-ui/src/features/notes/hooks/useGraphData.ts` | Fetches graph data for smart views |
| `desktop-ui/src/features/notes/hooks/useInbox.ts` | Inbox items with mutation helpers |

### Frontend — Files to Remove

| File | Reason |
|------|--------|
| `desktop-ui/src/features/notes/components/NotesView.tsx` | Dead component (319 lines) |
| `desktop-ui/src/features/notes/components/NoteList.tsx` | Unused (100 lines) |
| `desktop-ui/src/features/notes/components/NoteCard.tsx` | Unused (120 lines) |
| `desktop-ui/src/features/notes/components/WorkspaceFileTree.tsx` | Moved out of notes (82 lines) |
| `desktop-ui/src/features/notes/components/AgentFileTree.tsx` | Moved out of notes (209 lines) |
| `desktop-ui/src/features/notes/components/AgentFrontmatterForm.tsx` | Moved out of notes (277 lines) |

---

## Chunk 1: Backend Foundation

### Task 1: Dead Code Cleanup (Frontend)

Remove unused components before restructuring to avoid confusion.

**Files:**
- Remove: `desktop-ui/src/features/notes/components/NotesView.tsx`
- Remove: `desktop-ui/src/features/notes/components/NoteList.tsx`
- Remove: `desktop-ui/src/features/notes/components/NoteCard.tsx`
- Modify: `desktop-ui/src/features/notes/index.ts`

- [ ] **Step 1: Remove dead components**

Delete files:
- `desktop-ui/src/features/notes/components/NotesView.tsx` (319 lines, exported but never imported by router)
- `desktop-ui/src/features/notes/components/NoteList.tsx` (100 lines, only imported by NotesView)
- `desktop-ui/src/features/notes/components/NoteCard.tsx` (120 lines, only imported by NoteList)

- [ ] **Step 2: Update index.ts exports**

Remove these lines from `desktop-ui/src/features/notes/index.ts`:
```typescript
// Remove these exports:
export { NoteCard } from "./components/NoteCard";
export { NoteList } from "./components/NoteList";
// NotesView is not exported as a named export (it's only used internally)
```

- [ ] **Step 3: Remove workspace/agent components from notes feature**

These components are being moved out of the notes feature entirely. For now, just remove them from the notes feature — they'll live in a future Agent Studio feature.

Delete files:
- `desktop-ui/src/features/notes/components/WorkspaceFileTree.tsx` (82 lines)
- `desktop-ui/src/features/notes/components/AgentFileTree.tsx` (209 lines)
- `desktop-ui/src/features/notes/components/AgentFrontmatterForm.tsx` (277 lines)

- [ ] **Step 4: Remove workspace/agent imports from NotesPage.tsx**

In `desktop-ui/src/features/notes/pages/NotesPage.tsx`, remove:
- All `activeWorkspaceFile`, `workspaceContent`, `activeAgentName`, `activeAgentFilename`, `agentFrontmatter`, `agentBody` state variables (lines 71–77)
- All workspace/agent handler callbacks (`handleSelectWorkspaceFile`, `handleSelectAgentFile`, `handleWorkspaceSave`, `handleAgentFileSave`)
- The workspace/agent sections in the JSX (the conditional rendering blocks for workspace and agent files)
- Import statements for the removed components

- [ ] **Step 5: Verify build passes**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds with no errors.

- [ ] **Step 6: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add -A desktop-ui/src/features/notes/
git commit -m "refactor(notes): remove dead components and workspace/agent code

Remove NotesView, NoteList, NoteCard (unused), and WorkspaceFileTree,
AgentFileTree, AgentFrontmatterForm (moving to future Agent Studio).
Strip workspace/agent state and handlers from NotesPage."
```

---

### Task 2: Schema Consolidation + FTS5 + Inbox

Rewrite the migration to add FTS5 virtual table, inbox_items, and embedding_updated_at.

**Files:**
- Modify: `crates/feature-notes/migrations/001_create_notes.sql`
- Modify: `crates/feature-notes/src/lib.rs` (bump migration version)

- [ ] **Step 1: Rewrite the migration**

Replace the entire contents of `crates/feature-notes/migrations/001_create_notes.sql`. The new schema keeps all 6 existing tables and adds:

1. `embedding_updated_at TEXT` column on `notes`
2. `notes_fts` FTS5 virtual table on `(title, body)` with porter tokenizer
3. FTS5 sync triggers (`notes_fts_insert`, `notes_fts_update`, `notes_fts_delete`)
4. `inbox_items` table with `id`, `content`, `status` (default 'pending'), `created_at`

```sql
-- Notebooks (hierarchical folders)
CREATE TABLE IF NOT EXISTS notebooks (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES notebooks(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    icon TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_notebooks_parent_id ON notebooks(parent_id);

-- Notes (core entity)
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    notebook_id TEXT REFERENCES notebooks(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    body TEXT,
    body_html TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    archived INTEGER NOT NULL DEFAULT 0,
    embedding_updated_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_notes_notebook_id ON notes(notebook_id);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(pinned) WHERE pinned = 1;
CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);
CREATE INDEX IF NOT EXISTS idx_notes_archived ON notes(archived) WHERE archived = 1;

-- Note tags (many-to-many)
CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);

-- Note links (directed graph edges)
CREATE TABLE IF NOT EXISTS note_links (
    source_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, target_id),
    CHECK (source_id != target_id)
);
CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_id);

-- Entity mentions (cross-domain references)
CREATE TABLE IF NOT EXISTS note_entity_mentions (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    PRIMARY KEY (note_id, entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_note_entity_mentions_entity ON note_entity_mentions(entity_type, entity_id);

-- Note versions (body snapshots)
CREATE TABLE IF NOT EXISTS note_versions (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_note_versions_note_id ON note_versions(note_id);

-- FTS5 full-text search index
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    body,
    content='notes',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- FTS5 sync triggers
CREATE TRIGGER IF NOT EXISTS notes_fts_insert AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;

CREATE TRIGGER IF NOT EXISTS notes_fts_update AFTER UPDATE OF title, body ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body);
    INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;

CREATE TRIGGER IF NOT EXISTS notes_fts_delete AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body);
END;

-- Quick capture inbox
CREATE TABLE IF NOT EXISTS inbox_items (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_inbox_items_status ON inbox_items(status);
```

- [ ] **Step 2: Bump migration version in lib.rs**

In `crates/feature-notes/src/lib.rs`, update the `FeatureMigration` version from 1 to 2:

```rust
fn migrations(&self) -> Vec<FeatureMigration> {
    vec![FeatureMigration {
        version: 2,
        sql: include_str!("../migrations/001_create_notes.sql"),
    }]
}
```

- [ ] **Step 3: Write test for FTS5 search**

Add to `crates/feature-notes/src/repo/mod.rs` tests:

```rust
#[tokio::test]
async fn test_fts5_search_basic() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    // Create notes with searchable content
    let id1 = repo.create_note("Rust Programming", Some("Learning about ownership and borrowing"), None).await.unwrap();
    let id2 = repo.create_note("Python Tips", Some("List comprehensions and generators"), None).await.unwrap();
    let id3 = repo.create_note("Rust Async", Some("Futures and tokio runtime"), None).await.unwrap();

    // FTS5 search should find notes matching "rust"
    let results = repo.search_fts("rust").await.unwrap();
    assert_eq!(results.len(), 2);
    // Title matches should score higher
    assert!(results[0].0.title == "Rust Programming" || results[0].0.title == "Rust Async");
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo nextest run -p feature-notes -E 'test(fts5_search_basic)'`
Expected: FAIL — `search_fts` method does not exist yet.

- [ ] **Step 5: Add NoteSearchResult struct**

Add to `crates/feature-notes/src/models.rs`:

```rust
/// Note row with FTS5 BM25 relevance rank.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteSearchResult {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub pinned: i32,
    pub archived: i32,
    pub embedding_updated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub rank: f64,
}
```

- [ ] **Step 6: Implement FTS5 search in repo**

Add to `crates/feature-notes/src/repo/notes.rs`:

```rust
use crate::models::NoteSearchResult;

/// Full-text search using FTS5 with BM25 ranking.
/// Returns NoteSearchResult with rank scores (bm25 returns negative values — lower is better).
/// Converts to positive scores where higher = better for downstream use.
pub async fn search_fts(&self, query: &str) -> Result<Vec<NoteSearchResult>> {
    let escaped = query.replace('"', "\"\"");
    let fts_query = format!("\"{}\"", escaped);

    let mut rows = sqlx::query_as::<_, NoteSearchResult>(
        r#"
        SELECT n.id, n.notebook_id, n.title, n.body, n.body_html,
               n.pinned, n.archived, n.embedding_updated_at, n.created_at, n.updated_at,
               bm25(notes_fts, 5.0, 1.0) as rank
        FROM notes_fts fts
        JOIN notes n ON n.rowid = fts.rowid
        WHERE notes_fts MATCH ?1
          AND n.archived = 0
        ORDER BY rank
        LIMIT 50
        "#,
    )
    .bind(&fts_query)
    .fetch_all(self.pool.reader())
    .await?;

    // bm25() returns negative values (lower = better match).
    // Convert to positive: negate, then min-max normalize so top result = 1.0.
    if !rows.is_empty() {
        let scores: Vec<f64> = rows.iter().map(|r| -r.rank).collect();
        let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_score = scores.iter().cloned().fold(f64::INFINITY, f64::min);
        let range = max_score - min_score;
        for (i, row) in rows.iter_mut().enumerate() {
            row.rank = if range > 0.0 {
                (scores[i] - min_score) / range
            } else {
                1.0
            };
        }
    }

    Ok(rows)
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo nextest run -p feature-notes -E 'test(fts5_search_basic)'`
Expected: PASS

- [ ] **Step 7: Add NoteRow embedding_updated_at field**

Update `NoteRow` in `crates/feature-notes/src/models.rs` to add the new column:

```rust
pub struct NoteRow {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub pinned: i32,
    pub archived: i32,
    pub embedding_updated_at: Option<String>,  // NEW
    pub created_at: String,
    pub updated_at: String,
}
```

Update all queries that `SELECT *` or list columns from `notes` to include `embedding_updated_at`.

- [ ] **Step 8: Add InboxItem model**

Add to `crates/feature-notes/src/models.rs`:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InboxItemRow {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct InboxItem {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<InboxItemRow> for InboxItem {
    fn from(row: InboxItemRow) -> Self {
        Self {
            id: row.id,
            content: row.content,
            status: row.status,
            created_at: row.created_at.parse().unwrap_or_default(),
        }
    }
}
```

- [ ] **Step 9: Verify all existing tests pass**

Run: `cargo nextest run -p feature-notes`
Expected: All existing tests pass (schema is backward-compatible since we're pre-release and replacing the migration).

- [ ] **Step 10: Commit**

```bash
git add crates/feature-notes/
git commit -m "feat(notes): consolidate schema with FTS5, inbox, and embedding tracking

Replace migration v1 with v2: adds FTS5 virtual table with porter
tokenizer, sync triggers, inbox_items table, embedding_updated_at
column, and archived index. Add InboxItem model and FTS5 search."
```

---

### Task 3: Inbox Repository

**Files:**
- Create: `crates/feature-notes/src/repo/inbox.rs`
- Modify: `crates/feature-notes/src/repo/mod.rs`

- [ ] **Step 1: Write failing tests for inbox CRUD**

Add to `crates/feature-notes/src/repo/mod.rs` tests:

```rust
#[tokio::test]
async fn test_inbox_create_and_list() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    let item = repo.create_inbox_item("Quick thought about architecture").await.unwrap();
    assert_eq!(item.content, "Quick thought about architecture");
    assert_eq!(item.status, "pending");

    let items = repo.list_inbox_items().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, item.id);
}

#[tokio::test]
async fn test_inbox_delete() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    let item = repo.create_inbox_item("Temporary thought").await.unwrap();
    repo.delete_inbox_item(&item.id).await.unwrap();

    let items = repo.list_inbox_items().await.unwrap();
    assert!(items.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-notes -E 'test(inbox)'`
Expected: FAIL — methods don't exist yet.

- [ ] **Step 3: Implement inbox repo**

Create `crates/feature-notes/src/repo/inbox.rs`:

```rust
use common::Result;
use uuid::Uuid;
use super::{NoteRepo, utc_now_str};
use crate::models::InboxItemRow;

impl NoteRepo {
    pub async fn create_inbox_item(&self, content: &str) -> Result<InboxItemRow> {
        let id = Uuid::new_v4().to_string();
        let now = utc_now_str();

        sqlx::query_as::<_, InboxItemRow>(
            "INSERT INTO inbox_items (id, content, status, created_at) VALUES (?1, ?2, 'pending', ?3) RETURNING *"
        )
        .bind(&id)
        .bind(content)
        .bind(&now)
        .fetch_one(self.pool.writer())
        .await
        .map_err(Into::into)
    }

    pub async fn list_inbox_items(&self) -> Result<Vec<InboxItemRow>> {
        sqlx::query_as::<_, InboxItemRow>(
            "SELECT * FROM inbox_items WHERE status = 'pending' ORDER BY created_at DESC"
        )
        .fetch_all(self.pool.reader())
        .await
        .map_err(Into::into)
    }

    pub async fn delete_inbox_item(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM inbox_items WHERE id = ?1")
            .bind(id)
            .execute(self.pool.writer())
            .await?;
        Ok(())
    }

    pub async fn count_inbox_items(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM inbox_items WHERE status = 'pending'"
        )
        .fetch_one(self.pool.reader())
        .await?;
        Ok(row.0)
    }
}
```

Register in `crates/feature-notes/src/repo/mod.rs`:
```rust
mod inbox;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p feature-notes -E 'test(inbox)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-notes/src/repo/inbox.rs crates/feature-notes/src/repo/mod.rs
git commit -m "feat(notes): add inbox repository for quick capture items"
```

---

### Task 4: Suggestion Signal Queries

Backend queries that power the AI suggestions panel: structural holes, entity co-occurrence, tag overlap.

**Files:**
- Create: `crates/feature-notes/src/repo/suggestions.rs`
- Modify: `crates/feature-notes/src/repo/mod.rs`

- [ ] **Step 1: Write failing tests**

Add to `crates/feature-notes/src/repo/mod.rs` tests:

```rust
#[tokio::test]
async fn test_find_structural_holes() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    // Create A -> B, A -> C, B -> D, C -> D
    // D is a structural hole for A (shares neighbors B and C but no direct link)
    let a = repo.create_note("Note A", None, None).await.unwrap();
    let b = repo.create_note("Note B", None, None).await.unwrap();
    let c = repo.create_note("Note C", None, None).await.unwrap();
    let d = repo.create_note("Note D", None, None).await.unwrap();

    repo.set_links(&a, &[b.clone(), c.clone()]).await.unwrap();
    repo.set_links(&b, &[d.clone()]).await.unwrap();
    repo.set_links(&c, &[d.clone()]).await.unwrap();

    let holes = repo.find_structural_holes(&a).await.unwrap();
    assert!(holes.iter().any(|(id, count)| id == &d && *count >= 2));
}

#[tokio::test]
async fn test_find_tag_overlaps() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    let a = repo.create_note("Note A", None, None).await.unwrap();
    let b = repo.create_note("Note B", None, None).await.unwrap();

    repo.set_tags(&a, &["rust".into(), "async".into()]).await.unwrap();
    repo.set_tags(&b, &["rust".into(), "async".into()]).await.unwrap();

    let overlaps = repo.find_tag_overlaps(&a).await.unwrap();
    assert!(overlaps.iter().any(|(id, count)| id == &b && *count == 2));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-notes -E 'test(structural_holes|tag_overlaps)'`
Expected: FAIL

- [ ] **Step 3: Implement suggestion queries**

Create `crates/feature-notes/src/repo/suggestions.rs`:

```rust
use common::Result;
use super::NoteRepo;

impl NoteRepo {
    /// Find notes that share graph neighbors with the given note but aren't directly linked.
    /// Returns (note_id, shared_neighbor_count) sorted by count descending.
    pub async fn find_structural_holes(&self, note_id: &str) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT nl2.target_id, COUNT(*) as shared
            FROM note_links nl1
            JOIN note_links nl2 ON nl2.source_id = nl1.target_id
            WHERE nl1.source_id = ?1
              AND nl2.target_id != ?1
              AND nl2.target_id NOT IN (SELECT target_id FROM note_links WHERE source_id = ?1)
            GROUP BY nl2.target_id
            HAVING shared >= 2
            ORDER BY shared DESC
            LIMIT 10
            "#,
        )
        .bind(note_id)
        .fetch_all(self.pool.reader())
        .await?;
        Ok(rows)
    }

    /// Find notes that mention the same entities as the given note.
    /// Returns (note_id, shared_entity_count) sorted by count descending.
    pub async fn find_entity_cooccurrences(&self, note_id: &str) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT nem2.note_id, COUNT(*) as shared
            FROM note_entity_mentions nem1
            JOIN note_entity_mentions nem2
              ON nem2.entity_type = nem1.entity_type
              AND nem2.entity_id = nem1.entity_id
            WHERE nem1.note_id = ?1
              AND nem2.note_id != ?1
            GROUP BY nem2.note_id
            ORDER BY shared DESC
            LIMIT 10
            "#,
        )
        .bind(note_id)
        .fetch_all(self.pool.reader())
        .await?;
        Ok(rows)
    }

    /// Find notes that share tags with the given note but aren't linked.
    /// Returns (note_id, shared_tag_count) sorted by count descending.
    pub async fn find_tag_overlaps(&self, note_id: &str) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT nt2.note_id, COUNT(*) as shared
            FROM note_tags nt1
            JOIN note_tags nt2 ON nt2.tag = nt1.tag
            WHERE nt1.note_id = ?1
              AND nt2.note_id != ?1
              AND nt2.note_id NOT IN (SELECT target_id FROM note_links WHERE source_id = ?1)
            GROUP BY nt2.note_id
            ORDER BY shared DESC
            LIMIT 10
            "#,
        )
        .bind(note_id)
        .fetch_all(self.pool.reader())
        .await?;
        Ok(rows)
    }

    /// Get all unique tags with their usage counts, sorted by count descending.
    pub async fn get_all_tags(&self) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT tag, COUNT(*) as count FROM note_tags GROUP BY tag ORDER BY count DESC"
        )
        .fetch_all(self.pool.reader())
        .await?;
        Ok(rows)
    }
}
```

Register in `crates/feature-notes/src/repo/mod.rs`:
```rust
mod suggestions;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p feature-notes -E 'test(structural_holes|tag_overlaps)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-notes/src/repo/suggestions.rs crates/feature-notes/src/repo/mod.rs
git commit -m "feat(notes): add suggestion signal queries (structural holes, entity co-occurrence, tag overlap)"
```

---

### Task 5: Backlinks + Unlinked Mentions + Pagination

**Files:**
- Modify: `crates/feature-notes/src/repo/links.rs`
- Modify: `crates/feature-notes/src/repo/notes.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn test_get_backlinks_with_context() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    let target = repo.create_note("Target Note", None, None).await.unwrap();
    let source = repo.create_note("Source Note", Some("This links to [[Target Note]] in context"), None).await.unwrap();
    repo.set_links(&source, &[target.clone()]).await.unwrap();

    let backlinks = repo.get_backlinks_with_context(&target).await.unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].0.id, source);
}

#[tokio::test]
async fn test_list_notes_with_pagination() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    for i in 0..10 {
        repo.create_note(&format!("Note {}", i), None, None).await.unwrap();
    }

    let page1 = repo.list_notes_paginated(None, None, 5, 0).await.unwrap();
    assert_eq!(page1.len(), 5);

    let page2 = repo.list_notes_paginated(None, None, 5, 5).await.unwrap();
    assert_eq!(page2.len(), 5);
}

#[tokio::test]
async fn test_list_notes_with_tag_filter() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    NotesFeature::default().run_migrations(&pool).await.unwrap();
    let repo = NoteRepo::new(pool);

    let a = repo.create_note("Rust Note", None, None).await.unwrap();
    let b = repo.create_note("Python Note", None, None).await.unwrap();
    repo.set_tags(&a, &["rust".into()]).await.unwrap();
    repo.set_tags(&b, &["python".into()]).await.unwrap();

    let results = repo.list_notes_paginated(None, Some(&["rust".to_string()]), 50, 0).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Note");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-notes -E 'test(backlinks_with_context|pagination|tag_filter)'`
Expected: FAIL

- [ ] **Step 3: Implement backlinks with context**

Add to `crates/feature-notes/src/repo/links.rs`:

```rust
/// Get notes that link TO the given note, with the source note's full row.
pub async fn get_backlinks_with_context(&self, note_id: &str) -> Result<Vec<(NoteRow, Option<String>)>> {
    // Returns the source note row plus a context snippet (the body text around the link)
    let rows: Vec<NoteRow> = sqlx::query_as(
        r#"
        SELECT n.*
        FROM note_links nl
        JOIN notes n ON n.id = nl.source_id
        WHERE nl.target_id = ?1
          AND n.archived = 0
        ORDER BY n.updated_at DESC
        "#,
    )
    .bind(note_id)
    .fetch_all(self.pool.reader())
    .await?;

    // Extract context snippet from each source note's body
    Ok(rows.into_iter().map(|row| {
        let context = row.body.as_ref().and_then(|body| {
            // Find the sentence containing [[ that links to this note
            body.lines()
                .find(|line| line.contains("[["))
                .map(|line| line.trim().to_string())
        });
        (row, context)
    }).collect())
}
```

- [ ] **Step 4: Implement paginated list with tag filter**

Add to `crates/feature-notes/src/repo/notes.rs`:

```rust
/// List notes with optional notebook filter, tag filter, pagination.
pub async fn list_notes_paginated(
    &self,
    notebook_id: Option<&str>,
    tags: Option<&[String]>,
    limit: i64,
    offset: i64,
) -> Result<Vec<NoteRow>> {
    let mut sql = String::from(
        "SELECT n.* FROM notes n WHERE n.archived = 0"
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(nb_id) = notebook_id {
        sql.push_str(" AND n.notebook_id = ?");
        binds.push(nb_id.to_string());
    }

    if let Some(tag_list) = tags {
        if !tag_list.is_empty() {
            for tag in tag_list {
                sql.push_str(
                    " AND n.id IN (SELECT note_id FROM note_tags WHERE tag = ?)"
                );
                binds.push(tag.clone());
            }
        }
    }

    sql.push_str(" ORDER BY n.pinned DESC, n.updated_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query_as::<_, NoteRow>(&sql);
    for bind in &binds {
        query = query.bind(bind);
    }
    query = query.bind(limit).bind(offset);

    query.fetch_all(self.pool.reader()).await.map_err(Into::into)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p feature-notes -E 'test(backlinks_with_context|pagination|tag_filter)'`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo nextest run -p feature-notes`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-notes/src/repo/
git commit -m "feat(notes): add backlinks with context, paginated list, and tag filtering"
```

---

### Task 6: Archive/Unarchive + Tags All + Inbox Tauri Commands

**Depends on:** Task 3 (inbox repo), Task 4 (get_all_tags), Task 5 (backlinks)

**Files:**
- Modify: `crates/app-core/src/handlers/notes/crud.rs`
- Create: `crates/app-core/src/handlers/notes/inbox.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`

- [ ] **Step 1: Add archive handlers to app-core**

In `crates/app-core/src/handlers/notes/crud.rs`, add:

```rust
pub async fn note_archive(&self, id: &str) -> Result<EntityUpdates> {
    sqlx::query("UPDATE notes SET archived = 1, updated_at = ?2 WHERE id = ?1")
        .bind(id)
        .bind(utc_now_str())
        .execute(self.note_repo.pool.writer())
        .await?;
    Ok(EntityUpdates::single(EntityKind::Note, id))
}

pub async fn note_unarchive(&self, id: &str) -> Result<EntityUpdates> {
    sqlx::query("UPDATE notes SET archived = 0, updated_at = ?2 WHERE id = ?1")
        .bind(id)
        .bind(utc_now_str())
        .execute(self.note_repo.pool.writer())
        .await?;
    Ok(EntityUpdates::single(EntityKind::Note, id))
}

pub async fn note_list_archived(&self) -> Result<Vec<NoteResponse>> {
    let rows = sqlx::query_as::<_, NoteRow>(
        "SELECT * FROM notes WHERE archived = 1 ORDER BY updated_at DESC"
    )
    .fetch_all(self.note_repo.pool.reader())
    .await?;
    notes_with_tags_batch(&self.note_repo, rows).await
}
```

- [ ] **Step 2: Add IPC types for new commands**

In `crates/desktop-shared/src/commands/notes.rs`, add:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxCreateParams {
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkResponse {
    pub note: NoteResponse,
    pub context: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSuggestionsResponse {
    pub related_notes: Vec<ScoredNote>,
    pub link_suggestions: Vec<LinkSuggestion>,
    pub suggested_tags: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredNote {
    pub note: NoteResponse,
    pub score: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkSuggestion {
    pub note: NoteResponse,
    pub score: f64,
    pub reason: String,
}
```

- [ ] **Step 3: Add Tauri commands**

In `crates/desktop/src/commands/notes.rs`, add the new commands:

```rust
#[tauri::command]
pub async fn note_archive(state: State<'_, AppCore>, app: AppHandle, id: String) -> Result<(), String> {
    let updates = state.note_archive(&id).await.map_err(|e| e.to_string())?;
    emit_updates(&app, &updates);
    Ok(())
}

#[tauri::command]
pub async fn note_unarchive(state: State<'_, AppCore>, app: AppHandle, id: String) -> Result<(), String> {
    let updates = state.note_unarchive(&id).await.map_err(|e| e.to_string())?;
    emit_updates(&app, &updates);
    Ok(())
}

#[tauri::command]
pub async fn note_list_archived(state: State<'_, AppCore>) -> Result<Vec<NoteResponse>, String> {
    state.note_list_archived().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_backlinks(state: State<'_, AppCore>, id: String) -> Result<Vec<BacklinkResponse>, String> {
    state.note_backlinks(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inbox_create(state: State<'_, AppCore>, params: InboxCreateParams) -> Result<InboxItemResponse, String> {
    state.inbox_create(&params.content).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inbox_list(state: State<'_, AppCore>) -> Result<Vec<InboxItemResponse>, String> {
    state.inbox_list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inbox_delete(state: State<'_, AppCore>, id: String) -> Result<(), String> {
    state.inbox_delete(&id).await.map_err(|e| e.to_string())
}
```

Also add `note_tags_all` (calls `repo.get_all_tags()` from Task 4) and `note_unlinked_mentions` (stub returning empty vec — Phase 2 Task D5 implements real logic).

Update `DEV_COMMANDS` to include all new command names: `note_archive`, `note_unarchive`, `note_list_archived`, `note_backlinks`, `note_tags_all`, `note_unlinked_mentions`, `inbox_create`, `inbox_list`, `inbox_delete`.

- [ ] **Step 4: Add backlinks handler to app-core**

In `crates/app-core/src/handlers/notes/crud.rs`:

```rust
pub async fn note_backlinks(&self, note_id: &str) -> Result<Vec<BacklinkResponse>> {
    let backlinks = self.note_repo.get_backlinks_with_context(note_id).await?;
    let mut responses = Vec::new();
    for (row, context) in backlinks {
        let tags = self.note_repo.get_tags(&row.id).await.unwrap_or_default();
        responses.push(BacklinkResponse {
            note: note_row_to_response(row, tags),
            context,
        });
    }
    Ok(responses)
}
```

- [ ] **Step 5: Add inbox handlers to app-core**

Create `crates/app-core/src/handlers/notes/inbox.rs`:

```rust
use common::Result;
use crate::state::AppCore;
use desktop_shared::commands::notes::{InboxItemResponse, InboxCreateParams};

impl AppCore {
    pub async fn inbox_create(&self, content: &str) -> Result<InboxItemResponse> {
        let item = self.note_repo.create_inbox_item(content).await?;
        Ok(InboxItemResponse {
            id: item.id,
            content: item.content,
            status: item.status,
            created_at: item.created_at,
        })
    }

    pub async fn inbox_list(&self) -> Result<Vec<InboxItemResponse>> {
        let items = self.note_repo.list_inbox_items().await?;
        Ok(items.into_iter().map(|item| InboxItemResponse {
            id: item.id,
            content: item.content,
            status: item.status,
            created_at: item.created_at,
        }).collect())
    }

    pub async fn inbox_delete(&self, id: &str) -> Result<()> {
        self.note_repo.delete_inbox_item(id).await
    }
}
```

Register mod in `crates/app-core/src/handlers/notes/mod.rs`:
```rust
mod inbox;
```

- [ ] **Step 6: Verify build and tests pass**

Run: `cargo build --workspace && cargo nextest run -p feature-notes -p app-core`
Expected: Build succeeds, all tests pass.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-notes/ crates/app-core/ crates/desktop/ crates/desktop-shared/
git commit -m "feat(notes): add archive, backlinks, inbox Tauri commands and handlers"
```

---

## Chunk 2: Frontend Layout Restructure

### Task 7: Tag Color Utility

Shared utility used across all tag-related UI. Build this first since many components depend on it.

**Files:**
- Create: `desktop-ui/src/shared/lib/tagColor.ts`

- [ ] **Step 1: Create the utility**

```typescript
// Deterministic tag-to-color mapping. Hash-based, consistent across all UI.
const TAG_PALETTE = [
  '#a78bfa', // violet
  '#93c5fd', // blue
  '#6ee7b7', // green
  '#fcd34d', // amber
  '#fca5a5', // red
  '#f9a8d4', // pink
  '#a5b4fc', // indigo
  '#67e8f9', // cyan
  '#fdba74', // orange
  '#86efac', // emerald
  '#c4b5fd', // purple
  '#fde68a', // yellow
];

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash |= 0;
  }
  return Math.abs(hash);
}

export function tagColor(tagName: string): string {
  return TAG_PALETTE[hashString(tagName) % TAG_PALETTE.length];
}

export function tagBgColor(tagName: string): string {
  return `${tagColor(tagName)}25`; // 25 = ~15% opacity hex suffix
}
```

- [ ] **Step 2: Write tests for tagColor**

Create `desktop-ui/src/shared/lib/tagColor.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { tagColor, tagBgColor } from "./tagColor";

describe("tagColor", () => {
  it("returns a consistent color for the same tag", () => {
    expect(tagColor("rust")).toBe(tagColor("rust"));
    expect(tagColor("python")).toBe(tagColor("python"));
  });

  it("returns different colors for different tags", () => {
    // Not guaranteed but highly likely with distinct strings
    const colors = new Set(["rust", "python", "go", "java", "ruby"].map(tagColor));
    expect(colors.size).toBeGreaterThan(1);
  });

  it("returns a valid hex color", () => {
    expect(tagColor("test")).toMatch(/^#[0-9a-fA-F]{6}$/);
  });

  it("tagBgColor returns color with opacity suffix", () => {
    expect(tagBgColor("test")).toMatch(/^#[0-9a-fA-F]{6}25$/);
  });
});
```

- [ ] **Step 3: Run tests**

Run: `cd desktop-ui && bun run test -- tagColor`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/lib/tagColor.ts desktop-ui/src/shared/lib/tagColor.test.ts
git commit -m "feat(notes): add deterministic tag-to-color mapping utility with tests"
```

---

### Task 8: KnowledgeBasePage Layout Shell

Replace `NotesPage` with the new three-mode layout manager.

**Files:**
- Create: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`
- Modify: `desktop-ui/src/features/notes/index.ts`
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: Create KnowledgeBasePage**

This is the top-level layout component managing three modes: three-panel (default), focus, and graph. It holds the core state (selected note, view mode) and delegates to child panels.

```typescript
import { useState, useCallback, useEffect, useRef } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import { useMutation } from "@shared/hooks/useMutation";
import { useEvent } from "@shared/hooks/useEvent";
import { useSearchParams } from "react-router-dom";
import { NoteEditor } from "../components/NoteEditor";
import { NavigationSidebar } from "../components/NavigationSidebar";
import { ContextPanel } from "../components/ContextPanel";
import { GraphView } from "../components/GraphView";

type ViewMode = "editor" | "graph";
type LayoutMode = "three-panel" | "focus";

interface Note { id: string; title: string; /* ... */ }
interface Notebook { id: string; title: string; /* ... */ }

export function KnowledgeBasePage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("editor");
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("three-panel");

  // Panel widths
  const [leftWidth, setLeftWidth] = useState(220);
  const [rightWidth, setRightWidth] = useState(260);

  // Data
  const { data: notes, refetch: refetchNotes } = useQuery<Note[]>("note_list", undefined, []);
  const { data: notebooks, refetch: refetchNotebooks } = useQuery<Notebook[]>("notebook_list", undefined, []);
  const { mutate: createNote } = useMutation<Note>("note_create", "params");
  const { mutate: updateNote } = useMutation<Note>("note_update", "params");
  const { mutate: deleteNote } = useMutation<boolean>("note_delete");

  // URL param navigation
  useEffect(() => {
    const noteId = searchParams.get("noteId");
    if (noteId) {
      setSelectedNoteId(noteId);
      setViewMode("editor");
      setSearchParams({}, { replace: true });
    }
  }, [searchParams]);

  // Entity update listener
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "note") refetchNotes();
    if (payload.entityKind === "notebook") { refetchNotebooks(); refetchNotes(); }
  });

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.shiftKey && e.key === "Enter") {
        e.preventDefault();
        setLayoutMode((m) => m === "focus" ? "three-panel" : "focus");
      }
      if (e.metaKey && e.shiftKey && e.key === "g") {
        e.preventDefault();
        setViewMode((m) => m === "graph" ? "editor" : "graph");
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  const selectedNote = notes.find((n) => n.id === selectedNoteId) ?? null;
  const isFocusMode = layoutMode === "focus";
  const isGraphMode = viewMode === "graph";

  return (
    <div className="flex flex-1 min-w-0 h-full">
      {/* Left sidebar — hidden in focus mode */}
      {!isFocusMode && (
        <NavigationSidebar
          width={leftWidth}
          onWidthChange={setLeftWidth}
          notes={notes}
          notebooks={notebooks}
          selectedNoteId={selectedNoteId}
          onSelectNote={setSelectedNoteId}
          onCreateNote={createNote}
          onDeleteNote={deleteNote}
        />
      )}

      {/* Center — editor or graph */}
      <div className="flex-1 min-w-0 flex flex-col">
        {isGraphMode ? (
          <GraphView
            notes={notes}
            selectedNoteId={selectedNoteId}
            onSelectNote={(id) => {
              setSelectedNoteId(id);
              // In graph mode, clicking selects for preview — don't switch to editor
            }}
            onOpenInEditor={(id) => {
              setSelectedNoteId(id);
              setViewMode("editor");
            }}
          />
        ) : selectedNote ? (
          <NoteEditor
            note={selectedNote}
            onSave={(params) => updateNote(params)}
            onNavigateNote={(id) => {
              setSelectedNoteId(id);
              setViewMode("editor");
            }}
          />
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted">
            <div className="text-center">
              <p>Select a note or press <kbd className="px-1.5 py-0.5 rounded bg-white/5 text-xs">⌘N</kbd> to create one</p>
            </div>
          </div>
        )}
      </div>

      {/* Right context panel — hidden in focus mode */}
      {!isFocusMode && (
        <ContextPanel
          width={rightWidth}
          onWidthChange={setRightWidth}
          noteId={selectedNoteId}
          isGraphMode={isGraphMode}
          selectedNote={selectedNote}
          onSelectNote={(id) => {
            setSelectedNoteId(id);
            if (isGraphMode) setViewMode("editor");
          }}
        />
      )}
    </div>
  );
}
```

This is a starting skeleton — the child components will be built in subsequent tasks. The key architecture is established: three-panel layout with mode switching via state.

- [ ] **Step 2: Create stub NavigationSidebar**

Create `desktop-ui/src/features/notes/components/NavigationSidebar.tsx` with a minimal stub that renders the existing FileTree:

```typescript
import { FileTree } from "./FileTree";

interface NavigationSidebarProps {
  width: number;
  onWidthChange: (w: number) => void;
  notes: any[];
  notebooks: any[];
  selectedNoteId: string | null;
  onSelectNote: (id: string) => void;
  onCreateNote: (params: any) => void;
  onDeleteNote: (params: any) => void;
}

export function NavigationSidebar({ width, notes, notebooks, selectedNoteId, onSelectNote }: NavigationSidebarProps) {
  return (
    <div style={{ width }} className="border-r border-border flex flex-col flex-shrink-0">
      {/* Placeholder — will be replaced with search, quick access, tags, notebooks */}
      <div className="p-3 text-xs text-muted">Navigation sidebar (WIP)</div>
    </div>
  );
}
```

- [ ] **Step 3: Create stub ContextPanel**

Create `desktop-ui/src/features/notes/components/ContextPanel.tsx`:

```typescript
interface ContextPanelProps {
  width: number;
  onWidthChange: (w: number) => void;
  noteId: string | null;
  isGraphMode: boolean;
  selectedNote: any | null;
  onSelectNote: (id: string) => void;
}

export function ContextPanel({ width, noteId, isGraphMode }: ContextPanelProps) {
  return (
    <div style={{ width }} className="border-l border-border flex flex-col flex-shrink-0 overflow-y-auto">
      {isGraphMode ? (
        <div className="p-3 text-xs text-muted">Note preview (graph mode)</div>
      ) : noteId ? (
        <div className="p-3 text-xs text-muted">Context panel (WIP)</div>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 4: Update router and exports**

In `desktop-ui/src/app/router.tsx`, update the lazy import:
```typescript
const KnowledgeBasePage = lazy(() =>
  import("../features/notes").then((m) => ({ default: m.KnowledgeBasePage }))
);
```

And the route:
```typescript
{ path: "/notes", element: <KnowledgeBasePage /> }
```

In `desktop-ui/src/features/notes/index.ts`, add the export:
```typescript
export { KnowledgeBasePage } from "./pages/KnowledgeBasePage";
```

Remove the `NotesPage` export (or keep as alias temporarily).

- [ ] **Step 5: Verify dev server runs**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds. The page renders with the three-panel skeleton.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/
git commit -m "feat(notes): create KnowledgeBasePage layout shell with three-mode switching

Three-panel default, focus mode (Cmd+Shift+Enter), graph mode
(Cmd+Shift+G). Stub NavigationSidebar and ContextPanel."
```

---

### Task 9: NavigationSidebar — Search + Quick Access + Tags + Notebooks

Build the full left sidebar with all four sections.

**Files:**
- Rewrite: `desktop-ui/src/features/notes/components/NavigationSidebar.tsx`
- Create: `desktop-ui/src/features/notes/components/TagsExplorer.tsx`
- Create: `desktop-ui/src/features/notes/components/QuickAccessList.tsx`

- [ ] **Step 1: Create QuickAccessList**

```typescript
interface QuickAccessListProps {
  notes: Note[];
  selectedNoteId: string | null;
  onSelectNote: (id: string) => void;
}

export function QuickAccessList({ notes, selectedNoteId, onSelectNote }: QuickAccessListProps) {
  const pinnedNotes = notes.filter((n) => n.pinned);
  const recentNotes = notes
    .filter((n) => !n.pinned)
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, 8);

  return (
    <div className="px-3 py-2">
      <div className="text-[10px] uppercase tracking-wider text-dim mb-1.5">Quick Access</div>
      {pinnedNotes.map((note) => (
        <button
          key={note.id}
          onClick={() => onSelectNote(note.id)}
          className={`w-full text-left px-2 py-1 rounded text-sm truncate ${
            note.id === selectedNoteId ? "bg-white/[0.08] text-primary" : "text-secondary hover:bg-white/[0.04]"
          }`}
        >
          📌 {note.title}
        </button>
      ))}
      {recentNotes.map((note) => (
        <button
          key={note.id}
          onClick={() => onSelectNote(note.id)}
          className={`w-full text-left px-2 py-1 rounded text-sm truncate ${
            note.id === selectedNoteId ? "bg-white/[0.08] text-primary" : "text-muted hover:bg-white/[0.04]"
          }`}
        >
          🕐 {note.title}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Create TagsExplorer**

```typescript
import { tagColor, tagBgColor } from "@shared/lib/tagColor";
import { useQuery } from "@shared/hooks/useQuery";

interface TagsExplorerProps {
  activeTags: string[];
  onToggleTag: (tag: string, additive: boolean) => void;
}

export function TagsExplorer({ activeTags, onToggleTag }: TagsExplorerProps) {
  const { data: tags } = useQuery<[string, number][]>("note_tags_all", undefined, []);

  return (
    <div className="px-3 py-2 border-t border-border/50">
      <div className="text-[10px] uppercase tracking-wider text-dim mb-1.5">Tags</div>
      <div className="flex flex-wrap gap-1">
        {tags.map(([tag, count]) => {
          const isActive = activeTags.includes(tag);
          return (
            <button
              key={tag}
              onClick={(e) => onToggleTag(tag, e.metaKey)}
              className="px-2 py-0.5 rounded-full text-xs transition-colors"
              style={{
                backgroundColor: isActive ? tagColor(tag) + "40" : tagBgColor(tag),
                color: tagColor(tag),
                border: isActive ? `1px solid ${tagColor(tag)}60` : "1px solid transparent",
              }}
            >
              #{tag}
            </button>
          );
        })}
      </div>
    </div>
  );
}
```

Note: This requires a new Tauri command `note_tags_all` that calls `repo.get_all_tags()`. Add it to the desktop commands alongside the other new commands.

- [ ] **Step 3: Rewrite NavigationSidebar**

Rewrite `NavigationSidebar.tsx` to compose all four sections: search bar, quick access, tags explorer, and collapsible notebooks (reuse existing `FileTree` for the notebook section, stripped of workspace/agent trees).

The sidebar should:
- Have a search input at the top that calls `note_search` with 200ms debounce
- When search is active, replace all sections with search results
- `Escape` clears search
- Quick access shows pinned + recent
- Tags explorer shows tag cloud with click-to-filter
- Notebooks section is collapsible, collapsed by default
- Footer shows note count and inbox badge

- [ ] **Step 4: Wire into KnowledgeBasePage**

Update `KnowledgeBasePage.tsx` to pass the required props to `NavigationSidebar` and handle tag filtering state.

- [ ] **Step 5: Verify build and visual check**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds. Sidebar renders with all four sections.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): build NavigationSidebar with search, quick access, tags, notebooks"
```

---

### Task 10: Editable Title + Metadata Line in Editor

The note header gets an editable title (contentEditable) and metadata line.

**Files:**
- Create: `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Create NoteEditorPanel wrapper**

This wraps `NoteEditor` with the editable title, inline tags, and metadata line above it.

The editable title uses a `contentEditable` div that:
- Auto-focuses on empty titles
- Saves on blur or Enter key
- Shows as large, prominent text
- Prevents newlines (Enter triggers blur)

The metadata line shows: notebook breadcrumb (if any), created date, word count.

- [ ] **Step 2: Move title editing from FileTree to editor header**

Currently titles can only be renamed from the FileTree via inline rename. The new editable title in the editor header becomes the primary way to edit titles. The FileTree's inline rename can remain as a secondary method.

- [ ] **Step 3: Update NoteEditor to not show the old read-only title**

In `NoteEditor.tsx`, remove the `<span>` that shows the title at line 239 (the read-only display). The title is now managed by `NoteEditorPanel`.

- [ ] **Step 4: Verify build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add editable title and metadata line to editor header"
```

---

## Chunk 3: Context Panel

### Task 11: BacklinksPanel

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useBacklinks.ts`
- Create: `desktop-ui/src/features/notes/components/BacklinksPanel.tsx`

- [ ] **Step 1: Create hooks directory and useBacklinks hook**

First create the hooks directory: `mkdir -p desktop-ui/src/features/notes/hooks`

```typescript
import { useQuery } from "@shared/hooks/useQuery";

interface Backlink {
  note: { id: string; title: string; tags: string[]; updatedAt: string };
  context: string | null;
}

export function useBacklinks(noteId: string | null) {
  const { data, refetch } = useQuery<Backlink[]>(
    "note_backlinks",
    noteId ? { id: noteId } : null,
    []
  );
  return { backlinks: data, refetchBacklinks: refetch };
}
```

- [ ] **Step 2: Create BacklinksPanel component**

Shows backlinks with context snippets. Each entry has: note title, 1-line context, date. Click navigates. Below backlinks, show unlinked mentions (future — stub for now).

- [ ] **Step 3: Wire into ContextPanel**

Update `ContextPanel.tsx` to render `BacklinksPanel` as the second section.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add BacklinksPanel to context panel"
```

---

### Task 12: EntityReferencesPanel

**Files:**
- Create: `desktop-ui/src/features/notes/components/EntityReferencesPanel.tsx`

- [ ] **Step 1: Create EntityReferencesPanel**

Uses existing `note_list_by_entity` IPC (already exists). Queries tasks and projects that are mentioned in the current note via `note_entity_mentions`. Shows clickable entity cards grouped by type (Tasks, Projects) with status pills.

- [ ] **Step 2: Wire into ContextPanel**

Third section in `ContextPanel.tsx`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add EntityReferencesPanel to context panel"
```

---

### Task 13: GraphMinimap

**Files:**
- Create: `desktop-ui/src/features/notes/components/GraphMinimap.tsx`

- [ ] **Step 1: Create GraphMinimap**

Small D3-force SVG showing the current note's 1-2 hop neighborhood. Reuse the D3 setup from `GraphView.tsx` but simplified:
- Fixed size (matches context panel width × 120px height)
- Current note centered with highlight glow
- Nodes colored by primary tag using `tagColor()`
- Click a node to navigate
- "Expand" link at bottom-right switches to graph mode

Fetch neighborhood data by filtering `note_links_all` results to only include links within N hops of the current note (client-side filtering).

- [ ] **Step 2: Wire into ContextPanel**

Fourth section in `ContextPanel.tsx`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add GraphMinimap to context panel"
```

---

### Task 14: AISuggestionsPanel (Stub)

The AI suggestions panel depends on the embedding service (Phase 2 backend work). For now, create the UI shell that will be connected once the backend is ready.

**Files:**
- Create: `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx`
- Create: `desktop-ui/src/features/notes/hooks/useNoteSuggestions.ts`

- [ ] **Step 1: Create useNoteSuggestions hook**

```typescript
import { useQuery } from "@shared/hooks/useQuery";

export function useNoteSuggestions(noteId: string | null) {
  const { data, refetch } = useQuery(
    "note_suggestions",
    noteId ? { id: noteId } : null,
    { relatedNotes: [], linkSuggestions: [], suggestedTags: [] }
  );
  return { suggestions: data, refetchSuggestions: refetch };
}
```

- [ ] **Step 2: Create AISuggestionsPanel**

Shows three sub-sections:
1. Related Notes — list of semantically similar notes (clickable)
2. Link Suggestions — "Consider linking to X" with reasoning text
3. Suggested Tags — ghost pills with `+` to accept

Action buttons at the bottom: Synthesize, Ask AI, Create linked note.

For now, the panel shows "AI suggestions will appear here" placeholder when no data is available (the backend command doesn't exist yet).

- [ ] **Step 3: Wire into ContextPanel as first section**

Update `ContextPanel.tsx` to render `AISuggestionsPanel` as the first (top) section with purple accent styling.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add AISuggestionsPanel stub to context panel"
```

---

### Task 15: Assemble ContextPanel

**Files:**
- Modify: `desktop-ui/src/features/notes/components/ContextPanel.tsx`

- [ ] **Step 1: Assemble all four sections**

Rewrite `ContextPanel.tsx` to compose all four sections in order:
1. AISuggestionsPanel (expanded, purple accent)
2. BacklinksPanel (expanded)
3. EntityReferencesPanel (expanded)
4. GraphMinimap (expanded)

Plus a "More" accordion at the bottom with:
- Table of Contents (auto-generated from TipTap headings — listen to editor content)
- Note Metadata (dates, word count, version count)

Each section should be collapsible with a header that toggles visibility.

In Graph Mode, replace all sections with a read-only note preview pane showing the selected note's rendered body with an "Open in editor" button.

- [ ] **Step 2: Verify the full three-panel layout**

Run: `cd desktop-ui && bun run dev`
Open `localhost:1420/notes`. Verify:
- Left sidebar with search, quick access, tags, notebooks
- Center editor with editable title
- Right panel with all four context sections
- Cmd+Shift+Enter toggles focus mode (both panels collapse)
- Cmd+Shift+G toggles graph mode (editor replaced with graph, right panel shows preview)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): assemble full ContextPanel with all four sections + graph mode preview"
```

---

## Chunk 4: Graph Mode + Editor Improvements

### Task 16: Graph Mode Redesign — Smart Views + Interactions

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`
- Create: `desktop-ui/src/features/notes/components/GraphToolbar.tsx`
- Create: `desktop-ui/src/features/notes/components/GraphNodeTooltip.tsx`
- Create: `desktop-ui/src/features/notes/hooks/useGraphData.ts`

- [ ] **Step 1: Create GraphToolbar**

Toolbar at the top of the graph area with:
- Smart view selector buttons: Local (default), Full, By Tag, By Notebook, Orphans
- Hop radius slider (1-3) for Local view
- Search input for graph filtering

- [ ] **Step 2: Create useGraphData hook**

```typescript
export function useGraphData(view: SmartView, noteId: string | null, hopRadius: number) {
  const { data: allLinks } = useQuery("note_links_all", undefined, []);
  // Filter links based on view type:
  // - "local": BFS from noteId up to hopRadius hops
  // - "full": all links
  // - "by-tag": all links, group nodes by primary tag
  // - "by-notebook": all links, group by notebook
  // - "orphans": only notes with zero links
  // Return { nodes, links } filtered for the current view
}
```

- [ ] **Step 3: Create GraphNodeTooltip**

Hover tooltip showing: note title, first 2 lines of body, tag pills, link count. Positioned near the cursor. Uses a portal to `document.body`.

- [ ] **Step 4: Refactor GraphView for smart views**

Modify `GraphView.tsx`:
- Add `GraphToolbar` at the top
- Use `useGraphData` instead of raw `note_links_all`
- Node colors from `tagColor(primaryTag)` instead of hash-based palette
- Add hover handler that shows `GraphNodeTooltip`
- Click shows preview in context panel (call `onSelectNote`)
- Double-click calls `onOpenInEditor`
- Right-click context menu: "Open in editor", "Link to current note", "Show neighborhood", "Delete"
- Hover dims non-neighbor nodes to 20% opacity

- [ ] **Step 5: Verify graph mode**

Run dev server, open `/notes`, press Cmd+Shift+G. Verify:
- Smart view toolbar renders
- Local view shows neighborhood of selected note
- Hover shows tooltip
- Click shows preview in right panel
- Double-click returns to editor

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): redesign graph mode with smart views, tooltips, and interactions"
```

---

### Task 17: Wiki-Link Creation for Non-Existent Notes

The most requested missing feature: typing `[[New Title]]` and being able to create the note inline.

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/WikiLinkNode.tsx`

- [ ] **Step 1: Modify WikiLinkMenu to show "Create" option**

In `WikiLinkNode.tsx`, modify the search results rendering (around line 272):

When `results.length === 0` and `state.query.length > 0`, show a "Create 'query'" button at the top of the menu instead of "No matching notes":

```typescript
{results.length === 0 && state.query.length > 0 && (
  <button
    onClick={() => handleCreateAndLink(state.query)}
    className="w-full text-left px-3 py-2 text-sm hover:bg-white/[0.06] text-primary"
  >
    ✨ Create "{state.query}"
  </button>
)}
```

Also show it as the first result even when there ARE results, so the user can always create a new note:

```typescript
{state.query.length > 0 && (
  <button
    onClick={() => handleCreateAndLink(state.query)}
    className="w-full text-left px-3 py-2 text-sm hover:bg-white/[0.06] text-primary border-b border-border/50"
  >
    ✨ Create "{state.query}"
  </button>
)}
```

- [ ] **Step 2: Implement handleCreateAndLink (bidirectional)**

```typescript
const handleCreateAndLink = async (title: string) => {
  // Create the note with body containing a backlink to the current note
  const currentTitle = currentNote?.title ?? "Untitled";
  const newNote = await ipc<Note>("note_create", {
    params: {
      title,
      notebookId: currentNote?.notebookId ?? null,
      body: `Linked from [[${currentTitle}]]`,
    }
  });
  if (newNote) {
    // Insert wiki-link mark in the current note pointing to the new note
    insertWikiLink({ id: newNote.id, title: newNote.title });
    // The backlink [[currentTitle]] in the new note's body will be auto-extracted
    // by the extract_links_and_mentions pipeline on the next save, creating the
    // bidirectional link as required by Spec Section 8.3.
  }
};
```

- [ ] **Step 3: Test the flow manually**

1. Open a note, type `[[Nonexistent Note`
2. Autocomplete should show "Create 'Nonexistent Note'" option
3. Click it — note is created, wiki-link inserted
4. The new note appears in the sidebar

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/WikiLinkNode.tsx
git commit -m "feat(notes): add wiki-link creation for non-existent notes"
```

---

### Task 18: Replace window.prompt with Custom Dialogs

**Files:**
- Create: `desktop-ui/src/features/notes/components/LinkInsertDialog.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx`

- [ ] **Step 1: Create LinkInsertDialog**

A modal dialog for inserting links and images, replacing `window.prompt()`:

```typescript
interface LinkInsertDialogProps {
  type: "link" | "image";
  isOpen: boolean;
  onClose: () => void;
  onInsert: (url: string) => void;
}

export function LinkInsertDialog({ type, isOpen, onClose, onInsert }: LinkInsertDialogProps) {
  const [url, setUrl] = useState("");
  // Render a glass-panel dialog with URL input, preview (for images), and Insert/Cancel buttons
  // Auto-focus the input on open
  // Enter key submits, Escape closes
}
```

- [ ] **Step 2: Replace window.prompt calls in EditorToolbar**

In `EditorToolbar.tsx`, replace the link button handler (around line 145) and image button handler that use `window.prompt("URL:")` with state that opens `LinkInsertDialog`.

- [ ] **Step 3: Verify dialogs work**

Click the link button in the toolbar — custom dialog should appear instead of browser prompt.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): replace window.prompt with custom link/image insert dialogs"
```

---

### Task 19: Toolbar Mode Buttons

Add Focus Mode, Graph Mode, and Version History buttons to the editor toolbar.

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx`

- [ ] **Step 1: Add mode toggle buttons**

In the right side of the toolbar (after the existing vim toggle), add:
- Focus mode button (⛶ icon or "Focus" text)
- Graph mode button (graph icon or "Graph" text)
- Version history button (clock icon)

These call callbacks passed from `KnowledgeBasePage` via `NoteEditorPanel`.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx
git commit -m "feat(notes): add focus mode, graph mode, history buttons to toolbar"
```

---

## Chunk 5: Note Creation + Inbox

### Task 20: Note Creation Dialog

**Files:**
- Create: `desktop-ui/src/features/notes/components/NoteCreationDialog.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Create NoteCreationDialog**

Modal dialog triggered by Cmd+N:
1. Title input (auto-focus)
2. After 3+ characters, shows:
   - Similar existing notes (via `note_search` IPC, 200ms debounce)
   - A "Create" button and a "Create blank" button
3. On create, calls `note_create` and navigates to the new note

The AI suggestions (suggested tags, notebook, links) are a v2 enhancement that depends on the embedding service. For now, just show similar notes for duplicate prevention.

- [ ] **Step 2: Wire Cmd+N to open the dialog**

In `KnowledgeBasePage.tsx`, add dialog state and keyboard handler:

```typescript
const [showCreateDialog, setShowCreateDialog] = useState(false);

// In the keyboard handler:
if (e.metaKey && !e.shiftKey && e.key === "n") {
  e.preventDefault();
  setShowCreateDialog(true);
}
if (e.metaKey && e.shiftKey && e.key === "n") {
  e.preventDefault();
  // Quick create blank note
  handleCreateBlankNote();
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add AI-assisted note creation dialog (Cmd+N)"
```

---

### Task 21: Inbox Section in Sidebar

**Files:**
- Create: `desktop-ui/src/features/notes/components/InboxSection.tsx`
- Create: `desktop-ui/src/features/notes/hooks/useInbox.ts`
- Modify: `desktop-ui/src/features/notes/components/NavigationSidebar.tsx`

- [ ] **Step 1: Create useInbox hook**

```typescript
import { useQuery } from "@shared/hooks/useQuery";
import { useMutation } from "@shared/hooks/useMutation";

export function useInbox() {
  const { data: items, refetch } = useQuery("inbox_list", undefined, []);
  const { mutate: createItem } = useMutation("inbox_create", "params");
  const { mutate: deleteItem } = useMutation("inbox_delete");

  return { items, refetch, createItem, deleteItem };
}
```

- [ ] **Step 2: Create InboxSection**

Shows inbox items with triage actions:
- "Create as note" — opens NoteCreationDialog with content pre-filled
- "Discard" — deletes the inbox item
- Badge count in sidebar footer

- [ ] **Step 3: Wire into NavigationSidebar footer**

Show inbox badge in the sidebar footer. Clicking it expands/shows the inbox section.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): add inbox section to sidebar with triage UI"
```

---

## Chunk 6: Version History + Polish

### Task 22: Version History Overlay

Replace the current `NoteVersionHistory` panel with a full-screen overlay.

**Files:**
- Create: `desktop-ui/src/features/notes/components/VersionHistoryOverlay.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Install diff package**

Run: `cd desktop-ui && bun add diff`

- [ ] **Step 2: Create VersionHistoryOverlay**

Full-screen overlay triggered by Cmd+Shift+H:
- Left side: vertical timeline of version snapshots
  - Each entry: relative timestamp, word count delta
  - Diff summary computed on-demand using `diffWords` from `diff` package
- Right side: rendered preview of selected version (TipTap read-only)
  - Toggle for inline diff view (red/green highlighting via `diffLines`)
- "Restore this version" button
- Escape or X to close

- [ ] **Step 3: Wire Cmd+Shift+H keyboard shortcut**

In `KnowledgeBasePage.tsx`:
```typescript
if (e.metaKey && e.shiftKey && e.key === "h") {
  e.preventDefault();
  setShowVersionHistory((v) => !v);
}
```

- [ ] **Step 4: Remove old NoteVersionHistory from NoteEditor**

The old `NoteVersionHistory` component (114 lines) rendered inline in the editor. Remove the toggle and rendering from `NoteEditor.tsx`. The new overlay replaces it entirely.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): redesign version history as full-screen overlay with diff"
```

---

### Task 23: Config Values Read at Runtime

Fix the hardcoded version config values.

**Files:**
- Modify: `crates/feature-notes/src/lib.rs`
- Modify: `crates/app-core/src/handlers/notes/crud.rs`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Read maxVersionsPerNote from config in backend**

In `crates/app-core/src/handlers/notes/crud.rs`, replace the hardcoded `50` at line 234 with a value read from the feature config:

```rust
let max_versions = self.config
    .get_feature_config("notes")
    .and_then(|c| c.get("maxVersionsPerNote"))
    .and_then(|v| v.as_i64())
    .unwrap_or(50) as usize;
```

- [ ] **Step 2: Expose version cooldown to frontend**

Add a Tauri command `note_config` that returns the notes feature config so the frontend can read `versionCooldownMinutes`. Or simpler: include it in an existing response.

- [ ] **Step 3: Use config value in NoteEditor.tsx**

Replace the hardcoded `VERSION_INTERVAL_MS = 5 * 60 * 1000` at line 17 with a value from the backend config (or keep the hardcoded default and note that it should eventually be configurable).

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/ crates/feature-notes/ desktop-ui/
git commit -m "fix(notes): read maxVersionsPerNote and versionCooldown from config at runtime"
```

---

### Task 24: NotesTool Action Additions

Add new tool actions for the LLM/MCP interface.

**Files:**
- Modify: `crates/feature-notes/src/tool.rs`

- [ ] **Step 1: Write test for archive action**

Add to tool.rs tests:

```rust
#[tokio::test]
async fn test_archive_and_unarchive() {
    // Setup pool, run migrations, create tool
    let result = tool.execute(json!({ "action": "archive_note", "id": note_id }), &ctx).await.unwrap();
    assert!(result.contains("archived"));

    // Note should not appear in list_notes
    let list = tool.execute(json!({ "action": "list_notes" }), &ctx).await.unwrap();
    assert!(!list.contains(&note_id));

    // Unarchive
    tool.execute(json!({ "action": "unarchive_note", "id": note_id }), &ctx).await.unwrap();
    let list = tool.execute(json!({ "action": "list_notes" }), &ctx).await.unwrap();
    assert!(list.contains(&note_id));
}
```

- [ ] **Step 2: Implement new tool actions**

Add to the `execute()` match in `tool.rs`:

```rust
"archive_note" => self.handle_archive_note(params).await,
"unarchive_note" => self.handle_unarchive_note(params).await,
"list_archived" => self.handle_list_archived().await,
"get_backlinks" => self.handle_get_backlinks(params).await,
"capture_inbox" => self.handle_capture_inbox(params).await,
"list_inbox" => self.handle_list_inbox().await,
"update_notebook" => self.handle_update_notebook(params).await,
```

Update the tool description and parameters schema to include all new actions.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-notes`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-notes/src/tool.rs
git commit -m "feat(notes): add archive, backlinks, inbox, notebook tool actions"
```

---

### Task 25: Final Integration + Cleanup

**Files:**
- Modify: `desktop-ui/src/features/notes/index.ts`
- Modify: `desktop-ui/src/styles/editor.css`

- [ ] **Step 1: Update index.ts exports**

Clean up exports to reflect the new component structure:

```typescript
// Pages
export { KnowledgeBasePage } from "./pages/KnowledgeBasePage";

// Components used by other features
export { LinkedNotes } from "./components/LinkedNotes";
export { NoteEditor } from "./components/NoteEditor";

// Editor components
export { EditorToolbar } from "./components/editor/EditorToolbar";
export { VimCommandLine } from "./components/editor/VimCommandLine";
export { VimStatusLine } from "./components/editor/VimStatusLine";
```

- [ ] **Step 2: Remove old NotesPage.tsx**

Delete `desktop-ui/src/features/notes/pages/NotesPage.tsx` — replaced by `KnowledgeBasePage.tsx`.

- [ ] **Step 3: Update editor.css for new components**

Add CSS for:
- Context panel sections (collapsible animations)
- AI suggestions purple accent styling
- Graph minimap container
- Version history overlay

- [ ] **Step 4: Run full build and lint**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

- [ ] **Step 5: Run all backend tests**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/ desktop-ui/src/styles/editor.css
git commit -m "feat(notes): complete knowledge base redesign — layout, context panel, graph, search, inbox

Major restructure of the notes feature into a graph-first knowledge
management system with three-panel layout, context panel (AI suggestions,
backlinks, entity refs, graph minimap), smart graph views, FTS5 search,
wiki-link creation, version history overlay, and inbox quick capture."
```

---

## Phase 2: AI Intelligence Layer (Deferred)

These tasks depend on the `NoteEmbeddingService` which requires LanceDB integration. Phase 2 covers **Spec Sections 4.1 (AI Suggestions computation), 6.2 (Semantic Search), 6.3 (Hybrid scoring), 4.2 (Unlinked Mentions), and 8.2 (Global Hotkey)**. Phase 1 delivers stubs/UI shells for these features; Phase 2 connects them to real data.

**Execution order:** D1 → D2 → D3 → D4 (these are sequential). D5 and D6 can run in parallel with D4.

**Frontend files deferred:** `desktop-ui/src/features/notes/components/QuickCaptureWindow.tsx` (created in Task D6).

### Task D1: NoteEmbeddingService in app-core
- Create `crates/app-core/src/handlers/notes/embeddings.rs`
- Inject `Arc<dyn TextEmbedder>` via dependency inversion
- `embed_note()`: compute embedding, upsert into LanceDB `note_embeddings` table
- `find_similar()`: vector similarity search
- Async fire-and-forget on save, sync query on note open

### Task D2: NoteSuggestionsService
- Create `crates/app-core/src/handlers/notes/suggestions.rs`
- Orchestrate 4 signals (embedding similarity, structural holes, entity co-occurrence, tag overlap)
- Merge into unified score, return top 5 related + link suggestions + suggested tags
- New Tauri command `note_suggestions`

### Task D3: Semantic Search Layer
- Add `search_semantic()` to NoteRepo using LanceDB
- Implement hybrid search merging FTS5 + semantic with weighted scoring
- Update the sidebar search to show both "Exact" and "Related" result groups

### Task D4: Connect AISuggestionsPanel to Backend
- Update `useNoteSuggestions` hook to call `note_suggestions` command
- Wire refetch on save events
- Connect action buttons (Synthesize, Ask AI, Create linked note) to agent pipeline

### Task D5: Unlinked Mentions
- Implement `get_unlinked_mentions()` using FTS5 phrase matching
- Add to BacklinksPanel below the backlinks list
- One-click to convert mention to wiki-link

### Task D5: Unlinked Mentions
- Implement `get_unlinked_mentions()` in NoteRepo using FTS5 phrase matching
- Constraints: 3+ words or 8+ chars title, exclude notes with existing `[[Title]]`
- Replace the stub in `note_unlinked_mentions` Tauri command
- Add to BacklinksPanel below the backlinks list with one-click "Link" button

### Task D6: Quick Capture Global Hotkey
- Integrate `tauri-plugin-global-shortcut` for Cmd+Shift+C system-wide
- Create `QuickCaptureWindow.tsx` (Tauri secondary window)
- Handle macOS accessibility permissions

### Also deferred to Phase 2
- **Today's note** (Spec Section 2.2): Auto-created daily note on first app launch. Config key `notes.dailyNote.enabled`. Low complexity but not core to Phase 1.
- **Save status indicator** (Spec Section 3.4): "Saved 2s ago" text in status bar. Can be added to NoteEditorPanel.
- **Auto-save trigger for AI suggestions** (Spec Section 3.5): Wire `note_update` handler to push updated suggestions to frontend after async embedding completes.
