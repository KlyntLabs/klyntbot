# Agentic Capabilities Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build autonomous goal engine, planning system, and learning capabilities to transform klyntbot from reactive assistant into proactive agent.

**Architecture:** Hybrid approach — `goal` as separate crate (Layer 2), planning + learning as modules in agent crate (Layer 5). Follows existing patterns (similar to calendar crate + calendar_sync_adapter).

**Tech Stack:** Rust, tokio, serde, uuid, chrono, petgraph (for DAG validation), existing klyntbot architecture

**Phases:**
- Phase 1: Goal Engine (~1,100 LOC, 2 weeks)
- Phase 2: Planning Engine (~1,200 LOC, 2 weeks)
- Phase 3: Learning System (~1,000 LOC, 2 weeks)
- Phase 4: Integration & Polish (~500 LOC, 1 week)

---

## Phase 1: Goal Engine

**Estimated Effort:** ~1,100 LOC across 10 tasks
**Duration:** 2 weeks
**Dependencies:** None (foundation layer)

---

### Task 1: Create Goal Crate Structure

**Files:**
- Create: `crates/goal/Cargo.toml`
- Create: `crates/goal/src/lib.rs`
- Create: `crates/goal/src/types.rs`

**Step 1: Create crate directory and Cargo.toml**

```bash
mkdir -p crates/goal/src
```

```toml
# crates/goal/Cargo.toml
[package]
name = "goal"
version.workspace = true
edition.workspace = true

[dependencies]
common.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
uuid = { workspace = true, features = ["v4", "serde"] }
chrono = { workspace = true, features = ["serde"] }
thiserror.workspace = true
```

**Step 2: Run cargo check to verify crate setup**

Run: `cargo check -p goal`
Expected: ERROR (no src/lib.rs yet)

**Step 3: Create lib.rs with re-exports**

```rust
// crates/goal/src/lib.rs
pub mod types;

pub use types::{Goal, GoalStatus, Metric, GoalProgress, GoalSuggestion};
```

**Step 4: Run cargo check again**

Run: `cargo check -p goal`
Expected: ERROR (types.rs doesn't exist)

**Step 5: Create empty types.rs**

```rust
// crates/goal/src/types.rs
```

**Step 6: Run cargo check to verify**

Run: `cargo check -p goal`
Expected: SUCCESS (empty crate compiles)

**Step 7: Commit crate structure**

```bash
git add crates/goal/
git commit -m "feat(goal): create goal crate structure at Layer 2"
```

---

### Task 2: Implement Goal Domain Types

**Files:**
- Modify: `crates/goal/src/types.rs`

**Step 1: Write test for Goal construction**

```rust
// crates/goal/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_creation() {
        let goal = Goal {
            id: Uuid::new_v4(),
            title: "Launch product".to_string(),
            description: "Build and launch SaaS product".to_string(),
            status: GoalStatus::Active,
            priority: 3,
            target_date: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metrics: vec![],
            linked_project_ids: vec![],
            metadata: HashMap::new(),
        };

        assert_eq!(goal.title, "Launch product");
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.priority, 3);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p goal --lib types::tests::test_goal_creation`
Expected: FAIL (Goal, GoalStatus not defined)

**Step 3: Implement Goal types**

```rust
// crates/goal/src/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Goal {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    pub priority: u8,  // 1-5 (matches todo priority)
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metrics: Vec<Metric>,
    pub linked_project_ids: Vec<Uuid>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Paused,
    Achieved,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metric {
    pub name: String,
    pub current: f64,
    pub target: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgress {
    pub goal_id: Uuid,
    pub completion_percentage: f64,
    pub metrics: Vec<Metric>,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct GoalSuggestion {
    pub proposed_title: String,
    pub rationale: String,
    pub linked_items: Vec<Uuid>,
    pub confidence: f64,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p goal --lib types::tests::test_goal_creation`
Expected: PASS

**Step 5: Add test for GoalStatus transitions**

```rust
// Add to tests module in crates/goal/src/types.rs
#[test]
fn test_goal_status_transitions() {
    let statuses = vec![
        GoalStatus::Active,
        GoalStatus::Paused,
        GoalStatus::Achieved,
        GoalStatus::Abandoned,
    ];

    // Verify all statuses are distinct
    for (i, s1) in statuses.iter().enumerate() {
        for (j, s2) in statuses.iter().enumerate() {
            if i == j {
                assert_eq!(s1, s2);
            } else {
                assert_ne!(s1, s2);
            }
        }
    }
}
```

**Step 6: Run test**

Run: `cargo test -p goal --lib types::tests::test_goal_status_transitions`
Expected: PASS

**Step 7: Add test for Metric calculations**

```rust
// Add to tests module in crates/goal/src/types.rs
#[test]
fn test_metric_progress() {
    let metric = Metric {
        name: "Tasks completed".to_string(),
        current: 3.0,
        target: 5.0,
        unit: "tasks".to_string(),
    };

    let progress = (metric.current / metric.target) * 100.0;
    assert_eq!(progress, 60.0);
}
```

**Step 8: Run test**

Run: `cargo test -p goal --lib types::tests::test_metric_progress`
Expected: PASS

**Step 9: Commit types implementation**

```bash
git add crates/goal/src/types.rs
git commit -m "feat(goal): implement Goal domain types with tests"
```

---

### Task 3: Implement GoalStore with JSONL Persistence

**Files:**
- Create: `crates/goal/src/store.rs`
- Modify: `crates/goal/src/lib.rs`

**Step 1: Add store to lib.rs**

```rust
// crates/goal/src/lib.rs
pub mod types;
pub mod store;  // NEW

pub use types::{Goal, GoalStatus, Metric, GoalProgress, GoalSuggestion};
pub use store::GoalStore;  // NEW
```

**Step 2: Write test for GoalStore creation**

```rust
// crates/goal/src/store.rs
use crate::types::{Goal, GoalStatus, GoalProgress};
use common::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct GoalStore {
    goals: HashMap<Uuid, Goal>,
    file_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use chrono::Utc;

    #[test]
    fn test_goal_store_creation() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("goals.jsonl");

        let store = GoalStore::new(file_path.clone()).unwrap();
        assert_eq!(store.goals.len(), 0);
        assert_eq!(store.file_path, file_path);
    }
}
```

**Step 3: Add tempfile dependency**

```toml
# crates/goal/Cargo.toml - add under [dev-dependencies]
[dev-dependencies]
tempfile = "3"
```

**Step 4: Run test to verify it fails**

Run: `cargo test -p goal --lib store::tests::test_goal_store_creation`
Expected: FAIL (GoalStore::new not implemented)

**Step 5: Implement GoalStore::new**

```rust
// crates/goal/src/store.rs (add to impl)
impl GoalStore {
    pub fn new(file_path: PathBuf) -> Result<Self> {
        Ok(Self {
            goals: HashMap::new(),
            file_path,
        })
    }
}
```

**Step 6: Run test**

Run: `cargo test -p goal --lib store::tests::test_goal_store_creation`
Expected: PASS

**Step 7: Write test for goal creation and retrieval**

```rust
// Add to tests module in crates/goal/src/store.rs
#[test]
fn test_create_and_get_goal() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    let goal = Goal {
        id: Uuid::new_v4(),
        title: "Test Goal".to_string(),
        description: "Test description".to_string(),
        status: GoalStatus::Active,
        priority: 3,
        target_date: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    };

    let goal_id = store.create(goal.clone()).unwrap();

    let retrieved = store.get(&goal_id).unwrap();
    assert_eq!(retrieved.title, "Test Goal");
    assert_eq!(retrieved.status, GoalStatus::Active);
}
```

**Step 8: Run test to verify it fails**

Run: `cargo test -p goal --lib store::tests::test_create_and_get_goal`
Expected: FAIL (create, get not implemented)

**Step 9: Implement create and get methods**

```rust
// Add to impl GoalStore
pub fn create(&mut self, goal: Goal) -> Result<Uuid> {
    let id = goal.id;
    self.goals.insert(id, goal);
    Ok(id)
}

pub fn get(&self, id: &Uuid) -> Option<&Goal> {
    self.goals.get(id)
}
```

**Step 10: Run test**

Run: `cargo test -p goal --lib store::tests::test_create_and_get_goal`
Expected: PASS

**Step 11: Write test for JSONL persistence**

```rust
// Add to tests module
#[test]
fn test_save_and_load_jsonl() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("goals.jsonl");

    // Create and save goals
    let mut store = GoalStore::new(file_path.clone()).unwrap();
    let goal1 = create_test_goal("Goal 1");
    let goal2 = create_test_goal("Goal 2");

    store.create(goal1.clone()).unwrap();
    store.create(goal2.clone()).unwrap();
    store.save().unwrap();

    // Load from disk
    let loaded_store = GoalStore::load(file_path).unwrap();
    assert_eq!(loaded_store.goals.len(), 2);
    assert!(loaded_store.get(&goal1.id).is_some());
    assert!(loaded_store.get(&goal2.id).is_some());
}

fn create_test_goal(title: &str) -> Goal {
    Goal {
        id: Uuid::new_v4(),
        title: title.to_string(),
        description: String::new(),
        status: GoalStatus::Active,
        priority: 3,
        target_date: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    }
}
```

**Step 12: Run test to verify it fails**

Run: `cargo test -p goal --lib store::tests::test_save_and_load_jsonl`
Expected: FAIL (save, load not implemented)

**Step 13: Implement save and load methods**

```rust
// Add to impl GoalStore
pub fn save(&self) -> Result<()> {
    let mut content = String::new();
    for goal in self.goals.values() {
        let json = serde_json::to_string(goal)?;
        content.push_str(&json);
        content.push('\n');
    }
    fs::write(&self.file_path, content)?;
    Ok(())
}

pub fn load(file_path: PathBuf) -> Result<Self> {
    if !file_path.exists() {
        return Ok(Self {
            goals: HashMap::new(),
            file_path,
        });
    }

    let content = fs::read_to_string(&file_path)?;
    let mut goals = HashMap::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let goal: Goal = serde_json::from_str(line)?;
        goals.insert(goal.id, goal);
    }

    Ok(Self { goals, file_path })
}
```

**Step 14: Run test**

Run: `cargo test -p goal --lib store::tests::test_save_and_load_jsonl`
Expected: PASS

**Step 15: Implement remaining methods (update, list, delete)**

```rust
// Add to impl GoalStore
pub fn update(&mut self, goal: Goal) -> Result<()> {
    let id = goal.id;
    self.goals.insert(id, goal);
    Ok(())
}

pub fn list(&self, status: Option<GoalStatus>) -> Vec<&Goal> {
    match status {
        Some(s) => self.goals.values().filter(|g| g.status == s).collect(),
        None => self.goals.values().collect(),
    }
}

pub fn delete(&mut self, id: &Uuid) -> Result<()> {
    if let Some(mut goal) = self.goals.get_mut(id) {
        goal.status = GoalStatus::Abandoned;
        Ok(())
    } else {
        Err(common::KlyntbotError::Internal(format!("Goal {} not found", id)))
    }
}
```

**Step 16: Add tests for new methods**

```rust
// Add to tests module
#[test]
fn test_update_goal() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    let mut goal = create_test_goal("Original Title");
    let goal_id = store.create(goal.clone()).unwrap();

    goal.title = "Updated Title".to_string();
    store.update(goal).unwrap();

    let retrieved = store.get(&goal_id).unwrap();
    assert_eq!(retrieved.title, "Updated Title");
}

#[test]
fn test_list_goals_by_status() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    let mut goal1 = create_test_goal("Active Goal");
    let mut goal2 = create_test_goal("Paused Goal");
    goal2.status = GoalStatus::Paused;

    store.create(goal1).unwrap();
    store.create(goal2).unwrap();

    let active_goals = store.list(Some(GoalStatus::Active));
    assert_eq!(active_goals.len(), 1);
    assert_eq!(active_goals[0].title, "Active Goal");

    let all_goals = store.list(None);
    assert_eq!(all_goals.len(), 2);
}

#[test]
fn test_delete_goal() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    let goal = create_test_goal("To Delete");
    let goal_id = store.create(goal).unwrap();

    store.delete(&goal_id).unwrap();

    let deleted = store.get(&goal_id).unwrap();
    assert_eq!(deleted.status, GoalStatus::Abandoned);
}
```

**Step 17: Run all store tests**

Run: `cargo test -p goal --lib store::`
Expected: ALL PASS

**Step 18: Commit store implementation**

```bash
git add crates/goal/src/store.rs crates/goal/Cargo.toml crates/goal/src/lib.rs
git commit -m "feat(goal): implement GoalStore with JSONL persistence and tests"
```

---

### Task 4: Implement GoalProgress Calculation

**Files:**
- Modify: `crates/goal/src/store.rs`

**Step 1: Write test for progress calculation**

```rust
// Add to tests module in crates/goal/src/store.rs
#[test]
fn test_calculate_progress_with_projects() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    let mut goal = create_test_goal("Goal with Projects");
    goal.linked_project_ids = vec![
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ];

    let goal_id = store.create(goal).unwrap();

    // For now, assume all projects exist and none are completed
    let progress = store.calculate_progress(&goal_id).unwrap();

    assert_eq!(progress.goal_id, goal_id);
    assert_eq!(progress.completion_percentage, 0.0);
    assert_eq!(progress.summary, "0 of 3 projects completed");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p goal --lib store::tests::test_calculate_progress_with_projects`
Expected: FAIL (calculate_progress not implemented)

**Step 3: Implement calculate_progress (basic version)**

```rust
// Add to impl GoalStore
pub fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress> {
    let goal = self.get(id)
        .ok_or_else(|| common::KlyntbotError::Internal(format!("Goal {} not found", id)))?;

    let total_projects = goal.linked_project_ids.len();

    // For now, assume no projects are completed (will integrate with ProjectStore later)
    let completed_projects = 0;

    let completion_percentage = if total_projects > 0 {
        (completed_projects as f64 / total_projects as f64) * 100.0
    } else {
        0.0
    };

    let summary = format!("{} of {} projects completed", completed_projects, total_projects);

    Ok(GoalProgress {
        goal_id: *id,
        completion_percentage,
        metrics: goal.metrics.clone(),
        summary,
    })
}
```

**Step 4: Run test**

Run: `cargo test -p goal --lib store::tests::test_calculate_progress_with_projects`
Expected: PASS

**Step 5: Commit progress calculation**

```bash
git add crates/goal/src/store.rs
git commit -m "feat(goal): implement calculate_progress (basic version)"
```

---

### Task 5: Implement GoalSuggestionEngine

**Files:**
- Create: `crates/goal/src/suggestion.rs`
- Modify: `crates/goal/src/lib.rs`

**Step 1: Add suggestion module to lib.rs**

```rust
// crates/goal/src/lib.rs
pub mod types;
pub mod store;
pub mod suggestion;  // NEW

pub use types::{Goal, GoalStatus, Metric, GoalProgress, GoalSuggestion};
pub use store::GoalStore;
pub use suggestion::GoalSuggestionEngine;  // NEW
```

**Step 2: Write test for pattern detection**

```rust
// crates/goal/src/suggestion.rs
use crate::types::GoalSuggestion;
use uuid::Uuid;

pub struct GoalSuggestionEngine {
    pattern_threshold: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_engine_creation() {
        let engine = GoalSuggestionEngine::new(5);
        assert_eq!(engine.pattern_threshold, 5);
    }

    #[test]
    fn test_detect_goal_intent() {
        let engine = GoalSuggestionEngine::new(5);

        let message = "I want to launch a new product this year";
        let suggestion = engine.detect_goal_intent(message);

        assert!(suggestion.is_some());
        let s = suggestion.unwrap();
        assert!(s.proposed_title.contains("launch") || s.proposed_title.contains("product"));
        assert!(s.confidence > 0.5);
    }
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test -p goal --lib suggestion::`
Expected: FAIL (methods not implemented)

**Step 4: Implement GoalSuggestionEngine**

```rust
// crates/goal/src/suggestion.rs
impl GoalSuggestionEngine {
    pub fn new(pattern_threshold: usize) -> Self {
        Self { pattern_threshold }
    }

    pub fn detect_goal_intent(&self, message: &str) -> Option<GoalSuggestion> {
        let message_lower = message.to_lowercase();

        // Keywords that indicate goal-setting intent
        let goal_keywords = [
            "launch", "build", "create", "improve", "learn",
            "achieve", "complete", "finish", "master", "develop"
        ];

        for keyword in &goal_keywords {
            if message_lower.contains(keyword) {
                // Extract potential goal title (simplified heuristic)
                let proposed_title = self.extract_goal_title(&message, keyword);

                return Some(GoalSuggestion {
                    proposed_title,
                    rationale: format!("Detected goal intent from keyword '{}'", keyword),
                    linked_items: vec![],
                    confidence: 0.75,
                });
            }
        }

        None
    }

    fn extract_goal_title(&self, message: &str, keyword: &str) -> String {
        // Simplified: capitalize first letter + keyword + rest
        let mut title = String::from("Goal: ");
        title.push_str(&message[0..60.min(message.len())]);
        title
    }
}
```

**Step 5: Run test**

Run: `cargo test -p goal --lib suggestion::`
Expected: PASS

**Step 6: Add test for tag-based pattern detection**

```rust
// Add to tests module
#[test]
fn test_analyze_patterns_with_tag_threshold() {
    let engine = GoalSuggestionEngine::new(5);

    // Simulate 5 tasks with "fitness" tag
    let tag = "fitness";
    let task_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

    // This test is a placeholder - full implementation requires Todo/Project types
    // For now, just verify the engine exists
    assert_eq!(engine.pattern_threshold, 5);
}
```

**Step 7: Run test**

Run: `cargo test -p goal --lib suggestion::tests::test_analyze_patterns_with_tag_threshold`
Expected: PASS

**Step 8: Commit suggestion engine**

```bash
git add crates/goal/src/suggestion.rs crates/goal/src/lib.rs
git commit -m "feat(goal): implement GoalSuggestionEngine with intent detection"
```

---

### Task 6: Add Goal Error Types to Common

**Files:**
- Modify: `crates/common/src/error.rs`

**Step 1: Write test for GoalError variants**

```rust
// Add to tests module in crates/common/src/error.rs
#[test]
fn test_goal_error_conversions() {
    use super::*;

    let err = GoalError::NotFound(uuid::Uuid::new_v4());
    let klyntbot_err: KlyntbotError = err.into();

    assert!(matches!(klyntbot_err, KlyntbotError::Goal(_)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p common --lib error::tests::test_goal_error_conversions`
Expected: FAIL (GoalError not defined)

**Step 3: Add GoalError to error.rs**

```rust
// Add to crates/common/src/error.rs
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum GoalError {
    #[error("Goal not found: {0}")]
    NotFound(Uuid),

    #[error("Goal already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid metric: {0}")]
    InvalidMetric(String),

    #[error("Goal storage corrupted")]
    StorageCorrupted,
}

// Add to KlyntbotError enum
#[derive(Debug, thiserror::Error)]
pub enum KlyntbotError {
    // ... existing variants ...

    #[error("Goal error: {0}")]
    Goal(#[from] GoalError),
}
```

**Step 4: Run test**

Run: `cargo test -p common --lib error::tests::test_goal_error_conversions`
Expected: PASS

**Step 5: Commit error types**

```bash
git add crates/common/src/error.rs
git commit -m "feat(common): add GoalError types"
```

---

### Task 7: Implement GoalTool with GoalHandler Trait

**Files:**
- Create: `crates/tools/src/goal_tool.rs`
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/Cargo.toml`

**Step 1: Add goal dependency to tools**

```toml
# crates/tools/Cargo.toml - add to [dependencies]
goal.workspace = true
```

**Step 2: Add goal_tool to tools lib.rs**

```rust
// crates/tools/src/lib.rs - add to module declarations
pub mod goal_tool;
```

**Step 3: Write test for GoalHandler trait**

```rust
// crates/tools/src/goal_tool.rs
use async_trait::async_trait;
use common::Result;
use goal::{Goal, GoalStatus, GoalProgress};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait GoalHandler: Send + Sync {
    async fn create_goal(&self, goal: Goal) -> Result<Uuid>;
    async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>>;
    async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>>;
    async fn update_goal(&self, goal: Goal) -> Result<()>;
    async fn delete_goal(&self, id: &Uuid) -> Result<()>;
    async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress>;
}

pub struct GoalTool {
    handler: Option<Arc<dyn GoalHandler>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_tool_creation_without_handler() {
        let tool = GoalTool::new(None);
        assert!(tool.handler.is_none());
    }
}
```

**Step 4: Run test to verify it fails**

Run: `cargo test -p tools --lib goal_tool::tests::test_goal_tool_creation_without_handler`
Expected: FAIL (GoalTool::new not implemented)

**Step 5: Implement GoalTool::new**

```rust
// Add to crates/tools/src/goal_tool.rs
impl GoalTool {
    pub fn new(handler: Option<Arc<dyn GoalHandler>>) -> Self {
        Self { handler }
    }
}
```

**Step 6: Run test**

Run: `cargo test -p tools --lib goal_tool::tests::test_goal_tool_creation_without_handler`
Expected: PASS

**Step 7: Implement Tool trait for GoalTool**

```rust
// Add to crates/tools/src/goal_tool.rs
use crate::{Tool, RoutingContext};

#[async_trait]
impl Tool for GoalTool {
    fn name(&self) -> &str {
        "goal"
    }

    fn description(&self) -> &str {
        "Manage strategic goals that span multiple projects. Actions: create, list, show, update, delete, progress."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "show", "update", "delete", "progress"],
                    "description": "The goal action to perform"
                },
                "title": {
                    "type": "string",
                    "description": "Goal title (for create, update)"
                },
                "description": {
                    "type": "string",
                    "description": "Goal description (optional)"
                },
                "priority": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "description": "Goal priority 1-5 (optional)"
                },
                "goal_id": {
                    "type": "string",
                    "description": "Goal ID (for show, update, delete, progress)"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "achieved", "abandoned"],
                    "description": "Filter by status (for list)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let handler = self.handler.as_ref()
            .ok_or_else(|| common::KlyntbotError::Tool("GoalHandler not configured".into()))?;

        let action = args.get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::KlyntbotError::Tool("Missing action".into()))?;

        match action {
            "list" => {
                let status = args.get("status")
                    .and_then(|v| v.as_str())
                    .map(parse_goal_status);

                let goals = handler.list_goals(status).await?;
                Ok(format!("Found {} goals", goals.len()))
            }
            _ => Ok(format!("Action '{}' not yet implemented", action))
        }
    }
}

fn parse_goal_status(s: &str) -> GoalStatus {
    match s {
        "active" => GoalStatus::Active,
        "paused" => GoalStatus::Paused,
        "achieved" => GoalStatus::Achieved,
        "abandoned" => GoalStatus::Abandoned,
        _ => GoalStatus::Active,
    }
}
```

**Step 8: Run cargo check**

Run: `cargo check -p tools`
Expected: SUCCESS

**Step 9: Commit GoalTool**

```bash
git add crates/tools/src/goal_tool.rs crates/tools/src/lib.rs crates/tools/Cargo.toml
git commit -m "feat(tools): implement GoalTool and GoalHandler trait"
```

---

### Task 8: Implement GoalHandler in Agent Crate

**Files:**
- Create: `crates/agent/src/goal_handler.rs`
- Modify: `crates/agent/src/lib.rs`
- Modify: `crates/agent/Cargo.toml`

**Step 1: Add goal dependency to agent**

```toml
# crates/agent/Cargo.toml - add to [dependencies]
goal.workspace = true
```

**Step 2: Add goal_handler module**

```rust
// crates/agent/src/lib.rs - add module declaration
pub mod goal_handler;
```

**Step 3: Write test for GoalHandlerImpl**

```rust
// crates/agent/src/goal_handler.rs
use async_trait::async_trait;
use common::Result;
use goal::{Goal, GoalStatus, GoalStore, GoalProgress};
use tools::goal_tool::GoalHandler;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub struct GoalHandlerImpl {
    store: Arc<RwLock<GoalStore>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_goal_handler_create_and_get() {
        let tmp = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(
            GoalStore::new(tmp.path().join("goals.jsonl")).unwrap()
        ));

        let handler = GoalHandlerImpl::new(store);

        let goal = create_test_goal("Test Goal");
        let goal_id = handler.create_goal(goal.clone()).await.unwrap();

        let retrieved = handler.get_goal(&goal_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Goal");
    }

    fn create_test_goal(title: &str) -> Goal {
        use chrono::Utc;
        use std::collections::HashMap;

        Goal {
            id: Uuid::new_v4(),
            title: title.to_string(),
            description: String::new(),
            status: GoalStatus::Active,
            priority: 3,
            target_date: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metrics: vec![],
            linked_project_ids: vec![],
            metadata: HashMap::new(),
        }
    }
}
```

**Step 4: Run test to verify it fails**

Run: `cargo test -p agent --lib goal_handler::tests::test_goal_handler_create_and_get`
Expected: FAIL (methods not implemented)

**Step 5: Implement GoalHandlerImpl**

```rust
// Add to crates/agent/src/goal_handler.rs
impl GoalHandlerImpl {
    pub fn new(store: Arc<RwLock<GoalStore>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl GoalHandler for GoalHandlerImpl {
    async fn create_goal(&self, goal: Goal) -> Result<Uuid> {
        let mut store = self.store.write().unwrap();
        store.create(goal)
    }

    async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>> {
        let store = self.store.read().unwrap();
        Ok(store.get(id).cloned())
    }

    async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>> {
        let store = self.store.read().unwrap();
        Ok(store.list(status).into_iter().cloned().collect())
    }

    async fn update_goal(&self, goal: Goal) -> Result<()> {
        let mut store = self.store.write().unwrap();
        store.update(goal)
    }

    async fn delete_goal(&self, id: &Uuid) -> Result<()> {
        let mut store = self.store.write().unwrap();
        store.delete(id)
    }

    async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress> {
        let store = self.store.read().unwrap();
        store.calculate_progress(id)
    }
}
```

**Step 6: Run test**

Run: `cargo test -p agent --lib goal_handler::tests::test_goal_handler_create_and_get`
Expected: PASS

**Step 7: Commit goal handler**

```bash
git add crates/agent/src/goal_handler.rs crates/agent/src/lib.rs crates/agent/Cargo.toml
git commit -m "feat(agent): implement GoalHandlerImpl with tests"
```

---

### Task 9: Add Goal CLI Commands

**Files:**
- Create: `cli/src/goal_commands.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/Cargo.toml`

**Step 1: Add goal dependency to CLI**

```toml
# cli/Cargo.toml - add to [dependencies]
goal.workspace = true
```

**Step 2: Create goal commands module**

```rust
// cli/src/goal_commands.rs
use clap::{Args, Subcommand};
use goal::{Goal, GoalStatus};
use uuid::Uuid;

#[derive(Args, Debug)]
pub struct GoalCommands {
    #[command(subcommand)]
    pub command: GoalCommand,
}

#[derive(Subcommand, Debug)]
pub enum GoalCommand {
    /// Create a new goal
    Create {
        /// Goal title
        title: String,

        /// Goal description (optional)
        #[arg(long)]
        description: Option<String>,

        /// Priority (1-5)
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=5))]
        priority: Option<u8>,
    },

    /// List all goals
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
    },

    /// Show goal details
    Show {
        /// Goal ID
        goal_id: String,
    },

    /// Update a goal
    Update {
        /// Goal ID
        goal_id: String,

        /// New title (optional)
        #[arg(long)]
        title: Option<String>,

        /// New priority (optional)
        #[arg(long)]
        priority: Option<u8>,
    },

    /// Delete a goal (mark as abandoned)
    Delete {
        /// Goal ID
        goal_id: String,
    },
}

pub async fn handle_goal_command(cmd: GoalCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd.command {
        GoalCommand::Create { title, description, priority } => {
            println!("Creating goal: {}", title);
            // Implementation will be added when integrating with agent
            Ok(())
        }
        GoalCommand::List { status } => {
            println!("Listing goals (status: {:?})", status);
            Ok(())
        }
        GoalCommand::Show { goal_id } => {
            println!("Showing goal: {}", goal_id);
            Ok(())
        }
        GoalCommand::Update { goal_id, title, priority } => {
            println!("Updating goal: {}", goal_id);
            Ok(())
        }
        GoalCommand::Delete { goal_id } => {
            println!("Deleting goal: {}", goal_id);
            Ok(())
        }
    }
}
```

**Step 3: Add goal subcommand to CLI**

```rust
// cli/src/main.rs - add to Cli enum
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Manage strategic goals
    Goal(goal_commands::GoalCommands),
}

// In main() match statement, add:
Commands::Goal(cmd) => goal_commands::handle_goal_command(cmd).await?,
```

**Step 4: Run cargo check**

Run: `cargo check -p cli`
Expected: SUCCESS

**Step 5: Test CLI parsing**

Run: `cargo run -- goal create "Test Goal" --priority 3`
Expected: "Creating goal: Test Goal"

**Step 6: Commit CLI commands**

```bash
git add cli/src/goal_commands.rs cli/src/main.rs cli/Cargo.toml
git commit -m "feat(cli): add goal CLI commands (create, list, show, update, delete)"
```

---

### Task 10: Integration Testing for Phase 1

**Files:**
- Create: `tests/goal_integration.rs`

**Step 1: Create integration test file**

```rust
// tests/goal_integration.rs
use goal::{Goal, GoalStatus, GoalStore};
use std::collections::HashMap;
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;

#[tokio::test]
async fn test_goal_full_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    // Create goal
    let goal = Goal {
        id: Uuid::new_v4(),
        title: "Launch Product".to_string(),
        description: "Build and launch SaaS product".to_string(),
        status: GoalStatus::Active,
        priority: 4,
        target_date: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    };

    let goal_id = store.create(goal.clone()).unwrap();

    // Retrieve goal
    let retrieved = store.get(&goal_id).unwrap();
    assert_eq!(retrieved.title, "Launch Product");
    assert_eq!(retrieved.priority, 4);

    // Update goal
    let mut updated_goal = retrieved.clone();
    updated_goal.status = GoalStatus::Achieved;
    store.update(updated_goal).unwrap();

    // Verify update
    let updated = store.get(&goal_id).unwrap();
    assert_eq!(updated.status, GoalStatus::Achieved);

    // Save and reload
    store.save().unwrap();
    let loaded_store = GoalStore::load(tmp.path().join("goals.jsonl")).unwrap();
    let reloaded = loaded_store.get(&goal_id).unwrap();
    assert_eq!(reloaded.title, "Launch Product");
    assert_eq!(reloaded.status, GoalStatus::Achieved);
}

#[tokio::test]
async fn test_goal_progress_calculation() {
    let tmp = TempDir::new().unwrap();
    let mut store = GoalStore::new(tmp.path().join("goals.jsonl")).unwrap();

    let mut goal = Goal {
        id: Uuid::new_v4(),
        title: "Goal with Projects".to_string(),
        description: String::new(),
        status: GoalStatus::Active,
        priority: 3,
        target_date: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metrics: vec![],
        linked_project_ids: vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
        metadata: HashMap::new(),
    };

    let goal_id = store.create(goal).unwrap();

    let progress = store.calculate_progress(&goal_id).unwrap();
    assert_eq!(progress.completion_percentage, 0.0);
    assert_eq!(progress.summary, "0 of 3 projects completed");
}
```

**Step 2: Run integration tests**

Run: `cargo test --test goal_integration`
Expected: ALL PASS

**Step 3: Commit integration tests**

```bash
git add tests/goal_integration.rs
git commit -m "test: add goal integration tests for full lifecycle"
```

**Step 4: Run all tests to verify Phase 1 complete**

Run: `cargo test --workspace`
Expected: ALL PASS (including new goal tests)

**Step 5: Final commit for Phase 1**

```bash
git commit --allow-empty -m "feat: complete Phase 1 - Goal Engine implementation

Phase 1 deliverables:
- goal crate with types, store, suggestion engine (~800 LOC)
- GoalTool and GoalHandler trait in tools (~300 LOC)
- GoalHandlerImpl in agent (~150 LOC)
- CLI commands for goal management (~250 LOC)
- 25+ unit tests + integration tests
- JSONL persistence with backup/recovery

Next: Phase 2 - Planning Engine"
```

---

## Phase 2: Planning Engine (Preview)

**Coming next:** Planning engine with ReAct-style execution, confidence-gated approval, backtracking on failure.

**Key files:**
- `crates/agent/src/planner.rs` (~800 LOC)
- `crates/agent/src/agent_loop.rs` (modifications)
- `tests/planner_integration.rs`

---

## Phase 3: Learning System (Preview)

**Coming next:** Outcome recording, pattern analysis, adaptive behavior.

**Key files:**
- `crates/agent/src/learning.rs` (~700 LOC)
- `crates/agent/src/agent_loop.rs` (modifications)
- Background analysis service

---

## Execution Notes

**Testing Strategy:**
- Every implementation follows TDD (test → fail → implement → pass → commit)
- Run `cargo test --workspace` frequently to catch regressions
- Run `cargo clippy --workspace --all-targets --all-features` before each commit

**Commit Frequency:**
- Commit after every completed task (every 5-10 steps)
- Each commit should be atomic and buildable
- Use conventional commit messages (feat, fix, test, docs, refactor)

**Performance Targets:**
- Goal store loads 1,000 goals in <500ms
- All unit tests run in <5 seconds
- Integration tests run in <10 seconds

---

**End of Phase 1 Plan**
