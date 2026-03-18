# Learning System Design: Flashcard Engine + Learning Hub + AI Tutor

**Date:** 2026-03-18
**Status:** Approved
**Approach:** B — New `feature-learning` crate with `/learn` as a first-class peer to `/notes`

## Overview

Upgrade Klyntbot's basic flashcard system into a complete, interconnected learning platform. Two mental models for the user:

- **Notes (`/notes`)** = "Capture & Build" — creative workspace for research, translation, annotation
- **Learn (`/learn`)** = "Review & Master" — daily training ground for spaced repetition, AI tutoring, analytics

The systems are peers in the sidebar, connected through a shared layer of cognitive memory, FSRS-5 scheduling, BookRAG retrieval, and the agent runtime.

## Architecture

### Crate Layout

**New crate: `feature-learning` (Layer 4)**

```
crates/feature-learning/
  ├── src/
  │   ├── lib.rs              # FeaturePackage impl, re-exports
  │   ├── card_generator.rs   # AI-powered card generation pipeline
  │   ├── study_planner.rs    # AI study session plan generation
  │   ├── tutor.rs            # Socratic tutoring engine, misconception tracker
  │   ├── analytics.rs        # Session stats, retention curves, mastery
  │   ├── sentence_miner.rs   # Extract vocab from foreign text
  │   └── types.rs            # StudySession, StudyPlan, TutorExchange, etc.
  ├── migrations/
  │   └── 001_learning_tables.sql
  └── Cargo.toml
```

**Dependencies (strict upward flow):**

```
feature-learning (L4)
  ├── cognitive (L3)        → FlashcardRepo, FSRS engine, SemanticFactRepo
  ├── feature-notes (L4)    → NoteRepo (source linking, content retrieval)
  │                           Note: L4→L4 dep has precedent (feature-insights → feature-notes).
  │                           Only uses NoteRepo for read-only source lookups, not mutations.
  ├── context_engine (L3)   → BookRAG (grounding tutor in note content)
  ├── providers (L3)        → LLM calls for card generation, tutoring
  ├── tools-core (L1)       → Tool/FeaturePackage traits
  └── common (L0)           → Result, error types
```

**Component ownership:**

| Component | Crate | Rationale |
|-----------|-------|-----------|
| `FlashcardRepo`, `FlashcardRow`, FSRS-5 scheduling | `cognitive` | Flashcards are memory primitives — same decay model as `SemanticFact` |
| Card generation pipeline | `feature-learning` | Business logic for turning notes into cards |
| Study planner + tutor | `feature-learning` | Learning-specific orchestration |
| Analytics + mastery tracking | `feature-learning` | Learning-specific derived data |
| Split-pane editor modes | `desktop-ui/features/notes/` | Editor extension — belongs with the editor |
| `/learn` page + review UI | `desktop-ui/features/learn/` | New first-class UI feature |

### Frontend Structure

```
desktop-ui/src/features/learn/
  ├── pages/
  │   └── LearnPage.tsx           # Top-level route, dashboard home
  ├── components/
  │   ├── dashboard/
  │   │   ├── DashboardHome.tsx   # Stats, decks, weak areas, quick generate
  │   │   ├── StatsBar.tsx        # Streak, due, retention, weekly count
  │   │   ├── DeckList.tsx        # Per-deck stats and quick-review buttons
  │   │   ├── WeakAreas.tsx       # Concept mastery weak spots
  │   │   └── QuickGenerate.tsx   # From note / clipboard / last chat
  │   ├── review/
  │   │   ├── ImmersiveReview.tsx  # Full-screen card review mode
  │   │   ├── CardRenderer.tsx     # Renders all card types (basic, cloze, vocab, typed, image)
  │   │   ├── RatingButtons.tsx    # Again/Hard/Good/Easy with intervals
  │   │   ├── AskAIPanel.tsx       # Inline Socratic dialogue during review
  │   │   └── PostSession.tsx      # Session summary screen
  │   ├── tutor/
  │   │   ├── TutorSession.tsx     # AI-guided session with plan sidebar
  │   │   ├── StudyPlan.tsx        # Left sidebar: plan sections with progress
  │   │   └── TutorChat.tsx        # Socratic chat panel (reuses SSE pattern)
  │   ├── cards/
  │   │   ├── QuickAdd.tsx         # ⌘N modal: front/back/deck, AI-assisted
  │   │   ├── CardEditor.tsx       # Full card editor (all types)
  │   │   ├── ClozeEditor.tsx      # Cloze-specific editing with {{c1::}} syntax
  │   │   └── CardPreview.tsx      # Preview during generation approval flow
  │   └── analytics/
  │       ├── RetentionCurve.tsx    # 7-day retention chart (recharts)
  │       ├── ReviewHeatmap.tsx     # GitHub-style 365-day calendar
  │       └── MasteryMap.tsx        # Knowledge graph with mastery overlay
  └── hooks/
      ├── useFlashcards.ts         # CRUD, deck management, due queries
      ├── useStudySession.ts       # Session lifecycle, plan tracking
      ├── useTutor.ts              # Socratic dialogue state, SSE streaming
      └── useAnalytics.ts          # Dashboard stats queries
```

## Data Model

### Upgraded `cognitive` flashcard schema

**Migration note:** The existing `CardType` enum has `MultipleChoice` and `ShortAnswer` variants (from the quiz/insight system). These are replaced by the new card types below. Since this is pre-release with no user data to migrate, the `card_type` column and `CardType` enum are rewritten in-place. The existing `front`/`back` TEXT columns are retained and used by all card types.

```sql
-- Existing flashcards table, upgraded columns:
ALTER TABLE flashcards ADD COLUMN card_type TEXT DEFAULT 'basic';
  -- 'basic' | 'cloze' | 'vocabulary' | 'typed' | 'image_occlusion'
ALTER TABLE flashcards ADD COLUMN source_note_id TEXT;
ALTER TABLE flashcards ADD COLUMN source_context TEXT;      -- surrounding paragraph
ALTER TABLE flashcards ADD COLUMN cloze_data TEXT;           -- JSON for cloze fields
ALTER TABLE flashcards ADD COLUMN vocab_data TEXT;           -- JSON: {reading, meaning, example, audio_url}
ALTER TABLE flashcards ADD COLUMN image_data TEXT;           -- JSON: {image_path, occlusion_regions}
ALTER TABLE flashcards ADD COLUMN tags TEXT DEFAULT '[]';
ALTER TABLE flashcards ADD COLUMN suspended INTEGER DEFAULT 0;
ALTER TABLE flashcards ADD COLUMN recall_speed_ms INTEGER;

-- FSRS-5 personal parameters
CREATE TABLE fsrs_parameters (
  id TEXT PRIMARY KEY DEFAULT 'local',
  weights TEXT NOT NULL,              -- JSON: 19 FSRS-5 learned weights
  desired_retention REAL DEFAULT 0.9,
  trained_at TEXT,
  review_count INTEGER DEFAULT 0
);

-- Full review log (feeds FSRS-5 training)
CREATE TABLE review_log (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL REFERENCES flashcards(id),
  rating INTEGER NOT NULL,            -- 1=again, 2=hard, 3=good, 4=easy
  elapsed_days REAL NOT NULL,
  scheduled_days REAL NOT NULL,
  recall_speed_ms INTEGER,
  state TEXT NOT NULL,
  reviewed_at TEXT NOT NULL
);
```

### New `feature-learning` tables

```sql
-- Study sessions
CREATE TABLE study_sessions (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  plan TEXT,                          -- JSON: StudyPlan
  cards_reviewed INTEGER DEFAULT 0,
  cards_correct INTEGER DEFAULT 0,
  duration_seconds INTEGER,
  adapted INTEGER DEFAULT 0           -- did the plan adapt mid-session?
);

-- Concept mastery (derived from card performance)
CREATE TABLE concept_mastery (
  concept TEXT PRIMARY KEY,
  mastery_level REAL DEFAULT 0.0,
  card_count INTEGER DEFAULT 0,
  error_patterns TEXT,                -- JSON: common misconceptions
  last_reviewed TEXT,
  updated_at TEXT
);

-- Tutor exchanges (Socratic dialogue history)
CREATE TABLE tutor_exchanges (
  id TEXT PRIMARY KEY,
  session_id TEXT REFERENCES study_sessions(id),
  card_id TEXT REFERENCES flashcards(id),
  exchange TEXT NOT NULL,             -- JSON: [{role, content}]
  insight_saved_to_note INTEGER DEFAULT 0,
  created_at TEXT NOT NULL
);

-- Note review badges (feedback arrow: Learn → Notes)
CREATE TABLE note_review_stats (
  note_id TEXT PRIMARY KEY,
  card_count INTEGER DEFAULT 0,
  last_reviewed TEXT,
  avg_retention REAL,
  updated_at TEXT
);
```

### Notes split-pane storage

```sql
ALTER TABLE notes ADD COLUMN split_content TEXT;  -- JSON: {left, right, summary?}
ALTER TABLE notes ADD COLUMN split_mode TEXT;      -- null | 'translation' | 'annotation' | 'cornell'
```

When `split_content` is non-null, the editor reads from it. The `body` field stays as a flattened version (left + right concatenated) so FTS5 and BookRAG index all content regardless of mode.

## FSRS-5 Engine

### Scope of FSRS-5 changes

**Important:** The FSRS-5 upgrade applies **only to flashcard scheduling** in `FlashcardRepo`. The existing `decay.rs` functions (`retrievability()`, `update_stability()`, `relevance_score()`) used by the broader cognitive memory system (semantic fact retrieval, relevance scoring) are **unchanged**. The flashcard scheduler gets its own dedicated FSRS-5 implementation (new file: `cognitive/src/services/fsrs5.rs`), leaving the cognitive memory decay model intact.

### Upgrade from current system

```
Current: stability drives due_at, difficulty never updated, 90-day cap
    ↓
FSRS-5:  19 learned weights, difficulty updates per review,
         personal parameter training from review_log,
         no arbitrary cap, desired_retention configurable
```

### Core algorithm

```
record_review(card_id, rating, recall_speed_ms)
  → fetch card state (stability, difficulty, elapsed_days)
  → compute retrievability: R = (1 + elapsed / (9 * S))^(-1)
  → update difficulty: D' = w₇ · D₀(4) + (1 - w₇) · mean_revert(D, rating)
  → update stability based on rating:
      Good/Easy: S' = S · (e^(w₈) · (11-D) · S^(-w₉) · (e^(w₁₀·(1-R)) - 1) · hard_penalty · easy_bonus)
      Again:     S' = w₁₁ · D^(-w₁₂) · ((S+1)^w₁₃ - 1) · e^(w₁₄·(1-R))
  → compute next_due: interval = S' * 9 * (1/desired_retention - 1)
  → log to review_log for future weight training
  → update card: new stability, difficulty, due_at, state, lapses
```

Implementation: port FSRS-5 directly (~200 lines of math) rather than depending on `fsrs-rs` crate. Avoids external dependency, allows extending with recall-speed weighting and concept-level signals.

**Weight training:** Uses FSRS-5 default weights initially. Personal weight optimization runs after 400+ reviews in `review_log` (FSRS-5 recommended minimum). Training is triggered manually or on a weekly schedule. Until trained, default weights provide good-enough scheduling.

### Card types

| Type | Front | Back | Extra Fields |
|------|-------|------|--------------|
| **Basic** | Question text/HTML | Answer text/HTML | — |
| **Cloze** | Text with `{{c1::hidden}}` markers | Full text revealed | `cloze_data`: JSON with cloze indices and hints |
| **Vocabulary** | Word + reading (optional ruby) | Meaning + example sentence | `vocab_data`: `{word, reading, meaning, example_sentence, audio_url, part_of_speech}` |
| **Typed** | Question | Expected answer | User types answer, fuzzy-matched |
| **Image Occlusion** | Image with regions hidden | Image fully revealed | `image_data`: `{image_path, regions: [{x,y,w,h,label}]}` |

### Card states

`new` → `learning` → `review` ↔ `relearning`. Learning steps: 1m, 10m (configurable).

### Recall speed tracking

Every review records `recall_speed_ms`. Cards answered significantly slower than personal mean baseline get a stability penalty even on "Good" rating. Optional, non-blocking.

## Learning Hub UI (`/learn`)

### Route & Navigation

- Top-level sidebar entry: `/learn` (peer to `/notes`, `/tasks`, `/finance`)
- Sidebar icon shows due card count badge (e.g., "23")
- Tray countdown integration: "📚 23 cards due" when no focus session or calendar event

### Dashboard Home (default view)

```
┌──────────────────────────────────────────────────┐
│  Header: "Learning Hub"          [Quick Add ⌘N]  │
├──────────────────────────────────────────────────┤
│                                                  │
│  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────────┐    │
│  │🔥 7  │  │📚 23 │  │✅ 87%│  │📈 142    │    │
│  │streak│  │due   │  │retn  │  │this week │    │
│  └──────┘  └──────┘  └──────┘  └──────────┘    │
│                                                  │
│  ┌─────────────────────┐ ┌────────────────────┐ │
│  │  Start Review (23)  │ │ AI Study Session   │ │
│  │  ▸ 15 review        │ │ ▸ 25 min plan      │ │
│  │  ▸ 5 new            │ │ ▸ Focus: て-form   │ │
│  │  ▸ 3 relearn        │ │ ▸ Based on weak    │ │
│  │  [Start →]          │ │   areas + decay    │ │
│  │                     │ │ [Start Session →]  │ │
│  └─────────────────────┘ └────────────────────┘ │
│                                                  │
│  ┌─ Decks ────────────────────────────────────┐ │
│  │ Japanese Vocab N3    12 due  92% retention  │ │
│  │ Grammar Patterns      8 due  78% retention  │ │
│  │ Research Methods       3 due  95% retention  │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌─ Weak Areas ──────┐ ┌─ Quick Generate ────┐ │
│  │ て-form conjug.    │ │ From note...        │ │
│  │ N3 Kanji readings  │ │ From clipboard...   │ │
│  │ Passive voice      │ │ From last chat...   │ │
│  └────────────────────┘ └─────────────────────┘ │
│                                                  │
│  ┌─ Recent Activity ─────────────────────────┐  │
│  │ 📊 Retention curve (7-day mini chart)     │  │
│  │ 📅 Review heatmap (GitHub-style)          │  │
│  └───────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

### Immersive Review Mode

Entered via "Start Review". Full-screen, distraction-free. ESC returns to dashboard.

```
┌──────────────────────────────────────────────────┐
│ ← ESC                    5 / 23    Japanese N3   │
├──────────────────────────────────────────────────┤
│                                                  │
│              ┌─────────────────┐                │
│              │   食べてみる     │                │
│              │  (card front)   │                │
│              └─────────────────┘                │
│                                                  │
│         ─────────────────────────                │
│                                                  │
│              to try eating                       │
│              "I tried eating sushi"               │
│              (card back, after reveal)            │
│                                                  │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐           │
│  │Again │ │Hard  │ │Good  │ │Easy  │           │
│  │ 1m   │ │ 6m   │ │ 4d   │ │ 9d   │           │
│  │  1   │ │  2   │ │  3   │ │  4   │           │
│  └──────┘ └──────┘ └──────┘ └──────┘           │
│                                                  │
│  [💡 Ask AI]  [📝 Edit]  [🔗 Source note]       │
│                                                  │
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ progress bar │
└──────────────────────────────────────────────────┘

Keyboard: Space=reveal, 1/2/3/4=rate, E=edit, S=source, A=ask AI
```

### AI Tutor Session

Entered via "Start AI Session". Left sidebar shows plan (collapsible), main area switches between card review and Socratic chat.

```
┌──────────────────────────────────────────────────┐
│ ← Exit Session              15:23 remaining      │
├──────────────────────────────────────────────────┤
│                                                  │
│  Study Plan            ┌────────────────────┐   │
│  ─────────             │                    │   │
│  ✅ Vocab warm-up (8)  │   Active Area      │   │
│  ▸ て-form drill (5)   │                    │   │
│    ▸ + 2 adapted       │   (card review     │   │
│  ○ Conversation (3m)   │    OR tutor chat   │   │
│  ○ Wrap-up review      │    OR conversation)│   │
│                        │                    │   │
│  Plan adapted: +2      │  [💬 Talk to Tutor] │   │
│  cards added based     │  [▸ Resume Plan]    │   │
│  on performance        │                    │   │
│                        └────────────────────┘   │
└──────────────────────────────────────────────────┘
```

"Talk to Tutor" pauses the plan and opens Socratic chat scoped to current card + note context via BookRAG. "Resume Plan" continues where you left off.

### Key interactions

- **Quick Add (⌘N):** Minimal modal — front, back, deck picker. AI silently suggests back (accept with Tab, ignore with Enter). 3-second flow.
- **Ask AI (during review):** Inline panel slides up below card. Streams Socratic response via SSE. One-tap "Save insight to note."
- **Card editing:** Inline edit without leaving review.
- **Post-session summary:** Cards reviewed, accuracy, time, concepts improved, streak update.

## Card Generation Pipeline

### Generation flows

All in `feature-learning::card_generator`:

1. **"Generate from note"** (Notes page toolbar)
   - Fetch note body + BookRAG tree nodes for context
   - LLM extracts key concepts, generates cards of appropriate types
   - User sees preview: approve/edit/dismiss each card, pick deck
   - Approved cards → `FlashcardRepo::create_batch()`

2. **"Generate from selection"** (highlight in editor → context menu)
   - Selected text + surrounding paragraph as context
   - LLM generates 1-3 cards from selection
   - Inline preview below editor

3. **"Generate from sentence pairs"** (Translation split-pane mode)
   - Aligned paragraph pairs (source + translation)
   - LLM extracts vocabulary cards: word, reading, meaning, example = the sentence

4. **"Generate from Cornell Q&A"** (Cornell split-pane mode)
   - Cue questions (left) + answers (right) + summary
   - Direct mapping: each Q&A pair → Basic card, summary → Cloze cards
   - Minimal AI needed — mostly structural extraction

5. **"Quick Generate"** from `/learn` dashboard
   - "From note..." → note picker → flow 1
   - "From clipboard..." → paste text → LLM generates cards (no source link)
   - "From last chat..." → pulls recent agent conversation → extracts learnable facts

### Generation prompt strategy

The LLM receives:
- Note content (or selection)
- User's existing cards on this topic (via new `FlashcardRepo::list_by_source_note(note_id)` method — needs to be added)
- Cognitive facts about user's knowledge level (via `SemanticFactRepo`)
- Instruction to vary card types: cloze for definitions, vocab for foreign words, basic for concepts, typed for terms to spell

Avoids duplicates and adapts difficulty to existing knowledge.

### Source linking

Every generated card stores:
- `source_note_id` → the note it came from
- `source_context` → surrounding paragraph (snapshot at generation time)

Enables bidirectional navigation: card → source note, note → card stats.

## Split-Pane Editor

### Three modes (toolbar toggle in note editor)

Default remains single-pane. Mode persisted per note in `split_mode` column.

**Translation Mode:**
- Source language (left), translation (right)
- Synced scrolling, paragraph-level alignment
- AI actions: "Auto-translate paragraph", "Generate cards from pairs"

**Annotation Mode:**
- Main content wider (left, 2/3), personal annotations narrower (right, 1/3)
- Highlight text on left → creates linked annotation on right
- AI actions: "Summarize annotations"

**Cornell Method:**
- Cues/questions (left), detailed notes (right), summary (bottom)
- AI actions: "Generate cues from notes", "Cards from Q&A pairs"

### Implementation

```
notes/components/editor/
  ├── SplitEditor.tsx          # Orchestrates left/right panes
  ├── TranslationMode.tsx      # Source + translation, synced scroll
  ├── AnnotationMode.tsx       # Content (2/3) + annotations (1/3)
  ├── CornellMode.tsx          # Cues + notes + summary footer
  └── SplitToolbar.tsx         # Mode toggle + mode-specific actions
```

### Dual-editor architecture

Each pane is its own TipTap editor instance. Key behaviors:

- **Save lifecycle:** `SplitEditor` wraps both editors and owns a single debounced save (1s). On save, it serializes both panes into `split_content` JSON, concatenates them into `body`/`body_html` for FTS5/BookRAG, and calls `note_update` once. Both panes are always saved atomically.
- **Undo/redo:** Per-pane. Each TipTap instance has its own history stack. No cross-pane undo.
- **Synced scrolling (Translation mode):** Paragraph-level alignment using a scroll ratio approach — `onScroll` on either pane computes `scrollTop / scrollHeight` and applies the same ratio to the sibling pane. Not line-level precision, but good enough for paragraph-aligned content. Other modes do not sync scroll.
- **Resize handle:** Same imperative `pointermove` + `requestAnimationFrame` pattern used in `KnowledgeBasePage.tsx` for the three-panel layout. Includes `resizing` class for glass-filter suppression during drag.

## AI Tutor Engine

### Orchestrator skill

New skill at `skills/tutor/SKILL.md`:

```yaml
---
name: tutor
type: orchestrator
description: AI tutor for personalized learning sessions
triggers: [teach me, quiz me, explain this, study session, help me understand, practice, drill]
tools: [notes, memory, tasks]
mcp_tools: []
can_delegate_to: [general]
max_iterations: 15
---
```

### Three modes of operation

**1. Study Plan Generation:**
- Query FlashcardRepo (due cards by deck + concept), concept_mastery (weakest), review_log (failure patterns)
- LLM generates structured `StudyPlan { sections: [{type, cards, duration_estimate, rationale}] }`
- Plan stored in `study_sessions.plan`

**2. Socratic Dialogue:**
- Context assembly: card + source note paragraph + BookRAG retrieval + cognitive facts + past exchanges
- LLM with Socratic system prompt: guides with questions, doesn't give direct answers
- Streams via SSE (reuses `useInsightSSE` pattern)
- Exchange logged to `tutor_exchanges`

**3. Mid-Session Adaptation:**
- Runs every N cards (default 5)
- If concept accuracy < 60%: generate 2-3 extra cards, insert into queue, update plan
- If concept accuracy > 90%: skip remaining easy cards, advance to next section
- Adaptation is silent — user sees plan update in sidebar

### Misconception tracking

**Concept normalization:** Concepts are derived from flashcard tags and deck names. Tags are the canonical concept identifier (lowercased, hyphenated: `te-form`, `n3-kanji`). The LLM-based misconception detector groups by tag, not free-text — avoiding normalization ambiguity.

After each session, background analysis:
- Group failed cards by tag (= concept)
- If same concept failed 3+ times across sessions: LLM analyzes patterns
- Generates `error_pattern` (e.g., "Confuses て-form with た-form in conditionals")
- Stored in `concept_mastery.error_patterns`
- Next study plan prioritizes with targeted drill

### Integration with agent runtime

Uses existing infrastructure — no custom wiring:
- `AgentRuntime::process_message` with tutor skill active
- `SkillRouter` selects tutor based on triggers + context
- `ExecutionRouter` uses Reactive mode
- Context assembly includes card, source note, cognitive facts, past exchanges
- Gets BookRAG grounding and memory retrieval automatically

## Feedback Arrow (Learn → Notes)

### Active feedback (user-controlled)

**"Save insight to note"** (one-tap during Socratic dialogue):
- Appends to source note's annotation column (if Annotation/Cornell mode) or as new paragraph (single-pane)
- Format: `[3/18 — AI Tutor] {insight summary}`

### Passive feedback (always-on)

**Note review badge** (bottom of note editor):
- "12 cards · 89% retention · reviewed 1d ago"
- From `note_review_stats` table, updated after each review session
- Clicking opens mini-panel: per-card retention, worst cards, link to review this note's cards

### Card staleness

When a source note is significantly edited (`NoteContentChanged` event):
- Check if linked cards' `source_context` still matches
- If diverged: flag cards with notification in `/learn`: "Source note was updated — review these 3 cards?"

## Analytics

### Dashboard metrics

```
Top bar:       streak | due today | retention % | cards this week
Decks:         per-deck due count, retention %, mature/young ratio, trend arrow
Charts:        7-day retention curve (recharts), 365-day review heatmap
Weak areas:    concept mastery lowest-ranked items
```

### Post-session summary

Cards reviewed, correct, again count. Time spent. Concepts improved (mastery_level deltas). Streak update. Weakest card highlight with suggested action.

### Sentence Mining

For language learners — "immersion to flashcard" pipeline:

- Input: raw foreign text (pasted, clipboard, or from a note)
- LLM segments sentences, identifies vocabulary, assesses difficulty vs known vocab
- User previews: sentence list with extracted vocab highlighted
- Approved cards created as `card_type = 'vocabulary'` with source linking

Accessible from:
- `/learn` dashboard → Quick Generate → "From clipboard..."
- Notes → Translation mode → "Mine vocabulary" button
- Quick Add (⌘N) → paste text → auto-detects foreign language → offers mining

## Interconnection Map

```
Connection 1: Notes → Learn (generate cards)
  • "Generate Flashcards" button on note toolbar
  • "Generate from selection" in editor context menu
  • "Cards from pairs" in Translation mode
  • "Cards from Q&A" in Cornell mode
  → all flow through card_generator → FlashcardRepo
  → cards carry source_note_id + source_context

Connection 2: Learn → Notes (jump to source)
  • "Source note" button on every card during review
  • Tutor dialogue references note content via BookRAG
  → navigates to /notes?id={source_note_id}

Connection 3: Notes → Learn (content feeds learning)
  • BookRAG indexes all note content
  • Tutor study plans query note topics for drill material
  • Sentence mining from note content in Translation mode
  • NoteContentChanged event → triggers card staleness check

Connection 4: Learn → Notes (feedback arrow)
  • "Save insight to note" during Socratic dialogue
  • Note review badge: "12 cards · 89% · 1d ago"
  • Card staleness notification on source note edits

Connection 5: Both → Cognitive Memory (bidirectional)
  • Card reviews update SemanticFact stability
  • Cognitive facts inform card generation difficulty
  • Tutor uses cognitive facts for personalized explanations
  • Misconception patterns stored as cognitive annotations

Connection 6: Tray integration
  • Tray countdown: "📚 23 cards due" — lowest priority, only shown when
    no focus session active AND no calendar event within 30 min AND no task deadline.
    Existing tray_countdown.rs priority order: focus timer > calendar > task deadline > flashcard due count.
  • Focus session integration: study sessions register as focus via FOCUS_ACTIVE flag
```

## Implementation Phases

### MVP (Phases 1-4): Functional learning loop

The MVP delivers a working "write notes → generate cards → review cards" loop with the new `/learn` page. Each phase gets its own implementation plan.

| Phase | What | Depends On | Scope |
|-------|------|------------|-------|
| **1** | FSRS-5 engine upgrade + card types + review_log | — | `cognitive` crate |
| **2** | `/learn` page: dashboard + immersive review | Phase 1 | New UI + `app-core` handlers |
| **3** | Card generation pipeline + source linking | Phase 1 | `feature-learning` crate + notes toolbar |
| **4** | Split-pane editor (3 modes) | — (parallel with 2-3) | `desktop-ui` notes feature |

Phases 1 and 4 can run in parallel. Phases 2 and 3 can partially overlap.

### Post-MVP (Phases 5-10): Intelligence layer

These phases add AI tutoring, analytics, and polish. Each phase gets its own implementation plan after MVP is stable.

| Phase | What | Depends On | Scope |
|-------|------|------------|-------|
| **5** | AI Tutor skill + Socratic dialogue | Phases 1-3 | `feature-learning` + new skill |
| **6** | Study planner + mid-session adaptation | Phase 5 | `feature-learning` tutor extension |
| **7** | Analytics dashboard | Phases 1-2 | `feature-learning` analytics + UI |
| **8** | Sentence mining + language extensions | Phase 3 | `feature-learning` sentence_miner |
| **9** | Feedback arrow (Learn → Notes) | Phases 2-3 | Cross-feature wiring |
| **10** | Tray integration + coaching hooks | Phases 1-2 | `desktop` crate |
