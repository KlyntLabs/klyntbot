# Interactive Translation Practice — Design Spec

## Goal

Transform the translation system from passive LLM-generated output into an active, sentence-by-sentence translation practice workspace where users build a complete translated document while receiving real-time coaching evaluation. Every practice session feeds the knowledge graph (atoms, flashcards, coaching, spaced repetition).

## Architecture

Practice Mode is a proper `splitMode="practice"` value inside `SplitEditor`. The left pane renders `PracticeSourcePanel` (read-only source text with highlighting) and the right pane renders `PracticeDocPanel` (display-only growing document) instead of TipTap editors. `SplitEditor`'s content persistence logic skips saving for practice mode (no `SplitContentStore` entry needed — practice state lives in `practice_sessions` table). This reuses the existing split-pane infrastructure (resize handle, synced scrolling) and keeps the codebase consistent with other modes. The bottom bar (`PracticeBottomBar`) renders below the split panes as a full-width zone. No new toolbar tab — users enter via (1) a Quick Translate text-selection popup, (2) a footer button in the Translate panel, or (3) a keyboard shortcut.

## Phasing

- **Phase 1** (this spec): Practice Mode workspace + Quick Translate popup + basic atom/flashcard/coaching integration + simple in-session coaching nudge
- **Phase 2** (future): Deep coaching layer — cross-session error pattern detection, difficulty auto-tuning, translation mastery insight tab, cross-note semantic dedup via LanceDB

---

## 1. Quick Translate Popup

### Trigger
User selects text in any note body. After a 300ms debounce, a compact glassmorphism popup appears near the selection.

### Content
- **Translation**: LLM-translated text (3 lines max)
- **Vocabulary chips**: Auto-extracted words with reading (pinyin/romaji/IPA), meaning, proficiency level (HSK/JLPT/CEFR), and "new" badge if not in user's vocabulary
- **Two action buttons**:
  - "Save words" — saves highlighted vocab to atoms + flashcards via existing `language_save_vocabulary` pipeline
  - "Practice this note →" — **hero button** (purple glow). Dismisses popup, enters Practice Mode with the selected sentence as starting unit

### Dismiss
Click outside, Escape, or start typing. No explicit close button (macOS native feel).

### Backend
New IPC command `language_quick_translate`:
- Input: `{ text, sourceLang, targetLang }`
- Output: `QuickTranslateResponse { translation: String, words: Vec<WordBreakdown> }`
- New dedicated prompt (`quick_translate_prompt`) that requests only translation + word breakdown (no grammar patterns). Simpler than the full `translate_breakdown_prompt` — faster response, lower token cost. The UI enforces the 3-line-max display by truncating, not the prompt.

### Frontend
- `QuickTranslatePopup.tsx` — floating component, positioned near text selection
- `useQuickTranslate.ts` — selection detection (300ms debounce), LLM call, popup positioning

---

## 2. Practice Mode — Entry Points

Three ways to enter:

1. **Quick Translate popup → "Practice this note →"** (primary, 80% of users)
   - Selected sentence becomes starting unit
   - Smart Segmentation runs during preview overlay

2. **Translate panel footer → "Turn this into active practice"**
   - For users already in Translate mode
   - Same preview flow

3. **Keyboard shortcut: Cmd+Option+P**
   - Power user path, skips popup

All entry points set `splitMode="practice"` on `SplitEditor`. The selected sentence index (if entering from popup) is passed as `startIndex` prop to `PracticeMode`.

---

## 3. Smart Segmentation

### When
Called once when entering Practice Mode. Checks note metadata cache first; if stale or missing, calls LLM.

### LLM Prompt
```
You are Klyntbot's patient language coach preparing a deliberate practice session.

Split the following note into the smallest meaningful translation units.
Rules (never break them):
- One full sentence = one unit
- Headings and titles = one unit
- Grammar patterns + their examples stay together as one unit
- Cultural explanations stay together as one unit
- Skip any line that is already in the target language or purely romanized
- Prioritise units that build on each other for natural flow

Return ONLY this exact JSON (no extra text):
[{
  "index": 0,
  "text": "exact original text",
  "type": "heading | sentence | pattern | cultural",
  "suggested_focus": "vocabulary | grammar | naturalness | cultural"
}]
```

### Preview Overlay
Non-modal floating overlay (note visible behind, blurred):
- Header: "Your Personal Language Gym — 9 units · ~11 min"
- Focus summary derived from `suggested_focus` distribution
- Scrollable unit list (numbered, with type badges)
- User can: tap to merge/split/skip units, toggle "Include romanized examples"
- "Edit segments" (secondary) + "Start Practice" (hero button)
- **On return visits** (cached segments): collapsed banner — "9 units · 11 min · Edit segments" link. Instant start.

### Caching
After first completion, accepted segments are saved as note metadata:
```json
{ "practice_segments": [...], "lang_pair": "zh-en", "cached_at": "2026-03-22T..." }
```
Future sessions use cached segments (instant start). User can force re-segmentation via "Edit segments".

### Backend
New IPC command `practice_segment_note`:
- Input: `{ noteId, sourceLang, targetLang }`
- Output: `{ segments, estimatedMins, cachedAt? }`
- Checks note `perspective_config` metadata cache first, falls back to LLM

---

## 4. Practice Workspace Layout

### Three-Zone Architecture

```
┌──────────────────────────────────────────────────┐
│ PracticeProgressHeader (single thin bar)         │
│ Focus: naturalness · 3/9 · 87% · 🔥3 · [Exit]  │
├───────────────────────┬──────────────────────────┤
│ LEFT: Source Panel    │ RIGHT: Translation Doc   │
│                       │                          │
│ • Purple highlight    │ • Clean growing document │
│   on current unit     │ • Grade badges (A, B+)   │
│ • Completed units dim │ • Clickable badges →     │
│   with strikethrough  │   shows eval history     │
│ • Synced scroll       │                          │
├───────────────────────┴──────────────────────────┤
│ BOTTOM BAR (full width, two states)              │
│                                                  │
│ State 1 — INPUT:                                 │
│   Purple prompt: "我学到的关键短语有："          │
│   Large textarea + Enter = Submit                │
│                                                  │
│ State 2 — EVALUATION:                            │
│   Grade (B+) + score badges + corrections        │
│   Encouragement line (italic, purple)            │
│   Confidence tap (1-5 stars, pre-filled 4)       │
│   [Edit my translation] [Got it — Next ⏎]        │
└──────────────────────────────────────────────────┘
```

### PracticeProgressHeader
Single thin bar above both panes — one glance, zero clutter:
- Current unit's `suggested_focus` ("Focus: naturalness")
- Live progress ("Sentence 3/9 · 87% mastery so far") with thin progress fill behind the text
- Practice streak flame
- "Exit & Save Progress" pill (also Cmd+Escape) — saves session state, returns to normal note view, drops "Resume practice" badge on note in library

### Left Panel — PracticeSourcePanel
- Source text with purple highlight + left border on current unit
- Completed units: dimmed with strikethrough
- Future units: visible but subdued
- Synced scrolling with right panel

### Right Panel — PracticeDocPanel
- **Pure display** — no form elements, no input
- Growing document: user's translations appear sequentially as they complete units
- Each line has a small clickable grade badge (A, B+, etc.) → clicking scrolls left panel to that unit and shows full eval history
- Placeholder text ("Waiting for your translation...") for the current unit
- Exportable as markdown (feeds "Save as new note" on completion)

### Bottom Bar — PracticeBottomBar
Two mutually exclusive states, zero layout shift:

**Input State:**
- Purple prompt showing the source text to translate
- Large textarea (expandable on focus)
- Enter key submits
- Calls `practice_submit_unit` IPC → LLM evaluation

**Eval State** (replaces input after submission):
- Large grade (B+) + per-dimension score badges (Meaning A, Grammar B+, Natural B, Choice A)
- Corrections list: ~~original~~ → suggested + explanation
- Model translation (subtle, collapsible)
- Encouragement line: warm, short, personal (from LLM `encouragement` field)
- Improvement hint (optional, from LLM `improvement_hint` field)
- **Confidence tap**: 1-5 stars, pre-filled at 4. One-tap. Feeds FSRS calibration + overconfidence detection.
- **In-session coaching nudge** (Phase 1): If 3+ consecutive units score B+ or lower on the same dimension, one extra line appears referencing the unit's `suggested_focus` from segmentation: "You've hit naturalness B+ three times in restaurant contexts — want a 20-second micro-tip?" The focus context makes the coach feel smarter than a generic pattern detector.
- Two action buttons:
  - "Edit my translation" (secondary) — returns to Input State with user's text pre-filled. Edited version becomes `final_translation`.
  - "Got it — Next" (hero, Enter key) — confirms unit, advances to next. Triggers `practice_confirm_unit` IPC.

### Micro-toast
On "Got it — Next", a 1.5s toast appears bottom-right: "Atom saved · This unit now lives in your knowledge graph" (only for units where atom was created, i.e., grade ≤ A-).

---

## 5. Session Completion

When all units are done, `PracticeSessionComplete` replaces the workspace:

### Content
- **"I did this" moment**: Before showing the score, the right panel expands to full width displaying the complete user translation document. A 3-second admiration pause with a subtle fade-in of "You translated this." at the top. Then the score overlay slides in.
- **Score**: Large percentage (87%) + "9/9 units · 12 minutes · 4-day streak"
- **Per-dimension averages**: Meaning A-, Grammar B+, Natural B, Choice A
- **Weak units summary**: "3 units need review" with unit numbers + weak dimension
- **Actions**:
  - "View My Full Translation" — expands the translated document full-width again (the pride moment, on demand)
  - "Save to Spaced Repetition (3 cards)" — **hero button**. Creates flashcards for weak units (grade ≤ A-). Each card: front = source sentence, back = model translation + corrections. Initial FSRS stability calibrated from grade.
  - "Save as new note" (secondary) — creates a new note from `user_translation_doc`, auto-tagged with language pair + difficulty
  - "Review with Coach" (secondary) — opens coaching tab with session pattern insights
  - "Close" (tertiary)

### Backend
Calls `practice_complete_session` IPC:
- Computes `average_score`
- Builds `user_translation_doc` (concatenated final translations)
- Emits `PracticeSessionCompleted` domain event
- If `saveToSR=true`, batch-creates flashcards + atoms for weak units

---

## 6. Evaluation LLM Prompt

Called per unit via `practice_submit_unit`. Upgrades the existing `evaluate_translation_prompt`.

```
You are Klyntbot's patient, encouraging language coach — supportive but honest,
like a kind tutor who celebrates progress.

Current unit: {source_sentence}
User's translation: {user_input}
Full note context: {entire_source_text}
Previous unit result (for continuity): {last_unit_grade_and_correction_summary}

Note: Confidence rating is collected AFTER evaluation (in the eval card UI), so it is NOT
available at eval time. It feeds FSRS and coaching, not the LLM prompt.

Evaluate with document-level awareness and the user's personal history.
Be specific: explain WHY something is better, not just what.

Return ONLY this exact JSON:
{
  "overall_grade": "A+ | A | A- | B+ | B | B- | C+ | C | C- | D+ | D | F",
  "scores": {
    "meaning": "grade",
    "grammar": "grade",
    "naturalness": "grade",
    "wordChoice": "grade"
  },
  "corrections": [
    {
      "original": "exact phrase from user",
      "suggested": "better version",
      "explanation": "why this is more natural or correct in this context"
    }
  ],
  "model_translation": "polished version of the sentence",
  "encouragement": "warm, short, personal comment referencing progress (max 15 words)",
  "improvement_hint": "one micro-tip for next time (optional, null if grade >= A)"
}
```

---

## 7. Data Model

### Migration ownership
The `practice_sessions` table migration lives in `crates/feature-notes/` since practice is note-scoped. Registered via `FeatureMigration` in the feature-notes `FeaturePackage`. This follows the pattern of other note-adjacent tables.

### New table: `practice_sessions`

```sql
CREATE TABLE practice_sessions (
    id                   TEXT PRIMARY KEY,
    note_id              TEXT NOT NULL REFERENCES notes(id),
    source_lang          TEXT NOT NULL,
    target_lang          TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'in_progress',  -- in_progress | completed | abandoned
    segments             TEXT NOT NULL,       -- JSON: Smart Segmentation output
    current_index        INTEGER NOT NULL DEFAULT 0,
    results              TEXT NOT NULL DEFAULT '[]',  -- JSON: per-unit results array
    user_translation_doc TEXT,                -- complete built document (right panel)
    average_score        REAL,
    started_at           TEXT NOT NULL,
    completed_at         TEXT,
    created_at           TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Segments JSON
```json
[
  { "index": 0, "text": "在餐厅点餐", "type": "heading", "suggested_focus": "vocabulary", "skipped": false }
]
```

### Per-unit result JSON
```json
{
  "index": 1,
  "user_translation": "Today I practiced ordering food in Japanese.",
  "final_translation": "Today I practiced ordering food using Japanese.",
  "grade": "B+",
  "scores": { "meaning": "A", "grammar": "B+", "naturalness": "B", "wordChoice": "B+" },
  "corrections": [{ "original": "...", "suggested": "...", "explanation": "..." }],
  "model_translation": "Today I practiced ordering food in Japanese.",
  "encouragement": "Great vocabulary choices!",
  "edited": true,
  "confidence_rating": 4,
  "atom_id": "atom-uuid",
  "submitted_at": "2026-03-22T10:15:45Z"
}
```

### Note metadata addition
`practice_segments` field cached in note's `perspective_config`:
```json
{ "practice_segments": [...], "lang_pair": "zh-en", "cached_at": "2026-03-22T..." }
```

---

## 8. Backend — IPC Commands

All practice commands live in a new `crates/desktop/src/commands/practice.rs` module with its own `pub const DEV_COMMANDS: &[&str]` and `dispatch_dev()` handler. The `language_quick_translate` command is added to the existing `commands/language.rs`.

| Command | Input | Output | Purpose |
|---------|-------|--------|---------|
| `language_quick_translate` | `{ text, sourceLang, targetLang }` | `QuickTranslateResponse` | Lightweight popup translation |
| `practice_segment_note` | `{ noteId, sourceLang, targetLang }` | `PracticeSegmentResponse` | Smart Segmentation (cache-first) |
| `practice_start_session` | `{ noteId, segments, sourceLang, targetLang, startIndex? }` | `PracticeSessionResponse` | Create session row |
| `practice_submit_unit` | `{ sessionId, index, userTranslation }` | `PracticeEvalResponse` | LLM evaluation → grades + corrections |
| `practice_confirm_unit` | `{ sessionId, index, finalTranslation, confidenceRating, edited }` | `PracticeConfirmResponse { nextIndex, isComplete }` | Advances session. Creates atom if grade ≤ A-. |
| `practice_get_session` | `{ sessionId?, noteId? }` | `PracticeSessionResponse?` | Resume: fetches latest in-progress session |
| `practice_complete_session` | `{ sessionId, saveToSR }` | `PracticeCompleteResponse` | Finalizes session, emits event, optionally creates flashcards |

IPC types live in `crates/desktop-shared/src/commands/practice.rs` (new file). `PracticeEvalResponse` is a **new type** — it does NOT modify the existing `TranslationEvalResponse` used by `PracticeSection.tsx`. This preserves backward compatibility.

### Why submit and confirm are separate
- `submit` triggers the LLM eval call (1-3s) and returns the evaluation
- `confirm` is instant (DB write + atom creation) — called when user clicks "Got it — Next" or after editing
- Separating them lets the UI show eval immediately while the next action happens on user input
- If user clicks "Edit my translation", the edited version goes through `confirm` with `edited: true` — no second LLM call

### Error handling
- **LLM call fails during `practice_submit_unit`**: Return error to frontend. Bottom bar shows "Evaluation failed — Retry?" button. User can retry the same unit without losing their translation text.
- **`practice_confirm_unit` fails after eval was shown**: Retry silently on next "Got it — Next" press. Session state is consistent because `current_index` only advances on successful confirm.
- **Atom/flashcard creation fails**: Log warning, don't block the practice flow. Atom creation is best-effort; the session result is saved regardless.
- **LLM call fails during `practice_segment_note`**: Show error in preview overlay with "Retry" button. User can also dismiss and return to normal note view.

---

## 9. Integration with Existing Systems

### Knowledge Atoms
Each confirmed unit with grade ≤ A- creates a `KnowledgeAtom` (i.e., only A+ and A are considered "mastered" — A- and below trigger atom creation):
- `atom_type`: `"translation_unit"` (new type alongside `"vocabulary"`)
- `domain`: `"language:{pair}"` (e.g., `"language:zh-en"`)
- `subject`: source text
- `source_context`: user's final translation
- `metadata`: `{ grade, scores, corrections, confidence_rating, session_id }`
- Embedded in LanceDB via existing pipeline → searchable across notes

### Flashcards
On "Save to Spaced Repetition" (Session Complete):
- Each weak unit (grade < A) → flashcard
- `front`: source sentence, `back`: model translation + user's corrections
- Initial FSRS stability calibrated from grade: A- → 2.0, B+ → 1.5, B → 1.0, C → 0.5
- Linked to corresponding atom via `atom_id`
- `edited` flag lowers initial stability slightly (user needed correction)

### Coaching
- `PracticeUnitCompleted` event emitted after each `practice_confirm_unit`
- `PracticeSessionCompleted` event emitted after `practice_complete_session`
- **Phase 1 in-session nudge**: Current session's results array checked for 3+ consecutive low scores on same dimension → coaching line appears in eval card
- Phase 2: Cross-session pattern detection, difficulty auto-tuning

### Practice History (new tab in InsightReviewPanel)
New **"Practice"** tab added to the Learn panel's tab bar (alongside Atoms, Synthesis, Gaps, etc.):
- `TabId = "practice"` added to `useInsightReview.ts`
- Vertical timeline of past sessions for this note
- Each card: date, streak, units completed, average score, mini preview snippet
- Clickable score → opens session detail with grade badges
- Weak units get "Practice again" chip → re-enters Practice Mode with only that unit
- Empty state: "No practice sessions yet. Select text and tap 'Practice this note' to start."

### Semantic Facts
Each practice unit creates a `SemanticFact`:
- Domain: `"translation_practice"`
- Subject: source text, Predicate: `"translates_to"`, Object: user's final translation
- Confidence from grade mapping (A=1.0, B+=0.85, B=0.7, C=0.5, D=0.3, F=0.1)
- Enables cross-note similarity search (Phase 2)

### Domain Events

New variants added to `DomainEvent` enum in `crates/bus/src/domain_events.rs`:

| Event | Fields | When | Subscribers |
|-------|--------|------|------------|
| `PracticeUnitCompleted` | `{ session_id, note_id, unit_index, grade, scores, confidence_rating, edited }` | After `practice_confirm_unit` | Coaching (in-session pattern check) |
| `PracticeSessionCompleted` | `{ session_id, note_id, units_completed, average_score, source_lang, target_lang, weak_unit_count }` | After `practice_complete_session` | Coaching, Insight panel |
| `KnowledgeAtomCreated` | (existing) | When atom created from weak unit | Existing pipeline (coaching, insights, morning briefing) |

---

## 10. Frontend Components

### New files (`desktop-ui/src/features/notes/`)

```
components/practice/
  PracticeMode.tsx              — Main container, orchestrates layout + session state
  PracticePreview.tsx           — Floating overlay (segmentation preview, start button)
  PracticeProgressHeader.tsx    — Single thin bar: focus, progress, streak, exit pill
  PracticeSourcePanel.tsx       — Left pane: source text with progressive highlighting
  PracticeDocPanel.tsx          — Right pane: clean growing document, clickable grades
  PracticeBottomBar.tsx         — Two-state bar: InputState ↔ EvalState
  PracticeSessionComplete.tsx   — Pride moment + results + "Save to SR" hero + secondary actions
  ConfidenceTap.tsx             — 1-5 stars widget

components/QuickTranslatePopup.tsx — Floating glass panel on text selection

hooks/
  usePracticeSession.ts         — Session CRUD (create, resume, update, complete)
  useSmartSegmentation.ts       — Segmentation + note metadata cache
  usePracticeEvaluation.ts      — Per-unit LLM eval, builds results array
  useQuickTranslate.ts          — Text selection detection + lightweight translation
```

### Modified files

| File | Change |
|------|--------|
| `NoteEditor.tsx` | Add text selection listener → `QuickTranslatePopup` |
| `SplitEditor.tsx` | Add `"practice"` to `SplitMode` type. When `splitMode="practice"`, render `PracticeMode` instead of TipTap editors. Skip `SplitContentStore` persistence for practice mode. |
| `LanguageLearningPanel.tsx` | Add "Turn this into active practice" footer button (calls `onEnterPractice` prop → sets `splitMode="practice"`) |
| `InsightReviewPanel.tsx` | Add "Practice" tab (`TabId = "practice"`) rendering `PracticeHistoryTab` |
| `useInsightReview.ts` | Add `"practice"` to `TabId` union |

### Component hierarchy

```
NoteEditor
├── [text selection] → QuickTranslatePopup (floating)
└── SplitEditor (splitMode="practice")
    └── PracticeMode
        ├── PracticePreview (floating overlay, first visit)
        ├── PracticeProgressHeader (single bar: focus, progress, streak, exit)
        ├── PracticeSourcePanel (left pane, synced scroll via SplitEditor)
        ├── PracticeDocPanel (right pane, synced scroll via SplitEditor)
        ├── PracticeBottomBar (full-width)
        │   └── ConfidenceTap (inside eval state)
        └── PracticeSessionComplete (replaces workspace on finish)
```

---

## 11. Session Persistence & Resume

- Sessions saved to `practice_sessions` table on every `practice_confirm_unit`
- `current_index` tracks resume point
- "Exit & Save Progress" (Cmd+Escape) sets status to `in_progress` and exits
- Note library shows "Resume practice (3/9)" badge on notes with active sessions
- Re-entering Practice Mode checks for in-progress session via `practice_get_session({ noteId })`
- If found: preview overlay collapses to banner ("Resume 3/9 · 87% last time") with instant start
- Completed sessions remain queryable for Practice History
- **Abandoned sessions**: Sessions with status `in_progress` that haven't been updated in 7+ days are auto-marked `abandoned` on next `practice_get_session` call. They still appear in Practice History (dimmed) but don't trigger "Resume" prompts. No background job — cleanup happens lazily on access.

---

## 12. Keyboard Flow

| Key | Context | Action |
|-----|---------|--------|
| Enter | Input state | Submit translation for evaluation |
| Enter | Eval state | Confirm ("Got it — Next") → advance to next unit |
| Cmd+Option+P | Note editor | Enter Practice Mode |
| Cmd+Escape | Practice Mode | Exit & Save Progress |
| Escape | Quick Translate popup | Dismiss popup |
| 1-5 | Eval state (confidence tap focused) | Set confidence rating |

---

## Non-goals (Phase 1)

- Cross-note semantic dedup of practice units (Phase 2, needs data)
- Difficulty auto-tuning based on accuracy trends (Phase 2)
- Translation mastery insight tab with confidence heat-maps (Phase 2)
- In-practice coaching nudges beyond the simple 3-consecutive-low-score check (Phase 2)
- Mobile/tablet stacked layout (future, desktop-first)
- Real-time collaborative practice (not planned)
