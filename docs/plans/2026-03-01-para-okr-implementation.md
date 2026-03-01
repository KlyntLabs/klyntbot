# PARA + OKR Task System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the flat tasks/projects/goals/plans system with a PARA-method hierarchy (Areas → Projects → Objectives → Key Results → Actions) with OKR tracking.

**Architecture:** Clean-slate rewrite of domain, storage, and tools layers. Remove Goal and Plan systems entirely. Add Area, Objective, KeyResult as first-class entities. Rename Todo → Action. Keep all existing task features (focus, time tracking, recurrence, search, dependencies, attachments, enrichment). 4 tools: AreaTool, ProjectTool, OkrTool, TaskTool.

**Tech Stack:** Rust, SQLite (sqlx), async-trait, serde, chrono, uuid

**Design doc:** `docs/plans/2026-03-01-para-okr-redesign.md`

---

## Task 1: Domain Layer — New Types + Remove Old

**Files:**
- Create: `crates/domain/src/area.rs`
- Create: `crates/domain/src/objective.rs`
- Create: `crates/domain/src/key_result.rs`
- Create: `crates/domain/src/project.rs`
- Delete: `crates/domain/src/goal.rs`
- Delete: `crates/domain/src/plan.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/domain/Cargo.toml`

### Step 1: Create `crates/domain/src/area.rs`

```rust
//! Area domain type — top-level PARA container (Work, Personal, etc.).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A PARA area — the highest-level organizational container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: AreaColor,
    pub icon: Option<String>,
    pub position: i32,
    pub status: AreaStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Area {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AreaStatus {
    Active,
    Archived,
}

impl AreaStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl std::fmt::Display for AreaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AreaColor {
    Blue,
    Green,
    Purple,
    Orange,
    Red,
    Yellow,
    Gray,
}

impl Default for AreaColor {
    fn default() -> Self {
        Self::Blue
    }
}

impl AreaColor {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "blue" => Some(Self::Blue),
            "green" => Some(Self::Green),
            "purple" => Some(Self::Purple),
            "orange" => Some(Self::Orange),
            "red" => Some(Self::Red),
            "yellow" => Some(Self::Yellow),
            "gray" | "grey" => Some(Self::Gray),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Purple => "purple",
            Self::Orange => "orange",
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Gray => "gray",
        }
    }
}

/// Partial update for an area.
pub struct AreaPatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<AreaColor>,
    pub icon: Option<Option<String>>,
    pub position: Option<i32>,
    pub status: Option<AreaStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_area_id_generation() {
        let id = Area::generate_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_area_status_roundtrip() {
        assert_eq!(AreaStatus::from_str_loose("active"), Some(AreaStatus::Active));
        assert_eq!(AreaStatus::from_str_loose("ARCHIVED"), Some(AreaStatus::Archived));
        assert_eq!(AreaStatus::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_area_color_roundtrip() {
        for color in [AreaColor::Blue, AreaColor::Green, AreaColor::Purple, AreaColor::Orange, AreaColor::Red, AreaColor::Yellow, AreaColor::Gray] {
            let s = color.as_str();
            assert_eq!(AreaColor::from_str_loose(s), Some(color));
        }
    }
}
```

### Step 2: Create `crates/domain/src/objective.rs`

```rust
//! Objective domain type — OKR objective within a project.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: ObjectiveStatus,
    pub priority: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    pub progress: f64, // 0.0-100.0, aggregated from KRs
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Objective {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ObjectiveStatus {
    Active,
    Paused,
    Completed,
    Abandoned,
}

impl ObjectiveStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }
}

impl std::fmt::Display for ObjectiveStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct ObjectivePatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<ObjectiveStatus>,
    pub priority: Option<Option<u8>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_objective_status_roundtrip() {
        for status in [ObjectiveStatus::Active, ObjectiveStatus::Paused, ObjectiveStatus::Completed, ObjectiveStatus::Abandoned] {
            let s = status.as_str();
            assert_eq!(ObjectiveStatus::from_str_loose(s), Some(status));
        }
    }

    #[test]
    fn test_terminal_states() {
        assert!(!ObjectiveStatus::Active.is_terminal());
        assert!(!ObjectiveStatus::Paused.is_terminal());
        assert!(ObjectiveStatus::Completed.is_terminal());
        assert!(ObjectiveStatus::Abandoned.is_terminal());
    }
}
```

### Step 3: Create `crates/domain/src/key_result.rs`

```rust
//! KeyResult domain type — measurable result within an objective.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyResult {
    pub id: String,
    pub objective_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: KeyResultStatus,
    pub tracking_mode: TrackingMode,
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub progress: f64, // 0.0-100.0
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl KeyResult {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    /// Recalculate progress based on tracking mode.
    /// For metric mode: current_value / target_value * 100 (clamped 0-100).
    /// For action mode: caller must provide completed/total counts.
    pub fn recalculate_metric_progress(&mut self) {
        if self.tracking_mode == TrackingMode::Metric {
            self.progress = match self.target_value {
                Some(target) if target > 0.0 => {
                    (self.current_value / target * 100.0).clamp(0.0, 100.0)
                }
                _ => 0.0,
            };
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrackingMode {
    Metric,
    Action,
}

impl TrackingMode {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "metric" => Some(Self::Metric),
            "action" => Some(Self::Action),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Action => "action",
        }
    }
}

impl Default for TrackingMode {
    fn default() -> Self {
        Self::Action
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyResultStatus {
    Active,
    Completed,
    Abandoned,
}

impl KeyResultStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }
}

impl std::fmt::Display for KeyResultStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct KeyResultPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<KeyResultStatus>,
    pub tracking_mode: Option<TrackingMode>,
    pub target_value: Option<Option<f64>>,
    pub current_value: Option<f64>,
    pub unit: Option<Option<String>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_progress_calculation() {
        let mut kr = KeyResult {
            id: "test1234".into(),
            objective_id: "obj12345".into(),
            title: "Test KR".into(),
            description: None,
            status: KeyResultStatus::Active,
            tracking_mode: TrackingMode::Metric,
            target_value: Some(100.0),
            current_value: 60.0,
            unit: Some("%".into()),
            progress: 0.0,
            due_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
        };
        kr.recalculate_metric_progress();
        assert!((kr.progress - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metric_progress_clamped() {
        let mut kr = KeyResult {
            id: "test1234".into(),
            objective_id: "obj12345".into(),
            title: "Test".into(),
            description: None,
            status: KeyResultStatus::Active,
            tracking_mode: TrackingMode::Metric,
            target_value: Some(50.0),
            current_value: 100.0,
            unit: None,
            progress: 0.0,
            due_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
        };
        kr.recalculate_metric_progress();
        assert!((kr.progress - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tracking_mode_roundtrip() {
        assert_eq!(TrackingMode::from_str_loose("metric"), Some(TrackingMode::Metric));
        assert_eq!(TrackingMode::from_str_loose("action"), Some(TrackingMode::Action));
        assert_eq!(TrackingMode::from_str_loose("unknown"), None);
    }
}
```

### Step 4: Create `crates/domain/src/project.rs`

Move project types from `crates/tools/src/project_types.rs` to domain, adding `area_id`.

```rust
//! Project domain type — belongs to an Area, contains OKR objectives.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub area_id: String, // NEW: required FK to areas
    pub name: String,
    pub description: Option<String>,
    pub color: ProjectColor,
    pub tags: Vec<String>,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

impl ProjectStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

impl Default for ProjectColor {
    fn default() -> Self {
        Self::Orange
    }
}

impl ProjectColor {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "red" => Some(Self::Red),
            "orange" => Some(Self::Orange),
            "yellow" => Some(Self::Yellow),
            "green" => Some(Self::Green),
            "blue" => Some(Self::Blue),
            "purple" => Some(Self::Purple),
            "gray" | "grey" => Some(Self::Gray),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Gray => "gray",
        }
    }
}

pub struct ProjectPatch {
    pub name: Option<String>,
    pub area_id: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<ProjectColor>,
    pub tags: Option<Vec<String>>,
    pub status: Option<ProjectStatus>,
}

pub struct ProjectFilter {
    pub area_id: Option<String>,
    pub status: Option<ProjectStatus>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
}
```

### Step 5: Update `crates/domain/src/lib.rs`

```rust
//! Domain crate — PARA + OKR types for klyntbot.

pub mod area;
pub mod key_result;
pub mod objective;
pub mod project;

pub use area::{Area, AreaColor, AreaPatch, AreaStatus};
pub use key_result::{KeyResult, KeyResultPatch, KeyResultStatus, TrackingMode};
pub use objective::{Objective, ObjectivePatch, ObjectiveStatus};
pub use project::{Project, ProjectColor, ProjectFilter, ProjectPatch, ProjectStatus};
```

### Step 6: Update `crates/domain/Cargo.toml`

Remove `storage` dependency (domain no longer has SQL conversion helpers — those move to repos).

```toml
[package]
name = "domain"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
thiserror.workspace = true
uuid.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
```

### Step 7: Delete old domain files

```bash
rm crates/domain/src/goal.rs crates/domain/src/plan.rs
```

### Step 8: Run tests

```bash
cargo nextest run -p domain
```

Expected: All new unit tests pass. Old goal/plan tests are gone.

### Step 9: Commit

```bash
git add crates/domain/
git commit -m "refactor(domain): replace Goal/Plan with PARA+OKR types (Area, Objective, KeyResult, Project)"
```

---

## Task 2: Storage Layer — Schema + Row Types

**Files:**
- Rewrite: `crates/storage/migrations/001_initial.sql`
- Delete: `crates/storage/migrations/002_learning_loop.sql`
- Delete: `crates/storage/migrations/003_strategy_tool_columns.sql`
- Delete: `crates/storage/migrations/004_intent_pipeline.sql`
- Delete: `crates/storage/migrations/005_agent_tasks.sql`
- Create: `crates/storage/src/rows/area.rs`
- Create: `crates/storage/src/rows/objective.rs`
- Create: `crates/storage/src/rows/key_result.rs`
- Modify: `crates/storage/src/rows/project.rs` (add area_id)
- Rename: `crates/storage/src/rows/todo.rs` → `action.rs` (add area_id, key_result_id)
- Delete: `crates/storage/src/rows/goal.rs`
- Delete: `crates/storage/src/rows/plan.rs`
- Modify: `crates/storage/src/rows/mod.rs`

### Step 1: Rewrite `crates/storage/migrations/001_initial.sql`

Consolidate all migrations into a single file. The new schema includes:
- **New tables:** `areas`, `objectives`, `key_results`, `actions` (replaces `todos`), `action_attachments`, `action_time_entries`, `action_dependencies`, `resources`, `archive_items`
- **Updated tables:** `projects` (add `area_id NOT NULL REFERENCES areas`)
- **Removed tables:** `goals`, `goal_project_links`, `plans`, `plan_steps`
- **Kept as-is:** `sessions`, `session_messages`, `learning_outcomes`, `strategy_records`, `enrichment_feedback`, `usage_records`, `cron_jobs`, `calendar_sync_state`, `calendar_event_cache`, `memory_notes`, `learning_state`, `decision_log`, all finance tables, `_feature_migrations`, `agent_tasks`, `tool_usage`

Incorporate columns from migrations 002-005 that are still relevant:
- From 003: `tool_name`, `tool_success`, `tool_duration_ms` on `strategy_records`
- From 004: `complexity_signals`, `execution_mode` on `strategy_records`
- From 005: `agent_tasks` and `tool_usage` tables

Full SQL is in the design doc schema section. Write the complete file with all tables.

### Step 2: Delete old migration files

```bash
rm crates/storage/migrations/002_learning_loop.sql
rm crates/storage/migrations/003_strategy_tool_columns.sql
rm crates/storage/migrations/004_intent_pipeline.sql
rm crates/storage/migrations/005_agent_tasks.sql
```

### Step 3: Create `crates/storage/src/rows/area.rs`

```rust
//! Row struct for the `areas` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub position: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Step 4: Create `crates/storage/src/rows/objective.rs`

```rust
//! Row struct for the `objectives` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveRow {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<i16>,
    pub due_date: Option<DateTime<Utc>>,
    pub progress: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

### Step 5: Create `crates/storage/src/rows/key_result.rs`

```rust
//! Row struct for the `key_results` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultRow {
    pub id: String,
    pub objective_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub tracking_mode: String,
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub progress: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

### Step 6: Update `crates/storage/src/rows/project.rs`

Add `area_id` field:

```rust
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub area_id: String, // NEW
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Step 7: Rename `todo.rs` → `action.rs`, add `area_id` + `key_result_id`

Rename file and update struct names. Keep `TodoAttachmentRow` → `ActionAttachmentRow`, etc. Add `area_id: String` and `key_result_id: Option<String>` to the main row struct. Update supporting row structs with `action_id` field names.

```rust
//! Row structs for the `actions`, `action_attachments`, `action_time_entries`,
//! and `action_dependencies` tables.

// ActionRow: add area_id (String, NOT NULL) and key_result_id (Option<String>)
// ActionAttachmentRow: rename todo_id → action_id
// ActionTimeEntryRow: rename todo_id → action_id
// ActionDependencyRow: keep as-is (action_id, blocker_id)
```

### Step 8: Delete old row files + update mod.rs

```bash
rm crates/storage/src/rows/goal.rs crates/storage/src/rows/plan.rs
```

Update `crates/storage/src/rows/mod.rs`:
- Remove: `pub mod goal;`, `pub mod plan;`
- Rename: `pub mod todo;` → `pub mod action;`
- Add: `pub mod area;`, `pub mod objective;`, `pub mod key_result;`

### Step 9: Update `crates/storage/src/lib.rs`

Remove goal/plan row re-exports, add area/objective/key_result row re-exports. Rename todo row re-exports to action.

### Step 10: Commit

```bash
git add crates/storage/migrations/ crates/storage/src/rows/
git commit -m "refactor(storage): new PARA+OKR schema, row types, remove goal/plan"
```

---

## Task 3: Storage Layer — Repositories

**Files:**
- Create: `crates/storage/src/repos/area.rs`
- Create: `crates/storage/src/repos/objective.rs`
- Create: `crates/storage/src/repos/key_result.rs`
- Modify: `crates/storage/src/repos/project_repo.rs` (add area_id, use domain types)
- Rename: `crates/storage/src/repos/todo_repo.rs` → `action_repo.rs` (add area_id, key_result_id)
- Delete: `crates/storage/src/repos/goal.rs`
- Delete: `crates/storage/src/repos/plan.rs`
- Modify: `crates/storage/src/repos/mod.rs` (update Repos aggregate)
- Modify: `crates/storage/src/lib.rs` (update re-exports)

### Step 1: Create `crates/storage/src/repos/area.rs`

AreaRepo with methods: `create`, `get`, `get_or_err`, `list` (with optional status filter), `update`, `delete`, `reorder` (update position).

Pattern: follow existing `ProjectRepo` structure. Use `sqlx::query_as` with explicit column lists. Include unit tests using `StoragePool::connect_in_memory()`.

### Step 2: Write failing tests for AreaRepo

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (AreaRepo, crate::StoragePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = AreaRepo::new(pool.inner().clone());
        (repo, pool)
    }

    #[tokio::test]
    async fn test_create_and_get_area() { ... }

    #[tokio::test]
    async fn test_list_areas_filters_by_status() { ... }

    #[tokio::test]
    async fn test_update_area() { ... }

    #[tokio::test]
    async fn test_delete_area() { ... }

    #[tokio::test]
    async fn test_reorder() { ... }
}
```

### Step 3: Implement AreaRepo to pass tests

Run: `cargo nextest run -p storage -E 'test(area)'`

### Step 4: Create ObjectiveRepo + tests

ObjectiveRepo with methods: `create`, `get`, `get_or_err`, `list` (project_id?, status?), `update`, `delete`, `recalculate_progress` (avg of child KR progresses).

### Step 5: Create KeyResultRepo + tests

KeyResultRepo with methods: `create`, `get`, `get_or_err`, `list` (objective_id?), `update`, `delete`, `update_metric` (set current_value, recalculate progress), `recalculate_action_progress` (count completed/total child actions), `count_actions` (total + completed for a KR).

### Step 6: Update ProjectRepo

- Add `area_id` to all INSERT/SELECT queries
- Update `ProjectFilter` to include `area_id: Option<String>`
- Update `ProjectPatch` to include `area_id: Option<String>`
- Move filter/patch types to use domain types or keep local (match existing pattern)
- Fix tests

### Step 7: Rename todo_repo → action_repo

- Rename file: `todo_repo.rs` → `action_repo.rs`
- Rename struct: `TodoRepo` → `ActionRepo`
- Rename related types: `TodoFilter` → `ActionFilter`, `TodoPatch` → `ActionPatch`, `TodoSummary` → `ActionSummary`
- Add `area_id: String` (NOT NULL) to all INSERT queries
- Add `key_result_id: Option<String>` to INSERT/UPDATE queries
- Add `area_id` and `key_result_id` filter options to `ActionFilter`
- Add `unassigned: bool` filter (WHERE project_id IS NULL)
- Update all SQL: `todos` → `actions`, `todo_attachments` → `action_attachments`, etc.
- Fix all tests

### Step 8: Delete old repos + update Repos aggregate

```bash
rm crates/storage/src/repos/goal.rs crates/storage/src/repos/plan.rs
```

Update `crates/storage/src/repos/mod.rs`:
- Remove: `pub mod goal;`, `pub mod plan;`
- Add: `pub mod area;`, `pub mod objective;`, `pub mod key_result;`
- Rename: `pub mod todo_repo;` → `pub mod action_repo;`
- Update `Repos` struct: remove `goals: GoalRepo`, `plans: PlanRepo`, rename `todos: TodoRepo` → `actions: ActionRepo`, add `areas: AreaRepo`, `objectives: ObjectiveRepo`, `key_results: KeyResultRepo`
- Update `Repos::from_pool()` constructor

### Step 9: Update `crates/storage/src/lib.rs`

Remove goal/plan repo re-exports, rename todo → action, add area/objective/key_result.

### Step 10: Run all storage tests

```bash
cargo nextest run -p storage
```

### Step 11: Commit

```bash
git add crates/storage/
git commit -m "refactor(storage): PARA+OKR repos (Area, Objective, KeyResult, Action)"
```

---

## Task 4: Tools Layer — Remove Old, Add New

**Files:**
- Delete: `crates/tools/src/goal_tool.rs`
- Delete: `crates/tools/src/plan_tool.rs`
- Delete: `crates/tools/src/plan_response.rs`
- Delete: `crates/tools/src/project_types.rs`
- Create: `crates/tools/src/area_tool.rs`
- Create: `crates/tools/src/okr_tool.rs`
- Create: `crates/tools/src/progress_handler.rs`
- Modify: `crates/tools/src/project_tool.rs` (require area_id, use domain types)
- Modify: `crates/tools/src/todo_types.rs` (rename re-exports)
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/Cargo.toml` (add domain dependency)

### Step 1: Delete old tool files

```bash
rm crates/tools/src/goal_tool.rs crates/tools/src/plan_tool.rs crates/tools/src/plan_response.rs crates/tools/src/project_types.rs
```

### Step 2: Create ProgressHandler trait

`crates/tools/src/progress_handler.rs`:

```rust
//! ProgressHandler — dependency inversion for OKR progress recalculation.
//!
//! Defined here (Layer 4), implemented in agent (Layer 5).
//! Called when an action is completed to cascade progress updates
//! through KeyResult → Objective.

use async_trait::async_trait;
use common::Result;

#[async_trait]
pub trait ProgressHandler: Send + Sync {
    /// Recalculate progress for a key result and its parent objective.
    /// Called when an action linked to this KR is completed or uncompleted.
    async fn recalculate_kr_progress(&self, key_result_id: &str) -> Result<()>;
}
```

### Step 3: Create AreaTool

`crates/tools/src/area_tool.rs` — 5 actions: `create`, `list`, `show`, `update`, `reorder`.

Uses `AreaRepo` directly (no handler trait needed — areas are simple CRUD). Follow the existing `ProjectTool` pattern with `#[derive(Tool)]` or manual `Tool` impl.

Include tests with mock repos or in-memory DB.

### Step 4: Create OkrTool

`crates/tools/src/okr_tool.rs` — 14 actions with dotted namespace:
- `objective.create`, `objective.list`, `objective.show`, `objective.update`, `objective.delete`, `objective.progress`
- `kr.create`, `kr.list`, `kr.show`, `kr.update`, `kr.update_metric`, `kr.add_action`, `kr.remove_action`, `kr.delete`

Uses `ObjectiveRepo`, `KeyResultRepo`, `ActionRepo` directly. The `kr.update_metric` action calls `ProgressHandler` to cascade recalculation.

### Step 5: Update ProjectTool

- Import `Project`, `ProjectColor`, `ProjectStatus` from `domain` instead of `project_types`
- Add `area_id` as required param in `create` action
- Add `area_id` as optional param in `update` action (move project between areas)
- Add `area_id` filter to `list` action
- Update `show` to display area name + objective count
- Add `objectives` action: list objectives for a project with KR summaries
- Add `stats` action: full OKR + action breakdown

### Step 6: Update `todo_types.rs`

This file re-exports types from feature-todo. Update re-exports to use new names (Action, ActionStatus, etc.) once feature-todo is updated. For now, keep as alias bridge.

### Step 7: Update `crates/tools/src/lib.rs`

- Remove: `goal_tool`, `plan_tool`, `plan_response`, `project_types` modules
- Add: `area_tool`, `okr_tool`, `progress_handler` modules
- Update re-exports: remove GoalHandler, GoalTool, PlanHandler, PlanTool, PlanCompletionHandler
- Add re-exports: AreaTool, OkrTool, ProgressHandler

### Step 8: Update `crates/tools/Cargo.toml`

Add `domain.workspace = true` dependency (needed for Area/Project/Objective/KeyResult types).

### Step 9: Run tools tests

```bash
cargo nextest run -p tools
```

### Step 10: Commit

```bash
git add crates/tools/
git commit -m "refactor(tools): replace Goal/Plan tools with Area, OKR, ProgressHandler"
```

---

## Task 5: Feature-Todo — Rename Todo → Action

**Files:**
- Modify: `crates/feature-todo/src/types.rs` (Todo → Action, TodoStatus → ActionStatus)
- Modify: `crates/feature-todo/src/tool/mod.rs` (TodoTool → TaskTool, add area_id/key_result_id)
- Modify: `crates/feature-todo/src/tool/actions/*.rs` (update all action handlers)
- Modify: `crates/feature-todo/src/lib.rs` (update exports, FeaturePackage)
- Modify: `crates/feature-todo/src/enrichment.rs` (type renames)
- Modify: `crates/feature-todo/src/search.rs` (type renames)
- Modify: `crates/feature-todo/src/embedding.rs` (type renames)
- Modify: `crates/feature-todo/src/task_complexity.rs` (update refs)
- Modify: `crates/feature-todo/src/config.rs` (keep as-is, config key stays "todo")
- Modify: `crates/feature-todo/migrations/001_create_todos.sql` (update table names)
- Remove: `handle_plan` and `handle_execute` actions (Plan system removed)

### Step 1: Rename types in `types.rs`

- `Todo` → `Action`
- `TodoStatus` → `ActionStatus`
- `TodoRow` references → `ActionRow`
- `TodoAttachmentRow` → `ActionAttachmentRow`
- `TodoTimeEntryRow` → `ActionTimeEntryRow`
- Add `area_id: String` field to `Action` struct
- Add `key_result_id: Option<String>` field to `Action` struct
- Update `From<ActionRow> for Action` and `From<&Action> for ActionRow` conversions

### Step 2: Update tool/mod.rs

- `TodoTool` → `TaskTool`
- Tool `name()` returns `"task"` (was `"todo"`)
- Update `execute()` dispatch: remove `"plan"` and `"execute"` actions
- Add `area_id` param to `add` action (required)
- Add `key_result_id` param to `add` action (optional)
- Add `area_id`, `key_result_id`, `unassigned` params to `list` action
- Update `complete` action to call `ProgressHandler` when action has a `key_result_id`

### Step 3: Update action handlers

- `actions/add.rs`: require `area_id`, accept `key_result_id`
- `actions/update.rs`: accept `area_id`, `key_result_id` in updates; `handle_complete` calls ProgressHandler
- `actions/list.rs`: add `area_id`, `key_result_id`, `unassigned` filters
- `actions/search.rs`: update type references
- `actions/execute.rs`: remove `handle_plan` and `handle_execute`; keep focus/attach/enrich/recur

### Step 4: Update enrichment.rs, search.rs, embedding.rs, task_complexity.rs

Type renames only. `Todo` → `Action`, `TodoStatus` → `ActionStatus`.

### Step 5: Update feature migration

`crates/feature-todo/migrations/001_create_todos.sql` — update table names:
- `todos` → `actions`
- `todo_attachments` → `action_attachments`
- `todo_time_entries` → `action_time_entries`
- `todo_dependencies` → `action_dependencies`
- Add `area_id TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE`
- Add `key_result_id TEXT REFERENCES key_results(id) ON DELETE SET NULL`

Note: Since the core migration (Task 2) already creates these tables, the feature migration should use `CREATE TABLE IF NOT EXISTS` to be idempotent.

### Step 6: Update lib.rs exports

Rename all public type exports. Keep crate name as `feature-todo` (internal).

### Step 7: Run tests

```bash
cargo nextest run -p feature-todo
```

### Step 8: Commit

```bash
git add crates/feature-todo/
git commit -m "refactor(feature-todo): rename Todo→Action, add area_id/key_result_id, remove plan actions"
```

---

## Task 6: Agent Layer — Remove Plan/Goal, Update Pipeline

**Files:**
- Delete: `crates/agent/src/goal_handler.rs`
- Delete: `crates/agent/src/plan_handler.rs`
- Delete: `crates/agent/src/plan_executor.rs`
- Delete: `crates/agent/src/plan_step_generator.rs`
- Delete: `crates/agent/src/intent_pipeline/engines/planned.rs`
- Delete: `crates/agent/src/intent_pipeline/visibility.rs`
- Delete: `crates/agent/src/context_sources/goal.rs`
- Create: `crates/agent/src/progress_handler.rs` (ProgressHandler impl)
- Modify: `crates/agent/src/lib.rs` (remove modules, update exports)
- Modify: `crates/agent/src/intent_pipeline/types.rs` (remove Planned mode, update ToolGroup)
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs` (remove planned module)
- Modify: `crates/agent/src/intent_pipeline/router.rs` (remove PlannedEngine, simplify escalation)
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs` (remove plan cleanup)
- Modify: `crates/agent/src/intent_pipeline/analysis.rs` (remove plan classification)
- Modify: `crates/agent/src/intent_pipeline/mod.rs` (remove visibility module)
- Modify: `crates/agent/src/context_sources/mod.rs` (remove GoalSource)
- Modify: `crates/agent/src/agent_loop/builder.rs` (update tool registration)

### Step 1: Delete old files

```bash
rm crates/agent/src/goal_handler.rs
rm crates/agent/src/plan_handler.rs
rm crates/agent/src/plan_executor.rs
rm crates/agent/src/plan_step_generator.rs
rm crates/agent/src/intent_pipeline/engines/planned.rs
rm crates/agent/src/intent_pipeline/visibility.rs
rm crates/agent/src/context_sources/goal.rs
```

### Step 2: Create ProgressHandler impl

`crates/agent/src/progress_handler.rs`:

```rust
//! ProgressHandlerImpl — cascades action completion through KR → Objective progress.

use async_trait::async_trait;
use common::Result;
use storage::{KeyResultRepo, ObjectiveRepo, ActionRepo};
use tools::progress_handler::ProgressHandler;

pub struct ProgressHandlerImpl {
    kr_repo: KeyResultRepo,
    objective_repo: ObjectiveRepo,
    action_repo: ActionRepo,
}

impl ProgressHandlerImpl {
    pub fn new(kr_repo: KeyResultRepo, objective_repo: ObjectiveRepo, action_repo: ActionRepo) -> Self {
        Self { kr_repo, objective_repo, action_repo }
    }
}

#[async_trait]
impl ProgressHandler for ProgressHandlerImpl {
    async fn recalculate_kr_progress(&self, key_result_id: &str) -> Result<()> {
        // 1. Get the KR
        let kr = self.kr_repo.get(key_result_id).await?;
        if let Some(kr) = kr {
            if kr.tracking_mode == "action" {
                // 2. Count completed/total actions for this KR
                let (completed, total) = self.action_repo.count_by_kr(key_result_id).await?;
                let progress = if total > 0 { completed as f64 / total as f64 * 100.0 } else { 0.0 };
                // 3. Update KR progress
                self.kr_repo.update_progress(key_result_id, progress).await?;
            }
            // 4. Recalculate parent objective progress (avg of all KR progresses)
            self.objective_repo.recalculate_progress(&kr.objective_id).await?;
        }
        Ok(())
    }
}
```

### Step 3: Update `intent_pipeline/types.rs`

- Remove `ExecutionMode::Planned` variant
- Remove `use domain::PlanVisibility;`
- Update `short_name()`, `max_iterations()`, `Display` impl
- Remove `Planned` → `AutonomousTask` mapping in `From<&ExecutionMode> for ExecutionStrategy`
- Update `ToolGroup::TaskManagement` tool names: `["todo", "goal", "plan"]` → `["task", "area", "project", "okr"]`
- Update `ToolGroup::Calendar` from `["calendar", "todo"]` → `["calendar", "task"]`
- Fix all tests

### Step 4: Update `intent_pipeline/engines/mod.rs`

- Remove `pub mod planned;`

### Step 5: Update `intent_pipeline/router.rs`

- Remove `use super::engines::planned::PlannedEngine;`
- Remove `planned: Option<PlannedEngine>` from `ExecutionRouter`
- Remove Planned mode matching in `route()`
- Simplify escalation: Reactive no longer escalates to Planned
- When Reactive escalates, return the result (no further escalation)

### Step 6: Update `intent_pipeline/pipeline.rs`

- Remove `PlanCleanupService` spawning
- Remove any plan-related cleanup in pipeline initialization

### Step 7: Update `intent_pipeline/analysis.rs`

- Remove any heuristics that classify as `Planned`
- Simplify LLM classifier prompt (no "planned" option)
- Cap classification at `Reactive`

### Step 8: Update `intent_pipeline/mod.rs`

- Remove `pub mod visibility;`

### Step 9: Update `context_sources/mod.rs`

- Remove `pub mod goal;` and `pub use goal::GoalSource;`
- Remove GoalSource from context source registration

### Step 10: Update `agent_loop/builder.rs`

This is critical — update tool registration:

- Remove GoalTool + GoalHandlerImpl construction (lines ~409-416)
- Remove PlanTool + PlanHandlerImpl construction (lines ~419-425)
- Remove PlannedEngine construction (lines ~670+)
- Remove PlanCleanupService spawning (lines ~765+)
- Add AreaTool registration (from repos.areas)
- Add OkrTool registration (from repos.objectives, repos.key_results, repos.actions)
- Update ProjectTool construction (pass repos.areas for area lookups)
- Update TodoTool → TaskTool construction (pass ProgressHandler)
- Update GoalSource removal from context sources

### Step 11: Update `crates/agent/src/lib.rs`

- Remove: `pub mod goal_handler;`, `pub mod plan_handler;`, `pub mod plan_executor;`, `pub mod plan_step_generator;`
- Add: `pub mod progress_handler;`
- Remove re-exports: `GoalHandlerImpl`, `PlanHandlerImpl`, `PlanCompletionHandlerImpl`, `StepExecutionResult`
- Add re-export: `ProgressHandlerImpl`

### Step 12: Run agent tests

```bash
cargo nextest run -p agent
```

### Step 13: Commit

```bash
git add crates/agent/
git commit -m "refactor(agent): remove Goal/Plan handlers, add ProgressHandler, simplify pipeline"
```

---

## Task 7: Facade, Integration Tests, Verification

**Files:**
- Modify: `src/lib.rs` (update re-exports)
- Modify: `tests/` (update integration tests)
- Various: fix any remaining compilation errors across workspace

### Step 1: Update `src/lib.rs` facade

- Remove: `PlanCompletionHandlerImpl` from agent re-exports
- Remove: `Plan`, `PlanStatus` from domain re-exports
- Add: `ProgressHandlerImpl` to agent re-exports
- Add domain type re-exports if needed: `Area`, `AreaStatus`, `Project`, `Objective`, `KeyResult`

### Step 2: Fix integration tests

- `tests/integration/learning.rs`: Remove `goal_plan_completion_metrics_end_to_end` test
- `tests/e2e/agent_pipeline.rs`: Remove `test_complex_request_triggers_planned` and `test_reactive_escalates_to_planned` tests
- Update any test that references `repos.goals`, `repos.plans`, `repos.todos`
- Add basic PARA+OKR integration tests:
  - Create area → create project in area → create objective → create KR → create action
  - Test progress cascade: complete action → KR progress updates → Objective progress updates

### Step 3: Fix remaining compilation errors

```bash
cargo build --workspace 2>&1 | head -50
```

Iterate until clean. Common fixes:
- Update `use` statements across crates
- Fix any remaining `Todo`/`Goal`/`Plan` references
- Update mock providers in tests

### Step 4: Run full test suite

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

### Step 5: Run lints

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

### Step 6: Commit

```bash
git add .
git commit -m "refactor: complete PARA+OKR migration, update facade and tests"
```

---

## Summary

| Task | Description | Key files | Est. commits |
|------|-------------|-----------|-------------|
| 1 | Domain layer: new types, remove old | `crates/domain/src/` | 1 |
| 2 | Storage: schema + row types | `crates/storage/migrations/`, `rows/` | 1 |
| 3 | Storage: repositories | `crates/storage/src/repos/` | 1 |
| 4 | Tools: new tools, remove old | `crates/tools/src/` | 1 |
| 5 | Feature-todo: Todo → Action rename | `crates/feature-todo/src/` | 1 |
| 6 | Agent: remove Plan/Goal, update pipeline | `crates/agent/src/` | 1 |
| 7 | Facade + tests + verification | `src/lib.rs`, `tests/` | 1 |

**Total: 7 tasks, ~7 commits**

The dependency order is strict: 1 → 2 → 3 → 4 → 5 → 6 → 7. Each task builds on the previous. The workspace may not compile between tasks 1-6 (since removing Goal/Plan breaks dependents until they're all updated), but each task is a logical unit. For a compilable checkpoint at each commit, tasks 1-3 (storage) could be done as one atomic commit, then 4-6 (tools/agent) as another.
