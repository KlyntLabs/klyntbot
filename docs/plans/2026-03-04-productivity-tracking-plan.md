# Productivity Tracking & Focus Management — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Rize-inspired productivity tracking to Klyntbot — passive activity monitoring, focus sessions, proactive nudges, and a productivity dashboard.

**Architecture:** New `feature-productivity` crate following the `FeaturePackage` pattern. Activity tracker runs inside the Tauri app using macOS native APIs. A `ProductivityContextSource` feeds insights into the agent. A `NudgeService` background loop delivers proactive reminders.

**Tech Stack:** Rust (sqlx, objc2, objc2-app-kit, tokio, chrono, serde), TypeScript/React (Tauri IPC, Tailwind v4), SQLite.

**Design doc:** `docs/plans/2026-03-04-productivity-tracking-design.md`

---

## Phase 1: Storage & Config Foundation

### Task 1: Create `feature-productivity` crate skeleton

**Files:**
- Create: `crates/feature-productivity/Cargo.toml`
- Create: `crates/feature-productivity/src/lib.rs`
- Create: `crates/feature-productivity/src/config.rs`
- Create: `crates/feature-productivity/src/types.rs`
- Modify: `Cargo.toml` (workspace root)

**Step 1: Create Cargo.toml**

```toml
[package]
name = "feature-productivity"
version = "0.1.0"
edition = "2021"

[dependencies]
common.workspace = true
tools-core.workspace = true
storage.workspace = true
async-trait.workspace = true
serde = { workspace = true }
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono = { workspace = true }
uuid = { workspace = true }
sqlx.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

**Step 2: Create types.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: Option<i64>,
    pub app_name: String,
    pub window_title: Option<String>,
    pub bundle_id: Option<String>,
    pub url: Option<String>,
    pub category_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub is_idle: bool,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Productive,
    Neutral,
    Distracting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCategory {
    pub id: String,
    pub name: String,
    pub category_type: CategoryType,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub rules: Option<CategoryRules>,
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRules {
    pub app_names: Vec<String>,
    pub bundle_ids: Vec<String>,
    pub url_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Focus,
    Pomodoro,
    Break,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistractionEvent {
    pub timestamp: DateTime<Utc>,
    pub app_name: String,
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySummary {
    pub date: String,
    pub total_active_secs: i64,
    pub total_focus_secs: i64,
    pub total_break_secs: i64,
    pub total_idle_secs: i64,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub focus_sessions_count: i64,
    pub avg_session_quality: Option<f64>,
    pub interruptions_count: i64,
    pub context_switches: i64,
    pub top_apps: Vec<AppUsage>,
    pub top_categories: Vec<CategoryUsage>,
    pub ai_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub app_name: String,
    pub duration_secs: i64,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsage {
    pub category: String,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgeType {
    BreakReminder,
    FocusSuggestion,
    DailySummary,
    BurnoutAlert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NudgeRecord {
    pub id: Option<i64>,
    pub nudge_type: NudgeType,
    pub message: String,
    pub channel: Option<String>,
    pub acknowledged: bool,
    pub created_at: DateTime<Utc>,
}
```

**Step 3: Create config.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub focus: FocusConfig,
    #[serde(default)]
    pub nudges: NudgeConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_secs: u64,
    #[serde(default = "default_batch_interval")]
    pub batch_write_interval_secs: u64,
    #[serde(default = "default_retention")]
    pub retention_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusConfig {
    #[serde(default = "default_focus_duration")]
    pub default_duration_mins: u64,
    #[serde(default = "default_break_interval")]
    pub break_interval_mins: u64,
    #[serde(default = "default_break_duration")]
    pub break_duration_mins: u64,
    #[serde(default = "default_max_daily_focus")]
    pub max_daily_focus_hours: u64,
    #[serde(default = "default_true")]
    pub soft_block_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NudgeConfig {
    #[serde(default = "default_true")]
    pub break_reminders: bool,
    #[serde(default = "default_true")]
    pub focus_suggestions: bool,
    #[serde(default = "default_true")]
    pub daily_summary: bool,
    #[serde(default = "default_true")]
    pub burnout_alerts: bool,
    #[serde(default = "default_cooldown")]
    pub cooldown_mins: u64,
    #[serde(default)]
    pub quiet_hours_start: Option<String>,
    #[serde(default)]
    pub quiet_hours_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyConfig {
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    #[serde(default)]
    pub exclude_window_titles: bool,
    #[serde(default)]
    pub excluded_url_patterns: Vec<String>,
}

fn default_true() -> bool { true }
fn default_poll_interval() -> u64 { 5 }
fn default_idle_threshold() -> u64 { 120 }
fn default_batch_interval() -> u64 { 30 }
fn default_retention() -> u64 { 90 }
fn default_focus_duration() -> u64 { 45 }
fn default_break_interval() -> u64 { 90 }
fn default_break_duration() -> u64 { 10 }
fn default_max_daily_focus() -> u64 { 8 }
fn default_cooldown() -> u64 { 15 }

impl Default for ProductivityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tracking: TrackingConfig::default(),
            focus: FocusConfig::default(),
            nudges: NudgeConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval(),
            idle_threshold_secs: default_idle_threshold(),
            batch_write_interval_secs: default_batch_interval(),
            retention_days: default_retention(),
        }
    }
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            default_duration_mins: default_focus_duration(),
            break_interval_mins: default_break_interval(),
            break_duration_mins: default_break_duration(),
            max_daily_focus_hours: default_max_daily_focus(),
            soft_block_enabled: true,
        }
    }
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            break_reminders: true,
            focus_suggestions: true,
            daily_summary: true,
            burnout_alerts: true,
            cooldown_mins: default_cooldown(),
            quiet_hours_start: None,
            quiet_hours_end: None,
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            excluded_apps: Vec::new(),
            exclude_window_titles: false,
            excluded_url_patterns: Vec::new(),
        }
    }
}
```

**Step 4: Create lib.rs with FeaturePackage impl**

```rust
pub mod config;
pub mod types;

use async_trait::async_trait;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub use config::ProductivityConfig;
pub use types::*;

pub struct ProductivityFeature {
    // Will hold the tool once we create it in Phase 3
}

impl ProductivityFeature {
    pub fn new() -> Self {
        Self {}
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_productivity_tables.sql")
    }
}

#[async_trait]
impl FeaturePackage for ProductivityFeature {
    fn name(&self) -> &str {
        "productivity"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![] // Will be populated in Phase 3
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 1,
            description: "Create productivity tracking tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    fn config_key(&self) -> &str {
        "productivity"
    }

    fn default_config(&self) -> Value {
        serde_json::to_value(ProductivityConfig::default()).unwrap_or(Value::Null)
    }

    async fn health_check(&self) -> common::Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
```

**Step 5: Register in workspace Cargo.toml**

In root `Cargo.toml`, add to `[workspace] members`:
```toml
"crates/feature-productivity",
```
And to `[workspace.dependencies]`:
```toml
feature-productivity = { path = "crates/feature-productivity" }
```

**Step 6: Build to verify**

Run: `cargo build -p feature-productivity`
Expected: Compiles successfully (will warn about missing migration file — that's Task 2)

**Step 7: Commit**

```bash
git add crates/feature-productivity/ Cargo.toml Cargo.lock
git commit -m "feat(productivity): add crate skeleton with types and config"
```

---

### Task 2: Create migration and repository layer

**Files:**
- Create: `crates/feature-productivity/migrations/001_productivity_tables.sql`
- Create: `crates/feature-productivity/src/repos/mod.rs`
- Create: `crates/feature-productivity/src/repos/activity_event.rs`
- Create: `crates/feature-productivity/src/repos/activity_category.rs`
- Create: `crates/feature-productivity/src/repos/focus_session.rs`
- Create: `crates/feature-productivity/src/repos/daily_summary.rs`
- Create: `crates/feature-productivity/src/repos/nudge.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Create migration SQL**

```sql
-- Activity events: raw window tracking data (high-frequency)
CREATE TABLE IF NOT EXISTS activity_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name      TEXT NOT NULL,
    window_title  TEXT,
    bundle_id     TEXT,
    url           TEXT,
    category_id   TEXT REFERENCES activity_categories(id),
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    is_idle       BOOLEAN NOT NULL DEFAULT FALSE,
    metadata      TEXT
);

CREATE INDEX IF NOT EXISTS idx_activity_events_started ON activity_events(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_category ON activity_events(category_id);
CREATE INDEX IF NOT EXISTS idx_activity_events_app ON activity_events(app_name, started_at DESC);

-- Activity categories: user-defined or AI-inferred
CREATE TABLE IF NOT EXISTS activity_categories (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    category_type TEXT NOT NULL DEFAULT 'productive',
    color         TEXT,
    icon          TEXT,
    rules         TEXT,
    is_system     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Default categories
INSERT OR IGNORE INTO activity_categories (id, name, category_type, rules, is_system) VALUES
    ('coding', 'Coding', 'productive', '{"app_names":["Visual Studio Code","Code","Xcode","IntelliJ IDEA","WebStorm","Terminal","iTerm2","Warp","Alacritty","kitty"],"bundle_ids":["com.microsoft.VSCode","com.apple.Terminal","com.googlecode.iterm2"],"url_patterns":[]}', TRUE),
    ('communication', 'Communication', 'neutral', '{"app_names":["Slack","Discord","Telegram","WhatsApp","Messages","Microsoft Teams","Zoom"],"bundle_ids":["com.tinyspeck.slackmacgap","com.hnc.Discord","ru.keepcoder.Telegram"],"url_patterns":[]}', TRUE),
    ('browsing', 'Browsing', 'neutral', '{"app_names":["Safari","Firefox","Google Chrome","Arc","Brave Browser"],"bundle_ids":["com.apple.Safari","org.mozilla.firefox","com.google.Chrome"],"url_patterns":[]}', TRUE),
    ('design', 'Design', 'productive', '{"app_names":["Figma","Sketch","Adobe Photoshop","Adobe Illustrator","Affinity Designer"],"bundle_ids":["com.figma.Desktop"],"url_patterns":[]}', TRUE),
    ('documentation', 'Documentation', 'productive', '{"app_names":["Notion","Obsidian","Bear","Typora","Pages"],"bundle_ids":["notion.id","md.obsidian"],"url_patterns":["docs.google.com","notion.so"]}', TRUE),
    ('entertainment', 'Entertainment', 'distracting', '{"app_names":[],"bundle_ids":[],"url_patterns":["youtube.com","netflix.com","twitter.com","x.com","reddit.com","tiktok.com","instagram.com","facebook.com"]}', TRUE),
    ('email', 'Email', 'neutral', '{"app_names":["Mail","Spark","Airmail","Superhuman"],"bundle_ids":["com.apple.mail"],"url_patterns":["mail.google.com","outlook.live.com"]}', TRUE);

-- Focus sessions: explicit deep work periods
CREATE TABLE IF NOT EXISTS focus_sessions (
    id            TEXT PRIMARY KEY,
    action_id     TEXT,
    project_id    TEXT,
    session_type  TEXT NOT NULL DEFAULT 'focus',
    target_mins   INTEGER,
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    actual_mins   INTEGER,
    interruptions INTEGER NOT NULL DEFAULT 0,
    distraction_events TEXT,
    quality_score REAL,
    completed     BOOLEAN NOT NULL DEFAULT FALSE,
    notes         TEXT
);

CREATE INDEX IF NOT EXISTS idx_focus_sessions_started ON focus_sessions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_focus_sessions_action ON focus_sessions(action_id);

-- Daily aggregated summaries
CREATE TABLE IF NOT EXISTS daily_summaries (
    date                 TEXT PRIMARY KEY,
    total_active_secs    INTEGER NOT NULL DEFAULT 0,
    total_focus_secs     INTEGER NOT NULL DEFAULT 0,
    total_break_secs     INTEGER NOT NULL DEFAULT 0,
    total_idle_secs      INTEGER NOT NULL DEFAULT 0,
    productive_secs      INTEGER NOT NULL DEFAULT 0,
    neutral_secs         INTEGER NOT NULL DEFAULT 0,
    distracting_secs     INTEGER NOT NULL DEFAULT 0,
    focus_sessions_count INTEGER NOT NULL DEFAULT 0,
    avg_session_quality  REAL,
    interruptions_count  INTEGER NOT NULL DEFAULT 0,
    context_switches     INTEGER NOT NULL DEFAULT 0,
    top_apps             TEXT,
    top_categories       TEXT,
    ai_summary           TEXT,
    computed_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Nudge history: prevents over-nudging
CREATE TABLE IF NOT EXISTS nudge_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    nudge_type    TEXT NOT NULL,
    message       TEXT NOT NULL,
    channel       TEXT,
    acknowledged  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_nudge_history_type_created ON nudge_history(nudge_type, created_at DESC);
```

**Step 2: Write tests for ActivityEventRepo**

Create `crates/feature-productivity/src/repos/activity_event.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::types::ActivityEvent;

pub struct ActivityEventRepo {
    pool: SqlitePool,
}

impl ActivityEventRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, event: &ActivityEvent) -> common::Result<i64> {
        let metadata = event.metadata.as_ref().map(|m| m.to_string());
        let row = sqlx::query_scalar!(
            r#"INSERT INTO activity_events (app_name, window_title, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               RETURNING id"#,
            event.app_name,
            event.window_title,
            event.bundle_id,
            event.url,
            event.category_id,
            event.started_at,
            event.ended_at,
            event.duration_secs,
            event.is_idle,
            metadata,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn insert_batch(&self, events: &[ActivityEvent]) -> common::Result<()> {
        let mut tx = self.pool.begin().await?;
        for event in events {
            let metadata = event.metadata.as_ref().map(|m| m.to_string());
            sqlx::query!(
                r#"INSERT INTO activity_events (app_name, window_title, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                event.app_name,
                event.window_title,
                event.bundle_id,
                event.url,
                event.category_id,
                event.started_at,
                event.ended_at,
                event.duration_secs,
                event.is_idle,
                metadata,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_range(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> common::Result<Vec<ActivityEvent>> {
        let rows = sqlx::query_as!(
            ActivityEventRow,
            r#"SELECT id, app_name, window_title, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata
               FROM activity_events
               WHERE started_at >= ? AND started_at < ?
               ORDER BY started_at ASC"#,
            start, end,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(ActivityEvent::from).collect())
    }

    pub async fn purge_before(&self, before: &DateTime<Utc>) -> common::Result<u64> {
        let result = sqlx::query!("DELETE FROM activity_events WHERE started_at < ?", before)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn count_context_switches(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> common::Result<i64> {
        // Context switch = consecutive non-idle events with different app_name
        let rows = sqlx::query_scalar!(
            r#"SELECT app_name FROM activity_events
               WHERE started_at >= ? AND started_at < ? AND is_idle = FALSE
               ORDER BY started_at ASC"#,
            start, end,
        )
        .fetch_all(&self.pool)
        .await?;

        let switches = rows.windows(2).filter(|w| w[0] != w[1]).count();
        Ok(switches as i64)
    }

    pub async fn aggregate_by_category(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> common::Result<Vec<(Option<String>, i64)>> {
        let rows = sqlx::query!(
            r#"SELECT category_id, COALESCE(SUM(duration_secs), 0) as total_secs
               FROM activity_events
               WHERE started_at >= ? AND started_at < ? AND is_idle = FALSE
               GROUP BY category_id
               ORDER BY total_secs DESC"#,
            start, end,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.category_id, r.total_secs.unwrap_or(0))).collect())
    }

    pub async fn top_apps(&self, start: &DateTime<Utc>, end: &DateTime<Utc>, limit: i64) -> common::Result<Vec<(String, i64)>> {
        let rows = sqlx::query!(
            r#"SELECT app_name, COALESCE(SUM(duration_secs), 0) as total_secs
               FROM activity_events
               WHERE started_at >= ? AND started_at < ? AND is_idle = FALSE
               GROUP BY app_name
               ORDER BY total_secs DESC
               LIMIT ?"#,
            start, end, limit,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.app_name, r.total_secs.unwrap_or(0))).collect())
    }
}

// Internal row type for sqlx mapping (implement From<Row> -> ActivityEvent)
// This will need adjustment based on how sqlx maps the actual columns
```

**Note:** The repos for `activity_category`, `focus_session`, `daily_summary`, and `nudge` follow the same pattern. Each gets:
- `new(pool)` constructor
- CRUD methods relevant to its domain
- `list_*`, `get_*`, `insert`, `update`, `delete` as needed

Implement all 5 repos following the same repository pattern as `storage::ActionRepo`.

**Step 3: Write the remaining repos**

`activity_category.rs` — `get(id)`, `list_all()`, `upsert(category)`, `delete(id)`, `match_app(app_name, bundle_id, url) -> Option<ActivityCategory>` (rule matching logic)

`focus_session.rs` — `create(session)`, `get(id)`, `get_active() -> Option<FocusSession>`, `update(session)`, `end_session(id, quality_score)`, `list_range(start, end)`, `list_by_action(action_id)`

`daily_summary.rs` — `upsert(summary)`, `get(date)`, `list_range(start_date, end_date)`

`nudge.rs` — `insert(nudge)`, `last_of_type(nudge_type) -> Option<NudgeRecord>`, `acknowledge(id)`, `list_recent(limit)`

`repos/mod.rs`:
```rust
pub mod activity_event;
pub mod activity_category;
pub mod focus_session;
pub mod daily_summary;
pub mod nudge;

pub use activity_event::ActivityEventRepo;
pub use activity_category::ActivityCategoryRepo;
pub use focus_session::FocusSessionRepo;
pub use daily_summary::DailySummaryRepo;
pub use nudge::NudgeRepo;

/// Aggregate access to all productivity repos
pub struct ProductivityRepos {
    pub events: ActivityEventRepo,
    pub categories: ActivityCategoryRepo,
    pub sessions: FocusSessionRepo,
    pub summaries: DailySummaryRepo,
    pub nudges: NudgeRepo,
}

impl ProductivityRepos {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            events: ActivityEventRepo::new(pool.clone()),
            categories: ActivityCategoryRepo::new(pool.clone()),
            sessions: FocusSessionRepo::new(pool.clone()),
            summaries: DailySummaryRepo::new(pool.clone()),
            nudges: NudgeRepo::new(pool),
        }
    }
}
```

**Step 4: Write tests for repos**

Create `crates/feature-productivity/tests/repos_test.rs`:

Test each repo with an ephemeral SQLite pool. Key tests:
- `test_insert_and_list_activity_events` — insert 3 events, list by range, verify order
- `test_batch_insert_events` — batch insert 10 events, verify count
- `test_category_matching` — create categories with rules, verify `match_app()` works
- `test_focus_session_lifecycle` — create → get_active → end → verify quality_score set
- `test_daily_summary_upsert` — insert, upsert with new values, verify overwrite
- `test_nudge_cooldown` — insert nudge, verify `last_of_type` returns it, check cooldown logic
- `test_context_switch_count` — insert alternating app events, verify count
- `test_purge_old_events` — insert old + new events, purge, verify only new remain

Run: `cargo nextest run -p feature-productivity`
Expected: All tests pass

**Step 5: Commit**

```bash
git add crates/feature-productivity/
git commit -m "feat(productivity): add migration and repository layer with tests"
```

---

### Task 3: Register config in global config crate

**Files:**
- Create: `crates/config/src/schema/productivity.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

**Step 1: Create config schema**

`crates/config/src/schema/productivity.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub focus: FocusConfig,
    #[serde(default)]
    pub nudges: NudgeConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

// ... (same structs as feature-productivity/src/config.rs — mirror the shape)
// The config crate version is used for global Config deserialization.
// The feature crate version is self-contained for FeaturePackage::default_config().

impl Default for ProductivityConfig { ... }
```

**Step 2: Register in mod.rs and core.rs**

In `crates/config/src/schema/mod.rs`, add: `pub mod productivity;`

In `crates/config/src/schema/core.rs`, add:
- Import: `use super::productivity::ProductivityConfig;`
- Field: `#[serde(default)] pub productivity: ProductivityConfig,`

**Step 3: Build**

Run: `cargo build -p config`
Expected: Compiles

**Step 4: Commit**

```bash
git add crates/config/
git commit -m "feat(config): add productivity config schema"
```

---

## Phase 2: macOS Activity Tracker

### Task 4: Create the activity tracker core

**Files:**
- Create: `crates/feature-productivity/src/tracker/mod.rs`
- Create: `crates/feature-productivity/src/tracker/macos.rs`
- Create: `crates/feature-productivity/src/tracker/categorizer.rs`
- Modify: `crates/feature-productivity/src/lib.rs`
- Modify: `crates/feature-productivity/Cargo.toml`

**Step 1: Add macOS dependencies to Cargo.toml**

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = ["NSWorkspace", "NSRunningApplication"] }
objc2-foundation = { version = "0.3", features = ["NSString"] }
core-graphics = "0.24"
core-foundation = "0.10"
```

**Step 2: Create macos.rs — native window info**

```rust
//! macOS-specific window and idle detection using native APIs.

use common::Result;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
}

/// Get the currently focused window's app name and bundle ID.
#[cfg(target_os = "macos")]
pub fn get_frontmost_window() -> Result<Option<WindowInfo>> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = unsafe { NSWorkspace::sharedWorkspace() };
    let app = unsafe { workspace.frontmostApplication() };

    match app {
        Some(app) => {
            let name = unsafe { app.localizedName() }
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let bundle = unsafe { app.bundleIdentifier() }
                .map(|b| b.to_string());

            // Window title via CGWindowListCopyWindowInfo
            let title = get_window_title_cg();

            Ok(Some(WindowInfo {
                app_name: name,
                bundle_id: bundle,
                window_title: title,
            }))
        }
        None => Ok(None),
    }
}

#[cfg(target_os = "macos")]
fn get_window_title_cg() -> Option<String> {
    // Use CGWindowListCopyWindowInfo to get the frontmost window title
    // This requires the kCGWindowListOptionOnScreenOnly + kCGWindowListExcludeDesktopElements options
    // Filter for kCGWindowLayer == 0 (normal windows) and the frontmost PID
    // Return kCGWindowName from the matching entry
    //
    // Implementation uses core_graphics::display::* APIs
    // Fallback: return None if accessibility permission not granted
    todo!("Implement CGWindowListCopyWindowInfo title extraction")
}

/// Get seconds since last user input (mouse/keyboard).
#[cfg(target_os = "macos")]
pub fn seconds_since_last_input() -> f64 {
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    CGEventSource::seconds_since_last_event_type(
        CGEventSourceStateID::CombinedSessionState,
        // kCGAnyInputEventType
        core_graphics::event::CGEventType::Null,
    )
}

// Stubs for non-macOS (for compilation on CI/Linux)
#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window() -> Result<Option<WindowInfo>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn seconds_since_last_input() -> f64 {
    0.0
}
```

**Step 3: Create categorizer.rs**

```rust
use crate::repos::ActivityCategoryRepo;
use crate::types::{ActivityCategory, CategoryRules, CategoryType};

pub struct Categorizer {
    /// Cached categories loaded from DB
    categories: Vec<ActivityCategory>,
}

impl Categorizer {
    pub fn new(categories: Vec<ActivityCategory>) -> Self {
        Self { categories }
    }

    /// Reload categories from DB
    pub async fn refresh(&mut self, repo: &ActivityCategoryRepo) -> common::Result<()> {
        self.categories = repo.list_all().await?;
        Ok(())
    }

    /// Match an app to a category using rules
    pub fn categorize(&self, app_name: &str, bundle_id: Option<&str>, url: Option<&str>) -> Option<&ActivityCategory> {
        for cat in &self.categories {
            if let Some(ref rules) = cat.rules {
                // Check bundle_id first (most specific)
                if let Some(bid) = bundle_id {
                    if rules.bundle_ids.iter().any(|r| bid.eq_ignore_ascii_case(r)) {
                        return Some(cat);
                    }
                }
                // Check app_name
                if rules.app_names.iter().any(|r| app_name.eq_ignore_ascii_case(r)) {
                    return Some(cat);
                }
                // Check URL patterns
                if let Some(u) = url {
                    if rules.url_patterns.iter().any(|p| u.contains(p)) {
                        return Some(cat);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_by_app_name() {
        let categories = vec![
            ActivityCategory {
                id: "coding".into(),
                name: "Coding".into(),
                category_type: CategoryType::Productive,
                color: None,
                icon: None,
                rules: Some(CategoryRules {
                    app_names: vec!["Visual Studio Code".into(), "Terminal".into()],
                    bundle_ids: vec![],
                    url_patterns: vec![],
                }),
                is_system: true,
            },
        ];
        let cat = Categorizer::new(categories);
        assert_eq!(cat.categorize("Visual Studio Code", None, None).unwrap().id, "coding");
        assert!(cat.categorize("Unknown App", None, None).is_none());
    }

    #[test]
    fn test_categorize_by_url() {
        let categories = vec![
            ActivityCategory {
                id: "entertainment".into(),
                name: "Entertainment".into(),
                category_type: CategoryType::Distracting,
                color: None,
                icon: None,
                rules: Some(CategoryRules {
                    app_names: vec![],
                    bundle_ids: vec![],
                    url_patterns: vec!["youtube.com".into(), "reddit.com".into()],
                }),
                is_system: true,
            },
        ];
        let cat = Categorizer::new(categories);
        assert_eq!(
            cat.categorize("Safari", None, Some("https://www.youtube.com/watch?v=abc")).unwrap().id,
            "entertainment"
        );
    }
}
```

**Step 4: Create tracker/mod.rs — the main tracking loop**

```rust
//! Activity tracker: polls the active window every N seconds,
//! buffers events, and batch-writes to SQLite.

pub mod macos;
pub mod categorizer;

use std::sync::Arc;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use chrono::Utc;
use tracing::{debug, info, warn};

use crate::config::{ProductivityConfig, PrivacyConfig};
use crate::repos::ProductivityRepos;
use crate::types::ActivityEvent;
use categorizer::Categorizer;

pub struct ActivityTracker {
    config: ProductivityConfig,
    repos: ProductivityRepos,
    categorizer: Arc<RwLock<Categorizer>>,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl ActivityTracker {
    pub fn new(
        config: ProductivityConfig,
        repos: ProductivityRepos,
        categorizer: Categorizer,
    ) -> Self {
        Self {
            config,
            repos,
            categorizer: Arc::new(RwLock::new(categorizer)),
            cancel_token: CancellationToken::new(),
            task_handle: None,
        }
    }

    pub fn start(&mut self) {
        let cancel = self.cancel_token.clone();
        let poll_interval = std::time::Duration::from_secs(self.config.tracking.poll_interval_secs);
        let batch_interval = std::time::Duration::from_secs(self.config.tracking.batch_write_interval_secs);
        let idle_threshold = self.config.tracking.idle_threshold_secs as f64;
        let categorizer = Arc::clone(&self.categorizer);
        let privacy = self.config.privacy.clone();
        // Clone the repos for the spawned task
        // (repos hold SqlitePool which is Clone)
        let repos = ProductivityRepos::new(self.repos.events.pool().clone());

        let handle = tokio::spawn(async move {
            let mut buffer: Vec<ActivityEvent> = Vec::new();
            let mut last_app: Option<String> = None;
            let mut current_event: Option<ActivityEvent> = None;
            let mut last_flush = tokio::time::Instant::now();

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        // Flush remaining buffer
                        if !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("Failed to flush activity buffer on shutdown: {e}");
                            }
                        }
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        // Poll active window
                        match macos::get_frontmost_window() {
                            Ok(Some(info)) => {
                                // Check privacy exclusions
                                if is_excluded(&info, &privacy) {
                                    continue;
                                }

                                let idle_secs = macos::seconds_since_last_input();
                                let is_idle = idle_secs >= idle_threshold;
                                let now = Utc::now();

                                // Categorize
                                let cat = categorizer.read().await;
                                let category_id = cat.categorize(
                                    &info.app_name,
                                    info.bundle_id.as_deref(),
                                    None, // URL extraction from window title is future work
                                ).map(|c| c.id.clone());
                                drop(cat);

                                let same_app = last_app.as_deref() == Some(&info.app_name) && !is_idle;

                                if same_app {
                                    // Extend current event
                                    if let Some(ref mut evt) = current_event {
                                        evt.ended_at = Some(now);
                                        evt.duration_secs = Some(
                                            (now - evt.started_at).num_seconds()
                                        );
                                        // Update window title if changed
                                        evt.window_title = info.window_title;
                                    }
                                } else {
                                    // Finalize previous event and start new one
                                    if let Some(evt) = current_event.take() {
                                        buffer.push(evt);
                                    }
                                    current_event = Some(ActivityEvent {
                                        id: None,
                                        app_name: info.app_name.clone(),
                                        window_title: info.window_title,
                                        bundle_id: info.bundle_id,
                                        url: None,
                                        category_id,
                                        started_at: now,
                                        ended_at: Some(now),
                                        duration_secs: Some(0),
                                        is_idle,
                                        metadata: None,
                                    });
                                    last_app = Some(info.app_name);
                                }
                            }
                            Ok(None) => {
                                debug!("No frontmost window detected");
                            }
                            Err(e) => {
                                warn!("Failed to get window info: {e}");
                            }
                        }

                        // Batch write check
                        if last_flush.elapsed() >= batch_interval && !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("Failed to batch write activity events: {e}");
                            } else {
                                debug!("Flushed {} activity events", buffer.len());
                                buffer.clear();
                            }
                            last_flush = tokio::time::Instant::now();
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
        info!("Activity tracker started (poll: {}s, batch: {}s)",
            self.config.tracking.poll_interval_secs,
            self.config.tracking.batch_write_interval_secs);
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
        info!("Activity tracker stopped");
    }
}

fn is_excluded(info: &macos::WindowInfo, privacy: &PrivacyConfig) -> bool {
    if let Some(ref bid) = info.bundle_id {
        if privacy.excluded_apps.iter().any(|e| bid.eq_ignore_ascii_case(e)) {
            return true;
        }
    }
    if privacy.excluded_apps.iter().any(|e| info.app_name.eq_ignore_ascii_case(e)) {
        return true;
    }
    false
}
```

**Step 5: Build on macOS**

Run: `cargo build -p feature-productivity`
Expected: Compiles (with `todo!()` in `get_window_title_cg`)

**Step 6: Commit**

```bash
git add crates/feature-productivity/
git commit -m "feat(productivity): add macOS activity tracker with categorization"
```

---

### Task 5: Implement CGWindowList title extraction

**Files:**
- Modify: `crates/feature-productivity/src/tracker/macos.rs`

**Step 1: Implement `get_window_title_cg()`**

Replace the `todo!()` with actual CoreGraphics implementation using `CGWindowListCopyWindowInfo`. This requires:
- Get frontmost app's PID via `NSWorkspace`
- Call `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID)`
- Filter results for matching PID and `kCGWindowLayer == 0`
- Extract `kCGWindowName` from the first match

**Step 2: Test manually**

Run the tracker in debug mode, verify window titles are captured for common apps (VS Code, Safari, Terminal).

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/tracker/macos.rs
git commit -m "feat(productivity): implement CGWindowList title extraction"
```

---

## Phase 3: Focus Session Management

### Task 6: Focus session service

**Files:**
- Create: `crates/feature-productivity/src/focus.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Write tests for focus session lifecycle**

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_start_focus_session() { ... }

    #[tokio::test]
    async fn test_end_focus_session_calculates_quality() { ... }

    #[tokio::test]
    async fn test_cannot_start_two_sessions() { ... }

    #[tokio::test]
    async fn test_record_distraction() { ... }

    #[tokio::test]
    async fn test_quality_score_formula() { ... }
}
```

**Step 2: Implement FocusManager**

```rust
pub struct FocusManager {
    repos: ProductivityRepos,
    config: FocusConfig,
}

impl FocusManager {
    pub async fn start_session(&self, action_id: Option<String>, project_id: Option<String>, target_mins: Option<i64>) -> Result<FocusSession> { ... }
    pub async fn end_session(&self, notes: Option<String>) -> Result<Option<FocusSession>> { ... }
    pub async fn get_active(&self) -> Result<Option<FocusSession>> { ... }
    pub async fn record_distraction(&self, app_name: &str) -> Result<()> { ... }
    pub fn compute_quality(session: &FocusSession, on_task_ratio: f64) -> f64 { ... }
}
```

Quality score formula:
```
quality = (on_task_ratio * 0.6) + (focus_continuity * 0.3) + (completion_bonus * 0.1)
where:
  focus_continuity = 1.0 - (interruptions as f64 / max(1.0, expected_interruptions))
  expected_interruptions = target_mins / 15.0  (expect ~1 interruption per 15 min)
  completion_bonus = if completed { 1.0 } else if actual >= 0.8 * target { 0.5 } else { 0.0 }
```

**Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(focus)'`
Expected: All pass

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/focus.rs
git commit -m "feat(productivity): add focus session manager with quality scoring"
```

---

### Task 7: Daily summary aggregation

**Files:**
- Create: `crates/feature-productivity/src/aggregator.rs`

**Step 1: Write tests**

```rust
#[tokio::test]
async fn test_aggregate_daily_summary() {
    // Insert activity events for a day
    // Run aggregator
    // Verify summary matches expected totals
}

#[tokio::test]
async fn test_aggregate_includes_focus_sessions() { ... }

#[tokio::test]
async fn test_aggregate_top_apps() { ... }
```

**Step 2: Implement DailyAggregator**

```rust
pub struct DailyAggregator {
    repos: ProductivityRepos,
}

impl DailyAggregator {
    pub async fn compute_for_date(&self, date: &str) -> Result<DailySummary> {
        // Query activity_events for the date range
        // Sum durations by category_type (productive/neutral/distracting)
        // Count focus sessions and average quality
        // Count context switches
        // Get top apps and categories
        // Upsert into daily_summaries
    }

    pub async fn compute_today(&self) -> Result<DailySummary> { ... }
}
```

**Step 3: Run tests, commit**

```bash
git commit -m "feat(productivity): add daily summary aggregation"
```

---

## Phase 4: Agent Tools & Context Source

### Task 8: Create ProductivityTool

**Files:**
- Create: `crates/feature-productivity/src/tool/mod.rs`
- Create: `crates/feature-productivity/src/tool/actions/mod.rs`
- Create: `crates/feature-productivity/src/tool/actions/focus.rs`
- Create: `crates/feature-productivity/src/tool/actions/activity.rs`
- Create: `crates/feature-productivity/src/tool/actions/categories.rs`
- Modify: `crates/feature-productivity/src/lib.rs`

**Step 1: Create tool with actions**

Follow the `#[derive(Tool)]` + `#[tool_actions]` pattern from `crates/tools/src/filesystem.rs`:

```rust
#[derive(Tool)]
#[tool(
    name = "productivity",
    description = "Track productivity, manage focus sessions, and view activity data"
)]
#[tool_actions]
pub struct ProductivityTool {
    repos: ProductivityRepos,
    focus_manager: Arc<FocusManager>,
    aggregator: Arc<DailyAggregator>,
}
```

Actions:
- `focus_start` — params: `action_id?`, `project_id?`, `duration_mins?`
- `focus_end` — params: `notes?`
- `focus_status` — no params
- `activity_today` — no params
- `activity_summary` — params: `start_date`, `end_date`
- `activity_week` — no params
- `list_categories` — no params
- `set_category` — params: `id`, `name`, `category_type`, `app_names?`, `bundle_ids?`, `url_patterns?`

**Step 2: Update FeaturePackage to return the tool**

Update `ProductivityFeature` to hold and return the `ProductivityTool`.

**Step 3: Run tests, commit**

```bash
git commit -m "feat(productivity): add ProductivityTool with 8 actions"
```

---

### Task 9: Create ProductivityContextSource

**Files:**
- Create: `crates/agent/src/context_sources/productivity.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Implement the context source**

```rust
use async_trait::async_trait;
use context_engine::{ContextSource, SourceContext};
use feature_productivity::repos::ProductivityRepos;

pub struct ProductivityContextSource {
    repos: ProductivityRepos,
    // 60-second cache like LearningContextSource
    cache: tokio::sync::RwLock<Option<CachedProductivity>>,
}

struct CachedProductivity {
    content: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
impl ContextSource for ProductivityContextSource {
    fn name(&self) -> &str { "productivity" }
    fn priority(&self) -> u8 { 55 }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        // Check cache first
        // If expired or missing:
        //   1. Check for active focus session
        //   2. Get today's summary (or compute if missing)
        //   3. Format as context string
        // Cache for 60s
        // Return formatted string like:
        // "# Productivity Context\n\n## Current Focus\nFocusing on 'MailGate' (28min elapsed)...\n\n## Today\n3.5h productive, 1.2h neutral..."
    }
}
```

**Step 2: Register in builder.rs**

Add `ProductivityContextSource` to the `sources` vec in `AgentLoopBuilder::build()`, between existing sources.

**Step 3: Add dependency**

In `crates/agent/Cargo.toml`, add:
```toml
feature-productivity.workspace = true
```

**Step 4: Build and test**

Run: `cargo build -p agent`
Expected: Compiles

**Step 5: Commit**

```bash
git commit -m "feat(agent): add ProductivityContextSource (priority 55)"
```

---

### Task 10: Wire feature into AgentLoopBuilder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Add productivity tool registration**

Add a block after the finance tool registration:

```rust
// ── Productivity tool (requires real pool) ─────────────────────
if let Some(pool) = &self.pool {
    if config.productivity.enabled {
        let prod_repos = feature_productivity::repos::ProductivityRepos::new(pool.inner().clone());
        let focus_mgr = Arc::new(feature_productivity::FocusManager::new(
            prod_repos.clone(), config.productivity.focus.clone()
        ));
        let aggregator = Arc::new(feature_productivity::DailyAggregator::new(prod_repos.clone()));
        let productivity_tool = feature_productivity::ProductivityTool::new(
            prod_repos.clone(), focus_mgr, aggregator,
        );
        tool_registry.register(productivity_tool);
    }
}
```

**Step 2: Run feature migrations**

Ensure `ProductivityFeature::migrations()` are run during `StoragePool::connect()` or via the feature package migration runner.

**Step 3: Build**

Run: `cargo build --workspace`
Expected: Compiles

**Step 4: Commit**

```bash
git commit -m "feat(agent): wire feature-productivity into AgentLoopBuilder"
```

---

## Phase 5: Nudge Service

### Task 11: Create NudgeService background loop

**Files:**
- Create: `crates/feature-productivity/src/nudge.rs`

**Step 1: Write tests**

```rust
#[tokio::test]
async fn test_break_reminder_after_threshold() { ... }

#[tokio::test]
async fn test_nudge_cooldown_prevents_spam() { ... }

#[tokio::test]
async fn test_quiet_hours_suppresses_nudges() { ... }

#[tokio::test]
async fn test_burnout_detection() { ... }
```

**Step 2: Implement NudgeService**

```rust
pub struct NudgeService {
    repos: ProductivityRepos,
    config: NudgeConfig,
    focus_config: FocusConfig,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
    nudge_sender: mpsc::Sender<NudgeRecord>,
}

impl NudgeService {
    pub fn start(&mut self) {
        // 60-second check loop
        // Each tick:
        //   1. Check continuous active time → break reminder?
        //   2. Check daily totals → burnout alert?
        //   3. Check quiet hours → suppress if in range
        //   4. Check nudge_history cooldown → skip if too recent
        //   5. Send via nudge_sender channel
    }
}
```

The `nudge_sender` channel is consumed by whatever delivery mechanism the caller provides (Tauri notification, channel message, etc.).

**Step 3: Run tests, commit**

```bash
git commit -m "feat(productivity): add NudgeService with break/burnout detection"
```

---

## Phase 6: Tauri Integration

### Task 12: Add Tauri commands for productivity

**Files:**
- Create: `crates/desktop/src/commands/productivity.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs`
- Modify: `crates/desktop-shared/src/commands.rs`

**Step 1: Add DTO types to desktop-shared**

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivitySummaryResponse {
    pub total_active_secs: i64,
    pub total_focus_secs: i64,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub focus_sessions_count: i64,
    pub context_switches: i64,
    pub top_apps: Vec<AppUsageResponse>,
    pub top_categories: Vec<CategoryUsageResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSessionResponse { ... }

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTimelineResponse { ... }
```

**Step 2: Create Tauri commands**

```rust
#[tauri::command]
pub async fn productivity_today(state: State<'_, AppCore>) -> Result<ProductivitySummaryResponse, ApiError> { ... }

#[tauri::command]
pub async fn productivity_timeline(state: State<'_, AppCore>, date: String) -> Result<Vec<ActivityTimelineResponse>, ApiError> { ... }

#[tauri::command]
pub async fn productivity_focus_start(state: State<'_, AppCore>, action_id: Option<String>, target_mins: Option<i64>) -> Result<FocusSessionResponse, ApiError> { ... }

#[tauri::command]
pub async fn productivity_focus_end(state: State<'_, AppCore>) -> Result<Option<FocusSessionResponse>, ApiError> { ... }

#[tauri::command]
pub async fn productivity_focus_status(state: State<'_, AppCore>) -> Result<Option<FocusSessionResponse>, ApiError> { ... }

#[tauri::command]
pub async fn productivity_sessions(state: State<'_, AppCore>, date: String) -> Result<Vec<FocusSessionResponse>, ApiError> { ... }

#[tauri::command]
pub async fn productivity_weekly(state: State<'_, AppCore>) -> Result<Vec<ProductivitySummaryResponse>, ApiError> { ... }

#[tauri::command]
pub async fn productivity_categories(state: State<'_, AppCore>) -> Result<Vec<ActivityCategoryResponse>, ApiError> { ... }
```

**Step 3: Register commands in main.rs**

Add to `invoke_handler![]` macro.

**Step 4: Build**

Run: `cargo build -p desktop`
Expected: Compiles

**Step 5: Commit**

```bash
git commit -m "feat(desktop): add Tauri commands for productivity tracking"
```

---

### Task 13: Start activity tracker in Tauri app

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

**Step 1: Initialize and start ActivityTracker in AppCore**

Add `ActivityTracker` as a field on `AppCore`. Start it during app initialization. Stop it on app exit.

**Step 2: Request accessibility permission**

Add a Tauri command or startup check that prompts for macOS accessibility permission if not granted.

**Step 3: Manual test**

Run the desktop app, verify activity events are being written to the database.

**Step 4: Commit**

```bash
git commit -m "feat(desktop): start activity tracker on app launch"
```

---

## Phase 7: Desktop UI — Productivity View

### Task 14: Add Productivity route and sidebar item

**Files:**
- Create: `desktop-ui/src/components/views/Productivity.tsx`
- Modify: `desktop-ui/src/App.tsx`
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx`
- Modify: `desktop-ui/src/lib/types.ts`
- Modify: `desktop-ui/src/components/views/MainApp.tsx`

**Step 1: Add SidebarItem type**

In `types.ts`, add `'Productivity'` to the `SidebarItem` union.

**Step 2: Add sidebar entry**

In `Sidebar.tsx`, add:
```ts
{ key: 'Productivity', icon: Activity, path: '/productivity' },
```
(Use `Activity` or `Timer` from lucide-react)

**Step 3: Add route**

In `App.tsx`:
```ts
{ path: "/productivity", element: <Productivity /> },
```

**Step 4: Create initial Productivity view (skeleton)**

```tsx
export function Productivity() {
  return (
    <div className="flex flex-col h-full bg-background p-4 gap-4">
      <h1 className="text-lg font-semibold text-primary">Productivity</h1>
      {/* Components will be added in subsequent tasks */}
    </div>
  );
}
```

**Step 5: Wire navigation in MainApp.tsx**

Add the `if (item === 'Productivity') navigate('/productivity');` case.

**Step 6: Test**

Run: `cd desktop-ui && bun run dev`
Verify: Productivity appears in sidebar, clicking navigates to the view.

**Step 7: Commit**

```bash
git commit -m "feat(desktop-ui): add Productivity view route and sidebar item"
```

---

### Task 15: Build Today's Summary component

**Files:**
- Create: `desktop-ui/src/components/productivity/TodaySummary.tsx`
- Modify: `desktop-ui/src/components/views/Productivity.tsx`

**Step 1: Create TodaySummary component**

Displays: active time, focus time, break time, productive/neutral/distracting breakdown bars.

Uses `useQuery('productivity_today')` to fetch data.

**Step 2: Style with existing tokens**

- `bg-surface-base` for card background
- `text-success` for productive bar
- `text-muted` for neutral bar
- `text-destructive` for distracting bar
- `text-brand` for focus time highlight

**Step 3: Test visually**

Run dev server, verify summary renders with mock or real data.

**Step 4: Commit**

```bash
git commit -m "feat(desktop-ui): add TodaySummary productivity component"
```

---

### Task 16: Build Focus Status Card component

**Files:**
- Create: `desktop-ui/src/components/productivity/FocusStatusCard.tsx`
- Modify: `desktop-ui/src/components/views/Productivity.tsx`

**Step 1: Create FocusStatusCard**

- Shows active focus session with live countdown timer
- Progress bar (elapsed / target)
- Distraction count, quality indicator
- "End Focus" button
- If no active session: "Start Focus" button with task picker

Uses `useQuery('productivity_focus_status')` + `useEvent('entity:updated', refetch)`.

Live timer: `useEffect` with `setInterval(1000)` updating elapsed time.

**Step 2: Commit**

```bash
git commit -m "feat(desktop-ui): add FocusStatusCard component"
```

---

### Task 17: Build Timeline component

**Files:**
- Create: `desktop-ui/src/components/productivity/Timeline.tsx`
- Modify: `desktop-ui/src/components/views/Productivity.tsx`

**Step 1: Create Timeline**

Horizontal bar showing hour-by-hour app usage blocks, color-coded by category type:
- Green (`success`) = productive
- Gray (`text-muted`) = neutral
- Red (`destructive`) = distracting
- Dark (`surface-lowest`) = idle

Uses `useQuery('productivity_timeline', { date })`.

**Step 2: Commit**

```bash
git commit -m "feat(desktop-ui): add Timeline productivity component"
```

---

### Task 18: Build Top Apps and Weekly Trend components

**Files:**
- Create: `desktop-ui/src/components/productivity/TopApps.tsx`
- Create: `desktop-ui/src/components/productivity/WeeklyTrend.tsx`
- Create: `desktop-ui/src/components/productivity/FocusSessionsList.tsx`
- Modify: `desktop-ui/src/components/views/Productivity.tsx`

**Step 1: Create TopApps** — horizontal bar chart of most-used apps
**Step 2: Create WeeklyTrend** — table/mini chart of daily focus hours, context switches, quality
**Step 3: Create FocusSessionsList** — list of today's focus sessions with quality scores
**Step 4: Assemble full Productivity view layout**

```tsx
export function Productivity() {
  return (
    <div className="flex flex-col h-full bg-background p-4 gap-4 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-primary">Productivity</h1>
        <DatePicker />
      </div>
      <FocusStatusCard />
      <TodaySummary />
      <Timeline />
      <div className="grid grid-cols-2 gap-4">
        <FocusSessionsList />
        <TopApps />
      </div>
      <WeeklyTrend />
    </div>
  );
}
```

**Step 5: Commit**

```bash
git commit -m "feat(desktop-ui): add TopApps, WeeklyTrend, FocusSessionsList components"
```

---

## Phase 8: Learning System Integration

### Task 19: Extend PatternAnalyzer for productivity patterns

**Files:**
- Modify: `crates/agent/src/learning/pattern_analyzer.rs`

**Step 1: Add productivity pattern detection**

Add new pattern types to `PatternAnalyzer.analyze()`:
- `peak_focus_hours` — which hours of the day have the highest focus session quality
- `avg_session_length` — trending average focus session duration
- `productive_app_ratio` — ratio of productive to distracting time

These read from `focus_sessions` and `daily_summaries` tables.

**Step 2: Write to behavioral_patterns with type `productivity`**

**Step 3: Test**

```rust
#[tokio::test]
async fn test_productivity_patterns_detected() { ... }
```

**Step 4: Commit**

```bash
git commit -m "feat(learning): add productivity pattern detection to PatternAnalyzer"
```

---

### Task 20: Add productivity agent skill

**Files:**
- Create: `agents/task/skills/productivity.md`
- Modify: `agents/task/AGENT.md`

**Step 1: Create the productivity skill**

```markdown
---
name: productivity
description: Provide productivity insights, focus session management, and time analysis
trigger: on-demand
---

## When to activate
- User asks about focus, productivity, time tracking, or work patterns
- User wants to start/end a focus session
- User asks for daily/weekly reports
- User asks about their work habits or patterns

## Behavior
...
```

**Step 2: Add to task agent's skill list**

In `agents/task/AGENT.md`, add `productivity` to the skills list.

**Step 3: Commit**

```bash
git commit -m "feat(agents): add productivity skill to task agent"
```

---

## Phase 9: Feature Pack Registration

### Task 21: Add productivity feature pack

**Files:**
- Modify: `crates/cli/src/wizard/packs/registry.rs`

**Step 1: Add productivity pack to registry**

Follow the existing pattern for adding packs. Add `productivity` as a "Recommended" pack.

**Step 2: Test init wizard**

Run: `cargo run -- init --packs`
Verify: Productivity pack appears in the selection.

**Step 3: Commit**

```bash
git commit -m "feat(cli): add productivity feature pack to wizard"
```

---

## Phase 10: End-to-End Integration Testing

### Task 22: Integration tests

**Files:**
- Create: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Write integration tests**

- `test_full_focus_session_lifecycle` — start session, simulate activity events, end session, verify quality score and daily summary
- `test_categorizer_with_db` — load categories from DB, verify matching
- `test_daily_aggregation_accuracy` — insert known events, run aggregator, verify exact numbers
- `test_nudge_service_delivers_break_reminder` — simulate continuous work, verify nudge sent

**Step 2: Run all tests**

Run: `cargo nextest run -p feature-productivity`
Expected: All pass

**Step 3: Run workspace build**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 4: Commit**

```bash
git commit -m "test(productivity): add integration tests for full feature lifecycle"
```

---

## Summary: File Creation/Modification Map

### New files (create):
```
crates/feature-productivity/Cargo.toml
crates/feature-productivity/migrations/001_productivity_tables.sql
crates/feature-productivity/src/lib.rs
crates/feature-productivity/src/config.rs
crates/feature-productivity/src/types.rs
crates/feature-productivity/src/repos/mod.rs
crates/feature-productivity/src/repos/activity_event.rs
crates/feature-productivity/src/repos/activity_category.rs
crates/feature-productivity/src/repos/focus_session.rs
crates/feature-productivity/src/repos/daily_summary.rs
crates/feature-productivity/src/repos/nudge.rs
crates/feature-productivity/src/tracker/mod.rs
crates/feature-productivity/src/tracker/macos.rs
crates/feature-productivity/src/tracker/categorizer.rs
crates/feature-productivity/src/focus.rs
crates/feature-productivity/src/aggregator.rs
crates/feature-productivity/src/nudge.rs
crates/feature-productivity/src/tool/mod.rs
crates/feature-productivity/src/tool/actions/mod.rs
crates/feature-productivity/src/tool/actions/focus.rs
crates/feature-productivity/src/tool/actions/activity.rs
crates/feature-productivity/src/tool/actions/categories.rs
crates/feature-productivity/tests/repos_test.rs
crates/feature-productivity/tests/integration_test.rs
crates/config/src/schema/productivity.rs
crates/agent/src/context_sources/productivity.rs
crates/desktop/src/commands/productivity.rs
crates/desktop-shared/src/commands.rs (DTOs — modify)
desktop-ui/src/components/views/Productivity.tsx
desktop-ui/src/components/productivity/TodaySummary.tsx
desktop-ui/src/components/productivity/FocusStatusCard.tsx
desktop-ui/src/components/productivity/Timeline.tsx
desktop-ui/src/components/productivity/TopApps.tsx
desktop-ui/src/components/productivity/WeeklyTrend.tsx
desktop-ui/src/components/productivity/FocusSessionsList.tsx
agents/task/skills/productivity.md
```

### Existing files to modify:
```
Cargo.toml (workspace root)
crates/agent/Cargo.toml
crates/agent/src/agent_loop/builder.rs
crates/agent/src/context_sources/mod.rs
crates/agent/src/learning/pattern_analyzer.rs
crates/config/src/schema/mod.rs
crates/config/src/schema/core.rs
crates/desktop/src/commands/mod.rs
crates/desktop/src/main.rs
crates/desktop/src/app_core.rs
crates/desktop-shared/src/commands.rs
desktop-ui/src/App.tsx
desktop-ui/src/components/layout/Sidebar.tsx
desktop-ui/src/components/views/MainApp.tsx
desktop-ui/src/lib/types.ts
agents/task/AGENT.md
crates/cli/src/wizard/packs/registry.rs
```
