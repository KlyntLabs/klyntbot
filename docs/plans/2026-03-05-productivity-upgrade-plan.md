# Productivity Tracking Upgrade Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the productivity tracking system from 61/100 to ~80/100 by adding a productivity score, AI summaries, historical comparison, Pomodoro timer, data export, goals, and manual time entry.

**Architecture:** Extend the existing `feature-productivity` crate with new types, repo methods, tool actions, and a migration. The `ProductivityContextSource` gains productivity score injection. AI summaries use the existing LLM provider infrastructure via a new `ProductivityHandler` trait (dependency inversion pattern). No new crates needed.

**Tech Stack:** Rust, SQLite (sqlx), tokio, serde, chrono. LLM integration via handler trait injected from the agent layer.

---

## Phase Overview

| Phase | Tasks | What it delivers |
|-------|-------|------------------|
| **1: Productivity Score** | Tasks 1-5 | Single 0-100 daily score, injected into agent context |
| **2: AI Daily Summary** | Tasks 6-9 | LLM-generated natural language daily summary |
| **3: Pomodoro Timer** | Tasks 10-12 | Work/break cycle timer using existing SessionType::Pomodoro |
| **4: Historical Comparison** | Tasks 13-15 | "Today vs yesterday", "this week vs last week" |
| **5: Goals & Progress** | Tasks 16-20 | Daily/weekly targets with progress tracking |
| **6: Manual Time Entry** | Tasks 21-23 | Log meetings, offline work manually |
| **7: Data Export** | Tasks 24-25 | CSV export of activity data |
| **8: Retention Cleanup** | Task 26 | Background job to purge old data |

---

## Task 1: Add ProductivityScore type

**Files:**
- Modify: `crates/feature-productivity/src/types.rs`

**Step 1: Add the ProductivityScore struct to types.rs**

Append after the `NudgeRecord` impl block (~line 228):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityScore {
    pub date: String,
    pub overall: f64,
    pub productive_ratio_score: f64,
    pub focus_quality_score: f64,
    pub distraction_score: f64,
    pub continuity_score: f64,
}
```

**Step 2: Commit**

```bash
git add crates/feature-productivity/src/types.rs
git commit -m "feat(productivity): add ProductivityScore type"
```

---

## Task 2: Add productivity score column to daily_summaries

**Files:**
- Create: `crates/feature-productivity/migrations/002_productivity_score.sql`
- Modify: `crates/feature-productivity/src/lib.rs` (add migration to list)

**Step 1: Create migration file**

```sql
-- Add productivity score to daily summaries
ALTER TABLE daily_summaries ADD COLUMN productivity_score REAL;

-- Goals table for daily/weekly targets
CREATE TABLE IF NOT EXISTS productivity_goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_type     TEXT NOT NULL DEFAULT 'daily',
    metric        TEXT NOT NULL,
    target_value  REAL NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Manual time entries
CREATE TABLE IF NOT EXISTS time_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    description   TEXT NOT NULL,
    category_id   TEXT REFERENCES activity_categories(id),
    project_id    TEXT,
    started_at    TEXT NOT NULL,
    duration_secs INTEGER NOT NULL,
    source        TEXT NOT NULL DEFAULT 'manual',
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_time_entries_started ON time_entries(started_at DESC);
```

**Step 2: Update migrations_static() in lib.rs**

In `crates/feature-productivity/src/lib.rs`, change `migrations_static()` (~line 45) to return both migrations:

```rust
pub fn migrations_static() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 1,
            description: "Create productivity tracking tables".to_string(),
            sql: include_str!("../migrations/001_productivity_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 2,
            description: "Add productivity score, goals, and time entries".to_string(),
            sql: include_str!("../migrations/002_productivity_score.sql").to_string(),
        },
    ]
}
```

Also update `migration_sql()` to return both (or remove it if unused — check callers first).

**Step 3: Commit**

```bash
git add crates/feature-productivity/migrations/002_productivity_score.sql
git add crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add migration for score, goals, time entries"
```

---

## Task 3: Implement productivity score computation

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs`
- Test: inline `#[cfg(test)]` in same file

**Step 1: Write the failing test**

Add to `aggregator.rs` tests module:

```rust
#[tokio::test]
async fn test_compute_productivity_score() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let aggregator = DailyAggregator::new(repos.clone());

    // Insert a mix of productive and distracting activity
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let start = Utc::now() - chrono::Duration::hours(4);

    // 3h productive coding
    repos.events.insert(&ActivityEvent {
        id: None,
        app_name: "VS Code".into(),
        window_title: None,
        bundle_id: None,
        url: None,
        category_id: Some("coding".into()),
        started_at: start,
        ended_at: Some(start + chrono::Duration::hours(3)),
        duration_secs: Some(10800),
        is_idle: false,
        metadata: None,
    }).await.unwrap();

    // 1h distracting
    repos.events.insert(&ActivityEvent {
        id: None,
        app_name: "Chrome".into(),
        window_title: None,
        bundle_id: None,
        url: None,
        category_id: Some("entertainment".into()),
        started_at: start + chrono::Duration::hours(3),
        ended_at: Some(start + chrono::Duration::hours(4)),
        duration_secs: Some(3600),
        is_idle: false,
        metadata: None,
    }).await.unwrap();

    let summary = aggregator.compute_for_date(&today).await.unwrap();
    let score = summary.productivity_score.expect("score should be computed");
    assert!(score > 0.0 && score <= 100.0, "score {score} out of range");
    // 75% productive, no focus sessions, some distractions — expect moderate score
    assert!(score > 40.0 && score < 85.0, "score {score} unexpected for 75% productive");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(compute_productivity_score)' --nocapture`
Expected: FAIL (no `productivity_score` field on `DailySummary` yet)

**Step 3: Add productivity_score field to DailySummary**

In `types.rs`, add to `DailySummary` struct:

```rust
pub productivity_score: Option<f64>,
```

**Step 4: Add score computation to DailyAggregator::compute_for_date**

In `aggregator.rs`, add a function and call it in `compute_for_date` before the `self.repos.summaries.upsert` call:

```rust
/// Compute a 0-100 productivity score from daily metrics.
///
/// Formula:
/// - Productive ratio (40%): productive_secs / total_active_secs
/// - Focus quality (30%): avg_session_quality (or 0.5 if no sessions)
/// - Low distraction (20%): 1.0 - (distracting_secs / total_active_secs)
/// - Continuity (10%): 1.0 - (context_switches / expected_switches)
fn compute_productivity_score(summary: &DailySummary) -> f64 {
    let total = summary.total_active_secs as f64;
    if total < 60.0 {
        return 0.0; // Not enough data
    }

    let productive_ratio = summary.productive_secs as f64 / total;
    let focus_quality = summary.avg_session_quality.unwrap_or(0.5);
    let distraction_ratio = 1.0 - (summary.distracting_secs as f64 / total);
    let expected_switches = (total / 1800.0).max(1.0); // ~1 switch per 30min is normal
    let continuity = (1.0 - (summary.context_switches as f64 / expected_switches)).clamp(0.0, 1.0);

    let raw = (productive_ratio * 0.4)
        + (focus_quality * 0.3)
        + (distraction_ratio * 0.2)
        + (continuity * 0.1);

    (raw * 100.0).clamp(0.0, 100.0).round()
}
```

In `compute_for_date`, before the `self.repos.summaries.upsert` call, add:

```rust
let score = compute_productivity_score(&summary);
summary.productivity_score = Some(score);
```

(This requires making `summary` mutable: `let mut summary = DailySummary { ... };`)

**Step 5: Update DailySummaryRepo to handle the new column**

In `repos/daily_summary.rs`:
- Add `productivity_score: Option<f64>` to `SummaryRow`
- Add it to `SUMMARY_COLUMNS`
- Add it to the `From<SummaryRow>` impl
- Add it to the `upsert` INSERT and ON CONFLICT UPDATE

**Step 6: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(compute_productivity_score)' --nocapture`
Expected: PASS

**Step 7: Commit**

```bash
git add -A
git commit -m "feat(productivity): add productivity score computation"
```

---

## Task 4: Inject productivity score into agent context

**Files:**
- Modify: `crates/agent/src/context_sources/productivity.rs`

**Step 1: Add score to the Today section in build_context()**

In `ProductivityContextSource::build_context()` (~line 126), extend the today section:

```rust
if let Ok(summary) = self.aggregator.compute_today().await {
    let active_hours = summary.total_active_secs as f64 / 3600.0;
    let productive_hours = summary.productive_secs as f64 / 3600.0;
    let distracting_hours = summary.distracting_secs as f64 / 3600.0;

    let score_str = summary
        .productivity_score
        .map(|s| format!(" Score: {:.0}/100.", s))
        .unwrap_or_default();

    let mut today_line = format!(
        "## Today\n{:.1}h active ({:.1}h productive, {:.1}h distracting).{}",
        active_hours, productive_hours, distracting_hours, score_str
    );
    // ... rest unchanged
```

**Step 2: Commit**

```bash
git add crates/agent/src/context_sources/productivity.rs
git commit -m "feat(productivity): inject productivity score into agent context"
```

---

## Task 5: Add activity_score tool action

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Add the handler method**

```rust
async fn handle_activity_score(&self) -> Result<String> {
    let summary = self.aggregator.compute_today().await?;
    let score = summary.productivity_score.unwrap_or(0.0);

    let total = summary.total_active_secs as f64;
    let productive_pct = if total > 0.0 {
        (summary.productive_secs as f64 / total * 100.0).round()
    } else {
        0.0
    };
    let distracting_pct = if total > 0.0 {
        (summary.distracting_secs as f64 / total * 100.0).round()
    } else {
        0.0
    };

    Ok(format!(
        "Productivity score: {:.0}/100\n- Productive: {:.0}%\n- Distracting: {:.0}%\n- Focus sessions: {}\n- Context switches: {}",
        score, productive_pct, distracting_pct, summary.focus_sessions_count, summary.context_switches
    ))
}
```

**Step 2: Register in execute() match and parameters()**

Add `"activity_score"` to the `action` enum in `parameters()` and add the match arm:

```rust
"activity_score" => self.handle_activity_score().await,
```

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): add activity_score tool action"
```

---

## Task 6: Add ProductivityHandler trait for AI summaries

**Files:**
- Create: `crates/feature-productivity/src/handler.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Create the handler trait**

```rust
//! Handler trait for AI-powered productivity features.
//! Implemented in the agent crate to avoid circular dependencies.

use async_trait::async_trait;

#[async_trait]
pub trait ProductivityHandler: Send + Sync {
    /// Generate a natural language summary of the day's productivity data.
    async fn generate_daily_summary(&self, context: &str) -> common::Result<String>;
}
```

**Step 2: Add module to lib.rs**

```rust
pub mod handler;
pub use handler::ProductivityHandler;
```

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/handler.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add ProductivityHandler trait for AI summaries"
```

---

## Task 7: Implement AI summary generation in aggregator

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs`

**Step 1: Add handler to DailyAggregator**

```rust
use crate::handler::ProductivityHandler;
use std::sync::Arc;

pub struct DailyAggregator {
    repos: ProductivityRepos,
    handler: Option<Arc<dyn ProductivityHandler>>,
}

impl DailyAggregator {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self { repos, handler: None }
    }

    pub fn with_handler(mut self, handler: Arc<dyn ProductivityHandler>) -> Self {
        self.handler = Some(handler);
        self
    }
}
```

**Step 2: Add AI summary generation to compute_for_date**

After computing the score, before `upsert`:

```rust
// Generate AI summary if handler is available
if let Some(ref handler) = self.handler {
    let context = format!(
        "Date: {}. Active: {}h. Productive: {}h. Distracting: {}h. Focus sessions: {}. Context switches: {}. Score: {:.0}/100. Top apps: {}.",
        summary.date,
        summary.total_active_secs as f64 / 3600.0,
        summary.productive_secs as f64 / 3600.0,
        summary.distracting_secs as f64 / 3600.0,
        summary.focus_sessions_count,
        summary.context_switches,
        summary.productivity_score.unwrap_or(0.0),
        summary.top_apps.iter().take(3).map(|a| format!("{} ({}m)", a.app_name, a.duration_secs / 60)).collect::<Vec<_>>().join(", "),
    );
    match handler.generate_daily_summary(&context).await {
        Ok(ai_summary) => summary.ai_summary = Some(ai_summary),
        Err(e) => tracing::warn!("AI summary generation failed: {e}"),
    }
}
```

**Step 3: Fix all callers of DailyAggregator::new to compile**

No changes needed — `new()` still works without a handler. The handler is optional.

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/aggregator.rs
git commit -m "feat(productivity): add AI summary generation to daily aggregator"
```

---

## Task 8: Implement ProductivityHandler in agent crate

**Files:**
- Create: `crates/agent/src/handlers/productivity.rs`
- Modify: `crates/agent/src/handlers/mod.rs` (if exists, or wherever handlers live)

**Step 1: Find existing handler pattern**

Look at how `SpawnHandler`, `CronHandler`, etc. are implemented in the agent crate. Follow the same pattern.

**Step 2: Implement the handler**

```rust
use async_trait::async_trait;
use feature_productivity::ProductivityHandler;
use providers::ProviderRegistry;
use std::sync::Arc;

pub struct ProductivityHandlerImpl {
    provider_registry: Arc<ProviderRegistry>,
    model: String,
}

impl ProductivityHandlerImpl {
    pub fn new(provider_registry: Arc<ProviderRegistry>, model: String) -> Self {
        Self { provider_registry, model }
    }
}

#[async_trait]
impl ProductivityHandler for ProductivityHandlerImpl {
    async fn generate_daily_summary(&self, context: &str) -> common::Result<String> {
        let prompt = format!(
            "Generate a brief, friendly 2-3 sentence daily productivity summary based on this data. Be specific about numbers. Mention the top achievement and one improvement suggestion.\n\nData: {}",
            context
        );

        let provider = self.provider_registry.get_for_model(&self.model)?;
        let messages = vec![common::Message {
            role: common::MessageRole::User,
            content: prompt,
        }];

        let response = provider.chat(&messages, &self.model, None, None).await?;
        Ok(response.content)
    }
}
```

**Step 3: Wire into builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, where `DailyAggregator` is constructed (~line 571):

```rust
let mut aggregator = feature_productivity::DailyAggregator::new(prod_repos.clone());
if let Some(ref provider_registry) = self.provider_registry {
    let handler = Arc::new(ProductivityHandlerImpl::new(
        Arc::clone(provider_registry),
        config.agents.defaults.model.clone(),
    ));
    aggregator = aggregator.with_handler(handler);
}
let aggregator = Arc::new(aggregator);
```

**Step 4: Commit**

```bash
git add -A
git commit -m "feat(productivity): implement AI summary handler in agent crate"
```

---

## Task 9: Show AI summary in activity_today output

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Update format_summary function**

In `format_summary()` (~line 320), add after the top apps section:

```rust
if let Some(ref ai) = summary.ai_summary {
    lines.push(format!("\n{}", ai));
}
if let Some(score) = summary.productivity_score {
    lines.push(format!("\nProductivity score: {:.0}/100", score));
}
```

**Step 2: Commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): show AI summary and score in activity_today"
```

---

## Task 10: Implement Pomodoro timer

**Files:**
- Modify: `crates/feature-productivity/src/focus.rs`
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Write failing test for Pomodoro session**

In `focus.rs` tests:

```rust
#[tokio::test]
async fn test_start_pomodoro_session() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let mgr = FocusManager::new(repos, FocusConfig::default());

    let session = mgr.start_pomodoro(None, None, Some(25), Some(5)).await.unwrap();
    assert_eq!(session.session_type, SessionType::Pomodoro);
    assert_eq!(session.target_mins, Some(25));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(start_pomodoro)' --nocapture`
Expected: FAIL — `start_pomodoro` method doesn't exist

**Step 3: Implement start_pomodoro in FocusManager**

```rust
/// Start a Pomodoro session (work period).
pub async fn start_pomodoro(
    &self,
    action_id: Option<String>,
    project_id: Option<String>,
    work_mins: Option<i64>,
    break_mins: Option<i64>,
) -> common::Result<FocusSession> {
    if let Some(active) = self.repos.sessions.get_active().await? {
        return Err(common::ToolError::ExecutionFailed(format!(
            "Session already active (id: {}, type: {})",
            active.id, active.session_type
        )).into());
    }

    let work = work_mins.unwrap_or(25);
    let session = FocusSession {
        id: Uuid::new_v4().to_string(),
        action_id,
        project_id,
        session_type: SessionType::Pomodoro,
        target_mins: Some(work),
        started_at: Utc::now(),
        ended_at: None,
        actual_mins: None,
        interruptions: 0,
        distraction_events: vec![],
        quality_score: None,
        completed: false,
        notes: break_mins.map(|b| format!("break_mins:{b}")),
    };

    self.repos.sessions.create(&session).await?;
    Ok(session)
}
```

**Step 4: Add pomodoro_start and pomodoro_status tool actions**

In `tool/mod.rs`, add handler methods and register in `execute()`:

```rust
async fn handle_pomodoro_start(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let action_id = p.optional_str("action_id")?.map(|s| s.to_string());
    let project_id = p.optional_str("project_id")?.map(|s| s.to_string());
    let work_mins = p.optional_i64("work_mins")?;
    let break_mins = p.optional_i64("break_mins")?;

    let session = self.focus_manager.start_pomodoro(action_id, project_id, work_mins, break_mins).await?;
    let work = session.target_mins.unwrap_or(25);
    Ok(format!("Pomodoro started ({work}min work). Session ID: {}", session.id))
}
```

Add `"pomodoro_start"` to the action enum and match in `execute()`.

**Step 5: Run tests**

Run: `cargo nextest run -p feature-productivity --nocapture`
Expected: All PASS

**Step 6: Commit**

```bash
git add -A
git commit -m "feat(productivity): add Pomodoro timer support"
```

---

## Task 11: Write Pomodoro integration test

**Files:**
- Modify: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Write the test**

```rust
#[tokio::test]
async fn test_pomodoro_lifecycle() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let mgr = FocusManager::new(repos.clone(), FocusConfig::default());

    // Start pomodoro
    let session = mgr.start_pomodoro(None, None, Some(25), Some(5)).await.unwrap();
    assert_eq!(session.session_type, SessionType::Pomodoro);

    // Can't start another while active
    assert!(mgr.start_pomodoro(None, None, None, None).await.is_err());
    assert!(mgr.start_session(None, None, None).await.is_err());

    // End it
    let ended = mgr.end_session(None).await.unwrap().unwrap();
    assert_eq!(ended.session_type, SessionType::Pomodoro);
    assert!(ended.quality_score.is_some());
}
```

**Step 2: Run test**

Run: `cargo nextest run -p feature-productivity --test integration_test -E 'test(pomodoro_lifecycle)' --nocapture`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/feature-productivity/tests/integration_test.rs
git commit -m "test(productivity): add Pomodoro lifecycle integration test"
```

---

## Task 12: Skip — Pomodoro break auto-transition

Defer auto-transition between work/break cycles to a future phase. The current Pomodoro is manual start/stop which is sufficient for Phase 1.

---

## Task 13: Add historical comparison tool action

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Add handle_activity_compare method**

```rust
async fn handle_activity_compare(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let period = p.optional_str("period")?.unwrap_or("day");

    let today = Utc::now().date_naive();
    let (current_start, previous_start, previous_end, label) = match period {
        "day" => {
            let yesterday = today - Duration::days(1);
            (today.format("%Y-%m-%d").to_string(),
             yesterday.format("%Y-%m-%d").to_string(),
             yesterday.format("%Y-%m-%d").to_string(),
             "Today vs Yesterday")
        }
        "week" => {
            let week_ago = today - Duration::days(7);
            let two_weeks_ago = today - Duration::days(14);
            (week_ago.format("%Y-%m-%d").to_string(),
             two_weeks_ago.format("%Y-%m-%d").to_string(),
             (week_ago - Duration::days(1)).format("%Y-%m-%d").to_string(),
             "This week vs Last week")
        }
        _ => return Err(ToolError::InvalidParams("period must be 'day' or 'week'".into()).into()),
    };

    // Recompute today first
    let _ = self.aggregator.compute_today().await;

    let current = self.repos.summaries.list_range(&current_start, &today.format("%Y-%m-%d").to_string()).await?;
    let previous = self.repos.summaries.list_range(&previous_start, &previous_end).await?;

    let cur_productive: i64 = current.iter().map(|s| s.productive_secs).sum();
    let prev_productive: i64 = previous.iter().map(|s| s.productive_secs).sum();
    let cur_score: Option<f64> = {
        let scores: Vec<f64> = current.iter().filter_map(|s| s.productivity_score).collect();
        if scores.is_empty() { None } else { Some(scores.iter().sum::<f64>() / scores.len() as f64) }
    };
    let prev_score: Option<f64> = {
        let scores: Vec<f64> = previous.iter().filter_map(|s| s.productivity_score).collect();
        if scores.is_empty() { None } else { Some(scores.iter().sum::<f64>() / scores.len() as f64) }
    };

    let productive_change = if prev_productive > 0 {
        let pct = ((cur_productive as f64 - prev_productive as f64) / prev_productive as f64 * 100.0).round();
        if pct >= 0.0 { format!("+{:.0}%", pct) } else { format!("{:.0}%", pct) }
    } else {
        "N/A".into()
    };

    let score_line = match (cur_score, prev_score) {
        (Some(c), Some(p)) => {
            let diff = c - p;
            let sign = if diff >= 0.0 { "+" } else { "" };
            format!("\n- Score: {:.0} vs {:.0} ({}{:.0})", c, p, sign, diff)
        }
        _ => String::new(),
    };

    Ok(format!(
        "{label}:\n- Productive time: {} vs {} ({productive_change}){score_line}",
        format_duration(cur_productive),
        format_duration(prev_productive),
    ))
}
```

**Step 2: Register action**

Add `"activity_compare"` to the action enum and match arm.

**Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity --nocapture`
Expected: PASS (no regressions)

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): add historical comparison (day/week)"
```

---

## Task 14-15: Historical comparison tests

Write unit tests for `handle_activity_compare` by inserting summaries for yesterday and today, verifying the comparison output format. Follow the existing test patterns in `tests/integration_test.rs`. Commit separately.

---

## Task 16: Add Goal types

**Files:**
- Modify: `crates/feature-productivity/src/types.rs`

**Step 1: Add types**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityGoal {
    pub id: Option<i64>,
    pub goal_type: GoalType,
    pub metric: GoalMetric,
    pub target_value: f64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalMetric {
    ProductiveHours,
    FocusSessions,
    ProductivityScore,
    MaxDistractingMins,
}
```

Add `Display` and `FromStr` impls following the existing pattern (see `CategoryType`, `SessionType`).

**Step 2: Commit**

```bash
git add crates/feature-productivity/src/types.rs
git commit -m "feat(productivity): add Goal types"
```

---

## Task 17: Add GoalRepo

**Files:**
- Create: `crates/feature-productivity/src/repos/goal.rs`
- Modify: `crates/feature-productivity/src/repos/mod.rs`

**Step 1: Implement GoalRepo**

Follow the existing repo pattern (`NudgeRepo` is a good reference):

```rust
use sqlx::SqlitePool;
use crate::types::{GoalMetric, GoalType, ProductivityGoal};

#[derive(Debug, Clone)]
pub struct GoalRepo {
    pool: SqlitePool,
}

impl GoalRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, goal: &ProductivityGoal) -> common::Result<i64> {
        // INSERT INTO productivity_goals (goal_type, metric, target_value, enabled) ...
        // RETURNING id
    }

    pub async fn list_enabled(&self) -> common::Result<Vec<ProductivityGoal>> {
        // SELECT ... WHERE enabled = TRUE ORDER BY goal_type, metric
    }

    pub async fn delete(&self, id: i64) -> common::Result<bool> {
        // DELETE FROM productivity_goals WHERE id = ?1
    }

    pub async fn set_enabled(&self, id: i64, enabled: bool) -> common::Result<()> {
        // UPDATE productivity_goals SET enabled = ?2 WHERE id = ?1
    }
}
```

**Step 2: Add to ProductivityRepos**

```rust
pub goals: GoalRepo,
```

And in `ProductivityRepos::new`:

```rust
goals: GoalRepo::new(pool.clone()),
```

**Step 3: Write repo tests**

Add to `tests/repos_test.rs`:

```rust
#[tokio::test]
async fn test_goal_crud() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let goal = ProductivityGoal {
        id: None,
        goal_type: GoalType::Daily,
        metric: GoalMetric::ProductiveHours,
        target_value: 4.0,
        enabled: true,
        created_at: Utc::now(),
    };
    let id = repos.goals.insert(&goal).await.unwrap();
    assert!(id > 0);

    let goals = repos.goals.list_enabled().await.unwrap();
    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0].target_value, 4.0);

    repos.goals.delete(id).await.unwrap();
    let goals = repos.goals.list_enabled().await.unwrap();
    assert!(goals.is_empty());
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(goal_crud)' --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add -A
git commit -m "feat(productivity): add GoalRepo with CRUD operations"
```

---

## Task 18: Add goal checking logic

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs`

**Step 1: Add check_goals method**

```rust
pub async fn check_goals(&self, date: &str) -> common::Result<Vec<(ProductivityGoal, f64, bool)>> {
    let summary = self.get_or_compute(date).await?;
    let goals = self.repos.goals.list_enabled().await?;

    let mut results = Vec::new();
    for goal in goals {
        let current = match goal.metric {
            GoalMetric::ProductiveHours => summary.productive_secs as f64 / 3600.0,
            GoalMetric::FocusSessions => summary.focus_sessions_count as f64,
            GoalMetric::ProductivityScore => summary.productivity_score.unwrap_or(0.0),
            GoalMetric::MaxDistractingMins => summary.distracting_secs as f64 / 60.0,
        };
        let met = match goal.metric {
            GoalMetric::MaxDistractingMins => current <= goal.target_value,
            _ => current >= goal.target_value,
        };
        results.push((goal, current, met));
    }
    Ok(results)
}
```

**Step 2: Commit**

```bash
git add crates/feature-productivity/src/aggregator.rs
git commit -m "feat(productivity): add goal checking logic"
```

---

## Task 19: Add goal tool actions

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Add set_goal, check_goals, list_goals handlers**

```rust
async fn handle_set_goal(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let metric_str = p.required_str("metric")?;
    let metric: GoalMetric = metric_str.parse()?;
    let target = p.required_f64("target_value")?;
    let goal_type_str = p.optional_str("goal_type")?.unwrap_or("daily");
    let goal_type: GoalType = goal_type_str.parse()?;

    let goal = ProductivityGoal {
        id: None,
        goal_type,
        metric,
        target_value: target,
        enabled: true,
        created_at: Utc::now(),
    };
    let id = self.repos.goals.insert(&goal).await?;
    Ok(format!("Goal set: {} {} {} ({goal_type}). ID: {id}", metric, if matches!(metric, GoalMetric::MaxDistractingMins) { "<=" } else { ">=" }, target))
}

async fn handle_check_goals(&self) -> Result<String> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let results = self.aggregator.check_goals(&today).await?;

    if results.is_empty() {
        return Ok("No goals set. Use set_goal to create one.".into());
    }

    let mut lines = vec!["Goal progress:".to_string()];
    for (goal, current, met) in &results {
        let status = if *met { "MET" } else { "IN PROGRESS" };
        lines.push(format!("- {} {}: {:.1}/{:.1} [{}]", goal.goal_type, goal.metric, current, goal.target_value, status));
    }
    Ok(lines.join("\n"))
}

async fn handle_list_goals(&self) -> Result<String> {
    let goals = self.repos.goals.list_enabled().await?;
    if goals.is_empty() {
        return Ok("No goals set.".into());
    }
    let mut lines = vec!["Active goals:".to_string()];
    for g in &goals {
        lines.push(format!("- [{}] {} {} {} (id: {})", g.goal_type, g.metric,
            if matches!(g.metric, GoalMetric::MaxDistractingMins) { "<=" } else { ">=" },
            g.target_value, g.id.unwrap_or(0)));
    }
    Ok(lines.join("\n"))
}

async fn handle_remove_goal(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let id = p.required_i64("goal_id")?;
    if self.repos.goals.delete(id).await? {
        Ok(format!("Goal {id} removed."))
    } else {
        Ok(format!("Goal {id} not found."))
    }
}
```

**Step 2: Register all four actions**

Add `"set_goal"`, `"check_goals"`, `"list_goals"`, `"remove_goal"` to the action enum and match arms. Add relevant parameters to the JSON schema.

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): add goal management tool actions"
```

---

## Task 20: Goal integration test

**Files:**
- Modify: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Write test**

```rust
#[tokio::test]
async fn test_goal_tracking() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let aggregator = DailyAggregator::new(repos.clone());

    // Set a goal: 2h productive time
    let goal = ProductivityGoal {
        id: None,
        goal_type: GoalType::Daily,
        metric: GoalMetric::ProductiveHours,
        target_value: 2.0,
        enabled: true,
        created_at: Utc::now(),
    };
    repos.goals.insert(&goal).await.unwrap();

    // Insert 3h of productive activity
    let now = Utc::now();
    repos.events.insert(&ActivityEvent {
        id: None,
        app_name: "VS Code".into(),
        window_title: None,
        bundle_id: None,
        url: None,
        category_id: Some("coding".into()),
        started_at: now - chrono::Duration::hours(3),
        ended_at: Some(now),
        duration_secs: Some(10800),
        is_idle: false,
        metadata: None,
    }).await.unwrap();

    let today = now.format("%Y-%m-%d").to_string();
    let results = aggregator.check_goals(&today).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].2, "goal should be met with 3h > 2h target");
}
```

**Step 2: Run and commit**

```bash
cargo nextest run -p feature-productivity --test integration_test -E 'test(goal_tracking)' --nocapture
git add -A
git commit -m "test(productivity): add goal tracking integration test"
```

---

## Task 21: Add TimeEntry type and repo

**Files:**
- Modify: `crates/feature-productivity/src/types.rs`
- Create: `crates/feature-productivity/src/repos/time_entry.rs`
- Modify: `crates/feature-productivity/src/repos/mod.rs`

**Step 1: Add type**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: Option<i64>,
    pub description: String,
    pub category_id: Option<String>,
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub source: String,
    pub created_at: DateTime<Utc>,
}
```

**Step 2: Implement TimeEntryRepo**

Follow `NudgeRepo` pattern:

```rust
pub struct TimeEntryRepo { pool: SqlitePool }

impl TimeEntryRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn insert(&self, entry: &TimeEntry) -> common::Result<i64> { /* INSERT RETURNING id */ }
    pub async fn list_range(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> common::Result<Vec<TimeEntry>> { /* SELECT ... */ }
    pub async fn delete(&self, id: i64) -> common::Result<bool> { /* DELETE */ }
}
```

**Step 3: Add to ProductivityRepos**

```rust
pub time_entries: TimeEntryRepo,
```

**Step 4: Write repo test, run, commit**

```bash
git add -A
git commit -m "feat(productivity): add TimeEntry type and repo"
```

---

## Task 22: Add log_time tool action

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Add handler**

```rust
async fn handle_log_time(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let description = p.required_str("description")?;
    let duration_mins = p.required_i64("duration_mins")?;
    let category_id = p.optional_str("category_id")?.map(|s| s.to_string());
    let project_id = p.optional_str("project_id")?.map(|s| s.to_string());

    let entry = TimeEntry {
        id: None,
        description: description.to_string(),
        category_id,
        project_id,
        started_at: Utc::now() - Duration::minutes(duration_mins),
        duration_secs: duration_mins * 60,
        source: "manual".into(),
        created_at: Utc::now(),
    };

    let id = self.repos.time_entries.insert(&entry).await?;
    Ok(format!("Logged {}min: '{}' (id: {id})", duration_mins, description))
}
```

**Step 2: Register action, commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): add log_time action for manual time entry"
```

---

## Task 23: Include manual time entries in daily aggregation

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs`

**Step 1: Fetch time entries in compute_for_date**

Add `time_entries` to the `tokio::try_join!` call:

```rust
self.repos.time_entries.list_range(&start, &end),
```

**Step 2: Include in total_active_secs and category breakdown**

```rust
let manual_secs: i64 = time_entries.iter().map(|e| e.duration_secs).sum();
// Add manual_secs to total_active_secs in the summary
// Add manual entries to category_agg if they have category_id
```

**Step 3: Run all tests, commit**

```bash
cargo nextest run -p feature-productivity --nocapture
git add -A
git commit -m "feat(productivity): include manual time entries in daily aggregation"
```

---

## Task 24: Add data export action

**Files:**
- Modify: `crates/feature-productivity/src/tool/mod.rs`

**Step 1: Add handle_activity_export**

```rust
async fn handle_activity_export(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let start_date = p.required_str("start_date")?;
    let end_date = p.required_str("end_date")?;
    let format = p.optional_str("format")?.unwrap_or("csv");

    let start = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
        .map_err(|e| ToolError::InvalidParams(format!("invalid start_date: {e}")))?
        .and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .map_err(|e| ToolError::InvalidParams(format!("invalid end_date: {e}")))?
        .and_hms_opt(23, 59, 59).unwrap().and_utc();

    let events = self.repos.events.list_range(&start, &end, Some(50_000)).await?;

    match format {
        "csv" => {
            let mut csv = String::from("app_name,window_title,category_id,started_at,duration_secs,is_idle\n");
            for e in &events {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    e.app_name,
                    e.window_title.as_deref().unwrap_or(""),
                    e.category_id.as_deref().unwrap_or(""),
                    e.started_at,
                    e.duration_secs.unwrap_or(0),
                    e.is_idle,
                ));
            }
            Ok(format!("Exported {} events ({start_date} to {end_date}):\n\n{csv}", events.len()))
        }
        "json" => {
            let json = serde_json::to_string_pretty(&events)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(format!("Exported {} events ({start_date} to {end_date}):\n\n{json}", events.len()))
        }
        _ => Err(ToolError::InvalidParams("format must be 'csv' or 'json'".into()).into()),
    }
}
```

**Step 2: Register action, commit**

```bash
git add crates/feature-productivity/src/tool/mod.rs
git commit -m "feat(productivity): add data export (CSV/JSON)"
```

---

## Task 25: Export test

Write a test that inserts events and calls `handle_activity_export`, verifying CSV output contains the expected rows. Commit separately.

---

## Task 26: Add retention cleanup background job

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs`

**Step 1: Add purge_old_data method**

```rust
/// Purge activity events older than retention_days.
/// Returns the number of events purged.
pub async fn purge_old_data(&self, retention_days: u64) -> common::Result<u64> {
    let cutoff = Utc::now() - Duration::days(retention_days as i64);
    self.repos.events.purge_before(&cutoff).await
}
```

**Step 2: Wire into NudgeService or a dedicated background loop**

The simplest approach: add a daily purge check to `NudgeService::check_nudges()` (runs every 60s, but purge only runs once per day — use a static date check):

```rust
// In check_nudges, at the end:
// Purge is handled by the caller configuring a daily cron or
// by checking in the nudge loop with a daily guard.
```

Alternatively, expose `purge_old_data` as a tool action `"purge_old"` so users can trigger it manually or via cron.

**Step 3: Commit**

```bash
git add -A
git commit -m "feat(productivity): add retention cleanup"
```

---

## Task 27: Update skill file

**Files:**
- Modify: `skills/productivity-tracking/SKILL.md`

**Step 1: Add new actions to the skill triggers and examples**

Add documentation for all new tool actions: `activity_score`, `activity_compare`, `pomodoro_start`, `set_goal`, `check_goals`, `list_goals`, `remove_goal`, `log_time`, `activity_export`.

**Step 2: Commit**

```bash
git add skills/productivity-tracking/SKILL.md
git commit -m "docs(productivity): update skill file with new actions"
```

---

## Task 28: Final integration test suite

**Files:**
- Modify: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Run the full test suite**

```bash
cargo nextest run -p feature-productivity --nocapture
cargo clippy -p feature-productivity --all-targets --all-features
cargo fmt --all --check
```

**Step 2: Fix any issues**

**Step 3: Final commit**

```bash
git add -A
git commit -m "test(productivity): complete Phase 1-2 integration tests"
```

---

## Verification Checklist

Before declaring complete, verify:

- [ ] `cargo nextest run -p feature-productivity --nocapture` — all tests pass
- [ ] `cargo clippy -p feature-productivity --all-targets` — 0 warnings
- [ ] `cargo build --workspace` — clean build
- [ ] `cargo nextest run --workspace` — no regressions
- [ ] New migration applies cleanly on existing databases
- [ ] All new tool actions appear in `parameters()` JSON schema
- [ ] Productivity score appears in `ProductivityContextSource` output
- [ ] AI summary populates `ai_summary` field when handler is available
