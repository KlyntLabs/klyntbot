# Learning Hub Gaps — Integrated Design Spec

**Date:** 2026-03-25
**Status:** Draft
**Scope:** Wire five gaps in the `/learn` page to transform it from a study tool into the memory cortex of Klyntbot's second brain.

## Context

The Learning Hub (`desktop-ui/src/features/learn/`) has a fully-wired core loop: create cards (manual or AI-generated) → review with FSRS-5 scheduling → FIRe graph propagation. However, five gaps prevent it from feeling like a living knowledge dashboard:

1. **StatsBar** — streak/retention/weekly stats hard-coded to `"--"`
2. **Ask AI** — Socratic explain button disabled during review
3. **Edit card** — edit button disabled during review
4. **RetentionChart + AtomGraph** — components exist but aren't mounted on dashboard
5. **"From last chat"** — button shows "Soon" in QuickGenerate

All five gaps are solved as a single coherent upgrade.

## Design

### 1. StatsBar — The heartbeat of the second brain

**Goal:** Replace hard-coded `"--"` values with live data. Make the dashboard pulse.

#### Backend

**New file:** `crates/desktop/src/commands/review_stats.rs`

Single Tauri command `review_stats_summary` returning:

```rust
ReviewStatsSummaryResponse {
    streak: u32,          // consecutive days with review OR qualified atom acceptance
    retention: f64,       // importance-weighted avg retention (0.0–1.0)
    weekly: Vec<WeeklyStatPoint>,  // last 7 days
}

WeeklyStatPoint {
    date: String,         // "YYYY-MM-DD"
    reviews: i64,
    atoms_created: i64,
}
```

**New file:** `crates/app-core/src/handlers/review_stats.rs`

Handler composes three existing `ReviewStatsRepo` methods:
- `current_streak()` — **extended** with a `UNION` to include `knowledge_atoms.created_at` dates where `status = 'active'` and `salience >= 0.6` (qualified atom acceptances count as streak days)
- `knowledge_retention_score()` — returns the weighted average directly
- `daily_reviews(7)` — provides review counts per day for the sparkline
- **New method** `daily_atoms_created(7)` on `ReviewStatsRepo` — separate query: `SELECT DATE(created_at) as d, COUNT(*) FROM knowledge_atoms WHERE status = 'active' AND created_at > ?1 GROUP BY d`. The handler merges the two sparse date sets into a single `Vec<WeeklyStatPoint>` with zero-fill for missing days

**Streak extension SQL sketch:**
```sql
SELECT DISTINCT DATE(reviewed_at) as d FROM review_log
UNION
SELECT DISTINCT DATE(updated_at) as d FROM knowledge_atoms
  WHERE status = 'active' AND salience >= 0.6
ORDER BY d DESC LIMIT 60
```

Then walk backwards counting consecutive days (same algorithm as existing `current_streak`).

**Registration:**
- Add `review_stats_summary` to `main.rs` invoke handler list
- Add `DEV_COMMANDS` + `dispatch_dev` in the new `commands/review_stats.rs` module
- Add `commands::review_stats::DEV_COMMANDS` to the `dev_server/mod.rs` coverage slice (required — the `dev_server_covers_all_tauri_commands` test enforces this)

#### Frontend

**New hook:** `desktop-ui/src/features/learn/hooks/useReviewStats.ts`
```ts
useQuery<ReviewStatsSummary>("review_stats_summary", {}, defaults)
```

**Modified:** `StatsBar.tsx`
- Accept `stats: ReviewStatsSummary` as prop (from `DashboardHome`)
- **Streak:** Show number with flame icon. Tooltip: "Reviews + meaningful atom acceptances = knowledge momentum"
- **Due:** Already works (passed as `totalDue`)
- **Retention:** Show as percentage. Color: green (`text-emerald-400`) when ≥ 0.8, amber (`text-amber-400`) when ≥ 0.5, red (`text-red-400`) below 0.5
- **This week:** Replace static "--" with a tiny `<Sparkline>` using recharts `<AreaChart>` (same lib already used by `RetentionChart`). Shows reviews + atoms per day. Height ~24px.

### 2. Ask AI (Socratic) — Automatic + contextual

**Goal:** Remove the disabled "Ask AI" button. Auto-trigger Socratic explanation on weak answers.

#### Important: Two review modes

`ImmersiveReview` uses **flip-card** review via `useReviewSession.ts` → `flashcard_record_review` (quality-based: again/hard/good/easy). There is **no** call to `flashcard_submit_answer` (which is the typed-answer active-recall pipeline in a separate flow). The Socratic trigger must work with the flip-card flow.

#### Trigger logic

In `useReviewSession.ts`, after `rate()` calls `flashcard_record_review`:
- If quality is `"again"` or `"hard"` → auto-call `flashcard_explain_answer` in the background
- Store explanation in state: `socraticExplanation: string | null`
- Since this is flip-card mode (no typed answer), pass synthetic context:
  - `userAnswer`: `"(self-rated as {quality} after seeing the answer)"`
  - `gradeExplanation`: `"Student self-assessed as '{quality}' — they may not fully understand the concept."`

The backend handler (`grading.rs:285-357`) already:
- Builds a Socratic prompt using front/back/userAnswer/gradeExplanation
- Calls the cognitive LLM
- Creates a `KnowledgeAtom` (type: `socratic_exchange`) via `DomainEvent::KnowledgeAtomCreated`
- Returns `{ explanation: string, saved_as_memory: bool }`

No backend changes needed — the handler accepts any string for `user_answer` and `grade_explanation`, which are just context for the LLM prompt.

#### Frontend UX

**In `ImmersiveReview.tsx`:** After rating, if quality was weak and `socraticExplanation` is set:
- Show a subtle chip below the card: **"Let's understand why"** with a lightbulb icon
- Clicking the chip expands an inline explanation panel (animated slide-down, `glass-card` styling)
- The chip auto-appears with a gentle fade-in animation
- The Socratic call runs in parallel with the rating — it doesn't block moving to the next card
- If the user moves to the next card before the explanation loads, discard it (fire-and-forget)

**Remove:** The disabled `<Lightbulb>` "Ask AI" button from the footer actions.

#### Params needed for the call

```ts
// After rate() completes for "again" or "hard":
ipc("flashcard_explain_answer", {
  cardId: current.id,
  userAnswer: `(self-rated as ${quality} after seeing the answer)`,
  gradeExplanation: `Student self-assessed as '${quality}' — they may not fully understand the concept.`,
})
```

No additional state tracking needed in `useReviewSession` — the quality and card ID are already available at rating time.

### 3. Inline Edit — Zero-friction, zero-modal

**Goal:** Press `E` or tap edit icon → card becomes editable in place.

#### Frontend

**New component:** `CardEditor.tsx` (inline, not modal)
- Pre-filled textareas for `front` and `back`
- Deck text input
- "Save" and "Cancel" buttons
- "Save" and "Cancel" buttons (no AI regeneration — deferred to v2)

**In `ImmersiveReview.tsx`:**
- New state: `editing: boolean`
- `E` key handler (when card shown, not in input): `setEditing(true)`
- When editing, render `<CardEditor>` instead of `<CardRenderer>`
- On save: `ipc("flashcard_update", { id, front, back, deck })` → update the card in local `cards` array → `setEditing(false)`
- On cancel: `setEditing(false)`

**Enable the "Edit" button in footer:** Replace `disabled` with `onClick={() => setEditing(true)}`.

#### Backend fix: re-embed on update

**Modified:** `crates/app-core/src/handlers/notes/flashcard.rs` — `flashcard_update` handler

After the `repo.update_card(...)` call, add a fire-and-forget `tokio::spawn` that re-embeds the card's front and back into `flashcard_embeddings` (same pattern as `flashcard_save_generated` in `card_generation.rs:L231-L254`). This prevents semantic grading degradation when card text changes.

### 4. RetentionChart + AtomGraph — Discoverable delight

**Goal:** Mount both existing components on the populated dashboard as collapsible sections.

#### Frontend

**Modified:** `DashboardHome.tsx` (populated branch, after `DeckList`)

Two new collapsible sections:

```tsx
<CollapsibleSection
  title="Retention Trend"
  icon={<TrendingUp />}
  storageKey="learn-retention-open"
  defaultOpen={false}
>
  <RetentionChart data={retentionData.overall} height={160} />
</CollapsibleSection>

<CollapsibleSection
  title="Knowledge Graph"
  icon={<Network />}
  storageKey="learn-graph-open"
  defaultOpen={false}  // auto-open on first populated visit via one-time localStorage flag
>
  <AtomGraph />
</CollapsibleSection>
```

**New utility component:** `CollapsibleSection.tsx` in `features/learn/components/`
- Uses `<details>`/`<summary>` or controlled state with `localStorage` persistence
- Animated open/close (CSS `grid-template-rows` transition)

**First-populated-visit logic:** On first render where `decks.length > 0`, check `localStorage.getItem("learn-graph-shown-once")`. If null, auto-open the AtomGraph section and set the flag.

**AtomGraph color enhancement:** Map node `background-color` by retention:
- `avgRetention >= 0.8` → green (`#34d399`)
- `avgRetention >= 0.5` → amber (`#fbbf24`)
- `avgRetention < 0.5` → red (`#f87171`)

**Data:** `useRetentionHistory(30)` for the chart (hook already wired). `useKnowledgeHealth()` for the graph (hook already wired). Both called from `DashboardHome`.

### 5. "From Recent Learning Moments"

**Goal:** Replace "From last chat... Soon" with a working feature that feeds recent conversation content into flashcard generation.

#### Backend

**New handler:** `crates/app-core/src/handlers/notes/recent_learning.rs`

```rust
pub async fn flashcard_recent_learning_sessions(
    repos: &Repos,
    limit: usize,  // default 3
) -> Result<Vec<RecentLearningSession>, ApiError>
```

Logic:
1. `repos.sessions.list_sessions()` → sort by `updated_at DESC` → take first `limit` in application code. **Note:** `list_sessions()` loads all sessions. For v1 this is acceptable (single-user local app, typically < 200 sessions). If performance becomes an issue, add a `list_recent_sessions(limit)` with SQL `LIMIT`.
2. For each session: `repos.sessions.get_recent_messages(&key, 10)` → concatenate user + assistant messages as a preview (first 200 chars)
3. Optionally: count atoms created with `source_session_key` matching (if that field exists, otherwise skip)
4. Return `Vec<RecentLearningSession>` where each has `{ sessionKey, title, updatedAt, preview, atomCount }`

**New Tauri command:** `flashcard_recent_learning_sessions` in `commands/notes.rs`. Add to `DEV_COMMANDS` in the same file and to `dispatch_dev`.

**New IPC type** `RecentLearningSession` in `desktop-shared/src/commands/notes.rs` (alongside other flashcard types)

#### Frontend

**Modified:** `QuickGenerate.tsx`

Replace the disabled "From last chat..." button with "From recent conversations..." that, when clicked:
1. Calls `ipc("flashcard_recent_learning_sessions", { limit: 3 })`
2. Shows a dropdown/list of the 3 sessions with title + preview + atom count badge
3. Selecting one calls `ipc("chat_messages", { sessionKey, limit: 50 })` to get full content
4. Concatenates all user+assistant messages into a single text blob
5. Passes to existing `onGenerateFromText(blob)`

**UX:** The session list shows inside the `QuickGenerate` card (same pattern as the existing "note" and "clipboard" modes). Each session row shows:
- Title (bold)
- Time ago ("2 hours ago")
- Preview snippet (truncated, muted)
- Atom badge if atoms were extracted ("3 atoms")

## Files Changed

### New files
| File | Purpose |
|------|---------|
| `crates/desktop/src/commands/review_stats.rs` | Tauri command for `review_stats_summary` + `DEV_COMMANDS` + `dispatch_dev` |
| `crates/app-core/src/handlers/review_stats.rs` | Handler composing `ReviewStatsRepo` methods with date merge |
| `crates/app-core/src/handlers/notes/recent_learning.rs` | Handler for `flashcard_recent_learning_sessions` |
| `crates/desktop-shared/src/commands/review_stats.rs` | `ReviewStatsSummaryResponse`, `WeeklyStatPoint` IPC types |
| `desktop-ui/src/features/learn/hooks/useReviewStats.ts` | SWR hook for stats |
| `desktop-ui/src/features/learn/components/CardEditor.tsx` | Inline card editor during review |
| `desktop-ui/src/features/learn/components/CollapsibleSection.tsx` | Collapsible section with localStorage persistence |

### Modified files
| File | Change |
|------|--------|
| `crates/cognitive/src/repos/review_stats.rs` | Extend `current_streak()` with atom UNION + add `daily_atoms_created()` |
| `crates/app-core/src/handlers/notes/flashcard.rs` | Add re-embed on `flashcard_update` |
| `crates/desktop/src/commands/notes.rs` | Add `flashcard_recent_learning_sessions` command + `DEV_COMMANDS` entry |
| `crates/desktop/src/main.rs` | Register `review_stats_summary` and `flashcard_recent_learning_sessions` |
| `crates/desktop/src/dev_server/mod.rs` | Add `commands::review_stats::DEV_COMMANDS` to coverage slice |
| `crates/desktop-shared/src/commands/notes.rs` | Add `RecentLearningSession` IPC type |
| `crates/desktop-shared/src/commands/mod.rs` | Register `review_stats` submodule |
| `desktop-ui/src/features/learn/components/DashboardHome.tsx` | Add RetentionChart, AtomGraph sections, wire stats |
| `desktop-ui/src/features/learn/components/StatsBar.tsx` | Accept stats prop, add sparkline, color retention |
| `desktop-ui/src/features/learn/components/ImmersiveReview.tsx` | Auto-Socratic, editing mode, enable buttons |
| `desktop-ui/src/features/learn/components/QuickGenerate.tsx` | Replace "Soon" with recent conversations picker |
| `desktop-ui/src/features/learn/components/AtomGraph.tsx` | Retention-based node coloring |
| `desktop-ui/src/features/learn/hooks/useReviewSession.ts` | Auto-explain on weak rating, editing state |
| `desktop-ui/src/features/learn/pages/LearnPage.tsx` | Pass new data flows through |

## Testing

- `cargo nextest run -p cognitive -E 'test(review_stats)'` — existing tests still pass + new streak test with atom dates
- `cargo nextest run -p app-core` — handlers compile and return expected shapes
- `cargo clippy --workspace --all-targets --all-features` — zero warnings
- `cd desktop-ui && bun run lint:fix && bun run test` — frontend compiles and lints clean
- Manual: create a card via QuickAdd → verify StatsBar updates → review → verify Socratic chip appears on "again" → edit card mid-review → check dashboard charts render

## Non-goals

- FSRS weight training (separate initiative)
- Atom-to-flashcard auto-linking during generation (separate issue)
- "Regenerate Back" AI button in edit mode (defer to v2 — keep v1 simple with manual editing only)
- Persona-aware Socratic voice (backend would need to resolve the persona from the note context — defer)
- AtomGraph edge data (no inter-topic link data available yet)

## Open questions

None — all decisions resolved in design discussion.
