# Notes Import/Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Markdown file import/export to the notes feature — import `.md` files/folders into notebooks, export notes/notebooks as `.md` files with YAML front matter.

**Architecture:** Backend-heavy (Rust handles file I/O, parsing, bulk insert, export writing), frontend handles UX (drag-and-drop, file picker dialogs, context menus). New `front_matter.rs` module in `feature-notes` for shared YAML parsing/serialization. Two new Tauri commands (`note_import_files`, `note_export`) with corresponding AppCore handlers.

**Tech Stack:** Rust (`serde_yml`, `tokio::fs`, `sqlx` transactions), Tauri 2 (`tauri-plugin-dialog`), TypeScript/React (TipTap editor, existing `useMutation`/`useQuery` hooks).

**Spec:** `docs/superpowers/specs/2026-03-26-notes-import-export-design.md`

---

## File Map

### New files
| File | Responsibility |
|------|---------------|
| `crates/feature-notes/src/front_matter.rs` | `NoteFrontMatter` struct, `parse()`, `serialize()` — shared by import and export |
| `crates/app-core/src/handlers/notes/import_export.rs` | `AppCore::note_import_files()` and `AppCore::note_export()` handlers |

### Modified files
| File | Changes |
|------|---------|
| `crates/feature-notes/Cargo.toml` | Add `serde_yml` dependency |
| `crates/feature-notes/src/lib.rs` | Add `pub mod front_matter` |
| `crates/feature-notes/src/repo/mod.rs` | Add `pub fn pool()` accessor on `NoteRepo` |
| `crates/feature-notes/src/repo/notebooks.rs` | Add `find_by_parent_and_title()` method |
| `crates/desktop-shared/src/commands/notes.rs` | Add import/export IPC types; extend `NoteCreateParams` |
| `crates/app-core/src/handlers/notes/mod.rs` | Add `mod import_export` |
| `crates/app-core/src/handlers/notes/crud.rs` | Update `note_create` to accept new fields (`created_at`, `icon`, `color`) |
| `crates/desktop/Cargo.toml` | Add `tauri-plugin-dialog` |
| `crates/desktop/src/lib.rs` | Register dialog plugin |
| `crates/desktop/capabilities/default.json` | Add dialog permissions |
| `crates/desktop/src/commands/notes.rs` | Add Tauri commands + `DEV_COMMANDS` + `dispatch_dev` entries |
| `desktop-ui/package.json` | Add `@tauri-apps/plugin-dialog` |
| `desktop-ui/src/shared/types/notes.ts` | Add import/export TS types |
| `desktop-ui/src/features/notes/components/NotebookTree.tsx` | External file drop detection, context menu items, export menu items |
| `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Wire import/export handlers and mutations |

---

## Task 1: Front Matter Module (feature-notes)

**Files:**
- Modify: `crates/feature-notes/Cargo.toml`
- Create: `crates/feature-notes/src/front_matter.rs`
- Modify: `crates/feature-notes/src/lib.rs`

- [ ] **Step 1: Add serde_yml dependency**

In `crates/feature-notes/Cargo.toml`, add under `[dependencies]`:

```toml
serde_yml = "0.0.12"
```

- [ ] **Step 2: Write failing tests for front_matter::parse**

Create `crates/feature-notes/src/front_matter.rs` with the struct and test module:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NoteFrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Result of parsing a Markdown file that may contain YAML front matter.
#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub front_matter: Option<NoteFrontMatter>,
    pub body: String,
    /// If front matter was present but malformed YAML, this contains the error message.
    pub warning: Option<String>,
}

/// Parse a Markdown file's content, extracting YAML front matter if present.
///
/// Front matter is only recognized when the file starts with `---\n` at byte 0.
/// The closing delimiter is `\n---\n` or `\n---` at EOF.
pub fn parse(content: &str) -> ParsedMarkdown {
    todo!()
}

/// Serialize a `NoteFrontMatter` and body into a complete Markdown file with YAML front matter.
pub fn serialize(front_matter: &NoteFrontMatter, body: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_valid_front_matter() {
        let content = "---\ntitle: My Note\ntags:\n  - rust\n  - architecture\npinned: true\n---\n\nHello world";
        let result = parse(content);
        let fm = result.front_matter.unwrap();
        assert_eq!(fm.title.as_deref(), Some("My Note"));
        assert_eq!(fm.tags, Some(vec!["rust".into(), "architecture".into()]));
        assert_eq!(fm.pinned, Some(true));
        assert_eq!(result.body, "Hello world");
        assert!(result.warning.is_none());
    }

    #[test]
    fn parse_no_front_matter() {
        let content = "Just plain markdown\n\nWith paragraphs";
        let result = parse(content);
        assert!(result.front_matter.is_none());
        assert_eq!(result.body, content);
        assert!(result.warning.is_none());
    }

    #[test]
    fn parse_horizontal_rule_not_treated_as_front_matter() {
        let content = "Some text\n\n---\n\nMore text";
        let result = parse(content);
        assert!(result.front_matter.is_none());
        assert_eq!(result.body, content);
    }

    #[test]
    fn parse_malformed_yaml() {
        let content = "---\ntitle: [invalid yaml\n---\n\nBody here";
        let result = parse(content);
        assert!(result.front_matter.is_none());
        assert_eq!(result.body, content);
        assert!(result.warning.is_some());
    }

    #[test]
    fn parse_front_matter_only_no_body() {
        let content = "---\ntitle: Empty Note\n---\n";
        let result = parse(content);
        let fm = result.front_matter.unwrap();
        assert_eq!(fm.title.as_deref(), Some("Empty Note"));
        assert!(result.body.is_empty() || result.body.trim().is_empty());
    }

    #[test]
    fn parse_empty_file() {
        let result = parse("");
        assert!(result.front_matter.is_none());
        assert!(result.body.is_empty());
    }

    #[test]
    fn parse_unknown_fields_ignored() {
        let content = "---\ntitle: Note\naliases: [foo]\ncssclass: wide\n---\n\nBody";
        let result = parse(content);
        let fm = result.front_matter.unwrap();
        assert_eq!(fm.title.as_deref(), Some("Note"));
        assert_eq!(result.body, "Body");
    }

    #[test]
    fn serialize_round_trip() {
        let fm = NoteFrontMatter {
            title: Some("Round Trip".into()),
            tags: Some(vec!["test".into()]),
            created: Some("2026-03-26T10:00:00Z".into()),
            ..Default::default()
        };
        let body = "Hello world\n\nParagraph two";
        let serialized = serialize(&fm, body);
        let parsed = parse(&serialized);
        assert_eq!(parsed.front_matter.unwrap(), fm);
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn serialize_omits_none_fields() {
        let fm = NoteFrontMatter {
            title: Some("Minimal".into()),
            ..Default::default()
        };
        let serialized = serialize(&fm, "body");
        assert!(!serialized.contains("tags:"));
        assert!(!serialized.contains("pinned:"));
        assert!(!serialized.contains("icon:"));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p feature-notes -E 'test(front_matter)'`
Expected: FAIL — `todo!()` panics

- [ ] **Step 4: Implement parse() and serialize()**

Replace the `todo!()` bodies:

```rust
pub fn parse(content: &str) -> ParsedMarkdown {
    // Front matter only recognized at byte 0
    if !content.starts_with("---\n") {
        return ParsedMarkdown {
            front_matter: None,
            body: content.to_string(),
            warning: None,
        };
    }

    // Find closing delimiter: \n---\n or \n--- at EOF
    // Search starts at byte 4 (after opening "---\n")
    let after_opening = &content[4..];
    let close_pos = if let Some(pos) = after_opening.find("\n---\n") {
        pos
    } else if after_opening.ends_with("\n---") {
        after_opening.len() - 3
    } else {
        // No closing delimiter — treat entire content as body
        return ParsedMarkdown {
            front_matter: None,
            body: content.to_string(),
            warning: None,
        };
    };

    let yaml_str = &after_opening[..close_pos];
    // close_pos points to "\n---\n" (5 bytes) or "\n---" at EOF (4 bytes)
    let has_trailing_newline = after_opening[close_pos..].starts_with("\n---\n");
    let delimiter_len = if has_trailing_newline { 5 } else { 4 };
    let body_start = 4 + close_pos + delimiter_len; // opening "---\n" + yaml + delimiter
    let body = if body_start < content.len() {
        // Trim exactly one leading newline (not all — preserve intentional blank lines)
        let raw = &content[body_start..];
        if let Some(stripped) = raw.strip_prefix('\n') {
            stripped.to_string()
        } else {
            raw.to_string()
        }
    } else {
        String::new()
    };

    match serde_yml::from_str::<NoteFrontMatter>(yaml_str) {
        Ok(fm) => ParsedMarkdown {
            front_matter: Some(fm),
            body,
            warning: None,
        },
        Err(e) => ParsedMarkdown {
            front_matter: None,
            body: content.to_string(),
            warning: Some(format!("Malformed front matter: {e}")),
        },
    }
}

pub fn serialize(front_matter: &NoteFrontMatter, body: &str) -> String {
    let yaml = serde_yml::to_string(front_matter).unwrap_or_default();
    format!("---\n{}---\n\n{}", yaml, body)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p feature-notes -E 'test(front_matter)'`
Expected: All 8 tests PASS

- [ ] **Step 6: Register the module**

In `crates/feature-notes/src/lib.rs`, add after `pub mod tool;`:

```rust
pub mod front_matter;
```

- [ ] **Step 7: Verify full crate compiles**

Run: `cargo build -p feature-notes`
Expected: Compiles with 0 errors

- [ ] **Step 8: Commit**

```bash
git add crates/feature-notes/
git commit -m "feat(notes): add front_matter module for YAML parsing and serialization"
```

---

## Task 2: Notebook Deduplication Query

**Files:**
- Modify: `crates/feature-notes/src/repo/notebooks.rs`

- [ ] **Step 0: Add pool() accessor to NoteRepo**

In `crates/feature-notes/src/repo/mod.rs`, add a public accessor inside the `impl NoteRepo` block (after `pub fn new`):

```rust
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
```

This is needed by the import handler in `app-core` to open a raw SQLite transaction for bulk insert.

- [ ] **Step 1: Write failing test**

Add to the existing `#[cfg(test)] mod tests` in `crates/feature-notes/src/repo/mod.rs` (which already has a `setup()` helper that runs migrations):

```rust
    #[tokio::test]
    async fn test_find_notebook_by_parent_and_title() {
        let repo = setup().await;
        let now = utc_now_str();

        // Create a notebook
        let row = NotebookRow {
            id: "nb1".into(),
            parent_id: None,
            title: "Projects".into(),
            icon: None,
            color: None,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_notebook(&row).await.unwrap();

        // Should find it
        let found = repo.find_notebook_by_parent_and_title(None, "Projects").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "nb1");

        // Should not find non-existent
        let not_found = repo.find_notebook_by_parent_and_title(None, "Other").await.unwrap();
        assert!(not_found.is_none());

        // Should not match when parent differs
        let wrong_parent = repo.find_notebook_by_parent_and_title(Some("nb99"), "Projects").await.unwrap();
        assert!(wrong_parent.is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-notes -E 'test(find_notebook_by_parent_and_title)'`
Expected: FAIL — method doesn't exist

- [ ] **Step 3: Implement find_by_parent_and_title**

Add to the `impl NoteRepo` block in `crates/feature-notes/src/repo/notebooks.rs`, after the `resolve_titles_to_ids` method:

```rust
    /// Find a notebook by parent_id and title. Used for deduplication during import.
    pub async fn find_notebook_by_parent_and_title(
        &self,
        parent_id: Option<&str>,
        title: &str,
    ) -> Result<Option<NotebookRow>, StorageError> {
        let row = sqlx::query_as::<_, NotebookRow>(
            "SELECT * FROM notebooks WHERE title = ?1 AND (parent_id IS ?2) LIMIT 1",
        )
        .bind(title)
        .bind(parent_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p feature-notes -E 'test(find_notebook_by_parent_and_title)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-notes/src/repo/notebooks.rs
git commit -m "feat(notes): add find_notebook_by_parent_and_title for import dedup"
```

---

## Task 3: IPC Types (desktop-shared)

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs`

- [ ] **Step 1: Extend NoteCreateParams**

In `crates/desktop-shared/src/commands/notes.rs`, add three fields to `NoteCreateParams` (after `tags`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}
```

- [ ] **Step 2: Add import/export IPC types**

Add at the end of the file (before the closing, or in a new section):

```rust
// ── Import / Export ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteImportParams {
    pub paths: Vec<String>,
    pub notebook_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteImportResult {
    pub imported: u32,
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteExportParams {
    pub note_ids: Option<Vec<String>>,
    pub notebook_ids: Option<Vec<String>>,
    pub destination: String,
    pub output_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteExportResult {
    pub exported: u32,
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs
git commit -m "feat(notes): add import/export IPC types and extend NoteCreateParams"
```

---

## Task 4: Update note_create Handler (app-core)

**Files:**
- Modify: `crates/app-core/src/handlers/notes/crud.rs:102-126`

- [ ] **Step 1: Update note_create to use new NoteCreateParams fields**

In `crates/app-core/src/handlers/notes/crud.rs`, update the `note_create` method. Change the `NoteRow` construction (around line 109-126) to use the new optional fields:

Replace:
```rust
        let row = NoteRow {
            id: id.clone(),
            notebook_id: params.notebook_id,
            title: params.title,
            body: params.body.unwrap_or_default(),
            body_html: None,
            pinned: 0,
            archived: 0,
            icon: None,
            color: None,
            embedding_updated_at: None,
            split_content: None,
            split_mode: None,
            perspective_config: None,
            last_visited_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
```

With:
```rust
        let created_at = params.created_at.unwrap_or_else(|| now.clone());
        let row = NoteRow {
            id: id.clone(),
            notebook_id: params.notebook_id,
            title: params.title,
            body: params.body.unwrap_or_default(),
            body_html: None,
            pinned: 0,
            archived: 0,
            icon: params.icon,
            color: params.color,
            embedding_updated_at: None,
            split_content: None,
            split_mode: None,
            perspective_config: None,
            last_visited_at: None,
            created_at,
            updated_at: now,
        };
```

- [ ] **Step 2: Verify crate compiles**

Run: `cargo build -p app-core`
Expected: 0 errors

- [ ] **Step 3: Run existing note tests to check no regressions**

Run: `cargo nextest run -p app-core -E 'test(note)'`
Expected: All existing tests PASS (no regressions from adding optional fields)

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/crud.rs
git commit -m "feat(notes): accept created_at, icon, color in note_create"
```

---

## Task 5: Import Handler (app-core)

**Files:**
- Create: `crates/app-core/src/handlers/notes/import_export.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

This is the largest task. The import handler walks directories, parses `.md` files, creates notebooks, and bulk-inserts notes.

- [ ] **Step 1: Add mod declaration**

In `crates/app-core/src/handlers/notes/mod.rs`, add (follows the private-by-default convention of this module):

```rust
mod import_export;
```

- [ ] **Step 2: Create import_export.rs with import handler and tests**

Create `crates/app-core/src/handlers/notes/import_export.rs`. This file contains the `note_import_files` handler on `AppCore` and the `note_export` handler (implemented in Task 6).

Write the import handler with inline tests. The handler should:

1. Validate all paths (absolute, no `..`, canonicalize with broken-symlink skip)
2. Walk directories recursively, collecting `.md` files (case-insensitive), tracking inodes for cycle detection, enforcing 50 MB limit
3. Create notebook structure mirroring directories with dedup via `find_notebook_by_parent_and_title`
4. Parse each file using `feature_notes::front_matter::parse()`
5. Title from front_matter.title or filename (strip `.md` case-insensitive)
6. Bulk insert in a single SQLite transaction
7. Post-commit: publish domain events and queue embeddings

```rust
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use desktop_shared::commands::{NoteImportParams, NoteImportResult, SkippedFile};
use desktop_shared::errors::ApiError;
use feature_notes::front_matter;
use feature_notes::models::{NoteRow, NotebookRow};
use feature_notes::repo::utc_now_str;

use crate::state::AppCore;

const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

/// A collected file ready for import.
struct CollectedFile {
    path: PathBuf,
    content: String,
    notebook_id: Option<String>,
}

impl AppCore {
    pub async fn note_import_files(
        &self,
        params: NoteImportParams,
    ) -> Result<NoteImportResult, ApiError> {
        let mut skipped: Vec<SkippedFile> = Vec::new();
        let mut files: Vec<CollectedFile> = Vec::new();
        let mut visited_inodes: HashSet<u64> = HashSet::new();

        // Phase 1: Validate and collect files
        for path_str in &params.paths {
            let path = PathBuf::from(path_str);
            if !path.is_absolute() {
                return Err(ApiError::new("VALIDATION", format!("Path must be absolute: {path_str}")));
            }
            if path.components().any(|c| c == std::path::Component::ParentDir) {
                return Err(ApiError::new("VALIDATION", format!("Path traversal not allowed: {path_str}")));
            }
            match std::fs::canonicalize(&path) {
                Ok(canonical) => {
                    if canonical.is_dir() {
                        self.collect_dir(
                            &canonical,
                            params.notebook_id.clone(),
                            &mut files,
                            &mut skipped,
                            &mut visited_inodes,
                        ).await?;
                    } else if canonical.is_file() {
                        self.collect_file(&canonical, params.notebook_id.clone(), &mut files, &mut skipped);
                    }
                }
                Err(_) => {
                    skipped.push(SkippedFile {
                        path: path_str.clone(),
                        reason: "Cannot resolve path".into(),
                    });
                }
            }
        }

        if files.is_empty() {
            return Ok(NoteImportResult { imported: 0, skipped });
        }

        // Phase 2: Parse and insert in a single transaction
        let mut imported = 0u32;
        let mut created_ids: Vec<(String, String)> = Vec::new(); // (id, title)
        let mut content_events: Vec<(String, String)> = Vec::new(); // (id, content)

        // Use a raw SQLite transaction for bulk insert
        // NoteRepo.pool() is a pub accessor added in Task 2 Step 0
        let mut tx = self.note_repo.pool().begin().await.map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        for file in &files {
            let parsed = front_matter::parse(&file.content);

            // Handle malformed YAML warning
            if let Some(ref warning) = parsed.warning {
                skipped.push(SkippedFile {
                    path: file.path.display().to_string(),
                    reason: format!("Malformed front matter — imported without metadata: {warning}"),
                });
            }

            let fm = parsed.front_matter.unwrap_or_default();

            // Title: front_matter.title > filename
            let title = fm.title.unwrap_or_else(|| {
                file.path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".into())
            });

            let id = uuid::Uuid::new_v4().to_string();
            let now = utc_now_str();
            let created_at = fm.created.unwrap_or_else(|| now.clone());
            let pinned = if fm.pinned.unwrap_or(false) { 1 } else { 0 };

            let row = NoteRow {
                id: id.clone(),
                notebook_id: file.notebook_id.clone(),
                title: title.clone(),
                body: parsed.body.clone(),
                body_html: None,
                pinned,
                archived: 0,
                icon: fm.icon,
                color: fm.color,
                embedding_updated_at: None,
                split_content: None,
                split_mode: None,
                perspective_config: None,
                last_visited_at: None,
                created_at,
                updated_at: now.clone(),
            };

            sqlx::query(
                "INSERT INTO notes (id, notebook_id, title, body, body_html, pinned, archived, icon, color, embedding_updated_at, split_content, split_mode, perspective_config, last_visited_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            )
            .bind(&row.id)
            .bind(&row.notebook_id)
            .bind(&row.title)
            .bind(&row.body)
            .bind(&row.body_html)
            .bind(row.pinned)
            .bind(row.archived)
            .bind(&row.icon)
            .bind(&row.color)
            .bind(&row.embedding_updated_at)
            .bind(&row.split_content)
            .bind(&row.split_mode)
            .bind(&row.perspective_config)
            .bind(&row.last_visited_at)
            .bind(&row.created_at)
            .bind(&row.updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

            // Set tags if present
            if let Some(tags) = fm.tags {
                for tag in &tags {
                    sqlx::query("INSERT OR IGNORE INTO note_tags (note_id, tag) VALUES (?1, ?2)")
                        .bind(&id)
                        .bind(tag)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
                }
            }

            created_ids.push((id.clone(), title));
            if !parsed.body.is_empty() {
                content_events.push((id, parsed.body));
            }
            imported += 1;
        }

        tx.commit().await.map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        // Phase 3: Post-commit — domain events and embeddings
        if let Ok(bus) = self.domain_event_bus() {
            for (id, title) in &created_ids {
                bus.publish(bus::DomainEvent::NoteCreated {
                    note_id: id.clone(),
                    title: title.clone(),
                });
            }
            for (id, content) in &content_events {
                bus.publish(bus::DomainEvent::NoteContentChanged {
                    note_id: id.clone(),
                    content: content.clone(),
                });
            }
        }

        // Queue embeddings (batch — fire one spawn per note, same as existing note_create)
        if let Some(ref handler) = self.note_embedding_handler {
            for (id, _title) in &created_ids {
                let handler = std::sync::Arc::clone(handler);
                let repo = self.note_repo.clone();
                let note_id = id.clone();
                tokio::spawn(async move {
                    if let Ok(Some(row)) = repo.get_note(&note_id).await {
                        if let Err(e) = handler.embed_note(&row).await {
                            tracing::warn!(note_id, "import embedding failed (non-fatal): {e}");
                        } else {
                            let _ = repo.update_embedding_timestamp(&note_id).await;
                        }
                    }
                });
            }
        }

        Ok(NoteImportResult { imported, skipped })
    }

    /// Collect a single .md file for import.
    fn collect_file(
        &self,
        path: &Path,
        notebook_id: Option<String>,
        files: &mut Vec<CollectedFile>,
        skipped: &mut Vec<SkippedFile>,
    ) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("md") {
            skipped.push(SkippedFile {
                path: path.display().to_string(),
                reason: "Not a Markdown file".into(),
            });
            return;
        }
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_FILE_SIZE => {
                skipped.push(SkippedFile {
                    path: path.display().to_string(),
                    reason: "File too large".into(),
                });
            }
            Ok(_) => match std::fs::read_to_string(path) {
                Ok(content) => {
                    files.push(CollectedFile {
                        path: path.to_path_buf(),
                        content,
                        notebook_id,
                    });
                }
                Err(e) => {
                    skipped.push(SkippedFile {
                        path: path.display().to_string(),
                        reason: format!("Read error: {e}"),
                    });
                }
            },
            Err(e) => {
                skipped.push(SkippedFile {
                    path: path.display().to_string(),
                    reason: format!("Cannot read metadata: {e}"),
                });
            }
        }
    }

    /// Recursively collect .md files from a directory, creating notebooks as needed.
    async fn collect_dir(
        &self,
        dir: &Path,
        parent_notebook_id: Option<String>,
        files: &mut Vec<CollectedFile>,
        skipped: &mut Vec<SkippedFile>,
        visited_inodes: &mut HashSet<u64>,
    ) -> Result<(), ApiError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(dir) {
                if !visited_inodes.insert(meta.ino()) {
                    // Already visited — symlink cycle
                    return Ok(());
                }
            }
        }

        // Determine notebook for this directory
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Imported".into());

        let notebook_id = match self
            .note_repo
            .find_notebook_by_parent_and_title(parent_notebook_id.as_deref(), &dir_name)
            .await
        {
            Ok(Some(existing)) => existing.id,
            _ => {
                let now = utc_now_str();
                let nb = NotebookRow {
                    id: uuid::Uuid::new_v4().to_string(),
                    parent_id: parent_notebook_id.clone(),
                    title: dir_name,
                    icon: None,
                    color: None,
                    sort_order: 0,
                    created_at: now.clone(),
                    updated_at: now,
                };
                self.note_repo
                    .create_notebook(&nb)
                    .await
                    .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
                nb.id
            }
        };

        // Read directory entries sorted for deterministic order
        let mut entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(e) => {
                skipped.push(SkippedFile {
                    path: dir.display().to_string(),
                    reason: format!("Cannot read directory: {e}"),
                });
                return Ok(());
            }
        };
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            match std::fs::canonicalize(&path) {
                Ok(canonical) => {
                    if canonical.is_dir() {
                        Box::pin(self.collect_dir(
                            &canonical,
                            Some(notebook_id.clone()),
                            files,
                            skipped,
                            visited_inodes,
                        ))
                        .await?;
                    } else if canonical.is_file() {
                        self.collect_file(&canonical, Some(notebook_id.clone()), files, skipped);
                    }
                }
                Err(_) => {
                    skipped.push(SkippedFile {
                        path: path.display().to_string(),
                        reason: "Cannot resolve path".into(),
                    });
                }
            }
        }

        Ok(())
    }
}
```

**Note for implementor:** The `NoteRepo.pool()` public accessor was added in Task 2 Step 0. Verify that `bus::DomainEvent` is imported correctly in the `app-core` crate (check existing `crud.rs` imports for the pattern).

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app-core`
Expected: 0 errors (some warnings about unused `note_export` are fine — implemented in Task 6)

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/import_export.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(notes): add note_import_files handler with bulk import"
```

---

## Task 6: Export Handler (app-core)

**Files:**
- Modify: `crates/app-core/src/handlers/notes/import_export.rs`

- [ ] **Step 1: Add note_export handler**

Add the export method to the `impl AppCore` block in `import_export.rs`:

```rust
    pub async fn note_export(
        &self,
        params: desktop_shared::commands::NoteExportParams,
    ) -> Result<desktop_shared::commands::NoteExportResult, ApiError> {
        use desktop_shared::commands::NoteExportResult;

        // Validate
        let dest = PathBuf::from(&params.destination);
        if !dest.is_absolute() {
            return Err(ApiError::new("VALIDATION", "Destination must be an absolute path"));
        }
        if params.destination.contains("..") {
            return Err(ApiError::new("VALIDATION", "Path traversal not allowed"));
        }

        let has_notes = params.note_ids.as_ref().map_or(false, |v| !v.is_empty());
        let has_notebooks = params.notebook_ids.as_ref().map_or(false, |v| !v.is_empty());
        if !has_notes && !has_notebooks {
            return Err(ApiError::new("VALIDATION", "At least one of noteIds or notebookIds must be provided"));
        }

        // Collect notes
        let mut notes_with_notebook: Vec<(feature_notes::models::NoteRow, Option<String>)> = Vec::new();

        if let Some(ids) = &params.note_ids {
            for id in ids {
                if let Some(row) = self.note_repo.get_note(id).await.map_err(|e| ApiError::new("DB_ERROR", e.to_string()))? {
                    notes_with_notebook.push((row, None)); // unfiled for individual note export
                }
            }
        }

        if let Some(nb_ids) = &params.notebook_ids {
            for nb_id in nb_ids {
                self.collect_notebook_notes(nb_id, &dest, &mut notes_with_notebook).await?;
            }
        }

        // Get data_dir from config — same pattern as note_save_attachment in crud.rs:556
        let config = self.config.read().await;
        let data_dir = config.data_dir_path();
        drop(config);
        let attachment_prefix = format!("{}/attachments/", data_dir.display());
        let mut exported = 0u32;

        for (note, subdir) in &notes_with_notebook {
            let tags = self.note_repo.get_tags(&note.id).await.unwrap_or_default();
            let fm = front_matter::NoteFrontMatter {
                title: Some(note.title.clone()),
                tags: if tags.is_empty() { None } else { Some(tags) },
                created: Some(note.created_at.clone()),
                updated: Some(note.updated_at.clone()),
                pinned: if note.pinned != 0 { Some(true) } else { None },
                icon: note.icon.clone(),
                color: note.color.clone(),
            };

            // Handle attachments: rewrite absolute paths to relative
            let mut body = note.body.clone();
            if body.contains(&attachment_prefix) {
                let out_attachments = match subdir {
                    Some(sd) => dest.join(sd).join("attachments"),
                    None => dest.join("attachments"),
                };
                let _ = std::fs::create_dir_all(&out_attachments);
                // Replace each absolute attachment path with relative
                for line in note.body.lines() {
                    if let Some(start) = line.find(&attachment_prefix) {
                        // Extract the filename after the prefix
                        let after = &line[start + attachment_prefix.len()..];
                        let end = after.find(|c: char| c == ')' || c == '"' || c == ' ').unwrap_or(after.len());
                        let filename = &after[..end];
                        let src = PathBuf::from(&data_dir).join("attachments").join(filename);
                        if src.exists() {
                            let _ = std::fs::copy(&src, out_attachments.join(filename));
                        }
                        let abs_ref = format!("{}{}", attachment_prefix, filename);
                        let rel_ref = format!("./attachments/{}", filename);
                        body = body.replace(&abs_ref, &rel_ref);
                    }
                }
            }

            let content = front_matter::serialize(&fm, &body);

            // Determine output path
            let out_dir = match subdir {
                Some(sd) => dest.join(sd),
                None => dest.clone(),
            };
            let _ = std::fs::create_dir_all(&out_dir);

            let filename = if notes_with_notebook.len() == 1 && params.output_filename.is_some() {
                params.output_filename.clone().unwrap()
            } else {
                self.slugify_with_collision(&note.title, &out_dir)
            };

            let out_path = out_dir.join(&filename);
            tokio::fs::write(&out_path, content)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", format!("Failed to write {}: {e}", out_path.display())))?;

            exported += 1;
        }

        Ok(NoteExportResult { exported })
    }

    /// Recursively collect notes from a notebook, tracking subdirectory paths.
    async fn collect_notebook_notes(
        &self,
        notebook_id: &str,
        _base_dest: &Path,
        notes: &mut Vec<(feature_notes::models::NoteRow, Option<String>)>,
    ) -> Result<(), ApiError> {
        // Get notebook for its title (used as subdirectory name)
        let notebooks = self.note_repo.list_notebooks().await.map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
        let nb_map: HashMap<String, &feature_notes::models::NotebookRow> = notebooks.iter().map(|nb| (nb.id.clone(), nb)).collect();

        // Use raw notebook titles as directory names (not slugified).
        // This ensures export → import round-trip deduplicates correctly,
        // because import's find_notebook_by_parent_and_title matches on exact title.
        fn build_path(nb_id: &str, nb_map: &HashMap<String, &feature_notes::models::NotebookRow>) -> String {
            let mut parts = vec![];
            let mut current = Some(nb_id.to_string());
            while let Some(id) = current {
                if let Some(nb) = nb_map.get(&id) {
                    parts.push(nb.title.clone());
                    current = nb.parent_id.clone();
                } else {
                    break;
                }
            }
            parts.reverse();
            parts.join("/")
        }

        // Collect notes in this notebook
        let rows = self.note_repo.list_notes(Some(notebook_id)).await.map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
        let subdir = build_path(notebook_id, &nb_map);
        for row in rows {
            notes.push((row, Some(subdir.clone())));
        }

        // Recurse into child notebooks
        for nb in &notebooks {
            if nb.parent_id.as_deref() == Some(notebook_id) {
                Box::pin(self.collect_notebook_notes(&nb.id, _base_dest, notes)).await?;
            }
        }

        Ok(())
    }

    /// Create a URL-safe filename slug with collision handling.
    fn slugify_with_collision(&self, title: &str, dir: &Path) -> String {
        let base = slugify(title);
        let candidate = format!("{}.md", base);
        if !dir.join(&candidate).exists() {
            return candidate;
        }
        for i in 1..1000 {
            let candidate = format!("{}-{}.md", base, i);
            if !dir.join(&candidate).exists() {
                return candidate;
            }
        }
        format!("{}-{}.md", base, uuid::Uuid::new_v4())
    }
}

/// Slugify a string for use as a filename.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p app-core`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/notes/import_export.rs
git commit -m "feat(notes): add note_export handler with front matter and attachments"
```

---

## Task 7: Tauri Plugin Dialog Setup

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/capabilities/default.json`
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Add Rust dependency**

In `crates/desktop/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: Register plugin**

In `crates/desktop/src/lib.rs`, find where other plugins are registered (look for `.plugin(` calls in the builder chain) and add:

```rust
.plugin(tauri_plugin_dialog::init())
```

- [ ] **Step 3: Add capability permissions**

In `crates/desktop/capabilities/default.json`, add `"dialog:default"` to the `permissions` array.

- [ ] **Step 4: Add frontend dependency**

Run: `cd desktop-ui && bun add @tauri-apps/plugin-dialog`

- [ ] **Step 5: Verify full app builds**

Run: `cargo build -p desktop`
Expected: 0 errors

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/lib.rs crates/desktop/capabilities/default.json desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "feat(desktop): add tauri-plugin-dialog for file import/export dialogs"
```

---

## Task 8: Tauri Commands + DEV_COMMANDS

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`

- [ ] **Step 1: Add Tauri command functions**

Add before the `// ── Dev server dispatch` section in `crates/desktop/src/commands/notes.rs`:

```rust
// ── Import / Export ──────────────────────────────────────────────

#[tauri::command]
pub async fn note_import_files(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: desktop_shared::commands::NoteImportParams,
) -> Result<desktop_shared::commands::NoteImportResult, ApiError> {
    let result = state.note_import_files(params).await?;
    // Emit a generic entity update so the frontend refetches
    super::emit_updates(
        &app,
        &[::app_core::EntityUpdate {
            kind: desktop_shared::types::EntityKind::Note,
            id: "import".into(),
        }],
    );
    Ok(result)
}

#[tauri::command]
pub async fn note_export(
    state: State<'_, Arc<AppCore>>,
    _app: tauri::AppHandle,
    params: desktop_shared::commands::NoteExportParams,
) -> Result<desktop_shared::commands::NoteExportResult, ApiError> {
    state.note_export(params).await
}
```

- [ ] **Step 2: Add to DEV_COMMANDS**

Add `"note_import_files"` and `"note_export"` to the `DEV_COMMANDS` array.

- [ ] **Step 3: Add dispatch_dev entries**

Add to the `dispatch_dev` match block:

```rust
        "note_import_files" => Some(Err(ApiError::new(
            "UNSUPPORTED",
            "Import requires the desktop app",
        ))),
        "note_export" => Some(Err(ApiError::new(
            "UNSUPPORTED",
            "Export requires the desktop app",
        ))),
```

- [ ] **Step 4: Register commands in Tauri builder**

Find where commands are registered with `.invoke_handler(tauri::generate_handler![...])` in `crates/desktop/src/lib.rs` and add `note_import_files` and `note_export` to the list.

- [ ] **Step 5: Verify the dev_server test passes**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/notes.rs crates/desktop/src/lib.rs
git commit -m "feat(desktop): wire note_import_files and note_export Tauri commands"
```

---

## Task 9: Frontend Types

**Files:**
- Modify: `desktop-ui/src/shared/types/notes.ts`

- [ ] **Step 1: Extend NoteCreateParams**

In `desktop-ui/src/shared/types/notes.ts`, add to `NoteCreateParams`:

```typescript
export interface NoteCreateParams {
  title: string;
  notebookId?: string;
  body?: string;
  tags?: string[];
  createdAt?: string;
  icon?: string;
  color?: string;
}
```

- [ ] **Step 2: Add import/export types**

Add at the end of the file:

```typescript
// ── Import / Export ──────────────────────────────────────────

export interface NoteImportParams {
  paths: string[];
  notebookId?: string;
}

export interface NoteImportResult {
  imported: number;
  skipped: SkippedFile[];
}

export interface SkippedFile {
  path: string;
  reason: string;
}

export interface NoteExportParams {
  noteIds?: string[];
  notebookIds?: string[];
  destination: string;
  outputFilename?: string;
}

export interface NoteExportResult {
  exported: number;
}
```

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/types/notes.ts
git commit -m "feat(notes): add import/export TypeScript types"
```

---

## Task 10: Frontend — Import/Export UX in NotebookTree

**Files:**
- Modify: `desktop-ui/src/features/notes/components/NotebookTree.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

This task adds:
1. External file drop detection on the tree
2. "Import files..." / "Import folder..." / "Export as Markdown..." context menu items
3. Handler functions in KnowledgeBasePage

- [ ] **Step 1: Add import/export props to NotebookTree**

`NotebookTree` is a controlled component — it receives callbacks from `KnowledgeBasePage`. Add new props for import/export:

```typescript
// In NotebookTree's props type:
onImportFiles?: (paths: string[], notebookId?: string) => void;
onExportNote?: (noteId: string) => void;
onExportNotebook?: (notebookId: string) => void;
```

- [ ] **Step 2: Add external file drop detection**

In the drag-and-drop handlers within `NotebookTree`, detect external file drops by checking `e.dataTransfer.types` for `"Files"`. When external files are dropped:
- Extract file paths from the drop event (use Tauri's `DragDropEvent` or `e.dataTransfer.files`)
- Determine target notebook from the drop target (notebook row, note's notebook, or root = unfiled)
- Call `onImportFiles(paths, notebookId)`

- [ ] **Step 3: Add context menu items**

In `TreeContextMenu`:
- **For `kind: "blank"`**: Add "Import files..." and "Import folder..." items
- **For `kind: "folder"`**: Add "Import files...", "Import folder...", and "Export as Markdown..." items
- **For `kind: "note"`**: Add "Export as Markdown..." item

Import items use `@tauri-apps/plugin-dialog` → `open()` to get file/folder paths, then call `onImportFiles`.
Export items call `onExportNote` or `onExportNotebook`.

- [ ] **Step 4: Wire handlers in KnowledgeBasePage**

In `KnowledgeBasePage.tsx`:

```typescript
import { open, save } from "@tauri-apps/plugin-dialog";

const importMutation = useMutation("note_import_files", "params");
const exportMutation = useMutation("note_export", "params");

const handleImportFiles = async (paths: string[], notebookId?: string) => {
  const result = await importMutation.mutate({ paths, notebookId });
  // Show toast with result.imported count and result.skipped length
};

const handleExportNote = async (noteId: string) => {
  const note = notes.find((n) => n.id === noteId);
  if (!note) return;
  const path = await save({
    defaultPath: `${note.title}.md`,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
  if (!path) return;
  const dir = path.substring(0, path.lastIndexOf("/"));
  const filename = path.substring(path.lastIndexOf("/") + 1);
  await exportMutation.mutate({
    noteIds: [noteId],
    destination: dir,
    outputFilename: filename,
  });
  // Show toast
};

const handleExportNotebook = async (notebookId: string) => {
  const dir = await open({ directory: true });
  if (!dir) return;
  await exportMutation.mutate({
    notebookIds: [notebookId],
    destination: dir as string,
  });
  // Show toast
};
```

Pass these as props to `NotebookTree`.

- [ ] **Step 5: Run lint and check**

Run: `cd desktop-ui && bun run lint:fix`
Expected: 0 errors after auto-fix

- [ ] **Step 6: Manual smoke test**

Run: `cd desktop-ui && bun run dev` (in one terminal) and `cargo tauri dev` (in another).
- Right-click blank area → "Import files..." should open file picker
- Right-click notebook → "Import files..." should work
- Right-click note → "Export as Markdown..." should open save dialog
- Drag `.md` files from Finder onto a notebook in the tree → should trigger import

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/notes/components/NotebookTree.tsx desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx
git commit -m "feat(notes): add import/export UI — drag-and-drop, context menus, file dialogs"
```

---

## Task 11: Integration Test

**Files:**
- Modify: `crates/app-core/src/handlers/notes/import_export.rs` (add test module)

- [ ] **Step 1: Write integration test for import round-trip**

Add a `#[cfg(test)]` module at the bottom of `import_export.rs` that:
1. Creates a temp directory with `.md` files (some with front matter, some without, one non-`.md` file)
2. Calls `note_import_files` with the temp dir path
3. Asserts correct `imported` count and `skipped` entries
4. Reads back notes from DB and verifies title, body, tags, timestamps

**Note for implementor:** This test needs an `AppCore` with an in-memory `StoragePool`. Follow the pattern from existing tests in `tests/` — look at how `AppCore` is constructed in test helpers. If constructing a full `AppCore` in a unit test is too complex, write this as an integration test in the `tests/` directory instead.

- [ ] **Step 2: Write integration test for export round-trip**

1. Create notes in the DB via `note_create`
2. Call `note_export` to a temp directory
3. Read back the exported `.md` files
4. Verify front matter contains correct title, tags, timestamps
5. Verify body matches

- [ ] **Step 3: Write round-trip test: export → import**

1. Create notes with various metadata
2. Export to temp dir
3. Import from that temp dir into a fresh DB
4. Verify the imported notes match the originals

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 6: Run fmt check**

Run: `cargo fmt --all --check`
Expected: 0 formatting issues

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/notes/import_export.rs
git commit -m "test(notes): add import/export integration tests"
```
