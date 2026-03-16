# Insight Review — Design Specification

**Date:** 2026-03-16
**Status:** Approved
**Feature:** Transform the disabled "Synthesize" button in the AI Suggestions panel into a full "Insight Review" system — a deep AI-powered research companion that synthesizes knowledge, finds gaps, generates quizzes, and maps concepts.

---

## 1. Overview

### Trigger
- **Button name:** "Insight Review"
- **Icon:** `Brain` (lucide-react)
- **Tooltip:** "Deep analysis of your knowledge network"
- **Keyboard shortcut:** `Cmd+Shift+I` (toggle)
- **Location:** Replaces the current disabled "Synthesize" button in `AISuggestionsPanel.tsx`

### Behavior
When activated, the existing right-side context panel (~280px) smoothly expands to **640px** and replaces its content with the Insight Review interface. The editor area shrinks proportionally. On close (X, Escape, or `Cmd+Shift+I`), the panel contracts back to ~280px and restores normal context sections.

### Loading Strategy: Hybrid (Option C)
1. **On open:** Immediately start a streaming LLM call for Tab 1 (Synthesis). User sees first tokens in ~2-3 seconds.
2. **In parallel:** Fire off Tabs 2-4 (Gap Analysis, Self-Assessment, Concept Map) as structured JSON calls in the background.
3. **Tab switching:** Instant for any tab that has finished loading. Skeleton loaders shown for in-progress tabs.

### Edge Cases
- **Empty note / no body:** Show a friendly message: "Add some content to your note to generate insights." Disable the Insight Review button when `note.body` is empty or whitespace-only.
- **No related notes:** Still generate insights from the current note alone + cognitive memory. The LLM prompts handle this gracefully (context block will just have fewer entries). Synthesis tab notes "limited connections found" if < 2 related notes.
- **LLM failure:** Per-tab error state with retry button. Other tabs continue independently. Global "Regenerate All" in header retries all failed tabs.
- **Very long notes:** Context assembly caps total input at ~12,000 tokens. Current note gets up to 4,000 tokens, related notes split the remaining budget (truncate longest first).

---

## 2. The 4 Tabs

### Tab 1: Synthesis
- Deep, well-written synthesis connecting the current note + all related notes + relevant semantic facts from cognitive memory.
- Output: clean Markdown with key themes, connections, and non-obvious insights.
- **Streamed** — first tab visible, first tokens in ~2-3s.

### Tab 2: Gap Analysis
- Detects knowledge gaps, missing concepts, contradictions, shallow coverage.
- Suggests specific next steps (papers, topics, new notes to create).
- Leverages cognitive memory + vector search for context.
- Output: structured Markdown with clear sections.

### Tab 3: Self-Assessment
- Generates 8 quiz questions: 4 multiple choice + 4 short answer.
- Each question includes: type, question text, choices (if MC), correct answer, explanation, source notes, difficulty ("easy"/"medium"/"hard"), difficulty_score (0.0-1.0), and a unique id.
- Interactive quiz UI with reveal-on-check, running score, and "Reveal All Answers" button (appears after the user has answered 3+ individual questions).
- After completing quiz: prominent "Save as Flashcard Deck" option.

### Tab 4: Concept Map
- Mermaid `mindmap` diagram showing concept connections across the note cluster.
- Max 4 levels deep, max 5-6 branches per node, max 6 words per label.
- Fallback: if Mermaid parsing fails, show clean indented text outline.
- "Copy Mermaid code" button for power users.
- Dark theme matching glassmorphism design system.

---

## 3. Database Schema

### `flashcards` table (in `cognitive` crate)

```sql
CREATE TABLE IF NOT EXISTS flashcards (
    id TEXT PRIMARY KEY,
    source_note_id TEXT,
    source_session_id TEXT,
    insight_review_id TEXT,
    deck TEXT NOT NULL DEFAULT 'general',
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'short_answer',
    choices JSON,

    -- FSRS scheduling (defaults are for cards inserted outside of insight_save_flashcards)
    stability REAL NOT NULL DEFAULT 1.0,
    difficulty REAL NOT NULL DEFAULT 0.5,
    due_at TEXT,
    last_reviewed_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_flashcards_source_note ON flashcards(source_note_id);
CREATE INDEX idx_flashcards_due ON flashcards(due_at);
CREATE INDEX idx_flashcards_deck ON flashcards(deck);
CREATE INDEX idx_flashcards_insight ON flashcards(insight_review_id);
```

### `insight_review_cache` table

```sql
CREATE TABLE IF NOT EXISTS insight_review_cache (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    synthesis TEXT,
    gap_analysis TEXT,
    self_assessment TEXT,
    concept_map TEXT,
    updated_at TEXT NOT NULL,

    UNIQUE(note_id, content_hash)
);

CREATE INDEX idx_insight_cache_note ON insight_review_cache(note_id);
```

Per-tab caching: both combined response and individual tab outputs are stored so "Regenerate Tab X" works without re-running everything. The `updated_at` column tracks when any tab was last regenerated.

### Cache Invalidation Strategy
The `content_hash` is computed from: `SHA-256(note.body + sorted(related_note_ids))`. This means the cache invalidates when:
- The current note's body changes
- The set of related notes changes (e.g., new notes become semantically similar)

**Accepted trade-off:** Changes to a related note's *body* do not invalidate the cache (only the related note set matters). This is acceptable because: (1) insight reviews are a point-in-time analysis, (2) the user can always click "Regenerate" to refresh, and (3) computing hashes of all related note bodies on every open would be expensive.

### Migration
Both `flashcards` and `insight_review_cache` tables are added to the existing `cognitive` crate migration. Per CLAUDE.md pre-release policy: bump the `FeatureMigration` version in `crates/cognitive/src/repos/mod.rs` and append the new DDL to the existing SQL in-place (no incremental migration files).

### FSRS Init on Save
- easy difficulty → stability 4.0 days
- medium difficulty → stability 2.0 days
- hard difficulty → stability 0.8 days

### FlashcardRepo API (in `crates/cognitive/src/repos/`)

```rust
impl FlashcardRepo {
    pub async fn create_batch(&self, cards: Vec<NewFlashcard>) -> Result<Vec<FlashcardRow>>;
    pub async fn get_due_cards(&self, deck: Option<&str>, limit: usize) -> Result<Vec<FlashcardRow>>;
    pub async fn record_review(&self, id: &str, quality: ReviewQuality) -> Result<FlashcardRow>;
    pub async fn list_by_note(&self, note_id: &str) -> Result<Vec<FlashcardRow>>;
    pub async fn list_decks(&self) -> Result<Vec<DeckSummary>>;
    pub async fn delete_deck(&self, deck: &str) -> Result<()>;
}
```

`ReviewQuality` enum: `Again`, `Hard`, `Good`, `Easy`.

Deck naming: auto-generated as `Review: [Note Title]` or `[Domain] Knowledge Review`.

---

## 4. Context Assembly (Shared Across All Tabs)

Before calling the LLM, the handler assembles:

1. **Current note:** title + full body
2. **Related notes:** top 8 from `note_suggestions` scoring (title + first 500 chars each)
3. **Cognitive memory:** top 15 semantic facts via `MemoryRetriever` (query = note body, scored by FSRS retrievability + relevance)
4. **Link graph:** backlinks + outlinks for the current note
5. **Tags:** all tags across the note cluster

This context block is injected into each tab's prompt.

---

## 5. LLM Prompts

### Tab 1: Synthesis

```
You are a research synthesis assistant. Given the user's note and its related notes
from their knowledge base, write a deep synthesis that:

1. Identifies the 3-5 key themes across these notes
2. Draws non-obvious connections between concepts
3. Highlights where ideas reinforce or build on each other
4. Surfaces insights the user may not have explicitly written

Format as clean Markdown with ## headings for each theme.
Keep it focused and insightful — not a summary, but a synthesis.
Do not repeat content verbatim from the notes.

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 2: Gap Analysis

```
You are a knowledge gap analyst. Given the user's note cluster, identify:

1. **Missing concepts** — important topics referenced but never explored in depth
2. **Contradictions** — places where notes disagree or present conflicting info
3. **Shallow coverage** — topics mentioned briefly that deserve deeper treatment
4. **Research suggestions** — specific papers, books, or topics to explore next
5. **Notes to create** — suggest 2-3 new note titles that would strengthen the network

Format as Markdown with clear sections. Be specific and actionable.
For each gap, reference which note(s) it relates to.

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 3: Self-Assessment

```
You are an educational assessment designer. Generate a self-assessment quiz based
on the user's knowledge network.

Generate exactly 8 questions:
- 4 multiple choice (4 options each, one correct)
- 4 short answer (expecting 1-2 sentence responses)

For each question, include:
- A unique short id (e.g. "q1", "q2")
- The question text
- The correct answer
- A brief explanation of why
- Which note(s) the question draws from
- Difficulty: "easy", "medium", or "hard"
- Difficulty score: 0.0-1.0 (for FSRS initialization)

Questions should test understanding, not memorization. Include questions that
require connecting ideas across multiple notes.

Respond as JSON array:
[{
  "id": "q1",
  "type": "multiple_choice" | "short_answer",
  "question": "...",
  "choices": ["A", "B", "C", "D"] | null,
  "correct_answer": "...",
  "explanation": "...",
  "source_notes": ["note title 1", "note title 2"],
  "difficulty": "easy" | "medium" | "hard",
  "difficulty_score": 0.35
}]

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 4: Concept Map

```
You are a concept mapping specialist. Create a Mermaid mindmap diagram showing
how concepts connect across the user's note cluster.

Rules:
- Use Mermaid `mindmap` syntax exactly
- Root node = the current note's title wrapped in double parens: root((Title))
- Branch into major themes/concepts
- Show connections to related notes by name
- Max 4 levels deep, max 5-6 branches per node
- Max 6 words per node label
- Use clean, short labels (no full sentences)

If you cannot generate valid Mermaid syntax, return a clean indented text outline
instead, prefixed with "FALLBACK:" on the first line.

Example format:
mindmap
  root((Machine Learning Notes))
    Supervised Learning
      Regression
      Classification
    Neural Networks
      Deep Learning Architectures
      Transformer Models
    Applications
      NLP
      Computer Vision

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

---

## 6. Backend Handler

### Streaming IPC Contract

Tauri `#[tauri::command]` functions cannot stream and return a final value in one call. Following the existing `chat_send` pattern:

1. The `note_insight_review` command returns an initial `InsightReviewStarted { insightReviewId, contentHash, cached: bool }` immediately.
2. If `cached: true`, the frontend calls `note_insight_cache_get` to load all tabs instantly.
3. If `cached: false`, the backend spawns a background task that emits Tauri events:
   - **`insight:synthesis-chunk`** `{ insightReviewId, chunk: string }` — streaming tokens for Tab 1
   - **`insight:synthesis-done`** `{ insightReviewId, content: string }` — Tab 1 complete
   - **`insight:tab-done`** `{ insightReviewId, tab: string, content: string }` — Tabs 2-4 complete (one event per tab)
   - **`insight:error`** `{ insightReviewId, tab: string, error: string }` — per-tab error
4. Frontend listens for these events via `listen('insight:*')` and updates per-tab state.

### Layer Boundary: FlashcardRow → FlashcardResponse DTO

`FlashcardRow` lives in `crates/cognitive/src/repos/flashcard.rs` (L5). It cannot appear in `desktop-shared` (L7) IPC types. Instead, define a `FlashcardResponse` DTO in `desktop-shared` and map `FlashcardRow → FlashcardResponse` in the `app-core` handler layer (same pattern as `NoteRow → NoteResponse`).

### File: `crates/app-core/src/handlers/notes/insight.rs`

```rust
impl AppCore {
    /// Start insight review: check cache, spawn LLM tasks, return initial response
    pub async fn note_insight_review(&self, note_id: &str) -> Result<InsightReviewStarted, ApiError> {
        // 1. Load note + compute content_hash (SHA-256 of note.body + sorted related note IDs)
        // 2. Check insight_review_cache by (note_id, content_hash)
        // 3. If cache hit → return InsightReviewStarted { cached: true, ... }
        // 4. Else:
        //    a. Assemble context (note + related + cognitive facts + links + tags)
        //       Total context budget: ~12,000 tokens. Current note: up to 4,000.
        //       Related notes split remaining budget (truncate longest first).
        //    b. Generate insight_review_id (nanoid)
        //    c. Spawn background task:
        //       - Stream Tab 1 (Synthesis) via Tauri events (insight:synthesis-chunk)
        //       - Fire Tabs 2-4 in parallel (structured JSON mode)
        //       - Emit insight:tab-done for each completed tab
        //       - Cache all results
        //    d. Return InsightReviewStarted { cached: false, insightReviewId, contentHash }
    }

    /// Regenerate a single tab (uses cached context, only re-runs one prompt)
    pub async fn note_insight_regenerate_tab(
        &self,
        note_id: &str,
        tab: InsightTab,
    ) -> Result<TabContent, ApiError> {
        // Re-run single LLM call, update cache for that tab only
    }

    /// Save quiz questions as flashcards with FSRS init
    pub async fn insight_save_flashcards(
        &self,
        note_id: &str,
        insight_review_id: &str,
        deck_name: &str,
        questions: Vec<QuizQuestion>,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        // Map difficulty_score to initial FSRS stability:
        //   easy (< 0.33) → 4.0 days
        //   medium (0.33-0.66) → 2.0 days
        //   hard (> 0.66) → 0.8 days
        // Batch insert via FlashcardRepo
        // Convert FlashcardRow → FlashcardResponse for IPC
    }

    /// Get cached insight review (for instant re-open)
    pub async fn note_insight_cache_get(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewResponse>, ApiError> {
        // Load most recent cache entry for this note
    }
}
```

### IPC Commands

All four commands must be added to `tauri::generate_handler!` in `crates/desktop/src/main.rs` and to `DEV_COMMANDS` + `dispatch_dev` in `crates/desktop/src/commands/notes.rs` (enforced by the `dev_server_covers_all_tauri_commands` test).

| Command | Args | Returns |
|---------|------|---------|
| `note_insight_review` | `{ noteId }` | `InsightReviewStarted` (then streams via Tauri events) |
| `note_insight_regenerate_tab` | `{ noteId, tab }` | `TabContent` |
| `note_insight_save_flashcards` | `{ noteId, insightReviewId, deckName, questions }` | `Vec<FlashcardResponse>` |
| `note_insight_cache_get` | `{ noteId }` | `Option<InsightReviewResponse>` |

### Tauri Event Names
| Event | Payload | Direction |
|-------|---------|-----------|
| `insight:synthesis-chunk` | `{ insightReviewId, chunk }` | Backend → Frontend |
| `insight:synthesis-done` | `{ insightReviewId, content }` | Backend → Frontend |
| `insight:tab-done` | `{ insightReviewId, tab, content }` | Backend → Frontend |
| `insight:error` | `{ insightReviewId, tab, error }` | Backend → Frontend |

---

## 7. Frontend Components

### Component Tree

```
KnowledgeBasePage
  └── ContextPanel (width transitions: ~280px ↔ 640px)
        ├── [Normal mode] AISuggestionsPanel, BacklinksPanel, GraphMinimap, MoreSection
        └── [Insight mode] InsightReviewPanel
              ├── InsightHeader (title, close X, global regenerate)
              ├── InsightTabs (4 tab buttons + per-tab regenerate icons + status dots)
              ├── InsightContent (scrollable, switches by active tab)
              │     ├── SynthesisTab → MarkdownContent (streaming)
              │     ├── GapAnalysisTab → MarkdownContent
              │     ├── SelfAssessmentTab → QuizRenderer
              │     │     ├── QuizCard (per question, with reveal)
              │     │     ├── QuizScore (running tally)
              │     │     └── RevealAllButton (after 3+ attempts)
              │     └── ConceptMapTab → MermaidRenderer (with text fallback)
              │           └── CopyMermaidButton
              └── InsightFooter (action buttons, always visible)
```

### Files to Create

| File | Purpose |
|------|---------|
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Main panel: header, tabs, content router, footer |
| `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx` | Streaming markdown renderer |
| `desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx` | Gap analysis markdown |
| `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx` | Interactive quiz with cards |
| `desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx` | Mermaid renderer + fallback |
| `desktop-ui/src/features/notes/components/insight/MermaidRenderer.tsx` | Mermaid.js wrapper (dark theme) |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | State management, IPC calls, streaming |

### State Shape (`useInsightReview.ts`)

```typescript
interface InsightReviewState {
  isOpen: boolean;
  noteId: string | null;
  insightReviewId: string | null;
  contentHash: string | null;
  activeTab: 'synthesis' | 'gaps' | 'assessment' | 'concept-map';
  tabs: {
    synthesis: { status: 'idle' | 'streaming' | 'done' | 'error'; content: string };
    gaps: { status: 'idle' | 'loading' | 'done' | 'error'; content: string };
    assessment: { status: 'idle' | 'loading' | 'done' | 'error'; questions: QuizQuestion[] };
    conceptMap: { status: 'idle' | 'loading' | 'done' | 'error'; mermaid: string; fallbackText: string };
  };
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
}
```

### Panel Transition

In `ContextPanel.tsx`, the width is driven by a prop from `KnowledgeBasePage`:
- `insightOpen ? 640 : defaultContextWidth`
- CSS: `transition-[width] duration-250 ease-out`
- Content swaps between normal context sections and `InsightReviewPanel`

### Tab Status Indicators
- **Idle:** dim dot
- **Loading/Streaming:** pulsing purple dot (`animate-pulse` + `--purple`)
- **Done:** solid green dot (`--success`)
- **Error:** red dot (`--destructive`) with retry

### Footer Actions

| Button | Icon | Condition | Behavior |
|--------|------|-----------|----------|
| Insert into note | `FileInput` | Always enabled | Appends `## Insight Review — {date}` + active tab content |
| Create note | `FilePlus` | Always enabled | Creates "Insight: {Note Title}", auto-links source notes |
| Save as Deck | `BookOpen` | Enabled after 50%+ quiz answered. Brand pulse when ready. | Saves flashcards with FSRS init |
| Copy | `Copy` | Always enabled | Copies active tab content (markdown or mermaid source) |

### Keyboard Shortcuts
- `Cmd+Shift+I` — toggle open/close (local keyboard shortcut in KnowledgeBasePage, NOT a global Tauri shortcut — does not conflict with existing global shortcuts like `Cmd+Shift+C`)
- `1/2/3/4` — switch tabs (when panel focused)
- `Escape` — close panel
- `Cmd+Shift+R` — regenerate active tab

---

## 8. Styling

All components use the existing glassmorphism design system:
- Panel background: `glass-panel` class
- Inner sections: `bg-white/[0.04]` with `border-border` separators
- Tab bar: `bg-white/[0.06]` active state with `border-b border-border` separator
- Quiz cards: `glass-card` styling
- Footer: `border-t border-border` with `glass-button` styled actions
- Status dots: CSS custom properties (`--purple`, `--success`, `--destructive`)
- Mermaid: dark theme with `--brand` accent color integration
- All text follows hierarchy: `text-primary`, `text-secondary`, `text-muted`, `text-dim`

---

## 9. New & Modified Files

### Backend (new files)

| File | Crate | Purpose |
|------|-------|---------|
| `crates/cognitive/src/repos/flashcard.rs` | cognitive | FlashcardRepo + FlashcardRow + NewFlashcard + ReviewQuality types |
| `crates/app-core/src/handlers/notes/insight.rs` | app-core | Insight Review handler (context assembly, LLM calls, caching, streaming events) |

### Backend (modified files)

| File | Crate | Change |
|------|-------|--------|
| `crates/cognitive/src/repos/mod.rs` | cognitive | Register `flashcard` module, add FlashcardRepo to Repos struct |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | cognitive | Append `flashcards` + `insight_review_cache` DDL, bump version |
| `crates/desktop-shared/src/commands/notes.rs` | desktop-shared | Add `InsightReviewStarted`, `InsightReviewResponse`, `TabContent`, `QuizQuestion`, `FlashcardResponse` DTOs |
| `crates/desktop/src/commands/notes.rs` | desktop | Add 4 Tauri IPC commands + update `DEV_COMMANDS` + `dispatch_dev` |
| `crates/desktop/src/main.rs` | desktop | Register 4 new commands in `tauri::generate_handler!` |
| `crates/app-core/src/handlers/notes/mod.rs` | app-core | Register `insight` module |

---

## 10. Dependencies

### New (frontend)
- `mermaid` — Mermaid.js for concept map rendering (~200KB, note: not effectively tree-shakeable, full bundle includes all diagram parsers)

### Existing (no changes needed)
- `react-markdown` + `remark-gfm` + `rehype-highlight` — already in chat feature
- `lucide-react` — already used throughout
- `nanoid` — already available for ID generation
