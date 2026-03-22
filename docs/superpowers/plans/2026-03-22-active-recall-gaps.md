# Active Recall Gaps — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all remaining gaps between the active recall spec and the current implementation — wire frontend to existing backend handlers, add missing components, complete keyboard shortcuts, and integrate cognitive memory.

**Architecture:** All backend handlers already exist and are wired through Tauri commands. The work is primarily frontend wiring (calling existing IPC commands), adding 2 small components (SocraticPanel, PropagationRipple), and connecting the cognitive memory pipeline on grading events.

**Tech Stack:** TypeScript/React (frontend wiring + new components), Rust (cognitive memory integration + back_embedding_updated_at fix)

**Spec:** `docs/superpowers/specs/2026-03-22-active-recall-flashcard-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/components/review/SocraticPanel.tsx` | Expanded Socratic deep-dive panel calling `flashcard_explain_answer` |
| `desktop-ui/src/features/notes/components/review/PropagationRipple.tsx` | Inline notification showing graph propagation count |

### Modified files

| File | Changes |
|------|---------|
| `desktop-ui/src/features/notes/hooks/useActiveReview.ts` | Add `requestExplanation` IPC call, `saveSession` on complete/exit, `loadDistracters`, `saveInsight`, mode preference save, Tab key mode toggle |
| `desktop-ui/src/features/notes/components/review/ActiveReviewSession.tsx` | Add `s`/`j`/`Tab` keyboard shortcuts, wire `onSaveInsight`/`onReviewWeak`/`onJumpToSource`, use `stats` from hook, add SocraticPanel + PropagationRipple |
| `desktop-ui/src/features/notes/components/review/ReviewCard.tsx` | Load distractors for MC mode, pass to MultipleChoiceInput, show SocraticPanel in socratic phase, show PropagationRipple |
| `desktop-ui/src/features/notes/components/review/SessionSummary.tsx` | Wire reflection pulse save, add "Review weak spots" with 3 choices |
| `crates/app-core/src/handlers/notes/grading.rs` | Publish `KnowledgeAtomCreated` on low scores, save Socratic exchanges as episodic memory |
| `crates/app-core/src/handlers/notes/card_generation.rs` | Update `back_embedding_updated_at` after embedding |

---

## Task 1: Wire Socratic Panel to Backend

**Files:**
- Create: `desktop-ui/src/features/notes/components/review/SocraticPanel.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useActiveReview.ts`
- Modify: `desktop-ui/src/features/notes/components/review/ReviewCard.tsx`

- [ ] **Step 1: Create SocraticPanel component**

Create `desktop-ui/src/features/notes/components/review/SocraticPanel.tsx`:

```tsx
import { ipc } from "@shared/hooks/useIpc";
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

interface SocraticPanelProps {
  cardId: string;
  userAnswer: string;
  gradeExplanation: string;
}

export function SocraticPanel({ cardId, userAnswer, gradeExplanation }: SocraticPanelProps) {
  const [explanation, setExplanation] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    ipc<{ explanation: string }>("flashcard_explain_answer", {
      cardId,
      userAnswer,
      gradeExplanation,
    })
      .then((res) => {
        if (!cancelled) {
          setExplanation(res.explanation);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : "Failed to load explanation");
          setLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [cardId, userAnswer, gradeExplanation]);

  if (loading) {
    return (
      <div className="rounded-lg bg-white/[0.03] border border-accent/20 p-3 flex items-center gap-2">
        <Loader2 size={12} className="animate-spin text-accent" />
        <span className="text-[10px] text-dim">Thinking deeper...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg bg-red-500/10 border border-red-500/20 p-3">
        <p className="text-[10px] text-red-400">{error}</p>
      </div>
    );
  }

  return (
    <div className="rounded-lg bg-white/[0.03] border border-accent/20 p-3">
      <p className="text-[9px] text-accent font-medium mb-1.5">Socratic follow-up</p>
      <p className="text-[10px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
        {explanation}
      </p>
    </div>
  );
}
```

- [ ] **Step 2: Track userAnswer in useActiveReview state**

In `useActiveReview.ts`, add `lastAnswer: string` to `ActiveReviewState` (default `""`). In `submitAnswer`, set `lastAnswer: text` alongside `gradeResult`:

```typescript
// In ActiveReviewState interface:
lastAnswer: string;

// In initialState():
lastAnswer: "",

// In submitAnswer, inside the try block after getting result:
patchState({ gradeResult: result, cardPhase: "graded", lastAnswer: text });
```

Export `lastAnswer` from the hook's return.

- [ ] **Step 3: Render SocraticPanel in ReviewCard**

In `ReviewCard.tsx`, import `SocraticPanel` and add a `lastAnswer` prop to `ReviewCardProps`. In the graded/socratic render section, add:

```tsx
{cardPhase === "socratic" && gradeResult && (
  <SocraticPanel
    cardId={card.id}
    userAnswer={lastAnswer}
    gradeExplanation={gradeResult.explanation ?? ""}
  />
)}
```

- [ ] **Step 4: Pass lastAnswer through ActiveReviewSession**

In `ActiveReviewSession.tsx`, destructure `lastAnswer` from `useActiveReview()` and pass to `ReviewCard`:

```tsx
<ReviewCard
  ...existing props...
  lastAnswer={lastAnswer}
/>
```

- [ ] **Step 5: Run lint and tests**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(active-recall): wire SocraticPanel to flashcard_explain_answer backend"
```

---

## Task 2: Wire MC Distractors Loading

**Files:**
- Modify: `desktop-ui/src/features/notes/components/review/ReviewCard.tsx`

- [ ] **Step 1: Load distractors when mode is multiple_choice**

In `ReviewCard.tsx`, add state and effect to load distractors:

```tsx
import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useState } from "react";

// Inside ReviewCard component:
const [distractors, setDistractors] = useState<string[]>([]);

useEffect(() => {
  if (mode !== "multiple_choice" || cardPhase !== "answering") return;
  setDistractors([]);
  ipc<{ distractors: string[]; cached: boolean }>("flashcard_generate_distractors", {
    cardId: card.id,
    count: 3,
  })
    .then((res) => setDistractors(res.distractors))
    .catch(() => setDistractors([]));
}, [mode, card.id, cardPhase]);
```

- [ ] **Step 2: Pass distractors to MultipleChoiceInput**

Replace the hardcoded `distractors={[]}` in `AnswerInput`:

```tsx
case "multiple_choice":
  return <MultipleChoiceInput correctAnswer={card.back} distractors={distractors} onSelect={onSubmit} />;
```

Since `AnswerInput` is an inner function of `ReviewCard`, it can access `distractors` from the parent scope. If it's extracted as a separate component, pass it as a prop.

- [ ] **Step 3: Show loading state while distractors load**

In the `multiple_choice` case, if `distractors.length === 0`, show a loading spinner instead of the input:

```tsx
case "multiple_choice":
  if (distractors.length === 0) {
    return (
      <div className="flex items-center gap-2 py-3 justify-center">
        <span className="text-[10px] text-dim">Generating options...</span>
        <span className="w-3 h-3 rounded-full border border-accent/40 border-t-accent animate-spin" />
      </div>
    );
  }
  return <MultipleChoiceInput correctAnswer={card.back} distractors={distractors} onSelect={onSubmit} />;
```

- [ ] **Step 4: Run lint and tests**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(active-recall): load MC distractors from backend via flashcard_generate_distractors"
```

---

## Task 3: Wire Session Save + Missing Keyboard Shortcuts

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useActiveReview.ts`
- Modify: `desktop-ui/src/features/notes/components/review/ActiveReviewSession.tsx`

- [ ] **Step 1: Add saveSession action to useActiveReview**

In `useActiveReview.ts`, add a `saveSession` function that calls `flashcard_save_session`:

```typescript
const saveSession = useCallback(async (status: "completed" | "abandoned") => {
  try {
    await ipc("flashcard_save_session", {
      sessionId: state.sessionId,
      cardsReviewed: statsRef.current.cardsReviewed,
      avgScore: statsRef.current.cardsReviewed > 0
        ? statsRef.current.totalScore / statsRef.current.cardsReviewed
        : 0,
      durationSeconds: Math.round((Date.now() - statsRef.current.startTime) / 1000),
      modesUsed: Object.keys(statsRef.current.modeUsage),
      propagationCount: statsRef.current.propagationCount,
      weakCardIds: statsRef.current.weakCards.map(w => w.front), // Use front as identifier
      sessionData: JSON.stringify(statsRef.current),
      status,
    });
  } catch {
    // best-effort — don't block session end
  }
}, [state.sessionId]);
```

- [ ] **Step 2: Call saveSession on session completion**

In `advanceQueue`, when `nextIndex >= queue.length` (session complete), call `saveSession("completed")`:

```typescript
function advanceQueue() {
  setState((prev) => {
    const nextIndex = prev.currentIndex + 1;
    if (nextIndex >= prev.queue.length) {
      // Fire-and-forget session save
      saveSession("completed");
      return { ...prev, phase: "complete", cardPhase: "answering", currentIndex: nextIndex };
    }
    return { ...prev, currentIndex: nextIndex, cardPhase: "answering", gradeResult: null, lastAnswer: "" };
  });
}
```

- [ ] **Step 3: Add saveModePreference action**

```typescript
const saveModePreference = useCallback(async (mode: AnswerMode) => {
  if (!state.selectedDeck) return;
  try {
    await ipc("flashcard_save_mode_preference", {
      deck: state.selectedDeck,
      mode,
    });
  } catch {
    // best-effort
  }
}, [state.selectedDeck]);
```

Call it in `switchMode` after updating local state:

```typescript
const switchMode = useCallback((mode: AnswerMode) => {
  patchState({ selectedMode: mode });
  saveModePreference(mode);
}, [saveModePreference]);
```

- [ ] **Step 4: Export saveSession from the hook**

Add `saveSession` to the return object.

- [ ] **Step 5: Add missing keyboard shortcuts to ActiveReviewSession**

In `ActiveReviewSession.tsx`, add `s`, `j`, and `Tab` to the keyboard handler:

```typescript
case "s":
  if (cardPhase === "graded" || cardPhase === "socratic") {
    // Save as insight — no-op for now, placeholder
  }
  break;
case "j":
  if (cardPhase === "graded" || cardPhase === "socratic") {
    // Jump to source note — no-op for now, placeholder
  }
  break;
case "Tab":
  e.preventDefault();
  switchMode(selectedMode === "self_grade" ? "typed" : "self_grade");
  break;
```

Add `switchMode` and `selectedMode` to the destructured values and the useEffect dependency array.

- [ ] **Step 6: Wire onClose to save abandoned session**

In `ActiveReviewSession.tsx`, wrap `onClose` to save the session:

```tsx
const handleExit = useCallback(() => {
  if (phase === "reviewing" && statsRef.current.cardsReviewed > 0) {
    saveSession("abandoned");
  }
  onClose();
}, [phase, saveSession, onClose]);
```

Use `handleExit` instead of `onClose` for the exit button, Escape key, and SessionProgress `onExit`.

- [ ] **Step 7: Destructure `stats` (ref) and `saveSession` from hook**

In `ActiveReviewSession.tsx`, add `stats` and `saveSession` to the destructured values from `useActiveReview()`. Fix the `SessionSummary` to use `stats.current`:

```tsx
const { ..., stats, saveSession } = useActiveReview();
```

- [ ] **Step 8: Run lint and tests**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 9: Commit**

```bash
git commit -m "feat(active-recall): wire session save, mode preference persistence, and missing shortcuts"
```

---

## Task 4: PropagationRipple Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/review/PropagationRipple.tsx`
- Modify: `desktop-ui/src/features/notes/components/review/GradeDisplay.tsx`

- [ ] **Step 1: Create PropagationRipple component**

Create `desktop-ui/src/features/notes/components/review/PropagationRipple.tsx`:

```tsx
import { Zap } from "lucide-react";

interface PropagationRippleProps {
  count: number;
}

export function PropagationRipple({ count }: PropagationRippleProps) {
  if (count <= 0) return null;

  return (
    <div className="flex items-center gap-1.5 py-1 text-[9px] text-accent/70">
      <Zap size={10} className="text-accent" />
      <span>
        This review strengthened{" "}
        <span className="font-medium text-accent">{count}</span> linked{" "}
        {count === 1 ? "concept" : "concepts"}
      </span>
    </div>
  );
}
```

- [ ] **Step 2: Add propagationCount prop to GradeDisplay**

In `GradeDisplay.tsx`, add an optional `propagationCount` prop and render `PropagationRipple` at the bottom:

```tsx
import { PropagationRipple } from "./PropagationRipple";

interface GradeDisplayProps {
  result: GradeResult;
  propagationCount?: number;
}

// At the end of the component's render:
{propagationCount != null && <PropagationRipple count={propagationCount} />}
```

- [ ] **Step 3: Pass propagationCount through ReviewCard**

Add `propagationCount?: number` to `ReviewCardProps` and pass it to `GradeDisplay`:

```tsx
<GradeDisplay result={gradeResult} propagationCount={propagationCount} />
```

In `ActiveReviewSession.tsx`, pass `propagationCount={stats.current.propagationCount}` to `ReviewCard`.

Note: `propagationCount` is currently always 0 because the backend FIRe runs as fire-and-forget. This component is ready for when the backend returns the count (a future enhancement where `flashcard_record_review` returns a `propagation_count` field).

- [ ] **Step 4: Run lint and tests**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(active-recall): add PropagationRipple component for knowledge graph visibility"
```

---

## Task 5: Wire SessionSummary Actions

**Files:**
- Modify: `desktop-ui/src/features/notes/components/review/ActiveReviewSession.tsx`
- Modify: `desktop-ui/src/features/notes/components/review/SessionSummary.tsx`

- [ ] **Step 1: Implement onReviewWeak in ActiveReviewSession**

When "Review weak spots" is clicked, restart a mini-session with just the weak cards. In `ActiveReviewSession.tsx`:

```tsx
const handleReviewWeak = useCallback(() => {
  // Get weak card IDs from stats, re-fetch them as due cards
  // For now, just restart the review on the same deck
  if (selectedDeck) {
    startReview(selectedDeck);
  }
}, [selectedDeck, startReview]);
```

Pass to SessionSummary: `onReviewWeak={handleReviewWeak}`.

- [ ] **Step 2: Implement onSaveInsight**

When "Save as insight" is clicked, create a note with session data:

```tsx
const handleSaveInsight = useCallback(async () => {
  const s = stats.current;
  const avgPct = s.cardsReviewed > 0 ? Math.round((s.totalScore / s.cardsReviewed) * 100) : 0;
  const weakList = s.weakCards.map(w => `- ${w.front} (${Math.round(w.score * 100)}%)`).join("\n");

  const body = `# Review Session Summary\n\n**Score:** ${avgPct}%\n**Cards:** ${s.cardsReviewed}\n\n${weakList ? `## Weak spots\n${weakList}` : "All cards held strong."}`;

  try {
    await ipc("note_create", { title: `Review ${new Date().toLocaleDateString()}`, body });
  } catch {
    // best-effort
  }
}, [stats]);
```

Pass to both the reviewing-phase `onSaveInsight` in ReviewCard AND the SessionSummary `onSaveInsight`.

- [ ] **Step 3: Wire onJumpToSource**

In `ActiveReviewSession.tsx`, implement jump to source:

```tsx
const handleJumpToSource = useCallback(() => {
  if (current?.sourceNoteId) {
    // Emit a custom event that the KnowledgeBasePage listens to for note navigation
    window.dispatchEvent(new CustomEvent("navigate-to-note", { detail: { noteId: current.sourceNoteId } }));
    onClose();
  }
}, [current, onClose]);
```

Pass to `ReviewCard`: `onJumpToSource={handleJumpToSource}`.

- [ ] **Step 4: Update SessionSummary to save reflection text**

In `SessionSummary.tsx`, when the user types a reflection and clicks away or presses Enter, save it via a callback. Add an `onSaveReflection` prop:

```tsx
interface SessionSummaryProps {
  stats: SessionStats;
  onClose: () => void;
  onSaveInsight: () => void;
  onReviewWeak: () => void;
  onSaveReflection?: (text: string) => void;
}
```

On the Skip button, if there's text, call `onSaveReflection(reflectionText)` before dismissing.

In `ActiveReviewSession.tsx`, pass a handler that saves the reflection as a note:

```tsx
onSaveReflection={async (text) => {
  if (!text.trim()) return;
  try {
    await ipc("note_create", {
      title: `Reflection ${new Date().toLocaleDateString()}`,
      body: text,
    });
  } catch {}
}}
```

- [ ] **Step 5: Run lint and tests**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(active-recall): wire session summary actions (save insight, review weak, reflection)"
```

---

## Task 6: Cognitive Memory Integration (Backend)

**Files:**
- Modify: `crates/app-core/src/handlers/notes/grading.rs`
- Modify: `crates/app-core/src/handlers/notes/card_generation.rs`

- [ ] **Step 1: Publish KnowledgeAtomCreated on low grading scores**

In `grading.rs`, at the end of `flashcard_submit_answer`, after building the `GradeResultResponse`, check if the score is below 0.6 and publish a domain event:

```rust
// After building the response, before returning:
if let Some(score) = response.score {
    if score < 0.6 {
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                atom_id: uuid::Uuid::new_v4().to_string(),
                subject: card.front.clone(),
                content: format!(
                    "User answered: {}\nExpected: {}\nExplanation: {}",
                    params.user_answer,
                    card.back,
                    response.explanation.as_deref().unwrap_or(""),
                ),
                domain: card.deck.clone(),
                source_note_id: card.source_note_id.clone(),
                importance: 0.7,
            });
        }
    }
}
```

Check the exact fields of `KnowledgeAtomCreated` in `crates/bus/src/domain_events.rs` before implementing — the fields above are approximate. Match the actual variant's shape.

- [ ] **Step 2: Save Socratic exchanges as episodic memory**

In `grading.rs`, in `flashcard_explain_answer`, after getting the LLM response, publish an event:

```rust
// After getting the explanation text:
if let Some(bus) = &self.domain_event_bus {
    bus.publish(bus::DomainEvent::EpisodicMemoryCreated {
        content: format!(
            "Socratic exchange on '{}': Student said '{}', tutor explained: {}",
            card.front, params.user_answer, &text
        ),
        source: "flashcard_review".to_string(),
        source_note_id: card.source_note_id.clone(),
    });
}
```

Again, check the exact variant shape in `domain_events.rs` — `EpisodicMemoryCreated` may not exist. If it doesn't, use the closest existing event (e.g., `KnowledgeAtomCreated` with appropriate fields). Set `saved_as_memory: true` in the response when successfully published.

- [ ] **Step 3: Update back_embedding_updated_at after embedding**

In `card_generation.rs`, in `embed_flashcard_batch`, after successfully upserting the back embedding, update the SQLite column:

```rust
// After the back embedding upsert succeeds:
if let Ok(pool) = card_pool_ref.as_ref() {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE flashcards SET back_embedding_updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(card_id)
        .execute(pool)
        .await;
}
```

This requires passing a `SqlitePool` clone to `embed_flashcard_batch`. Update the function signature to accept `Option<SqlitePool>` as an additional parameter. At the call sites in `flashcard_save_generated` and `flashcard_create`, pass `self.flashcard_repo().ok().map(|r| r.pool().clone())`.

Alternatively, if the repo doesn't expose `pool()`, use `FlashcardRepo::update_embedding_timestamp(card_id)` — add a simple repo method:

```rust
pub async fn update_embedding_timestamp(&self, id: &str) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE flashcards SET back_embedding_updated_at = ?1 WHERE id = ?2")
        .bind(&now).bind(id)
        .execute(&self.pool).await?;
    Ok(())
}
```

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p app-core && cargo nextest run -p cognitive
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(active-recall): integrate cognitive memory on low scores and Socratic exchanges"
```

---

## Task 7: Final Verification

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace Rust tests**

```bash
cargo nextest run --workspace
```

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```

Expected: 0 warnings.

- [ ] **Step 3: Run frontend lint + tests**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```

Expected: 46+ tests pass, 0 lint errors in review/ components.

- [ ] **Step 4: Verify all IPC commands have frontend callers**

Check that these commands are now called from the frontend:
- `flashcard_submit_answer` — useActiveReview.submitAnswer ✓ (existing)
- `flashcard_explain_answer` — SocraticPanel ✓ (Task 1)
- `flashcard_generate_distractors` — ReviewCard ✓ (Task 2)
- `flashcard_save_mode_preference` — useActiveReview.switchMode ✓ (Task 3)
- `flashcard_get_mode_preference` — useActiveReview.startReview ✓ (existing)
- `flashcard_save_session` — useActiveReview.saveSession ✓ (Task 3)
- `flashcard_get_prerequisites` — still no frontend caller (deferred — prerequisite injection UI is a future task)

- [ ] **Step 5: Commit**

```bash
git commit -m "test(active-recall): verify full spec coverage"
```

---

## Summary

| Task | What it delivers | Key gap closed |
|------|------------------|----------------|
| 1 | SocraticPanel + IPC call | `flashcard_explain_answer` wired to frontend |
| 2 | MC distractors loading | `flashcard_generate_distractors` wired to frontend |
| 3 | Session save + shortcuts + mode save | `flashcard_save_session`, `flashcard_save_mode_preference` wired; `s`/`j`/`Tab` shortcuts added |
| 4 | PropagationRipple component | Visual feedback for graph propagation |
| 5 | SessionSummary actions | "Save as insight", "Review weak spots", reflection persistence |
| 6 | Cognitive memory integration | Low-score → KnowledgeAtom, Socratic → EpisodicMemory, back_embedding_updated_at |
| 7 | Final verification | Full workspace test + lint + coverage check |

### Spec items explicitly deferred (not in this plan):
- First-Review Tutorial Deck (separate onboarding feature)
- Confetti animation on ≥ 0.85 (visual polish)
- Adaptive mode suggestions after 10+ reviews (needs review history aggregation)
- Cloze-specific Levenshtein fuzzy matching (backend grading handles it generically)
- Mid-session mode switch preserving typed answer (complex state management)
- 3rd graph source: cosine overlap on atom.subject (needs new embedding queries)
- `note_entity_mentions` as graph source (needs new SQL joins)
- `propagationCount` relay from backend (needs protocol change in `flashcard_record_review` response)
- Fullscreen layout visual differentiation (CSS-only, deferred)
