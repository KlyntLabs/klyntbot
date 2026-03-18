# Active Learning Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the note editor into an AI-powered learning surface with inline annotations, per-section perspectives, floating toolbar, context menu, and cognitive memory integration.

**Architecture:** Smart Hybrid — TipTap marks for annotation anchoring, React components for all interactive UI, lightweight JSON storage for perspective state. Minimal backend changes (column additions to existing tables), maximal frontend flexibility. Optimistic mark application with IPC rollback.

**Tech Stack:** Rust (app-core handlers, cognitive repos), TipTap 3 (AnnotationMark, BubbleMenu, UniqueID), React (popovers, menus, perspectives), Radix UI (ContextMenu), React Query (caching), SQLite (annotations + notes tables).

**Spec:** `docs/superpowers/specs/2026-03-18-active-learning-surface-design.md`

---

## File Structure

### Backend — New/Modified Files

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Modify | Add 5 columns to annotations table, update UNIQUE constraint |
| `crates/cognitive/src/types.rs` | Modify | Add new fields to `Annotation` struct |
| `crates/cognitive/src/repos/annotation.rs` | Modify | Update CRUD methods for new fields, add `list_for_note` |
| `crates/feature-notes/migrations/001_create_notes.sql` | Modify | Add `perspective_config`, `last_visited_at` columns |
| `crates/feature-notes/src/models.rs` | Modify | Add new fields to `NoteRow` |
| `crates/feature-notes/src/repo/notes.rs` | Modify | Include new columns in queries, add `update_perspective_config` |
| `crates/desktop-shared/src/commands/notes.rs` | Modify | Add fields to `NoteResponse`, `NoteUpdateParams` |
| `crates/desktop-shared/src/commands/annotations.rs` | Create | `AnnotationCreateParams`, `AnnotationResponse`, `LinkedContextResponse` |
| `crates/app-core/src/handlers/annotations.rs` | Create | Annotation CRUD + AI suggestion + linked context handlers |
| `crates/app-core/src/handlers/mod.rs` | Modify | Register annotations module |
| `crates/desktop/src/commands/annotations.rs` | Create | Tauri IPC commands for annotations + linked context |
| `crates/desktop/src/commands/mod.rs` | Modify | Register annotations module + invoke handlers |
| `crates/desktop/src/dev_server/mod.rs` | Modify | Add annotations DEV_COMMANDS coverage |

### Frontend — New Files

| File | Responsibility |
|------|----------------|
| `desktop-ui/src/features/notes/components/editor/AnnotationMark.ts` | TipTap custom mark extension |
| `desktop-ui/src/features/notes/components/editor/BubbleToolbar.tsx` | Floating toolbar (AI actions on selection) |
| `desktop-ui/src/features/notes/components/editor/EditorContextMenu.tsx` | Right-click context menu with grouped actions |
| `desktop-ui/src/features/notes/components/AnnotationPopover.tsx` | Glassmorphic annotation viewer/editor |
| `desktop-ui/src/features/notes/components/LinkedViewPanel.tsx` | Cognitive memory results panel |
| `desktop-ui/src/features/notes/components/PerspectiveOverlay.tsx` | Contextual split-pane perspective renderer |
| `desktop-ui/src/features/notes/components/perspectives/AnnotatedView.tsx` | Annotated perspective view |
| `desktop-ui/src/features/notes/components/perspectives/StudyModeView.tsx` | Study Mode perspective view |
| `desktop-ui/src/features/notes/hooks/useAnnotations.ts` | Annotation CRUD + mark sync |
| `desktop-ui/src/features/notes/hooks/usePerspective.ts` | Perspective state + AI result caching |
| `desktop-ui/src/features/notes/hooks/useLinkedContext.ts` | Fetches cognitive memory context |
| `desktop-ui/src/features/notes/hooks/useEditorActions.ts` | Shared action handlers (Annotate, Flashcard, Translate, Ask AI) |

### Frontend — Modified Files

| File | Changes |
|------|---------|
| `desktop-ui/src/features/notes/components/editor/EditorCore.tsx` | Register AnnotationMark + UniqueID extensions |
| `desktop-ui/src/features/notes/components/NoteEditor.tsx` | Integrate BubbleToolbar, EditorContextMenu, perspective state |
| `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx` | Contextual right pane based on active perspective |
| `desktop-ui/src/features/notes/components/editor/editor.css` | Annotation highlight styles, perspective badge styles |
| `desktop-ui/package.json` | Add `ulid` (UniqueID implemented inline — see Task 7) |
| `crates/feature-notes/src/models.rs` | Also update `NoteSearchResult` struct |

---

## Task 1: Schema — Annotations Table Changes

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql:253-301`
- Modify: `crates/cognitive/src/types.rs:83-96`

- [ ] **Step 1: Update annotations table DDL in migration**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, update the `CREATE TABLE annotations` block. Add the 5 new columns. Replace the UNIQUE constraint from `UNIQUE(target_type, target_id, content)` to remove it entirely (pre-release, duplicates are acceptable — we'll add a new constraint after mark_id is populated).

Add these columns after `access_count`:
```sql
mark_id TEXT,
quoted_text TEXT,
range_start INTEGER,
range_end INTEGER,
ai_suggestion TEXT,
```

Remove the line: `UNIQUE(target_type, target_id, content)`

- [ ] **Step 2: Update Annotation struct in types.rs**

In `crates/cognitive/src/types.rs`, add the new fields to the `Annotation` struct after `access_count`:

```rust
pub mark_id: Option<String>,
pub quoted_text: Option<String>,
pub range_start: Option<i64>,
pub range_end: Option<i64>,
pub ai_suggestion: Option<String>,
```

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build -p cognitive`
Expected: Compilation succeeds (repo code will need updates next — build errors in annotation.rs are expected and addressed in Task 3).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/types.rs
git commit -m "feat(cognitive): add annotation anchoring columns to schema + types"
```

---

## Task 2: Schema — Notes Table Changes

**Files:**
- Modify: `crates/feature-notes/migrations/001_create_notes.sql:15-30`
- Modify: `crates/feature-notes/src/models.rs:66-82`
- Modify: `crates/desktop-shared/src/commands/notes.rs:5-57`

- [ ] **Step 1: Add columns to notes migration**

In `crates/feature-notes/migrations/001_create_notes.sql`, add to the `CREATE TABLE notes` block (after `split_mode`):

```sql
perspective_config TEXT,
last_visited_at TEXT,
```

- [ ] **Step 2: Add fields to NoteRow**

In `crates/feature-notes/src/models.rs`, add to the `NoteRow` struct (after `split_mode`):

```rust
pub perspective_config: Option<String>,
pub last_visited_at: Option<String>,
```

- [ ] **Step 3: Add fields to NoteResponse**

In `crates/desktop-shared/src/commands/notes.rs`, add to `NoteResponse` (after split fields):

```rust
pub perspective_config: Option<String>,
pub last_visited_at: Option<String>,
```

- [ ] **Step 4: Add fields to NoteUpdateParams**

In `crates/desktop-shared/src/commands/notes.rs`, add to `NoteUpdateParams` (after split fields):

```rust
#[serde(default, deserialize_with = "deserialize_nullable_field")]
pub perspective_config: Option<Option<String>>,
```

- [ ] **Step 5: Update NoteSearchResult struct**

In `crates/feature-notes/src/models.rs`, also add the same fields to `NoteSearchResult` (used by FTS5 search joins — `SELECT *` will include the new columns):

```rust
pub perspective_config: Option<String>,
pub last_visited_at: Option<String>,
```

- [ ] **Step 6: Update NoteRow → NoteResponse conversion**

Find the conversion code in `crates/app-core/src/handlers/notes/converters.rs` that maps `NoteRow` to `NoteResponse`. Add:

```rust
perspective_config: row.perspective_config.clone(),
last_visited_at: row.last_visited_at.clone(),
```

- [ ] **Step 7: Update note queries to include new columns**

In `crates/feature-notes/src/repo/notes.rs`, update all SELECT queries that read from the `notes` table to include `perspective_config` and `last_visited_at`. Also update the INSERT in `create_note` to include the new columns (both default to NULL). Update `update_note` to handle `perspective_config` via the existing nullable sentinel pattern.

- [ ] **Step 8: Build and test**

Run: `cargo build -p feature-notes -p desktop-shared -p app-core`
Run: `cargo nextest run -p feature-notes`
Expected: All pass.

- [ ] **Step 9: Commit**

```bash
git add crates/feature-notes/ crates/desktop-shared/ crates/app-core/src/handlers/notes/converters.rs
git commit -m "feat(notes): add perspective_config and last_visited_at columns"
```

---

## Task 3: Backend — Annotation Repository Updates

**Files:**
- Modify: `crates/cognitive/src/repos/annotation.rs`
- Test: `crates/cognitive/src/repos/annotation.rs` (inline tests)

- [ ] **Step 1: Write test for annotation creation with new fields**

Add a test in `crates/cognitive/src/repos/annotation.rs`:

```rust
#[tokio::test]
async fn test_create_annotation_with_mark_id() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = AnnotationRepo::new(pool.clone());

    let annotation = Annotation {
        id: "ann-1".into(),
        target_type: "note".into(),
        target_id: "note-123".into(),
        content: "My annotation".into(),
        tags: "learning".into(),
        author: "user".into(),
        priority: 0,
        created_at: "2026-03-18T00:00:00Z".into(),
        updated_at: "2026-03-18T00:00:00Z".into(),
        expires_at: None,
        access_count: 0,
        mark_id: Some("mark-abc".into()),
        quoted_text: Some("selected text".into()),
        range_start: Some(42),
        range_end: Some(55),
        ai_suggestion: Some("Related to X".into()),
    };

    repo.upsert(&annotation).await.unwrap();
    let results = repo.get_for_target("note", "note-123").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].mark_id, Some("mark-abc".into()));
    assert_eq!(results[0].quoted_text, Some("selected text".into()));
    assert_eq!(results[0].range_start, Some(42));
    assert_eq!(results[0].range_end, Some(55));
    assert_eq!(results[0].ai_suggestion, Some("Related to X".into()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_create_annotation_with_mark_id)'`
Expected: FAIL — upsert doesn't include new columns.

- [ ] **Step 3: Update upsert method**

In `annotation.rs`, update the `upsert` method's INSERT statement to include all new columns: `mark_id`, `quoted_text`, `range_start`, `range_end`, `ai_suggestion`. Also update the ON CONFLICT clause to SET these new fields.

- [ ] **Step 4: Update get_for_target and other SELECT queries**

Update all SELECT queries in the repo to include the 5 new columns. Update the row scanning/mapping to populate the new `Annotation` fields.

- [ ] **Step 5: Add list_for_note method**

Add a new method for fetching annotations by note ID with a limit. Note: the repo stores a plain `SqlitePool`, not a `StoragePool`, so the correct accessor is `&self.pool`:

```rust
pub async fn list_for_note(&self, note_id: &str, limit: i64) -> Result<Vec<Annotation>> {
    let rows = sqlx::query_as::<_, Annotation>(
        "SELECT * FROM annotations WHERE target_type = 'note' AND target_id = ?1 ORDER BY created_at DESC LIMIT ?2"
    )
    .bind(note_id)
    .bind(limit)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p cognitive`
Expected: All pass, including the new test.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/repos/annotation.rs
git commit -m "feat(cognitive): update annotation repo with mark anchoring fields"
```

---

## Task 4: Backend — Shared Types for Annotation IPC

**Files:**
- Create: `crates/desktop-shared/src/commands/annotations.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`

- [ ] **Step 1: Create annotation shared types**

Create `crates/desktop-shared/src/commands/annotations.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationCreateParams {
    pub note_id: String,
    pub mark_id: String,
    pub content: String,
    pub quoted_text: Option<String>,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub ai_suggestion: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationUpdateParams {
    pub id: String,
    pub content: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationResponse {
    pub id: String,
    pub note_id: String,
    pub mark_id: Option<String>,
    pub content: String,
    pub quoted_text: Option<String>,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub ai_suggestion: Option<String>,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedContextParams {
    pub note_id: String,
    pub section_text: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkedContextResponse {
    pub semantic_facts: Vec<LinkedFact>,
    pub episodic_memories: Vec<LinkedMemory>,
    pub related_annotations: Vec<AnnotationResponse>,
    pub procedural_rules: Vec<LinkedRule>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkedFact {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source_note: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkedMemory {
    pub id: String,
    pub content: String,
    pub domain: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkedRule {
    pub id: String,
    pub rule_text: String,
    pub domain: String,
    pub signal_count: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiSuggestionResponse {
    pub suggestion: Option<String>,
    pub confidence: f64,
    pub related_fact_ids: Vec<String>,
}
```

- [ ] **Step 2: Register module in mod.rs**

In `crates/desktop-shared/src/commands/mod.rs`, add: `pub mod annotations;`

- [ ] **Step 3: Build to verify**

Run: `cargo build -p desktop-shared`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/annotations.rs crates/desktop-shared/src/commands/mod.rs
git commit -m "feat(desktop-shared): add annotation and linked context IPC types"
```

---

## Task 5: Backend — App-Core Annotation Handlers

**Files:**
- Create: `crates/app-core/src/handlers/annotations.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`

- [ ] **Step 1: Write test for annotation repo interaction (not AppCore)**

Testing annotation handlers at the repo level (not AppCore — AppCore has a heavy construction footprint requiring agent loop, channel manager, etc.). Add a test in `crates/cognitive/src/repos/annotation.rs`:

```rust
#[tokio::test]
async fn test_list_for_note() {
    let pool = cognitive_test_pool().await;
    let repo = AnnotationRepo::new(pool);
    // Create 2 annotations for same note
    let ann1 = Annotation { id: "ann-1".into(), target_type: "note".into(), target_id: "note-1".into(), mark_id: Some("m1".into()), /* ... */ };
    let ann2 = Annotation { id: "ann-2".into(), target_type: "note".into(), target_id: "note-1".into(), mark_id: Some("m2".into()), /* ... */ };
    repo.upsert(&ann1).await.unwrap();
    repo.upsert(&ann2).await.unwrap();
    let results = repo.list_for_note("note-1", 10).await.unwrap();
    assert_eq!(results.len(), 2);
}
```

- [ ] **Step 2: Implement annotation handlers**

Create `crates/app-core/src/handlers/annotations.rs` with these methods on `AppCore`.

**Critical pattern:** Cognitive repos are NOT on the `Repos` struct. They must be constructed from `self.storage_pool.inner().clone()`:

```rust
impl AppCore {
    pub async fn annotation_create(&self, params: AnnotationCreateParams) -> Result<AnnotationResponse> {
        let repo = AnnotationRepo::new(self.storage_pool.inner().clone());
        let now = chrono::Utc::now().to_rfc3339();
        let annotation = Annotation {
            id: params.mark_id.clone(),
            target_type: "note".into(),
            target_id: params.note_id.clone(),
            content: params.content,
            tags: params.tags.unwrap_or_default(),
            author: "user".into(),
            priority: 0,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
            access_count: 0,
            mark_id: Some(params.mark_id),
            quoted_text: params.quoted_text,
            range_start: params.range_start,
            range_end: params.range_end,
            ai_suggestion: params.ai_suggestion,
        };
        repo.upsert(&annotation).await?;
        Ok(annotation_to_response(&annotation, &params.note_id))
    }
    // ... annotation_update, annotation_delete, annotation_list_for_note
}
```

Methods:
- `annotation_create(params: AnnotationCreateParams) -> Result<AnnotationResponse>`
- `annotation_update(params: AnnotationUpdateParams) -> Result<AnnotationResponse>`
- `annotation_delete(id: String) -> Result<()>`
- `annotation_list_for_note(note_id: String, limit: Option<i64>) -> Result<Vec<AnnotationResponse>>`
- `annotation_get_ai_suggestion(note_id: String, selected_text: String) -> Result<AiSuggestionResponse>` — queries `SemanticFactRepo::search_fts` for related facts
- `note_get_linked_context(params: LinkedContextParams) -> Result<LinkedContextResponse>`

For `note_get_linked_context`, construct each cognitive repo and query in parallel. **Important: all `search_fts` methods take 3 arguments: (query, domain: Option<&str>, limit):**

```rust
let pool = self.storage_pool.inner().clone();
let sf_repo = SemanticFactRepo::new(pool.clone());
let em_repo = EpisodicMemoryRepo::new(pool.clone());
let pr_repo = ProceduralRuleRepo::new(pool.clone());
let ann_repo = AnnotationRepo::new(pool);

let (facts, memories, rules, annotations) = tokio::join!(
    sf_repo.search_fts(&params.section_text, None, 10),
    em_repo.search_fts(&params.section_text, None, 10),
    pr_repo.search_fts(&params.section_text, None, 10),
    ann_repo.search(&params.section_text, 10),
);
```

- [ ] **Step 3: Register module**

In `crates/app-core/src/handlers/mod.rs`, add: `pub mod annotations;`

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p app-core -E 'test(annotation)'`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/annotations.rs crates/app-core/src/handlers/mod.rs
git commit -m "feat(app-core): add annotation CRUD and linked context handlers"
```

---

## Task 6: Backend — Tauri IPC Commands

**Files:**
- Create: `crates/desktop/src/commands/annotations.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create annotation Tauri commands**

Create `crates/desktop/src/commands/annotations.rs` following the exact pattern in `entity_links.rs`. **Critical:** Use `State<'_, Arc<AppCore>>` (not `AppState`) and `ApiError` (not `String`):

```rust
use desktop_shared::commands::annotations::*;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn annotation_create(
    state: State<'_, Arc<AppCore>>,
    params: AnnotationCreateParams,
) -> Result<AnnotationResponse, ApiError> {
    state.annotation_create(params).await
}

#[tauri::command]
pub async fn annotation_update(
    state: State<'_, Arc<AppCore>>,
    params: AnnotationUpdateParams,
) -> Result<AnnotationResponse, ApiError> {
    state.annotation_update(params).await
}

#[tauri::command]
pub async fn annotation_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.annotation_delete(id).await
}

#[tauri::command]
pub async fn annotation_list_for_note(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    limit: Option<i64>,
) -> Result<Vec<AnnotationResponse>, ApiError> {
    state.annotation_list_for_note(note_id, limit).await
}

#[tauri::command]
pub async fn annotation_get_ai_suggestion(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    selected_text: String,
) -> Result<AiSuggestionResponse, ApiError> {
    state.annotation_get_ai_suggestion(note_id, selected_text).await
}

#[tauri::command]
pub async fn note_get_linked_context(
    state: State<'_, Arc<AppCore>>,
    params: LinkedContextParams,
) -> Result<LinkedContextResponse, ApiError> {
    state.note_get_linked_context(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "annotation_create",
    "annotation_update",
    "annotation_delete",
    "annotation_list_for_note",
    "annotation_get_ai_suggestion",
    "note_get_linked_context",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "annotation_create" => dev::val(core.annotation_create(try_field!(dev::parse_params(body))).await),
        "annotation_update" => dev::val(core.annotation_update(try_field!(dev::parse_params(body))).await),
        "annotation_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.annotation_delete(id).await)
        }
        "annotation_list_for_note" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            let limit = body.get("limit").and_then(|v| v.as_i64());
            dev::val(core.annotation_list_for_note(note_id, limit).await)
        }
        "annotation_get_ai_suggestion" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            let selected_text = try_field!(dev::get_str(body, "selectedText"));
            dev::val(core.annotation_get_ai_suggestion(note_id, selected_text).await)
        }
        "note_get_linked_context" => dev::val(core.note_get_linked_context(try_field!(dev::parse_params(body))).await),
        _ => return None,
    })
}
```

- [ ] **Step 2: Register in commands/mod.rs**

Add `pub mod annotations;` and register the command functions in the Tauri builder's `invoke_handler`.

- [ ] **Step 3: Add to dev_server/mod.rs**

Add `annotations::DEV_COMMANDS` to the dev server coverage list. Follow the existing pattern.

- [ ] **Step 4: Build and test**

Run: `cargo build -p desktop`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: All pass — the DEV_COMMANDS test confirms coverage.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/annotations.rs crates/desktop/src/commands/mod.rs crates/desktop/src/dev_server/mod.rs
git commit -m "feat(desktop): wire annotation and linked context IPC commands"
```

---

## Task 7: Frontend — Install Dependencies + TipTap Extensions

**Files:**
- Modify: `desktop-ui/package.json`
- Create: `desktop-ui/src/features/notes/components/editor/AnnotationMark.ts`
- Modify: `desktop-ui/src/features/notes/components/editor/EditorCore.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/editor.css`

- [ ] **Step 1: Install npm dependencies**

Run: `cd desktop-ui && bun add ulid`

Note: `@tiptap/extension-unique-id` is a TipTap Pro (paid) package. We implement UniqueID inline instead — it's a simple global attribute that generates a ULID per heading node on creation. See Step 3b below.

- [ ] **Step 2: Create AnnotationMark extension**

Create `desktop-ui/src/features/notes/components/editor/AnnotationMark.ts`:

```typescript
import { Mark, mergeAttributes } from "@tiptap/core";

export interface AnnotationMarkOptions {
  HTMLAttributes: Record<string, unknown>;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    annotationMark: {
      setAnnotation: (annotationId: string) => ReturnType;
      unsetAnnotation: (annotationId: string) => ReturnType;
    };
  }
}

export const AnnotationMark = Mark.create<AnnotationMarkOptions>({
  name: "annotation",

  addOptions() {
    return { HTMLAttributes: {} };
  },

  addAttributes() {
    return {
      annotationId: {
        default: null,
        parseHTML: (el) => el.getAttribute("data-annotation-id"),
        renderHTML: (attrs) => ({ "data-annotation-id": attrs.annotationId }),
      },
      pending: {
        default: false,
        renderHTML: (attrs) => (attrs.pending ? { "data-pending": "true" } : {}),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-annotation-id]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, { class: "annotation-highlight" }), 0];
  },

  addCommands() {
    return {
      setAnnotation:
        (annotationId: string) =>
        ({ commands }) =>
          commands.setMark(this.name, { annotationId, pending: true }),
      unsetAnnotation:
        (annotationId: string) =>
        ({ tr, state }) => {
          // Remove only marks matching this annotationId
          state.doc.descendants((node, pos) => {
            node.marks.forEach((mark) => {
              if (mark.type.name === this.name && mark.attrs.annotationId === annotationId) {
                tr.removeMark(pos, pos + node.nodeSize, mark);
              }
            });
          });
          return true;
        },
    };
  },

  addKeyboardShortcuts() {
    return {
      "Alt-a": () => {
        // Handled by useEditorActions — dispatch custom event
        window.dispatchEvent(new CustomEvent("editor-action", { detail: { action: "annotate" } }));
        return true;
      },
      "Alt-f": () => {
        window.dispatchEvent(new CustomEvent("editor-action", { detail: { action: "flashcard" } }));
        return true;
      },
      "Alt-l": () => {
        window.dispatchEvent(new CustomEvent("editor-action", { detail: { action: "linked-view" } }));
        return true;
      },
    };
  },
});
```

- [ ] **Step 3a: Create inline UniqueID extension**

Create `desktop-ui/src/features/notes/components/editor/UniqueID.ts`:

```typescript
import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { ulid } from "ulid";

export const UniqueID = Extension.create({
  name: "uniqueID",

  addGlobalAttributes() {
    return [
      {
        types: ["heading"],
        attributes: {
          id: {
            default: null,
            parseHTML: (element) => element.getAttribute("data-id"),
            renderHTML: (attributes) => {
              if (!attributes.id) return {};
              return { "data-id": attributes.id };
            },
          },
        },
      },
    ];
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("uniqueID"),
        appendTransaction: (_, __, newState) => {
          const { tr } = newState;
          let modified = false;
          newState.doc.descendants((node, pos) => {
            if (node.type.name === "heading" && !node.attrs.id) {
              tr.setNodeMarkup(pos, undefined, { ...node.attrs, id: ulid() });
              modified = true;
            }
          });
          return modified ? tr : null;
        },
      }),
    ];
  },
});
```

- [ ] **Step 3b: Register extensions in EditorCore.tsx**

In `desktop-ui/src/features/notes/components/editor/EditorCore.tsx`, import and add to the extensions array:

```typescript
import { AnnotationMark } from "./AnnotationMark";
import { UniqueID } from "./UniqueID";

// In the extensions array:
AnnotationMark,
UniqueID,
```

- [ ] **Step 4: Add annotation CSS**

In `desktop-ui/src/features/notes/components/editor/editor.css`, add:

```css
/* Annotation highlights */
.annotation-highlight {
  background: rgba(255, 140, 50, 0.15);
  border-bottom: 2px solid rgba(255, 140, 50, 0.5);
  cursor: pointer;
  transition: background 0.15s ease;
}

.annotation-highlight:hover {
  background: rgba(255, 140, 50, 0.25);
}

.annotation-highlight[data-pending="true"] {
  background: rgba(255, 140, 50, 0.08);
  border-bottom: 2px dashed rgba(255, 140, 50, 0.3);
  cursor: default;
}

/* Perspective section badge */
.perspective-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  cursor: pointer;
  transition: opacity 0.15s ease;
}

.perspective-badge:hover {
  opacity: 0.8;
}
```

- [ ] **Step 5: Build and verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lockb desktop-ui/src/features/notes/components/editor/AnnotationMark.ts desktop-ui/src/features/notes/components/editor/UniqueID.ts desktop-ui/src/features/notes/components/editor/EditorCore.tsx desktop-ui/src/features/notes/components/editor/editor.css
git commit -m "feat(ui): add AnnotationMark + UniqueID TipTap extensions"
```

---

## Task 8: Frontend — useAnnotations Hook + useEditorActions

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useAnnotations.ts`
- Create: `desktop-ui/src/features/notes/hooks/useEditorActions.ts`

- [ ] **Step 1: Create useAnnotations hook**

Create `desktop-ui/src/features/notes/hooks/useAnnotations.ts`:

This hook manages:
- `useQuery` for fetching annotations via `annotation_list_for_note` IPC
- `useMutation` for create/update/delete with optimistic mark sync
- Mark confirmation (pending → confirmed) on successful IPC
- Mark rollback on failed IPC

Key function signatures:
```typescript
export function useAnnotations(noteId: string | null, editor: Editor | null) {
  // Query: fetch all annotations for this note
  const annotationsQuery = useQuery({
    queryKey: ["annotations", noteId],
    queryFn: () => invoke<AnnotationResponse[]>("annotation_list_for_note", { noteId, limit: 200 }),
    enabled: !!noteId,
  });

  // Mutation: create annotation (optimistic mark + IPC)
  const createAnnotation = useMutation({
    mutationFn: (params: AnnotationCreateParams) => invoke<AnnotationResponse>("annotation_create", { params }),
    onSuccess: (response) => {
      // Confirm mark: find the specific mark by annotationId and remove pending attribute
      if (editor) {
        const { tr, doc } = editor.state;
        doc.descendants((node, pos) => {
          node.marks.forEach((mark) => {
            if (mark.type.name === "annotation" && mark.attrs.annotationId === response.markId && mark.attrs.pending) {
              tr.removeMark(pos, pos + node.nodeSize, mark);
              tr.addMark(pos, pos + node.nodeSize, mark.type.create({ ...mark.attrs, pending: false }));
            }
          });
        });
        editor.view.dispatch(tr);
      }
      queryClient.invalidateQueries({ queryKey: ["annotations", noteId] });
    },
    onError: (_, variables) => {
      // Rollback: remove the optimistic mark
      editor?.commands.unsetAnnotation(variables.markId);
      toast.error("Failed to create annotation");
    },
  });

  // ... update, delete mutations following same pattern

  return { annotations: annotationsQuery.data ?? [], createAnnotation, updateAnnotation, deleteAnnotation };
}
```

- [ ] **Step 2: Create useEditorActions hook**

Create `desktop-ui/src/features/notes/hooks/useEditorActions.ts`:

This hook provides the shared action handlers used by both the BubbleToolbar and EditorContextMenu:

```typescript
export function useEditorActions(editor: Editor | null, noteId: string | null) {
  const { createAnnotation } = useAnnotations(noteId, editor);
  const { generateFromText } = useCardGeneration();

  const handleAnnotate = useCallback(() => {
    if (!editor || editor.state.selection.empty) return;
    const { from, to } = editor.state.selection;
    const selectedText = editor.state.doc.textBetween(from, to);
    const markId = ulid();

    // Apply optimistic mark
    editor.commands.setAnnotation(markId);

    // Create annotation via IPC
    createAnnotation.mutate({
      noteId: noteId!,
      markId,
      content: "",
      quotedText: selectedText,
      rangeStart: from,
      rangeEnd: to,
    });
  }, [editor, noteId, createAnnotation]);

  const handleFlashcard = useCallback(() => {
    if (!editor || editor.state.selection.empty) return;
    const { from, to } = editor.state.selection;
    const selectedText = editor.state.doc.textBetween(from, to);
    generateFromText(selectedText);
  }, [editor, generateFromText]);

  const handleTranslate = useCallback(() => {
    // Activate translation split-pane with selection
    // Dispatch event to NoteEditor to switch split mode
  }, [editor]);

  const handleAskAI = useCallback(() => {
    // Open inline prompt input
  }, [editor]);

  return { handleAnnotate, handleFlashcard, handleTranslate, handleAskAI };
}
```

- [ ] **Step 3: Verify types compile**

Run: `cd desktop-ui && bun run build`
Expected: Compiles (IPC calls may not exist yet in dev — that's OK, they'll work when the Tauri backend is running).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useAnnotations.ts desktop-ui/src/features/notes/hooks/useEditorActions.ts
git commit -m "feat(ui): add useAnnotations and useEditorActions hooks"
```

---

## Task 9: Frontend — BubbleToolbar Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/BubbleToolbar.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Create BubbleToolbar component**

Create `desktop-ui/src/features/notes/components/editor/BubbleToolbar.tsx`:

Uses TipTap's `BubbleMenu` with 4 AI action buttons. `shouldShow` returns true only when selection is non-empty and not inside a code block. 200ms `tippyOptions.delay` for debounce.

```tsx
import { BubbleMenu, type Editor } from "@tiptap/react";

interface BubbleToolbarProps {
  editor: Editor;
  onAnnotate: () => void;
  onFlashcard: () => void;
  onTranslate: () => void;
  onAskAI: () => void;
}

export function BubbleToolbar({ editor, onAnnotate, onFlashcard, onTranslate, onAskAI }: BubbleToolbarProps) {
  return (
    <BubbleMenu
      editor={editor}
      tippyOptions={{ duration: 150, delay: [200, 0] }}
      shouldShow={({ editor, state }) => {
        if (state.selection.empty) return false;
        if (editor.isActive("codeBlock")) return false;
        return true;
      }}
    >
      <div className="glass-panel flex items-center gap-0.5 rounded-[10px] p-1.5 shadow-lg">
        <button onClick={onAnnotate} className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs text-brand hover:bg-surface-hover" title="Add Annotation (⌥A)">
          📝 Annotate
        </button>
        <button onClick={onFlashcard} className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs text-purple-400 hover:bg-surface-hover" title="Create Flashcard (⌥F)">
          ⚡ Flashcard
        </button>
        <button onClick={onTranslate} className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs text-green-400 hover:bg-surface-hover" title="Translate Selection">
          🌐 Translate
        </button>
        <button onClick={onAskAI} className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs text-blue-400 hover:bg-surface-hover" title="Ask AI">
          ✦ Ask AI
        </button>
      </div>
    </BubbleMenu>
  );
}
```

- [ ] **Step 2: Integrate in NoteEditor.tsx**

In `NoteEditor.tsx`, import and render `BubbleToolbar` when the editor is available. Pass the shared action handlers from `useEditorActions`.

- [ ] **Step 3: Build and visually verify**

Run: `cd desktop-ui && bun run dev`
Open a note, select text → floating toolbar should appear with 4 AI action buttons.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/BubbleToolbar.tsx desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat(ui): add floating toolbar (BubbleMenu) with AI actions"
```

---

## Task 10: Frontend — EditorContextMenu Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/EditorContextMenu.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Create EditorContextMenu**

Create `desktop-ui/src/features/notes/components/editor/EditorContextMenu.tsx`:

Uses Radix `ContextMenu` with `glass-panel` styling. 4 groups: Selection (conditional), AI Actions, Perspectives, Utility. Shows keyboard shortcuts.

```tsx
import * as ContextMenu from "@radix-ui/react-context-menu";

interface EditorContextMenuProps {
  children: React.ReactNode;
  hasSelection: boolean;
  onAnnotate: () => void;
  onFlashcard: () => void;
  onTranslate: () => void;
  onExplainDefine: () => void;
  onAskAI: () => void;
  onGenerateStudyPack: () => void;
  onShowLinkedMemory: () => void;
  onLinkedView: () => void;
  onApplyPerspective: (type: string) => void;
}
```

Implement with Radix ContextMenu primitives (`Root`, `Trigger`, `Portal`, `Content`, `Item`, `Sub`, `SubTrigger`, `SubContent`, `Separator`, `Label`). Apply `glass-panel` class to Content. Show Selection group only when `hasSelection` is true. Add `⌥A`, `⌥F`, `⌥L` shortcut labels via Radix's `ContextMenu.Item` shortcut prop or a right-aligned span.

- [ ] **Step 2: Wrap NoteEditor content with ContextMenu**

In `NoteEditor.tsx`, wrap the editor area with `EditorContextMenu` as the trigger. Pass `hasSelection` from the editor state and all action handlers.

- [ ] **Step 3: Build and visually verify**

Run: `cd desktop-ui && bun run dev`
Right-click in editor → context menu appears with grouped actions.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/EditorContextMenu.tsx desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat(ui): add right-click context menu with AI actions and perspectives"
```

---

## Task 11: Frontend — AnnotationPopover Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/AnnotationPopover.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Create AnnotationPopover**

Create `desktop-ui/src/features/notes/components/AnnotationPopover.tsx`:

Glassmorphic popover showing:
- Header with timestamp + tag badges
- Quoted text with orange left border
- User annotation content (editable)
- Cognitive Memory block with confidence bar
- Action buttons row (Create Flashcard, Link to Fact, Open in Insights, Edit, Delete)

Position via `editor.view.coordsAtPos()`. Render as a React Portal to avoid container clipping. Only one popover open at a time (managed by state in NoteEditor).

Props:
```typescript
interface AnnotationPopoverProps {
  annotation: AnnotationResponse;
  position: { top: number; left: number };
  onClose: () => void;
  onEdit: (id: string, content: string) => void;
  onDelete: (id: string) => void;
  onCreateFlashcard: (quotedText: string, content: string) => void;
  onLinkToFact: (annotationId: string) => void;
  onOpenInInsights: (annotation: AnnotationResponse) => void;
}
```

- [ ] **Step 2: Wire click handler for annotation marks**

In `NoteEditor.tsx`, add a click handler that detects clicks on `.annotation-highlight` elements. Extract the `annotationId` from `data-annotation-id`, look up the annotation data from `useAnnotations`, compute position via `coordsAtPos`, and show the `AnnotationPopover`.

- [ ] **Step 3: Build and visually verify**

Run: `cd desktop-ui && bun run dev`
Create an annotation → click on the orange highlight → popover appears with annotation details.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/AnnotationPopover.tsx desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat(ui): add glassmorphic annotation popover with AI insight and actions"
```

---

## Task 12: Frontend — Perspective State + useLinkedContext Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/usePerspective.ts`
- Create: `desktop-ui/src/features/notes/hooks/useLinkedContext.ts`

- [ ] **Step 1: Create usePerspective hook**

Manages per-section perspective configuration. Reads from `note.perspectiveConfig` JSON. Provides methods to set/clear perspectives. Handles the cleanup of orphaned entries (IDs not matching current headings).

```typescript
export type PerspectiveType = "linked-view" | "annotated" | "study-mode";

interface PerspectiveConfig {
  sections: Record<string, { active: PerspectiveType; params?: Record<string, unknown> } | null>;
}

export function usePerspective(noteId: string | null, editor: Editor | null) {
  // Parse perspectiveConfig from note data
  // Track focused section (cursor position → heading ID)
  // Provide setPerspective(sectionId, type) and clearPerspective(sectionId)
  // Save config changes via note_update IPC (debounced)
  // Return: activePerspective (for current section), allPerspectives, setPerspective, clearPerspective
}
```

- [ ] **Step 2: Create useLinkedContext hook**

Fetches cognitive memory results for a given section:

```typescript
export function useLinkedContext(noteId: string | null, sectionText: string | null) {
  return useQuery({
    queryKey: ["linked-context", noteId, sectionText?.slice(0, 100)],
    queryFn: () => invoke<LinkedContextResponse>("note_get_linked_context", { params: { noteId, sectionText } }),
    enabled: !!noteId && !!sectionText && sectionText.length > 10,
    staleTime: 10 * 60 * 1000, // 10 minutes
  });
}
```

- [ ] **Step 3: Build and verify types**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/usePerspective.ts desktop-ui/src/features/notes/hooks/useLinkedContext.ts
git commit -m "feat(ui): add usePerspective and useLinkedContext hooks"
```

---

## Task 13: Frontend — LinkedViewPanel + PerspectiveOverlay

**Files:**
- Create: `desktop-ui/src/features/notes/components/LinkedViewPanel.tsx`
- Create: `desktop-ui/src/features/notes/components/PerspectiveOverlay.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx`

- [ ] **Step 1: Create LinkedViewPanel**

Renders categorized cognitive memory results in the split-pane right side:

- Semantic Facts section (purple, with confidence scores and "Link ↗" buttons)
- Episodic Memories section (orange, with timestamps)
- Related Annotations section (green, with source note references)
- Connection count header ("7 cognitive links")

Uses the `useLinkedContext` hook data.

- [ ] **Step 2: Create PerspectiveOverlay**

Routes to the correct perspective view based on `activePerspective`:

```typescript
interface PerspectiveOverlayProps {
  perspective: PerspectiveType | null;
  noteId: string;
  sectionText: string;
  sectionId: string;
}

export function PerspectiveOverlay({ perspective, noteId, sectionText, sectionId }: PerspectiveOverlayProps) {
  switch (perspective) {
    case "linked-view":
      return <LinkedViewPanel noteId={noteId} sectionText={sectionText} />;
    case "annotated":
      return <AnnotatedView noteId={noteId} sectionId={sectionId} />;
    case "study-mode":
      return <StudyModeView noteId={noteId} sectionId={sectionId} />;
    default:
      return null;
  }
}
```

- [ ] **Step 3: Integrate into SplitEditor**

Modify `SplitEditor.tsx` to render the `PerspectiveOverlay` in the right pane when a perspective is active on the focused section. Add a 200ms crossfade transition (`transition-opacity duration-200`). When no perspective is active, fall back to the existing split-pane mode content.

- [ ] **Step 4: Build and visually verify**

Run: `cd desktop-ui && bun run dev`
Apply Linked View via context menu → right pane shows cognitive memory results.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/LinkedViewPanel.tsx desktop-ui/src/features/notes/components/PerspectiveOverlay.tsx desktop-ui/src/features/notes/components/editor/SplitEditor.tsx
git commit -m "feat(ui): add LinkedViewPanel, PerspectiveOverlay, and contextual split-pane"
```

---

## Task 14: Frontend — Annotated + Study Mode Perspectives

**Files:**
- Create: `desktop-ui/src/features/notes/components/perspectives/AnnotatedView.tsx`
- Create: `desktop-ui/src/features/notes/components/perspectives/StudyModeView.tsx`

- [ ] **Step 1: Create AnnotatedView**

Shows all annotations for a given section:
- List of annotations with quoted text, content, tags, timestamps
- Annotation cluster density visualization
- Click to scroll to annotation in editor
- AI-suggested follow-up actions (optional, via LLM call)

- [ ] **Step 2: Create StudyModeView**

Shows flashcards related to a section:
- Query flashcards where `source_note_id` matches and content overlaps with section text
- Show front/back with click-to-reveal
- Due cards highlighted (amber glow)
- "Quick Review" button to start a mini FSRS-5 session

- [ ] **Step 3: Build and verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/perspectives/
git commit -m "feat(ui): add Annotated and Study Mode perspective views"
```

---

## Task 15: Integration — End-to-End Wiring

**Files:**
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx`

- [ ] **Step 1: Wire all hooks into NoteEditor**

Connect `useAnnotations`, `useEditorActions`, `usePerspective` in `NoteEditor.tsx`. Pass action handlers down to `BubbleToolbar`, `EditorContextMenu`, and `AnnotationPopover`. Track focused section ID from cursor position.

- [ ] **Step 2: Wire perspective state into SplitEditor**

Pass `activePerspective` and `sectionText` from NoteEditor into SplitEditor. SplitEditor renders `PerspectiveOverlay` when a perspective is active, otherwise falls back to the existing mode-based content.

- [ ] **Step 3: Add perspective badge rendering**

In EditorCore or NoteEditor, render perspective badges below headings that have active perspectives. Use TipTap's `editor.view.coordsAtPos()` to position them.

- [ ] **Step 4: End-to-end manual test**

Run: `cd desktop-ui && bun run dev` (and `cargo tauri dev` for the backend)

Test the full hero workflow:
1. Open a note with content
2. Select text → floating toolbar appears → click "Annotate" → popover appears → add comment
3. Right-click → "Linked View" → right pane shows cognitive memory
4. Click annotation highlight → popover opens → click "Create Flashcard"
5. Right-click → "Apply Perspective" → "Study Mode" → right pane shows flashcards
6. Keyboard shortcuts: ⌥A (annotate), ⌥F (flashcard), ⌥L (linked view)

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(ui): wire end-to-end annotation + perspective + menu integration"
```

---

## Task 16 (Stretch): Knowledge Heat Map

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useKnowledgeHeat.ts`
- Modify: `desktop-ui/src/features/notes/components/editor/editor.css`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Create useKnowledgeHeat hook**

Queries annotation count + flashcard count per section heading ID. Returns a map of `sectionId → { annotationCount, flashcardCount, heatLevel: "hot" | "warm" | "cool" }`.

- [ ] **Step 2: Render heat indicators**

In NoteEditor, render left-margin gradient indicators per section. Use CSS left border with dynamic color: `border-left: 4px solid` with orange (hot) / yellow (warm) / blue (cool).

- [ ] **Step 3: Add CSS**

```css
.knowledge-heat-hot { border-left: 4px solid rgba(255, 140, 50, 0.8); }
.knowledge-heat-warm { border-left: 4px solid rgba(251, 191, 36, 0.6); }
.knowledge-heat-cool { border-left: 4px solid rgba(59, 130, 246, 0.4); }
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useKnowledgeHeat.ts desktop-ui/src/features/notes/components/editor/editor.css desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat(ui): add knowledge heat map engagement visualization"
```

---

## Task 17 (Stretch): Active Recall Gate

**Files:**
- Create: `desktop-ui/src/features/notes/components/ActiveRecallGate.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Backend — update last_visited_at on note open**

In `crates/app-core/src/handlers/notes/crud.rs`, in the `note_get` handler (or wherever a single note is fetched), add a query to update `last_visited_at` to the current UTC timestamp.

- [ ] **Step 2: Create ActiveRecallGate component**

Modal that appears before note content loads if `last_visited_at` is older than 3 days. Shows 2-3 due flashcards from the note's deck. On completion, opens the note with results feedback (green flash for all correct, amber highlights for missed sections).

- [ ] **Step 3: Wire in NoteEditor**

Before rendering editor content, check if recall gate should show. Render `ActiveRecallGate` modal first, then transition to editor on completion.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/crud.rs desktop-ui/src/features/notes/components/ActiveRecallGate.tsx desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat: add active recall gate for note reopen micro-quiz"
```

---

## Task 18: Final — Lint, Format, Clippy

- [ ] **Step 1: Rust checks**

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

Fix any warnings or formatting issues.

- [ ] **Step 2: Frontend checks**

```bash
cd desktop-ui && bun run lint:fix
```

- [ ] **Step 3: Full test suite**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run test
```

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: fix lint and formatting for active learning surface"
```
