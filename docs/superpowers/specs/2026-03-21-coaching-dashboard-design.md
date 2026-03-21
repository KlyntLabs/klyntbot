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

**New table:** `coaching_intervention_log` in the cognitive migration.

```sql
CREATE TABLE IF NOT EXISTS coaching_intervention_log (
    id TEXT PRIMARY KEY,
    intervention_type TEXT NOT NULL,
    message TEXT NOT NULL,
    trigger_name TEXT NOT NULL,
    feedback TEXT,  -- 'helpful' | 'dismissed' | 'ignored' | NULL
    delivered_at TEXT NOT NULL,
    feedback_at TEXT
);
```

**New repo:** `CoachingInterventionLogRepo` in `crates/storage/` with:
- `insert(intervention)` — called from `FeedbackTracker::record_delivery()`
- `update_feedback(id, feedback, feedback_at)` — called from `coaching_submit_feedback` handler
- `list_recent(limit)` — returns recent interventions with feedback status

**New command:** `coaching_intervention_log` — returns `Vec<InterventionLogResponse>` with fields: `id`, `interventionType`, `message`, `triggerName`, `feedback`, `deliveredAt`, `feedbackAt`.

**Modified handler:** `coaching_submit_feedback` must also write to `coaching_intervention_log` via `update_feedback()`, enabling retroactive feedback on expired interventions (not just pending ones).

**Modified service:** `FeedbackTracker::record_delivery()` must also persist to `coaching_intervention_log` via `insert()`.

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

**Polling:** The Overview health card and History tab should use a 5-second polling interval (matching existing `useCoachingNudge`) since receptivity and rate limit data changes in real-time.

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

**Retroactive feedback:** For interventions where `feedback` is null or "ignored", show inline **Helpful** / **Dismiss** buttons. Clicking calls `coaching_submit_feedback(id, response)` which writes to both the in-memory tracker and `coaching_intervention_log`. Row updates optimistically.

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
| `crates/storage/src/repos/mod.rs` | Register new repo module |
| `crates/desktop-shared/src/cognitive_commands.rs` | Add `InterventionLogResponse` struct |
| `crates/desktop/src/commands/cognitive.rs` | Add `coaching_intervention_log` command + DEV_COMMANDS + dispatch_dev |
| `crates/app-core/src/handlers/coaching.rs` | Add `coaching_intervention_log()` handler; modify `coaching_submit_feedback()` to persist feedback |
| `crates/feature-coaching/src/feedback.rs` | Modify `record_delivery()` to persist to intervention log |
| Cognitive migration SQL | Add `coaching_intervention_log` table |

---

## UI Conventions

- Follow `glass-card` pattern for all cards
- Tab buttons use `glass-button-active` for selected state (matching `FinanceLayout`)
- Timestamps displayed via `formatTime()` from `@shared/lib/dates.ts`
- Feedback badges use semantic colors: green (helpful), orange (dismissed), gray (ignored), neutral/dim (no feedback)
- Data fetching via `useQuery(cmd, args)` with 5s polling for real-time data (health card, history)
- Mutations via `useMutation(cmd)` with optimistic updates on feedback submission
- Empty states for all tabs when no data is available
- Loading states via standard `useQuery` `isLoading` pattern
