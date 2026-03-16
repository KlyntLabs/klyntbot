# Productivity Dashboard Enhancements

**Date:** 2026-03-14
**Status:** Approved
**Scope:** Fix backend data leaks, wire disconnected features, add score trend chart, add hourly heatmap, add flexible aggregation to productivity tool

## Context

Analysis of the productivity subsystem revealed:
1. Three valuable metrics (`deep_work_blocks`, `deep_work_secs`, `avg_recovery_secs`) are computed by the aggregator and stored in the DB but silently dropped in `summary_to_response()` converter
2. `top_projects` always returns empty for cached past-date queries due to a deserialization bug
3. `ProductivityPatternAnalyzer` (peak hours, best day, avg session) is not exposed via any Tauri command
4. `productivity_weekly_assessment` command exists but no frontend component calls it
5. No productivity score trend visualization across days
6. No hour-of-day productivity breakdown
7. The productivity tool's `activity_summary` action lacks flexible grouping

## Layer 1: Backend Data Fixes

### 1a. Converter Data Leak Fix

**Files:** `crates/desktop-shared/src/commands/productivity.rs`, `crates/app-core/src/handlers/productivity/converters.rs`

Add three fields to `ProductivitySummaryResponse`:

```rust
pub deep_work_blocks: i64,
pub deep_work_secs: i64,
pub avg_recovery_secs: Option<f64>,
```

Wire in `summary_to_response()`:

```rust
deep_work_blocks: summary.deep_work_blocks,
deep_work_secs: summary.deep_work_secs,
avg_recovery_secs: summary.avg_recovery_secs,
```

Add corresponding fields to the frontend TypeScript type `ProductivitySummary` in `desktop-ui/src/shared/types/productivity.ts`.

### 1b. Fix `top_projects` Cache Bug

**File:** `crates/feature-productivity/src/repos/daily_summary.rs`

Three-part fix required:

1. **Add `top_projects` column to `SummaryRow` struct** (`daily_summary.rs` near L7-27): add `pub top_projects: Option<String>` field
2. **Add `top_projects` to `SUMMARY_COLUMNS`** (`daily_summary.rs` near L78): the column must be in the SELECT list or `sqlx::FromRow` will fail at runtime
3. **Add `top_projects` to the `upsert()` INSERT/UPDATE query** (`daily_summary.rs` near L104-126): without this the computed data is never persisted to the cache
4. **Deserialize in `From<SummaryRow>` for `DailySummary`** (`daily_summary.rs` near L68): parse `row.top_projects` via `serde_json::from_str()` into `Vec<ProjectUsage>`, matching how `top_apps` and `top_categories` are already handled. Currently hardcoded to `vec![]`.

### 1c. Wire ProductivityPatternAnalyzer

**Files:**
- `crates/app-core/src/handlers/productivity/` — new handler function
- `crates/desktop-shared/src/commands/productivity.rs` — new response type
- `crates/desktop/src/commands/productivity.rs` — new Tauri command

New response type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityPatternsResponse {
    pub peak_focus_hours: Vec<u32>,
    pub avg_session_mins: f64,
    pub productive_ratio: f64,
    pub avg_context_switches: f64,
    pub best_day_of_week: Option<String>,
    pub days_analyzed: usize,
}
```

New app-core handler: `productivity_patterns(repos, days: Option<u32>) -> Result<ProductivityPatternsResponse>`. Default to 14 days. Construct `ProductivityPatternAnalyzer::new(repos.clone())` inline in the handler, then call `.analyze(days)`. Convert `Weekday` to display string via `weekday.to_string()` (produces "Mon", "Tue", etc.) before returning.

New Tauri command: `productivity_patterns` — thin adapter calling the app-core handler. Must be added to `DEV_COMMANDS` array and `dispatch_dev()` in `crates/desktop/src/commands/productivity.rs`.

### 1d. Hourly Breakdown Command

**Files:** Same as 1c pattern.

New response type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyBreakdownEntry {
    pub hour: u32,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub idle_secs: i64,
    pub total_secs: i64,
    pub productive_ratio: f64,
}
```

New repo method on `BucketRepo` (accessed as `repos.buckets`): `aggregate_by_hour(start_date, end_date) -> Vec<HourlyBreakdownEntry>`.

SQL (note: `total_secs` is computed, not a column; `bucket_start` stores UTC ISO-8601 timestamps so `strftime('%H', ...)` is safe):
```sql
SELECT
    CAST(strftime('%H', bucket_start) AS INTEGER) as hour,
    SUM(productive_secs) as productive_secs,
    SUM(neutral_secs) as neutral_secs,
    SUM(distracting_secs) as distracting_secs,
    SUM(idle_secs) as idle_secs,
    SUM(productive_secs) + SUM(neutral_secs) + SUM(distracting_secs) + SUM(idle_secs) as total_secs
FROM activity_buckets
WHERE date BETWEEN ?1 AND ?2
GROUP BY hour
ORDER BY hour
```

Compute `productive_ratio` in Rust after the query: `productive_secs as f64 / total_secs.max(1) as f64`.

New app-core handler: `productivity_hourly_breakdown(repos, start_date, end_date)`.

New Tauri command: `productivity_hourly_breakdown`. Must be added to `DEV_COMMANDS` and `dispatch_dev()`.

### 1e. Add `group_by` to Productivity Tool

**File:** `crates/feature-productivity/src/tool/mod.rs`

Add optional `group_by` parameter to the `activity_summary` action. Values: `day`, `week`, `month`, `project`.

When `group_by` is provided:
- `day`: Return one summary line per day in the range
- `week`: Aggregate summaries into ISO weeks
- `month`: Aggregate summaries into months
- `project`: Group time entries by project_id, return per-project totals

When omitted, behavior is unchanged (flat aggregate).

Implementation: For `day`, use existing `repos.summaries.list_range()` and format each. For `week`/`month`, group the daily summaries by week/month key and sum their numeric fields. For `project`, query `activity_events` directly with `SELECT project_id, SUM(duration_secs) FROM activity_events WHERE started_at BETWEEN ? AND ? AND project_id IS NOT NULL GROUP BY project_id` — do NOT sum `top_projects` from daily summaries as that would double-count.

Must also add `group_by` to the tool's `parameters()` JSON schema so LLMs can discover the parameter:
```json
"group_by": {
    "type": "string",
    "enum": ["day", "week", "month", "project"],
    "description": "Group results by time period or project (optional)"
}
```

## Layer 2: Frontend — New Metrics in SummaryPanel

**File:** `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx`

### Deep Work Metrics
Add a "Deep Work" row below the existing score sub-bars:
- Icon + "{N} blocks" + "{Xh Ym} total"
- Uses `deep_work_blocks` and `deep_work_secs` from `ProductivitySummary`

### Recovery Time
Add "Avg recovery: {N}s" as a subtle stat line below the deep work row. Only shown when `avg_recovery_secs` is not null.

### Patterns Card
New `PatternsCard` component shown in the SummaryPanel below the weekly sparkline:
- "Peak hours: 9-11am"
- "Best day: Tuesday"
- "Avg session: 42min"
- Data source: `productivity_patterns` Tauri command
- Cache with 5 minute stale time (patterns update as sessions complete during the day)

### Weekly Assessment
When viewing week view, show a `WeeklyAssessmentCard`:
- Average score, total focus time, summary text
- Data source: `productivity_weekly_assessment` Tauri command (already exists)

## Layer 3: Frontend — Score Trend Line (Inline)

**File:** New component `desktop-ui/src/features/productivity/components/ScoreTrendChart.tsx`

**Placement:** Below the existing `WeeklyChart`/`MonthlyChart` stacked bar charts in the week and month calendar views.

**Component:**
- Recharts `LineChart` with `ResponsiveContainer`
- X-axis: date labels (Mon, Tue, Wed... for week; 1, 2, 3... for month)
- Y-axis: 0-100 scale
- Primary line: daily productivity score (solid, theme accent color)
- Secondary line: reconstructed baseline (dashed, muted color) — computed as `score - scoreTrend` since `scoreTrend` is a delta from the 28-day rolling average, not the average itself
- Tooltip: date, score, trend delta
- Height: 120px (compact, complementary to the stacked bar above it)

**Data source:** `productivity_summary_range` already returns per-day `productivityScore` and `scoreTrend`. No new backend work needed.

**Integration:** Import into `WeekCalendarView` and `MonthCalendarView`, render below the existing chart.

## Layer 4: Frontend — Hour-of-Day Heatmap (Sidebar)

**File:** New component `desktop-ui/src/features/productivity/components/HourlyHeatmap.tsx`

**Placement:** In `SummaryPanel`, below the patterns card.

**Component:**
- Compact vertical list of hours 6:00-22:00 (working hours)
- Each row: hour label + horizontal bar colored by productive ratio
- Color scale: low productivity (muted/gray) → high productivity (accent/green)
- Highlight the peak hour(s) with a subtle label
- Width: fits the sidebar (~200px)

**Data source:** New `productivity_hourly_breakdown` Tauri command.

**Query:** Fetch for the currently viewed date range (today for day view, week range for week view). Cache with 60s stale time.

## Frontend Type Updates

Add to `desktop-ui/src/shared/types/productivity.ts`:

```typescript
// In ProductivitySummary:
deepWorkBlocks: number
deepWorkSecs: number
avgRecoverySecs: number | null

// New types:
export interface ProductivityPatterns {
  peakFocusHours: number[]
  avgSessionMins: number
  productiveRatio: number
  avgContextSwitches: number
  bestDayOfWeek: string | null
  daysAnalyzed: number
}

export interface HourlyBreakdown {
  hour: number
  productiveSecs: number
  neutralSecs: number
  distractingSecs: number
  idleSecs: number
  totalSecs: number
  productiveRatio: number
}
```

## Testing

- **Backend:** Unit tests for new repo methods (hourly aggregation), converter field mapping, pattern analyzer wiring, group_by tool action
- **Frontend:** Verify new fields render in SummaryPanel, charts render with mock data

## Non-Goals

- Billable tracking, team features, multi-user — intentionally excluded (personal tracker)
- Redesigning existing chart components — only adding new ones
- Real-time updates for trend/heatmap charts — stale-time caching is sufficient
