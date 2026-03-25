# Learning Hub Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire five gaps in the `/learn` page — live stats, auto-Socratic explanations, inline card editing, dashboard charts, and recent conversation flashcard generation.

**Architecture:** Backend-first — new Tauri commands and handler modifications, then frontend wiring. Each task is independently shippable. Tasks 1-2 (StatsBar), 3 (re-embed fix), 4-5 (Socratic), 6-7 (edit), 8 (charts), 9-10 (recent conversations) form natural pairs.

**Tech Stack:** Rust (Tauri 2, sqlx, tokio), TypeScript/React (recharts, cytoscape, Tailwind v4), SWR-style `useQuery`/`ipc()`.

**Spec:** `docs/superpowers/specs/2026-03-25-learning-hub-gaps-design.md`

---

### Task 1: Backend — Review Stats IPC Types + Handler

**Files:**
- Create: `crates/desktop-shared/src/commands/review_stats.rs`
- Create: `crates/app-core/src/handlers/review_stats.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs` (add module)
- Modify: `crates/app-core/src/handlers/mod.rs` (add module)
- Modify: `crates/cognitive/src/repos/review_stats.rs` (extend streak + add `daily_atoms_created`)

- [ ] **Step 1: Add IPC types in desktop-shared**

Create `crates/desktop-shared/src/commands/review_stats.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyStatPoint {
    pub date: String,
    pub reviews: i64,
    pub atoms_created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatsSummaryResponse {
    pub streak: u32,
    pub retention: f64,
    pub weekly: Vec<WeeklyStatPoint>,
}
```

Register in `crates/desktop-shared/src/commands/mod.rs` — add `mod review_stats; pub use review_stats::*;` (alphabetical, between `retention_history` and `settings`).

- [ ] **Step 2: Extend `ReviewStatsRepo` — streak with atoms + `daily_atoms_created`**

In `crates/cognitive/src/repos/review_stats.rs`:

Replace the `current_streak()` SQL query with:

```rust
pub async fn current_streak(&self) -> Result<usize, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT DISTINCT d FROM (
            SELECT DATE(reviewed_at) as d FROM review_log
            UNION
            SELECT DATE(updated_at) as d FROM knowledge_atoms
              WHERE status = 'active' AND salience >= 0.6
        ) ORDER BY d DESC LIMIT 60"#,
    )
    .fetch_all(&self.pool)
    .await?;
    // ... rest of walk-backwards algorithm stays the same
```

Add new method:

```rust
pub async fn daily_atoms_created(&self, days: i64) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    sqlx::query_as(
        r#"SELECT DATE(created_at) as d, COUNT(*) as cnt
           FROM knowledge_atoms
           WHERE status = 'active' AND created_at > ?1
           GROUP BY d ORDER BY d DESC"#,
    )
    .bind(&cutoff)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 3: Create handler in app-core**

Create `crates/app-core/src/handlers/review_stats.rs`:

```rust
use desktop_shared::commands::{ReviewStatsSummaryResponse, WeeklyStatPoint};
use desktop_shared::errors::ApiError;

use super::atoms::map_db;
use crate::state::AppCore;

impl AppCore {
    pub async fn review_stats_summary(&self) -> Result<ReviewStatsSummaryResponse, ApiError> {
        let repo = cognitive::ReviewStatsRepo::new(self.storage_pool.inner().clone());

        let (streak, retention, daily_reviews, daily_atoms) = tokio::try_join!(
            repo.current_streak(),
            repo.knowledge_retention_score(),
            repo.daily_reviews(7),
            repo.daily_atoms_created(7),
        )
        .map_err(map_db)?;

        // Merge sparse date sets into 7-day Vec<WeeklyStatPoint> with zero-fill
        let today = chrono::Utc::now().date_naive();
        let mut weekly = Vec::with_capacity(7);
        for i in (0..7).rev() {
            let date = (today - chrono::Duration::days(i)).format("%Y-%m-%d").to_string();
            let reviews = daily_reviews
                .iter()
                .find(|r| r.date == date)
                .map(|r| r.review_count)
                .unwrap_or(0);
            let atoms = daily_atoms
                .iter()
                .find(|(d, _)| d == &date)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            weekly.push(WeeklyStatPoint {
                date,
                reviews,
                atoms_created: atoms,
            });
        }

        Ok(ReviewStatsSummaryResponse {
            streak: streak as u32,
            retention,
            weekly,
        })
    }
}
```

Register in `crates/app-core/src/handlers/mod.rs` — add `pub mod review_stats;` (alphabetical, between `retention_history` and `settings`).

- [ ] **Step 4: Verify backend compiles**

Run: `cargo build -p app-core -p desktop-shared`
Expected: Compiles with zero errors.

- [ ] **Step 5: Add test for extended streak**

Add to `crates/cognitive/src/repos/review_stats.rs` tests module:

```rust
#[tokio::test]
async fn test_current_streak_includes_atoms() {
    let pool = cognitive_test_pool().await;
    let repo = ReviewStatsRepo::new(pool.clone());
    let now = Utc::now().to_rfc3339();
    let today = Utc::now().date_naive();

    // Insert an active atom created today (no reviews)
    sqlx::query(
        r#"INSERT INTO knowledge_atoms
            (id, subject, atom_type, domain, retention_pct, stability, difficulty,
             personal_importance, status, salience, created_at, updated_at)
        VALUES ('a-streak', 'test', 'concept', 'test', 0.8, 1.0, 5.0, 1.0, 'active', 0.8, ?1, ?1)"#,
    )
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let streak = repo.current_streak().await.unwrap();
    assert_eq!(streak, 1, "Atom acceptance alone should count as a streak day");
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(review_stats)'`
Expected: All review_stats tests pass including the new one.

- [ ] **Step 7: Commit**

```
git add crates/desktop-shared/src/commands/review_stats.rs crates/desktop-shared/src/commands/mod.rs crates/app-core/src/handlers/review_stats.rs crates/app-core/src/handlers/mod.rs crates/cognitive/src/repos/review_stats.rs
git commit -m "feat(learn): add review_stats_summary handler with atom-inclusive streak"
```

---

### Task 2: Backend — Review Stats Tauri Command

**Files:**
- Create: `crates/desktop/src/commands/review_stats.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add module)
- Modify: `crates/desktop/src/main.rs` (register command)
- Modify: `crates/desktop/src/dev_server/mod.rs` (add DEV_COMMANDS to `dev_command_names()`)
- Modify: `crates/desktop/src/dev_server/dispatch.rs` (add routing call)

- [ ] **Step 1: Create Tauri command module**

Create `crates/desktop/src/commands/review_stats.rs`:

```rust
use desktop_shared::commands::ReviewStatsSummaryResponse;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn review_stats_summary(
    state: State<'_, Arc<AppCore>>,
) -> Result<ReviewStatsSummaryResponse, ApiError> {
    state.review_stats_summary().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["review_stats_summary"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "review_stats_summary" => dev::val(core.review_stats_summary().await),
        _ => return None,
    })
}
```

- [ ] **Step 2: Register in commands/mod.rs**

Add `pub mod review_stats;` to `crates/desktop/src/commands/mod.rs` (alphabetical, between `retention_history` and `settings`).

- [ ] **Step 3: Register in main.rs**

Add after the `// Retention History` section (~line 496):

```rust
// Review Stats
commands::review_stats::review_stats_summary,
```

- [ ] **Step 4: Register in dev_server/mod.rs**

Add `commands::review_stats::DEV_COMMANDS,` to the `modules` slice in `dev_command_names()` (~line 226, between `retention_history` and `view`).

Also add the dispatch call in the dev server's main dispatch function — find where other modules dispatch and add:

```rust
if let Some(r) = commands::review_stats::dispatch_dev(cmd, &core, &body).await {
    return r;
}
```

- [ ] **Step 5: Build and test registration**

Run: `cargo build -p desktop && cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: Build succeeds. `dev_server_covers_all_tauri_commands` test passes.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p desktop -p app-core -p desktop-shared --all-targets`
Expected: Zero warnings.

- [ ] **Step 7: Commit**

```
git add crates/desktop/src/commands/review_stats.rs crates/desktop/src/commands/mod.rs crates/desktop/src/main.rs crates/desktop/src/dev_server/mod.rs
git commit -m "feat(learn): register review_stats_summary Tauri command"
```

---

### Task 3: Backend — Re-embed on flashcard_update

**Files:**
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs:200-218`

- [ ] **Step 1: Add fire-and-forget embed after update_card**

In `crates/app-core/src/handlers/notes/flashcard.rs`, modify `flashcard_update`:

```rust
pub async fn flashcard_update(
    &self,
    params: FlashcardUpdateParams,
) -> Result<FlashcardResponse, ApiError> {
    let repo = self.flashcard_repo()?;
    let row = repo
        .update_card(
            &params.id,
            &params.front,
            &params.back,
            &params.deck,
            &params.tags.unwrap_or_default(),
            params.cloze_data.as_ref(),
            params.vocab_data.as_ref(),
        )
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

    // Fire-and-forget: re-embed updated card for semantic grading accuracy.
    if let (Some(engine), Some(vs)) = (self.embedding_engine.clone(), self.vector_store.clone())
    {
        let rows_for_embed = vec![row.clone()];
        let repo_opt = self.flashcard_repo.clone();
        tokio::spawn(async move {
            super::card_generation::embed_flashcard_batch(
                engine,
                &vs,
                &rows_for_embed,
                repo_opt.as_ref(),
            )
            .await;
        });
    }

    Ok(flashcard_to_response(row))
}
```

- [ ] **Step 2: Verify `embed_flashcard_batch` is pub**

Check that `embed_flashcard_batch` in `card_generation.rs` is `pub(crate)` or `pub`. If not, add `pub(crate)`.

- [ ] **Step 3: Build and clippy**

Run: `cargo build -p app-core && cargo clippy -p app-core --all-targets`
Expected: Compiles, zero clippy warnings.

- [ ] **Step 4: Commit**

```
git add crates/app-core/src/handlers/notes/flashcard.rs crates/app-core/src/handlers/notes/card_generation.rs
git commit -m "fix(learn): re-embed flashcard vectors on update"
```

---

### Task 4: Frontend — useReviewStats Hook + StatsBar Wiring

**Files:**
- Create: `desktop-ui/src/features/learn/hooks/useReviewStats.ts`
- Modify: `desktop-ui/src/features/learn/components/StatsBar.tsx`
- Modify: `desktop-ui/src/features/learn/components/DashboardHome.tsx`

- [ ] **Step 1: Create useReviewStats hook**

Create `desktop-ui/src/features/learn/hooks/useReviewStats.ts`:

```ts
import { useQuery } from "@shared/hooks/useQuery";

export interface WeeklyStatPoint {
  date: string;
  reviews: number;
  atomsCreated: number;
}

export interface ReviewStatsSummary {
  streak: number;
  retention: number;
  weekly: WeeklyStatPoint[];
}

export function useReviewStats() {
  return useQuery<ReviewStatsSummary>("review_stats_summary", undefined, {
    streak: 0,
    retention: 1.0,
    weekly: [],
  });
}
```

- [ ] **Step 2: Rewrite StatsBar to accept live data**

Replace `desktop-ui/src/features/learn/components/StatsBar.tsx` with:

```tsx
import { BarChart3, CheckCircle, Flame, Library } from "lucide-react";
import { Area, AreaChart, ResponsiveContainer } from "recharts";
import type { WeeklyStatPoint } from "../hooks/useReviewStats";

interface StatsBarProps {
  totalDue: number;
  streak: number;
  retention: number;
  weekly: WeeklyStatPoint[];
}

function retentionColor(r: number): string {
  if (r >= 0.8) return "text-emerald-400";
  if (r >= 0.5) return "text-amber-400";
  return "text-red-400";
}

function StatCard({
  icon,
  label,
  value,
  valueClass,
}: {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  valueClass?: string;
}) {
  return (
    <div className="glass-card flex items-center gap-2.5 px-3 py-2.5 flex-1 min-w-0">
      <div className="text-muted-foreground shrink-0">{icon}</div>
      <div className="min-w-0">
        <p className="text-[11px] text-muted-foreground leading-none mb-0.5">{label}</p>
        <p className={`text-sm font-semibold tabular-nums leading-none ${valueClass ?? "text-foreground"}`}>
          {value}
        </p>
      </div>
    </div>
  );
}

export function StatsBar({ totalDue, streak, retention, weekly }: StatsBarProps) {
  const retPct = Math.round(retention * 100);

  return (
    <div className="flex gap-2">
      <StatCard
        icon={<Flame size={16} strokeWidth={1.5} />}
        label="Streak"
        value={streak > 0 ? `${streak}d` : "--"}
      />
      <StatCard
        icon={<Library size={16} strokeWidth={1.5} />}
        label="Due"
        value={totalDue}
      />
      <StatCard
        icon={<CheckCircle size={16} strokeWidth={1.5} />}
        label="Retention"
        value={retPct > 0 && retPct < 100 ? `${retPct}%` : "--"}
        valueClass={retention < 1.0 ? retentionColor(retention) : undefined}
      />
      <div className="glass-card flex items-center gap-2.5 px-3 py-2.5 flex-1 min-w-0">
        <div className="text-muted-foreground shrink-0">
          <BarChart3 size={16} strokeWidth={1.5} />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-[11px] text-muted-foreground leading-none mb-1">This week</p>
          {weekly.length > 0 ? (
            <ResponsiveContainer width="100%" height={24}>
              <AreaChart data={weekly}>
                <Area
                  type="monotone"
                  dataKey="reviews"
                  stroke="var(--brand)"
                  fill="var(--brand)"
                  fillOpacity={0.15}
                  strokeWidth={1.5}
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-sm font-semibold text-foreground tabular-nums leading-none">--</p>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Wire into DashboardHome**

In `desktop-ui/src/features/learn/components/DashboardHome.tsx`, add the import and hook call:

```tsx
import { useReviewStats } from "../hooks/useReviewStats";
```

Inside `DashboardHome`, add after `useLearnDashboard()`:

```tsx
const { data: stats } = useReviewStats();
```

Replace `<StatsBar totalDue={totalDue} />` with:

```tsx
<StatsBar
  totalDue={totalDue}
  streak={stats.streak}
  retention={stats.retention}
  weekly={stats.weekly}
/>
```

- [ ] **Step 4: Lint and type-check**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors. Biome may auto-fix import ordering.

- [ ] **Step 5: Commit**

```
git add desktop-ui/src/features/learn/hooks/useReviewStats.ts desktop-ui/src/features/learn/components/StatsBar.tsx desktop-ui/src/features/learn/components/DashboardHome.tsx
git commit -m "feat(learn): wire StatsBar to live review stats with sparkline"
```

---

### Task 5: Frontend — Auto-Socratic on Weak Answers

**Files:**
- Modify: `desktop-ui/src/features/learn/hooks/useReviewSession.ts`
- Modify: `desktop-ui/src/features/learn/components/ImmersiveReview.tsx`

- [ ] **Step 1: Add Socratic state and auto-trigger to useReviewSession**

In `desktop-ui/src/features/learn/hooks/useReviewSession.ts`:

Add state:

```ts
const [socraticExplanation, setSocraticExplanation] = useState<string | null>(null);
const [socraticLoading, setSocraticLoading] = useState(false);
const [showSocratic, setShowSocratic] = useState(false);
```

Modify the `rate` callback — the key insight is that `rate()` sets `revealed=false` and advances the card index synchronously. The Socratic call is async and returns after the card has moved. So we use a separate `showSocratic` boolean that is NOT gated on `revealed`. The chip shows between the old card dismissal and the user engaging with the next card.

```ts
const rate = useCallback(
  async (quality: ReviewQuality) => {
    const card = cards[currentIndex];
    if (!card) return;
    const recallSpeedMs = Date.now() - cardShownAt.current;
    await ipc("flashcard_record_review", {
      cardId: card.id,
      quality,
      recallSpeedMs,
    });
    setTotalReviewed((n) => n + 1);
    if (quality !== "again") setCorrectCount((n) => n + 1);

    // Auto-trigger Socratic explanation on weak answers.
    // Set showSocratic BEFORE advancing — it persists across card transition.
    setSocraticExplanation(null);
    setShowSocratic(false);
    if (quality === "again" || quality === "hard") {
      setShowSocratic(true);
      setSocraticLoading(true);
      ipc<{ explanation: string }>("flashcard_explain_answer", {
        cardId: card.id,
        userAnswer: `(self-rated as ${quality} after seeing the answer)`,
        gradeExplanation: `Student self-assessed as '${quality}' — they may not fully understand the concept.`,
      })
        .then((r) => setSocraticExplanation(r.explanation))
        .catch(() => setShowSocratic(false))
        .finally(() => setSocraticLoading(false));
    }

    setRevealed(false);
    const nextIndex = currentIndex + 1;
    setCurrentIndex(nextIndex);
    cardShownAt.current = Date.now();
  },
  [cards, currentIndex],
);
```

Add a `dismissSocratic` callback to allow the user to close the chip:

```ts
const dismissSocratic = useCallback(() => {
  setShowSocratic(false);
  setSocraticExplanation(null);
}, []);
```

Add to return object: `socraticExplanation, socraticLoading, showSocratic, dismissSocratic`.

- [ ] **Step 2: Add Socratic chip to ImmersiveReview**

In `desktop-ui/src/features/learn/components/ImmersiveReview.tsx`:

Destructure new fields from `useReviewSession()`:

```tsx
const { ..., socraticExplanation, socraticLoading, showSocratic, dismissSocratic } = session;
```

Add state for expanding the chip:

```tsx
const [socraticOpen, setSocraticOpen] = useState(false);
```

Reset when card changes — add a `useEffect`:

```tsx
useEffect(() => {
  setSocraticOpen(false);
}, [currentIndex]);
```

After the card area `</div>` and before the bottom rating area, add. **Important:** The chip is gated on `showSocratic` (NOT `revealed`), because `rate()` sets `revealed=false` synchronously before the async Socratic call returns:

```tsx
{/* Socratic explanation */}
{showSocratic && (socraticLoading || socraticExplanation) && (
  <div className="px-6 animate-[fade-in-up_0.2s_ease-out]">
    {socraticLoading ? (
      <div className="flex items-center gap-2 text-xs text-muted-foreground justify-center py-2">
        <Lightbulb size={14} strokeWidth={1.5} className="animate-pulse" />
        Thinking...
      </div>
    ) : socraticExplanation && !socraticOpen ? (
      <button
        type="button"
        onClick={() => setSocraticOpen(true)}
        className="mx-auto flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors py-2"
      >
        <Lightbulb size={14} strokeWidth={1.5} />
        Let's understand why
      </button>
    ) : socraticExplanation ? (
      <div className="glass-card p-4 text-sm text-foreground whitespace-pre-wrap animate-[fade-in-up_0.2s_ease-out]">
        {socraticExplanation}
        <button
          type="button"
          onClick={dismissSocratic}
          className="block mt-2 text-xs text-muted-foreground hover:text-foreground"
        >
          Dismiss
        </button>
      </div>
    ) : null}
  </div>
)}
```

Remove the disabled "Ask AI" `<button>` from the footer actions section.

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 4: Commit**

```
git add desktop-ui/src/features/learn/hooks/useReviewSession.ts desktop-ui/src/features/learn/components/ImmersiveReview.tsx
git commit -m "feat(learn): auto-trigger Socratic explanation on weak answers"
```

---

### Task 6: Frontend — Inline Card Editor Component

**Files:**
- Create: `desktop-ui/src/features/learn/components/CardEditor.tsx`

- [ ] **Step 1: Create CardEditor component**

Create `desktop-ui/src/features/learn/components/CardEditor.tsx`:

```tsx
import { ipc } from "@shared/hooks/useIpc";
import { Check, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Flashcard } from "../../notes/hooks/useFlashcards";

interface CardEditorProps {
  card: Flashcard;
  onSaved: (updated: { front: string; back: string; deck: string }) => void;
  onCancel: () => void;
}

export function CardEditor({ card, onSaved, onCancel }: CardEditorProps) {
  const [front, setFront] = useState(card.front);
  const [back, setBack] = useState(card.back);
  const [deck, setDeck] = useState(card.deck);
  const [saving, setSaving] = useState(false);
  const frontRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    frontRef.current?.focus();
  }, []);

  const handleSave = useCallback(async () => {
    if (!front.trim()) return;
    setSaving(true);
    try {
      await ipc("flashcard_update", {
        id: card.id,
        front: front.trim(),
        back: back.trim(),
        deck: deck.trim() || card.deck,
      });
      onSaved({ front: front.trim(), back: back.trim(), deck: deck.trim() || card.deck });
    } catch {
      setSaving(false);
    }
  }, [card.id, card.deck, front, back, deck, onSaved]);

  // Escape to cancel, Cmd+Enter to save
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onCancel();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        handleSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onCancel, handleSave]);

  return (
    <div className="w-full max-w-lg space-y-4">
      <div className="glass-card p-6 space-y-4">
        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Front</span>
          <textarea
            ref={frontRef}
            value={front}
            onChange={(e) => setFront(e.target.value)}
            rows={3}
            className="glass-input w-full px-3 py-2 text-sm text-foreground resize-none"
          />
        </label>
        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Back</span>
          <textarea
            value={back}
            onChange={(e) => setBack(e.target.value)}
            rows={3}
            className="glass-input w-full px-3 py-2 text-sm text-foreground resize-none"
          />
        </label>
        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Deck</span>
          <input
            type="text"
            value={deck}
            onChange={(e) => setDeck(e.target.value)}
            className="glass-input w-full px-3 py-1.5 text-sm text-foreground"
          />
        </label>
      </div>
      <div className="flex items-center justify-center gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="glass-button px-3 py-1.5 text-sm text-muted-foreground flex items-center gap-1"
        >
          <X size={14} strokeWidth={1.5} />
          Cancel
          <span className="text-2xs text-muted-foreground ml-1">Esc</span>
        </button>
        <button
          type="button"
          onClick={handleSave}
          disabled={!front.trim() || saving}
          className="glass-button px-4 py-1.5 text-sm text-foreground flex items-center gap-1 disabled:opacity-40"
        >
          <Check size={14} strokeWidth={1.5} />
          {saving ? "Saving..." : "Save"}
          <span className="text-2xs text-muted-foreground ml-1">{"\u2318"}↩</span>
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 3: Commit**

```
git add desktop-ui/src/features/learn/components/CardEditor.tsx
git commit -m "feat(learn): add CardEditor component for inline editing"
```

---

### Task 7: Frontend — Wire Inline Editing into ImmersiveReview

**Files:**
- Modify: `desktop-ui/src/features/learn/components/ImmersiveReview.tsx`

- [ ] **Step 1: Add editing state and wire CardEditor**

In `ImmersiveReview.tsx`, add imports:

```tsx
import { CardEditor } from "./CardEditor";
```

Add state:

```tsx
const [editing, setEditing] = useState(false);
```

Add `E` key handler in the existing keyboard effect — inside the handler, before the rating key map:

```tsx
if (e.key === "e" || e.key === "E") {
  if (revealed && current && !editing) {
    e.preventDefault();
    setEditing(true);
    return;
  }
}
```

In the card area, replace the card rendering with a conditional:

```tsx
{current && (
  <div className="w-full max-w-lg">
    {editing ? (
      <CardEditor
        card={current}
        onSaved={(updated) => {
          // Update card in local array
          const newCards = [...cards];
          newCards[currentIndex] = {
            ...current,
            front: updated.front,
            back: updated.back,
            deck: updated.deck,
          };
          // Note: useReviewSession doesn't expose setCards directly,
          // so we need to add a mutation method or just continue with current card.
          setEditing(false);
        }}
        onCancel={() => setEditing(false)}
      />
    ) : (
      <div className="glass-card p-8">
        <CardRenderer card={current} revealed={revealed} />
      </div>
    )}
  </div>
)}
```

Enable the "Edit" button in the footer — replace the disabled edit button with:

```tsx
<button
  type="button"
  onClick={() => setEditing(true)}
  disabled={!revealed || !current}
  className={`flex items-center gap-1 text-[11px] transition-colors ${
    revealed && current
      ? "text-muted-foreground hover:text-foreground cursor-pointer"
      : "text-muted-foreground opacity-50 cursor-not-allowed"
  }`}
>
  <Edit3 size={12} strokeWidth={1.5} />
  Edit
  <span className="text-2xs text-muted-foreground ml-0.5">E</span>
</button>
```

Reset editing state when moving to next card — add `setEditing(false)` at the start of `rate` or in a `useEffect` on `currentIndex`.

- [ ] **Step 2: Add `updateCard` method to useReviewSession**

In `useReviewSession.ts`, add:

```ts
const updateCard = useCallback(
  (index: number, updated: Partial<Flashcard>) => {
    setCards((prev) => prev.map((c, i) => (i === index ? { ...c, ...updated } : c)));
  },
  [],
);
```

Return it from the hook: `updateCard`.

Use it in `ImmersiveReview` on save:

```tsx
onSaved={(updated) => {
  session.updateCard(currentIndex, updated);
  setEditing(false);
}}
```

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 4: Commit**

```
git add desktop-ui/src/features/learn/components/ImmersiveReview.tsx desktop-ui/src/features/learn/hooks/useReviewSession.ts
git commit -m "feat(learn): wire inline card editing with E shortcut"
```

---

### Task 8: Frontend — Dashboard Charts (RetentionChart + AtomGraph)

**Files:**
- Create: `desktop-ui/src/features/learn/components/CollapsibleSection.tsx`
- Modify: `desktop-ui/src/features/learn/components/DashboardHome.tsx`
- Modify: `desktop-ui/src/features/learn/components/AtomGraph.tsx`

- [ ] **Step 1: Create CollapsibleSection component**

Create `desktop-ui/src/features/learn/components/CollapsibleSection.tsx`:

```tsx
import { ChevronDown } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useState } from "react";

interface CollapsibleSectionProps {
  title: string;
  icon: ReactNode;
  storageKey: string;
  defaultOpen?: boolean;
  children: ReactNode;
}

export function CollapsibleSection({
  title,
  icon,
  storageKey,
  defaultOpen = false,
  children,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(() => {
    const stored = localStorage.getItem(storageKey);
    return stored !== null ? stored === "true" : defaultOpen;
  });

  useEffect(() => {
    localStorage.setItem(storageKey, String(open));
  }, [storageKey, open]);

  const toggle = useCallback(() => setOpen((o) => !o), []);

  return (
    <div className="space-y-0">
      <button
        type="button"
        onClick={toggle}
        className="w-full flex items-center gap-2 px-1 py-2 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
      >
        <span className="shrink-0">{icon}</span>
        {title}
        <ChevronDown
          size={14}
          strokeWidth={1.5}
          className={`ml-auto transition-transform duration-200 ${open ? "rotate-180" : ""}`}
        />
      </button>
      <div
        className={`grid transition-[grid-template-rows] duration-300 ease-out ${
          open ? "grid-rows-[1fr]" : "grid-rows-[0fr]"
        }`}
      >
        <div className="overflow-hidden">{children}</div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add retention-based coloring to AtomGraph**

In `desktop-ui/src/features/learn/components/AtomGraph.tsx`, modify the node style to use retention-based colors:

Replace the fixed `"background-color": "var(--brand)"` in the cytoscape style with a function-based color. Since cytoscape's style doesn't support functions inline, set color per-node in the data:

```tsx
const nodes = health.topics.map((t) => ({
  data: {
    id: t.id,
    label: t.name,
    size: Math.max(20, t.atomCount * 4),
    retention: t.avgRetention,
    color: t.avgRetention >= 0.8 ? "#34d399" : t.avgRetention >= 0.5 ? "#fbbf24" : "#f87171",
  },
}));
```

And in the style block, use `"data(color)"`:

```tsx
"background-color": "data(color)",
"border-color": "data(color)",
```

- [ ] **Step 3: Mount charts on DashboardHome**

In `desktop-ui/src/features/learn/components/DashboardHome.tsx`:

Add imports:

```tsx
import { Activity, Focus, GraduationCap, Network, Play, Plus, TrendingUp } from "lucide-react";
import { useRetentionHistory } from "../hooks/useRetentionHistory";
import { AtomGraph } from "./AtomGraph";
import { CollapsibleSection } from "./CollapsibleSection";
import { RetentionChart } from "./RetentionChart";
```

Inside the populated branch of `DashboardHome`, add hooks after existing ones:

```tsx
const { data: retentionData } = useRetentionHistory(30);
```

After `<DeckList ... />`, add:

```tsx
{/* Charts */}
<CollapsibleSection
  title="Retention Trend"
  icon={<TrendingUp size={14} strokeWidth={1.5} />}
  storageKey="learn-retention-open"
>
  <div className="glass-card p-4">
    <RetentionChart data={retentionData.overall} height={160} />
  </div>
</CollapsibleSection>

<CollapsibleSection
  title="Knowledge Graph"
  icon={<Network size={14} strokeWidth={1.5} />}
  storageKey="learn-graph-open"
>
  <div className="glass-card p-4">
    <AtomGraph />
  </div>
</CollapsibleSection>
```

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 5: Commit**

```
git add desktop-ui/src/features/learn/components/CollapsibleSection.tsx desktop-ui/src/features/learn/components/DashboardHome.tsx desktop-ui/src/features/learn/components/AtomGraph.tsx
git commit -m "feat(learn): mount RetentionChart and AtomGraph on dashboard"
```

---

### Task 9: Backend — Recent Learning Sessions Handler + Command

**Files:**
- Create: `crates/app-core/src/handlers/notes/recent_learning.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs` (register submodule)
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add IPC type)
- Modify: `crates/desktop/src/commands/notes.rs` (add command + DEV_COMMANDS + dispatch_dev)
- Modify: `crates/desktop/src/main.rs` (register command)

- [ ] **Step 1: Add IPC type**

In `crates/desktop-shared/src/commands/notes.rs`, add near the flashcard types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentLearningSession {
    pub session_key: String,
    pub title: String,
    pub updated_at: String, // RFC 3339 timestamp (project convention)
    pub preview: String,
}
```

- [ ] **Step 2: Create handler**

Create `crates/app-core/src/handlers/notes/recent_learning.rs`:

```rust
use desktop_shared::commands::RecentLearningSession;
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    pub async fn flashcard_recent_learning_sessions(
        &self,
        limit: usize,
    ) -> Result<Vec<RecentLearningSession>, ApiError> {
        let mut sessions = self
            .repos
            .sessions
            .list_sessions()
            .await
            .map_err(map_storage_err)?;

        // Sort by updated_at descending, take first `limit`
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions.truncate(limit);

        let mut result = Vec::with_capacity(sessions.len());
        for s in &sessions {
            let messages = self
                .repos
                .sessions
                .get_recent_messages(&s.key, 10)
                .await
                .unwrap_or_default();

            let preview: String = messages
                .iter()
                .filter(|m| m.role == "user" || m.role == "assistant")
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            let title = s
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            result.push(RecentLearningSession {
                session_key: s.key.clone(),
                title,
                updated_at: chrono::DateTime::from_timestamp(s.updated_at, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                preview: preview.chars().take(200).collect(),
            });
        }

        Ok(result)
    }
}
```

Register in `crates/app-core/src/handlers/notes/mod.rs` — add `pub mod recent_learning;`.

- [ ] **Step 3: Add Tauri command**

In `crates/desktop/src/commands/notes.rs`:

Add the command function (near other flashcard commands):

```rust
#[tauri::command]
pub async fn flashcard_recent_learning_sessions(
    state: State<'_, Arc<AppCore>>,
    limit: Option<usize>,
) -> Result<Vec<desktop_shared::commands::RecentLearningSession>, ApiError> {
    state
        .flashcard_recent_learning_sessions(limit.unwrap_or(3))
        .await
}
```

Add `"flashcard_recent_learning_sessions"` to `DEV_COMMANDS`.

Add dispatch case:

```rust
"flashcard_recent_learning_sessions" => {
    let limit = body.get("limit").and_then(|v| v.as_u64()).map(|l| l as usize);
    dev::val(core.flashcard_recent_learning_sessions(limit.unwrap_or(3)).await)
}
```

- [ ] **Step 4: Register in main.rs**

Add `commands::notes::flashcard_recent_learning_sessions,` to the flashcard section in `main.rs` (~line 459).

- [ ] **Step 5: Build and verify registration**

Run: `cargo build -p desktop && cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: Build succeeds. Coverage test passes.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p desktop -p app-core --all-targets`
Expected: Zero warnings.

- [ ] **Step 7: Commit**

```
git add crates/app-core/src/handlers/notes/recent_learning.rs crates/app-core/src/handlers/notes/mod.rs crates/desktop-shared/src/commands/notes.rs crates/desktop/src/commands/notes.rs crates/desktop/src/main.rs
git commit -m "feat(learn): add flashcard_recent_learning_sessions command"
```

---

### Task 10: Frontend — Recent Conversations Picker in QuickGenerate

**Files:**
- Modify: `desktop-ui/src/features/learn/components/QuickGenerate.tsx`

- [ ] **Step 1: Replace "From last chat" with recent conversations mode**

In `desktop-ui/src/features/learn/components/QuickGenerate.tsx`:

Change the `QuickGenMode` type:

```ts
type QuickGenMode = null | "note" | "clipboard" | "conversations";
```

Add the conversations mode rendering (after the clipboard mode block):

```tsx
if (mode === "conversations") {
  return <ConversationPicker onSelect={handleSelectConversation} onCancel={() => setMode(null)} />;
}
```

Replace the disabled "From last chat..." button with:

```tsx
<button
  type="button"
  onClick={() => setMode("conversations")}
  className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-left"
>
  <MessageSquare size={14} strokeWidth={1.5} />
  From recent conversations...
</button>
```

Add the `ConversationPicker` as a local component in the same file (or extract if large):

```tsx
function ConversationPicker({
  onSelect,
  onCancel,
}: {
  onSelect: (text: string) => void;
  onCancel: () => void;
}) {
  const [sessions, setSessions] = useState<
    { sessionKey: string; title: string; updatedAt: string; preview: string }[]
  >([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    ipc<typeof sessions>("flashcard_recent_learning_sessions", { limit: 3 })
      .then(setSessions)
      .catch(() => setSessions([]))
      .finally(() => setLoading(false));
  }, []);

  const handleSelect = useCallback(
    async (sessionKey: string) => {
      const messages = await ipc<{ role: string; content: string }[]>("chat_messages", {
        sessionKey,
        limit: 50,
      });
      const text = messages
        .filter((m) => m.role === "user" || m.role === "assistant")
        .map((m) => m.content)
        .join("\n\n");
      onSelect(text);
    },
    [onSelect],
  );

  if (loading) {
    return (
      <div className="glass-card p-4 text-center">
        <p className="text-xs text-muted-foreground">Loading conversations...</p>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="glass-card p-4 space-y-2">
        <p className="text-xs text-muted-foreground">No recent conversations found.</p>
        <button
          type="button"
          onClick={onCancel}
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          Back
        </button>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 space-y-2">
      <p className="text-xs text-muted-foreground mb-2">Select a conversation:</p>
      {sessions.map((s) => (
        <button
          key={s.sessionKey}
          type="button"
          onClick={() => handleSelect(s.sessionKey)}
          className="w-full text-left px-3 py-2 rounded-lg hover:bg-accent transition-colors"
        >
          <p className="text-sm font-medium text-foreground truncate">{s.title}</p>
          <p className="text-[11px] text-muted-foreground truncate mt-0.5">{s.preview}</p>
        </button>
      ))}
      <button
        type="button"
        onClick={onCancel}
        className="text-xs text-muted-foreground hover:text-foreground mt-1"
      >
        Cancel
      </button>
    </div>
  );
}
```

Wire the handler:

```tsx
const handleSelectConversation = useCallback(
  (text: string) => {
    setMode(null);
    onGenerateFromText(text);
  },
  [onGenerateFromText],
);
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 3: Commit**

```
git add desktop-ui/src/features/learn/components/QuickGenerate.tsx
git commit -m "feat(learn): add recent conversations picker for flashcard generation"
```

---

### Task 11: Final — Full Build + Lint + Test Pass

**Files:** None (verification only)

- [ ] **Step 1: Full Rust build**

Run: `cargo build --workspace`
Expected: Compiles with zero errors.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 3: Rust tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 5: Frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

- [ ] **Step 6: Format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.
