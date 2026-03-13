# Workspace Config Editor — Design Spec

**Date:** 2026-03-13
**Status:** Approved

## Summary

Add a collapsible "System Config" section to the bottom of the Notes page sidebar, allowing users to view and edit workspace configuration files (SOUL.md, AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, IDENTITY.md, HEARTBEAT.md) directly in the existing ProseMirror editor. Also fix backend gaps: add HEARTBEAT.md to bootstrap, embed and copy templates on first init.

## Motivation

Workspace files define the agent's personality, response style, and operational instructions. Currently they can only be edited by navigating the filesystem manually. Exposing them in the UI makes customization accessible and discoverable. Several gaps exist between documented and actual behavior that should be fixed simultaneously.

## Backend: Workspace File Commands

### New app-core handler: `crates/app-core/src/handlers/workspace.rs`

Three operations:

- **`list_workspace_files()`** — Returns `Vec<WorkspaceFile>` with name, description, and exists flag for each allowed file.
- **`read_workspace_file(filename: &str)`** — Reads file from workspace directory. If file doesn't exist, returns the embedded template content. Validates filename against hardcoded whitelist.
- **`write_workspace_file(filename: &str, content: &str)`** — Writes content to file. Validates filename against whitelist (no path traversal).

### Allowed files (hardcoded whitelist)

```
SOUL.md, AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, IDENTITY.md, HEARTBEAT.md
```

### Shared types: `crates/desktop-shared/`

```rust
struct WorkspaceFile {
    name: String,
    description: String,  // e.g., "Agent personality and values"
    exists: bool,
}
```

### Desktop adapter: `crates/desktop/src/commands/workspace.rs`

Thin Tauri command adapter delegating to AppCore. Also exposed via dev_server for browser-only dev.

## Backend: Gap Fixes

### Fix 1: HEARTBEAT.md in BOOTSTRAP_FILES

Add `"HEARTBEAT.md"` to the `BOOTSTRAP_FILES` array in `crates/agent/src/context_sources/bootstrap.rs`.

### Fix 2: Embed and copy templates on first init

In `crates/config/src/loader.rs`:
- Use `include_str!()` to embed the 7 template files from `workspace/` at compile time.
- In `init()`, after `create_dir_all("workspace")`, check each file — if it doesn't exist, write the embedded template.
- Files that already exist are never overwritten (user edits preserved).

### Caching note

BootstrapSource uses `OnceCell` — workspace file edits require agent restart. The UI shows a hint: "Restart agent to apply changes."

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
│   ├── IDENTITY.md       │
│   └── HEARTBEAT.md      │
└─────────────────────────┘
```

### Behavior

- Collapsed by default. Collapse state persisted in `localStorage`.
- Clicking a file fetches content via `workspace_read_file` and opens in the existing ProseMirror editor.
- Active file highlighted with same styling as selected note.
- Save triggers `workspace_write_file` using existing auto-save debounce (1000ms).
- Small muted hint below editor title: "Restart agent to apply changes."
- Filenames styled in monospace with `text-muted` color to distinguish from user notes.

### State management

- New state: `activeWorkspaceFile: string | null` in the notes store.
- Mutually exclusive with `selectedNoteId` — selecting a workspace file clears the note selection and vice versa.
- Editor content source switches based on which is active.

### Frontend IPC

- `useQuery("workspace_list_files", {}, [])` — load file list on mount
- `ipc("workspace_read_file", { filename })` — load file content on click
- `ipc("workspace_write_file", { filename, content })` — save on debounced change

## Files to Create/Modify

### Create
- `crates/app-core/src/handlers/workspace.rs` — handler logic
- `crates/desktop/src/commands/workspace.rs` — Tauri command adapter
- `desktop-ui/src/features/notes/components/WorkspaceFileTree.tsx` — sidebar section component

### Modify
- `crates/app-core/src/handlers/mod.rs` — register workspace handler
- `crates/app-core/src/lib.rs` — expose workspace methods on AppCore
- `crates/desktop/src/commands/mod.rs` — register Tauri commands
- `crates/desktop/src/dev_server/` — add dev server endpoints
- `crates/desktop-shared/src/lib.rs` — add WorkspaceFile type
- `crates/agent/src/context_sources/bootstrap.rs` — add HEARTBEAT.md
- `crates/config/src/loader.rs` — embed and copy templates
- `desktop-ui/src/features/notes/components/FileTree.tsx` — add WorkspaceFileTree below notebooks
- `desktop-ui/src/features/notes/pages/NotesPage.tsx` — handle workspace file selection/editing state
- `desktop-ui/src/features/notes/stores/` or equivalent state — add activeWorkspaceFile

## Out of Scope

- Implementing the HEARTBEAT.md 30-minute cron checker (separate feature)
- Implementing `edit_file`/`exec` agent tools described in AGENTS.md
- Hot-reloading workspace files without restart
- Structured form editing for USER.md
