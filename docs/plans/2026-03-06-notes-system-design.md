# Notes System Design

## Overview

Full-featured notes system for klyntbot, inspired by HelixNotes. Rich text editor (TipTap v3), notebook hierarchy, wiki-links, knowledge graph, entity mentions, full-text + semantic search, and version history.

**Approach:** Hybrid — port TipTap editor config from HelixNotes (framework-agnostic), build everything else fresh using existing klyntbot patterns.

## Phasing

- **Phase 1 (Core):** Rich editor + notes CRUD + notebook hierarchy + note list
- **Phase 2 (Knowledge):** Wiki-links + graph view + full-text search (Tantivy) + vector embeddings (LanceDB) + tags
- **Phase 3 (Integration):** Entity mentions (@task, @project) + cross-feature linking + version history + image paste

## Architecture

### Backend: `feature-notes` Crate

New crate following `feature-*` pattern (FeaturePackage with tools + migrations + config).

```
crates/feature-notes/
  src/
    lib.rs          -- FeaturePackage impl
    repo.rs         -- NotesRepo, NotebookRepo (SQLite)
    search.rs       -- Tantivy full-text index
    history.rs      -- Version snapshots in SQLite
    models.rs       -- Note, Notebook, NoteVersion, NoteLink
```

### SQLite Schema

```sql
CREATE TABLE notebooks (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT REFERENCES notebooks(id),
    title       TEXT NOT NULL,
    icon        TEXT,
    sort_order  INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE notes (
    id          TEXT PRIMARY KEY,
    notebook_id TEXT REFERENCES notebooks(id),
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,       -- Markdown
    body_html   TEXT,                -- Rendered HTML for fast display
    pinned      INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE note_tags (
    note_id TEXT REFERENCES notes(id),
    tag     TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);

CREATE TABLE note_links (
    source_id TEXT REFERENCES notes(id),
    target_id TEXT REFERENCES notes(id),
    PRIMARY KEY (source_id, target_id)
);

CREATE TABLE note_entity_mentions (
    note_id     TEXT REFERENCES notes(id),
    entity_type TEXT NOT NULL,   -- 'task', 'project', 'session'
    entity_id   TEXT NOT NULL,
    PRIMARY KEY (note_id, entity_type, entity_id)
);

CREATE TABLE note_versions (
    id         TEXT PRIMARY KEY,
    note_id    TEXT REFERENCES notes(id),
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

### LanceDB Vectors

Embed note content into existing LanceDB via context_engine. On save: chunk + embed + upsert. Enables semantic search and agent context retrieval.

### Search

Tantivy index (ported from HelixNotes) for full-text. Fields: path, title, body, tags. Rebuilt on startup, incremental updates on note changes. Combined with LanceDB semantic search, results merged and deduped.

## Frontend

### Layout

New top-level sidebar entry "Notes" at route `#/notes`. Three-panel workspace:

```
[App Sidebar] [Note List Panel] [Editor Panel]
```

### Component Tree

```
src/components/notes/
  NotesView.tsx              -- Route wrapper, three-panel layout
  NotebookTree.tsx           -- Folder hierarchy sidebar
  NoteList.tsx               -- Sortable, filterable note cards
  NoteCard.tsx               -- Single note preview
  NoteEditor.tsx             -- Orchestrator: toolbar + editor + metadata
  editor/
    EditorCore.tsx           -- TipTap useEditor() + extensions
    EditorToolbar.tsx        -- Formatting buttons
    SlashCommandMenu.tsx     -- "/" keystroke popup
    WikiLinkNode.tsx         -- Custom TipTap node for [[links]]
    EntityMention.tsx        -- Custom TipTap node for @mentions
    CodeBlockNode.tsx        -- Syntax-highlighted code blocks
    MathNode.tsx             -- KaTeX rendering
  NoteTags.tsx               -- Tag pills
  NoteSearchBar.tsx          -- Full-text + semantic search
  GraphView.tsx              -- Force-directed wiki-link graph
  NoteVersionHistory.tsx     -- Version list with diff/restore
```

### State Pattern

Uses existing useQuery/useMutation IPC hooks:

```tsx
const { data: notes } = useQuery<Note[]>("note_list", { notebook_id });
const { mutate: createNote } = useMutation("note_create");
const { mutate: updateNote } = useMutation("note_update");
```

Auto-save via debounced editor onChange (1s idle).

### Styling

All components use existing glass design system tokens: glass-panel, glass-input, glass-button, bg-surface-base, text-primary/muted, border-border.

## Editor Features

Ported from HelixNotes TipTap config:

| Feature | Status |
|---------|--------|
| WYSIWYG editing | Port |
| Formatting toolbar | Port |
| Slash commands (/) | Port |
| Code blocks + syntax highlighting | Port (Lowlight) |
| Math (KaTeX) | Port |
| Wiki-links ([[note]]) | Port |
| Tables | Port |
| Task lists (checkboxes) | Port |
| Image paste from clipboard | Port |
| Knowledge graph view | Build fresh |
| Version history | Build fresh |

Cut from HelixNotes:
- Source mode toggle (unnecessary complexity)
- Frontmatter editing (metadata in SQLite, not YAML)
- AI writing tools (use existing agent system instead)

## Key Interactions

### Wiki-Links

`[[` triggers fuzzy note search popup. Selecting inserts WikiLinkNode with target ID. On save, backend parses links and updates note_links table. Clicking navigates to target note.

### Entity Mentions

`@` triggers entity type tabs (Tasks/Projects/Sessions). Selecting inserts colored chip. On save, backend updates note_entity_mentions. Entity detail pages show "Referenced in notes" section.

### Version History

On save, if last version > 5 min old, snapshot to note_versions. Max 50 versions (pruned). UI: side panel with version list, click to diff or restore.

### Cross-Feature Integration

- Task/Project detail pages show linked notes
- Agent context_engine pulls relevant notes via LanceDB vectors
- Notes searchable from global search
