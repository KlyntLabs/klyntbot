# Knowledge Base Redesign — Design Spec

> Redesign the notes feature from basic note-taking into a complete knowledge management system with graph-first navigation, AI-powered suggestions, hybrid search, and deep cross-domain entity integration.

## Context

The current `feature-notes` crate and `desktop-ui/src/features/notes/` implement a functional note editor with TipTap, wiki-links, entity mentions, version history, and a basic D3-force graph. This redesign evolves it into a graph-first knowledge management system that leverages Klyntbot's unique strengths: the AI agent pipeline, cognitive memory, LanceDB embeddings, and deep entity integration across tasks, projects, OKRs, and finance.

**Pre-release status:** Breaking changes are acceptable. No migration compatibility required.

## Design Decisions

### Paradigm: Graph-First

Notes are the atomic unit. Links between notes are the primary organizational structure. Notebooks (folders) exist as optional lightweight containers but are de-emphasized. The sidebar, search, and AI suggestions all prioritize connections over hierarchy.

### AI Role: Suggestive + Collaborative

The AI agent is both suggestive (proactive recommendations while writing) and collaborative (can create notes, synthesize content, answer questions using the knowledge base as context). AI suggestions are the #1 feature in the context panel.

### Writing/Organizing Balance

Both the editor experience and the knowledge exploration experience are first-class. The hybrid layout adapts to whichever mode the user is in.

---

## 1. Layout Architecture

### Three Modes

**Three-Panel (default):** Left navigation sidebar | Center editor | Right context panel. All three panels visible. This is the everyday working mode.

**Focus Mode (Cmd+Shift+Enter):** Both side panels collapse. Editor fills the screen, centered with comfortable max-width. For distraction-free long-form writing. Pressing the hotkey again restores three-panel.

**Graph Mode (Cmd+Shift+G):** Full-screen graph visualization replaces the editor area. The right panel becomes a note preview panel (click a node to preview). Left sidebar remains for search and filtering. Pressing the hotkey again returns to editor mode.

### Panel Proportions

- Left sidebar: 220px default, resizable [180, 320], collapsible
- Center editor: flex-1 (fills remaining space)
- Right context panel: 260px default, resizable [200, 360], collapsible
- Focus mode editor: max-width 720px, centered

---

## 2. Left Navigation Sidebar

Top-to-bottom section order:

### 2.1 Search Bar (always visible, top)

Hybrid FTS5 + semantic search (see Section 6). `Cmd+F` focuses the search bar from anywhere. Results display inline in the sidebar, replacing the sections below temporarily. Results grouped: "Exact matches" (FTS5) and "Related" (semantic), with unified relevance scoring.

### 2.2 Quick Access

- **Pinned notes** — user-pinned notes, sorted by pin order
- **Recent notes** — last 8 notes opened, sorted by recency
- **Today's note** — auto-created daily note (if the user opts in via config key `notes.dailyNote.enabled`, default `false`). Created on first app launch of the day. Title format: `YYYY-MM-DD` (e.g., `2026-03-16`). Empty body. Tagged `#daily`.

### 2.3 Tags Explorer

Tag cloud with pill-style chips. Click a tag to filter all notes by that tag (shows filtered list inline). Supports multi-tag filtering (AND logic). Click a tag while holding Cmd to add it to the filter rather than replacing.

**Tag-to-color mapping:** Deterministic hash of the tag name to an index into a fixed 12-color palette. This mapping is shared across the tags explorer, graph minimap, graph mode node colors, and tag pills throughout the UI. Defined once in a shared utility (`tagColor(tagName: string) -> string`).

### 2.4 Notebooks (collapsible, de-emphasized)

Hierarchical folder tree, collapsed by default. Existing drag-and-drop support retained. Notes can exist without a notebook. A note's notebook is shown in the editor header metadata but is not the primary navigation path.

### 2.5 Footer

Note count, notebook count. "Inbox (N)" badge if quick capture items exist.

---

## 3. Center Editor

### 3.1 Note Header

- **Editable title** — inline contentEditable, not a separate input. Click to edit. Large font, prominent.
- **Inline tags** — tag pills below title. Click `+` to add. Click a tag to remove. AI-suggested tags appear as ghost pills with a `+` button.
- **Metadata line** — notebook location (clickable breadcrumb), created date, word count. Subtle, below tags.

### 3.2 Editor (TipTap)

Retain all existing extensions. Additions:

- **Wiki-link creation from non-existent links:** Typing `[[New Title]]` where no matching note exists shows the autocomplete with a "Create 'New Title'" option at the top. Selecting it creates the note and inserts the link in one action.
- **Improved link/image insertion:** Replace `window.prompt()` with custom modal dialogs (URL input + preview).
- **Slash command enhancements:** Add `/ai` commands: `/ai summarize`, `/ai expand`, `/ai link-suggestions`. These invoke the agent pipeline inline.

### 3.3 Toolbar

Existing formatting buttons retained. Additions:
- Focus mode toggle button (⛶ icon)
- Graph mode toggle button (🕸 icon)
- Version history button (🕐 icon, opens Cmd+Shift+H overlay)
- Vim mode toggle (existing)

### 3.4 Status Bar

Vim mode indicator (existing), word count, save status ("Saved 2s ago"), keyboard shortcut hint for current context.

### 3.5 Auto-Save

Retain 1-second debounce. On save, trigger incremental AI suggestion update (see Section 7).

---

## 4. Right Context Panel

Four core sections, always in this order. Each section is collapsible. Default state: all expanded.

### 4.1 AI Suggestions (top, expanded, purple accent)

**Related Notes:** Top 5 semantically similar notes, computed on note open and updated incrementally on save. Powered by LanceDB embedding similarity. Each entry is clickable (opens note) with a small relevance indicator.

**Link Suggestions:** "Consider linking to X" — notes that share graph neighbors, entity co-occurrences, or tag overlap but aren't directly linked. Shows the reasoning ("shares 3 tags", "both mention @task:migrate-to-fts5"). One-click to insert the wiki-link.

**Suggested Tags:** Tags that appear on related notes but not on the current note. Ghost pills with `+` to accept.

**Action Buttons:**
- **Synthesize** — asks the AI agent to create a synthesis note from the current note and its related notes
- **Ask AI** — opens an inline chat input to ask questions using this note + related notes as context
- **Create linked note** — creates a new note pre-linked to the current one, with AI-suggested title based on content gaps

### 4.2 Backlinks (expanded)

Notes that link TO the current note via wiki-links. Sorted by relevance (semantic similarity + recency hybrid score). Each entry shows: note title, a 1-line context snippet (the sentence containing the wiki-link), and the date. Clicking opens the note.

**Unlinked mentions:** Below backlinks, show notes that mention this note's title in their body but don't have an explicit wiki-link. One-click to convert to a link. **Constraints:** Only computed for notes with titles of 3+ words or 8+ characters (short titles like "Work" produce too many false positives). Uses FTS5 phrase matching (`"exact title"`) rather than LIKE. Results are filtered to exclude notes that already contain `[[Title]]` in their body.

### 4.3 Entity References

Cross-domain entities that mention or are mentioned by this note. Grouped by type:

- **Tasks** — title, status pill (todo/in-progress/done), due date
- **Projects** — title, status, task count
- **Areas** — title
- **OKRs** — objective title, progress percentage

Each entry is a clickable card that navigates to the entity's feature page. One-click "mention in note" button to insert an entity mention at cursor.

### 4.4 Graph Minimap

Small interactive D3-force visualization showing the current note's 1-2 hop neighborhood. Current note is centered and highlighted. Nodes colored by tag (matching the tag explorer colors). Clickable nodes open the note. "Expand" link switches to full Graph Mode focused on this note.

### 4.5 Secondary Sections (collapsed by default, in "More" accordion)

- **Table of Contents** — auto-generated from headings. Click to scroll. Updates live as you type.
- **Note Metadata** — created/updated timestamps, character count, version count, notebook path.

---

## 5. Graph Mode

Full-screen graph visualization replacing the editor area. Left sidebar remains. Right panel becomes a **read-only note preview pane**: when a node is clicked, the panel shows the note's rendered body (TipTap read-only mode) with a compact header (title, tags, word count, updated date). The four context panel sections (AI Suggestions, Backlinks, etc.) are hidden in Graph Mode. An "Open in editor" button at the top of the preview returns to three-panel mode with that note loaded.

### 5.1 Smart Views

Toolbar at the top of the graph area with view presets:

- **Local** (default) — selected note's neighborhood, 1-3 hop radius (configurable slider)
- **Full** — all notes in the system
- **By Tag** — cluster nodes by tag, with tag labels on clusters
- **By Notebook** — cluster nodes by notebook, unfiled notes in a separate cluster
- **Orphans** — only notes with zero links (useful for cleanup/triage)

### 5.2 Visual Encoding

- **Node size:** proportional to link count (inbound + outbound)
- **Node color:** by primary tag (first tag), using the same color palette as the tag explorer
- **Edge thickness:** by co-reference frequency (how many shared mentions/links)
- **Active node:** glow effect + larger radius, with label always visible
- **Hover:** all other nodes except the hovered node and its neighbors dim to 20% opacity

### 5.3 Interactions

- **Hover** — tooltip showing note title + first 2 lines + tag pills + link count
- **Click** — opens note preview in the right panel (does NOT navigate away from graph)
- **Double-click** — switches to editor mode for that note
- **Right-click** — context menu: "Open in editor", "Link to current note", "Show neighborhood", "Delete"
- **Drag** — move nodes, temporarily pins them
- **Scroll** — zoom (15% to 400%, cursor-anchored)
- **Background drag** — pan

### 5.4 Graph Search

Search bar overlaid on the graph. Typing highlights matching nodes and dims non-matches. Supports tag filters (`#rust`), notebook filters (`in:Research`), and free text.

---

## 6. Hybrid Search (FTS5 + Semantic)

### 6.1 FTS5 Layer

New SQLite FTS5 virtual table `notes_fts` indexing `title` and `body` (markdown). Kept in sync via triggers on INSERT/UPDATE/DELETE to the `notes` table. Provides: stemming (porter tokenizer), phrase matching (`"exact phrase"`), boolean operators (AND/OR/NOT), BM25 ranking.

### 6.2 Semantic Layer

Notes indexed in LanceDB with embeddings computed on save. Reuses the existing cognitive memory embedding infrastructure (same model, same LanceDB instance, separate table `note_embeddings`). Provides: conceptual/meaning-based search, similarity scoring.

### 6.3 Unified Results

Search queries run against both layers in parallel. Results are merged with weighted scoring:
- FTS5 BM25 score (min-max normalized within the result batch, so top FTS5 result = 1.0) × 0.6
- Semantic similarity score (cosine similarity, already 0.0–1.0) × 0.4
- Bonus: +0.1 for pinned notes, +0.05 for notes updated in last 7 days

Results grouped in the UI: "Best matches" (top merged results), with a toggle to see "Text matches" and "Related" separately.

### 6.4 Search UX

- Sidebar search bar with 200ms debounce (existing)
- Results replace sidebar sections while active
- Each result shows: title, tag pills, 1-line context snippet with highlighted match, relevance indicator
- `Escape` clears search and restores sidebar
- Empty state suggests: recent searches, trending tags, orphan notes

---

## 7. AI Suggestions Pipeline

### 7.1 Computation Triggers

**On note open:** Full computation of all four signals. Results cached in memory keyed by `(note_id, content_hash)`.

**On save (incremental):** After each auto-save (1-second debounce), recompute only if content hash changed. Two phases:
1. **Synchronous (<300ms):** Re-query SQL-based signals (graph topology, entity co-occurrence, tag overlap) and re-query existing embeddings in LanceDB. This updates the panel immediately.
2. **Asynchronous (fire-and-forget):** Recompute the note's embedding via `TextEmbedder` and upsert into LanceDB. When complete, re-query top-k similar notes and push updated results to the panel. Semantic similarity may be briefly stale (using the previous embedding) until the async update finishes. This matches how `SemanticFactEmbedder` already works in the cognitive system.

### 7.2 Four Signals

1. **Embedding similarity** (weight: 0.4) — LanceDB vector search, top 10 nearest neighbors by cosine similarity to the current note's embedding.

2. **Graph topology** (weight: 0.25) — notes that share 2+ graph neighbors with the current note but aren't directly linked (structural holes). Computed via SQL: `SELECT target_id, COUNT(*) as shared FROM note_links WHERE source_id IN (SELECT target_id FROM note_links WHERE source_id = ?) AND target_id != ? GROUP BY target_id ORDER BY shared DESC LIMIT 10`.

3. **Entity co-occurrence** (weight: 0.2) — notes that mention the same tasks/projects/areas. Computed via `note_entity_mentions` join.

4. **Tag overlap** (weight: 0.15) — notes sharing tags but not linked. Computed via `note_tags` join.

### 7.3 Merged Output

Signals are combined into a unified score per candidate note. Top 5 become "Related Notes." Notes that score high on topology or entity co-occurrence but aren't linked become "Link Suggestions" with explanatory text.

### 7.4 Action Delegation

"Synthesize", "Ask AI", and "Create linked note" buttons invoke the existing agent pipeline. The current note's content + related notes are included in the agent's context. No new AI infrastructure needed — this delegates to `AgentRuntime` via existing handlers.

---

## 8. Note Creation & Quick Capture

### 8.1 AI-Assisted Creation (Cmd+N)

Modal dialog with:
1. Title input (auto-focus)
2. After typing 3+ characters, AI suggestions appear below (200ms debounce):
   - Similar existing notes ("Did you mean...?" — prevents duplicates)
   - Suggested tags (from title semantic analysis)
   - Suggested notebook
   - Potential links ("Link to...")
3. "Create" button applies selected suggestions. "Create blank" skips all suggestions.
4. Note opens in editor immediately after creation.

### 8.2 Quick Capture (Global Hotkey)

System-wide hotkey (configurable, default: Cmd+Shift+C) opens a small floating capture window. **Platform note:** Uses Tauri's `tauri-plugin-global-shortcut` API. On macOS, requires Accessibility permissions (the app should prompt on first use and guide the user to System Settings > Privacy > Accessibility if not granted).
1. Single text area, no formatting
2. `Cmd+Enter` to capture, `Escape` to dismiss
3. Captured items land in an "Inbox" section (new `inbox_items` table, separate from notes)
4. Inbox badge appears in sidebar footer

**Inbox triage:** Clicking an inbox item opens a triage UI:
- "Create as note" — converts to a full note (optionally with AI-assisted dialog)
- "Append to existing note" — search for a note, append as a new paragraph
- "Discard" — delete the inbox item
- AI can suggest which action is most appropriate based on content

### 8.3 Wiki-Link Creation

In the editor, typing `[[` triggers the autocomplete. If no matching note exists:
- First result shows "Create 'Title'" with a ✨ icon
- Selecting it creates the note with that title and inserts the wiki-link
- The new note is pre-linked back to the current note (bidirectional)
- New note inherits the current note's notebook (if any)

### 8.4 Blank Note (Cmd+Shift+N)

Power user shortcut. Creates a blank untitled note immediately, no dialog. Opens in editor. Equivalent to current Cmd+N behavior.

---

## 9. Version History

Dedicated overlay panel triggered by Cmd+Shift+H or the toolbar history button. Not part of the right context panel.

### 9.1 Timeline View

Vertical timeline of version snapshots. Each entry shows:
- Timestamp (relative: "2 hours ago", absolute on hover)
- Word count delta (+47 words)
- Diff summary computed client-side on demand using the `diff` npm package (`diffWords` for word-level changes). Only computed when the version entry is visible (intersection observer) to avoid processing all versions upfront.

### 9.2 Preview

Clicking a version shows a rendered preview (TipTap read-only mode, not raw markdown). Inline diff view available (current vs. selected version) with additions highlighted in green and deletions in red, computed on the markdown `body` field using the `diff` npm package (`diffLines`). Full side-by-side is out of scope — inline diff is sufficient and simpler.

### 9.3 Restore

"Restore this version" creates a safety snapshot of the current state first (existing behavior), then applies the selected version's content. Undo is possible by restoring the safety snapshot.

### 9.4 Configuration

`maxVersionsPerNote` and `versionCooldownMinutes` read from config at runtime (fix current hardcoding). Defaults: 50 versions, 5-minute cooldown.

---

## 10. Backend Changes

### 10.1 Schema Changes

**New tables:**
- `notes_fts` — FTS5 virtual table on `title` + `body`
- `inbox_items` — `id TEXT PK`, `content TEXT`, `status TEXT NOT NULL DEFAULT 'pending'`, `created_at TEXT`. Status values: `pending`, `triaged`.

**Modified tables:**
- `notes` — add `embedding_updated_at TEXT` column for tracking stale embeddings

**New triggers:**
- `notes_fts_insert` / `notes_fts_update` / `notes_fts_delete` — keep FTS5 in sync with notes table

**LanceDB:**
- New `note_embeddings` table with `note_id`, `embedding`, `content_hash`, `updated_at`

### 10.2 Repository Changes

**NoteRepo additions:**
- `search_fts(query) -> Vec<(NoteRow, f32)>` — FTS5 search with BM25 ranking
- `search_hybrid(query, semantic_results) -> Vec<(NoteRow, f32)>` — merge FTS5 + semantic results
- `get_backlinks(note_id) -> Vec<NoteRow>` — notes linking TO this note (currently only `get_links_to` returns IDs)
- `get_unlinked_mentions(note_id, title) -> Vec<NoteRow>` — notes containing the title text but without a wiki-link
- `find_structural_holes(note_id) -> Vec<(String, i64)>` — graph topology signal
- `find_entity_cooccurrences(note_id) -> Vec<(String, i64)>` — entity co-occurrence signal
- `find_tag_overlaps(note_id) -> Vec<(String, i64)>` — tag overlap signal
- `create_inbox_item(content) -> InboxItem`
- `list_inbox_items() -> Vec<InboxItem>`
- `delete_inbox_item(id)`

**NoteRepo changes:**
- `search_notes` — replace LIKE with FTS5 query
- `list_notes` — add pagination (`offset`, `limit` parameters)
- `list_notes` — add optional `tag` filter parameter

### 10.3 Embedding Service

New `NoteEmbeddingService` in `app-core` (NOT in `feature-notes` — must live at L7 to access both `feature-notes` at L4 and the embedding infrastructure):
- `embed_note(note_id, content)` — compute embedding, upsert into LanceDB
- `find_similar(note_id, top_k) -> Vec<(String, f32)>` — vector similarity search
- Reuses the embedding model from `crates/cognitive/src/embedder.rs` (`TextEmbedder` trait) and `crates/tools/src/embedding/embedding_engine.rs` (`EmbeddingEngine`)
- Injected into the note update handler via `Arc<dyn TextEmbedder>` (dependency inversion pattern, consistent with existing `SpawnHandler`/`CronHandler` traits)
- Called from `AppCore::note_update` handler when body changes

### 10.4 AI Suggestions Handler

New `NoteSuggestionsService` in `app-core`:
- `compute_suggestions(note_id) -> NoteSuggestions` — orchestrates all 4 signals, returns merged results
- `NoteSuggestions` struct: `related_notes`, `link_suggestions`, `suggested_tags`, with scores and explanations
- Exposed via new Tauri command `note_suggestions`

### 10.5 Tool Updates

`NotesTool` additions (bringing total to ~20 actions — consider migrating to `#[derive(Tool)]` + `#[tool_actions]` + `#[derive(ActionParams)]` pattern for cleaner dispatch):
- `archive_note` / `unarchive_note` / `list_archived`
- `get_backlinks` — notes linking to a given note
- `search` — upgraded to hybrid search
- `get_suggestions` — AI suggestions for a note
- `capture_inbox` / `list_inbox` / `triage_inbox`
- Notebook hierarchy management (`update_notebook` with `parent_id`)

### 10.6 Tauri Command Additions

- `note_suggestions` — fetch AI suggestions for a note
- `note_backlinks` — get backlinks for a note
- `note_unlinked_mentions` — get unlinked mentions
- `note_archive` / `note_unarchive` / `note_list_archived`
- `inbox_create` / `inbox_list` / `inbox_delete`

---

## 11. Frontend Changes

### 11.1 Remove

- `NotesView.tsx` — dead component (duplicate of NotesPage)
- `NoteList.tsx` / `NoteCard.tsx` — unused in current layout (tag filtering moves to sidebar)
- `WorkspaceFileTree.tsx` / `AgentFileTree.tsx` / `AgentFrontmatterForm.tsx` — moved out of notes feature (future Agent Studio)
- All workspace/agent IPC calls from notes feature

### 11.2 Restructure

`NotesPage.tsx` — refactored from monolithic state holder into:
- `KnowledgeBasePage.tsx` — top-level layout manager, handles mode switching (three-panel / focus / graph)
- `NavigationSidebar.tsx` — left panel with search, quick access, tags, notebooks
- `NoteEditorPanel.tsx` — center panel, delegates to `NoteEditor`
- `ContextPanel.tsx` — right panel with AI suggestions, backlinks, entity refs, graph minimap
- `GraphView.tsx` — refactored for smart views, hover previews, click-to-preview

### 11.3 New Components

- `AISuggestionsPanel.tsx` — fetches and displays AI suggestions, handles action buttons
- `BacklinksPanel.tsx` — backlinks + unlinked mentions
- `EntityReferencesPanel.tsx` — cross-domain entity cards
- `GraphMinimap.tsx` — small neighborhood graph for the context panel
- `TagsExplorer.tsx` — tag cloud with click-to-filter
- `QuickAccessList.tsx` — pinned + recent notes
- `NoteCreationDialog.tsx` — AI-assisted creation modal
- `QuickCaptureWindow.tsx` — global capture overlay
- `InboxTriage.tsx` — inbox item review UI
- `VersionHistoryOverlay.tsx` — redesigned version history with rendered preview and diff
- `GraphToolbar.tsx` — smart view selector, search, filter controls
- `GraphNodePreview.tsx` — hover tooltip for graph nodes

### 11.4 State Management

No global store. Continue with component state + `useQuery`/`useMutation`/`useEvent` hooks. Add:
- `useNoteSuggestions(noteId)` — fetches AI suggestions, refetches on save
- `useBacklinks(noteId)` — fetches backlinks + unlinked mentions
- `useGraphData(view, filters)` — fetches graph data for the selected smart view
- `useInbox()` — inbox items with mutation helpers

---

## 12. Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+N` | AI-assisted note creation dialog |
| `Cmd+Shift+N` | Create blank note (no dialog) |
| `Cmd+Shift+C` | Quick capture (global hotkey, works system-wide) |
| `Cmd+Shift+Enter` | Toggle focus mode |
| `Cmd+F` | Focus search bar |
| `Cmd+Shift+G` | Toggle graph mode |
| `Cmd+Shift+H` | Toggle version history overlay |
| `Cmd+S` | Force save |
| `Cmd+Backspace` | Delete selected note |
| `Cmd+K` | Insert link (custom dialog) |

---

## 13. Breaking Changes

All acceptable pre-release:

1. **Schema:** Consolidated migration replacing `001_create_notes.sql`. Adds FTS5, inbox, embedding tracking.
2. **Removed components:** `NotesView`, `NoteList`, `NoteCard`, workspace/agent trees.
3. **Renamed route:** `/notes` stays, but the page component changes from `NotesPage` to `KnowledgeBasePage`.
4. **Sidebar restructure:** File tree replaced by search + quick access + tags + collapsible notebooks.
5. **Config values:** `maxVersionsPerNote` and `versionCooldownMinutes` now read from config (previously hardcoded).

---

## 14. Out of Scope (v2+)

- Daily notes / journal mode (opt-in daily auto-creation is included, but a dedicated journal view is v2)
- Saved/smart filters ("unlinked notes created this week")
- Note templates
- Collaborative editing (multi-user)
- Export (PDF, markdown archive)
- Agent Studio (workspace/agent file editing — separate feature)
- Mobile/web client
