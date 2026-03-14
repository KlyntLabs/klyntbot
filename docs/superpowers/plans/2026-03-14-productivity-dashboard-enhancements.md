# Productivity Dashboard Enhancements Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix backend data leaks, wire disconnected features, add score trend chart, hourly heatmap, and flexible aggregation to the productivity tool.

**Architecture:** Incremental backend-first approach. Layer 1 fixes data flows in existing Rust crates (converter, repos, migration). Layer 2 adds new Tauri commands. Layer 3 adds frontend components. Each task produces a compilable, testable commit.

**Tech Stack:** Rust (sqlx, tokio, serde), TypeScript/React (Recharts), Tauri IPC

**Spec:** `docs/superpowers/specs/2026-03-14-productivity-dashboard-enhancements-design.md`

---

## Chunk 1: Backend Data Fixes

### Task 1: Expose deep_work and recovery fields in ProductivitySummaryResponse

**Files:**
- Modify: `crates/desktop-shared/src/commands/productivity.rs:8-29` — add 3 fields to `ProductivitySummaryResponse`
- Modify: `crates/app-core/src/handlers/productivity/converters.rs:30-79` — wire fields in `summary_to_response()`

- [ ] **Step 1: Add fields to ProductivitySummaryResponse**

In `crates/desktop-shared/src/commands/productivity.rs`, add after line 28 (`pub active_time_trend: Option<f64>,`):

```rust
pub deep_work_blocks: i64,
pub deep_work_secs: i64,
pub avg_recovery_secs: Option<f64>,
```

- [ ] **Step 2: Wire fields in summary_to_response()**

In `crates/app-core/src/handlers/productivity/converters.rs`, add after `active_time_trend: None,` (line 77):

```rust
deep_work_blocks: s.deep_work_blocks,
deep_work_secs: s.deep_work_secs,
avg_recovery_secs: s.avg_recovery_secs,
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p desktop-shared -p app-core`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/productivity.rs crates/app-core/src/handlers/productivity/converters.rs
git commit -m "fix(productivity): expose deep_work_blocks, deep_work_secs, avg_recovery_secs in summary response"
```

---

### Task 2: Fix top_projects cache bug — add column to migration and repo

The `daily_summaries` table has no `top_projects` column. Since we're pre-release, modify the migration directly.

**Files:**
- Modify: `crates/feature-productivity/migrations/001_productivity_tables.sql:131-153` — add `top_projects` column
- Modify: `crates/feature-productivity/src/repos/daily_summary.rs:7-27,29-76,78,99-151` — add to SummaryRow, From impl, SUMMARY_COLUMNS, upsert

- [ ] **Step 1: Add top_projects column to migration**

In `crates/feature-productivity/migrations/001_productivity_tables.sql`, add `top_projects TEXT,` after the `top_categories TEXT,` line (around line 146 in the `daily_summaries` CREATE TABLE):

```sql
    top_categories       TEXT,
    top_projects         TEXT,
    productivity_score   REAL,
```

- [ ] **Step 2: Add top_projects to SummaryRow struct**

In `crates/feature-productivity/src/repos/daily_summary.rs`, add after `top_categories: Option<String>,` (line 21):

```rust
    top_projects: Option<String>,
```

- [ ] **Step 3: Add top_projects to SUMMARY_COLUMNS**

In `crates/feature-productivity/src/repos/daily_summary.rs`, update the `SUMMARY_COLUMNS` constant (line 78) to include `top_projects` after `top_categories`:

```rust
const SUMMARY_COLUMNS: &str = "date, total_active_secs, total_focus_secs, total_break_secs, total_idle_secs, productive_secs, neutral_secs, distracting_secs, focus_sessions_count, avg_session_quality, interruptions_count, context_switches, top_apps, top_categories, top_projects, productivity_score, ai_summary, deep_work_blocks, deep_work_secs, avg_recovery_secs";
```

- [ ] **Step 4: Deserialize top_projects in From<SummaryRow>**

In the `From<SummaryRow>` impl, replace `top_projects: vec![],` (line 68) with the same deserialization pattern used for top_apps:

```rust
        let top_projects: Vec<crate::types::ProjectUsage> = row
            .top_projects
            .as_deref()
            .and_then(|s| match serde_json::from_str(s) {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!(field = "top_projects", %e, "failed to deserialize JSON from daily_summaries");
                    None
                }
            })
            .unwrap_or_default();
```

And use `top_projects,` in the struct literal instead of `top_projects: vec![],`.

- [ ] **Step 5: Add top_projects to upsert query**

In the `upsert()` method, add `top_projects` to both the INSERT column list and the ON CONFLICT UPDATE clause. Add a new bind for the serialized JSON:

After `top_categories_json`:
```rust
        let top_projects_json = serde_json::to_string(&summary.top_projects)
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
```

Add `top_projects` in the INSERT columns after `top_categories`, update parameter numbering (top_projects becomes ?15, shifting subsequent params), and add to ON CONFLICT:
```rust
                   top_projects = excluded.top_projects,
```

Bind `&top_projects_json` after `&top_categories_json`.

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(daily_summary)' --no-capture`
Expected: PASS (existing tests should still pass with the schema change)

- [ ] **Step 7: Commit**

```bash
git add crates/feature-productivity/migrations/001_productivity_tables.sql crates/feature-productivity/src/repos/daily_summary.rs
git commit -m "fix(productivity): add top_projects column to daily_summaries — fixes empty projects on cached dates"
```

---

### Task 3: Add aggregate_by_hour to BucketRepo

**Files:**
- Modify: `crates/feature-productivity/src/repos/bucket.rs:10-107` — add `aggregate_by_hour()` method
- Test in existing test module: `crates/feature-productivity/src/repos/bucket.rs:109-254`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/feature-productivity/src/repos/bucket.rs` (uses the production `HourlyRow` struct via `use super::*`):

```rust
    #[tokio::test]
    async fn test_aggregate_by_hour() {
        let pool = setup_pool().await;
        let repo = BucketRepo::new(pool);

        // Create buckets at 10:00 and 10:05 (same hour) and 14:00
        for (time, productive) in &[("10:00:00", 200), ("10:05:00", 100), ("14:00:00", 250)] {
            let bucket = ActivityBucket {
                bucket_start: format!("2026-03-06T{}+00:00", time),
                date: "2026-03-06".to_string(),
                dominant_app: None,
                dominant_site: None,
                dominant_category: None,
                productive_secs: *productive,
                neutral_secs: 30,
                distracting_secs: 20,
                idle_secs: 50,
                context_switches: 1,
                focus_depth: None,
                tick_count: 60,
                dominant_project: None,
            };
            repo.upsert(&bucket).await.unwrap();
        }

        let rows = repo.aggregate_by_hour("2026-03-06", "2026-03-06").await.unwrap();
        assert_eq!(rows.len(), 2); // hours 10 and 14
        // Hour 10: 200+100=300 productive
        assert_eq!(rows[0].hour, 10);
        assert_eq!(rows[0].productive_secs, 300);
        // Hour 14: 250 productive
        assert_eq!(rows[1].hour, 14);
        assert_eq!(rows[1].productive_secs, 250);
        // total_secs is computed
        assert_eq!(rows[0].total_secs, 300 + 60 + 40 + 100); // productive + neutral + distracting + idle
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(aggregate_by_hour)'`
Expected: FAIL — `aggregate_by_hour` method doesn't exist

- [ ] **Step 3: Add the HourlyRow struct and aggregate_by_hour method**

Add just above the `#[cfg(test)]` block in `crates/feature-productivity/src/repos/bucket.rs`:

```rust
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyRow {
    pub hour: i32,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub idle_secs: i64,
    pub total_secs: i64,
}
```

Add method to `impl BucketRepo`:

```rust
    pub async fn aggregate_by_hour(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<HourlyRow>> {
        let rows = sqlx::query_as::<_, HourlyRow>(
            r#"SELECT
                   CAST(strftime('%H', bucket_start) AS INTEGER) as hour,
                   COALESCE(SUM(productive_secs), 0) as productive_secs,
                   COALESCE(SUM(neutral_secs), 0) as neutral_secs,
                   COALESCE(SUM(distracting_secs), 0) as distracting_secs,
                   COALESCE(SUM(idle_secs), 0) as idle_secs,
                   COALESCE(SUM(productive_secs) + SUM(neutral_secs) + SUM(distracting_secs) + SUM(idle_secs), 0) as total_secs
               FROM activity_buckets
               WHERE date >= ?1 AND date <= ?2
               GROUP BY hour
               ORDER BY hour"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p feature-productivity -E 'test(aggregate_by_hour)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/src/repos/bucket.rs
git commit -m "feat(productivity): add aggregate_by_hour to BucketRepo for hourly heatmap"
```

---

### Task 4: Add new Tauri commands — productivity_patterns and productivity_hourly_breakdown

**Files:**
- Modify: `crates/desktop-shared/src/commands/productivity.rs:222-237` — add 2 new response types
- Modify: `crates/app-core/src/handlers/productivity/` — add new handler file or extend summaries.rs
- Modify: `crates/desktop/src/commands/productivity.rs:444-673` — add 2 new commands + DEV_COMMANDS + dispatch_dev

- [ ] **Step 1: Add response types to desktop-shared**

In `crates/desktop-shared/src/commands/productivity.rs`, add after `WeeklyAssessmentResponse` (after line 222):

```rust
// ── Productivity Patterns ────────────────────────────────────────────

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

// ── Hourly Breakdown ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyBreakdownResponse {
    pub hour: u32,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub idle_secs: i64,
    pub total_secs: i64,
    pub productive_ratio: f64,
}
```

Also add the new types to the pub use in `crates/desktop-shared/src/commands/mod.rs` if there is a barrel export (check the existing pattern — if all types are individually imported by consumers, skip this step).

- [ ] **Step 2: Add app-core handlers**

In `crates/app-core/src/handlers/productivity/summaries.rs`, add two new methods to the `impl AppCore` block:

```rust
    pub async fn productivity_patterns(
        &self,
        days: Option<u32>,
    ) -> Result<ProductivityPatternsResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let analyzer = feature_productivity::ProductivityPatternAnalyzer::new(repos);
        let patterns = analyzer.analyze(days.unwrap_or(14)).await.map_err(map_prod_err)?;
        Ok(ProductivityPatternsResponse {
            peak_focus_hours: patterns.peak_focus_hours,
            avg_session_mins: patterns.avg_session_mins,
            productive_ratio: patterns.productive_ratio,
            avg_context_switches: patterns.avg_context_switches,
            best_day_of_week: patterns.best_day_of_week.map(|d| d.to_string()),
            days_analyzed: patterns.days_analyzed,
        })
    }

    pub async fn productivity_hourly_breakdown(
        &self,
        start_date: String,
        end_date: String,
    ) -> Result<Vec<HourlyBreakdownResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let rows = repos
            .buckets
            .aggregate_by_hour(&start_date, &end_date)
            .await
            .map_err(map_prod_err)?;
        Ok(rows
            .into_iter()
            .map(|r| HourlyBreakdownResponse {
                hour: r.hour as u32,
                productive_secs: r.productive_secs,
                neutral_secs: r.neutral_secs,
                distracting_secs: r.distracting_secs,
                idle_secs: r.idle_secs,
                total_secs: r.total_secs,
                productive_ratio: if r.total_secs > 0 {
                    r.productive_secs as f64 / r.total_secs as f64
                } else {
                    0.0
                },
            })
            .collect())
    }
```

Add the necessary imports at the top of `summaries.rs`:
```rust
use desktop_shared::commands::{ProductivityPatternsResponse, HourlyBreakdownResponse};
```

Also add `ProductivityPatternAnalyzer` to the re-exports in `crates/feature-productivity/src/lib.rs` if not already exported:
```rust
pub use patterns::ProductivityPatternAnalyzer;
```

- [ ] **Step 3: Add Tauri commands**

In `crates/desktop/src/commands/productivity.rs`, add before the `// ── Dev server dispatch` comment:

```rust
#[tauri::command]
pub async fn productivity_patterns(
    core: State<'_, Arc<AppCore>>,
    days: Option<u32>,
) -> Result<ProductivityPatternsResponse, ApiError> {
    core.productivity_patterns(days).await
}

#[tauri::command]
pub async fn productivity_hourly_breakdown(
    core: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
) -> Result<Vec<HourlyBreakdownResponse>, ApiError> {
    core.productivity_hourly_breakdown(start_date, end_date).await
}
```

Add the necessary imports at the top of the file:
```rust
use desktop_shared::commands::{ProductivityPatternsResponse, HourlyBreakdownResponse};
```

- [ ] **Step 4: Add to DEV_COMMANDS and dispatch_dev**

In `DEV_COMMANDS` array (line 457), add:
```rust
    "productivity_patterns",
    "productivity_hourly_breakdown",
```

In `dispatch_dev()` function, add before `_ => return None,`:
```rust
        "productivity_patterns" => dev::val(
            core.productivity_patterns(dev::get(body, "days")).await,
        ),
        "productivity_hourly_breakdown" => {
            let start_date = try_field!(dev::get_str(body, "startDate"));
            let end_date = try_field!(dev::get_str(body, "endDate"));
            dev::val(core.productivity_hourly_breakdown(start_date, end_date).await)
        }
```

- [ ] **Step 5: Register Tauri commands in the app builder**

In `crates/desktop/src/main.rs`, find the `.invoke_handler(tauri::generate_handler![...])` macro invocation. Add `productivity::productivity_patterns` and `productivity::productivity_hourly_breakdown` to the handler list, following the existing pattern for other productivity commands.

- [ ] **Step 6: Verify compilation**

Run: `cargo build -p desktop`
Expected: compiles (may have warnings, but no errors)

- [ ] **Step 7: Run dev_server_covers_all test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: PASS (the new commands are in DEV_COMMANDS)

- [ ] **Step 8: Commit**

```bash
git add crates/desktop-shared/src/commands/productivity.rs crates/app-core/src/handlers/productivity/summaries.rs crates/desktop/src/commands/productivity.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add productivity_patterns and productivity_hourly_breakdown Tauri commands"
```

---

### Task 5: Add group_by parameter to productivity tool's activity_summary

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs:106-134,521-578,581-606` — add group_by param, schema, and handler logic

- [ ] **Step 0: Add aggregate_by_project to ActivityEventRepo**

In `crates/feature-productivity/src/repos/activity_event.rs`, add a new method to `impl ActivityEventRepo`:

```rust
    pub async fn aggregate_by_project(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row { project_id: String, total_secs: i64 }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT project_id, COALESCE(SUM(duration_secs), 0) as total_secs
               FROM activity_events
               WHERE started_at >= ?1 AND started_at <= ?2 || 'T23:59:59'
                 AND project_id IS NOT NULL
               GROUP BY project_id
               ORDER BY total_secs DESC"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|r| (r.project_id, r.total_secs)).collect())
    }
```

- [ ] **Step 1: Add group_by to parameters() JSON schema**

In `crates/feature-productivity/src/tool/mod.rs`, in the `parameters()` method, add after the `"end_date"` property:

```rust
                "group_by": {
                    "type": "string",
                    "enum": ["day", "week", "month", "project"],
                    "description": "Group results by time period or project (optional, for activity_summary)"
                },
```

- [ ] **Step 2: Implement grouped activity_summary**

In `handle_activity_summary()`, add group_by handling. Replace the existing method body with:

```rust
    async fn handle_activity_summary(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let start_date = p.required_str("start_date")?;
        let end_date = p.required_str("end_date")?;
        let group_by = p.optional_str("group_by")?;

        let summaries = self
            .repos
            .summaries
            .list_range(start_date, end_date)
            .await?;

        if summaries.is_empty() {
            return Ok(format!("No data for {start_date} to {end_date}."));
        }

        match group_by {
            Some("day") => {
                let mut lines = vec![format!("Daily breakdown ({start_date} to {end_date}):")];
                for s in &summaries {
                    let score = s.productivity_score.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "N/A".into());
                    lines.push(format!(
                        "- {}: active {} | productive {} | score {}",
                        s.date,
                        format_duration(s.total_active_secs),
                        format_duration(s.productive_secs),
                        score,
                    ));
                }
                Ok(lines.join("\n"))
            }
            Some("week") => {
                let mut weeks: std::collections::BTreeMap<String, (i64, i64, i64, Vec<Option<f64>>)> =
                    std::collections::BTreeMap::new();
                for s in &summaries {
                    let week_key = chrono::NaiveDate::parse_from_str(&s.date, "%Y-%m-%d")
                        .map(|d| format!("{}-W{:02}", d.iso_week().year(), d.iso_week().week()))
                        .unwrap_or_else(|_| "unknown".into());
                    let entry = weeks.entry(week_key).or_insert((0, 0, 0, vec![]));
                    entry.0 += s.total_active_secs;
                    entry.1 += s.productive_secs;
                    entry.2 += s.distracting_secs;
                    entry.3.push(s.productivity_score);
                }
                let mut lines = vec![format!("Weekly breakdown ({start_date} to {end_date}):")];
                for (week, (active, productive, distracting, scores)) in &weeks {
                    let avg_score = {
                        let valid: Vec<f64> = scores.iter().filter_map(|s| *s).collect();
                        if valid.is_empty() { "N/A".into() } else { format!("{:.0}", valid.iter().sum::<f64>() / valid.len() as f64) }
                    };
                    lines.push(format!(
                        "- {week}: active {} | productive {} | distracting {} | avg score {avg_score}",
                        format_duration(*active),
                        format_duration(*productive),
                        format_duration(*distracting),
                    ));
                }
                Ok(lines.join("\n"))
            }
            Some("month") => {
                let mut months: std::collections::BTreeMap<String, (i64, i64, i64, Vec<Option<f64>>)> =
                    std::collections::BTreeMap::new();
                for s in &summaries {
                    let month_key = s.date.get(..7).unwrap_or("unknown").to_string();
                    let entry = months.entry(month_key).or_insert((0, 0, 0, vec![]));
                    entry.0 += s.total_active_secs;
                    entry.1 += s.productive_secs;
                    entry.2 += s.distracting_secs;
                    entry.3.push(s.productivity_score);
                }
                let mut lines = vec![format!("Monthly breakdown ({start_date} to {end_date}):")];
                for (month, (active, productive, distracting, scores)) in &months {
                    let avg_score = {
                        let valid: Vec<f64> = scores.iter().filter_map(|s| *s).collect();
                        if valid.is_empty() { "N/A".into() } else { format!("{:.0}", valid.iter().sum::<f64>() / valid.len() as f64) }
                    };
                    lines.push(format!(
                        "- {month}: active {} | productive {} | distracting {} | avg score {avg_score}",
                        format_duration(*active),
                        format_duration(*productive),
                        format_duration(*distracting),
                    ));
                }
                Ok(lines.join("\n"))
            }
            Some("project") => {
                // Aggregate per-project from activity_events using SQL (not in-memory)
                let results = self.repos.events.aggregate_by_project(start_date, end_date).await?;
                let mut lines = vec![format!("By project ({start_date} to {end_date}):")];
                for (pid, secs) in &results {
                    lines.push(format!("- {pid}: {}", format_duration(*secs)));
                }
                if results.is_empty() {
                    lines.push("  No project data found.".into());
                }
                Ok(lines.join("\n"))
            }
            Some(other) => Err(ToolError::InvalidParams(format!("group_by must be 'day', 'week', 'month', or 'project', got '{other}'")).into()),
            None => {
                // Original flat aggregate behavior
                let total_active: i64 = summaries.iter().map(|s| s.total_active_secs).sum();
                let total_productive: i64 = summaries.iter().map(|s| s.productive_secs).sum();
                let total_distracting: i64 = summaries.iter().map(|s| s.distracting_secs).sum();
                let total_focus: i64 = summaries.iter().map(|s| s.focus_sessions_count).sum();
                Ok(format!(
                    "Activity summary ({} to {}):\n- Days tracked: {}\n- Total active: {}\n- Productive: {}\n- Distracting: {}\n- Focus sessions: {}",
                    start_date, end_date, summaries.len(),
                    format_duration(total_active), format_duration(total_productive),
                    format_duration(total_distracting), total_focus,
                ))
            }
        }
    }
```

Add `use chrono::Datelike;` at the top of the file if not already imported.

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p feature-productivity`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): add group_by parameter to activity_summary tool action"
```

---

### Task 6: Run full backend test suite

- [ ] **Step 1: Run all productivity tests**

Run: `cargo nextest run -p feature-productivity`
Expected: all PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (except known desktop exceptions)

---

## Chunk 2: Frontend Changes

### Task 7: Update frontend TypeScript types

**Files:**
- Modify: `desktop-ui/src/shared/types/productivity.ts:1-24` — add new fields and types

- [ ] **Step 1: Add deep_work fields to ProductivitySummary**

In `desktop-ui/src/shared/types/productivity.ts`, add after `activeTimeTrend` (line 23):

```typescript
  deepWorkBlocks: number;
  deepWorkSecs: number;
  avgRecoverySecs: number | null;
```

- [ ] **Step 2: Add new interfaces**

Add at the end of the file:

```typescript
// ── Productivity Patterns ──────────────────────────────────

export interface ProductivityPatterns {
  peakFocusHours: number[];
  avgSessionMins: number;
  productiveRatio: number;
  avgContextSwitches: number;
  bestDayOfWeek: string | null;
  daysAnalyzed: number;
}

// ── Hourly Breakdown ───────────────────────────────────────

export interface HourlyBreakdown {
  hour: number;
  productiveSecs: number;
  neutralSecs: number;
  distractingSecs: number;
  idleSecs: number;
  totalSecs: number;
  productiveRatio: number;
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/types/productivity.ts
git commit -m "feat(ui): add deep work, patterns, and hourly breakdown types"
```

---

### Task 8: Show deep work metrics in SummaryPanel

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx` — add deep work row and recovery time

- [ ] **Step 1: Add deep work display**

In `SummaryPanel.tsx`, find where the score sub-bars are rendered (look for `ScoreBar` components). After the last `ScoreBar`, add:

```tsx
{/* Deep Work */}
{summary && summary.deepWorkBlocks > 0 && (
  <div className="flex items-center justify-between text-xs text-muted px-1 mt-2">
    <span>{summary.deepWorkBlocks} deep work block{summary.deepWorkBlocks !== 1 ? "s" : ""}</span>
    <span>{Math.floor(summary.deepWorkSecs / 3600)}h {Math.floor((summary.deepWorkSecs % 3600) / 60)}m</span>
  </div>
)}

{/* Recovery Time */}
{summary?.avgRecoverySecs != null && (
  <div className="text-xs text-muted px-1 mt-1">
    Avg recovery: {Math.round(summary.avgRecoverySecs)}s
  </div>
)}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/SummaryPanel.tsx
git commit -m "feat(ui): show deep work blocks and recovery time in SummaryPanel"
```

---

### Task 9: Add PatternsCard component

**Files:**
- Create: `desktop-ui/src/features/productivity/components/PatternsCard.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx` — import and render

- [ ] **Step 1: Create PatternsCard component**

```tsx
import { useQuery } from "../../../shared/hooks/useQuery";
import type { ProductivityPatterns } from "../../../shared/types/productivity";

export function PatternsCard() {
  const { data } = useQuery<ProductivityPatterns>(
    "productivity_patterns", {}, undefined, 5 * 60 * 1000
  );

  if (!data || data.daysAnalyzed < 3) return null;

  const peakLabel = data.peakFocusHours.length > 0
    ? data.peakFocusHours.map(h => `${h}:00`).join(", ")
    : "—";

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-xs font-medium text-foreground">Your Patterns</div>
      <div className="text-xs text-muted space-y-0.5">
        <div>Peak hours: {peakLabel}</div>
        {data.bestDayOfWeek && <div>Best day: {data.bestDayOfWeek}</div>}
        <div>Avg session: {Math.round(data.avgSessionMins)}min</div>
        <div className="text-[10px] text-muted/60">{data.daysAnalyzed} days analyzed</div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Import and render in SummaryPanel**

In `SummaryPanel.tsx`, import and add below the weekly sparkline section:

```tsx
import { PatternsCard } from "../../productivity/components/PatternsCard";
```

Then render `<PatternsCard />` after the sparkline.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/productivity/components/PatternsCard.tsx desktop-ui/src/features/dashboard/components/SummaryPanel.tsx
git commit -m "feat(ui): add PatternsCard showing peak hours, best day, avg session"
```

---

### Task 10: Add WeeklyAssessmentCard

**Files:**
- Create: `desktop-ui/src/features/productivity/components/WeeklyAssessmentCard.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx` — render in week view

- [ ] **Step 1: Create WeeklyAssessmentCard component**

```tsx
import { useQuery } from "../../../shared/hooks/useQuery";
import type { WeeklyAssessment } from "../../../shared/types/productivity";

interface Props {
  weekStart: string;
}

export function WeeklyAssessmentCard({ weekStart }: Props) {
  const { data } = useQuery<WeeklyAssessment | null>(
    "productivity_weekly_assessment",
    { weekStart },
  );

  if (!data) return null;

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-xs font-medium text-foreground">Weekly Assessment</div>
      <div className="text-xs text-muted space-y-0.5">
        {data.avgScore != null && <div>Avg score: {data.avgScore.toFixed(0)}</div>}
        {data.totalFocusMins != null && (
          <div>Focus: {Math.floor(data.totalFocusMins / 60)}h {data.totalFocusMins % 60}m</div>
        )}
        {data.summary && <div className="italic">{data.summary}</div>}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Import and render in SummaryPanel (week view only)**

In `SummaryPanel.tsx`, conditionally render when the current view mode is "week":

```tsx
import { WeeklyAssessmentCard } from "../../productivity/components/WeeklyAssessmentCard";
// ... in the render, after PatternsCard:
{viewMode === "week" && weekStart && <WeeklyAssessmentCard weekStart={weekStart} />}
```

(Determine `viewMode` and `weekStart` from context/props already available in SummaryPanel.)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/productivity/components/WeeklyAssessmentCard.tsx desktop-ui/src/features/dashboard/components/SummaryPanel.tsx
git commit -m "feat(ui): add WeeklyAssessmentCard in SummaryPanel for week view"
```

---

### Task 11: Add ScoreTrendChart (inline in week/month views)

**Files:**
- Create: `desktop-ui/src/features/productivity/components/ScoreTrendChart.tsx`
- Modify: `desktop-ui/src/features/productivity/components/WeeklyChart.tsx` or `WeeklyStats.tsx` — render below existing chart
- Modify: `desktop-ui/src/features/productivity/components/MonthlyChart.tsx` or `MonthlyStats.tsx` — same

- [ ] **Step 1: Create ScoreTrendChart component**

```tsx
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine } from "recharts";
import type { ProductivitySummary } from "../../../shared/types/productivity";

interface Props {
  summaries: ProductivitySummary[];
}

export function ScoreTrendChart({ summaries }: Props) {
  const data = summaries
    .filter(s => s.productivityScore != null)
    .map(s => ({
      date: s.date.slice(5), // "MM-DD"
      score: Math.round(s.productivityScore ?? 0),
      baseline: s.scoreTrend != null && s.productivityScore != null
        ? Math.round(s.productivityScore - s.scoreTrend)
        : null,
    }));

  if (data.length < 2) return null;

  return (
    <div className="mt-2">
      <div className="text-xs font-medium text-muted mb-1 px-1">Score Trend</div>
      <ResponsiveContainer width="100%" height={120}>
        <LineChart data={data} margin={{ top: 4, right: 8, bottom: 0, left: -20 }}>
          <XAxis dataKey="date" tick={{ fontSize: 10 }} />
          <YAxis domain={[0, 100]} tick={{ fontSize: 10 }} />
          <Tooltip
            contentStyle={{ fontSize: 11 }}
            formatter={(value: number, name: string) =>
              [value, name === "score" ? "Score" : "Baseline"]
            }
          />
          <Line
            type="monotone"
            dataKey="score"
            stroke="var(--accent)"
            strokeWidth={2}
            dot={{ r: 3 }}
          />
          <Line
            type="monotone"
            dataKey="baseline"
            stroke="var(--text-muted)"
            strokeWidth={1}
            strokeDasharray="4 4"
            dot={false}
            connectNulls
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
```

- [ ] **Step 2: Integrate into WeeklyStats/WeeklyChart area**

Import `ScoreTrendChart` in the component that renders the weekly overview (check `WeeklyStats.tsx` or `WeekView.tsx` — whichever renders the `WeeklyChart`). Pass the summaries data:

```tsx
import { ScoreTrendChart } from "./ScoreTrendChart";
// ... after the WeeklyChart:
<ScoreTrendChart summaries={summaries} />
```

Do the same in the monthly view equivalent.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/productivity/components/ScoreTrendChart.tsx
git commit -m "feat(ui): add ScoreTrendChart with score line and baseline trend"
```

---

### Task 12: Add HourlyHeatmap in SummaryPanel

**Files:**
- Create: `desktop-ui/src/features/productivity/components/HourlyHeatmap.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx` — import and render

- [ ] **Step 1: Create HourlyHeatmap component**

```tsx
import { useQuery } from "../../../shared/hooks/useQuery";
import type { HourlyBreakdown } from "../../../shared/types/productivity";

interface Props {
  startDate: string;
  endDate: string;
}

export function HourlyHeatmap({ startDate, endDate }: Props) {
  const { data } = useQuery<HourlyBreakdown[]>(
    "productivity_hourly_breakdown",
    { startDate, endDate },
    undefined,
    60_000,
  );

  if (!data || data.length === 0) return null;

  // Filter to working hours (6-22)
  const working = data.filter(h => h.hour >= 6 && h.hour <= 22);
  const maxRatio = Math.max(...working.map(h => h.productiveRatio), 0.01);

  const peakHour = working.reduce((best, h) =>
    h.productiveRatio > best.productiveRatio ? h : best
  , working[0]);

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-xs font-medium text-foreground">
        Hourly Productivity
        {peakHour && (
          <span className="text-muted font-normal ml-1">
            Peak: {peakHour.hour}:00
          </span>
        )}
      </div>
      <div className="space-y-px">
        {working.map(h => {
          const width = (h.productiveRatio / maxRatio) * 100;
          return (
            <div key={h.hour} className="flex items-center gap-1.5">
              <span className="text-[10px] text-muted w-6 text-right tabular-nums">
                {h.hour}
              </span>
              <div className="flex-1 h-2.5 rounded-sm bg-surface-raised overflow-hidden">
                <div
                  className="h-full rounded-sm bg-accent transition-all"
                  style={{
                    width: `${width}%`,
                    opacity: 0.3 + (h.productiveRatio * 0.7),
                  }}
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

- [ ] **Step 2: Import and render in SummaryPanel**

In `SummaryPanel.tsx`, import and add below the PatternsCard:

```tsx
import { HourlyHeatmap } from "../../productivity/components/HourlyHeatmap";
```

Render with the current view's date range:
```tsx
<HourlyHeatmap startDate={dateRange.start} endDate={dateRange.end} />
```

(Determine from context how `SummaryPanel` receives the current date/range — likely via props or context.)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/productivity/components/HourlyHeatmap.tsx desktop-ui/src/features/dashboard/components/SummaryPanel.tsx
git commit -m "feat(ui): add HourlyHeatmap showing productive ratio by hour of day"
```

---

### Task 13: Final verification

- [ ] **Step 1: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors

- [ ] **Step 2: Run frontend type check**

Run: `cd desktop-ui && bun run build`
Expected: builds without type errors

- [ ] **Step 3: Run full Rust test suite**

Run: `cargo nextest run --workspace`
Expected: all pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (except known desktop exceptions)
