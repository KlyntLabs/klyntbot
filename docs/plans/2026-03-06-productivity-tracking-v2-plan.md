# Productivity Tracking V2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the productivity tracking system from 62/100 to 100/100 with event bus architecture, automatic focus detection, 5-min bucket aggregation, distraction pattern analysis, heuristic insights, and real-time Tauri event push.

**Architecture:** `ActivityTracker` publishes `ActivityTick` events via `tokio::broadcast`. Independent subscribers (`BatchWriter`, `AutoFocusDetector`, `BucketAggregator`, `DistractionAnalyzer`, `DashboardEmitter`) consume the stream. A `ProductivityEngine` struct owns everything, replacing scattered `AppCore` wiring.

**Tech Stack:** Rust, tokio (broadcast channels), sqlx (SQLite), chrono, serde, Tauri events, React/TypeScript frontend

---

## Task 1: Database Migration `002_productivity_v2.sql`

**Files:**
- Create: `crates/feature-productivity/migrations/002_productivity_v2.sql`
- Modify: `crates/feature-productivity/src/lib.rs:44-55`

**Step 1: Write the migration SQL**

Create `crates/feature-productivity/migrations/002_productivity_v2.sql`:

```sql
-- Productivity V2: buckets, distraction patterns, insights, focus source

-- 5-minute activity buckets (365-day retention)
CREATE TABLE IF NOT EXISTS activity_buckets (
    bucket_start      TEXT NOT NULL,
    date              TEXT NOT NULL,
    dominant_app      TEXT,
    dominant_site     TEXT,
    dominant_category TEXT,
    productive_secs   INTEGER NOT NULL DEFAULT 0,
    neutral_secs      INTEGER NOT NULL DEFAULT 0,
    distracting_secs  INTEGER NOT NULL DEFAULT 0,
    idle_secs         INTEGER NOT NULL DEFAULT 0,
    context_switches  INTEGER NOT NULL DEFAULT 0,
    focus_depth       REAL,
    tick_count        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket_start)
);
CREATE INDEX IF NOT EXISTS idx_buckets_date ON activity_buckets(date);

-- Distraction pattern tracking
CREATE TABLE IF NOT EXISTS distraction_patterns (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    date                  TEXT NOT NULL,
    hour_of_day           INTEGER NOT NULL,
    hours_active_today    REAL NOT NULL,
    mins_since_break      REAL NOT NULL,
    preceding_app         TEXT,
    preceding_category    TEXT,
    preceding_duration_mins REAL,
    distraction_app       TEXT NOT NULL,
    distraction_category  TEXT,
    recovery_secs         INTEGER,
    created_at            TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_distraction_date ON distraction_patterns(date);

-- Heuristic insight cards
CREATE TABLE IF NOT EXISTS insight_cards (
    id              TEXT PRIMARY KEY,
    insight_type    TEXT NOT NULL,
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    sentiment       TEXT NOT NULL,
    metric_value    REAL,
    baseline_value  REAL,
    date            TEXT NOT NULL,
    dismissed       BOOLEAN NOT NULL DEFAULT FALSE,
    generated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_insights_date ON insight_cards(date);
CREATE UNIQUE INDEX IF NOT EXISTS idx_insights_type_date ON insight_cards(insight_type, date);

-- Add source column to focus_sessions
ALTER TABLE focus_sessions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

-- Add deep work fields to daily_summaries
ALTER TABLE daily_summaries ADD COLUMN deep_work_blocks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_summaries ADD COLUMN deep_work_secs INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_summaries ADD COLUMN avg_recovery_secs REAL;
```

**Step 2: Register migration in `lib.rs`**

Update `crates/feature-productivity/src/lib.rs` — change `migrations_static()` to include V2:

```rust
pub fn migration_v2_sql() -> &'static str {
    include_str!("../migrations/002_productivity_v2.sql")
}

pub fn migrations_static() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 1,
            description: "Create productivity tracking tables".to_string(),
            sql: Self::migration_sql().to_string(),
        },
        FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 2,
            description: "Add buckets, distraction patterns, insights, focus source".to_string(),
            sql: Self::migration_v2_sql().to_string(),
        },
    ]
}
```

**Step 3: Verify migration runs**

Run: `cargo nextest run -p feature-productivity -E 'test(setup_pool)' --nocapture`
Expected: All existing tests that call `setup_pool()` still pass (migration runs on in-memory pool).

**Step 4: Commit**

```bash
git add crates/feature-productivity/migrations/002_productivity_v2.sql crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add V2 migration — buckets, distraction patterns, insights"
```

---

## Task 2: New Types — `ActivityTick`, `SessionSource`, `InsightCard`

**Files:**
- Modify: `crates/feature-productivity/src/types.rs`

**Step 1: Add new types to `types.rs`**

Append to `crates/feature-productivity/src/types.rs`:

```rust
/// Real-time tick emitted by ActivityTracker every poll interval.
/// Consumed by all event bus subscribers.
#[derive(Debug, Clone)]
pub struct ActivityTick {
    pub timestamp: DateTime<Utc>,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub category_type: Option<CategoryType>,
    pub is_idle: bool,
    pub idle_secs: f64,
    pub is_context_switch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Manual,
    AutoDetected,
    Pomodoro,
}

impl std::fmt::Display for SessionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::AutoDetected => write!(f, "auto_detected"),
            Self::Pomodoro => write!(f, "pomodoro"),
        }
    }
}

impl std::str::FromStr for SessionSource {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(Self::Manual),
            "auto_detected" => Ok(Self::AutoDetected),
            "pomodoro" => Ok(Self::Pomodoro),
            _ => Err(common::ToolError::InvalidParams(format!("unknown session source: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sentiment {
    Positive,
    Neutral,
    Warning,
    Negative,
}

impl std::fmt::Display for Sentiment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Positive => write!(f, "positive"),
            Self::Neutral => write!(f, "neutral"),
            Self::Warning => write!(f, "warning"),
            Self::Negative => write!(f, "negative"),
        }
    }
}

impl std::str::FromStr for Sentiment {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "positive" => Ok(Self::Positive),
            "neutral" => Ok(Self::Neutral),
            "warning" => Ok(Self::Warning),
            "negative" => Ok(Self::Negative),
            _ => Err(common::ToolError::InvalidParams(format!("unknown sentiment: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightType {
    DeepWorkTrend,
    DistractionSpike,
    PeakHourShift,
    StreakAchieved,
    FatigueWarning,
    RecoveryImprovement,
    CategoryShift,
    NewPersonalBest,
    ConsistencyNote,
}

impl std::fmt::Display for InsightType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeepWorkTrend => write!(f, "deep_work_trend"),
            Self::DistractionSpike => write!(f, "distraction_spike"),
            Self::PeakHourShift => write!(f, "peak_hour_shift"),
            Self::StreakAchieved => write!(f, "streak_achieved"),
            Self::FatigueWarning => write!(f, "fatigue_warning"),
            Self::RecoveryImprovement => write!(f, "recovery_improvement"),
            Self::CategoryShift => write!(f, "category_shift"),
            Self::NewPersonalBest => write!(f, "new_personal_best"),
            Self::ConsistencyNote => write!(f, "consistency_note"),
        }
    }
}

impl std::str::FromStr for InsightType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "deep_work_trend" => Ok(Self::DeepWorkTrend),
            "distraction_spike" => Ok(Self::DistractionSpike),
            "peak_hour_shift" => Ok(Self::PeakHourShift),
            "streak_achieved" => Ok(Self::StreakAchieved),
            "fatigue_warning" => Ok(Self::FatigueWarning),
            "recovery_improvement" => Ok(Self::RecoveryImprovement),
            "category_shift" => Ok(Self::CategoryShift),
            "new_personal_best" => Ok(Self::NewPersonalBest),
            "consistency_note" => Ok(Self::ConsistencyNote),
            _ => Err(common::ToolError::InvalidParams(format!("unknown insight type: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightCard {
    pub id: String,
    pub insight_type: InsightType,
    pub title: String,
    pub body: String,
    pub sentiment: Sentiment,
    pub metric_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub date: String,
    pub dismissed: bool,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBucket {
    pub bucket_start: String,
    pub date: String,
    pub dominant_app: Option<String>,
    pub dominant_site: Option<String>,
    pub dominant_category: Option<String>,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub idle_secs: i64,
    pub context_switches: i64,
    pub focus_depth: Option<f64>,
    pub tick_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistractionPattern {
    pub id: Option<i64>,
    pub date: String,
    pub hour_of_day: i32,
    pub hours_active_today: f64,
    pub mins_since_break: f64,
    pub preceding_app: Option<String>,
    pub preceding_category: Option<String>,
    pub preceding_duration_mins: Option<f64>,
    pub distraction_app: String,
    pub distraction_category: Option<String>,
    pub recovery_secs: Option<i64>,
    pub created_at: DateTime<Utc>,
}
```

Also add `source` field to `FocusSession` (line 108-121):

```rust
pub struct FocusSession {
    pub id: String,
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub session_type: SessionType,
    pub target_mins: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub actual_mins: Option<i64>,
    pub interruptions: i64,
    pub distraction_events: Vec<DistractionEvent>,
    pub quality_score: Option<f64>,
    pub completed: bool,
    pub notes: Option<String>,
    pub source: SessionSource,
}
```

Add `deep_work_blocks`, `deep_work_secs`, `avg_recovery_secs` to `DailySummary` (line 131-150):

```rust
pub struct DailySummary {
    // ... existing fields ...
    pub productivity_score: Option<f64>,
    pub ai_summary: Option<String>,
    pub deep_work_blocks: i64,
    pub deep_work_secs: i64,
    pub avg_recovery_secs: Option<f64>,
}
```

**Step 2: Update FocusSession and DailySummary repos for new fields**

Update `crates/feature-productivity/src/repos/focus_session.rs`:
- Add `source: String` to `SessionRow`
- Add `source` to `SESSION_COLUMNS` const
- Parse `source` in `From<SessionRow> for FocusSession`
- Bind `source` in `create()` and `update()`

Update `crates/feature-productivity/src/repos/daily_summary.rs`:
- Add `deep_work_blocks: i64`, `deep_work_secs: i64`, `avg_recovery_secs: Option<f64>` to `SummaryRow`
- Add fields to `SUMMARY_COLUMNS`
- Add to `From<SummaryRow> for DailySummary`
- Bind in `upsert()`

**Step 3: Fix all compilation errors**

Every place that constructs `FocusSession` needs `source: SessionSource::Manual` (or appropriate variant). Every place that constructs `DailySummary` needs the three new fields.

Key files:
- `crates/feature-productivity/src/focus.rs` — `start_session()` sets `source: SessionSource::Manual`
- `crates/feature-productivity/src/aggregator.rs` — `compute_for_date()` adds `deep_work_blocks: 0, deep_work_secs: 0, avg_recovery_secs: None`

**Step 4: Run tests**

Run: `cargo nextest run -p feature-productivity`
Expected: PASS (all existing tests compile and pass with new fields)

**Step 5: Commit**

```bash
git add crates/feature-productivity/src/types.rs crates/feature-productivity/src/repos/
git commit -m "feat(productivity): add ActivityTick, SessionSource, InsightCard, ActivityBucket, DistractionPattern types"
```

---

## Task 3: New Repos — `BucketRepo`, `DistractionPatternRepo`, `InsightRepo`

**Files:**
- Create: `crates/feature-productivity/src/repos/bucket.rs`
- Create: `crates/feature-productivity/src/repos/distraction_pattern.rs`
- Create: `crates/feature-productivity/src/repos/insight.rs`
- Modify: `crates/feature-productivity/src/repos/mod.rs`

**Step 1: Write `BucketRepo`**

Create `crates/feature-productivity/src/repos/bucket.rs`:

```rust
use sqlx::SqlitePool;

use crate::types::ActivityBucket;

#[derive(Debug, Clone)]
pub struct BucketRepo {
    pool: SqlitePool,
}

impl BucketRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, bucket: &ActivityBucket) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO activity_buckets (bucket_start, date, dominant_app, dominant_site, dominant_category, productive_secs, neutral_secs, distracting_secs, idle_secs, context_switches, focus_depth, tick_count)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
               ON CONFLICT(bucket_start) DO UPDATE SET
                   dominant_app = excluded.dominant_app,
                   dominant_site = excluded.dominant_site,
                   dominant_category = excluded.dominant_category,
                   productive_secs = excluded.productive_secs,
                   neutral_secs = excluded.neutral_secs,
                   distracting_secs = excluded.distracting_secs,
                   idle_secs = excluded.idle_secs,
                   context_switches = excluded.context_switches,
                   focus_depth = excluded.focus_depth,
                   tick_count = excluded.tick_count"#,
        )
        .bind(&bucket.bucket_start)
        .bind(&bucket.date)
        .bind(&bucket.dominant_app)
        .bind(&bucket.dominant_site)
        .bind(&bucket.dominant_category)
        .bind(bucket.productive_secs)
        .bind(bucket.neutral_secs)
        .bind(bucket.distracting_secs)
        .bind(bucket.idle_secs)
        .bind(bucket.context_switches)
        .bind(bucket.focus_depth)
        .bind(bucket.tick_count)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(&self, start_date: &str, end_date: &str) -> common::Result<Vec<ActivityBucket>> {
        let rows = sqlx::query_as::<_, ActivityBucket>(
            r#"SELECT bucket_start, date, dominant_app, dominant_site, dominant_category,
                      productive_secs, neutral_secs, distracting_secs, idle_secs,
                      context_switches, focus_depth, tick_count
               FROM activity_buckets
               WHERE date >= ?1 AND date <= ?2
               ORDER BY bucket_start ASC"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn purge_before(&self, before_date: &str) -> common::Result<u64> {
        let result = sqlx::query("DELETE FROM activity_buckets WHERE date < ?1")
            .bind(before_date)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    /// Aggregate buckets into daily totals for a date range (for historical queries).
    pub async fn aggregate_day(&self, date: &str) -> common::Result<Option<(i64, i64, i64, i64, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            productive: i64,
            neutral: i64,
            distracting: i64,
            idle: i64,
            switches: i64,
        }
        let row = sqlx::query_as::<_, Row>(
            r#"SELECT COALESCE(SUM(productive_secs), 0) as productive,
                      COALESCE(SUM(neutral_secs), 0) as neutral,
                      COALESCE(SUM(distracting_secs), 0) as distracting,
                      COALESCE(SUM(idle_secs), 0) as idle,
                      COALESCE(SUM(context_switches), 0) as switches
               FROM activity_buckets WHERE date = ?1"#,
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(|r| (r.productive, r.neutral, r.distracting, r.idle, r.switches)))
    }
}
```

Note: Add `#[derive(sqlx::FromRow)]` to `ActivityBucket` in `types.rs`.

**Step 2: Write `DistractionPatternRepo`**

Create `crates/feature-productivity/src/repos/distraction_pattern.rs`:

```rust
use sqlx::SqlitePool;

use crate::types::DistractionPattern;

#[derive(Debug, Clone)]
pub struct DistractionPatternRepo {
    pool: SqlitePool,
}

impl DistractionPatternRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, pattern: &DistractionPattern) -> common::Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO distraction_patterns (date, hour_of_day, hours_active_today, mins_since_break, preceding_app, preceding_category, preceding_duration_mins, distraction_app, distraction_category, recovery_secs)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               RETURNING id"#,
        )
        .bind(&pattern.date)
        .bind(pattern.hour_of_day)
        .bind(pattern.hours_active_today)
        .bind(pattern.mins_since_break)
        .bind(&pattern.preceding_app)
        .bind(&pattern.preceding_category)
        .bind(pattern.preceding_duration_mins)
        .bind(&pattern.distraction_app)
        .bind(&pattern.distraction_category)
        .bind(pattern.recovery_secs)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn update_recovery(&self, id: i64, recovery_secs: i64) -> common::Result<()> {
        sqlx::query("UPDATE distraction_patterns SET recovery_secs = ?1 WHERE id = ?2")
            .bind(recovery_secs)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(&self, start_date: &str, end_date: &str) -> common::Result<Vec<DistractionPattern>> {
        let rows = sqlx::query_as::<_, DistractionPattern>(
            r#"SELECT id, date, hour_of_day, hours_active_today, mins_since_break,
                      preceding_app, preceding_category, preceding_duration_mins,
                      distraction_app, distraction_category, recovery_secs, created_at
               FROM distraction_patterns
               WHERE date >= ?1 AND date <= ?2
               ORDER BY created_at ASC"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn avg_recovery_secs(&self, start_date: &str, end_date: &str) -> common::Result<Option<f64>> {
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(CAST(recovery_secs AS REAL)) FROM distraction_patterns WHERE date >= ?1 AND date <= ?2 AND recovery_secs IS NOT NULL",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(avg)
    }

    /// Count distractions per hour for a date range (for pattern analysis).
    pub async fn count_by_hour(&self, start_date: &str, end_date: &str) -> common::Result<Vec<(i32, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row { hour_of_day: i32, cnt: i64 }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT hour_of_day, COUNT(*) as cnt FROM distraction_patterns WHERE date >= ?1 AND date <= ?2 GROUP BY hour_of_day ORDER BY hour_of_day",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|r| (r.hour_of_day, r.cnt)).collect())
    }
}
```

Note: Add `#[derive(sqlx::FromRow)]` to `DistractionPattern` in `types.rs`.

**Step 3: Write `InsightRepo`**

Create `crates/feature-productivity/src/repos/insight.rs`:

```rust
use sqlx::SqlitePool;

use crate::types::{InsightCard, InsightType, Sentiment};

#[derive(sqlx::FromRow)]
struct InsightRow {
    id: String,
    insight_type: String,
    title: String,
    body: String,
    sentiment: String,
    metric_value: Option<f64>,
    baseline_value: Option<f64>,
    date: String,
    dismissed: bool,
    generated_at: String,
}

impl From<InsightRow> for InsightCard {
    fn from(row: InsightRow) -> Self {
        Self {
            id: row.id,
            insight_type: row.insight_type.parse().unwrap_or(InsightType::ConsistencyNote),
            title: row.title,
            body: row.body,
            sentiment: row.sentiment.parse().unwrap_or(Sentiment::Neutral),
            metric_value: row.metric_value,
            baseline_value: row.baseline_value,
            date: row.date,
            dismissed: row.dismissed,
            generated_at: common::utils::date::parse_datetime(&row.generated_at, "UTC")
                .unwrap_or_else(chrono::Utc::now),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InsightRepo {
    pool: SqlitePool,
}

impl InsightRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, card: &InsightCard) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO insight_cards (id, insight_type, title, body, sentiment, metric_value, baseline_value, date, dismissed, generated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(id) DO UPDATE SET
                   title = excluded.title,
                   body = excluded.body,
                   sentiment = excluded.sentiment,
                   metric_value = excluded.metric_value,
                   baseline_value = excluded.baseline_value"#,
        )
        .bind(&card.id)
        .bind(card.insight_type.to_string())
        .bind(&card.title)
        .bind(&card.body)
        .bind(card.sentiment.to_string())
        .bind(card.metric_value)
        .bind(card.baseline_value)
        .bind(&card.date)
        .bind(card.dismissed)
        .bind(card.generated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn exists_for_date(&self, insight_type: InsightType, date: &str) -> common::Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM insight_cards WHERE insight_type = ?1 AND date = ?2",
        )
        .bind(insight_type.to_string())
        .bind(date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(count > 0)
    }

    pub async fn list_for_date(&self, date: &str) -> common::Result<Vec<InsightCard>> {
        let rows = sqlx::query_as::<_, InsightRow>(
            "SELECT id, insight_type, title, body, sentiment, metric_value, baseline_value, date, dismissed, generated_at FROM insight_cards WHERE date = ?1 ORDER BY generated_at DESC",
        )
        .bind(date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(InsightCard::from).collect())
    }

    pub async fn dismiss(&self, id: &str) -> common::Result<()> {
        sqlx::query("UPDATE insight_cards SET dismissed = TRUE WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_undismissed(&self, limit: i64) -> common::Result<Vec<InsightCard>> {
        let rows = sqlx::query_as::<_, InsightRow>(
            "SELECT id, insight_type, title, body, sentiment, metric_value, baseline_value, date, dismissed, generated_at FROM insight_cards WHERE dismissed = FALSE ORDER BY generated_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(InsightCard::from).collect())
    }
}
```

**Step 4: Register repos in `ProductivityRepos`**

Update `crates/feature-productivity/src/repos/mod.rs`:

```rust
pub mod bucket;
pub mod distraction_pattern;
pub mod insight;
// ... existing mods ...

pub use bucket::BucketRepo;
pub use distraction_pattern::DistractionPatternRepo;
pub use insight::InsightRepo;
// ... existing uses ...

pub struct ProductivityRepos {
    pub events: ActivityEventRepo,
    pub categories: ActivityCategoryRepo,
    pub sessions: FocusSessionRepo,
    pub summaries: DailySummaryRepo,
    pub nudges: NudgeRepo,
    pub goals: GoalRepo,
    pub time_entries: TimeEntryRepo,
    pub learned_rules: LearnedRuleRepo,
    pub buckets: BucketRepo,
    pub distraction_patterns: DistractionPatternRepo,
    pub insights: InsightRepo,
}

impl ProductivityRepos {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            events: ActivityEventRepo::new(pool.clone()),
            categories: ActivityCategoryRepo::new(pool.clone()),
            sessions: FocusSessionRepo::new(pool.clone()),
            summaries: DailySummaryRepo::new(pool.clone()),
            nudges: NudgeRepo::new(pool.clone()),
            goals: GoalRepo::new(pool.clone()),
            time_entries: TimeEntryRepo::new(pool.clone()),
            learned_rules: LearnedRuleRepo::new(pool.clone()),
            buckets: BucketRepo::new(pool.clone()),
            distraction_patterns: DistractionPatternRepo::new(pool.clone()),
            insights: InsightRepo::new(pool),
        }
    }
}
```

**Step 5: Write tests for new repos**

Add tests in each repo file (inline `#[cfg(test)] mod tests`). Test insert, query, upsert idempotency. Use `setup_pool()` pattern from existing tests.

**Step 6: Run tests**

Run: `cargo nextest run -p feature-productivity`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/feature-productivity/src/repos/
git commit -m "feat(productivity): add BucketRepo, DistractionPatternRepo, InsightRepo"
```

---

## Task 4: Config Changes — Auto-Focus and Retention

**Files:**
- Modify: `crates/config/src/schema/productivity.rs`

**Step 1: Update `TrackingConfig`**

Replace `retention_days` with `raw_retention_days` (7) and `bucket_retention_days` (365):

```rust
pub struct TrackingConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_secs: u64,
    #[serde(default = "default_batch_interval")]
    pub batch_write_interval_secs: u64,
    #[serde(default = "default_raw_retention")]
    pub raw_retention_days: u64,
    #[serde(default = "default_bucket_retention")]
    pub bucket_retention_days: u64,
}

fn default_raw_retention() -> u64 { 7 }
fn default_bucket_retention() -> u64 { 365 }
```

Remove `default_retention()` and `retention_days`.

**Step 2: Add auto-focus fields to `FocusConfig`**

```rust
pub struct FocusConfig {
    // ... existing fields ...
    #[serde(default = "default_true")]
    pub auto_detect_enabled: bool,
    #[serde(default = "default_auto_detect_min_mins")]
    pub auto_detect_min_mins: u64,
    #[serde(default = "default_auto_detect_productive_threshold")]
    pub auto_detect_productive_threshold: f64,
    #[serde(default = "default_auto_detect_max_switches")]
    pub auto_detect_max_switches: u64,
    #[serde(default = "default_cooldown_grace_secs")]
    pub cooldown_grace_secs: u64,
}

fn default_auto_detect_min_mins() -> u64 { 15 }
fn default_auto_detect_productive_threshold() -> f64 { 0.8 }
fn default_auto_detect_max_switches() -> u64 { 2 }
fn default_cooldown_grace_secs() -> u64 { 120 }
```

Update `Default` impls and fix all references to `retention_days` in the codebase (grep for it — appears in `aggregator.rs` `purge_old_data()`).

**Step 3: Fix compilation**

Grep for `retention_days` and `config.tracking.retention_days` — update to `raw_retention_days`. The `purge_old_data` in `aggregator.rs` and any caller in `desktop` crate.

**Step 4: Run tests**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/config/src/schema/productivity.rs crates/feature-productivity/
git commit -m "feat(productivity): config breaking changes — dual retention, auto-focus settings"
```

---

## Task 5: Refactor `ActivityTracker` to Broadcast-Only

**Files:**
- Modify: `crates/feature-productivity/src/tracker/mod.rs`

**Step 1: Add `tokio::sync::broadcast` to ActivityTracker**

The tracker should:
1. Poll macOS APIs (unchanged)
2. Categorize (unchanged)
3. Build an `ActivityTick` from the poll result
4. Send it via `broadcast::Sender<ActivityTick>`
5. **Remove** all batch writing, distraction detection, and buffer management from the tracker loop

The tracker becomes a thin poller. All consumers move to independent subscribers.

```rust
use tokio::sync::broadcast;
use crate::types::ActivityTick;

pub struct ActivityTracker {
    config: ProductivityConfig,
    categorizer: Arc<RwLock<Categorizer>>,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
    tick_sender: broadcast::Sender<ActivityTick>,
}

impl ActivityTracker {
    pub fn new(
        config: ProductivityConfig,
        categorizer: Categorizer,
        tick_sender: broadcast::Sender<ActivityTick>,
    ) -> Self {
        Self {
            config,
            categorizer: Arc::new(RwLock::new(categorizer)),
            cancel_token: CancellationToken::new(),
            task_handle: None,
            tick_sender,
        }
    }

    pub fn categorizer(&self) -> &Arc<RwLock<Categorizer>> {
        &self.categorizer
    }

    pub fn start(&mut self) { /* simplified loop: poll -> categorize -> broadcast */ }
    pub async fn stop(&mut self) { /* unchanged */ }
}
```

The `start()` loop should:
- Poll `macos::get_frontmost_window()` every `poll_interval_secs`
- Get idle seconds via `macos::seconds_since_last_input()`
- Categorize via `categorizer.read().await.categorize_full()`
- Compute `site_name` via existing `compute_site_name()`
- Track previous app/site for `is_context_switch` detection
- Build `ActivityTick` and send via `self.tick_sender.send(tick)`
- On cancel: just break (no buffer flushing needed)

Remove: `repos`, `focus_manager`, `distraction_sender`, `DistractionAlert`, buffer, batch writing, distraction detection logic. These all move to subscribers.

**Step 2: Run tests**

Run: `cargo build -p feature-productivity`
Expected: Compilation errors for removed fields — fix callers.

Run: `cargo nextest run -p feature-productivity`
Expected: Tests that used `ActivityTracker` directly may need updates. The tracker no longer writes to DB.

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/tracker/
git commit -m "refactor(productivity): simplify ActivityTracker to broadcast-only poller"
```

---

## Task 6: `BatchWriter` Subscriber

**Files:**
- Create: `crates/feature-productivity/src/batch_writer.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write `BatchWriter`**

Create `crates/feature-productivity/src/batch_writer.rs`:

```rust
//! BatchWriter — subscribes to ActivityTick broadcast, buffers events,
//! and batch-writes to the activity_events table.

use std::sync::Arc;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::config::PrivacyConfig;
use crate::repos::ProductivityRepos;
use crate::types::{ActivityEvent, ActivityTick};

const MAX_BUFFER_SIZE: usize = 1000;

pub struct BatchWriter {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BatchWriter {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        privacy: PrivacyConfig,
        batch_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let batch_interval = std::time::Duration::from_secs(batch_interval_secs);
            let mut buffer: Vec<ActivityEvent> = Vec::new();
            let mut current_event: Option<ActivityEvent> = None;
            let mut last_flush = tokio::time::Instant::now();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        if let Some(evt) = current_event.take() {
                            buffer.push(evt);
                        }
                        if !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("BatchWriter: failed to flush on shutdown: {e}");
                            }
                        }
                        break;
                    }
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("BatchWriter lagged, skipped {n} ticks");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        let persisted_title = if privacy.exclude_window_titles {
                            None
                        } else {
                            tick.window_title.clone()
                        };
                        let persisted_site = if privacy.exclude_window_titles {
                            None
                        } else {
                            tick.site_name.clone()
                        };

                        let same_context = !tick.is_idle
                            && !tick.is_context_switch
                            && current_event.as_ref().is_some_and(|e| {
                                e.app_name == tick.app_name && e.site_name == persisted_site
                            });

                        if same_context {
                            if let Some(ref mut evt) = current_event {
                                evt.ended_at = Some(tick.timestamp);
                                evt.duration_secs = Some(
                                    (tick.timestamp - evt.started_at).num_seconds()
                                );
                                evt.window_title = persisted_title;
                            }
                        } else {
                            if let Some(evt) = current_event.take() {
                                buffer.push(evt);
                            }
                            current_event = Some(ActivityEvent {
                                id: None,
                                app_name: tick.app_name.clone(),
                                window_title: persisted_title,
                                site_name: persisted_site,
                                bundle_id: tick.bundle_id.clone(),
                                url: None,
                                category_id: tick.category_id.clone(),
                                started_at: tick.timestamp,
                                ended_at: Some(tick.timestamp),
                                duration_secs: Some(0),
                                is_idle: tick.is_idle,
                                metadata: None,
                            });
                        }

                        // Batch write check
                        if last_flush.elapsed() >= batch_interval && !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("BatchWriter: failed to batch write: {e}");
                            } else {
                                debug!("BatchWriter: flushed {} events", buffer.len());
                                buffer.clear();
                            }
                            last_flush = tokio::time::Instant::now();
                        }

                        if buffer.len() > MAX_BUFFER_SIZE {
                            let overflow = buffer.len() - MAX_BUFFER_SIZE;
                            warn!("BatchWriter: buffer exceeded {MAX_BUFFER_SIZE}, dropping {overflow} oldest");
                            buffer.drain(..overflow);
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("BatchWriter task panicked: {e}");
            }
        }
    }
}
```

**Step 2: Add module to `lib.rs`**

Add `pub mod batch_writer;` to `crates/feature-productivity/src/lib.rs`.

**Step 3: Run tests**

Run: `cargo build -p feature-productivity`
Expected: PASS (compiles)

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/batch_writer.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): extract BatchWriter as broadcast subscriber"
```

---

## Task 7: `BucketAggregator` Subscriber

**Files:**
- Create: `crates/feature-productivity/src/bucket_aggregator.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write `BucketAggregator`**

Create `crates/feature-productivity/src/bucket_aggregator.rs`:

```rust
//! BucketAggregator — accumulates ActivityTicks into 5-minute windows,
//! then persists each completed bucket to activity_buckets.

use std::collections::HashMap;

use chrono::{Duration, Utc};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::repos::ProductivityRepos;
use crate::types::{ActivityBucket, ActivityTick, CategoryType};

const BUCKET_DURATION_SECS: i64 = 300; // 5 minutes

/// Align a timestamp down to the nearest 5-minute boundary.
fn bucket_start_for(ts: &chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    let secs = ts.timestamp();
    let aligned = secs - (secs % BUCKET_DURATION_SECS);
    chrono::DateTime::from_timestamp(aligned, 0).unwrap_or(*ts)
}

struct PendingBucket {
    bucket_start: chrono::DateTime<Utc>,
    app_counts: HashMap<String, i64>,
    site_counts: HashMap<String, i64>,
    category_counts: HashMap<String, i64>,
    productive_secs: i64,
    neutral_secs: i64,
    distracting_secs: i64,
    idle_secs: i64,
    context_switches: i64,
    tick_count: i64,
    tick_interval_secs: i64,
}

impl PendingBucket {
    fn new(bucket_start: chrono::DateTime<Utc>, tick_interval_secs: i64) -> Self {
        Self {
            bucket_start,
            app_counts: HashMap::new(),
            site_counts: HashMap::new(),
            category_counts: HashMap::new(),
            productive_secs: 0,
            neutral_secs: 0,
            distracting_secs: 0,
            idle_secs: 0,
            context_switches: 0,
            tick_count: 0,
            tick_interval_secs,
        }
    }

    fn add_tick(&mut self, tick: &ActivityTick) {
        self.tick_count += 1;
        let secs = self.tick_interval_secs;

        if tick.is_idle {
            self.idle_secs += secs;
            return;
        }

        *self.app_counts.entry(tick.app_name.clone()).or_default() += secs;
        if let Some(ref site) = tick.site_name {
            *self.site_counts.entry(site.clone()).or_default() += secs;
        }

        match tick.category_type {
            Some(CategoryType::Productive) => self.productive_secs += secs,
            Some(CategoryType::Distracting) => self.distracting_secs += secs,
            Some(CategoryType::Neutral) | None => self.neutral_secs += secs,
        }

        if let Some(ref cat) = tick.category_id {
            *self.category_counts.entry(cat.clone()).or_default() += secs;
        }

        if tick.is_context_switch {
            self.context_switches += 1;
        }
    }

    fn into_bucket(self) -> ActivityBucket {
        let dominant_app = self.app_counts.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k);
        let dominant_site = self.site_counts.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k);
        let dominant_category = self.category_counts.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k);

        let total_active = self.productive_secs + self.neutral_secs + self.distracting_secs;
        let focus_depth = if total_active > 0 {
            let max_app_secs = dominant_app.as_ref()
                .map(|_| self.productive_secs) // approximate
                .unwrap_or(0);
            Some((max_app_secs as f64 / total_active as f64).clamp(0.0, 1.0))
        } else {
            None
        };

        ActivityBucket {
            bucket_start: self.bucket_start.to_rfc3339(),
            date: self.bucket_start.format("%Y-%m-%d").to_string(),
            dominant_app,
            dominant_site,
            dominant_category,
            productive_secs: self.productive_secs,
            neutral_secs: self.neutral_secs,
            distracting_secs: self.distracting_secs,
            idle_secs: self.idle_secs,
            context_switches: self.context_switches,
            focus_depth,
            tick_count: self.tick_count,
        }
    }
}

pub struct BucketAggregator {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BucketAggregator {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        poll_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let mut current_bucket: Option<PendingBucket> = None;

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        // Flush current bucket
                        if let Some(bucket) = current_bucket.take() {
                            if bucket.tick_count > 0 {
                                let ab = bucket.into_bucket();
                                if let Err(e) = repos.buckets.upsert(&ab).await {
                                    warn!("BucketAggregator: failed to flush on shutdown: {e}");
                                }
                            }
                        }
                        break;
                    }
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("BucketAggregator lagged, skipped {n} ticks");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        let tick_bucket_start = bucket_start_for(&tick.timestamp);

                        // Check if we've moved to a new bucket
                        let need_new = current_bucket.as_ref()
                            .map(|b| b.bucket_start != tick_bucket_start)
                            .unwrap_or(true);

                        if need_new {
                            // Flush previous bucket
                            if let Some(old_bucket) = current_bucket.take() {
                                if old_bucket.tick_count > 0 {
                                    let ab = old_bucket.into_bucket();
                                    debug!("BucketAggregator: flushing bucket {}", ab.bucket_start);
                                    if let Err(e) = repos.buckets.upsert(&ab).await {
                                        warn!("BucketAggregator: failed to upsert bucket: {e}");
                                    }
                                }
                            }
                            current_bucket = Some(PendingBucket::new(tick_bucket_start, poll_interval_secs as i64));
                        }

                        if let Some(ref mut bucket) = current_bucket {
                            bucket.add_tick(&tick);
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("BucketAggregator task panicked: {e}");
            }
        }
    }
}
```

**Step 2: Add module to `lib.rs`**

Add `pub mod bucket_aggregator;` to `crates/feature-productivity/src/lib.rs`.

**Step 3: Write a test**

Add an integration-style test that creates a broadcast channel, sends ticks, and verifies buckets are written. Use `setup_pool()` pattern.

**Step 4: Run tests**

Run: `cargo nextest run -p feature-productivity`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/feature-productivity/src/bucket_aggregator.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add BucketAggregator — 5-min window subscriber"
```

---

## Task 8: `AutoFocusDetector` FSM

**Files:**
- Create: `crates/feature-productivity/src/auto_focus.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write the FSM**

Create `crates/feature-productivity/src/auto_focus.rs`:

The FSM has 5 states: `Unfocused`, `Building`, `Focused`, `Cooldown`, `Ended`.

Key logic:
- Accumulates ticks into 5-min `WindowStats`
- `Unfocused → Building`: 3 consecutive productive windows (>80% productive, <2 switches)
- `Building → Focused`: 15min elapsed
- `Focused → Cooldown`: idle >3min OR productive_ratio < 0.5
- `Cooldown → Focused`: recovered within grace period
- `Cooldown → Ended`: not recovered → emit `AutoFocusSession` → reset to `Unfocused`

The detector should accept a `tokio::sync::mpsc::Sender<AutoFocusSession>` to notify when a session is detected. It does NOT write to DB — the `ProductivityEngine` or desktop crate handles confirmation.

```rust
pub struct AutoFocusSession {
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub dominant_app: String,
    pub dominant_category: Option<String>,
    pub productive_ratio: f64,
    pub total_secs: i64,
}
```

The detector is a subscriber that consumes `broadcast::Receiver<ActivityTick>` and runs its FSM on each tick.

**Step 2: Write tests**

Test each state transition:
- Test `Unfocused → Building` with 3 productive windows
- Test `Building → Focused` after 15min
- Test `Focused → Cooldown → Focused` (recovery)
- Test `Focused → Cooldown → Ended` (no recovery)
- Test that `AutoFocusSession` is emitted on `Ended`

Use deterministic ticks (constructed manually, not from macOS APIs).

**Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(auto_focus)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/auto_focus.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add AutoFocusDetector FSM with confirmable sessions"
```

---

## Task 9: `DistractionAnalyzer` Subscriber

**Files:**
- Create: `crates/feature-productivity/src/distraction_analyzer.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write `DistractionAnalyzer`**

Create `crates/feature-productivity/src/distraction_analyzer.rs`:

Subscribes to `broadcast::Receiver<ActivityTick>`. Tracks:
- `last_productive_tick: Option<ActivityTick>` — most recent productive tick
- `productive_streak_start: Option<DateTime<Utc>>` — when current productive streak began
- `last_break_time: Option<DateTime<Utc>>` — last idle period >5min
- `day_start: Option<DateTime<Utc>>` — first tick of the day
- `pending_recovery: Option<(i64, DateTime<Utc>)>` — (pattern_id, distraction_start)

On each tick:
1. If current tick is distracting AND previous was productive → **distraction transition**
   - Record `DistractionPattern` to DB (preceding_app, hours_active, mins_since_break, etc.)
   - Set `pending_recovery = Some((pattern_id, now))`
2. If `pending_recovery.is_some()` AND current tick is productive → **recovery**
   - Compute `recovery_secs = now - distraction_start`
   - Update DB with `repos.distraction_patterns.update_recovery(id, recovery_secs)`
   - Clear `pending_recovery`
3. Track idle periods >5min as "breaks" for `mins_since_break` calculation

**Step 2: Add `is_high_risk_window()` method**

```rust
/// Check current fatigue signals against historical patterns.
/// Used by NudgeService to suggest proactive breaks.
pub async fn is_high_risk_window(repos: &ProductivityRepos, now: DateTime<Utc>) -> common::Result<bool> {
    let today = now.format("%Y-%m-%d").to_string();
    let hour = now.hour() as i32;
    // Check if this hour historically has >2x average distraction rate
    let fourteen_days_ago = (now - chrono::Duration::days(14)).format("%Y-%m-%d").to_string();
    let by_hour = repos.distraction_patterns.count_by_hour(&fourteen_days_ago, &today).await?;
    let total: i64 = by_hour.iter().map(|(_, c)| c).sum();
    let hours_with_data = by_hour.len().max(1) as f64;
    let avg_per_hour = total as f64 / hours_with_data;
    let this_hour_count = by_hour.iter().find(|(h, _)| *h == hour).map(|(_, c)| *c).unwrap_or(0);
    Ok(this_hour_count as f64 > avg_per_hour * 2.0)
}
```

**Step 3: Write tests**

Test distraction transition detection, recovery measurement, break tracking.

**Step 4: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(distraction_analyzer)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/feature-productivity/src/distraction_analyzer.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add DistractionAnalyzer — trigger + fatigue pattern tracking"
```

---

## Task 10: Heuristic Insight Engine

**Files:**
- Create: `crates/feature-productivity/src/insights.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write the insight engine**

Create `crates/feature-productivity/src/insights.rs`:

```rust
pub struct InsightEngine {
    repos: ProductivityRepos,
}
```

Key methods:
- `generate_for_date(date: &str) -> Result<Vec<InsightCard>>` — runs all heuristic checks
- `compute_baseline(end_date: &str) -> Result<Baseline>` — 14-day rolling averages from daily_summaries

```rust
struct Baseline {
    avg_productive_hours: f64,
    avg_deep_work_blocks: f64,
    avg_context_switches: f64,
    avg_distraction_rate: f64,
    avg_recovery_secs: f64,
    avg_score: f64,
    std_score: f64,
    max_score_30d: f64,
}
```

Each insight type is a function that checks today's metrics against baseline:
- `check_deep_work_trend(today, baseline)` — deep_work_blocks > baseline.avg + 1
- `check_distraction_spike(today, baseline)` — distraction_rate > baseline * 1.5
- `check_new_personal_best(today, baseline)` — score > max_score_30d
- `check_streak(recent_summaries, threshold)` — N consecutive days above threshold
- `check_fatigue_warning(distraction_patterns)` — >2x rate after 3h active
- etc.

Deduplication: check `repos.insights.exists_for_date(type, date)` before creating.

**Step 2: Write tests**

Test each insight type with constructed baselines and today values.

**Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(insight)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/insights.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add heuristic InsightEngine with 9 insight types"
```

---

## Task 11: `ProductivityEngine` Orchestrator

**Files:**
- Create: `crates/feature-productivity/src/engine.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write `ProductivityEngine`**

Create `crates/feature-productivity/src/engine.rs`:

```rust
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::auto_focus::{AutoFocusDetector, AutoFocusSession};
use crate::batch_writer::BatchWriter;
use crate::bucket_aggregator::BucketAggregator;
use crate::config::ProductivityConfig;
use crate::distraction_analyzer::DistractionAnalyzer;
use crate::repos::ProductivityRepos;
use crate::tracker::ActivityTracker;
use crate::tracker::categorizer::Categorizer;
use crate::types::ActivityTick;

const BROADCAST_CAPACITY: usize = 128;

pub struct ProductivityEngine {
    tracker: ActivityTracker,
    batch_writer: Option<BatchWriter>,
    bucket_aggregator: Option<BucketAggregator>,
    auto_focus: Option<AutoFocusDetector>,
    distraction_analyzer: Option<DistractionAnalyzer>,
    cancel_token: CancellationToken,
    auto_focus_rx: Option<mpsc::Receiver<AutoFocusSession>>,
    tick_sender: broadcast::Sender<ActivityTick>,
}

impl ProductivityEngine {
    pub fn new(
        config: ProductivityConfig,
        repos: ProductivityRepos,
        categorizer: Categorizer,
    ) -> Self {
        let (tick_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let cancel = CancellationToken::new();

        let tracker = ActivityTracker::new(config.clone(), categorizer, tick_tx.clone());

        // BatchWriter
        let batch_writer = BatchWriter::start(
            tick_tx.subscribe(),
            repos.clone(),
            config.privacy.clone(),
            config.tracking.batch_write_interval_secs,
            cancel.child_token(),
        );

        // BucketAggregator
        let bucket_aggregator = BucketAggregator::start(
            tick_tx.subscribe(),
            repos.clone(),
            config.tracking.poll_interval_secs,
            cancel.child_token(),
        );

        // AutoFocusDetector
        let (auto_focus_tx, auto_focus_rx) = mpsc::channel(16);
        let auto_focus = if config.focus.auto_detect_enabled {
            Some(AutoFocusDetector::start(
                tick_tx.subscribe(),
                auto_focus_tx,
                config.focus.clone(),
                cancel.child_token(),
            ))
        } else {
            None
        };

        // DistractionAnalyzer
        let distraction_analyzer = DistractionAnalyzer::start(
            tick_tx.subscribe(),
            repos.clone(),
            cancel.child_token(),
        );

        Self {
            tracker,
            batch_writer: Some(batch_writer),
            bucket_aggregator: Some(bucket_aggregator),
            auto_focus,
            distraction_analyzer: Some(distraction_analyzer),
            cancel_token: cancel,
            auto_focus_rx: Some(auto_focus_rx),
            tick_sender: tick_tx,
        }
    }

    /// Take the auto-focus session receiver (for desktop crate to consume).
    pub fn take_auto_focus_rx(&mut self) -> Option<mpsc::Receiver<AutoFocusSession>> {
        self.auto_focus_rx.take()
    }

    /// Get a new broadcast subscriber (for DashboardEmitter).
    pub fn subscribe(&self) -> broadcast::Receiver<ActivityTick> {
        self.tick_sender.subscribe()
    }

    pub fn start(&mut self) {
        self.tracker.start();
    }

    pub async fn stop(&mut self) {
        self.tracker.stop().await;
        self.cancel_token.cancel();
        if let Some(mut bw) = self.batch_writer.take() {
            bw.stop().await;
        }
        if let Some(mut ba) = self.bucket_aggregator.take() {
            ba.stop().await;
        }
        if let Some(mut af) = self.auto_focus.take() {
            af.stop().await;
        }
        if let Some(mut da) = self.distraction_analyzer.take() {
            da.stop().await;
        }
    }

    pub fn categorizer(&self) -> &std::sync::Arc<tokio::sync::RwLock<Categorizer>> {
        self.tracker.categorizer()
    }
}
```

**Step 2: Export from `lib.rs`**

Add `pub mod engine;` and `pub use engine::ProductivityEngine;`.

**Step 3: Run tests**

Run: `cargo build -p feature-productivity`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/engine.rs crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add ProductivityEngine orchestrator with broadcast bus"
```

---

## Task 12: Tauri Event Constants and `DashboardEmitter`

**Files:**
- Modify: `crates/desktop-shared/src/events.rs`
- Create: `crates/feature-productivity/src/dashboard_emitter.rs` (or in desktop crate)

**Step 1: Add event constants to `desktop-shared/src/events.rs`**

Append after line 55 (`PRODUCTIVITY_NUDGE`):

```rust
pub const ACTIVITY_TICK: &str = "activity:tick";
pub const ACTIVITY_SWITCH: &str = "activity:switch";
pub const FOCUS_STATE_CHANGED: &str = "focus:state_changed";
pub const FOCUS_AUTO_DETECTED: &str = "focus:auto_detected";
pub const SCORE_UPDATED: &str = "score:updated";
pub const BUCKET_COMPLETED: &str = "bucket:completed";
pub const INSIGHT_GENERATED: &str = "insight:generated";
```

Add corresponding payload structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTickPayload {
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_type: Option<String>,
    pub is_idle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySwitchPayload {
    pub from_app: Option<String>,
    pub to_app: String,
    pub to_site: Option<String>,
    pub category_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusStatePayload {
    pub state: String,
    pub since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoFocusPayload {
    pub started_at: String,
    pub ended_at: String,
    pub duration_mins: i64,
    pub dominant_app: String,
    pub productive_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScorePayload {
    pub score: f64,
    pub productive_secs: i64,
    pub distracting_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketPayload {
    pub bucket_start: String,
    pub productive_secs: i64,
    pub distracting_secs: i64,
    pub dominant_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightPayload {
    pub id: String,
    pub insight_type: String,
    pub title: String,
    pub sentiment: String,
}
```

**Step 2: Commit**

```bash
git add crates/desktop-shared/src/events.rs
git commit -m "feat(desktop-shared): add Tauri event constants and payloads for real-time dashboard"
```

---

## Task 13: Desktop Crate Integration

**Files:**
- Modify: `crates/desktop/src/app_core.rs`
- Modify: `crates/desktop/src/commands/productivity.rs`

**Step 1: Simplify `AppCore::init()`**

Replace the scattered `ActivityTracker` + `FocusManager` + `NudgeService` setup with:

```rust
let engine = ProductivityEngine::new(config.productivity.clone(), prod_repos.clone(), categorizer);
```

Store `engine` in `AppCore`. Wire up `engine.take_auto_focus_rx()` to a Tauri event emitter task.

Wire up `engine.subscribe()` for the `DashboardEmitter` that forwards ticks to Tauri events.

**Step 2: Add new Tauri commands**

In `crates/desktop/src/commands/productivity.rs`, add:

```rust
#[tauri::command]
pub async fn productivity_insights(
    state: tauri::State<'_, AppState>,
    date: Option<String>,
) -> Result<Vec<InsightCard>, String> {
    let date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let engine = InsightEngine::new(state.prod_repos.clone());
    engine.generate_for_date(&date).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn productivity_auto_focus_confirm(
    state: tauri::State<'_, AppState>,
    session: AutoFocusSession,
) -> Result<(), String> {
    // Convert AutoFocusSession → FocusSession with source: SessionSource::AutoDetected
    // Save via FocusManager or directly via repos
    // ...
}

#[tauri::command]
pub async fn productivity_auto_focus_dismiss() -> Result<(), String> {
    Ok(()) // Just acknowledge — stats still feed pattern analysis
}
```

**Step 3: Run tests**

Run: `cargo build -p desktop`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/desktop/
git commit -m "feat(desktop): integrate ProductivityEngine, add insight and auto-focus commands"
```

---

## Task 14: Frontend — Remove Polling, Add Event Subscriptions

**Files:**
- Modify: `desktop-ui/src/components/productivity/ActivityFeed.tsx`
- Create: `desktop-ui/src/components/productivity/AutoFocusToast.tsx`
- Create: `desktop-ui/src/components/productivity/InsightCardList.tsx`
- Create: `desktop-ui/src/components/productivity/LiveScoreRing.tsx`
- Create: `desktop-ui/src/components/productivity/FocusStateIndicator.tsx`

**Step 1: Remove polling from `ActivityFeed.tsx`**

Replace `setInterval(refetch, 5000)` with Tauri event listener:

```typescript
import { listen } from '@tauri-apps/api/event';

useEffect(() => {
  const unlisten = listen('activity:switch', (event) => {
    // Prepend new activity to feed
    setActivities(prev => [event.payload, ...prev].slice(0, 50));
  });
  return () => { unlisten.then(fn => fn()); };
}, []);
```

**Step 2: Create `AutoFocusToast.tsx`**

Listens to `focus:auto_detected` event. Shows a non-intrusive toast with Confirm / Dismiss / Adjust buttons. On Confirm, calls `invoke('productivity_auto_focus_confirm', { session })`.

**Step 3: Create `InsightCardList.tsx`**

Fetches insights via `invoke('productivity_insights')` on mount. Listens to `insight:generated` for real-time additions. Each card shows title, body, sentiment badge, metric comparison.

**Step 4: Create `LiveScoreRing.tsx`**

Subscribes to `score:updated` event. Animated circular progress showing current productivity score.

**Step 5: Create `FocusStateIndicator.tsx`**

Subscribes to `focus:state_changed` event. Shows current auto-focus state (building/focused/cooldown) as a subtle indicator.

**Step 6: Run frontend lint**

Run: `cd desktop-ui && bun run lint`
Expected: No errors

**Step 7: Commit**

```bash
git add desktop-ui/src/components/productivity/
git commit -m "feat(desktop-ui): real-time dashboard — event subscriptions, insight cards, auto-focus toast"
```

---

## Task 15: Update `DailyAggregator` for Bucket Fallback

**Files:**
- Modify: `crates/feature-productivity/src/aggregator.rs`

**Step 1: Add bucket fallback logic**

In `compute_for_date()`, check if the date is within raw retention (7 days). If not, query `activity_buckets` instead of `activity_events`:

```rust
let within_raw_retention = {
    let cutoff = Utc::now() - chrono::Duration::days(config.tracking.raw_retention_days as i64);
    start >= cutoff
};

if within_raw_retention {
    // Existing logic: query activity_events
} else {
    // Query activity_buckets with SUM aggregation
    let bucket_data = repos.buckets.aggregate_day(date).await?;
    // Build DailySummary from bucket aggregates
}
```

**Step 2: Add deep_work_blocks computation**

In `compute_for_date()`, after fetching focus sessions, compute deep work:

```rust
let deep_work_blocks = sessions.iter()
    .filter(|s| s.session_type != SessionType::Break)
    .filter(|s| s.actual_mins.unwrap_or(0) >= 25)
    .count() as i64;
let deep_work_secs = sessions.iter()
    .filter(|s| s.session_type != SessionType::Break)
    .filter(|s| s.actual_mins.unwrap_or(0) >= 25)
    .filter_map(|s| s.actual_mins)
    .sum::<i64>() * 60;
```

**Step 3: Add avg_recovery_secs**

```rust
let avg_recovery_secs = repos.distraction_patterns
    .avg_recovery_secs(date, date).await?;
```

**Step 4: Update purge logic**

Replace single `purge_old_data` with dual purge:

```rust
pub async fn purge_old_data(&self, raw_days: u64, bucket_days: u64) -> common::Result<(u64, u64)> {
    let raw_cutoff = Utc::now() - chrono::Duration::days(raw_days as i64);
    let raw_purged = self.repos.events.purge_before(&raw_cutoff).await?;
    let bucket_cutoff = (Utc::now() - chrono::Duration::days(bucket_days as i64))
        .format("%Y-%m-%d").to_string();
    let bucket_purged = self.repos.buckets.purge_before(&bucket_cutoff).await?;
    Ok((raw_purged, bucket_purged))
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(aggregate)'`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/feature-productivity/src/aggregator.rs
git commit -m "feat(productivity): aggregator bucket fallback, deep work metrics, dual-tier purge"
```

---

## Task 16: Wire `DistractionAnalyzer` into `NudgeService`

**Files:**
- Modify: `crates/feature-productivity/src/nudge.rs`

**Step 1: Add fatigue-based nudge**

In `check_nudges()`, after existing break/burnout checks, add:

```rust
// 3. Proactive break suggestion — high distraction risk window.
if config.focus_suggestions
    && should_send(repos, NudgeType::FocusSuggestion, config.cooldown_mins, now).await?
{
    if DistractionAnalyzer::is_high_risk_window(repos, now).await? {
        let record = NudgeRecord::new(
            NudgeType::FocusSuggestion,
            "You're in a historically high-distraction period. Consider a short break.".into(),
            now,
        );
        send_nudge(repos, sender, record).await?;
    }
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(nudge)'`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/nudge.rs
git commit -m "feat(productivity): NudgeService uses DistractionAnalyzer for proactive break suggestions"
```

---

## Task 17: Final Integration Test & Cleanup

**Files:**
- All modified files

**Step 1: Run full test suite**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

Fix any warnings or failures.

**Step 2: Run desktop build**

```bash
cargo build -p desktop
```

**Step 3: Run frontend build**

```bash
cd desktop-ui && bun run build
cd desktop-ui && bun run lint
```

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore(productivity): fix clippy warnings and build issues"
```

---

## Dependency Graph

```
Task 1 (migration) ─────────────────────────────┐
Task 2 (types) ──────────────────────────────────┤
                                                 ├── Task 3 (repos)
Task 4 (config) ─────────────────────────────────┘
                                                      │
Task 5 (tracker refactor) ───────────────────────────┤
                                                      │
Task 6 (batch_writer) ──────────────────────────────┤
Task 7 (bucket_aggregator) ─────────────────────────┤
Task 8 (auto_focus) ─────────────────────────────────┤
Task 9 (distraction_analyzer) ──────────────────────┤
Task 10 (insights) ──────────────────────────────────┤
                                                      │
Task 11 (engine) ◄────────────────────────────────────┘
                    │
Task 12 (events) ───┤
                    │
Task 13 (desktop) ◄─┘
                    │
Task 14 (frontend) ◄┘
                    │
Task 15 (aggregator update) ─── independent
Task 16 (nudge update) ──────── independent
                    │
Task 17 (integration) ◄─────── all tasks
```

**Parallelizable groups:**
- Tasks 1-4: can run in parallel (migration, types, config)
- Tasks 5-10: can run in parallel after Tasks 1-4 complete (all subscribers)
- Task 11: depends on Tasks 5-10
- Tasks 12-16: can run in parallel after Task 11
- Task 17: final integration after all
