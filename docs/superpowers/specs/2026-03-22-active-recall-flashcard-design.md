# Active Recall Flashcard System — Design Spec

**Date:** 2026-03-22
**Status:** Draft
**Scope:** Full production system — typed answers, LLM grading, knowledge graph propagation, multi-mode review, session intelligence

## Problem

The current flashcard review is passive: show front, reveal back, self-rate (Again/Hard/Good/Easy). This undermines FSRS-5 scheduling integrity — users over-rate themselves and the system never verifies what they actually understood. The result: inflated confidence, weaker retention, and reviews that feel like homework instead of thinking.

## Vision

Every review session becomes a thoughtful dialogue with your past self and the AI that helped you learn it. Active recall with AI verification turns the flashcard system from "good spaced repetition" into a true second-brain extension — review feels like insight-building, not testing.

## Architecture: Approach A — Grading Pipeline in `app-core`

Grading is a **user-facing handler concern** (needs LLM, embeddings, UI feedback). Scheduling is a **pure algorithm concern** (FSRS-5). These stay separate.

- `app-core` handlers own the grading pipeline (mirrors existing `practice_submit_unit` pattern)
- `cognitive` crate owns FSRS-5 scheduling (unchanged, community-compatible)
- Continuous scores (0.0–1.0) map to discrete FSRS-5 ratings (1–4) via tunable thresholds
- No new crates, no circular dependencies

---

## Section 1: Backend Grading Pipeline

### New handler: `flashcard_submit_answer`

**Location:** `crates/app-core/src/handlers/notes/flashcard.rs`

**Input:**

```rust
FlashcardSubmitAnswerParams {
    card_id: String,
    user_answer: String,
    mode: AnswerMode,  // "typed" | "voice" | "cloze_fill"
}
```

**Output:**

```rust
GradeResult {
    score: Option<f64>,                   // 0.0–1.0, None for self_grade mode
    suggested_rating: ReviewQuality,     // mapped from score
    grading_method: GradingMethod,       // "semantic_auto" | "llm" | "exact_match" | "self_grade"
    explanation: Option<String>,         // LLM explanation for borderline/wrong
    diff_highlights: Vec<DiffSegment>,   // what matched vs. missed
    expected_answer: String,             // the card's back
    coaching_nudge: Option<String>,      // optional coaching tip
    socratic_suggestion: Option<String>, // inline Socratic prompt (generated on borderline/wrong)
    key_concepts_present: Vec<String>,
    key_concepts_missing: Vec<String>,
}

struct DiffSegment {
    text: String,
    status: DiffStatus,  // "match" | "missing" | "extra" | "partial"
}

enum GradingMethod {
    ExactMatch,
    SemanticAuto,
    Llm,
    SelfGrade,
}
```

### Score-to-rating mapping

Tunable thresholds, exposed in config:

| Score     | Rating   | Meaning                    |
| --------- | -------- | -------------------------- |
| 0.85–1.0  | Easy (4) | Nailed it                  |
| 0.60–0.84 | Good (3) | Got the core, missed details |
| 0.30–0.59 | Hard (2) | Partial, significant gaps  |
| 0.00–0.29 | Again (1)| Didn't know it             |

### Grading pipeline stages

1. **Exact match** — `user_answer.trim().to_lowercase() == card.back.trim().to_lowercase()` → score = 1.0, done.

2. **Semantic pre-filter** — embed user answer via fastembed (same model used for note embeddings). Cosine similarity against the card's pre-stored back embedding.
   - ≥ 0.78 → auto-accept (score = 0.85 + scaled bonus up to 1.0)
   - ≤ 0.45 → auto-fail (score = 0.15, no LLM needed)
   - 0.45–0.78 → proceed to LLM

3. **LLM grading** (only for borderline band) — structured prompt:
   ```
   You are grading a flashcard answer.
   Question: {front}
   Expected answer: {back}
   User's answer: {user_answer}
   Source context: {source_context}

   Return JSON: {
     score: 0.0-1.0,
     explanation: "...",
     key_concepts_present: [...],
     key_concepts_missing: [...],
     coaching_nudge: "...",
     socratic_suggestion: "..."
   }
   ```

4. **Diff generation** — compare key_concepts_present/missing against expected answer to build `DiffSegment[]` for UI highlighting.

### Error handling & fallback

If LLM grading fails (rate limit, timeout, malformed JSON), fall back to pure semantic score with a gentle message: "AI grading temporarily unavailable — using semantic match instead." Still records a rating so the session never breaks.

### Card embeddings at creation time

When `flashcard_save_generated` or `flashcard_create` saves a card, spawn a background task to embed **both** `card.front` and `card.back` in LanceDB. Table: `flashcard_embeddings` keyed by `(card_id, side)` where `side` is `"front"` or `"back"`.

New column on `flashcards` table: `back_embedding_updated_at TEXT` (mirrors `notes.embedding_updated_at`).

Both-side embedding future-proofs reverse cards, "explain in your own words" mode, and bidirectional practice.

### Socratic follow-up: `flashcard_explain_answer`

Separate handler for deep-dive (the inline `socratic_suggestion` in `GradeResult` handles 80% of cases):

```rust
FlashcardExplainParams {
    card_id: String,
    user_answer: String,
    grade_result_explanation: String,
}
```

Returns coaching-style LLM response. Optionally saves the exchange as a new episodic memory via `DomainEvent::KnowledgeAtomCreated`.

### Config

Exposed in `config.json` under `learning.activeRecall` (nested under existing `LearningConfig` to avoid collision with the existing adaptive confidence fields in `learning`):

```json
{
  "learning": {
    "activeRecall": {
      "semanticAutoAcceptThreshold": 0.78,
      "semanticAutoFailThreshold": 0.45,
      "graphPropagationStrength": "gentle",
      "graphPropagationDailyCap": 15,
      "defaultAnswerMode": "auto"
    }
  }
}
```

---

## Section 2: Frontend Architecture

### Component hierarchy

```
ActiveReviewSession (layout: "compact" | "fullscreen")
├── DeckPicker                       (existing, minimal changes)
├── ReviewCard                       (main review surface)
│   ├── CardFront                    (question, card type badge, deck label)
│   ├── AnswerInput                  (per-mode input area)
│   │   ├── TypedAnswerInput         (textarea, char count, Enter to submit)
│   │   ├── ClozeInput               (inline fill-in-the-blank)
│   │   ├── MultipleChoiceInput      (4 options: 1 correct + 3 AI distractors)
│   │   ├── VoiceInput               (record button, waveform, live transcript)
│   │   └── SelfGradeInput           (current show/reveal/rate — escape hatch)
│   ├── GradeDisplay                 (shown after grading)
│   │   ├── ScoreBadge               (color-coded, "Nailed it" / "Close" / etc.)
│   │   ├── DiffHighlights           (green = present, red = missing concepts)
│   │   ├── ExpectedAnswer           (card back, always shown)
│   │   ├── SocraticSuggestion       (inline coaching nudge, collapsible)
│   │   └── PropagationRipple        (inline: "This strengthened 3 linked concepts")
│   └── GradeActions                 (confirm rating, override, explain, save, jump to source)
├── ModeSelector                     (per-deck/per-card toggle, remembers preference)
├── SocraticPanel                    (expanded deep-dive, calls flashcard_explain_answer)
├── SessionProgress                  (remaining count, progress bar, avg score)
└── SessionSummary                   (end screen: stats, narrative, reflection pulse)
```

### State management — `useActiveReview` hook

Replaces both `useFlashcards` (notes feature) and `useReviewSession` (learn feature). State machine:

```
idle → deck_picker → reviewing → complete
                       ↓
              card phases: answering → grading → graded → [socratic] → confirming → next_card
```

```typescript
interface ActiveReviewState {
  phase: "idle" | "deck_picker" | "reviewing" | "complete";
  cards: Flashcard[];
  currentIndex: number;
  cardPhase: "answering" | "grading" | "graded" | "socratic" | "confirming";
  currentAnswer: string;
  gradeResult: GradeResult | null;
  selectedMode: AnswerMode;
  modePreferences: Record<string, AnswerMode>;
  sessionStats: SessionStats;
}

type AnswerMode = "typed" | "self_grade" | "multiple_choice" | "cloze_fill" | "voice" | "auto";
```

Exposed methods: `submitAnswer(text)`, `confirmRating(quality, override?)`, `requestExplanation()`, `saveAsInsight()`, `switchMode(mode)`, `skipCard()`.

### Rendering contexts

`ActiveReviewSession` works in two layouts via a `layout` prop:

1. **Compact** (insight panel) — side-panel layout, SocraticPanel renders inline as collapsible
2. **Fullscreen** (focus mode) — triggered by button or shortcut, SocraticPanel slides in from the right

### UX transitions (hide the state machine)

- Submit → grading spinner (200ms max) dissolves into GradeDisplay
- Subtle confetti burst on scores ≥ 0.85
- SocraticSuggestion appears as inline expandable card that grows downward (no modal)
- "Explain my answer" smoothly slides SocraticPanel in fullscreen, expands inline in compact
- PropagationRipple shows on ~20% of reviews (smart sampling) to stay special

### Keyboard shortcuts

| Key           | Action                        | Context       |
| ------------- | ----------------------------- | ------------- |
| `Enter`       | Submit answer                 | Answering     |
| `Shift+Enter` | Newline in answer             | Answering     |
| `1/2/3/4`     | Override rating               | Graded        |
| `Enter`       | Confirm suggested rating      | Graded        |
| `e`           | Explain my answer             | Graded        |
| `s`           | Save as insight               | Graded        |
| `j`           | Jump to source note           | Graded        |
| `Tab`         | Toggle self-grade mode        | Any           |
| `Escape`      | Exit review                   | Any           |

### New IPC commands

| Command                         | Purpose                                       |
| ------------------------------- | --------------------------------------------- |
| `flashcard_submit_answer`       | Semantic + LLM grading pipeline               |
| `flashcard_explain_answer`      | Deep Socratic follow-up                       |
| `flashcard_generate_distractors`| AI-generated wrong options for MC mode         |
| `flashcard_save_mode_preference`| Persist deck/card mode choice                  |
| `flashcard_get_prerequisites`   | Find prerequisite cards for injection           |
| `flashcard_save_session`        | Persist review session data                     |

Existing `flashcard_record_review` stays unchanged — called after user confirms rating.

---

## Section 3: Knowledge Graph Propagation

### Fractional Implicit Repetition (FIRe)

After `flashcard_record_review` succeeds, a background task walks the knowledge graph and applies fractional stability boosts to related cards.

### Graph sources

1. **`note_links`** — wiki-links between notes. If card A's source note links to card B's source note, A and B are related.
2. **`note_entity_mentions`** — cross-entity references create edges.
3. **`knowledge_atoms`** — atoms with same domain or overlapping subject text share a conceptual neighborhood.

### Propagation algorithm

```
1. Get the reviewed card's source_note_id and atom_id
2. Find related cards:
   a. Cards linked via note_links (source notes connected)
   b. Cards sharing the same atom domain
   c. Cards with semantic overlap on atom.subject (cosine > 0.6)
3. For each related card:
   - relationship_strength: 1.0 (direct link), 0.5 (same domain), 0.3 (semantic overlap)
   - review_quality_factor: Easy=1.0, Good=0.8, Hard=0.3, Again=0.0
   - fractional_boost = relationship_strength × review_quality_factor × 0.15
   - If card due within 48h: extend due_at by (fractional_boost × current_interval) days
   - Cap: never extend more than 20% of card's current interval
4. Log propagation events for session summary
```

**Execution:** Background task via `tauri::async_runtime::spawn`. Non-blocking.

### Negative propagation

On Again ratings, apply a tiny fractional stability penalty to related cards:
- Max penalty: –0.08 of current interval
- Only applies to cards due within 72h
- Prevents knowledge graph from inflating on shaky foundations

### Prerequisite injection on wrong answers

When a card scores < 0.30 (Again):

1. Find prerequisite cards (cards whose source notes are linked FROM this card's source note)
2. Filter to cards not already in session, due within 7 days
3. Show a preview toast: "This builds on '[prerequisite front]' (due in 4 days). Review it now?" [Yes / Later]
4. If Yes: inject at `currentIndex + 2` (give a breather, not immediate next)
5. If Later: queue for next session with coaching nudge

### Propagation visibility

On ~20% of reviews that trigger boosts, show inline in GradeDisplay:
"This review quietly strengthened 3 linked concepts across your notes."
Tap → tiny inline graph preview (2–4 connected nodes). Another tap → full note graph view.

Smart sampling keeps it special — not on every card.

### User controls

**Knowledge Web Strength** slider in learning settings:
- Gentle (default): conservative boosts, rare injection
- Balanced: moderate boosts, injection on Again
- Aggressive: stronger boosts, injection on Again + Hard

Per-deck toggle: "Disable propagation for this deck."
Daily cap: configurable (default 15 boosted cards/day).

### Difficulty estimation at card generation time

Add to the LLM generation prompt:

```
For each card, also output:
  "difficulty_estimate": 1-5 (1=recall a fact, 5=synthesize multiple concepts)
  "prerequisite_concepts": ["concept1", "concept2"]
```

Maps to initial FSRS-5 parameters:

| Difficulty | Initial stability | Initial difficulty |
| ---------- | ---------------- | ----------------- |
| 1          | 4.0              | 3.0               |
| 2          | 3.0              | 4.0               |
| 3 (default)| 2.0              | 5.0               |
| 4          | 1.2              | 6.5               |
| 5          | 0.8              | 8.0               |

`prerequisite_concepts` stored as new JSON field on card for graph enrichment.

After 5+ reviews, the system adjusts stored difficulty based on actual performance + graph density. Cards the user consistently nails quietly lower their difficulty.

---

## Section 4: Multi-Mode Review System

### Mode definitions

```typescript
type AnswerMode = "typed" | "self_grade" | "multiple_choice" | "cloze_fill" | "voice" | "auto";
```

| Mode              | Input component        | Grading path                                        | Default for            |
| ----------------- | ---------------------- | --------------------------------------------------- | ---------------------- |
| `typed`           | `TypedAnswerInput`     | Full pipeline (exact → semantic → LLM)              | `basic`, `vocabulary`  |
| `self_grade`      | `SelfGradeInput`       | Direct record_review, `grading_method: "self_grade"` | Escape hatch           |
| `multiple_choice` | `MultipleChoiceInput`  | Exact match on selection, score 1.0 or 0.0           | Quick review sessions  |
| `cloze_fill`      | `ClozeInput`           | Exact + fuzzy (Levenshtein ≤ 2) + semantic safety net (cosine > 0.72) | `cloze` cards |
| `voice`           | `VoiceInput`           | Whisper/Web Speech transcript → typed pipeline        | Language `vocabulary`  |
| `auto`            | System-selected        | System picks best mode per card from performance data | Default for decks      |

### Mode selection precedence

1. **Card-level override** — new `preferred_mode` column on `flashcards` (nullable). User pins via "Always use [mode] for this card."
2. **Deck-level preference** — new `deck_preferences` table (`deck TEXT PK, answer_mode TEXT, updated_at TEXT`).
3. **Auto mode** — system picks from performance data (voice accuracy, typed score patterns, session context).
4. **Card-type default** — `basic` → typed, `cloze` → cloze_fill, `vocabulary` → typed (voice if enabled globally).

Resolution: card override > deck preference > auto > card-type default.

### Auto mode logic

After 10+ reviews on a deck, Auto picks modes based on:
- Highest avg score per mode for this deck
- Session context ("quick review" → MC bias, "focus session" → typed/voice)
- Shows tiny badge: "Auto-picked voice for you" with one-tap "Lock this"

### Multiple choice distractor generation

New handler `flashcard_generate_distractors`:

```rust
FlashcardDistractorParams {
    card_id: String,
    count: usize,  // default 3
}
```

LLM generates 3 plausible but incorrect distractors (same length/style as correct answer, semantically related but clearly wrong). Post-filter: reject distractors with cosine > 0.65 to correct answer (guarantees semantic diversity).

Cached in `card_distractors` column (JSON). Regenerated only on user request or card edit.

### Voice input

- **Primary:** Browser Web Speech API (zero dependencies, works in Tauri WebView)
- **Future:** Whisper IPC command for offline/better accuracy (interface ready, not in v1)
- Live transcript preview while recording
- Tiny waveform visualizer during recording
- On stop: transcript goes through standard `flashcard_submit_answer` with `mode: "voice"`
- Pronunciation is not graded in v1 — transcript graded as text

### Cloze fuzzy matching

Levenshtein ≤ 2 for typo tolerance. If fuzzy fails but cosine similarity to correct blank > 0.72, accept with "Close enough — you captured the idea." Prevents frustration on spelling variations.

### Mid-session mode switch

User can change mode for the current card without losing typed answer. Example: typed something → tap microphone → voice mode starts with typed text as initial transcript. Non-destructive, keeps flow.

### Adaptive suggestions

After 10+ reviews, if a pattern emerges (user scores 18% higher with voice on a deck), show a one-time suggestion in SessionSummary:
"You seem to do better with voice on [deck]. Switch default?"
Accept → saves preference. Dismiss → never shown again for this deck.

---

## Section 5: Session Intelligence & Cognitive Integration

### Session lifecycle

```
Session start
  → Load due cards + apply mode preferences
  → Create review_sessions row (status = 'active')
  → Track: start_time, per-card data, propagation events

Per card:
  → Record: answer, grade, mode, recall_speed_ms,
            socratic_used, saved_as_insight

Session end (all cards done OR user exits)
  → Compute session stats (local, no LLM)
  → Display SessionSummary with timed reveals
  → Optionally: save as insight, trigger coaching, reflection pulse
  → Update review_sessions row
```

### SessionSummary data

Computed entirely from local session state — no extra IPC calls:

```typescript
interface SessionSummary {
  duration: number;
  cardsReviewed: number;
  avgScore: number;
  scoresByMode: Record<AnswerMode, { count: number; avg: number }>;
  knowledgeConnectionsStrengthened: number;
  prerequisitesSurfaced: number;
  weakCards: Array<{
    front: string;
    score: number;
    linkedPrerequisites: string[];
  }>;
  streakDays: number;
  topChain: string[];  // longest propagation chain (card fronts)
}
```

### SessionSummary display — three timed beats

**Beat 1 (immediate):** Score ring animation — overall percentage with color gradient.

**Beat 2 (after 1s):** Stats cards fade in:
- "12 cards reviewed in 8 minutes"
- "Typed: 82% avg · Voice: 91% avg"
- "Day 14 streak"

**Beat 3 (after 2s):** The second-brain narrative:
- "You strengthened 7 knowledge connections"
- "Deepest chain: Stability → Forgetting curve → Interval calculation"
- "3 weak spots surfaced — [Create coaching session]"

### Reflection Pulse (after Beat 3)

A gentle auto-generated prompt that disappears if ignored:
> "What felt different about today's answers compared to last week?"

User can type 1–2 sentences or tap "Skip." The response becomes a new EpisodicMemory that coaching can reference later ("Last month you said stability felt confusing — today you nailed it").

### SessionSummary actions

| Action                    | What it does                                                      |
| ------------------------- | ----------------------------------------------------------------- |
| Save as insight note      | Creates note pre-filled with answers + explanations + weak spots  |
| Create coaching session   | Calls coaching feature with weak card topics                      |
| Visualize knowledge web   | Opens note graph filtered to session's source notes               |
| Review weak spots now     | Context-aware mini-session with 3 smart choices:                  |
|                           | - Quick 3-card warm-up (MC only)                                  |
|                           | - Deep dive (typed + Socratic, fullscreen)                        |
|                           | - Voice immersion (language decks)                                |
| Done                      | Close, return to deck picker                                      |

### Cognitive memory integration (automatic)

1. **On low scores (< 0.6):** Create `SemanticFact` from `{user_answer, expected_answer, explanation}` via `DomainEvent::KnowledgeAtomCreated`.
2. **On Socratic exchanges:** Q&A pair becomes `EpisodicMemory`. Cognitive pipeline can later surface "You had this same misconception 2 weeks ago."
3. **On Reflection Pulse responses:** New `EpisodicMemory` event.
4. **Session-level:** Publish a new `DomainEvent::FlashcardSessionCompleted` variant (to be added to `crates/bus/src/domain_events.rs`). Fields: `session_id: String`, `cards_reviewed: usize`, `avg_score: f64`, `weak_domains: Vec<String>`, `propagation_count: usize`.

### review_sessions table

New table in cognitive migrations — see Schema Changes Summary for full DDL with indexes.

### Production safeguards

- **No LLM calls in summary path** — everything from local state
- **Graceful exit** — mid-session close persists partial data, marks abandoned
- **Offline-safe** — semantic pre-filter uses local fastembed, LLM grading degrades to semantic score
- **Privacy** — answers never sent externally unless user explicitly saves as insight
- **Self-grade tracking** — when self-grade escape hatch is used, `grading_method: "self_grade"` and `score: None` are recorded for coaching ("You've been self-grading a lot — want to try typed mode?")

---

## Schema Changes Summary

### flashcards table (alter)

New columns added to `flashcards` in `crates/cognitive/migrations/001_cognitive_tables.sql` (consolidated, pre-release):

- `back_embedding_updated_at TEXT` — tracks when back embedding was computed
- `preferred_mode TEXT` — nullable, user's pinned mode for this card
- `difficulty_estimate INTEGER` — 1-5, from LLM at generation time
- `prerequisite_concepts TEXT` — JSON array, from LLM at generation time
- `card_distractors TEXT` — JSON array, cached MC distractors

**Rust struct updates required:** `FlashcardRow` (line 73, `crates/cognitive/src/repos/flashcard.rs`) must add all 5 new fields. `NewFlashcard` must add `difficulty_estimate: Option<i32>` and `prerequisite_concepts: Option<String>`. All `INSERT` and `SELECT *` queries in the repo must be updated to include the new columns.

### New tables

Both tables go in `crates/cognitive/migrations/001_cognitive_tables.sql` (consolidated, pre-release):

```sql
CREATE TABLE IF NOT EXISTS deck_preferences (
    deck TEXT PRIMARY KEY,
    answer_mode TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS review_sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    cards_reviewed INTEGER DEFAULT 0,
    avg_score REAL,
    duration_seconds INTEGER,
    modes_used TEXT,           -- JSON array
    propagation_count INTEGER DEFAULT 0,
    weak_card_ids TEXT,        -- JSON array
    session_data TEXT,         -- full SessionSummary as JSON
    status TEXT DEFAULT 'active'  -- 'active' | 'completed' | 'abandoned'
);

CREATE INDEX IF NOT EXISTS idx_review_sessions_status ON review_sessions(status);
CREATE INDEX IF NOT EXISTS idx_review_sessions_started ON review_sessions(started_at);
```

### LanceDB

New table: `flashcard_embeddings`. Follows the existing LanceDB schema pattern from `crates/storage/src/vector_store/schemas.rs` — single `id TEXT` primary key, `vector FixedSizeList<Float32, 384>`.

Key format: `id = "{card_id}_front"` or `id = "{card_id}_back"` (synthetic composite key). Additional metadata columns: `card_id TEXT`, `side TEXT` ("front" | "back"), `timestamp TEXT`.

Uses the same `paraphrase-multilingual-MiniLM-L12-v2` embedding model (384-dim) as note embeddings.

**Batch optimization:** `flashcard_save_generated` often saves multiple cards. Embed the entire batch in one `EmbeddingEngine` call rather than spawning N background tasks.

### New DomainEvent variant

Add to `crates/bus/src/domain_events.rs`:

```rust
FlashcardSessionCompleted {
    session_id: String,
    cards_reviewed: usize,
    avg_score: f64,
    weak_domains: Vec<String>,
    propagation_count: usize,
}
```

---

## New IPC Commands Summary

| Command                          | Direction | Purpose                                    |
| -------------------------------- | --------- | ------------------------------------------ |
| `flashcard_submit_answer`        | UI → Rust | Grading pipeline                           |
| `flashcard_explain_answer`       | UI → Rust | Deep Socratic follow-up                    |
| `flashcard_generate_distractors` | UI → Rust | MC distractors                             |
| `flashcard_save_mode_preference` | UI → Rust | Persist mode choice                        |
| `flashcard_get_prerequisites`    | UI → Rust | Find cards for prerequisite injection       |
| `flashcard_save_session`         | UI → Rust | Persist review session                     |

All existing flashcard commands stay unchanged.

---

## Integration Points

| System          | How it connects                                                     |
| --------------- | ------------------------------------------------------------------- |
| Cognitive memory| Low scores → SemanticFact, Socratic → EpisodicMemory, Reflection → EpisodicMemory |
| Coaching        | Low-score clusters → proactive coaching walks                       |
| Knowledge graph | FIRe propagation, prerequisite injection, difficulty adjustment     |
| Notes           | "Save as insight" creates note, "Jump to source" opens original     |
| Productivity    | Fullscreen sessions = deep work, compact = micro-habits             |
| Launcher        | "Start focus review on [deck]" quick action                        |
| Insights        | "Knowledge Web Health" chart from propagation data                  |

---

## Non-goals

- **Pronunciation grading** — voice is for convenience and active recall, not pronunciation training
- **Image occlusion input** — future mode, not in this spec
- **FSRS-5 weight training** — the review_log data is there for future personalization, but training is not in scope
- **Continuous FSRS-5 scoring** — discrete 1-4 ratings stay, continuous score is mapped via thresholds

## Naming Clarification

The existing `CardType::Typed` enum variant refers to a card type (typed-answer card). The new `AnswerMode::typed` refers to the review interaction mode (user types their answer). These are orthogonal concepts — a `basic` card type can use a `typed` answer mode. Code and documentation should use `CardType` vs `AnswerMode` to distinguish.

## Auto Mode Fallback

When `auto` mode has insufficient data (< 10 reviews on a deck), it falls back through: auto (no data) → card-type default → `typed`. This ensures new decks always have a sensible default without requiring user configuration.
