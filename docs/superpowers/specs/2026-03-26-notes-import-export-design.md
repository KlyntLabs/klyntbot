# Notes Import/Export Design

**Date:** 2026-03-26
**Status:** Approved
**Scope:** Import `.md` files/folders into the knowledge base; export notes/notebooks as `.md` files with YAML front matter.

## Motivation

Users should be able to treat their notes as portable Markdown files — importable from Obsidian vaults or any folder of `.md` files, and exportable for sharing, backup, or use with other tools. This is the first step toward an Obsidian-style vault model.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Import folder structure | Mirror directories → notebooks | Matches Obsidian mental model; degrades to flat for flat folders |
| Export scope | Single note + entire notebook | Both from context menus; share the same backend command |
| Front matter | Always include on export | Obsidian-compatible; preserves round-trip fidelity |
| Front matter on import | Parse and apply known fields | Completes the round-trip; unknown fields silently ignored |
| Import triggers | Drag-and-drop + context menu file picker | Power users drag; discoverability via menu |
| Architecture | Hybrid — backend commands, frontend UX | Atomic bulk import, fast; frontend owns drag-and-drop + dialogs |
| Non-.md files | Silently skipped, reported in result | No error dialogs for mixed folders |

## Section 1: Data Model Changes

### Front Matter Struct

New struct in `feature-notes` for parsing/serializing YAML front matter:

```rust
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NoteFrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,     // ISO 8601
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,     // ISO 8601 — maps to `updated_at` in Note model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
```

- Parsed with `serde_yml` (maintained fork of `serde_yaml`, which is archived). New dependency on `feature-notes`.
- `#[serde(default)]` allows partial front matter. `#[serde(skip_serializing_if)]` omits `None` fields on export.
- Unknown fields are silently ignored (no `deny_unknown_fields`). Forward-compatible with Obsidian fields like `aliases`, `cssclass`, etc.
- **Field name mapping:** YAML key `updated` maps to the `updated_at` field in the Note domain model. The YAML key name follows Obsidian convention; conversion happens in the import/export handler.

### NoteCreateParams Extended

`NoteCreateParams` (in `desktop-shared`) gains optional fields:

```rust
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    // New fields:
    pub created_at: Option<String>,  // ISO 8601 — import preserves original timestamps
    pub icon: Option<String>,
    pub color: Option<String>,
}
```

The `note_create` handler in `app-core` uses these if present, falling back to `utc_now_str()` for `created_at` and `None` for icon/color.

**No schema migration needed** — the `notes` table already has all these columns.

## Section 2: Backend Commands

### `note_import_files`

New Tauri command for bulk importing `.md` files and directories.

**IPC types** (in `desktop-shared`):

```rust
pub struct NoteImportParams {
    pub paths: Vec<String>,          // file or directory paths from frontend
    pub notebook_id: Option<String>, // target notebook (None = unfiled)
}

pub struct NoteImportResult {
    pub imported: u32,
    pub skipped: Vec<SkippedFile>,
}

pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}
```

**Handler logic** (in `app-core`):

1. **Validate paths:** All input `paths` must be absolute. Reject any path containing `..` components to prevent path traversal. Canonicalize via `std::fs::canonicalize()` before reading.
2. **Collect files:** Walk all `paths`. Directories are recursed. Only `.md`/`.MD` files (case-insensitive extension check) are collected; everything else is added to `skipped` with reason "Not a Markdown file". **File size limit:** files over 50 MB are skipped with reason "File too large". **Symlink cycle detection:** track visited inodes in a `HashSet<u64>` during directory walk; skip any directory whose inode has already been visited.
3. **Create notebook structure:** For directories, create notebooks mirroring the subfolder hierarchy (depth-first). Parent directories become parent notebooks. Notebook names derived from directory names. **Deduplication:** before creating a notebook, look up by `(parent_id, title)` — if a matching notebook already exists, reuse it. This prevents duplicate notebooks on repeated imports.
4. **Parse each `.md` file:**
   - Front matter is **only** parsed when the file begins with `---\n` at byte 0. A `---` appearing elsewhere in the file (e.g., a horizontal rule) is NOT treated as front matter.
   - Split content at the first `---` / `---` boundary to extract YAML front matter
   - Parse front matter via `serde_yml::from_str::<NoteFrontMatter>()`
   - Body = everything after the closing `---`
   - If no front matter or malformed YAML: entire file content is body (add warning to `skipped` for malformed YAML)
   - Title resolution order: `front_matter.title` → filename (sans `.md` extension, case-insensitive strip)
5. **Bulk insert:** All notes inserted in a **single SQLite transaction**. If any insert fails, the entire import is rolled back.
6. **Post-commit (after `tx.commit()` succeeds):** Batch-fire `DomainEvent::NoteCreated` for each imported note. Queue embeddings (batched, not one-per-note fire-and-forget). Events MUST be published only after the transaction commits — never inside the transaction — so consumers querying the DB see the committed data.
7. **Return** `NoteImportResult` with `imported` count and `skipped` list.

**Tauri command signature:**

```rust
#[tauri::command]
async fn note_import_files(app: AppHandle, params: NoteImportParams) -> Result<NoteImportResult, String>
```

**Front matter parsing** lives in `feature-notes` as a reusable module (`front_matter.rs`), shared by both import and export.

### `note_export`

New Tauri command for exporting notes as `.md` files.

**IPC types** (in `desktop-shared`):

```rust
pub struct NoteExportParams {
    pub note_ids: Option<Vec<String>>,     // specific notes
    pub notebook_ids: Option<Vec<String>>, // entire notebooks (recursive)
    pub destination: String,               // directory path from save/open dialog
    pub output_filename: Option<String>,   // explicit filename for single-note export (from save dialog)
}

pub struct NoteExportResult {
    pub exported: u32,
}
```

**Tauri command signature:**

```rust
#[tauri::command]
async fn note_export(app: AppHandle, params: NoteExportParams) -> Result<NoteExportResult, String>
```

**Handler logic** (in `app-core`):

1. **Validate:** `destination` must be an absolute path. Reject paths containing `..`. At least one of `note_ids` or `notebook_ids` must be `Some` with a non-empty vec — otherwise return a validation error.
2. **Collect notes:** If `note_ids` provided, fetch those notes. If `notebook_ids` provided, fetch all notes in those notebooks recursively (including sub-notebooks). Both can be provided (union).
3. **For each note:**
   - Build `NoteFrontMatter` from note metadata (title, tags, created_at, updated_at → `updated`, pinned, icon, color)
   - Serialize: `---\n{yaml}\n---\n\n{body}`
   - Filename: if `output_filename` is set and this is a single-note export, use it directly (user chose this via save dialog). Otherwise: slugified title (lowercase, spaces → hyphens, strip non-alphanumeric), collision handling with `-1`, `-2`, etc.
4. **Directory structure:** Notebook hierarchy maps to subdirectories. Unfiled notes go directly in `destination`.
5. **Attachments:** Scan Markdown body for references to `{data_dir}/attachments/` paths. For each found:
   - Copy the file to `{destination}/attachments/{original-filename}`
   - Rewrite the absolute path in the Markdown body to a relative `./attachments/{filename}` reference
6. **Write files** to `destination`.
7. **Return** `NoteExportResult` with `exported` count.

**Note on attachment round-trip:** On import, relative `./attachments/` references in Markdown body are left as-is (not rewritten to absolute paths). The frontend resolves them relative to the import source at display time, or the user re-saves to trigger attachment path normalization.

## Section 3: Frontend — Import UX

Three entry points converge on one handler.

### Drag-and-Drop onto NotebookTree

Extend the existing HTML5 drag-and-drop in `NotebookTree.tsx`:

- **Detection:** Check `e.dataTransfer.types` includes `"Files"` (external drop) vs `"application/json"` (internal note/notebook move). External drops go to import flow; internal drops use existing move logic.
- **Drop targets:**
  - Drop onto a notebook → import with `notebookId` set to that notebook
  - Drop onto root zone → import as unfiled (`notebookId: null`)
  - Drop onto a note → import into that note's notebook
- **Visual feedback:** Reuse existing highlight (`bg-brand/[0.12] ring-1 ring-brand/40`). Optionally show a tooltip/badge with file count during drag-over.
- **File path extraction:** Use Tauri's file drop event or `e.dataTransfer.files` to get paths.

### Context Menu "Import files..."

Add a new item to two existing context menu variants:

- **Blank area context menu** (`kind: "blank"`): Add "Import files..." → opens file picker → imports as unfiled
- **Notebook context menu** (`kind: "folder"`): Add "Import files..." → opens file picker → imports into that notebook

File picker uses `@tauri-apps/plugin-dialog` → `open({ multiple: true, directory: false, filters: [{ name: "Markdown", extensions: ["md"] }] })`. Also allow `directory: true` as a separate "Import folder..." option or combined.

### Shared Handler

```typescript
async function handleImportFiles(paths: string[], notebookId?: string) {
  const result = await ipc("note_import_files", {
    params: { paths, notebook_id: notebookId }
  });
  // Toast notification: "Imported {n} notes" + skipped count if any
  // Refetch notes list via invalidateQueries("note")
}
```

**No progress bar for v1.** Single-transaction bulk insert is fast. Progress feedback can be added later if large vaults prove slow.

## Section 4: Frontend — Export UX

Two entry points.

### Single Note Export

- **Trigger:** Note context menu (`kind: "note"`) → "Export as Markdown..."
- **Dialog:** `@tauri-apps/plugin-dialog` → `save({ defaultPath: "{note-title}.md", filters: [{ name: "Markdown", extensions: ["md"] }] })`
- **IPC:** Extract `parentDir` and `filename` from the save dialog result path. Call `ipc("note_export", { params: { noteIds: [id], destination: parentDir, outputFilename: filename } })` — this ensures the exported file uses the exact name the user chose in the save dialog.
- **Toast:** "Exported to {path}"

### Notebook Export

- **Trigger:** Notebook context menu (`kind: "folder"`) → "Export as Markdown..."
- **Dialog:** `@tauri-apps/plugin-dialog` → `open({ directory: true })` (pick destination folder)
- **IPC:** `ipc("note_export", { params: { notebookIds: [id], destination: dir } })`
- **Result:** Creates a folder named after the notebook inside the chosen destination, with sub-notebooks as subdirectories.
- **Toast:** "Exported {n} notes to {path}"

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Empty `.md` file | Import with empty body, title from filename |
| `.md` file with only front matter, no body | Import with empty body, metadata from front matter |
| Malformed YAML front matter | Skip front matter parsing, treat entire content as body, add to `skipped` with warning |
| Duplicate title in same notebook on import | Allowed — notes are ID-based, titles aren't unique |
| Filename collision on export | Append `-1`, `-2`, etc. to slug |
| Broken attachment reference on export | Leave the path as-is in Markdown, skip file copy, log warning |
| Circular notebook references (symlinks in imported folder) | Inode-based cycle detection via `HashSet<u64>` — skip directories whose inode was already visited |
| Very large import (1000+ files) | Single transaction; no streaming. If this becomes a perf issue, add batching later |
| File over 50 MB | Skip with reason "File too large" |
| Path traversal attempt (`..` in paths) | Reject with validation error |
| Both `note_ids` and `notebook_ids` are None on export | Validation error — at least one must be provided |

## Dependencies

### New Rust Dependencies
- `serde_yml` on `feature-notes` (YAML front matter parsing/serialization — maintained fork of the archived `serde_yaml`)

### Tauri Plugins
Tauri 2 plugins require changes in three places:

1. **Rust crate** (`crates/desktop/Cargo.toml`): add `tauri-plugin-dialog`
2. **Plugin registration** (`crates/desktop/src/lib.rs` or `setup()`): register `.plugin(tauri_plugin_dialog::init())`
3. **Capabilities** (`crates/desktop/capabilities/default.json`): add `"dialog:default"` (or specific `"dialog:allow-open"`, `"dialog:allow-save"`)
4. **Frontend npm** (`desktop-ui/package.json`): add `@tauri-apps/plugin-dialog`

For file drop events: Tauri 2's built-in `DragDropEvent` (via `app.listen("tauri://drag-drop")`) provides file paths without needing `@tauri-apps/plugin-fs`.

## Files to Create/Modify

### Rust (backend)
- `crates/feature-notes/src/front_matter.rs` — **new** — `NoteFrontMatter` struct, `parse()`, `serialize()` functions
- `crates/feature-notes/src/lib.rs` — add `pub mod front_matter`
- `crates/desktop-shared/src/commands/notes.rs` — add `NoteImportParams`, `NoteImportResult`, `SkippedFile`, `NoteExportParams`, `NoteExportResult`; extend `NoteCreateParams`
- `crates/app-core/src/handlers/notes/crud.rs` — update `note_create` to accept new fields; add `note_import_files` and `note_export` handlers
- `crates/desktop/src/commands/notes.rs` — add `note_import_files` and `note_export` Tauri commands; **must** add both to `DEV_COMMANDS` array and `dispatch_dev` match arms (`dev_server_covers_all_tauri_commands` test enforces this)
- `crates/desktop/Cargo.toml` — add `tauri-plugin-dialog` dependency
- `crates/desktop/src/lib.rs` — register `tauri_plugin_dialog::init()` plugin
- `crates/desktop/capabilities/default.json` — add dialog permissions

### TypeScript (frontend)
- `desktop-ui/package.json` — add `@tauri-apps/plugin-dialog` dependency
- `desktop-ui/src/shared/types/notes.ts` — add import/export param/result types; extend `NoteCreateParams`
- `desktop-ui/src/features/notes/components/NotebookTree.tsx` — extend drag-and-drop to detect external file drops; add "Import files..." to context menus; add "Export as Markdown..." to context menus
- `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` — add `handleImportFiles` and `handleExport` handlers; wire mutations

## Non-Goals

- Full vault sync (file watcher, bidirectional sync) — that's a future phase
- Progress bar / streaming import — not needed at current scale
- Import from non-Markdown formats (HTML, Notion export, etc.)
- Custom export templates or formats
