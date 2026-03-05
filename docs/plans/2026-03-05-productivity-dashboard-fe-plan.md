# Productivity Dashboard Frontend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the productivity page into a Rize.io-inspired dense dashboard with Day/Week/Month views, 3-column grid, Recharts for charts, and new IPC commands for score, goals, activity feed, and pomodoro.

**Architecture:** Add `ProductivityLayout` wrapper (like `FinanceLayout`) with period tabs + date navigation. Three view components (`DayView`, `WeekView`, `MonthView`) compose card components. New Tauri IPC commands bridge the existing backend data to the frontend. Recharts handles bar/pie charts; hand-rolled SVG for the score ring.

**Tech Stack:** React, TypeScript, Tailwind v4, Recharts, react-router, Tauri IPC, Lucide icons.

**Design doc:** `docs/plans/2026-03-05-productivity-dashboard-fe-design.md`

---

## Phase Overview

| Phase | Tasks | What it delivers |
|-------|-------|------------------|
| **1: Backend IPC** | Tasks 1-4 | New Tauri commands + response types for score, goals, feed, pomodoro, range |
| **2: Foundation** | Tasks 5-8 | Install Recharts, new TS types, date helpers, ProductivityLayout + routing |
| **3: Day View Cards** | Tasks 9-18 | All Day view components (score ring, work hours, timeline, pomodoro, sessions, feed, breakdown, categories, goals, AI summary) |
| **4: Day View Assembly** | Task 19 | DayView 3-column grid composing all cards |
| **5: Week View** | Tasks 20-22 | Weekly stacked bar chart + stats + assembly |
| **6: Month View** | Tasks 23-25 | Monthly stacked bar chart + stats + assembly |
| **7: Cleanup** | Task 26 | Remove old components, verify build |

---

## Task 1: Add productivity_score to ProductivitySummaryResponse

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs:367-382`
- Modify: `crates/desktop/src/commands/productivity.rs:20-53`

**Step 1: Add field to response struct**

In `crates/desktop-shared/src/commands.rs`, add after `ai_summary` (line 382):

```rust
pub productivity_score: Option<f64>,
```

**Step 2: Update mapper function**

In `crates/desktop/src/commands/productivity.rs`, add to `summary_to_response` (after line 51):

```rust
productivity_score: s.productivity_score,
```

**Step 3: Verify it compiles**

Run: `cargo build -p desktop-shared -p desktop`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands.rs crates/desktop/src/commands/productivity.rs
git commit -m "feat(desktop): add productivity_score to ProductivitySummaryResponse"
```

---

## Task 2: Add new response types for goals, time entries, activity feed

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs`

**Step 1: Add new response structs**

Append after `ActivityCategoryResponse` (after line 437):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalProgressResponse {
    pub id: i64,
    pub goal_type: String,
    pub metric: String,
    pub target_value: f64,
    pub current_value: f64,
    pub met: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

**Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands.rs
git commit -m "feat(desktop-shared): add GoalProgressResponse and TimeEntryResponse types"
```

---

## Task 3: Add new Tauri IPC commands

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs`

**Step 1: Add productivity_summary_range command**

```rust
#[tauri::command]
pub async fn productivity_summary_range(
    state: State<'_, AppCore>,
    start_date: String,
    end_date: String,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let summaries = repos
        .summaries
        .list_range(&start_date, &end_date)
        .await
        .map_err(map_prod_err)?;
    Ok(summaries.into_iter().map(summary_to_response).collect())
}
```

**Step 2: Add productivity_activity_feed command**

```rust
#[tauri::command]
pub async fn productivity_activity_feed(
    state: State<'_, AppCore>,
    limit: Option<i64>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let now = Utc::now();
    let start = now - chrono::Duration::hours(24);
    let cap = limit.unwrap_or(50).min(200);
    let events = repos
        .events
        .list_range_offset(&start, &now, Some(cap), None)
        .await
        .map_err(map_prod_err)?;
    Ok(events
        .into_iter()
        .rev()
        .map(|e| ActivityTimelineResponse {
            app_name: e.app_name,
            window_title: e.window_title,
            category_id: e.category_id,
            started_at: e.started_at,
            duration_secs: e.duration_secs,
            is_idle: e.is_idle,
        })
        .collect())
}
```

**Step 3: Add productivity_goals command**

```rust
use desktop_shared::commands::GoalProgressResponse;

#[tauri::command]
pub async fn productivity_goals(
    state: State<'_, AppCore>,
) -> Result<Vec<GoalProgressResponse>, ApiError> {
    let aggregator = state.aggregator()?;
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let results = aggregator.check_goals(&today).await.map_err(map_prod_err)?;
    Ok(results
        .into_iter()
        .map(|(goal, current, met)| GoalProgressResponse {
            id: goal.id.unwrap_or(0),
            goal_type: goal.goal_type.to_string(),
            metric: goal.metric.to_string(),
            target_value: goal.target_value,
            current_value: current,
            met,
        })
        .collect())
}
```

**Step 4: Add productivity_pomodoro_start command**

```rust
#[tauri::command]
pub async fn productivity_pomodoro_start(
    state: State<'_, AppCore>,
    work_mins: Option<i64>,
    break_mins: Option<i64>,
) -> Result<FocusSessionResponse, ApiError> {
    let focus_mgr = state.focus_manager()?;
    let session = focus_mgr
        .start_pomodoro(None, None, work_mins, break_mins)
        .await
        .map_err(map_prod_err)?;
    Ok(session_to_response(session))
}
```

**Step 5: Add productivity_time_entries command**

```rust
use desktop_shared::commands::TimeEntryResponse;

#[tauri::command]
pub async fn productivity_time_entries(
    state: State<'_, AppCore>,
    date: String,
) -> Result<Vec<TimeEntryResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let start = super::parse_date_or_err(&date)?;
    let end = start + chrono::Duration::days(1);
    let entries = repos
        .time_entries
        .list_range(&start, &end)
        .await
        .map_err(map_prod_err)?;
    Ok(entries
        .into_iter()
        .map(|e| TimeEntryResponse {
            id: e.id.unwrap_or(0),
            description: e.description,
            category_id: e.category_id,
            project_id: e.project_id,
            started_at: e.started_at,
            duration_secs: e.duration_secs,
            source: e.source,
        })
        .collect())
}
```

**Step 6: Verify it compiles**

Run: `cargo build -p desktop`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/desktop/src/commands/productivity.rs
git commit -m "feat(desktop): add IPC commands for summary_range, activity_feed, goals, pomodoro, time_entries"
```

---

## Task 4: Register new Tauri commands

**Files:**
- Modify: `crates/desktop/src/main.rs:158-167`

**Step 1: Add new commands to the invoke_handler**

After the existing productivity commands (line 166), add:

```rust
commands::productivity::productivity_summary_range,
commands::productivity::productivity_activity_feed,
commands::productivity::productivity_goals,
commands::productivity::productivity_pomodoro_start,
commands::productivity::productivity_time_entries,
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(desktop): register new productivity Tauri commands"
```

---

## Task 5: Install Recharts

**Files:**
- Modify: `desktop-ui/package.json`

**Step 1: Install**

Run: `cd desktop-ui && bun add recharts`

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 3: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "chore(desktop-ui): add recharts dependency"
```

---

## Task 6: Add new TypeScript types

**Files:**
- Modify: `desktop-ui/src/lib/types.ts`

**Step 1: Add productivityScore to ProductivitySummary**

In the `ProductivitySummary` interface (line 418-434), add after `aiSummary`:

```typescript
productivityScore: number | null;
```

**Step 2: Add new interfaces**

After `ActivityCategory` (line 478), add:

```typescript
export interface GoalProgress {
  id: number;
  goalType: string;
  metric: string;
  targetValue: number;
  currentValue: number;
  met: boolean;
}

export interface TimeEntry {
  id: number;
  description: string;
  categoryId: string | null;
  projectId: string | null;
  startedAt: string;
  durationSecs: number;
  source: string;
}

export type ProductivityPeriod = 'day' | 'week' | 'month';
```

**Step 3: Update SidebarItem — no change needed (already has 'Productivity')**

**Step 4: Commit**

```bash
git add desktop-ui/src/lib/types.ts
git commit -m "feat(desktop-ui): add productivity TypeScript types for goals, time entries, score"
```

---

## Task 7: Add date helper functions

**Files:**
- Modify: `desktop-ui/src/lib/dates.ts`

**Step 1: Add period-aware date helpers**

Append to `dates.ts`:

```typescript
const LONG_MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];

/** Format as "Thursday, March 5, 2026" */
export function formatFullDate(iso: string): string {
  const d = new Date(iso + 'T00:00:00');
  return `${WEEKDAYS[d.getDay()]}, ${LONG_MONTHS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}

/** Format as "Mar 3 - Mar 9, 2026" */
export function formatWeekRange(weekStart: string): string {
  const start = new Date(weekStart + 'T00:00:00');
  const end = new Date(start);
  end.setDate(end.getDate() + 6);
  const sy = start.getFullYear();
  const ey = end.getFullYear();
  const s = `${SHORT_MONTHS[start.getMonth()]} ${start.getDate()}`;
  const e = `${SHORT_MONTHS[end.getMonth()]} ${end.getDate()}, ${ey}`;
  if (sy !== ey) return `${s}, ${sy} - ${e}`;
  return `${s} - ${e}`;
}

/** Format as "March 2026" */
export function formatMonthLabel(yearMonth: string): string {
  const [y, m] = yearMonth.split('-').map(Number);
  return `${LONG_MONTHS[m - 1]} ${y}`;
}

/** Get today as YYYY-MM-DD */
export function todayISO(): string {
  return new Date().toISOString().slice(0, 10);
}

/** Get the Monday of the week containing the given date */
export function weekStartISO(iso: string): string {
  const d = new Date(iso + 'T00:00:00');
  const day = d.getDay();
  const diff = d.getDate() - day + (day === 0 ? -6 : 1);
  d.setDate(diff);
  return d.toISOString().slice(0, 10);
}

/** Get YYYY-MM from a date */
export function monthISO(iso: string): string {
  return iso.slice(0, 7);
}

/** Navigate a date by offset: +1 day, -1 day, etc. */
export function shiftDate(iso: string, days: number): string {
  const d = new Date(iso + 'T00:00:00');
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

/** Navigate a month by offset: +1 month, -1 month */
export function shiftMonth(yearMonth: string, months: number): string {
  const [y, m] = yearMonth.split('-').map(Number);
  const d = new Date(y, m - 1 + months, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
}

/** Get the last day of a month as YYYY-MM-DD */
export function monthEndISO(yearMonth: string): string {
  const [y, m] = yearMonth.split('-').map(Number);
  const d = new Date(y, m, 0);
  return d.toISOString().slice(0, 10);
}

/** Format seconds as "Xh Ym" with large text style (e.g. "7 hr 33 min") */
export function formatLongDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0 && m > 0) return `${h} hr ${m} min`;
  if (h > 0) return `${h} hr`;
  return `${m} min`;
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/lib/dates.ts
git commit -m "feat(desktop-ui): add date navigation helpers for day/week/month periods"
```

---

## Task 8: Create ProductivityLayout + routing

**Files:**
- Create: `desktop-ui/src/components/productivity/ProductivityLayout.tsx`
- Create: `desktop-ui/src/components/productivity/DateNavigator.tsx`
- Modify: `desktop-ui/src/App.tsx`

**Step 1: Create DateNavigator**

```typescript
import { ChevronLeft, ChevronRight, Calendar } from 'lucide-react';

interface DateNavigatorProps {
  label: string;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
}

export function DateNavigator({ label, onPrev, onNext, onToday }: DateNavigatorProps) {
  return (
    <div className="flex items-center gap-2">
      <button
        onClick={onPrev}
        className="w-7 h-7 rounded-md bg-surface-base flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
      >
        <ChevronLeft className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <button
        onClick={onToday}
        className="w-7 h-7 rounded-md bg-surface-base flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
      >
        <Calendar className="w-3.5 h-3.5" strokeWidth={1.5} />
      </button>
      <button
        onClick={onNext}
        className="w-7 h-7 rounded-md bg-surface-base flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
      >
        <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <span className="text-[13px] font-medium text-primary ml-1">{label}</span>
    </div>
  );
}
```

**Step 2: Create ProductivityLayout**

```typescript
import { useNavigate, useLocation } from 'react-router';
import { Sidebar } from '../layout/Sidebar';
import { DateNavigator } from './DateNavigator';
import {
  todayISO, weekStartISO, monthISO,
  formatFullDate, formatWeekRange, formatMonthLabel,
  shiftDate, shiftMonth,
} from '../../lib/dates';
import type { ProductivityPeriod } from '../../lib/types';

interface ProductivityLayoutProps {
  children: React.ReactNode;
  period: ProductivityPeriod;
  dateParam: string;
}

const periods: { key: ProductivityPeriod; label: string }[] = [
  { key: 'day', label: 'Day' },
  { key: 'week', label: 'Week' },
  { key: 'month', label: 'Month' },
];

export function ProductivityLayout({ children, period, dateParam }: ProductivityLayoutProps) {
  const navigate = useNavigate();

  const handlePeriodChange = (p: ProductivityPeriod) => {
    const today = todayISO();
    if (p === 'day') navigate(`/productivity/day/${today}`);
    else if (p === 'week') navigate(`/productivity/week/${weekStartISO(today)}`);
    else navigate(`/productivity/month/${monthISO(today)}`);
  };

  const handlePrev = () => {
    if (period === 'day') navigate(`/productivity/day/${shiftDate(dateParam, -1)}`);
    else if (period === 'week') navigate(`/productivity/week/${shiftDate(dateParam, -7)}`);
    else navigate(`/productivity/month/${shiftMonth(dateParam, -1)}`);
  };

  const handleNext = () => {
    if (period === 'day') navigate(`/productivity/day/${shiftDate(dateParam, 1)}`);
    else if (period === 'week') navigate(`/productivity/week/${shiftDate(dateParam, 7)}`);
    else navigate(`/productivity/month/${shiftMonth(dateParam, 1)}`);
  };

  const handleToday = () => {
    const today = todayISO();
    if (period === 'day') navigate(`/productivity/day/${today}`);
    else if (period === 'week') navigate(`/productivity/week/${weekStartISO(today)}`);
    else navigate(`/productivity/month/${monthISO(today)}`);
  };

  const dateLabel =
    period === 'day' ? formatFullDate(dateParam) :
    period === 'week' ? formatWeekRange(dateParam) :
    formatMonthLabel(dateParam);

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar active="Productivity" />
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top bar: period tabs + date navigator */}
        <div className="h-14 bg-background flex items-center px-4 gap-4 flex-shrink-0">
          <div className="flex items-center gap-1">
            {periods.map((p) => (
              <button
                key={p.key}
                onClick={() => handlePeriodChange(p.key)}
                className={`px-3 py-1.5 rounded-md text-[13px] font-light transition-colors ${
                  period === p.key
                    ? 'bg-surface-highest text-white'
                    : 'bg-surface-low text-muted hover:bg-surface-base hover:text-secondary'
                }`}
              >
                {p.label}
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <DateNavigator
            label={dateLabel}
            onPrev={handlePrev}
            onNext={handleNext}
            onToday={handleToday}
          />
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {children}
        </div>
      </div>
    </div>
  );
}
```

**Step 3: Update App.tsx routing**

Replace the single `/productivity` route with:

```typescript
import { Navigate } from "react-router";
// ... existing imports ...

// Remove the old: { path: "/productivity", element: <Productivity /> },
// Add these:
{ path: "/productivity", element: <Navigate to={`/productivity/day/${new Date().toISOString().slice(0, 10)}`} replace /> },
{ path: "/productivity/day/:date", element: <ProductivityDayPage /> },
{ path: "/productivity/week/:weekStart", element: <ProductivityWeekPage /> },
{ path: "/productivity/month/:yearMonth", element: <ProductivityMonthPage /> },
```

For now, create placeholder page components:

Create `desktop-ui/src/components/productivity/pages/DayPage.tsx`:

```typescript
import { useParams } from 'react-router';
import { ProductivityLayout } from '../ProductivityLayout';
import { todayISO } from '../../../lib/dates';

export function ProductivityDayPage() {
  const { date } = useParams();
  const d = date ?? todayISO();

  return (
    <ProductivityLayout period="day" dateParam={d}>
      <div className="text-muted text-[13px]">Day view for {d} — components coming soon</div>
    </ProductivityLayout>
  );
}
```

Create `desktop-ui/src/components/productivity/pages/WeekPage.tsx`:

```typescript
import { useParams } from 'react-router';
import { ProductivityLayout } from '../ProductivityLayout';
import { todayISO, weekStartISO } from '../../../lib/dates';

export function ProductivityWeekPage() {
  const { weekStart } = useParams();
  const ws = weekStart ?? weekStartISO(todayISO());

  return (
    <ProductivityLayout period="week" dateParam={ws}>
      <div className="text-muted text-[13px]">Week view from {ws} — components coming soon</div>
    </ProductivityLayout>
  );
}
```

Create `desktop-ui/src/components/productivity/pages/MonthPage.tsx`:

```typescript
import { useParams } from 'react-router';
import { ProductivityLayout } from '../ProductivityLayout';
import { todayISO, monthISO } from '../../../lib/dates';

export function ProductivityMonthPage() {
  const { yearMonth } = useParams();
  const ym = yearMonth ?? monthISO(todayISO());

  return (
    <ProductivityLayout period="month" dateParam={ym}>
      <div className="text-muted text-[13px]">Month view for {ym} — components coming soon</div>
    </ProductivityLayout>
  );
}
```

Update `App.tsx` imports and routes accordingly. Remove import of old `Productivity`.

**Step 4: Verify dev server**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 5: Commit**

```bash
git add desktop-ui/src/components/productivity/ProductivityLayout.tsx \
      desktop-ui/src/components/productivity/DateNavigator.tsx \
      desktop-ui/src/components/productivity/pages/DayPage.tsx \
      desktop-ui/src/components/productivity/pages/WeekPage.tsx \
      desktop-ui/src/components/productivity/pages/MonthPage.tsx \
      desktop-ui/src/App.tsx
git commit -m "feat(desktop-ui): add ProductivityLayout, DateNavigator, period routing"
```

---

## Task 9: ProductivityScoreRing (SVG)

**Files:**
- Create: `desktop-ui/src/components/productivity/ProductivityScoreRing.tsx`

**Step 1: Create component**

```typescript
interface ProductivityScoreRingProps {
  score: number;
  size?: number;
}

function scoreColor(score: number): string {
  if (score >= 80) return 'var(--success)';
  if (score >= 60) return 'var(--brand)';
  if (score >= 40) return 'var(--text-muted)';
  return 'var(--destructive)';
}

export function ProductivityScoreRing({ score, size = 100 }: ProductivityScoreRingProps) {
  const strokeWidth = 8;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const progress = Math.min(score / 100, 1);
  const offset = circumference * (1 - progress);
  const center = size / 2;

  return (
    <div className="flex flex-col items-center gap-1">
      <svg width={size} height={size} className="-rotate-90">
        <circle
          cx={center}
          cy={center}
          r={radius}
          fill="none"
          stroke="var(--surface-raised)"
          strokeWidth={strokeWidth}
        />
        <circle
          cx={center}
          cy={center}
          r={radius}
          fill="none"
          stroke={scoreColor(score)}
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          className="transition-all duration-700"
        />
      </svg>
      <div className="absolute flex flex-col items-center justify-center" style={{ width: size, height: size }}>
        <span className="text-[24px] font-light text-primary tabular-nums">{Math.round(score)}</span>
        <span className="text-[10px] font-light text-dim">/100</span>
      </div>
    </div>
  );
}
```

Note: The parent must have `relative` positioning for the absolute center text. Wrap in a `relative` div when used.

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/ProductivityScoreRing.tsx
git commit -m "feat(desktop-ui): add ProductivityScoreRing SVG component"
```

---

## Task 10: WorkHoursCard

**Files:**
- Create: `desktop-ui/src/components/productivity/WorkHoursCard.tsx`

**Step 1: Create component**

```typescript
import { formatLongDuration } from '../../lib/dates';

interface WorkHoursCardProps {
  totalActiveSecs: number;
  workDayHours?: number;
}

export function WorkHoursCard({ totalActiveSecs, workDayHours = 8 }: WorkHoursCardProps) {
  const pct = Math.min((totalActiveSecs / (workDayHours * 3600)) * 100, 100);

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-2">
      <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
      <div className="flex items-baseline justify-between">
        <span className="text-[28px] font-light text-primary tabular-nums">
          {formatLongDuration(totalActiveSecs)}
        </span>
        <div className="flex flex-col items-end">
          <span className="text-[11px] font-light text-dim">Percent of work day</span>
          <span className="text-[18px] font-light text-primary tabular-nums">{pct.toFixed(1)}%</span>
          <span className="text-[10px] font-light text-dim">of {workDayHours} hr 0 min</span>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/WorkHoursCard.tsx
git commit -m "feat(desktop-ui): add WorkHoursCard component"
```

---

## Task 11: BreakdownDonuts (Recharts)

**Files:**
- Create: `desktop-ui/src/components/productivity/BreakdownDonuts.tsx`

**Step 1: Create component**

```typescript
import { PieChart, Pie, Cell } from 'recharts';

interface BreakdownSegment {
  name: string;
  value: number;
  color: string;
}

interface BreakdownDonutsProps {
  segments: BreakdownSegment[];
  totalSecs: number;
}

function MiniDonut({ name, value, total, color }: { name: string; value: number; total: number; color: string }) {
  const pct = total > 0 ? Math.round((value / total) * 100) : 0;
  const data = [
    { value: value },
    { value: Math.max(total - value, 0) },
  ];
  const h = Math.floor(value / 3600);
  const m = Math.floor((value % 3600) / 60);

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative">
        <PieChart width={56} height={56}>
          <Pie
            data={data}
            cx={27}
            cy={27}
            innerRadius={18}
            outerRadius={25}
            startAngle={90}
            endAngle={-270}
            dataKey="value"
            stroke="none"
          >
            <Cell fill={color} />
            <Cell fill="var(--surface-raised)" />
          </Pie>
        </PieChart>
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-[11px] font-light text-primary tabular-nums">{pct}%</span>
        </div>
      </div>
      <span className="text-[11px] font-medium text-secondary">{name}</span>
      <span className="text-[10px] font-light text-dim">
        {h > 0 ? `${h} hr ${m} min` : `${m} min`}
      </span>
    </div>
  );
}

export function BreakdownDonuts({ segments, totalSecs }: BreakdownDonutsProps) {
  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Breakdown</h2>
      <div className="flex items-start justify-around">
        {segments.map((s) => (
          <MiniDonut key={s.name} name={s.name} value={s.value} total={totalSecs} color={s.color} />
        ))}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/BreakdownDonuts.tsx
git commit -m "feat(desktop-ui): add BreakdownDonuts component with Recharts"
```

---

## Task 12: CategoriesList

**Files:**
- Create: `desktop-ui/src/components/productivity/CategoriesList.tsx`

**Step 1: Create component**

```typescript
import { formatHumanDuration } from '../../lib/dates';
import type { CategoryUsage } from '../../lib/types';

interface CategoriesListProps {
  categories: CategoryUsage[];
  totalSecs: number;
}

const CATEGORY_COLORS = [
  'var(--brand)', 'var(--purple)', 'var(--info)', 'var(--success)',
  'var(--text-muted)', 'var(--destructive)', 'var(--dim)',
];

export function CategoriesList({ categories, totalSecs }: CategoriesListProps) {
  if (categories.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Categories</h2>
        <p className="text-[12px] font-light text-dim">No category data</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Categories</h2>
        <span className="text-[10px] font-light text-dim">Total tracked time: {formatHumanDuration(totalSecs)}</span>
      </div>
      <div className="flex flex-col gap-2">
        {categories.map((cat, i) => {
          const pct = totalSecs > 0 ? Math.round((cat.durationSecs / totalSecs) * 100) : 0;
          const color = CATEGORY_COLORS[i % CATEGORY_COLORS.length];
          return (
            <div key={cat.category} className="flex items-center gap-3">
              <span className="text-[11px] font-light text-muted w-8 text-right tabular-nums">{pct}%</span>
              <span className="text-[11px] font-light text-primary flex-1 truncate">{cat.category}</span>
              <div className="w-20 h-1.5 rounded-full bg-surface-raised overflow-hidden flex-shrink-0">
                <div className="h-full rounded-full" style={{ width: `${pct}%`, backgroundColor: color }} />
              </div>
              <span className="text-[11px] font-light text-muted tabular-nums w-16 text-right">
                {formatHumanDuration(cat.durationSecs)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/CategoriesList.tsx
git commit -m "feat(desktop-ui): add CategoriesList component"
```

---

## Task 13: Redesign TimelineBar

**Files:**
- Modify: `desktop-ui/src/components/productivity/Timeline.tsx`

**Step 1: Rewrite Timeline as full-width Rize-style bar**

Completely rewrite `Timeline.tsx`. The new version shows hour blocks from 6am to 9pm (configurable), with colored blocks for activity and grey for idle. Taller than the current version. Shows hour tick marks below.

```typescript
import { useMemo } from 'react';
import { useQuery } from '../../hooks/useQuery';
import type { ActivityTimeline, ActivityCategory } from '../../lib/types';

interface TimelineBarProps {
  date: string;
}

interface Block {
  leftPct: number;
  widthPct: number;
  color: string;
  label: string;
}

const START_HOUR = 6;
const END_HOUR = 22;
const SPAN_HOURS = END_HOUR - START_HOUR;

function categoryColor(categoryType: string | undefined, isIdle: boolean): string {
  if (isIdle) return 'var(--surface-lowest)';
  switch (categoryType) {
    case 'productive': return 'var(--success)';
    case 'distracting': return 'var(--destructive)';
    default: return 'var(--text-muted)';
  }
}

const TICK_LABELS: { hour: number; label: string }[] = [];
for (let h = START_HOUR; h <= END_HOUR; h += 2) {
  TICK_LABELS.push({
    hour: h,
    label: h === 0 ? '12a' : h < 12 ? `${h}a` : h === 12 ? '12p' : `${h - 12}p`,
  });
}

export function TimelineBar({ date }: TimelineBarProps) {
  const { data: events } = useQuery<ActivityTimeline[]>('productivity_timeline', { date }, []);
  const { data: categories } = useQuery<ActivityCategory[]>('productivity_categories', undefined, []);

  const categoryMap = useMemo(
    () => new Map(categories.map((c) => [c.id, c])),
    [categories],
  );

  const blocks: Block[] = useMemo(() => {
    if (events.length === 0) return [];
    const spanSecs = SPAN_HOURS * 3600;
    const startSecs = START_HOUR * 3600;

    return events
      .map((e) => {
        const start = new Date(e.startedAt);
        const eSecs = start.getHours() * 3600 + start.getMinutes() * 60 + start.getSeconds();
        const dur = e.durationSecs ?? 0;
        if (eSecs + dur < startSecs || eSecs > END_HOUR * 3600) return null;

        const clampedStart = Math.max(eSecs - startSecs, 0);
        const clampedEnd = Math.min(eSecs + dur - startSecs, spanSecs);
        const cat = e.categoryId ? categoryMap.get(e.categoryId) : undefined;

        return {
          leftPct: (clampedStart / spanSecs) * 100,
          widthPct: Math.max(((clampedEnd - clampedStart) / spanSecs) * 100, 0.3),
          color: categoryColor(cat?.categoryType, e.isIdle),
          label: e.appName,
        };
      })
      .filter(Boolean) as Block[];
  }, [events, categoryMap]);

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-2 col-span-3">
      <h2 className="text-[13px] font-medium text-secondary">Timeline</h2>

      <div className="relative h-10 rounded-lg bg-surface-raised overflow-hidden">
        {blocks.map((b, i) => (
          <div
            key={i}
            className="absolute top-0 h-full rounded-sm"
            style={{
              left: `${b.leftPct}%`,
              width: `${b.widthPct}%`,
              backgroundColor: b.color,
            }}
            title={b.label}
          />
        ))}
      </div>

      <div className="flex justify-between text-[9px] font-light text-dim px-0.5">
        {TICK_LABELS.map(({ hour, label }) => (
          <span key={hour} style={{ width: `${100 / TICK_LABELS.length}%` }}>{label}</span>
        ))}
      </div>

      <div className="flex items-center gap-4 text-[10px] font-light text-muted">
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-success" />Productive</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-text-muted" />Neutral</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-destructive" />Distracting</span>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/Timeline.tsx
git commit -m "refactor(desktop-ui): redesign Timeline as Rize-style full-width bar"
```

---

## Task 14: Redesign PomodoroTimer (was FocusStatusCard)

**Files:**
- Modify: `desktop-ui/src/components/productivity/FocusStatusCard.tsx`

**Step 1: Rewrite as PomodoroTimer**

Rename the file to `PomodoroTimer.tsx`. Add support for Pomodoro-specific features: break timer countdown, break-to-work ratio display, and a "Start Break" button variant. Uses the new `productivity_pomodoro_start` command.

The component should:
- Show elapsed time in large text (like Rize's `3:01:54`)
- Show "Time since last break" and "Break to work ratio"
- Have "Start Focus (25m)" and "Start Break (5m)" dropdown
- Continue using `useEvent` for live updates

Keep the full code in the plan but note: this is the most complex component. Follow the existing `FocusStatusCard` patterns for `useQuery`, `useMutation`, `useEvent`, `useEffect` timer.

```typescript
import { useState, useEffect, useCallback } from 'react';
import { Play, Square, Coffee, Timer } from 'lucide-react';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import { formatElapsed } from '../../lib/dates';
import type { FocusSession } from '../../lib/types';

export function PomodoroTimer() {
  const { data: session, refetch } = useQuery<FocusSession | null>('productivity_focus_status', undefined, null);
  const startFocus = useMutation<FocusSession, { target_mins?: number }>('productivity_focus_start');
  const startPomodoro = useMutation<FocusSession, { work_mins?: number; break_mins?: number }>('productivity_pomodoro_start');
  const endFocus = useMutation<FocusSession | null, { notes?: string }>('productivity_focus_end');

  const [elapsed, setElapsed] = useState(0);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    if (payload?.entityKind === 'focus_session') refetch();
  });

  useEffect(() => {
    if (!session) { setElapsed(0); return; }
    const startTime = new Date(session.startedAt).getTime();
    const tick = () => setElapsed(Math.floor((Date.now() - startTime) / 1000));
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, [session]);

  const handleStartFocus = useCallback(async () => {
    await startFocus.mutate({ target_mins: 25 });
    refetch();
  }, [startFocus, refetch]);

  const handleStartPomodoro = useCallback(async () => {
    await startPomodoro.mutate({ work_mins: 25, break_mins: 5 });
    refetch();
  }, [startPomodoro, refetch]);

  const handleEnd = useCallback(async () => {
    await endFocus.mutate({});
    refetch();
  }, [endFocus, refetch]);

  const targetSecs = session?.targetMins ? session.targetMins * 60 : 0;
  const isPomodoro = session?.sessionType === 'pomodoro';

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">
          {isPomodoro ? 'Pomodoro Timer' : 'Focus Session'}
        </h2>
        {session && (
          <span className="text-[10px] font-light text-dim">
            {session.interruptions} interruptions
          </span>
        )}
      </div>

      {session ? (
        <>
          <div className="flex items-center justify-between">
            <span className="text-[32px] font-light text-brand tabular-nums">
              {formatElapsed(elapsed)}
            </span>
            {targetSecs > 0 && (
              <div className="flex flex-col items-end gap-0.5">
                <span className="text-[10px] font-light text-dim">
                  Target: {session.targetMins}m
                </span>
                {session.qualityScore != null && (
                  <span className="text-[10px] font-light text-dim">
                    Quality: {Math.round(session.qualityScore * 100)}%
                  </span>
                )}
              </div>
            )}
          </div>

          {targetSecs > 0 && (
            <div className="h-1.5 rounded-full bg-surface-raised overflow-hidden">
              <div
                className="h-full rounded-full bg-brand transition-all"
                style={{ width: `${Math.min((elapsed / targetSecs) * 100, 100)}%` }}
              />
            </div>
          )}

          <button
            onClick={handleEnd}
            disabled={endFocus.loading}
            className="flex items-center justify-center gap-2 py-2 rounded-lg bg-surface-raised text-destructive text-[12px] font-light hover:bg-surface-highest transition-colors"
          >
            <Square className="w-3.5 h-3.5" strokeWidth={1.5} />
            End Session
          </button>
        </>
      ) : (
        <div className="flex gap-2">
          <button
            onClick={handleStartFocus}
            disabled={startFocus.loading}
            className="flex-1 flex items-center justify-center gap-2 py-3 rounded-lg bg-brand text-white text-[13px] font-medium hover:bg-brand-hover transition-colors"
          >
            <Play className="w-4 h-4" strokeWidth={1.5} />
            Focus (25m)
          </button>
          <button
            onClick={handleStartPomodoro}
            disabled={startPomodoro.loading}
            className="flex items-center justify-center gap-2 px-4 py-3 rounded-lg bg-surface-raised text-secondary text-[13px] font-light hover:bg-surface-highest transition-colors"
          >
            <Timer className="w-4 h-4" strokeWidth={1.5} />
            Pomodoro
          </button>
        </div>
      )}
    </div>
  );
}
```

**Step 2: Delete old `FocusStatusCard.tsx` if desired or keep both during transition**

**Step 3: Commit**

```bash
git add desktop-ui/src/components/productivity/PomodoroTimer.tsx
git commit -m "feat(desktop-ui): add PomodoroTimer component (replaces FocusStatusCard)"
```

---

## Task 15: ActivityFeed

**Files:**
- Create: `desktop-ui/src/components/productivity/ActivityFeed.tsx`

**Step 1: Create component**

```typescript
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { formatTime } from '../../lib/dates';
import type { ActivityTimeline } from '../../lib/types';

export function ActivityFeed() {
  const { data: events, refetch } = useQuery<ActivityTimeline[]>('productivity_activity_feed', { limit: 30 }, []);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    if (payload?.entityKind === 'productivity') refetch();
  });

  if (events.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Activity</h2>
        <p className="text-[12px] font-light text-dim">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Activity</h2>
        <span className="text-[10px] font-light text-dim">Tracking: Ok</span>
      </div>
      <div className="flex flex-col gap-0.5 max-h-64 overflow-y-auto">
        {events.map((e, i) => (
          <div key={i} className="flex items-center gap-2 py-1 text-[11px] font-light">
            <span className="text-dim tabular-nums w-14 flex-shrink-0">{formatTime(e.startedAt)}</span>
            <span className="text-primary truncate">{e.appName}</span>
            {e.windowTitle && (
              <span className="text-dim truncate flex-1">{e.windowTitle}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/ActivityFeed.tsx
git commit -m "feat(desktop-ui): add ActivityFeed component"
```

---

## Task 16: GoalsProgress

**Files:**
- Create: `desktop-ui/src/components/productivity/GoalsProgress.tsx`

**Step 1: Create component**

```typescript
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import type { GoalProgress } from '../../lib/types';

function metricLabel(metric: string): string {
  switch (metric) {
    case 'productive_hours': return 'productive hours';
    case 'focus_sessions': return 'focus sessions';
    case 'productivity_score': return 'score';
    case 'max_distracting_mins': return 'distracting mins';
    default: return metric;
  }
}

function formatValue(metric: string, value: number): string {
  if (metric === 'productive_hours') return `${value.toFixed(1)}h`;
  if (metric === 'max_distracting_mins') return `${Math.round(value)}m`;
  return `${Math.round(value)}`;
}

export function GoalsProgress() {
  const { data: goals, refetch } = useQuery<GoalProgress[]>('productivity_goals', undefined, []);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    if (payload?.entityKind === 'productivity') refetch();
  });

  if (goals.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Goals</h2>
        <p className="text-[12px] font-light text-dim">No goals set</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Goals</h2>
      <div className="flex flex-col gap-2">
        {goals.map((g) => {
          const pct = g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
          return (
            <div key={g.id} className="flex flex-col gap-1">
              <div className="flex items-center justify-between text-[11px] font-light">
                <div className="flex items-center gap-2">
                  <span className={g.met ? 'text-success' : 'text-brand'}>
                    {g.met ? 'MET' : 'IN PROGRESS'}
                  </span>
                  <span className="text-primary">
                    {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                  </span>
                  <span className="text-dim">({g.goalType})</span>
                </div>
                <span className="text-muted tabular-nums">
                  {formatValue(g.metric, g.currentValue)} / {formatValue(g.metric, g.targetValue)}
                </span>
              </div>
              <div className="h-1.5 rounded-full bg-surface-raised overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all ${g.met ? 'bg-success' : 'bg-brand'}`}
                  style={{ width: `${pct}%` }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/GoalsProgress.tsx
git commit -m "feat(desktop-ui): add GoalsProgress component"
```

---

## Task 17: AiSummaryCard

**Files:**
- Create: `desktop-ui/src/components/productivity/AiSummaryCard.tsx`

**Step 1: Create component**

```typescript
interface AiSummaryCardProps {
  summary: string | null;
}

export function AiSummaryCard({ summary }: AiSummaryCardProps) {
  if (!summary) return null;

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-2">
      <h2 className="text-[13px] font-medium text-secondary">AI Summary</h2>
      <p className="text-[12px] font-light text-muted leading-relaxed">{summary}</p>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/AiSummaryCard.tsx
git commit -m "feat(desktop-ui): add AiSummaryCard component"
```

---

## Task 18: Redesign SessionsList and TopAppsBar

**Files:**
- Modify: `desktop-ui/src/components/productivity/FocusSessionsList.tsx`
- Modify: `desktop-ui/src/components/productivity/TopApps.tsx`

**Step 1: Update FocusSessionsList to Rize-style layout**

In the existing `FocusSessionsList.tsx`, update the session row to show `time  name/action  duration  ...menu`. Add a "+" button in the header. Keep the `qualityBadge` logic. The key change is making rows more Rize-like:

```typescript
// Update the row rendering inside the map:
<div
  key={s.id}
  className="flex items-center gap-3 py-2 border-b border-border-subtle last:border-b-0"
>
  <span className="text-[11px] font-light text-muted tabular-nums w-14">{formatTime(s.startedAt)}</span>
  <span className="text-[11px] font-light text-primary flex-1 truncate">
    {s.sessionType === 'pomodoro' ? 'Pomodoro' : 'Focus'}
    {s.notes ? ` — ${s.notes}` : ''}
  </span>
  <span className="text-[11px] font-light text-muted tabular-nums">
    {s.actualMins != null ? `${s.actualMins} min` : 'In progress'}
  </span>
  <span className={`text-[11px] font-light tabular-nums ${quality.color}`}>
    {quality.text}
  </span>
</div>
```

**Step 2: Update TopApps to show percentage + horizontal bars like Rize categories**

In the existing `TopApps.tsx`, update to show `pct%  appName  bar  duration`:

```typescript
// Update the app row rendering:
{apps.slice(0, 10).map((app, i) => {
  const pct = totalDuration > 0 ? Math.round((app.durationSecs / totalDuration) * 100) : 0;
  return (
    <div key={app.appName} className="flex items-center gap-3">
      <span className="text-[11px] font-light text-muted w-8 text-right tabular-nums">{pct}%</span>
      <span className="text-[11px] font-light text-primary flex-1 truncate">{app.appName}</span>
      <div className="w-20 h-1.5 rounded-full bg-surface-raised overflow-hidden flex-shrink-0">
        <div
          className="h-full rounded-full bg-brand"
          style={{ width: `${(app.durationSecs / maxDuration) * 100}%` }}
        />
      </div>
      <span className="text-[11px] font-light text-muted tabular-nums w-16 text-right">
        {formatHumanDuration(app.durationSecs)}
      </span>
    </div>
  );
})}
```

Add `totalDuration` computed from `apps.reduce(...)`.

**Step 3: Commit**

```bash
git add desktop-ui/src/components/productivity/FocusSessionsList.tsx \
      desktop-ui/src/components/productivity/TopApps.tsx
git commit -m "refactor(desktop-ui): redesign SessionsList and TopApps to Rize style"
```

---

## Task 19: Assemble DayView

**Files:**
- Create: `desktop-ui/src/components/productivity/DayView.tsx`
- Modify: `desktop-ui/src/components/productivity/pages/DayPage.tsx`

**Step 1: Create DayView**

Composes all day cards in a 3-column grid:

```typescript
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { TimelineBar } from './Timeline';
import { PomodoroTimer } from './PomodoroTimer';
import { FocusSessionsList } from './FocusSessionsList';
import { ActivityFeed } from './ActivityFeed';
import { WorkHoursCard } from './WorkHoursCard';
import { ProductivityScoreRing } from './ProductivityScoreRing';
import { BreakdownDonuts } from './BreakdownDonuts';
import { CategoriesList } from './CategoriesList';
import { GoalsProgress } from './GoalsProgress';
import { AiSummaryCard } from './AiSummaryCard';
import { TopApps } from './TopApps';
import type { ProductivitySummary } from '../../lib/types';

interface DayViewProps {
  date: string;
}

export function DayView({ date }: DayViewProps) {
  const { data: summary, refetch } = useQuery<ProductivitySummary | null>('productivity_today', undefined, null);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    const k = payload?.entityKind;
    if (k === 'productivity' || k === 'focus_session') refetch();
  });

  const breakdownSegments = summary ? [
    { name: 'Focus', value: summary.totalFocusSecs, color: 'var(--brand)' },
    { name: 'Active', value: summary.totalActiveSecs - summary.totalFocusSecs - summary.totalBreakSecs, color: 'var(--purple)' },
    { name: 'Breaks', value: summary.totalBreakSecs, color: 'var(--info)' },
  ] : [];

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      {/* Row 1: Timeline (full width) */}
      <TimelineBar date={date} />

      {/* Row 2-3: Left column */}
      <div className="flex flex-col gap-4">
        <PomodoroTimer />
        <ActivityFeed />
      </div>

      {/* Row 2-3: Center column */}
      <div className="flex flex-col gap-4">
        <FocusSessionsList date={date} />
        <TopApps apps={summary?.topApps ?? []} />
      </div>

      {/* Row 2-3: Right column */}
      <div className="flex flex-col gap-4">
        <WorkHoursCard totalActiveSecs={summary?.totalActiveSecs ?? 0} />
        <div className="bg-surface-base rounded-xl p-4 flex items-center justify-center relative">
          <ProductivityScoreRing score={summary?.productivityScore ?? 0} />
        </div>
        <BreakdownDonuts segments={breakdownSegments} totalSecs={summary?.totalActiveSecs ?? 0} />
        <CategoriesList categories={summary?.topCategories ?? []} totalSecs={summary?.totalActiveSecs ?? 0} />
        <AiSummaryCard summary={summary?.aiSummary ?? null} />
      </div>

      {/* Row 4: Goals (spans left+center) */}
      <div className="col-span-2">
        <GoalsProgress />
      </div>
    </div>
  );
}
```

**Step 2: Update DayPage to use DayView**

In `pages/DayPage.tsx`, replace placeholder with:

```typescript
import { DayView } from '../DayView';

// In the return:
<ProductivityLayout period="day" dateParam={d}>
  <DayView date={d} />
</ProductivityLayout>
```

**Step 3: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 4: Commit**

```bash
git add desktop-ui/src/components/productivity/DayView.tsx \
      desktop-ui/src/components/productivity/pages/DayPage.tsx
git commit -m "feat(desktop-ui): assemble DayView with 3-column grid layout"
```

---

## Task 20: WeeklyChart (Recharts StackedBarChart)

**Files:**
- Create: `desktop-ui/src/components/productivity/WeeklyChart.tsx`

**Step 1: Create component**

```typescript
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { useQuery } from '../../hooks/useQuery';
import { formatDayLabel } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

interface WeeklyChartProps {
  weekStart: string;
}

export function WeeklyChart({ weekStart }: WeeklyChartProps) {
  const weekEnd = (() => {
    const d = new Date(weekStart + 'T00:00:00');
    d.setDate(d.getDate() + 6);
    return d.toISOString().slice(0, 10);
  })();

  const { data: summaries } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: weekStart, end_date: weekEnd },
    [],
  );

  const chartData = summaries.map((s) => ({
    day: formatDayLabel(s.date),
    productive: +(s.productiveSecs / 3600).toFixed(1),
    neutral: +(s.neutralSecs / 3600).toFixed(1),
    distracting: +(s.distractingSecs / 3600).toFixed(1),
  }));

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3 col-span-3">
      <h2 className="text-[13px] font-medium text-secondary">Weekly Overview</h2>
      <div className="h-48">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} barCategoryGap="20%">
            <XAxis
              dataKey="day"
              tick={{ fill: 'var(--text-dim)', fontSize: 11, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
            />
            <YAxis
              tick={{ fill: 'var(--text-dim)', fontSize: 10, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
              width={30}
              label={{ value: 'Hours', angle: -90, position: 'insideLeft', style: { fill: 'var(--text-dim)', fontSize: 10, fontWeight: 300 } }}
            />
            <Tooltip
              contentStyle={{
                background: 'var(--surface-floating)',
                border: '1px solid var(--border)',
                borderRadius: 'var(--radius)',
                fontSize: 11,
                fontWeight: 300,
              }}
              labelStyle={{ color: 'var(--text-primary)' }}
            />
            <Bar dataKey="productive" stackId="a" fill="var(--success)" radius={[0, 0, 0, 0]} />
            <Bar dataKey="neutral" stackId="a" fill="var(--text-muted)" />
            <Bar dataKey="distracting" stackId="a" fill="var(--destructive)" radius={[2, 2, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/WeeklyChart.tsx
git commit -m "feat(desktop-ui): add WeeklyChart with Recharts stacked bar"
```

---

## Task 21: WeeklyStats

**Files:**
- Create: `desktop-ui/src/components/productivity/WeeklyStats.tsx`

**Step 1: Create component**

Aggregate stats from weekly summaries: avg score, total active, avg daily, total focus sessions, avg quality.

```typescript
import { formatHumanDuration } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

interface WeeklyStatsProps {
  summaries: ProductivitySummary[];
}

export function WeeklyStats({ summaries }: WeeklyStatsProps) {
  const days = summaries.length || 1;
  const totalActive = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
  const totalProductive = summaries.reduce((s, d) => s + d.productiveSecs, 0);
  const totalFocusSessions = summaries.reduce((s, d) => s + d.focusSessionsCount, 0);
  const scores = summaries.map((s) => s.productivityScore).filter((s): s is number => s != null);
  const avgScore = scores.length > 0 ? Math.round(scores.reduce((a, b) => a + b, 0) / scores.length) : 0;
  const qualities = summaries.map((s) => s.avgSessionQuality).filter((q): q is number => q != null);
  const avgQuality = qualities.length > 0 ? Math.round(qualities.reduce((a, b) => a + b, 0) / qualities.length * 100) : 0;

  const stats = [
    { label: 'Avg Score', value: `${avgScore}/100` },
    { label: 'Total Active', value: formatHumanDuration(totalActive) },
    { label: 'Avg Daily', value: formatHumanDuration(Math.round(totalActive / days)) },
    { label: 'Productive', value: formatHumanDuration(totalProductive) },
    { label: 'Focus Sessions', value: `${totalFocusSessions}` },
    { label: 'Avg Quality', value: `${avgQuality}%` },
  ];

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Weekly Stats</h2>
      <div className="grid grid-cols-2 gap-3">
        {stats.map((s) => (
          <div key={s.label} className="flex flex-col gap-0.5">
            <span className="text-[10px] font-light text-dim">{s.label}</span>
            <span className="text-[16px] font-light text-primary tabular-nums">{s.value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/WeeklyStats.tsx
git commit -m "feat(desktop-ui): add WeeklyStats component"
```

---

## Task 22: Assemble WeekView

**Files:**
- Create: `desktop-ui/src/components/productivity/WeekView.tsx`
- Modify: `desktop-ui/src/components/productivity/pages/WeekPage.tsx`

**Step 1: Create WeekView**

```typescript
import { useQuery } from '../../hooks/useQuery';
import { WeeklyChart } from './WeeklyChart';
import { WeeklyStats } from './WeeklyStats';
import { BreakdownDonuts } from './BreakdownDonuts';
import { CategoriesList } from './CategoriesList';
import { TopApps } from './TopApps';
import { GoalsProgress } from './GoalsProgress';
import type { ProductivitySummary } from '../../lib/types';

interface WeekViewProps {
  weekStart: string;
}

export function WeekView({ weekStart }: WeekViewProps) {
  const weekEnd = (() => {
    const d = new Date(weekStart + 'T00:00:00');
    d.setDate(d.getDate() + 6);
    return d.toISOString().slice(0, 10);
  })();

  const { data: summaries } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: weekStart, end_date: weekEnd },
    [],
  );

  const totalActive = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
  const totalFocus = summaries.reduce((s, d) => s + d.totalFocusSecs, 0);
  const totalBreak = summaries.reduce((s, d) => s + d.totalBreakSecs, 0);

  const allApps = new Map<string, number>();
  summaries.forEach((s) => s.topApps.forEach((a) => allApps.set(a.appName, (allApps.get(a.appName) ?? 0) + a.durationSecs)));
  const topApps = Array.from(allApps.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 10)
    .map(([appName, durationSecs]) => ({ appName, durationSecs, category: null }));

  const allCats = new Map<string, number>();
  summaries.forEach((s) => s.topCategories.forEach((c) => allCats.set(c.category, (allCats.get(c.category) ?? 0) + c.durationSecs)));
  const topCats = Array.from(allCats.entries())
    .sort((a, b) => b[1] - a[1])
    .map(([category, durationSecs]) => ({ category, durationSecs }));

  const breakdownSegments = [
    { name: 'Focus', value: totalFocus, color: 'var(--brand)' },
    { name: 'Active', value: totalActive - totalFocus - totalBreak, color: 'var(--purple)' },
    { name: 'Breaks', value: totalBreak, color: 'var(--info)' },
  ];

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      <WeeklyChart weekStart={weekStart} />
      <WeeklyStats summaries={summaries} />
      <BreakdownDonuts segments={breakdownSegments} totalSecs={totalActive} />
      <CategoriesList categories={topCats} totalSecs={totalActive} />
      <TopApps apps={topApps} />
      <div className="col-span-2">
        <GoalsProgress />
      </div>
    </div>
  );
}
```

**Step 2: Update WeekPage**

Replace placeholder with `<WeekView weekStart={ws} />`.

**Step 3: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 4: Commit**

```bash
git add desktop-ui/src/components/productivity/WeekView.tsx \
      desktop-ui/src/components/productivity/pages/WeekPage.tsx
git commit -m "feat(desktop-ui): assemble WeekView with chart, stats, and breakdown"
```

---

## Task 23: MonthlyChart

**Files:**
- Create: `desktop-ui/src/components/productivity/MonthlyChart.tsx`

**Step 1: Create component**

Same pattern as `WeeklyChart` but for a full month. Uses `productivity_summary_range` with the month's date range.

```typescript
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { useQuery } from '../../hooks/useQuery';
import { monthEndISO } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

interface MonthlyChartProps {
  yearMonth: string;
}

export function MonthlyChart({ yearMonth }: MonthlyChartProps) {
  const startDate = `${yearMonth}-01`;
  const endDate = monthEndISO(yearMonth);

  const { data: summaries } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: startDate, end_date: endDate },
    [],
  );

  const chartData = summaries.map((s) => ({
    day: parseInt(s.date.slice(8), 10),
    productive: +(s.productiveSecs / 3600).toFixed(1),
    neutral: +(s.neutralSecs / 3600).toFixed(1),
    distracting: +(s.distractingSecs / 3600).toFixed(1),
  }));

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3 col-span-3">
      <h2 className="text-[13px] font-medium text-secondary">Monthly Overview</h2>
      <div className="h-48">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} barCategoryGap="10%">
            <XAxis
              dataKey="day"
              tick={{ fill: 'var(--text-dim)', fontSize: 9, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
              interval={1}
            />
            <YAxis
              tick={{ fill: 'var(--text-dim)', fontSize: 10, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
              width={30}
              label={{ value: 'Hours', angle: -90, position: 'insideLeft', style: { fill: 'var(--text-dim)', fontSize: 10, fontWeight: 300 } }}
            />
            <Tooltip
              contentStyle={{
                background: 'var(--surface-floating)',
                border: '1px solid var(--border)',
                borderRadius: 'var(--radius)',
                fontSize: 11,
                fontWeight: 300,
              }}
              labelStyle={{ color: 'var(--text-primary)' }}
              labelFormatter={(day) => `Day ${day}`}
            />
            <Bar dataKey="productive" stackId="a" fill="var(--success)" />
            <Bar dataKey="neutral" stackId="a" fill="var(--text-muted)" />
            <Bar dataKey="distracting" stackId="a" fill="var(--destructive)" radius={[2, 2, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/MonthlyChart.tsx
git commit -m "feat(desktop-ui): add MonthlyChart with Recharts stacked bar"
```

---

## Task 24: MonthlyStats (with month-over-month deltas)

**Files:**
- Create: `desktop-ui/src/components/productivity/MonthlyStats.tsx`

**Step 1: Create component**

Fetches current month + previous month summaries, computes deltas:

```typescript
import { useQuery } from '../../hooks/useQuery';
import { formatHumanDuration, shiftMonth, monthEndISO } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

interface MonthlyStatsProps {
  yearMonth: string;
}

function delta(current: number, previous: number): string {
  const diff = current - previous;
  if (diff === 0) return '—';
  const sign = diff > 0 ? '+' : '';
  return `${sign}${formatHumanDuration(Math.abs(diff))}`;
}

function scoreDelta(current: number, previous: number): string {
  const diff = Math.round(current - previous);
  if (diff === 0) return '—';
  return diff > 0 ? `+${diff}` : `${diff}`;
}

export function MonthlyStats({ yearMonth }: MonthlyStatsProps) {
  const startDate = `${yearMonth}-01`;
  const endDate = monthEndISO(yearMonth);
  const prevMonth = shiftMonth(yearMonth, -1);
  const prevStart = `${prevMonth}-01`;
  const prevEnd = monthEndISO(prevMonth);

  const { data: current } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: startDate, end_date: endDate },
    [],
  );
  const { data: previous } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: prevStart, end_date: prevEnd },
    [],
  );

  const curDays = current.length || 1;
  const prevDays = previous.length || 1;

  const curActive = current.reduce((s, d) => s + d.totalActiveSecs, 0);
  const prevActive = previous.reduce((s, d) => s + d.totalActiveSecs, 0);
  const curAvgDaily = Math.round(curActive / curDays);
  const prevAvgDaily = Math.round(prevActive / prevDays);
  const curAvgWeekly = Math.round((curActive / curDays) * 7);
  const prevAvgWeekly = Math.round((prevActive / prevDays) * 7);

  const curScores = current.map((s) => s.productivityScore).filter((s): s is number => s != null);
  const prevScores = previous.map((s) => s.productivityScore).filter((s): s is number => s != null);
  const curAvgScore = curScores.length > 0 ? curScores.reduce((a, b) => a + b, 0) / curScores.length : 0;
  const prevAvgScore = prevScores.length > 0 ? prevScores.reduce((a, b) => a + b, 0) / prevScores.length : 0;

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
      <div className="flex flex-col gap-3">
        <div>
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-light text-dim">Avg. Work Hours per week</span>
          </div>
          <span className="text-[22px] font-light text-primary tabular-nums">{formatHumanDuration(curAvgWeekly)}</span>
          <div className="flex gap-2 text-[10px] font-light text-dim">
            <span>Last month: {formatHumanDuration(prevAvgWeekly)}</span>
            <span>Change: {delta(curAvgWeekly, prevAvgWeekly)}</span>
          </div>
        </div>
        <div className="border-t border-border-subtle pt-3">
          <span className="text-[10px] font-light text-dim">Avg. time worked per day</span>
          <div className="text-[18px] font-light text-primary tabular-nums">{formatHumanDuration(curAvgDaily)}</div>
          <div className="flex gap-2 text-[10px] font-light text-dim">
            <span>Last month: {formatHumanDuration(prevAvgDaily)}</span>
            <span>Change: {delta(curAvgDaily, prevAvgDaily)}</span>
          </div>
        </div>
        <div className="border-t border-border-subtle pt-3">
          <span className="text-[10px] font-light text-dim">Avg. Score</span>
          <div className="text-[18px] font-light text-primary tabular-nums">{Math.round(curAvgScore)}/100</div>
          <div className="flex gap-2 text-[10px] font-light text-dim">
            <span>Last month: {Math.round(prevAvgScore)}</span>
            <span>Change: {scoreDelta(curAvgScore, prevAvgScore)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/productivity/MonthlyStats.tsx
git commit -m "feat(desktop-ui): add MonthlyStats with month-over-month deltas"
```

---

## Task 25: Assemble MonthView

**Files:**
- Create: `desktop-ui/src/components/productivity/MonthView.tsx`
- Modify: `desktop-ui/src/components/productivity/pages/MonthPage.tsx`

**Step 1: Create MonthView**

Same pattern as WeekView but uses `MonthlyChart`, `MonthlyStats`, aggregates categories from the month.

```typescript
import { useQuery } from '../../hooks/useQuery';
import { monthEndISO } from '../../lib/dates';
import { MonthlyChart } from './MonthlyChart';
import { MonthlyStats } from './MonthlyStats';
import { BreakdownDonuts } from './BreakdownDonuts';
import { CategoriesList } from './CategoriesList';
import type { ProductivitySummary } from '../../lib/types';

interface MonthViewProps {
  yearMonth: string;
}

export function MonthView({ yearMonth }: MonthViewProps) {
  const startDate = `${yearMonth}-01`;
  const endDate = monthEndISO(yearMonth);

  const { data: summaries } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: startDate, end_date: endDate },
    [],
  );

  const totalActive = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
  const totalFocus = summaries.reduce((s, d) => s + d.totalFocusSecs, 0);
  const totalBreak = summaries.reduce((s, d) => s + d.totalBreakSecs, 0);

  const allCats = new Map<string, number>();
  summaries.forEach((s) => s.topCategories.forEach((c) => allCats.set(c.category, (allCats.get(c.category) ?? 0) + c.durationSecs)));
  const topCats = Array.from(allCats.entries())
    .sort((a, b) => b[1] - a[1])
    .map(([category, durationSecs]) => ({ category, durationSecs }));

  const breakdownSegments = [
    { name: 'Focus', value: totalFocus, color: 'var(--brand)' },
    { name: 'Active', value: totalActive - totalFocus - totalBreak, color: 'var(--purple)' },
    { name: 'Breaks', value: totalBreak, color: 'var(--info)' },
  ];

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      <MonthlyChart yearMonth={yearMonth} />
      <CategoriesList categories={topCats} totalSecs={totalActive} />
      <BreakdownDonuts segments={breakdownSegments} totalSecs={totalActive} />
      <MonthlyStats yearMonth={yearMonth} />
    </div>
  );
}
```

**Step 2: Update MonthPage**

Replace placeholder with `<MonthView yearMonth={ym} />`.

**Step 3: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 4: Commit**

```bash
git add desktop-ui/src/components/productivity/MonthView.tsx \
      desktop-ui/src/components/productivity/pages/MonthPage.tsx
git commit -m "feat(desktop-ui): assemble MonthView with chart, stats, and breakdown"
```

---

## Task 26: Cleanup old components + verify build

**Files:**
- Delete: `desktop-ui/src/components/productivity/TodaySummary.tsx`
- Delete: `desktop-ui/src/components/productivity/WeeklyTrend.tsx`
- Delete: `desktop-ui/src/components/productivity/FocusStatusCard.tsx`
- Delete: `desktop-ui/src/components/views/Productivity.tsx`

**Step 1: Delete old files**

```bash
rm desktop-ui/src/components/productivity/TodaySummary.tsx
rm desktop-ui/src/components/productivity/WeeklyTrend.tsx
rm desktop-ui/src/components/productivity/FocusStatusCard.tsx
rm desktop-ui/src/components/views/Productivity.tsx
```

**Step 2: Verify no remaining imports**

Search for imports of deleted components and remove any dead references:

```bash
grep -r "TodaySummary\|WeeklyTrend\|FocusStatusCard" desktop-ui/src/ --include="*.tsx" --include="*.ts"
grep -r "from.*views/Productivity" desktop-ui/src/ --include="*.tsx" --include="*.ts"
```

Fix any remaining imports.

**Step 3: Full build verification**

Run: `cd desktop-ui && bun run build`
Expected: PASS

Run: `cargo build -p desktop -p desktop-shared`
Expected: PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor(desktop-ui): remove old productivity components, complete dashboard upgrade"
```

---

## Verification Checklist

Before declaring complete, verify:

- [ ] `cd desktop-ui && bun run build` — clean build, 0 errors
- [ ] `cargo build -p desktop -p desktop-shared` — clean Rust build
- [ ] Navigate to `/productivity` — redirects to day view with today's date
- [ ] Day/Week/Month tabs switch between views
- [ ] Date navigation arrows work (prev/next/today)
- [ ] All cards render without errors (even with empty data)
- [ ] Recharts stacked bar charts render on Week and Month views
- [ ] Score ring SVG renders correctly
- [ ] Breakdown donuts render with Recharts
- [ ] No imports of deleted components remain
- [ ] No TypeScript errors (`bun run build` catches these)
