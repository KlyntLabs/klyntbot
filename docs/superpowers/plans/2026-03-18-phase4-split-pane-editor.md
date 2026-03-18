# Phase 4: Split-Pane Editor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three split-pane editor modes (Translation, Annotation, Cornell) to the note editor, each with dual TipTap instances, a resize handle, and atomic save that keeps `body` in sync for FTS5/BookRAG.

**Architecture:** New `split_content` (JSON) and `split_mode` (text) columns on the `notes` table. `SplitEditor` component orchestrates two TipTap editor instances with mode-specific layouts. Each mode component (Translation, Annotation, Cornell) receives editor instances and renders its layout. A single debounced save serializes both panes into `split_content` JSON and concatenates them into `body`/`body_html` for search indexing. Mode toggle lives in `SplitToolbar`, rendered above the editor when in split mode.

**Tech Stack:** Rust (SQLite migration, NoteRow/NoteResponse/NoteUpdateParams updates), React + TipTap (`@tiptap/react` useEditor), Tailwind v4 glass design system, imperative resize (pointermove + RAF pattern from KnowledgeBasePage).

**Key patterns to follow:**
- TipTap editor setup: `useNoteEditor()` in `EditorCore.tsx` — creates editor with 20+ extensions
- Debounced save: `pendingRef` + 1s `setTimeout` → flush via `onSave({ id, body, bodyHtml })` (see `NoteEditor.tsx:96-121`)
- Resize handle: `onPointerDown` → RAF loop → update ref imperatively → `classList.add("resizing")` during drag (see `KnowledgeBasePage.tsx:275-329`)
- Backend update: `note_repo.update_note()` uses `COALESCE` for optional fields (see `feature-notes/src/repo/notes.rs:78-110`)
- CSS: glass-card, glass-toolbar classes. `.resizing` class suppresses backdrop-filter during drag. Never hardcode hex/rgba.

**Depends on:** Phase 1 (FSRS-5) and Phase 2 (/learn page). Independent of Phase 3 (card generation).

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx` | Dual TipTap orchestrator: creates left/right editors, resize handle, debounced atomic save |
| `desktop-ui/src/features/notes/components/editor/SplitToolbar.tsx` | Mode toggle (single/translation/annotation/cornell) + mode-specific action buttons |

### Modified files

| File | Change |
|------|--------|
| `crates/feature-notes/migrations/001_create_notes.sql` | Add `split_content TEXT` and `split_mode TEXT` columns to `notes` table |
| `crates/feature-notes/src/lib.rs` | Bump migration version from 5 to 6 |
| `crates/feature-notes/src/models.rs` | Add `split_content` and `split_mode` to `NoteRow` |
| `crates/feature-notes/src/repo/notes.rs` | Add `split_content` and `split_mode` to `create_note`, `update_note`, SELECT queries |
| `crates/desktop-shared/src/commands/notes.rs` | Add fields to `NoteResponse` and `NoteUpdateParams` |
| `crates/app-core/src/handlers/notes/converters.rs` | Add fields to `note_row_to_response` |
| `crates/app-core/src/handlers/notes/crud.rs` | Thread `split_content` and `split_mode` through `note_update` |
| `desktop-ui/src/shared/types/notes.ts` | Add `splitContent`, `splitMode` to `Note` and `NoteUpdateParams` |
| `desktop-ui/src/features/notes/components/NoteEditor.tsx` | Conditionally render `SplitEditor` when `splitMode` is set |
| `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx` | Thread `onSplitModeChange` prop |
| `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx` | Add split-mode toggle button |
| `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Handle split mode persistence |

---

### Task 1: Backend — schema migration + model + repo + handler updates

**Files:**
- Modify: `crates/feature-notes/migrations/001_create_notes.sql`
- Modify: `crates/feature-notes/src/lib.rs`
- Modify: `crates/feature-notes/src/models.rs`
- Modify: `crates/feature-notes/src/repo/notes.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/app-core/src/handlers/notes/converters.rs`
- Modify: `crates/app-core/src/handlers/notes/crud.rs`

- [ ] **Step 1: Add columns to migration SQL**

In `crates/feature-notes/migrations/001_create_notes.sql`, add these lines at the end of the `notes` CREATE TABLE (before the closing `);`):

```sql
-- In the CREATE TABLE notes statement, add after the `embedding_updated_at` column:
    split_content   TEXT,
    split_mode      TEXT,
```

Since this is pre-release (no user data to migrate), modify the CREATE TABLE directly rather than writing an ALTER TABLE migration.

- [ ] **Step 2: Bump migration version**

In `crates/feature-notes/src/lib.rs`, change:
```rust
version: 5,
```
to:
```rust
version: 6,
```

- [ ] **Step 3: Update NoteRow struct**

In `crates/feature-notes/src/models.rs`, add to the `NoteRow` struct (after `embedding_updated_at`):

```rust
pub split_content: Option<String>,
pub split_mode: Option<String>,
```

- [ ] **Step 4: Update repo — create_note**

In `crates/feature-notes/src/repo/notes.rs`, update `create_note`:

1. Add `split_content` and `split_mode` to the INSERT column list and VALUES placeholders
2. Add `.bind(&row.split_content)` and `.bind(&row.split_mode)` binds

The INSERT should now include 14 columns (was 12).

- [ ] **Step 5: Update repo — update_note**

In `crates/feature-notes/src/repo/notes.rs`, update `update_note` to accept two new optional parameters:

```rust
pub async fn update_note(
    &self,
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
    body_html: Option<&str>,
    pinned: Option<bool>,
    notebook_id: Option<Option<&str>>,
    icon: Option<Option<&str>>,
    color: Option<Option<&str>>,
    split_content: Option<Option<&str>>,  // NEW: None=keep, Some(None)=clear, Some(Some(json))=set
    split_mode: Option<Option<&str>>,     // NEW: None=keep, Some(None)=clear, Some(Some(mode))=set
) -> Result<NoteRow, StorageError>
```

Add to the UPDATE SET clause:
```sql
split_content = CASE
    WHEN ?10 IS NULL THEN split_content
    WHEN ?10 = '' THEN NULL
    ELSE ?10
END,
split_mode = CASE
    WHEN ?11 IS NULL THEN split_mode
    WHEN ?11 = '' THEN NULL
    ELSE ?11
END,
```

Add sentinel conversions and binds:
```rust
let split_content_sentinel = nullable_to_sentinel(split_content);
let split_mode_sentinel = nullable_to_sentinel(split_mode);
// ... .bind(&split_content_sentinel) .bind(&split_mode_sentinel)
```

- [ ] **Step 6: Update repo — SELECT queries with explicit column lists**

Several queries use explicit column lists (not `SELECT *`) and will break after adding new NoteRow fields. Fix each:

1. **`search_notes`** (~line 242): Add `n.split_content, n.split_mode` to the inner CTE SELECT list after `n.updated_at`.
2. **`search_fts`** (~line 347): Add `n.split_content, n.split_mode` to the SELECT list after `n.updated_at`.
3. **`NoteSearchResult`** in `models.rs`: Add `pub split_content: Option<String>` and `pub split_mode: Option<String>` to the struct.
4. **`get_unlinked_mentions`** (~line 320): Uses `n.*` so should be fine, but verify.

Also update the `Note` domain struct in `models.rs` (if it exists separately from `NoteRow`) and its `from_row` conversion to include the new fields.

- [ ] **Step 6b: Update all callers of update_note for new signature**

The `update_note` signature now has 2 extra params. Fix all callers besides `note_update` (which is updated in Step 10):

1. **`crud.rs:note_version_restore`** (~line 344): Add `None, None,` for `split_content`, `split_mode` after the existing `None` args.
2. **`feature-notes/src/tool.rs`** (~line 172): Add `, None, None` after the last `None`.
3. Any test file calling `update_note`: Add `None, None` params.

- [ ] **Step 6c: Update note_create handler**

In `crates/app-core/src/handlers/notes/crud.rs`, find the `NoteRow { ... }` construction inside `note_create` (~line 109). Add:
```rust
split_content: None,
split_mode: None,
```

- [ ] **Step 7: Update NoteResponse in desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add to `NoteResponse`:

```rust
pub split_content: Option<String>,
pub split_mode: Option<String>,
```

- [ ] **Step 8: Update NoteUpdateParams in desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add to `NoteUpdateParams`:

```rust
/// `None` = don't change, `Some(None)` = clear split, `Some(Some(json))` = set split content
#[serde(default, deserialize_with = "deserialize_nullable_field")]
pub split_content: Option<Option<String>>,
/// `None` = don't change, `Some(None)` = clear mode (back to single), `Some(Some(mode))` = set mode
#[serde(default, deserialize_with = "deserialize_nullable_field")]
pub split_mode: Option<Option<String>>,
```

- [ ] **Step 9: Update converters**

In `crates/app-core/src/handlers/notes/converters.rs`, find `note_row_to_response` and add:

```rust
split_content: row.split_content.clone(),
split_mode: row.split_mode.clone(),
```

- [ ] **Step 10: Update crud handler**

In `crates/app-core/src/handlers/notes/crud.rs`, update `note_update` to pass the new fields to `update_note`:

```rust
self.note_repo.update_note(
    &params.id,
    params.title.as_deref(),
    params.body.as_deref(),
    params.body_html.as_deref(),
    params.pinned,
    params.notebook_id.as_ref().map(|o| o.as_deref()),
    params.icon.as_ref().map(|o| o.as_deref()),
    params.color.as_ref().map(|o| o.as_deref()),
    params.split_content.as_ref().map(|o| o.as_deref()),  // NEW
    params.split_mode.as_ref().map(|o| o.as_deref()),     // NEW
)
```

- [ ] **Step 11: Verify compilation and tests**

Run:
```bash
cargo build -p desktop && cargo nextest run -p feature-notes
```
Expected: Compiles and all existing tests pass. The migration version bump to 6 forces re-run on dev databases.

- [ ] **Step 12: Commit**

```bash
git add crates/feature-notes/ crates/desktop-shared/ crates/app-core/src/handlers/notes/
git commit -m "feat(notes): add split_content + split_mode columns for split-pane editor"
```

---

### Task 2: Frontend types

**Files:**
- Modify: `desktop-ui/src/shared/types/notes.ts`

- [ ] **Step 1: Update Note interface**

Add after `tags: string[]`:

```typescript
splitContent: string | null;
splitMode: string | null;
```

- [ ] **Step 2: Update NoteUpdateParams interface**

Add:

```typescript
splitContent?: string | null;
splitMode?: string | null;
```

- [ ] **Step 3: Verify build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/types/notes.ts
git commit -m "feat(notes): add splitContent/splitMode to frontend Note types"
```

---

### Task 3: SplitEditor core — dual TipTap editors with resize handle and atomic save

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx`

This is the core component. It:
1. Creates two TipTap editor instances (left + right) using the same `useNoteEditor` hook from `EditorCore.tsx`
2. Manages a resize handle between the panes (imperative RAF pattern)
3. Owns a single debounced save that serializes both panes into `split_content` JSON and concatenates into `body`/`body_html`
4. Delegates layout to mode-specific child components

- [ ] **Step 1: Create SplitEditor component**

```tsx
import type { Note, NoteUpdateParams } from "@shared/types";
import { useCallback, useEffect, useRef, useState } from "react";
import { EditorContentWrapper, useNoteEditor } from "./EditorCore";

export type SplitMode = "translation" | "annotation" | "cornell";

interface SplitContent {
  left: { html: string; markdown: string };
  right: { html: string; markdown: string };
  summary?: { html: string; markdown: string }; // Cornell only
}

interface SplitEditorProps {
  note: Note;
  splitMode: SplitMode;
  onSave: (params: NoteUpdateParams) => void;
}

function parseSplitContent(note: Note): SplitContent {
  if (note.splitContent) {
    try {
      return JSON.parse(note.splitContent);
    } catch {
      // Fallback: put existing body in left pane
    }
  }
  // Initialize: existing content goes to left pane, right starts empty
  return {
    left: { html: note.bodyHtml || note.body || "", markdown: note.body || "" },
    right: { html: "", markdown: "" },
  };
}

export function SplitEditor({ note, splitMode, onSave }: SplitEditorProps) {
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteIdRef = useRef(note.id);

  // Parse initial split content
  const initialContent = useRef(parseSplitContent(note));

  // Debounced save — same pattern as NoteEditor.tsx
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingLeftRef = useRef<{ html: string; markdown: string } | null>(null);
  const pendingRightRef = useRef<{ html: string; markdown: string } | null>(null);
  const pendingSummaryRef = useRef<{ html: string; markdown: string } | null>(null);

  // Track latest content for save serialization
  const leftContentRef = useRef(initialContent.current.left);
  const rightContentRef = useRef(initialContent.current.right);
  const summaryContentRef = useRef(initialContent.current.summary || { html: "", markdown: "" });

  const flushSave = useCallback(() => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }

    const hasLeftChange = pendingLeftRef.current !== null;
    const hasRightChange = pendingRightRef.current !== null;
    const hasSummaryChange = pendingSummaryRef.current !== null;

    if (!hasLeftChange && !hasRightChange && !hasSummaryChange) return;

    // Update latest refs from pending
    if (pendingLeftRef.current) {
      leftContentRef.current = pendingLeftRef.current;
      pendingLeftRef.current = null;
    }
    if (pendingRightRef.current) {
      rightContentRef.current = pendingRightRef.current;
      pendingRightRef.current = null;
    }
    if (pendingSummaryRef.current) {
      summaryContentRef.current = pendingSummaryRef.current;
      pendingSummaryRef.current = null;
    }

    // Serialize split_content JSON
    const splitContent: SplitContent = {
      left: leftContentRef.current,
      right: rightContentRef.current,
    };
    if (summaryContentRef.current.markdown) {
      splitContent.summary = summaryContentRef.current;
    }

    // Concatenate body for FTS5/BookRAG
    const bodyParts = [leftContentRef.current.markdown, rightContentRef.current.markdown];
    if (summaryContentRef.current.markdown) {
      bodyParts.push(summaryContentRef.current.markdown);
    }
    const body = bodyParts.filter(Boolean).join("\n\n---\n\n");

    const htmlParts = [leftContentRef.current.html, rightContentRef.current.html];
    if (summaryContentRef.current.html) {
      htmlParts.push(summaryContentRef.current.html);
    }
    const bodyHtml = htmlParts.filter(Boolean).join('<hr class="split-divider">');

    onSaveRef.current({
      id: noteIdRef.current,
      body,
      bodyHtml,
      splitContent: JSON.stringify(splitContent),
    });
  }, []);

  const scheduleSave = useCallback(() => {
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(flushSave, 1000);
  }, [flushSave]);

  // Create left editor
  const handleLeftUpdate = useCallback(
    (html: string, markdown: string) => {
      pendingLeftRef.current = { html, markdown };
      scheduleSave();
    },
    [scheduleSave],
  );

  const handleRightUpdate = useCallback(
    (html: string, markdown: string) => {
      pendingRightRef.current = { html, markdown };
      scheduleSave();
    },
    [scheduleSave],
  );

  const leftEditor = useNoteEditor({
    content: initialContent.current.left.html || initialContent.current.left.markdown,
    onUpdate: handleLeftUpdate,
    onNavigateNote: () => {},
    onNavigateEntity: () => {},
  });

  const rightEditor = useNoteEditor({
    content: initialContent.current.right.html || initialContent.current.right.markdown,
    onUpdate: handleRightUpdate,
    onNavigateNote: () => {},
    onNavigateEntity: () => {},
  });

  // Flush on note change
  useEffect(() => {
    if (noteIdRef.current !== note.id) {
      flushSave();
      noteIdRef.current = note.id;
      const newContent = parseSplitContent(note);
      initialContent.current = newContent;
      leftContentRef.current = newContent.left;
      rightContentRef.current = newContent.right;
      summaryContentRef.current = newContent.summary || { html: "", markdown: "" };
      if (leftEditor) leftEditor.commands.setContent(newContent.left.html || newContent.left.markdown);
      if (rightEditor) rightEditor.commands.setContent(newContent.right.html || newContent.right.markdown);
    }
  }, [note.id, leftEditor, rightEditor, flushSave]);

  // Flush on unmount and Cmd+S
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        flushSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => {
      document.removeEventListener("keydown", handler);
      flushSave();
    };
  }, [flushSave]);

  // ── Resize handle ─────────────────────────────────────
  const containerRef = useRef<HTMLDivElement>(null);
  const defaultRatio = splitMode === "annotation" ? 0.67 : 0.5;
  const [splitRatio, setSplitRatio] = useState(defaultRatio);
  const splitRatioRef = useRef(defaultRatio);

  // Update default ratio when mode changes
  useEffect(() => {
    const newDefault = splitMode === "annotation" ? 0.67 : 0.5;
    setSplitRatio(newDefault);
    splitRatioRef.current = newDefault;
  }, [splitMode]);

  const onResizeStart = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startRatio = splitRatioRef.current;
    const containerW = containerRef.current?.offsetWidth || 800;
    let raf = 0;

    containerRef.current?.classList.add("resizing");

    const onMove = (ev: globalThis.PointerEvent) => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const delta = ev.clientX - startX;
        const newRatio = Math.min(0.8, Math.max(0.2, startRatio + delta / containerW));
        splitRatioRef.current = newRatio;
        if (containerRef.current) {
          const left = containerRef.current.querySelector("[data-pane='left']") as HTMLElement;
          const right = containerRef.current.querySelector("[data-pane='right']") as HTMLElement;
          if (left) left.style.width = `${newRatio * 100}%`;
          if (right) right.style.width = `${(1 - newRatio) * 100}%`;
        }
      });
    };

    const onUp = () => {
      cancelAnimationFrame(raf);
      setSplitRatio(splitRatioRef.current);
      containerRef.current?.classList.remove("resizing");
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };

    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }, []);

  // ── Synced scrolling (Translation mode only) ──────────
  const leftPaneRef = useRef<HTMLDivElement>(null);
  const rightPaneRef = useRef<HTMLDivElement>(null);
  const isSyncingRef = useRef(false);

  const handleSyncScroll = useCallback(
    (source: "left" | "right") => {
      if (splitMode !== "translation") return;
      if (isSyncingRef.current) return;
      isSyncingRef.current = true;

      const sourceEl = source === "left" ? leftPaneRef.current : rightPaneRef.current;
      const targetEl = source === "left" ? rightPaneRef.current : leftPaneRef.current;

      if (sourceEl && targetEl) {
        const ratio = sourceEl.scrollTop / (sourceEl.scrollHeight - sourceEl.clientHeight || 1);
        targetEl.scrollTop = ratio * (targetEl.scrollHeight - targetEl.clientHeight);
      }

      requestAnimationFrame(() => {
        isSyncingRef.current = false;
      });
    },
    [splitMode],
  );

  // ── Cornell summary ───────────────────────────────────
  const [summaryText, setSummaryText] = useState(
    initialContent.current.summary?.markdown || "",
  );

  const handleSummaryChange = useCallback(
    (text: string) => {
      setSummaryText(text);
      pendingSummaryRef.current = { html: `<p>${text}</p>`, markdown: text };
      scheduleSave();
    },
    [scheduleSave],
  );

  if (!leftEditor || !rightEditor) return null;

  // ── Mode labels ───────────────────────────────────────
  const leftLabel =
    splitMode === "translation" ? "Source" : splitMode === "cornell" ? "Cues / Questions" : "Content";
  const rightLabel =
    splitMode === "translation"
      ? "Translation"
      : splitMode === "cornell"
        ? "Notes"
        : "Annotations";

  return (
    <div ref={containerRef} className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 flex min-h-0">
        {/* Left pane */}
        <div
          data-pane="left"
          ref={leftPaneRef}
          className="flex flex-col min-h-0 overflow-y-auto"
          style={{ width: `${splitRatio * 100}%` }}
          onScroll={() => handleSyncScroll("left")}
        >
          <div className="px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider border-b border-border shrink-0">
            {leftLabel}
          </div>
          <div className="flex-1 overflow-y-auto">
            <EditorContentWrapper editor={leftEditor} className="flex-1 min-h-0" />
          </div>
        </div>

        {/* Resize handle */}
        <div
          className="w-1 cursor-col-resize bg-border hover:bg-brand/30 transition-colors shrink-0"
          onPointerDown={onResizeStart}
        />

        {/* Right pane */}
        <div
          data-pane="right"
          ref={rightPaneRef}
          className="flex flex-col min-h-0 overflow-y-auto"
          style={{ width: `${(1 - splitRatio) * 100}%` }}
          onScroll={() => handleSyncScroll("right")}
        >
          <div className="px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider border-b border-border shrink-0">
            {rightLabel}
          </div>
          <div className="flex-1 overflow-y-auto">
            <EditorContentWrapper editor={rightEditor} className="flex-1 min-h-0" />
          </div>
        </div>
      </div>

      {/* Cornell summary footer */}
      {splitMode === "cornell" && (
        <div className="border-t border-border">
          <div className="px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider">
            Summary
          </div>
          <textarea
            value={summaryText}
            onChange={(e) => handleSummaryChange(e.target.value)}
            placeholder="Write a brief summary of this note..."
            className="w-full bg-transparent px-3 py-2 text-sm text-foreground placeholder:text-dim resize-none"
            rows={3}
          />
        </div>
      )}
    </div>
  );
}
```

**Implementation notes for the implementer:**

- `useNoteEditor` is imported from `./EditorCore` — it creates a full TipTap editor with all extensions. Verify the import path and that `useNoteEditor` accepts `{ content, onUpdate, onNavigateNote, onNavigateEntity }` params.
- `EditorContentWrapper` renders the TipTap editor content — also from `./EditorCore`.
- The resize handle follows the exact pattern from `KnowledgeBasePage.tsx` (pointermove + RAF + resizing class toggle).
- The `body` concatenation uses `\n\n---\n\n` as separator so markdown renders cleanly as a divider.
- The `body_html` concatenation uses `<hr class="split-divider">` as separator.
- `isSyncingRef` prevents infinite scroll loop (left scrolls right, which would scroll left again).

- [ ] **Step 2: Verify with Biome**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/SplitEditor.tsx
git commit -m "feat(notes): add SplitEditor with dual TipTap editors + resize handle"
```

---

### Task 4: SplitToolbar — mode toggle

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/SplitToolbar.tsx`

- [ ] **Step 1: Create SplitToolbar component**

```tsx
import { BookOpen, Columns, FileText, Languages, StickyNote } from "lucide-react";
import type { SplitMode } from "./SplitEditor";

type EditorMode = "single" | SplitMode;

interface SplitToolbarProps {
  currentMode: EditorMode;
  onModeChange: (mode: EditorMode) => void;
}

const modes: { key: EditorMode; icon: typeof Columns; label: string; shortLabel: string }[] = [
  { key: "single", icon: FileText, label: "Single pane", shortLabel: "Single" },
  { key: "translation", icon: Languages, label: "Translation mode", shortLabel: "Translate" },
  { key: "annotation", icon: StickyNote, label: "Annotation mode", shortLabel: "Annotate" },
  { key: "cornell", icon: BookOpen, label: "Cornell method", shortLabel: "Cornell" },
];

export function SplitToolbar({ currentMode, onModeChange }: SplitToolbarProps) {
  return (
    <div className="flex items-center gap-0.5 px-2 py-1">
      {modes.map((mode) => {
        const Icon = mode.icon;
        const isActive = currentMode === mode.key;
        return (
          <button
            key={mode.key}
            type="button"
            onClick={() => onModeChange(mode.key)}
            title={mode.label}
            className={`flex items-center gap-1 px-2 py-1 rounded-lg text-[11px] transition-all ${
              isActive
                ? "bg-brand/15 text-brand"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            }`}
          >
            <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
            {mode.shortLabel}
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Verify with Biome**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/SplitToolbar.tsx
git commit -m "feat(notes): add SplitToolbar mode toggle component"
```

---

### Task 5: NoteEditor + NoteEditorPanel + KnowledgeBasePage integration

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Add split-mode toggle to EditorToolbar**

In `EditorToolbar.tsx`:

1. Add `Columns` to lucide-react imports.
2. Add `onToggleSplitMode?: () => void` and `splitModeActive?: boolean` to `EditorToolbarProps`.
3. In the `modeButtons` JSX, add a split-mode button **before** the generate cards button:

```tsx
{onToggleSplitMode && (
  <button
    type="button"
    onClick={onToggleSplitMode}
    title="Split-pane editor"
    className={`p-1.5 rounded-lg transition-all ${
      splitModeActive
        ? "bg-brand/15 text-brand"
        : "text-dim hover:text-muted-foreground hover:bg-card"
    }`}
  >
    <Columns className="w-3.5 h-3.5" strokeWidth={1.5} />
  </button>
)}
```

- [ ] **Step 2: Modify NoteEditor to conditionally render SplitEditor**

In `NoteEditor.tsx`:

1. Import `SplitEditor` and `SplitToolbar`:
```tsx
import { SplitEditor, type SplitMode } from "./editor/SplitEditor";
import { SplitToolbar } from "./editor/SplitToolbar";
```

2. Add props: `splitMode?: SplitMode | null` and `onSplitModeChange?: (mode: SplitMode | null) => void` to `NoteEditorProps`.

3. Determine the editor mode from `note.splitMode`:
```tsx
const activeSplitMode = splitMode || (note.splitMode as SplitMode | null);
```

4. Add a toggle handler for the toolbar button:
```tsx
const handleToggleSplitMode = useCallback(() => {
  if (activeSplitMode) {
    onSplitModeChange?.(null);
  } else {
    onSplitModeChange?.("translation"); // Default to translation when first enabling
  }
}, [activeSplitMode, onSplitModeChange]);
```

5. Pass to EditorToolbar:
```tsx
onToggleSplitMode={onSplitModeChange ? handleToggleSplitMode : undefined}
splitModeActive={!!activeSplitMode}
```

6. In the content area (after the gradient separator), conditionally render:
```tsx
{activeSplitMode ? (
  <>
    <SplitToolbar
      currentMode={activeSplitMode}
      onModeChange={(mode) => onSplitModeChange?.(mode === "single" ? null : mode as SplitMode)}
    />
    <SplitEditor note={note} splitMode={activeSplitMode} onSave={onSave} />
  </>
) : (
  <div className="flex-1 overflow-y-auto min-h-0 relative">
    <EditorContentWrapper editor={editor} className={editorContentClass} />
    {/* existing vim command line etc. */}
  </div>
)}
```

**Important:** When in split mode, the single-pane editor (`editor`) should still exist (for the toolbar) but its content area is hidden. The SplitEditor creates its own TipTap instances.

**Restoring main editor when leaving split mode:** Add a `useEffect` that watches `activeSplitMode`. When it transitions from non-null to null (user clicked "Single"), the SplitEditor unmounts (triggering its `flushSave`). After unmount, refresh the main editor content from the note's latest body:

```tsx
const prevSplitModeRef = useRef(activeSplitMode);
useEffect(() => {
  if (prevSplitModeRef.current && !activeSplitMode && editor) {
    // Transitioning from split → single: refresh main editor from note body
    // SplitEditor's unmount flush already saved concatenated body
    const freshContent = note.bodyHtml || note.body || "";
    editor.commands.setContent(freshContent);
  }
  prevSplitModeRef.current = activeSplitMode;
}, [activeSplitMode, editor, note.bodyHtml, note.body]);
```

- [ ] **Step 3: Thread props through NoteEditorPanel**

In `NoteEditorPanel.tsx`, add:
```tsx
splitMode?: SplitMode | null;
onSplitModeChange?: (mode: SplitMode | null) => void;
```

Pass to `<NoteEditor>`.

- [ ] **Step 4: Wire split mode in KnowledgeBasePage**

In `KnowledgeBasePage.tsx`:

1. Import `SplitMode` type.
2. Add handler:
```tsx
const handleSplitModeChange = useCallback(
  (mode: SplitMode | null) => {
    if (!selectedNote) return;
    // Persist to DB
    updateNote({
      id: selectedNote.id,
      splitMode: mode,
    });
  },
  [selectedNote, updateNote],
);
```

`updateNote` is from `useMutation<Note, NoteUpdateParams>("note_update", "params")` — already declared in KnowledgeBasePage (line ~134).

3. Pass to NoteEditorPanel:
```tsx
splitMode={selectedNote?.splitMode as SplitMode | null}
onSplitModeChange={handleSplitModeChange}
```

- [ ] **Step 5: Verify build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(notes): integrate split-pane editor with mode toggle and persistence"
```

---

## Testing Checklist

After all tasks are complete, verify the full flow:

- [ ] **Schema migration**: Delete dev database (`~/.klyntbot-dev/data.db`), restart server. Verify `notes` table has `split_content` and `split_mode` columns.
- [ ] **Single pane (default)**: Open a note. Editor works as before. No split toolbar visible. Columns icon in toolbar is not highlighted.
- [ ] **Enable split mode**: Click Columns icon in toolbar. SplitToolbar appears with Translation mode active. Two editor panes shown side-by-side with labels "Source" / "Translation".
- [ ] **Resize handle**: Drag the divider between panes. Panes resize smoothly (no jank). Glass-filter suppressed during drag.
- [ ] **Type in both panes**: Type content in left pane and right pane. Both save correctly (check via page reload).
- [ ] **Synced scrolling (Translation)**: Fill both panes with content. Scroll one — the other scrolls proportionally.
- [ ] **Switch to Annotation mode**: Click "Annotate" in SplitToolbar. Left pane takes 2/3, right takes 1/3. Labels change to "Content" / "Annotations". No synced scrolling.
- [ ] **Switch to Cornell mode**: Click "Cornell". Left/right panes + summary textarea at bottom. Labels: "Cues / Questions" / "Notes".
- [ ] **Summary textarea (Cornell)**: Type in the summary area. Content saves with note.
- [ ] **Switch back to Single**: Click "Single" in SplitToolbar. Returns to normal single-pane editor. Content preserved (left+right concatenated in body).
- [ ] **Mode persistence**: Switch to Translation mode, reload page. Note reopens in Translation mode.
- [ ] **Body stays in sync**: After editing in split mode, search for content in the note search. FTS5 should find text from both panes.
- [ ] **Cmd+S**: Force save in split mode works.
- [ ] **Note switching**: Select a different note while in split mode. Content flushes and new note loads correctly.

---

## Architecture Decisions

1. **Single SplitEditor component with mode-specific layout** — rather than separate TranslationMode/AnnotationMode/CornellMode components (as the spec suggested), the differences are small enough (label names, default ratio, synced scroll flag, summary textarea) to handle with conditionals in one component. This avoids duplicating editor creation and resize logic across 3 files. If modes become more complex in later phases (AI actions), they can be extracted then.

2. **`body` stays as concatenated text** — FTS5, BookRAG, and all search features continue to work without modification. The `split_content` JSON is the source of truth for the editor; `body`/`body_html` are derived snapshots.

3. **Tri-state nullable pattern for split_mode** — `None` in DB = never split, matches the `Option<Option<&str>>` sentinel pattern already used for `notebook_id`, `icon`, `color`. Frontend sends `splitMode: null` to clear (back to single), `splitMode: "translation"` to set.

4. **Two TipTap instances per split** — each pane has independent undo/redo history, cursor position, and extension state. This matches the spec requirement and avoids complex state synchronization.

5. **Cornell summary is a plain textarea** — not a full TipTap editor. Summaries are short, plain text. Adding a third TipTap instance would be over-engineering.

6. **Synced scrolling uses scroll ratio** — `scrollTop / (scrollHeight - clientHeight)` ratio mapping. Not paragraph-precise, but smooth and sufficient for aligned content. Only active in Translation mode.
