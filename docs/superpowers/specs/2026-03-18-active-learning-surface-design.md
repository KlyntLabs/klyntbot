# Active Learning Surface — Design Spec

**Date:** 2026-03-18
**Status:** Approved
**Approach:** Smart Hybrid (Approach 3)
**Phase 1 Hero Workflow:** Active Reading & Annotation (Workflow C)

## Overview

Transform Klyntbot's note editor from a static writing canvas into a dynamic, AI-powered learning surface. Users can annotate text with cognitive memory connections, apply per-section "perspectives" that transform how they see content, and interact with AI through a floating toolbar and context menu — all without modifying the underlying document.

The system builds on the existing TipTap 3 editor, split-pane layout, FSRS-5 flashcard system, and cognitive memory infrastructure. No new crates or tables are needed — only column additions to existing tables and new frontend components.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Interaction model | Floating toolbar (selection) + right-click context menu (power) | Toolbar is discoverable and fast; context menu has depth for advanced workflows. Both share the same action handlers. |
| Annotation anchoring | Hybrid: TipTap marks + external storage | Lightweight mark (just `annotationId`) in document survives edits via ProseMirror transforms. All content/metadata in existing `annotations` table. 4-tier fallback: mark → offsets → quoted text → orphan list. |
| Perspectives | Per-section view transforms, not document mutations | Document JSON stays immutable. Perspectives are ephemeral rendering layers. AI results cached client-side; important ones promoted to annotations or cognitive facts. |
| Phase 1 scope | Active Reading & Annotation (Workflow C) | Showcases Linked View (cognitive memory), annotations, and flashcard generation. Sets foundation for Translation (A) and Deep Learning (B) in Phase 2. |
| Perspective storage | JSON column on notes table | `perspective_config` stores section→perspective mappings keyed by TipTap UniqueID. No new table needed in Phase 1. |
| AI result lifecycle | Ephemeral → persistent promotion | AI results are cached client-side (React Query, 10min staleTime). Users can explicitly promote valuable results to annotations or cognitive facts. |

## Section 1: Annotation System Architecture

### Data Flow

1. **User selects text** — TipTap selection → `coordsAtPos()` → position
2. **Generate annotationId (ULID)** — Frontend generates ULID, passed to both TipTap mark and backend INSERT in the same transaction
3. **Floating toolbar / Right-click** — "Add Annotation" action triggered
4. **AI pre-fills suggestion** — `MemoryRetriever` queried with selected text → returns related facts + confidence score (0.0–1.0). Latency target: <500ms.
5. **User confirms / edits / adds thought** — Popover with AI suggestion + free-form input
6. **Dual storage:**
   - **6a: TipTap Mark** — `AnnotationMark { annotationId: "ulid" }` applied to selected range
   - **6b: annotations table** — `INSERT { id, target_type: "note", target_id, mark_id, content, tags, quoted_text, range_start, range_end, ai_suggestion }`

### Schema Changes

```sql
-- annotations table (existing, in cognitive crate)
ALTER TABLE annotations ADD COLUMN mark_id TEXT;        -- links to TipTap mark
ALTER TABLE annotations ADD COLUMN quoted_text TEXT;    -- fallback text anchor
ALTER TABLE annotations ADD COLUMN range_start INTEGER; -- char offset start
ALTER TABLE annotations ADD COLUMN range_end INTEGER;   -- char offset end
ALTER TABLE annotations ADD COLUMN ai_suggestion TEXT;  -- original AI hint

-- notes table (existing, in feature-notes crate)
ALTER TABLE notes ADD COLUMN perspective_config TEXT;    -- JSON
```

### TipTap Mark

```
type: "annotation"
attrs: { annotationId: "ulid" }
```

Lightweight pin — all data lives in the annotations table.

### Fallback Re-Anchor Strategy

When loading a note, annotations are re-anchored using a 4-tier strategy:

1. **Primary:** `mark_id` found in document marks
2. **Fallback:** `range_start` / `range_end` character offsets
3. **Last resort:** Fuzzy match on `quoted_text`
4. **Orphan:** Show in "unanchored annotations" list in the Annotated perspective

### Annotation Popover

Appears when user clicks an annotation highlight. Glassmorphic (`glass-panel`) design with:

- **Header:** Timestamp + tag badges
- **Quoted text:** The highlighted passage with orange left border
- **User annotation:** Free-form comment
- **Cognitive Memory block:** AI insight with confidence bar (0.0–1.0) and "Cognitive Memory" label. Shows related semantic facts, flashcard connections, and source notes.
- **Action buttons:**
  - ⚡ Create Flashcard — sends `quoted_text + annotation.content` to existing `flashcard_generate` pipeline with `source_context` pointing to the annotation
  - 🔗 Link to Fact — creates `entity_link` between annotation and a semantic fact / episodic memory
  - ✦ Open in Insights — opens Insight Review Panel with this annotation pre-loaded
  - ✏️ Edit — inline edit of annotation content
  - 🗑 Delete — removes annotation row + TipTap mark

### Key Design Decisions

- **Mark resilience:** When text under a mark is deleted, the mark disappears from the document. The annotation row persists in the DB with `quoted_text` and `range_start`/`range_end` as fallback anchors for re-attachment or orphan cleanup.
- **AI suggestion trigger:** On "Add Annotation", the selected text is sent to `MemoryRetriever` for a fast semantic search. Result pre-fills the popover's AI section. User can dismiss or keep.
- **Annotation ↔ Cognitive link:** "Link to Fact" creates an `entity_link` row between the annotation and a semantic fact / episodic memory. This feeds the Linked View perspective and strengthens retrieval.
- **Flashcard from annotation:** "Create Flashcard" sends `quoted_text + annotation.content` to the existing `flashcard_generate` pipeline with `source_context` pointing back to the annotation. One-tap flow reusing existing infrastructure.

## Section 2: Floating Toolbar + Context Menu

### A. Floating Toolbar (BubbleMenu)

Auto-appears on any text selection. Uses TipTap's built-in `BubbleMenu` extension with `shouldShow` callback (only when selection is non-empty and not inside a code block). 200ms debounce prevents flicker on click-drag.

**Layout:** Format group (B/I/H) | divider | AI group (Annotate, Flashcard, Translate, Ask AI)

**Actions:**

| Action | Icon | Behavior |
|--------|------|----------|
| **Annotate** | 📝 | Opens annotation popover. AI pre-fill runs in parallel. Mark applied immediately with pending state. |
| **Flashcard** | ⚡ | Sends selection to `flashcard_generate`. Opens existing `CardGenerationModal` with preview. |
| **Translate** | 🌐 | Activates Translation split-pane with selection pre-loaded. Auto-detects source language. |
| **Ask AI** | ✦ | Opens inline prompt input below toolbar. Response streams into split-pane right side or Insight Review Panel. |

### B. Right-Click Context Menu

Radix `ContextMenu` component wrapping the editor area. `glass-panel` styled. `preventDefault()` on contextmenu event within editor bounds. Context-aware: "Selection" group only shows when text is selected.

**Groups:**

**Selection** (conditional — only when text is selected):
- 📝 Add Annotation — `⌥A`
- ⚡ Create Flashcard — `⌥F`
- 🌐 Translate Selection
- 💡 Explain / Define

**AI Actions:**
- ✦ Ask AI about this...
- 📚 Generate Study Pack — submenu (▸)
- 🧠 Show Linked Memory

**Perspectives:**
- 🔍 **Linked View** — `⌥L` (promoted, hero feature)
- 👓 Apply Perspective… — submenu (▸)

**Utility:**
- 📎 Send to Task / Project — submenu (▸)
- 📸 Version Snapshot

### C. Interaction Model

- **Selection → Toolbar:** Text selection triggers BubbleMenu with 200ms debounce. Disappears on click-outside or Escape.
- **Right-click → Context Menu:** `preventDefault()` within editor bounds. Submenus for Perspectives, Study Pack, Send to. Keyboard shortcuts shown for discoverability.
- **Annotation Click → Popover:** Click on an annotation mark opens the popover from Section 1. Uses `editor.view.coordsAtPos()` for positioning. Only one popover open at a time.
- **Keyboard shortcuts:** `⌥A` (Annotate), `⌥F` (Flashcard), `⌥L` (Linked View). All registered via TipTap's `addKeyboardShortcuts()`.

### D. Implementation Notes

- **Floating Toolbar:** Extends existing `EditorToolbar.tsx` pattern. Uses TipTap's `BubbleMenu` — no new TipTap plugin needed.
- **Context Menu:** Radix `ContextMenu` component, pure React, positioned via mouse event coordinates (not ProseMirror). Uses `glass-panel` class.
- **Shared action handlers:** All AI actions (Annotate, Flashcard, Translate, Ask AI) share the exact same handler functions so behavior is identical whether triggered from the floating toolbar, context menu, or keyboard shortcut. Entry point is cosmetic — logic is unified.

## Section 3: Perspectives System

### Core Concept

Perspectives are intelligent, per-section lenses that transform how a user sees and interacts with a specific chunk of content — without ever altering the underlying source text. Each section (heading + content until next heading of equal or higher level) can independently have its own active perspective.

The existing split-pane right side becomes **contextual**: when the cursor enters a section with an active perspective, the right pane updates to show that perspective's output with a 200ms crossfade transition via the `usePerspective` hook. If no perspective is active, the right pane shows the current global mode (Translation/Annotation/Cornell).

### Available Perspectives — Phase 1

| Perspective | Description | Data Source | LLM Required |
|-------------|-------------|-------------|--------------|
| **🔍 Linked View** (★ Hero) | Overlays semantic facts, episodic memories, procedural rules, and cross-note annotations from cognitive memory | `MemoryRetriever` | No |
| **📝 Annotated** | Shows all inline annotations for this section with AI-suggested follow-ups. Highlights cluster density. | `annotations` table | Optional |
| **📖 Study Mode** | Inline flashcard prompts from this section. Click to reveal answers. Due cards highlighted. Quick-review button. | `flashcards` table (FSRS-5) | No |

### Available Perspectives — Phase 2+

| Perspective | Description | LLM Required |
|-------------|-------------|--------------|
| 🌐 Translated | Live translation in split-pane. Auto-detect source lang. Synced scrolling. | Yes |
| ✨ Simplified | AI rewrites at 5th-grade level. Auto-generates beginner flashcards. | Yes |
| 🔬 Expert | Deep-dive analysis with agent persona. Links to cognitive facts. | Yes |
| 🗺 Concept Map | Mini mind-map generated from section. Mermaid rendering. | Yes |

### Data Model

```json
// notes.perspective_config (TEXT, JSON)
{
  "sections": {
    // key = TipTap UniqueID (stable across edits, generated by UniqueID extension)
    "heading-a1b2": {
      "active": "linked-view",
      "params": { "memoryTypes": ["semantic", "episodic"] }
    },
    "heading-c3d4": {
      "active": "translated",
      "params": { "targetLang": "es" }
    },
    "heading-e5f6": null  // raw (default)
  }
}
```

### Section Identity

Sections are identified by heading node IDs. TipTap's `UniqueID` extension generates stable UUIDs per block node. A "section" is a heading + all content until the next heading of equal or higher level.

### Caching Strategy

AI results cached client-side via React Query (`staleTime: 10min`). Cache key: `[noteId, sectionId, perspectiveType, contentHash]`. Content changes invalidate automatically. Important results can be promoted by the user to annotations or cognitive facts for permanent storage.

### Perspective Activation

Three entry points:
1. Right-click → "Apply Perspective…" submenu
2. Click the section badge (e.g., `👓 Linked View`) that appears below each heading
3. Keyboard shortcut `⌥L` (for Linked View)

Click the badge to toggle off.

### Linked View — Deep Dive (Hero Feature)

When Linked View is active on a section, the split-pane right side shows categorized cognitive memory results:

**Semantic Facts** — Related factual knowledge from the cognitive DB, with confidence scores and source notes. Each fact has a "Link ↗" button (brand orange) to create an `entity_link` between the section's annotation and the fact.

**Episodic Memories** — Past learning sessions related to this content, including session IDs, timestamps, and recall performance.

**Related Annotations** — Cross-note annotations from other notes that are semantically related to this section's content, with source note and section references.

The left pane shows the original text with annotation highlights (orange underlines for user annotations, purple for AI-suggested). The section badge (`🔍 Linked View · ⌥L to toggle`) appears below the heading.

### New Backend Endpoint

```
IPC: note_get_linked_context(note_id, section_text) → LinkedContextResponse
```

Returns:
- `semantic_facts: Vec<SemanticFactResult>` — with confidence scores
- `episodic_memories: Vec<EpisodicMemoryResult>` — with session context
- `related_annotations: Vec<AnnotationResult>` — from other notes
- `procedural_rules: Vec<ProceduralRuleResult>` — if applicable

Implementation: Delegates to existing `MemoryRetriever` with section text as the query. No new retrieval logic needed.

## Section 4: Creative Additions

### Phase 1 Additions

#### Knowledge Heat Map

A subtle visual overlay on the left margin of each section showing engagement density — how many annotations, flashcards, and review sessions exist for that section. Sections with no engagement glow cool (blue), heavily annotated sections glow warm (orange). At a glance, you instantly see where your knowledge is deep versus shallow.

**Implementation:** Query annotations + flashcards per section heading ID. CSS gradient on left border. ~50 lines of frontend code.

**Visual:**
- 🟠 Hot (orange): 5+ annotations, high recall (>80%)
- 🟡 Warm (yellow): 1-4 annotations, moderate engagement
- 🔵 Cool (blue): 0 annotations, unread/unengaged

#### Active Recall Gate

When you reopen a note you haven't visited in 3+ days, before showing the content, briefly present 2-3 flashcard prompts from that note's deck. Get them right → note opens normally with a green flash. Get them wrong → the missed sections are highlighted amber, nudging you to re-read. Turns every note revisit into a micro-review session that strengthens long-term retention.

**Implementation:** Check `last_visited` timestamp + query due cards on note open. Modal with existing flashcard review component. ~100 lines of frontend code.

**UX flow:**
1. Open note → "🧠 Quick recall check..."
2. Show 2-3 due cards from this note's deck
3. ✅ All correct → note opens, green flash
4. ❌ Missed one → note opens, missed section glows amber

### Phase 2+ Ideas

- **Smart Highlights** — AI auto-identifies key definitions, important claims, novel concepts, and contradictions with existing knowledge via faint dotted underlines. Click to see reasoning. One-tap promote to annotation or flashcard. LLM call on note save (debounced).
- **Section Confidence Scoring** — Quick 1-5 rating after reading a section, fed into FSRS-5 for section-level review scheduling. Builds personal mastery map.
- **Reading Progress Tracker** — Track which sections you've read (scroll + dwell time). Show progress bar on note cards.
- **Personal Glossary** — Auto-build from "Explain / Define" actions across all notes. Terms linked back to source annotations.
- **Voice Annotations** — Record voice memos attached to text selections. Whisper transcription → annotation content.
- **Focus Reading Mode** — Dims everything except current paragraph. AI context in margins (mini Linked View). Arrow keys advance.
- **Cross-Note Study Sessions** — Select multiple notes → interleaved flashcard session. FSRS-5 scheduling across decks.
- **AI Reading Companion** — Persistent mini-chat following cursor position. Proactive definitions, context, and connections.

## Full Schema Summary

### Existing Tables Modified

```sql
-- cognitive.annotations (5 new columns)
mark_id TEXT           -- links annotation to TipTap mark in document
quoted_text TEXT       -- fallback text content for re-anchoring
range_start INTEGER    -- character offset start (fallback)
range_end INTEGER      -- character offset end (fallback)
ai_suggestion TEXT     -- original AI suggestion at creation time

-- feature-notes.notes (1 new column)
perspective_config TEXT -- JSON: per-section perspective configuration
```

### New TipTap Extensions

- `AnnotationMark` — Custom mark with `{ annotationId: string }` attribute
- `BubbleToolbar` — Selection floating toolbar (extends built-in `BubbleMenu`)
- `UniqueID` — Stable block node IDs (may already exist or use `@tiptap/extension-unique-id`)

### New Frontend Components

- `AnnotationPopover` — Glassmorphic popover for viewing/editing annotations
- `EditorContextMenu` — Radix ContextMenu with grouped actions
- `BubbleToolbarActions` — AI action buttons for the floating toolbar
- `PerspectiveOverlay` — Renders active perspective in split-pane right side
- `LinkedViewPanel` — Categorized cognitive memory results
- `KnowledgeHeatMap` — Left margin engagement indicators
- `ActiveRecallGate` — Modal quiz on note reopen

### New Hooks

- `useAnnotations(noteId)` — CRUD operations for annotations with mark sync
- `usePerspective(sectionId)` — Perspective state + AI result caching
- `useLinkedContext(noteId, sectionText)` — Fetches cognitive memory context
- `useKnowledgeHeat(noteId)` — Engagement density per section

### New IPC Commands

- `annotation_create(noteId, markId, content, quotedText, rangeStart, rangeEnd)` → `AnnotationResponse`
- `annotation_update(annotationId, content, tags)` → `AnnotationResponse`
- `annotation_delete(annotationId)` → `()`
- `annotation_list_for_note(noteId)` → `Vec<AnnotationResponse>`
- `annotation_get_ai_suggestion(noteId, selectedText)` → `AiSuggestionResponse`
- `note_get_linked_context(noteId, sectionText)` → `LinkedContextResponse`
- `note_update_perspective_config(noteId, config)` → `()`

## Phase 1 Scope Summary

**In scope:**
- Annotation system (marks + external storage + popover + AI suggestions)
- Floating toolbar (BubbleMenu with Annotate, Flashcard, Translate, Ask AI)
- Right-click context menu (grouped actions with keyboard shortcuts)
- Perspectives engine (per-section config, contextual split-pane)
- Linked View perspective (cognitive memory overlay — hero feature)
- Annotated perspective (annotation list + AI follow-ups)
- Study Mode perspective (inline flashcard prompts)
- Knowledge Heat Map (engagement density visualization)
- Active Recall Gate (micro-quiz on note reopen)
- Keyboard shortcuts (⌥A, ⌥F, ⌥L)

**Out of scope (Phase 2+):**
- Translated, Simplified, Expert, Concept Map perspectives
- Smart Highlights (AI auto-flagging)
- Section Confidence Scoring
- Voice Annotations
- Focus Reading Mode
- Cross-Note Study Sessions
- AI Reading Companion
- Personal Glossary
- Reading Progress Tracker
