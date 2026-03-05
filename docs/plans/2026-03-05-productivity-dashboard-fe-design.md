# Productivity Dashboard Frontend Design

**Goal:** Redesign the productivity page from a single-column scroll into a Rize.io-inspired dense dashboard with Day/Week/Month period views, 3-column grid layout, and rich data visualization.

**Dependencies:** Requires the backend productivity upgrade (see `2026-03-05-productivity-upgrade-plan.md`) which adds productivity score, goals, time entries, pomodoro, compare, and export.

---

## Routing

```
/productivity                    -> redirect to /productivity/day/:today
/productivity/day/:date          -> Day view (e.g. /productivity/day/2026-03-05)
/productivity/week/:weekStart    -> Week view (e.g. /productivity/week/2026-03-03)
/productivity/month/:yearMonth   -> Month view (e.g. /productivity/month/2026-03)
```

URL-based date/period state — survives navigation, consistent with Finance sub-routes.

---

## Layout

### ProductivityLayout (wrapper)

Follows the `FinanceLayout` pattern:
- Left: shared `Sidebar` (active: Productivity)
- Top bar: period tabs (Day | Week | Month) + `DateNavigator` (`< March 5, 2026 >`)
- Content area: renders child view

### Day View (3-column grid)

```
┌─────────────────────────────────────────────────────────────────┐
│  Day  Week  Month    Thursday, March 5, 2026       < cal >     │
├─────────────────────────────────────────────────────────────────┤
│  TIMELINE (full-width, spanning all 3 columns)                 │
│  6am  7  8  9  10  11  12pm  1  2  3  4  5  6  7  8  9pm      │
├────────────────────┬────────────────────┬───────────────────────┤
│  POMODORO/BREAK    │   SESSIONS         │  WORK HOURS           │
│  TIMER             │   list with times  │  7h 33m  94.5%        │
│  3:01:54           │   and durations    │                       │
│  Start Break (5m)  │                    │  PRODUCTIVITY SCORE   │
│                    │                    │  ring gauge 78/100    │
├────────────────────┤                    ├───────────────────────┤
│  ACTIVITY FEED     │                    │  BREAKDOWN            │
│  18:48 Chrome      │                    │  Focus  69%  donut   │
│  18:47 Slack       │                    │  Meetings 18%        │
│  18:46 VS Code     │                    │  Breaks   6%         │
│                    │                    ├───────────────────────┤
│                    │                    │  CATEGORIES           │
│                    │                    │  51% Video Conf 3h40m │
│                    │                    │  9%  Twitter    37m   │
├────────────────────┴────────────────────┤                      │
│  GOALS PROGRESS                         ├───────────────────────┤
│  4h productive (3.2/4.0) MET            │  AI SUMMARY           │
│  3 focus sessions (2/3) IN PROGRESS     │  "Great focus day..." │
└─────────────────────────────────────────┴───────────────────────┘
```

Grid: `grid-cols-3` with cards spanning as needed via `col-span-*`.

### Week View

```
┌─────────────────────────────────────────────────────────────────┐
│  Day  Week  Month    Mar 3 - Mar 9, 2026           < cal >     │
├─────────────────────────────────────────────────────────────────┤
│  STACKED BAR CHART (Recharts)                                   │
│  hours/day, colored by productive/neutral/distracting           │
│  Mon  Tue  Wed  Thu  Fri  Sat  Sun                             │
├────────────────────┬────────────────────┬───────────────────────┤
│  WEEKLY STATS      │  BREAKDOWN DONUTS  │  TOP CATEGORIES       │
│  Avg score: 74     │  Focus 62%         │  Coding    18h 20m    │
│  Total active: 42h │  Meetings 22%      │  Video     8h 10m     │
│  Avg daily: 6h     │  Breaks 8%         │  ...                  │
├────────────────────┼────────────────────┼───────────────────────┤
│  TOP APPS          │  GOALS WEEKLY      │  FOCUS SESSIONS       │
│  VS Code  22h      │  20h productive OK │  12 sessions          │
│  Chrome   8h       │  15 focus sess WIP │  Avg quality: 82%     │
└────────────────────┴────────────────────┴───────────────────────┘
```

### Month View

```
┌─────────────────────────────────────────────────────────────────┐
│  Day  Week  Month    March 2026                    < cal >     │
├─────────────────────────────────────────────────────────────────┤
│  STACKED BAR CHART (Recharts) - daily bars for full month       │
│  1  2  3  4  5 ... 28  29  30  31                              │
├────────────────────┬────────────────────┬───────────────────────┤
│  WORK CATEGORIES   │  BREAKDOWN         │  WORK HOURS           │
│  25% Web Dev 41h   │  Focus 56%         │  Avg/week: 44h 39m    │
│  12% Video   20h   │    last mo: 62%    │  Avg/day:  7h 44m     │
│  7%  Comms   11h   │    change: -6%     │    last mo: 6h 52m    │
│  ...               │  Meetings 13%      │    change: +52m       │
│                    │    last mo: 7%     │                       │
│                    │    change: +7%     │  SCORE TREND          │
│                    │                    │  Avg: 76 (prev: 71)   │
└────────────────────┴────────────────────┴───────────────────────┘
```

---

## Components

### New Components

| Component | File | Description |
|---|---|---|
| `ProductivityLayout` | `components/productivity/ProductivityLayout.tsx` | Wrapper: sidebar + period tabs + date nav + content slot |
| `DateNavigator` | `components/productivity/DateNavigator.tsx` | `< March 5, 2026 >` arrows, period-aware label formatting |
| `WorkHoursCard` | `components/productivity/WorkHoursCard.tsx` | Big duration display + % of work day |
| `ProductivityScoreRing` | `components/productivity/ProductivityScoreRing.tsx` | SVG circular gauge, 0-100, color-coded |
| `BreakdownDonuts` | `components/productivity/BreakdownDonuts.tsx` | Focus/Meetings/Breaks percentage rings (Recharts PieChart) |
| `CategoriesList` | `components/productivity/CategoriesList.tsx` | Category breakdown with % + colored bars |
| `ActivityFeed` | `components/productivity/ActivityFeed.tsx` | Real-time app switch log, polled/event-driven |
| `GoalsProgress` | `components/productivity/GoalsProgress.tsx` | Goal cards with progress bars, MET/IN PROGRESS status |
| `AiSummaryCard` | `components/productivity/AiSummaryCard.tsx` | LLM-generated daily summary text |
| `WeeklyChart` | `components/productivity/WeeklyChart.tsx` | Recharts StackedBarChart for 7-day view |
| `MonthlyChart` | `components/productivity/MonthlyChart.tsx` | Recharts StackedBarChart for 28-31 day view |
| `WeeklyStats` | `components/productivity/WeeklyStats.tsx` | Aggregate stats for week period |
| `MonthlyStats` | `components/productivity/MonthlyStats.tsx` | Aggregate stats with month-over-month deltas |
| `DayView` | `components/productivity/DayView.tsx` | Day page layout (3-col grid, composes cards) |
| `WeekView` | `components/productivity/WeekView.tsx` | Week page layout |
| `MonthView` | `components/productivity/MonthView.tsx` | Month page layout |

### Redesigned Components

| Component | Changes |
|---|---|
| `TimelineBar` (was `Timeline`) | Full-width hourly blocks like Rize, 6am-9pm visible range |
| `PomodoroTimer` (was `FocusStatusCard`) | Add Pomodoro mode, break timer countdown, break-to-work ratio |
| `SessionsList` (was `FocusSessionsList`) | Rize-style: time + session name + duration + menu |
| `TopAppsBar` (was `TopApps`) | Horizontal bars with percentage + duration like Rize categories |

### Removed Components

| Component | Reason |
|---|---|
| `WeeklyTrend` | Replaced by `WeeklyChart` + `WeeklyStats` |
| `TodaySummary` | Split into `WorkHoursCard` + `ProductivityScoreRing` + `BreakdownDonuts` |

---

## New Tauri IPC Commands (Backend)

| Command | Params | Returns | Backend Source |
|---|---|---|---|
| `productivity_summary_range` | `start_date: String, end_date: String` | `Vec<ProductivitySummaryResponse>` | `summaries.list_range()` |
| `productivity_activity_feed` | `limit: Option<i64>` (default 50) | `Vec<ActivityFeedItem>` | `events.list_range_offset()` |
| `productivity_goals` | none | `Vec<GoalProgressResponse>` | `aggregator.check_goals()` |
| `productivity_pomodoro_start` | `work_mins: Option<i64>, break_mins: Option<i64>` | `FocusSessionResponse` | `focus_manager.start_pomodoro()` |
| `productivity_time_entries` | `date: String` | `Vec<TimeEntryResponse>` | `time_entries.list_range()` |

Also: add `productivity_score: Option<f64>` field to `ProductivitySummaryResponse` in `desktop-shared`.

### New Response Types (desktop-shared)

```rust
// Add to ProductivitySummaryResponse:
pub productivity_score: Option<f64>,

// New types:
pub struct ActivityFeedItem {
    pub app_name: String,
    pub window_title: Option<String>,
    pub category_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: Option<i64>,
}

pub struct GoalProgressResponse {
    pub id: i64,
    pub goal_type: String,       // "daily" | "weekly"
    pub metric: String,          // "productive_hours" etc.
    pub target_value: f64,
    pub current_value: f64,
    pub met: bool,
}

pub struct TimeEntryResponse {
    pub id: i64,
    pub description: String,
    pub category_id: Option<String>,
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub source: String,
}
```

---

## Frontend Types (lib/types.ts)

```typescript
// Add to ProductivitySummary:
productivityScore: number | null;

// New types:
interface ActivityFeedItem {
  appName: string;
  windowTitle: string | null;
  categoryId: string | null;
  startedAt: string;
  durationSecs: number | null;
}

interface GoalProgress {
  id: number;
  goalType: 'daily' | 'weekly';
  metric: string;
  targetValue: number;
  currentValue: number;
  met: boolean;
}

interface TimeEntry {
  id: number;
  description: string;
  categoryId: string | null;
  projectId: string | null;
  startedAt: string;
  durationSecs: number;
  source: string;
}
```

---

## Data Flow

```
URL params (date, period)
    |
    v
ProductivityLayout (extracts date/period from URL)
    |
    v
DayView / WeekView / MonthView (composes cards)
    |
    v
Card components call useQuery() with date params
    |
    v
Tauri IPC commands -> Rust backend -> SQLite
    |
    v
Recharts / SVG renders data
```

Real-time updates: existing `useEvent('entity:updated')` pattern refetches on `productivity` or `focus_session` entity changes.

---

## Charting

**Recharts** (shared with future Finance charts):
- `StackedBarChart` for Week and Month views (productive/neutral/distracting stacked)
- `PieChart` for breakdown donuts (Focus/Meetings/Breaks)

**Hand-rolled SVG**:
- Productivity score ring (circular gauge)
- Small inline progress indicators

All chart colors use CSS variable tokens from `theme.css`:
- Productive: `--success` (#22C55E)
- Neutral: `--text-muted` (#8B949E)
- Distracting: `--destructive` (#d4183d)
- Focus: `--brand` (#F97316)
- Meetings: `--purple` (#8B5CF6)
- Breaks: `--info` (#3B82F6)

---

## Styling

- Dark theme only (existing `theme.css` tokens)
- All cards use `bg-surface-base rounded-xl` pattern (existing)
- Dense grid with `gap-4`
- Text sizing: headers `text-[13px] font-medium`, values `text-[18px]-[28px]`, labels `text-[11px] font-light`
- No new CSS variables needed

---

## Dependencies

- `recharts` (add via `bun add recharts`)
- Existing: `react-router`, `lucide-react`, `@tauri-apps/api`

---

## Scope Boundaries

**In scope:**
- ProductivityLayout with period tabs and date navigation
- Day/Week/Month views with all components listed above
- New Tauri IPC commands
- Add productivity_score to response types
- Install Recharts

**Out of scope:**
- Light theme
- Export UI (backend export exists, UI deferred)
- Category management UI
- Notification preferences UI
- Time entry creation UI (log_time via chat only for now)
