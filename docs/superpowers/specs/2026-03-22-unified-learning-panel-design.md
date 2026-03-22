# Unified Learning Panel

**Date:** 2026-03-22
**Status:** Draft
**Scope:** Merge the Knowledge Atoms panel and Insight Review panel into a single "Learn" panel in the notes right-side context area, connecting gap analysis to atom creation, insight flashcards to atom retention, and quiz scores to FSRS retention tracking.

## Problem

The notes page has two separate AI-powered learning systems that don't know about each other:

1. **Knowledge Atoms panel** — auto-extracts concepts from note content, shows below the editor. Has accept/dismiss, inline review, retention tracking via FSRS-5, coaching integration.
2. **Insight Review panel** — on-demand LLM analysis with 5 tabs (Synthesis, Gap Analysis, Self-Assessment, Concept Map, Perspectives). Shows in the right panel, replacing all other context.

Users encounter confusion:
- "10 new facts" badge in Insight Review counts SemanticFacts from conversation reflection, not note-related atoms
- Gap Analysis identifies missing concepts but never creates learning items
- Insight-generated flashcards have `atom_id = NULL` — invisible to Knowledge Health, coaching, and retention tracking
- Quiz scores in Self-Assessment don't update atom retention despite testing the same concepts
- Two separate panels compete for attention with no cross-references

## Solution

Merge both into a single **"Learn" panel** in the right-side context area with atoms as the default landing tab, and wire the insight analysis pipeline into the atom lifecycle.

## 1. Panel Structure

### Tab layout

```
[ Atoms ] [ Synthesis ] [ Gaps ] [ Quiz ] [ Map ] [ Perspectives ]
```

- **Atoms** — default landing tab, loads instantly from cached `useQuery`
- **Synthesis through Perspectives** — existing Insight Review tabs, unchanged behavior
- Tab status dots: filled green = content generated, empty = not yet generated, pulsing = generating

### Panel identity

- The button in `AISuggestionsPanel` changes from "Insight Review" to **"Learn"**
- Panel header changes from "Insight Review" to **"Learn"** with the same Brain icon
- Same panel mechanics: replaces right panel context, wider width (65% clamped 360-640px), no resize handle
- Scope selector, Squad picker, History toggle — all remain, positioned in the header

### What moves

- `KnowledgeAtomsPanel` is removed from `NoteEditor` (below-editor position)
- Its content (active atoms, suggested atoms, bulk accept, inline review) moves into the new `AtomsTab` component inside the unified panel
- The `KnowledgeGrowthMetrics` "N new facts" bar is replaced with a note-level atom summary: "N atoms · M suggested" using data already available from the Atoms tab query

## 2. Atoms Tab (Default)

### Layout

```
┌─────────────────────────────────────────┐
│ ACTIVE ATOMS (N)                        │
│ ┌─────────────────────────────────────┐ │
│ │ Rust Ownership Rules     100%  Review│ │
│ │ Borrowing Rules          100%  Review│ │
│ │ ...                                  │ │
│ └─────────────────────────────────────┘ │
│                                         │
│ SUGGESTED (M)              Accept all ▸ │
│ ┌─────────────────────────────────────┐ │
│ │ ⚡ Error Handling        From gaps  +×│ │
│ │ ⚡ Lifetime Elision                 +×│ │
│ └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

- Active atoms: subject, retention %, Review button (inline quick review)
- Suggested atoms: subject, source badge ("From gaps" / "Auto-extracted"), Accept (+) / Dismiss (×)
- "Accept all" opens the existing `BulkAcceptModal` with importance picker
- Inline quick review: same behavior as today — card expands in-place, rating buttons, collapses after rating
- `data-atom-id` attributes for deep-link scroll targeting (existing `atomId` URL param)

### Data source

`useQuery("atoms_for_note", { noteId })` — same IPC command, same 30s SWR cache. No new backend work.

### Live updates while editing

- Auto-extraction fires on `NoteContentChanged` with 5s debounce (existing behavior)
- New suggested atoms appear at the bottom of the Suggested section via `useQuery` invalidation
- No jarring full-refresh — atoms list is additive
- Active atoms are never modified by extraction (user-accepted = stable)

## 3. Gap Analysis → Atom Pipeline

### Flow

1. User generates Gap Analysis tab (existing on-demand flow)
2. Gap Analysis LLM response returns markdown + trailing JSON block with gap items
3. After storing the insight, the backend parses the JSON gaps
4. For each gap item:
   - Check if a matching active/suggested atom already exists (case-insensitive subject match on the same note)
   - If no match: create a `knowledge_atoms` row with:
     - `status = 'suggested'`
     - `atom_type = 'concept'`
     - `domain` = inferred from note domain or gap context
     - `source_note_id` = the note being analyzed
     - `source_context` = the gap description text
     - `metadata = '{"source": "gap_analysis", "insight_review_id": "<id>"}'`
   - Emit `KnowledgeAtomCreated` domain event
5. Frontend: `useQuery("atoms_for_note")` invalidates → new suggested atoms appear in Atoms tab with "From gaps" badge

### Badge display

Atoms with `metadata.source === "gap_analysis"` render a small "From gaps" pill badge in the Atoms tab to distinguish them from auto-extracted atoms. The badge is display-only — accept/dismiss behavior is identical.

## 4. Insight Flashcards → Atom Linking

### Current behavior

`insight_save_flashcards` in `crates/app-core/src/handlers/notes/insight.rs` creates flashcards from Self-Assessment quiz questions with `atom_id = NULL`. These cards exist in the flashcard system but are invisible to Knowledge Health and coaching.

### New behavior

When creating flashcards from quiz questions:

1. For each flashcard being created, extract the question's primary topic/concept
2. Query `knowledge_atoms` for active atoms on the same note with subject keyword overlap (case-insensitive substring match)
3. If a matching atom is found: set `flashcard.atom_id = atom.id`
4. If no match: keep `atom_id = NULL` (no regression)

This means insight-generated flashcards participate in:
- FSRS retention scheduling per atom
- Knowledge Health topic aggregates
- Coaching pattern detection (AtomFlashcardReviewed events)
- Retention charts and trends

## 5. Quiz Score → Atom Retention Signal

### Flow

When `revealAll()` submits quiz results via `note_insight_submit_quiz`:

1. The backend receives the quiz score and individual question results
2. For each answered question:
   - Match the question topic to an active atom on the same note (same keyword overlap as Section 4)
   - If matched: emit `AtomInteracted { atom_id, interaction_type: "quiz_answer" }` domain event
   - If the answer was correct: update the atom's `last_interaction_ts` (refreshes salience decay timer)
   - If the answer was wrong: no retention penalty (quizzes are lower-stakes than flashcard reviews — we don't want to punish exploration)
3. The existing coaching pipeline sees the `AtomInteracted` events and factors them into pattern detection

### Why not full FSRS review?

FSRS reviews (Again/Hard/Good/Easy) are calibrated for spaced repetition flashcards with specific recall timing. Quiz questions are broader assessments — a correct quiz answer doesn't mean the same thing as a successful flashcard recall at the scheduled interval. Using `AtomInteracted` instead of `AtomFlashcardReviewed` gives the system signal without distorting the retention curve.

## 6. Context Enrichment

### CognitiveAccessor gains atom awareness

Add `search_atoms(&self, note_id: &str) -> Vec<String>` to the `CognitiveAccessor` trait. Implementation queries active atoms for the note and returns their subjects.

In `PromptBuilder::build_context()`, after the cognitive data section, add:

```
## Already Learned
The user has accepted these concepts as known: [atom subjects]
Consider these as established knowledge — don't re-explain them in the synthesis.
Focus gap analysis on what's NOT yet covered.
```

This prevents the synthesis from redundantly summarizing concepts the user has already accepted as atoms, and focuses gap analysis on genuinely missing knowledge.

## 7. Cleanup

### Remove dead prompts file

Delete `crates/feature-insights/src/prompts.rs` — unused duplicate that has diverged from the live `insight_prompts.rs`. The canonical prompts live in `crates/app-core/src/handlers/notes/insight_prompts.rs`.

### Fix "new facts" semantics

Replace `KnowledgeGrowthMetrics` (workspace-wide SemanticFacts count) with a note-level atom summary that shows "N atoms · M suggested" using data from the Atoms tab query. This aligns the displayed metric with what the user is actually looking at.

### Remove KnowledgeAtomsPanel from editor

Delete the `KnowledgeAtomsPanel` render from `NoteEditor` component. All atom interaction moves to the Atoms tab in the unified panel.

## 8. Files Affected

### New files
| File | Responsibility |
|---|---|
| `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx` | Atoms tab component (migrated from KnowledgeAtomsPanel) |

### Modified files
| File | Changes |
|---|---|
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Add Atoms tab as first tab, rename header to "Learn" |
| `desktop-ui/src/features/notes/components/ContextPanel.tsx` | Update button label from "Insight Review" to "Learn" |
| `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx` | Rename button label |
| `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx` | Remove KnowledgeAtomsPanel render |
| `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx` | Replace SemanticFacts query with note-level atom count |
| `desktop-ui/src/features/notes/components/AtomCard.tsx` | Add "From gaps" badge when `metadata.source === "gap_analysis"` |
| `crates/app-core/src/handlers/notes/insight.rs` | After gap generation: parse gaps → create suggested atoms. In `insight_save_flashcards`: match flashcards to atoms. In `note_insight_submit_quiz`: emit AtomInteracted for matched questions. |
| `crates/app-core/src/adapters/cognitive_accessor.rs` | Add `search_atoms()` implementation |
| `crates/feature-insights/src/traits.rs` | Add `search_atoms()` to CognitiveAccessor trait |
| `crates/feature-insights/src/prompt_builder.rs` | Add "Already Learned" section from atoms |

### Deleted files
| File | Reason |
|---|---|
| `crates/feature-insights/src/prompts.rs` | Dead duplicate — live prompts in `insight_prompts.rs` |
| `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx` | Replaced by `AtomsTab.tsx` in the unified panel |

## 9. What Does NOT Change

- All 5 existing Insight Review tab behaviors (Synthesis, Gaps, Quiz, Concept Map, Perspectives)
- Squad picker, Scope selector, History/Evolution panel
- Atom auto-extraction pipeline (background, debounced)
- Knowledge Health page (Topics, Trends, Graph tabs)
- Coaching integration, decay cron, morning briefing, micro-review
- Atom CRUD IPC commands (atoms_for_note, atom_accept, atom_dismiss, etc.)
- Deep-link navigation from coaching interventions
- RelevantAtoms component on project overview
- Retention history charts and APIs

## 10. Testing Strategy

- Unit tests: gap JSON parsing → atom creation, flashcard → atom matching, quiz → atom interaction
- Integration: write note → generate gaps → atoms appear in Atoms tab → accept → generate quiz → score → atom retention updated
- Frontend: Atoms tab renders same content as old KnowledgeAtomsPanel, tab switching preserves state, deep-link scroll works
- Regression: all existing Insight Review tests still pass, coaching pipeline unaffected
