# Coaching Dashboard — Design Spec

> **Date:** 2026-03-21
> **Scope:** Dedicated coaching page with tabbed layout (Overview, Patterns, History) — surfacing behavioral patterns, intervention history, and retroactive feedback.
> **Goal:** Give users visibility into coaching system behavior and let them correct feedback on past interventions.

---

## Problem Statement

The coaching backend is fully operational — signal accumulation, pattern detection, LLM/heuristic reasoning, intervention routing with rate limiting, exponential dismiss backoff, and a three-channel feedback loop. However, all of this is invisible to the user:

- **Nudge banners** in chat, tray, and overview are the only surfaces — they show the latest intervention and auto-collapse after 60 seconds
- **Detected patterns** (e.g., post-meeting context switching, energy dips) are never shown to the user
- **Intervention history** is not reviewable — once a nudge collapses, the user can't see what was said or change their feedback
- **Coaching health** (receptivity score, rate limits) is invisible

Users have no way to understand *why* they're being nudged, *what patterns* the system has detected, or correct an "ignored" nudge that was actually useful.

## Out of Scope

- **Manual coaching preferences** (e.g., "never nudge about breaks") — the system already self-tunes via dismissal backoff and receptivity adjustments. Adding manual controls would create competing tuning mechanisms.
- **Pattern editing/deletion** — patterns are system-detected; users observe them, not manage them.
- **Coaching settings/configuration** — config fields (`productivity.coaching.*`) can be exposed later via the settings UI gap.

---

## Architecture

### Page Structure

Tabbed page at `/coaching` following the `FinanceLayout` pattern (flat children, not nested `<Outlet />`):

```
CoachingLayout (flex-1 flex-col gap-2)
├── Top toolbar (h-12, glass tabs)
│   ├── Tab: "Overview"  → /coaching
│   ├── Tab: "Patterns"  → /coaching/patterns
│   └── Tab: "History"   → /coaching/history
└── Content area (flex-1 overflow-y-auto p-4)
    └── {children} (passed as prop, not Outlet)
```

Each sub-route is a separate top-level route in `router.tsx` wrapping its page in `<CoachingLayout>`, matching the finance pattern:

```tsx
{ path: "/coaching", element: <CoachingLayout><OverviewPage /></CoachingLayout> },
{ path: "/coaching/patterns", element: <CoachingLayout><PatternsPage /></CoachingLayout> },
{ path: "/coaching/history", element: <CoachingLayout><HistoryPage /></CoachingLayout> },
```

### Navigation Entry Points

1. **Left sidebar** — new coaching icon in the navigation, grouped near productivity. Requires adding `"Coaching"` to the `SidebarItem` union in `shared/types/common.ts` and a `/coaching` branch to `activeSidebarItem` in `AppShell.tsx`.
2. **Dashboard SummaryPanel** — existing `CoachingCard` becomes clickable, navigates to `/coaching`.

### Backend Changes Required

The History tab requires **persistent intervention history**. Currently, `coaching_pending_interventions` only returns in-memory pending interventions that expire after 10 minutes. Once expired, interventions and their feedback status are lost.

**New table:** `coaching_intervention_log` — append to `crates/cognitive/migrations/001_cognitive_tables.sql` (pre-release, in-place update per CLAUDE.md convention) and bump the `FeatureMigration` version from 9 to 10 in `crates/cognitive/src/repos/mod.rs`.

```sql
CREATE TABLE IF NOT EXISTS coaching_intervention_log (
    id TEXT PRIMARY KEY,
    intervention_type TEXT NOT NULL,
    message TEXT NOT NULL,
    trigger_name TEXT NOT NULL,
    feedback TEXT,  -- 'helpful' | 'dismissed' | 'stop' | 'ignored' | NULL
    delivered_at TEXT NOT NULL,
    feedback_at TEXT
);
```

**New repo:** `CoachingInterventionLogRepo` in `crates/storage/src/repos/` with:
- `insert(intervention)` — async, called after releasing the `FeedbackTracker` mutex lock
- `update_feedback(id, feedback, feedback_at)` — called from `coaching_submit_feedback` handler
- `list_recent(limit)` — returns recent interventions with feedback status

Must also be registered in `Repos` aggregate struct (`crates/storage/src/repos/mod.rs`) and wired into `AppCore` state via `init_coaching()` in `crates/app-core/src/init/coaching.rs`.

**New command:** `coaching_intervention_log` — returns `Vec<InterventionLogResponse>`. Must be added to `DEV_COMMANDS` (gated `#[cfg(test)]`) and `dispatch_dev` (gated `#[cfg(debug_assertions)]`) in `crates/desktop/src/commands/cognitive.rs`.

**New DTO:** `InterventionLogResponse` in `crates/desktop-shared/src/cognitive_commands.rs` with `#[serde(rename_all = "camelCase")]`:

```rust
pub struct InterventionLogResponse {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub feedback: Option<String>,      // "helpful" | "dismissed" | "stop" | "ignored" | None
    pub delivered_at: String,          // serializes as deliveredAt
    pub feedback_at: Option<String>,   // serializes as feedbackAt
}
```

**Modified handler: `coaching_submit_feedback`** — two changes:
1. Write feedback to `coaching_intervention_log` via `repo.update_feedback()` — this is the primary persistence path for retroactive feedback.
2. For retroactive feedback on expired interventions (where `pending_behavioral` lookup returns `None`), fall through to a direct DB write rather than silently no-oping. The in-memory `record_explicit` call remains for live interventions, but the DB write must happen regardless.

Wire values passed by the frontend must be exact lowercase strings: `"helpful"`, `"dismissed"`, or `"stop"`. The handler rejects anything else with `INVALID_RESPONSE`.

**Modified handler: `coaching_report_ignored`** — also write `feedback = 'ignored'` to `coaching_intervention_log` so the History tab can distinguish ignored from no-feedback.

**Modified service: `CoachingService` delivery path** — `FeedbackTracker` is held behind `tokio::Mutex`, so the lock is async-safe. After calling `record_delivery()` and dropping the lock, issue the async `repo.insert()` call directly in the same task context — no `tokio::spawn` needed. The pattern is: `{ let mut fb = feedback.lock().await; fb.record_delivery(&intervention); }` then `repo.insert(&intervention).await;` outside the lock scope.

### Data Flow

Frontend data fetching via `useQuery`/`useMutation`:

| Command | Returns | Used In |
|---------|---------|---------|
| `coaching_situation` | `UserSituationResponse` | Overview health card (receptivity gauge) |
| `coaching_router_status` | `RouterStatusResponse` | Overview health card (rate limits) |
| `coaching_patterns` | `Vec<DetectedPatternResponse>` | Overview preview + Patterns tab |
| `coaching_feedback_stats` | `Vec<StrategyFeedbackResponse>` | Overview health card (strategy effectiveness) |
| `coaching_intervention_log` | `Vec<InterventionLogResponse>` | Overview preview + History tab |
| `coaching_submit_feedback` | `bool` | History tab retroactive feedback |

**Polling:** `coaching_situation` should use a 5-second polling interval (matching existing `useCoachingNudge`) since receptivity changes frequently. Other queries (`coaching_router_status`, `coaching_patterns`, `coaching_intervention_log`, `coaching_feedback_stats`) can use the default 30s stale time — they change infrequently.

---

## Tab Designs

### Overview Tab (`/coaching`)

Three rows of glass-cards providing an at-a-glance summary:

**Row 1 — Coaching Health (single wide card)**
- Receptivity score (0–1 from `coaching_situation → coachingReceptivity`) displayed as a circular gauge or arc
- Today's intervention counts vs rate limits (e.g., "2 / 3 hourly, 4 / 5 daily") from `coaching_router_status`
- Strategy effectiveness summary from `coaching_feedback_stats` — e.g., acceptance rate across strategies

**Row 2 — Recent Patterns (preview, 2–3 cards)**
- Most recent detected patterns as small cards
- Each shows: `name`, `confidence` (as percentage), `signalCount`, `description`
- "View all" link navigates to `/coaching/patterns`

**Row 3 — Recent Interventions (preview, 3–5 items)**
- Last interventions from `coaching_intervention_log` as a compact list
- Each shows: message preview, `deliveredAt` timestamp, `feedback` status badge
- "View all" link navigates to `/coaching/history`

### Patterns Tab (`/coaching/patterns`)

Grid of pattern cards, sorted by confidence (descending). Each card contains:

- **Name** (`name` field)
- **Description** (`description` field) — human-readable explanation
- **Domain** (`domain` field) — e.g., "productivity", "focus"
- **Confidence** (`confidence` field, 0–1) — displayed as percentage or bar
- **Signal count** (`signalCount` field) — number of supporting signals

Read-only — patterns are system-detected, not user-managed.

**Empty state:** "No patterns detected yet. Patterns emerge as the coaching system observes your work habits over time."

### History Tab (`/coaching/history`)

Chronological list of past interventions from `coaching_intervention_log`, most recent first. Each row contains:

- **Intervention message** — the coaching text that was displayed
- **Type badge** — `ChatMessage` | `DashboardCard` | `Notification` | `Overlay`
- **Delivered at** — timestamp via `formatTime()`
- **Trigger** — what caused it (e.g., "distraction_streak")
- **Feedback status** — color-coded badge:
  - Helpful (green)
  - Dismissed (orange)
  - Ignored (gray — auto-collapsed without interaction)
  - No feedback (neutral — `feedback` is null)

**Retroactive feedback:** For interventions where `feedback` is null or `"ignored"`, show inline **Helpful** / **Dismiss** buttons. Clicking calls `coaching_submit_feedback(id, "helpful")` or `coaching_submit_feedback(id, "dismissed")` — wire values must be exact lowercase strings. The handler writes to both the in-memory tracker (if still pending) and `coaching_intervention_log` (always). Row updates optimistically.

**Empty state:** "No coaching interventions yet. The system will start offering suggestions as it learns your patterns."

---

## File Structure

```
desktop-ui/src/features/coaching/
├── index.ts                          # Public exports (all pages)
├── components/
│   ├── CoachingLayout.tsx            # Tabbed layout (mirrors FinanceLayout, children prop)
│   ├── CoachingHealthCard.tsx        # Receptivity gauge + rate limits
│   ├── PatternCard.tsx               # Single pattern display card
│   ├── InterventionRow.tsx           # Single intervention with feedback controls
│   └── FeedbackButtons.tsx           # Retroactive Helpful/Dismiss buttons
└── pages/
    ├── OverviewPage.tsx              # /coaching — summary with previews
    ├── PatternsPage.tsx              # /coaching/patterns — pattern grid
    └── HistoryPage.tsx               # /coaching/history — intervention list
```

**Modifications to existing files:**

| File | Change |
|------|--------|
| `app/router.tsx` | Add 3 `/coaching` routes with `CoachingLayout` wrapper |
| `app/layouts/Sidebar.tsx` | Add coaching icon + nav entry |
| `app/layouts/AppShell.tsx` | Add `/coaching` branch to `activeSidebarItem` memo |
| `shared/types/common.ts` | Add `"Coaching"` to `SidebarItem` union |
| `features/dashboard/components/SummaryPanel.tsx` | Make `CoachingCard` clickable → `navigate("/coaching")` |

**New backend files:**

| File | Change |
|------|--------|
| `crates/storage/src/repos/coaching_intervention_log.rs` | New repo for persistent intervention history |
| `crates/storage/src/repos/mod.rs` | Register new repo module + add to `Repos` aggregate struct |
| `crates/desktop-shared/src/cognitive_commands.rs` | Add `InterventionLogResponse` struct |
| `crates/desktop/src/commands/cognitive.rs` | Add `coaching_intervention_log` command + `DEV_COMMANDS` (`#[cfg(test)]`) + `dispatch_dev` (`#[cfg(debug_assertions)]`) |
| `crates/app-core/src/handlers/coaching.rs` | Add `coaching_intervention_log()` handler; modify `coaching_submit_feedback()` for DB fallback; modify `coaching_report_ignored()` to persist |
| `crates/app-core/src/init/coaching.rs` | Wire `CoachingInterventionLogRepo` into `AppCore` state |
| `crates/feature-coaching/src/service.rs` | Add `repo.insert()` call after `record_delivery()` lock scope |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Append `coaching_intervention_log` table, bump migration version |

---

## UI Conventions

- Follow `glass-card` pattern for all cards
- Tab buttons use `glass-button-active` for selected state (matching `FinanceLayout`)
- Timestamps displayed via `formatTime()` from `@shared/lib/dates.ts`
- Feedback badges use semantic colors: green (helpful), orange (dismissed), gray (ignored), neutral/dim (no feedback)
- Data fetching via `useQuery(cmd, args)` — 5s polling for `coaching_situation` only; default 30s stale time for all other queries
- Mutations via `useMutation(cmd)` with optimistic updates on feedback submission
- Empty states for all tabs when no data is available
- Loading states via standard `useQuery` `isLoading` pattern
