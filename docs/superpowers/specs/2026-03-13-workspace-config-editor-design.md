# Workspace Config Editor — Design Spec

**Date:** 2026-03-13
**Status:** Approved

## Summary

Add a collapsible "System Config" section to the bottom of the Notes page sidebar, allowing users to view and edit workspace configuration files (SOUL.md, AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, HEARTBEAT.md) directly in the ProseMirror editor. Also fix backend gaps: add HEARTBEAT.md to bootstrap, embed and copy templates on first init.

## Motivation

Workspace files define the agent's personality, response style, and operational instructions. Currently they can only be edited by navigating the filesystem manually. Exposing them in the UI makes customization accessible and discoverable. Several gaps exist between documented and actual behavior that should be fixed simultaneously.

## Backend: Workspace File Commands

### New app-core handler: `crates/app-core/src/handlers/workspace.rs`

Flat handler file (3 operations, similar to `cron.rs`/`timeline.rs`). Must be registered in `crates/app-core/src/handlers/mod.rs` as `pub mod workspace;`.

Three operations, each accessing workspace path via `self.config.read().await.workspace_path()`:

- **`list_workspace_files()`** — Returns `Vec<WorkspaceFile>` with name, description, and exists flag for each allowed file.
- **`read_workspace_file(filename: &str)`** — Reads file from workspace directory. If file doesn't exist, returns the embedded template content. Validates filename against hardcoded whitelist.
- **`write_workspace_file(filename: &str, content: &str)`** — Writes content to file. Validates filename against whitelist (no path traversal).

### Allowed files (hardcoded whitelist)

```
SOUL.md, AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, HEARTBEAT.md
```

Note: `IDENTITY.md` is in `BOOTSTRAP_FILES` but has no template in the repo. Exclude from the UI whitelist. If a user manually creates it, the agent will still load it.

### Shared types: `crates/desktop-shared/src/commands/workspace.rs`

New file, registered in `crates/desktop-shared/src/commands/mod.rs`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFile {
    pub name: String,
    pub description: String,  // e.g., "Agent personality and values"
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileContent {
    pub name: String,
    pub content: String,
}
```

### Desktop adapter: `crates/desktop/src/commands/workspace.rs`

Thin Tauri command adapter delegating to AppCore. Each command function must be added to the `tauri::generate_handler![...]` invocation in the desktop crate's setup (typically `crates/desktop/src/lib.rs`).

Each command file also defines a `dispatch_dev()` function for the dev server. Register it in `crates/desktop/src/dev_server/dispatch.rs` with `if let Some(r) = commands::workspace::dispatch_dev(...).await`.

## Backend: Gap Fixes

### Fix 1: HEARTBEAT.md in BOOTSTRAP_FILES

Add `"HEARTBEAT.md"` to the `BOOTSTRAP_FILES` array in `crates/agent/src/context_sources/bootstrap.rs`.

### Fix 2: Embed and copy templates on first init

In `crates/config/src/loader.rs`:
- Use `include_str!()` to embed the 6 template files at compile time. Paths are relative to the source file, so use `include_str!("../../../workspace/SOUL.md")` etc.
- In `init()`, after `create_dir_all("workspace")`, check each file — if it doesn't exist, write the embedded template.
- Files that already exist are never overwritten (user edits preserved).

### Caching note

BootstrapSource uses `OnceCell` — edits to bootstrap files (SOUL.md, AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, HEARTBEAT.md) require agent restart to take effect. The UI shows a muted hint: "Restart agent to apply changes."

## Frontend: Collapsible System Config Section

### Location

Bottom of the existing FileTree sidebar in NotesPage, below notebooks.

### UI structure

```
┌─────────────────────────┐
│ Notebooks               │  ← existing
│   ├── Personal          │
│   ├── Work              │
│   └── Ideas             │
│                         │
│ ─────────────────────── │  ← subtle divider
│ ⚙ System Config    ▾   │  ← collapsible header (collapsed by default)
│   ├── SOUL.md           │
│   ├── AGENTS.md         │
│   ├── USER.md           │
│   ├── TOOLS.md          │
│   ├── RESPONSE.md       │
│   └── HEARTBEAT.md      │
└─────────────────────────┘
```

### Behavior

- Collapsed by default. Collapse state persisted in `localStorage`.
- Clicking a file fetches content via `workspace_read_file` and opens in the ProseMirror editor pane.
- Active file highlighted with same styling as selected note.
- Save triggers `workspace_write_file` using a debounced callback (1000ms), implemented locally in NotesPage (not reusing NoteEditor's internal debounce, which is coupled to Note objects).
- Small muted hint below editor title: "Restart agent to apply changes."
- Filenames styled in monospace with `text-muted` color to distinguish from user notes.

### Editor integration

NoteEditor expects a `Note` object. For workspace files, construct a synthetic Note-like object:
- `id`: the filename (e.g., `"__workspace__SOUL.md"`)
- `body`: the raw markdown content from `workspace_read_file`
- `title`: the filename
- Other fields: sensible defaults (empty tags, not pinned, etc.)

The save callback is swapped: instead of calling `note_update`, it calls `workspace_write_file` with the editor's text content. NotesPage already controls which save handler the editor uses via props — workspace mode passes a different `onSave`.

### State management

All state is local `useState` in `NotesPage.tsx` (there is no notes store):
- New state: `activeWorkspaceFile: string | null`
- Mutually exclusive with `selectedNoteId` — selecting a workspace file sets `selectedNoteId` to null and vice versa.
- Editor content source switches based on which is active.

### Frontend IPC

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { ipc } from "@shared/hooks/useIpc";

// Load file list on mount
const { data: workspaceFiles } = useQuery<WorkspaceFile[]>("workspace_list_files", undefined, []);

// Load file content on click
const content = await ipc<WorkspaceFileContent>("workspace_read_file", { filename });

// Save on debounced change (via useMutation or direct ipc call)
await ipc("workspace_write_file", { filename, content: newContent });
```

## Files to Create/Modify

### Create
- `crates/app-core/src/handlers/workspace.rs` — handler logic
- `crates/desktop/src/commands/workspace.rs` — Tauri command adapter + `dispatch_dev()`
- `crates/desktop-shared/src/commands/workspace.rs` — shared types
- `desktop-ui/src/features/notes/components/WorkspaceFileTree.tsx` — sidebar section component

### Modify
- `crates/app-core/src/handlers/mod.rs` — add `pub mod workspace;`
- `crates/app-core/src/lib.rs` — expose workspace methods on AppCore
- `crates/desktop/src/commands/mod.rs` — add `pub mod workspace;`
- `crates/desktop/src/lib.rs` — add workspace commands to `generate_handler![]`
- `crates/desktop/src/dev_server/dispatch.rs` — register workspace dispatch
- `crates/desktop-shared/src/commands/mod.rs` — add `pub mod workspace;`
- `crates/agent/src/context_sources/bootstrap.rs` — add HEARTBEAT.md to BOOTSTRAP_FILES
- `crates/config/src/loader.rs` — embed templates via `include_str!()`, copy on init
- `desktop-ui/src/features/notes/components/FileTree.tsx` — render WorkspaceFileTree below notebooks
- `desktop-ui/src/features/notes/pages/NotesPage.tsx` — add `activeWorkspaceFile` useState, wire editor mode switching, workspace save handler

## Out of Scope

- Implementing the HEARTBEAT.md 30-minute cron checker (separate feature)
- Implementing `edit_file`/`exec` agent tools described in AGENTS.md
- Hot-reloading workspace files without restart
- Structured form editing for USER.md
- IDENTITY.md template creation (no template exists; agent loads it if user creates it manually)
