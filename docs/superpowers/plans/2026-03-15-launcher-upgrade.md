# Launcher Upgrade Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the launcher from a basic AI chat window into a full command center with app search, clipboard history, productivity dashboard, window management, calendar, calculator, scripts, and system commands.

**Architecture:** `feature-launcher` crate (L4) holds data types, storage, and provider logic. `app-core` orchestrates via `LauncherService` and `LauncherSearchEngine`. Frontend is a new `features/launcher/` directory with state machine, dashboard widgets, and search results UI.

**Tech Stack:** Rust (tokio, sqlx, nucleo-matcher, meval, objc2, notify), TypeScript/React, Tailwind v4, Tauri 2 IPC + events.

**Spec:** `docs/superpowers/specs/2026-03-15-launcher-upgrade-design.md`

---

## Chunk 1: Foundation — Crate, Types, Migrations

### Task 1.1: Create `feature-launcher` crate skeleton

**Files:**
- Create: `crates/feature-launcher/Cargo.toml`
- Create: `crates/feature-launcher/src/lib.rs`
- Create: `crates/feature-launcher/src/types.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "feature-launcher"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
storage = { path = "../storage" }
tools-core = { path = "../tools-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
tracing = "0.1"

[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
```

- [ ] **Step 2: Create types.rs with core data types**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub kind: LauncherItemKind,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LauncherItemKind {
    Application {
        path: PathBuf,
        running: bool,
    },
    Task {
        task_id: String,
        status: String,
    },
    Note {
        note_id: String,
        preview: String,
    },
    ClipboardEntry {
        entry_id: i64,
        content_type: ClipboardContentType,
    },
    SystemCommand {
        action: SystemAction,
    },
    Script {
        path: PathBuf,
        name: String,
    },
    Calculator {
        expression: String,
        result: f64,
    },
    Calendar {
        event_id: String,
        starts_at: DateTime<Utc>,
    },
    AiChat {
        query: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardContentType {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemAction {
    LockScreen,
    Sleep,
    Restart,
    Shutdown,
    EmptyTrash,
    ToggleDarkMode,
    ToggleDoNotDisturb,
    EjectAll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowAction {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    LeftThird,
    CenterThird,
    RightThird,
    Maximize,
    Center,
    Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub focus: Option<FocusDashboard>,
    pub calendar: Vec<CalendarDashboard>,
    pub tasks: Vec<TaskDashboard>,
    pub productivity: ProductivityDashboard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusDashboard {
    pub task_name: Option<String>,
    pub elapsed_secs: i64,
    pub target_secs: Option<i64>,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarDashboard {
    pub event_id: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub minutes_until: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDashboard {
    pub id: String,
    pub title: String,
    pub status: String,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductivityDashboard {
    pub total_minutes: i64,
    pub top_category: String,
    pub top_category_pct: f64,
    pub score: i64,
}

/// Result from a search provider before ranking
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub item: LauncherItem,
    pub base_score: f64,
}
```

- [ ] **Step 3: Create lib.rs with FeaturePackage impl**

```rust
pub mod types;

use async_trait::async_trait;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};
use serde_json::Value;

pub use types::*;

pub struct LauncherFeature;

#[async_trait]
impl FeaturePackage for LauncherFeature {
    fn name(&self) -> &str {
        "launcher"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![] // Launcher doesn't expose MCP tools
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Self::migrations_static()
    }

    fn config_key(&self) -> &str {
        "launcher"
    }

    fn default_config(&self) -> Value {
        serde_json::json!({
            "enabled": true,
            "clipboardHistoryEnabled": true,
            "clipboardMaxEntries": 1000,
            "scriptsDir": "~/.klyntbot/scripts"
        })
    }

    async fn health_check(&self) -> common::Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}

impl LauncherFeature {
    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature: "launcher",
            version: 1,
            sql: include_str!("../migrations/001_launcher_tables.sql"),
        }]
    }
}
```

- [ ] **Step 4: Add to workspace members in root Cargo.toml**

Add `"crates/feature-launcher"` to the `[workspace] members` array.

- [ ] **Step 5: Run `cargo build -p feature-launcher`**

Expected: Compiles successfully (after creating migrations file in next task).

- [ ] **Step 6: Commit**

```bash
git add crates/feature-launcher/ Cargo.toml Cargo.lock
git commit -m "feat(launcher): create feature-launcher crate skeleton with types"
```

### Task 1.2: Create migrations

**Files:**
- Create: `crates/feature-launcher/migrations/001_launcher_tables.sql`

- [ ] **Step 1: Write migration SQL**

```sql
-- Frequency learning for search ranking
CREATE TABLE IF NOT EXISTS launcher_frequencies (
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    last_used TEXT NOT NULL,
    PRIMARY KEY (item_id, kind)
);

-- Clipboard history
CREATE TABLE IF NOT EXISTS clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'text',
    source_app TEXT,
    preview TEXT,
    file_path TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

-- FTS5 index for clipboard search
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts USING fts5(
    content, preview, content='clipboard_history', content_rowid='id'
);

-- FTS5 sync triggers
CREATE TRIGGER IF NOT EXISTS clipboard_fts_insert AFTER INSERT ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(rowid, content, preview)
    VALUES (new.id, new.content, new.preview);
END;

CREATE TRIGGER IF NOT EXISTS clipboard_fts_delete AFTER DELETE ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
    VALUES ('delete', old.id, old.content, old.preview);
END;

CREATE TRIGGER IF NOT EXISTS clipboard_fts_update AFTER UPDATE ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
    VALUES ('delete', old.id, old.content, old.preview);
    INSERT INTO clipboard_fts(rowid, content, preview)
    VALUES (new.id, new.content, new.preview);
END;
```

- [ ] **Step 2: Run `cargo build -p feature-launcher`**

Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/migrations/
git commit -m "feat(launcher): add launcher database migrations"
```

### Task 1.3: Create repos (FrequencyRepo + ClipboardRepo)

**Files:**
- Create: `crates/feature-launcher/src/repos/mod.rs`
- Create: `crates/feature-launcher/src/repos/frequency.rs`
- Create: `crates/feature-launcher/src/repos/clipboard.rs`
- Modify: `crates/feature-launcher/src/lib.rs` (add `pub mod repos`)

- [ ] **Step 1: Write failing tests for FrequencyRepo**

In `crates/feature-launcher/src/repos/frequency.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> FrequencyRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(pool.inner(), &crate::LauncherFeature::migrations_static())
            .await
            .unwrap();
        FrequencyRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_increment_and_get() {
        let repo = setup().await;
        repo.increment("com.apple.Safari", "app").await.unwrap();
        repo.increment("com.apple.Safari", "app").await.unwrap();
        let count = repo.get_count("com.apple.Safari", "app").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_zero() {
        let repo = setup().await;
        let count = repo.get_count("nonexistent", "app").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_boost_calculation() {
        let repo = setup().await;
        for _ in 0..10 {
            repo.increment("frequent", "app").await.unwrap();
        }
        repo.increment("rare", "app").await.unwrap();
        let frequent_boost = repo.get_boost("frequent", "app").await.unwrap();
        let rare_boost = repo.get_boost("rare", "app").await.unwrap();
        assert!(frequent_boost > rare_boost);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-launcher -E 'test(frequency)'`
Expected: FAIL — `FrequencyRepo` not defined

- [ ] **Step 3: Implement FrequencyRepo**

```rust
use chrono::Utc;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct FrequencyRepo {
    pool: SqlitePool,
}

impl FrequencyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn increment(&self, item_id: &str, kind: &str) -> common::Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO launcher_frequencies (item_id, kind, count, last_used) \
             VALUES (?, ?, 1, ?) \
             ON CONFLICT(item_id, kind) DO UPDATE SET count = count + 1, last_used = ?",
        )
        .bind(item_id)
        .bind(kind)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_count(&self, item_id: &str, kind: &str) -> common::Result<i64> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT count FROM launcher_frequencies WHERE item_id = ? AND kind = ?",
        )
        .bind(item_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map_or(0, |r| r.0))
    }

    /// Returns log2(count + 1) as a frequency boost multiplier
    pub async fn get_boost(&self, item_id: &str, kind: &str) -> common::Result<f64> {
        let count = self.get_count(item_id, kind).await?;
        Ok((count as f64 + 1.0).log2())
    }

    pub async fn get_boosts_batch(&self, items: &[(String, String)]) -> common::Result<Vec<f64>> {
        let mut boosts = Vec::with_capacity(items.len());
        for (item_id, kind) in items {
            boosts.push(self.get_boost(item_id, kind).await?);
        }
        Ok(boosts)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p feature-launcher -E 'test(frequency)'`
Expected: PASS

- [ ] **Step 5: Write failing tests for ClipboardRepo**

In `crates/feature-launcher/src/repos/clipboard.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> ClipboardRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(pool.inner(), &crate::LauncherFeature::migrations_static())
            .await
            .unwrap();
        ClipboardRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_insert_and_list() {
        let repo = setup().await;
        repo.insert("hello world", "text", Some("Safari"), None).await.unwrap();
        repo.insert("second entry", "text", Some("VSCode"), None).await.unwrap();
        let entries = repo.list(10, 0).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "second entry"); // most recent first
    }

    #[tokio::test]
    async fn test_search_fts() {
        let repo = setup().await;
        repo.insert("rust programming language", "text", None, None).await.unwrap();
        repo.insert("python scripting", "text", None, None).await.unwrap();
        let results = repo.search("rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("rust"));
    }

    #[tokio::test]
    async fn test_pin_and_delete() {
        let repo = setup().await;
        repo.insert("keep me", "text", None, None).await.unwrap();
        let entries = repo.list(10, 0).await.unwrap();
        let id = entries[0].id;
        repo.pin(id, true).await.unwrap();
        let entry = repo.get(id).await.unwrap().unwrap();
        assert!(entry.pinned);
        repo.delete(id).await.unwrap();
        assert!(repo.get(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_eviction_respects_pins() {
        let repo = setup().await;
        // Insert 3 entries, pin the first
        repo.insert("first", "text", None, None).await.unwrap();
        let entries = repo.list(10, 0).await.unwrap();
        repo.pin(entries[0].id, true).await.unwrap();
        repo.insert("second", "text", None, None).await.unwrap();
        repo.insert("third", "text", None, None).await.unwrap();

        // Evict to max 2 entries
        repo.evict_to_max(2).await.unwrap();
        let remaining = repo.list(10, 0).await.unwrap();
        // Pinned entry survives, oldest unpinned evicted
        assert!(remaining.iter().any(|e| e.content == "first")); // pinned
        assert_eq!(remaining.len(), 2);
    }
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cargo nextest run -p feature-launcher -E 'test(clipboard)'`
Expected: FAIL

- [ ] **Step 7: Implement ClipboardRepo**

```rust
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    pub source_app: Option<String>,
    pub preview: Option<String>,
    pub file_path: Option<String>,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ClipboardRepo {
    pool: SqlitePool,
}

impl ClipboardRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        content: &str,
        content_type: &str,
        source_app: Option<&str>,
        file_path: Option<&str>,
    ) -> common::Result<i64> {
        let now = Utc::now().to_rfc3339();
        let preview: String = content.chars().take(200).collect();
        let result = sqlx::query(
            "INSERT INTO clipboard_history (content, content_type, source_app, preview, file_path, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(content)
        .bind(content_type)
        .bind(source_app)
        .bind(&preview)
        .bind(file_path)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get(&self, id: i64) -> common::Result<Option<ClipboardEntry>> {
        let entry = sqlx::query_as::<_, ClipboardEntry>(
            "SELECT * FROM clipboard_history WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(entry)
    }

    pub async fn list(&self, limit: i64, offset: i64) -> common::Result<Vec<ClipboardEntry>> {
        let entries = sqlx::query_as::<_, ClipboardEntry>(
            "SELECT * FROM clipboard_history ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn search(&self, query: &str, limit: i64) -> common::Result<Vec<ClipboardEntry>> {
        let fts_query = format!("{}*", query); // prefix search
        let entries = sqlx::query_as::<_, ClipboardEntry>(
            "SELECT ch.* FROM clipboard_history ch \
             JOIN clipboard_fts fts ON ch.id = fts.rowid \
             WHERE clipboard_fts MATCH ? \
             ORDER BY rank LIMIT ?",
        )
        .bind(&fts_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn pin(&self, id: i64, pinned: bool) -> common::Result<()> {
        sqlx::query("UPDATE clipboard_history SET pinned = ? WHERE id = ?")
            .bind(pinned)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> common::Result<()> {
        sqlx::query("DELETE FROM clipboard_history WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn evict_to_max(&self, max_entries: i64) -> common::Result<i64> {
        let result = sqlx::query(
            "DELETE FROM clipboard_history WHERE id IN ( \
                SELECT id FROM clipboard_history \
                WHERE pinned = 0 \
                ORDER BY created_at ASC \
                LIMIT MAX(0, (SELECT COUNT(*) FROM clipboard_history) - ?) \
             )",
        )
        .bind(max_entries)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as i64)
    }
}
```

- [ ] **Step 8: Create repos/mod.rs and wire into lib.rs**

`repos/mod.rs`:
```rust
pub mod frequency;
pub mod clipboard;

pub use frequency::FrequencyRepo;
pub use clipboard::{ClipboardRepo, ClipboardEntry};
```

Add to `lib.rs`: `pub mod repos;` and re-export `pub use repos::*;`

- [ ] **Step 9: Run all tests**

Run: `cargo nextest run -p feature-launcher`
Expected: All pass

- [ ] **Step 10: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add FrequencyRepo and ClipboardRepo with tests"
```

---

## Chunk 2: Search Providers — App Index, Calculator, System Commands, Scripts

### Task 2.1: Calculator provider

**Files:**
- Create: `crates/feature-launcher/src/search/mod.rs`
- Create: `crates/feature-launcher/src/search/calculator.rs`
- Modify: `crates/feature-launcher/Cargo.toml` (add `meval`)

- [ ] **Step 1: Add `meval = "0.2"` to Cargo.toml dependencies**

- [ ] **Step 2: Write failing tests**

In `search/calculator.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_math() {
        let result = Calculator::try_eval("3 + 4").unwrap();
        assert!((result.result - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_prefix_stripped() {
        let result = Calculator::try_eval("=sqrt(16)").unwrap();
        assert!((result.result - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_not_math_returns_none() {
        assert!(Calculator::try_eval("hello world").is_none());
        assert!(Calculator::try_eval("3d printer").is_none());
    }

    #[test]
    fn test_complex_expression() {
        let result = Calculator::try_eval("(10 + 5) * 2 / 3").unwrap();
        assert!((result.result - 10.0).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p feature-launcher -E 'test(calculator)'`

- [ ] **Step 4: Implement Calculator**

```rust
pub struct CalculatorResult {
    pub expression: String,
    pub result: f64,
}

pub struct Calculator;

impl Calculator {
    /// Try to evaluate a query as a math expression.
    /// Strips leading `=` prefix if present.
    /// Returns None if the query is not a valid expression.
    pub fn try_eval(query: &str) -> Option<CalculatorResult> {
        let expr = query.strip_prefix('=').unwrap_or(query).trim();
        if expr.is_empty() {
            return None;
        }

        // Quick check: must start with digit, (, or - followed by digit
        let first = expr.chars().next()?;
        if !first.is_ascii_digit() && first != '(' && first != '-' {
            return None;
        }
        if first == '-' && expr.chars().nth(1).map_or(true, |c| !c.is_ascii_digit() && c != '(') {
            return None;
        }

        match meval::eval_str(expr) {
            Ok(result) if result.is_finite() => Some(CalculatorResult {
                expression: expr.to_string(),
                result,
            }),
            _ => None,
        }
    }
}
```

- [ ] **Step 5: Create search/mod.rs**

```rust
pub mod calculator;
pub use calculator::Calculator;
```

Add `pub mod search;` to `lib.rs`.

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p feature-launcher -E 'test(calculator)'`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add calculator provider with meval"
```

### Task 2.2: System commands provider

**Files:**
- Create: `crates/feature-launcher/src/search/system_commands.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_exact() {
        let results = SystemCommands::search("lock");
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].kind, LauncherItemKind::SystemCommand { action: SystemAction::LockScreen }));
    }

    #[test]
    fn test_search_fuzzy() {
        let results = SystemCommands::search("dark");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_empty_returns_all() {
        let results = SystemCommands::search("");
        assert_eq!(results.len(), 8);
    }
}
```

- [ ] **Step 2: Implement SystemCommands**

```rust
use crate::types::*;

struct CommandDef {
    title: &'static str,
    subtitle: &'static str,
    action: SystemAction,
    keywords: &'static [&'static str],
}

const COMMANDS: &[CommandDef] = &[
    CommandDef { title: "Lock Screen", subtitle: "Lock the display", action: SystemAction::LockScreen, keywords: &["lock", "screen", "security"] },
    CommandDef { title: "Sleep", subtitle: "Put the Mac to sleep", action: SystemAction::Sleep, keywords: &["sleep", "suspend"] },
    CommandDef { title: "Restart", subtitle: "Restart the Mac", action: SystemAction::Restart, keywords: &["restart", "reboot"] },
    CommandDef { title: "Shutdown", subtitle: "Shut down the Mac", action: SystemAction::Shutdown, keywords: &["shutdown", "power off", "turn off"] },
    CommandDef { title: "Empty Trash", subtitle: "Empty the Trash", action: SystemAction::EmptyTrash, keywords: &["trash", "empty", "clean"] },
    CommandDef { title: "Toggle Dark Mode", subtitle: "Switch appearance", action: SystemAction::ToggleDarkMode, keywords: &["dark", "light", "mode", "appearance", "theme"] },
    CommandDef { title: "Toggle Do Not Disturb", subtitle: "Toggle Focus mode", action: SystemAction::ToggleDoNotDisturb, keywords: &["disturb", "dnd", "focus", "notifications", "quiet"] },
    CommandDef { title: "Eject All", subtitle: "Eject all external drives", action: SystemAction::EjectAll, keywords: &["eject", "drives", "unmount"] },
];

pub struct SystemCommands;

impl SystemCommands {
    pub fn search(query: &str) -> Vec<LauncherItem> {
        let query_lower = query.to_lowercase();
        COMMANDS
            .iter()
            .filter(|cmd| {
                if query.is_empty() {
                    return true;
                }
                let title_match = cmd.title.to_lowercase().contains(&query_lower);
                let keyword_match = cmd.keywords.iter().any(|k| k.contains(&query_lower));
                title_match || keyword_match
            })
            .map(|cmd| {
                let score = if query.is_empty() {
                    0.5
                } else if cmd.title.to_lowercase().starts_with(&query_lower) {
                    1.0
                } else {
                    0.7
                };
                LauncherItem {
                    id: format!("system:{:?}", cmd.action),
                    title: cmd.title.to_string(),
                    subtitle: Some(cmd.subtitle.to_string()),
                    icon: Some("terminal".to_string()),
                    kind: LauncherItemKind::SystemCommand { action: cmd.action.clone() },
                    score,
                }
            })
            .collect()
    }

    #[cfg(target_os = "macos")]
    pub async fn execute(action: &SystemAction) -> common::Result<()> {
        use std::process::Command;
        match action {
            SystemAction::LockScreen => {
                Command::new("pmset").args(["displaysleepnow"]).spawn()?;
            }
            SystemAction::Sleep => {
                Command::new("pmset").args(["sleepnow"]).spawn()?;
            }
            SystemAction::Restart => {
                Command::new("osascript").args(["-e", "tell application \"System Events\" to restart"]).spawn()?;
            }
            SystemAction::Shutdown => {
                Command::new("osascript").args(["-e", "tell application \"System Events\" to shut down"]).spawn()?;
            }
            SystemAction::EmptyTrash => {
                Command::new("osascript").args(["-e", "tell application \"Finder\" to empty trash"]).spawn()?;
            }
            SystemAction::ToggleDarkMode => {
                Command::new("osascript").args(["-e", "tell application \"System Events\" to tell appearance preferences to set dark mode to not dark mode"]).spawn()?;
            }
            SystemAction::ToggleDoNotDisturb => {
                Command::new("shortcuts").args(["run", "Toggle Do Not Disturb"]).spawn()?;
            }
            SystemAction::EjectAll => {
                Command::new("osascript").args(["-e", "tell application \"Finder\" to eject (every disk whose ejectable is true)"]).spawn()?;
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn execute(_action: &SystemAction) -> common::Result<()> {
        Err(common::KlyntbotError::Internal("System commands only supported on macOS".into()))
    }
}
```

- [ ] **Step 3: Add to search/mod.rs**

```rust
pub mod system_commands;
pub use system_commands::SystemCommands;
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-launcher -E 'test(system_commands)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add system commands provider"
```

### Task 2.3: App indexer

**Files:**
- Create: `crates/feature-launcher/src/search/app_index.rs`
- Modify: `crates/feature-launcher/Cargo.toml` (add `nucleo-matcher`, `notify`)

- [ ] **Step 1: Add dependencies**

```toml
nucleo-matcher = "0.3"
notify = { version = "7", default-features = false, features = ["macos_fsevent"] }
parking_lot = "0.12"
```

- [ ] **Step 2: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_app_entry_from_path() {
        let path = std::path::PathBuf::from("/Applications/Safari.app");
        let entry = AppEntry::from_path(&path);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Safari");
    }

    #[test]
    fn test_fuzzy_search() {
        let index = AppIndex::new();
        index.set_apps(vec![
            AppEntry { name: "Visual Studio Code".into(), path: "/Applications/Visual Studio Code.app".into(), bundle_id: None },
            AppEntry { name: "Safari".into(), path: "/Applications/Safari.app".into(), bundle_id: None },
            AppEntry { name: "Slack".into(), path: "/Applications/Slack.app".into(), bundle_id: None },
        ]);
        let results = index.search("vsc", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Visual Studio Code"));
    }

    #[test]
    fn test_search_empty_returns_none() {
        let index = AppIndex::new();
        index.set_apps(vec![
            AppEntry { name: "Safari".into(), path: "/Applications/Safari.app".into(), bundle_id: None },
        ]);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }
}
```

- [ ] **Step 3: Implement AppIndex**

```rust
use crate::types::*;
use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Matcher, Config,
};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<String>,
}

impl AppEntry {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        if ext != "app" {
            return None;
        }
        let name = path.file_stem()?.to_string_lossy().to_string();
        Some(Self {
            name,
            path: path.to_path_buf(),
            bundle_id: None,
        })
    }
}

#[derive(Clone)]
pub struct AppIndex {
    apps: Arc<RwLock<Vec<AppEntry>>>,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write() = apps;
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        let apps = self.apps.read();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &AppEntry)> = apps
            .iter()
            .filter_map(|app| {
                let mut buf = Vec::new();
                let score = pattern.score(nucleo_matcher::Utf32Str::new(&app.name, &mut buf), &mut matcher)?;
                Some((score, app))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, app)| LauncherItem {
                id: format!("app:{}", app.path.display()),
                title: app.name.clone(),
                subtitle: Some(app.path.display().to_string()),
                icon: Some("app-window".to_string()),
                kind: LauncherItemKind::Application {
                    path: app.path.clone(),
                    running: false, // TODO: check running state
                },
                score: (score as f64) / 1000.0, // normalize
            })
            .collect()
    }

    /// Walk application directories and populate the index
    #[cfg(target_os = "macos")]
    pub async fn index_applications(&self) {
        let dirs = ["/Applications", "/System/Applications"];
        let home = std::env::var("HOME").unwrap_or_default();
        let user_apps = format!("{}/Applications", home);

        let mut apps = Vec::new();
        for dir in dirs.iter().chain(std::iter::once(&user_apps.as_str())) {
            if let Ok(entries) = Self::walk_apps(Path::new(dir), 3) {
                apps.extend(entries);
            }
        }

        tracing::info!("Indexed {} applications", apps.len());
        self.set_apps(apps);
    }

    #[cfg(target_os = "macos")]
    fn walk_apps(dir: &Path, max_depth: usize) -> std::io::Result<Vec<AppEntry>> {
        let mut apps = Vec::new();
        if max_depth == 0 {
            return Ok(apps);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "app") {
                if let Some(app) = AppEntry::from_path(&path) {
                    apps.push(app);
                }
            } else if path.is_dir() {
                if let Ok(sub) = Self::walk_apps(&path, max_depth - 1) {
                    apps.extend(sub);
                }
            }
        }
        Ok(apps)
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn index_applications(&self) {
        // No-op on non-macOS
    }
}
```

- [ ] **Step 4: Add to search/mod.rs, run tests**

Run: `cargo nextest run -p feature-launcher -E 'test(app_index)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add app indexer with fuzzy search via nucleo-matcher"
```

### Task 2.4: Script runner provider

**Files:**
- Create: `crates/feature-launcher/src/search/script_runner.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_script(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "{}", content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn test_parse_metadata() {
        let content = "#!/bin/bash\n# name: Deploy Staging\n# icon: rocket\n# description: Deploy to staging\necho hello";
        let meta = ScriptRunner::parse_metadata(content);
        assert_eq!(meta.name.as_deref(), Some("Deploy Staging"));
        assert_eq!(meta.icon.as_deref(), Some("rocket"));
        assert_eq!(meta.description.as_deref(), Some("Deploy to staging"));
    }

    #[test]
    fn test_discover_scripts() {
        let dir = TempDir::new().unwrap();
        create_script(dir.path(), "deploy.sh", "#!/bin/bash\n# name: Deploy\necho deploy");
        create_script(dir.path(), "backup.sh", "#!/bin/bash\n# name: Backup\necho backup");
        create_script(dir.path(), "readme.txt", "not a script");

        let scripts = ScriptRunner::discover(dir.path());
        assert_eq!(scripts.len(), 2);
    }

    #[test]
    fn test_search_scripts() {
        let runner = ScriptRunner::new();
        runner.set_scripts(vec![
            ScriptEntry { name: "Deploy Staging".into(), path: "/scripts/deploy.sh".into(), icon: None, description: None },
            ScriptEntry { name: "Backup DB".into(), path: "/scripts/backup.sh".into(), icon: None, description: None },
        ]);
        let results = runner.search("deploy", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Deploy"));
    }
}
```

- [ ] **Step 2: Add `tempfile` to dev-dependencies**

- [ ] **Step 3: Implement ScriptRunner**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ScriptEntry {
    pub name: String,
    pub path: PathBuf,
    pub icon: Option<String>,
    pub description: Option<String>,
}

pub struct ScriptMetadata {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
}

pub struct ScriptRunner {
    scripts: Arc<RwLock<Vec<ScriptEntry>>>,
}

impl ScriptRunner {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_scripts(&self, scripts: Vec<ScriptEntry>) {
        *self.scripts.write() = scripts;
    }

    pub fn parse_metadata(content: &str) -> ScriptMetadata {
        let mut meta = ScriptMetadata { name: None, icon: None, description: None };
        for line in content.lines().take(5) {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("# name:") {
                meta.name = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("# icon:") {
                meta.icon = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("# description:") {
                meta.description = Some(rest.trim().to_string());
            }
        }
        meta
    }

    pub fn discover(dir: &Path) -> Vec<ScriptEntry> {
        let mut scripts = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return scripts,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            if !matches!(ext, Some("sh" | "applescript" | "scpt")) {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let meta = Self::parse_metadata(&content);
            let name = meta.name.unwrap_or_else(|| {
                path.file_stem().unwrap_or_default().to_string_lossy().to_string()
            });
            scripts.push(ScriptEntry {
                name,
                path,
                icon: meta.icon,
                description: meta.description,
            });
        }
        scripts
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let scripts = self.scripts.read();
        let query_lower = query.to_lowercase();

        let mut results: Vec<LauncherItem> = scripts
            .iter()
            .filter(|s| {
                query.is_empty()
                    || s.name.to_lowercase().contains(&query_lower)
                    || s.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&query_lower))
            })
            .map(|s| LauncherItem {
                id: format!("script:{}", s.path.display()),
                title: s.name.clone(),
                subtitle: s.description.clone(),
                icon: s.icon.clone().or_else(|| Some("file-code".to_string())),
                kind: LauncherItemKind::Script {
                    path: s.path.clone(),
                    name: s.name.clone(),
                },
                score: if s.name.to_lowercase().starts_with(&query_lower) { 1.0 } else { 0.7 },
            })
            .collect();

        results.truncate(limit);
        results
    }

    pub async fn execute(path: &Path) -> common::Result<String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let output = match ext {
            "applescript" | "scpt" => {
                tokio::process::Command::new("osascript")
                    .arg(path)
                    .output()
                    .await?
            }
            _ => {
                tokio::process::Command::new(path)
                    .output()
                    .await?
            }
        };
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(common::KlyntbotError::Internal(format!("Script failed: {}", stderr)))
        }
    }
}
```

- [ ] **Step 4: Add to search/mod.rs, run tests**

Run: `cargo nextest run -p feature-launcher -E 'test(script)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add script runner provider with metadata parsing"
```

---

## Chunk 3: Search Engine, App-Core Integration, Tauri Commands

### Task 3.1: Search engine in app-core

**Files:**
- Create: `crates/app-core/src/handlers/launcher/mod.rs`
- Create: `crates/app-core/src/handlers/launcher/search_engine.rs`
- Create: `crates/app-core/src/handlers/launcher/dashboard.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` (add `pub mod launcher`)
- Modify: `crates/app-core/Cargo.toml` (add `feature-launcher` dep)

- [ ] **Step 1: Add feature-launcher dependency to app-core**

In `crates/app-core/Cargo.toml`:
```toml
feature-launcher = { path = "../feature-launcher" }
```

- [ ] **Step 2: Create launcher handler module**

`handlers/launcher/mod.rs`:
```rust
pub mod search_engine;
pub mod dashboard;

pub use search_engine::LauncherSearchEngine;
pub use dashboard::build_dashboard_data;
```

- [ ] **Step 3: Implement LauncherSearchEngine**

`handlers/launcher/search_engine.rs`:
```rust
use feature_launcher::types::*;
use feature_launcher::search::{AppIndex, Calculator, SystemCommands, ScriptRunner};
use feature_launcher::repos::FrequencyRepo;

pub struct LauncherSearchEngine {
    pub app_index: AppIndex,
    pub script_runner: ScriptRunner,
    pub frequency_repo: FrequencyRepo,
    pub clipboard_repo: feature_launcher::repos::ClipboardRepo,
}

impl LauncherSearchEngine {
    /// Main search entry point — fans out to all providers
    pub async fn search(
        &self,
        query: &str,
        task_repo: &storage::repos::TaskRepo,
        note_repo: &storage::repos::NoteRepo,
        calendar_repo: Option<&feature_productivity::repos::CalendarEventRepo>,
    ) -> common::Result<Vec<LauncherItem>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        // Handle prefix routing
        if let Some(calc_query) = trimmed.strip_prefix('=') {
            return Ok(self.calc_only(calc_query));
        }
        if let Some(sys_query) = trimmed.strip_prefix('>') {
            return Ok(SystemCommands::search(sys_query.trim()));
        }
        if let Some(script_query) = trimmed.strip_prefix('/') {
            return Ok(self.script_runner.search(script_query.trim(), 20));
        }
        // @ prefix handled in frontend (goes directly to chat mode)

        // Universal search — fan out to all providers concurrently
        let (apps, system, scripts, calc, clipboard, tasks, notes, calendar) = tokio::join!(
            async { self.app_index.search(trimmed, 5) },
            async { SystemCommands::search(trimmed) },
            async { self.script_runner.search(trimmed, 5) },
            async { Calculator::try_eval(trimmed) },
            self.search_clipboard(trimmed),
            self.search_tasks(trimmed, task_repo),
            self.search_notes(trimmed, note_repo),
            self.search_calendar(trimmed, calendar_repo),
        );

        let mut results = Vec::with_capacity(32);

        // Calculator result first if present
        if let Some(calc_result) = calc {
            results.push(LauncherItem {
                id: format!("calc:{}", calc_result.expression),
                title: format!("{}", calc_result.result),
                subtitle: Some(calc_result.expression.clone()),
                icon: Some("calculator".to_string()),
                kind: LauncherItemKind::Calculator {
                    expression: calc_result.expression,
                    result: calc_result.result,
                },
                score: 2.0, // Always on top when present
            });
        }

        results.extend(apps);
        results.extend(system);
        results.extend(scripts);
        results.extend(clipboard.unwrap_or_default());
        results.extend(tasks.unwrap_or_default());
        results.extend(notes.unwrap_or_default());
        results.extend(calendar.unwrap_or_default());

        // Apply frequency boosts
        for item in &mut results {
            let kind_str = match &item.kind {
                LauncherItemKind::Application { .. } => "app",
                LauncherItemKind::Task { .. } => "task",
                LauncherItemKind::Note { .. } => "note",
                LauncherItemKind::SystemCommand { .. } => "command",
                LauncherItemKind::Script { .. } => "script",
                LauncherItemKind::ClipboardEntry { .. } => "clipboard",
                LauncherItemKind::Calculator { .. } => continue,
                LauncherItemKind::Calendar { .. } => "calendar",
                LauncherItemKind::AiChat { .. } => continue,
            };
            let boost = self.frequency_repo.get_boost(&item.id, kind_str).await.unwrap_or(0.0);
            item.score *= 1.0 + boost;
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(20);

        // Always add AI chat as last fallback
        results.push(LauncherItem {
            id: "ai-chat".to_string(),
            title: format!("Ask Klynt AI: {}", trimmed),
            subtitle: Some("Chat with your AI assistant".to_string()),
            icon: Some("sparkles".to_string()),
            kind: LauncherItemKind::AiChat { query: trimmed.to_string() },
            score: 0.0,
        });

        Ok(results)
    }

    fn calc_only(&self, query: &str) -> Vec<LauncherItem> {
        match Calculator::try_eval(query) {
            Some(r) => vec![LauncherItem {
                id: format!("calc:{}", r.expression),
                title: format!("{}", r.result),
                subtitle: Some(r.expression.clone()),
                icon: Some("calculator".to_string()),
                kind: LauncherItemKind::Calculator { expression: r.expression, result: r.result },
                score: 1.0,
            }],
            None => vec![],
        }
    }

    async fn search_clipboard(&self, query: &str) -> common::Result<Vec<LauncherItem>> {
        let entries = self.clipboard_repo.search(query, 5).await?;
        Ok(entries.into_iter().map(|e| {
            let content_type = match e.content_type.as_str() {
                "image" => ClipboardContentType::Image,
                "file" => ClipboardContentType::File,
                _ => ClipboardContentType::Text,
            };
            LauncherItem {
                id: format!("clipboard:{}", e.id),
                title: e.preview.clone().unwrap_or_else(|| e.content.chars().take(80).collect()),
                subtitle: e.source_app.clone().map(|a| format!("From {}", a)),
                icon: Some("clipboard".to_string()),
                kind: LauncherItemKind::ClipboardEntry { entry_id: e.id, content_type },
                score: 0.6,
            }
        }).collect())
    }

    async fn search_tasks(&self, query: &str, repo: &storage::repos::TaskRepo) -> common::Result<Vec<LauncherItem>> {
        // Use existing task search — implementation depends on TaskRepo API
        // This is a placeholder that will be adapted to the actual repo method
        let tasks = repo.search(query, 5).await?;
        Ok(tasks.into_iter().map(|t| LauncherItem {
            id: format!("task:{}", t.id),
            title: t.title.clone(),
            subtitle: t.project_name.clone(),
            icon: Some("check-square".to_string()),
            kind: LauncherItemKind::Task { task_id: t.id.clone(), status: t.status.clone() },
            score: 0.7,
        }).collect())
    }

    async fn search_notes(&self, query: &str, repo: &storage::repos::NoteRepo) -> common::Result<Vec<LauncherItem>> {
        let notes = repo.search(query, 5).await?;
        Ok(notes.into_iter().map(|n| {
            let preview: String = n.content.chars().take(100).collect();
            LauncherItem {
                id: format!("note:{}", n.id),
                title: n.title.clone().unwrap_or_else(|| preview.clone()),
                subtitle: Some(preview),
                icon: Some("file-text".to_string()),
                kind: LauncherItemKind::Note { note_id: n.id.clone(), preview: n.content.chars().take(100).collect() },
                score: 0.6,
            }
        }).collect())
    }

    async fn search_calendar(
        &self,
        query: &str,
        repo: Option<&feature_productivity::repos::CalendarEventRepo>,
    ) -> common::Result<Vec<LauncherItem>> {
        let repo = match repo {
            Some(r) => r,
            None => return Ok(vec![]),
        };
        let events = repo.search(query, 5).await?;
        Ok(events.into_iter().map(|e| LauncherItem {
            id: format!("calendar:{}", e.id),
            title: e.title.clone(),
            subtitle: Some(e.starts_at.clone()),
            icon: Some("calendar".to_string()),
            kind: LauncherItemKind::Calendar {
                event_id: e.id.clone(),
                starts_at: chrono::DateTime::parse_from_rfc3339(&e.starts_at)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            },
            score: 0.65,
        }).collect())
    }

    /// Record that the user selected an item (for frequency learning)
    pub async fn record_execution(&self, item_id: &str, kind: &str) -> common::Result<()> {
        self.frequency_repo.increment(item_id, kind).await
    }
}
```

**Note:** The `search_tasks`, `search_notes`, and `search_calendar` methods reference repo search methods that may need to be added. Check the actual `TaskRepo` and `NoteRepo` APIs during implementation. If `search()` doesn't exist, add a simple `LIKE`-based search method.

- [ ] **Step 4: Implement dashboard data builder**

`handlers/launcher/dashboard.rs`:
```rust
use feature_launcher::types::*;
use chrono::Utc;

pub async fn build_dashboard_data(
    focus_manager: Option<&feature_productivity::focus::FocusManager>,
    calendar_repo: Option<&feature_productivity::repos::CalendarEventRepo>,
    task_repo: &storage::repos::TaskRepo,
    productivity_repos: Option<&feature_productivity::repos::ProductivityRepos>,
) -> common::Result<DashboardData> {
    let focus = match focus_manager {
        Some(fm) => fm.get_active().await?.map(|s| FocusDashboard {
            task_name: s.task_name,
            elapsed_secs: (Utc::now() - chrono::DateTime::parse_from_rfc3339(&s.started_at)
                .unwrap_or_default().with_timezone(&Utc)).num_seconds(),
            target_secs: s.target_minutes.map(|m| m * 60),
            session_id: s.id,
        }),
        None => None,
    };

    let calendar = match calendar_repo {
        Some(repo) => {
            let now = Utc::now();
            let two_hours = now + chrono::Duration::hours(2);
            repo.list_range(&now.to_rfc3339(), &two_hours.to_rfc3339())
                .await
                .unwrap_or_default()
                .into_iter()
                .take(2)
                .map(|e| {
                    let starts = chrono::DateTime::parse_from_rfc3339(&e.starts_at)
                        .unwrap_or_default().with_timezone(&Utc);
                    let ends = chrono::DateTime::parse_from_rfc3339(&e.ends_at)
                        .unwrap_or_default().with_timezone(&Utc);
                    CalendarDashboard {
                        event_id: e.id,
                        title: e.title,
                        starts_at: starts,
                        ends_at: ends,
                        minutes_until: (starts - now).num_minutes(),
                    }
                })
                .collect()
        }
        None => vec![],
    };

    // Top 3 tasks by priority (incomplete, current project)
    let tasks = task_repo
        .list_top_priority(3)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| TaskDashboard {
            id: t.id,
            title: t.title,
            status: t.status,
            project_name: t.project_name,
        })
        .collect();

    let productivity = match productivity_repos {
        Some(repos) => {
            let today = Utc::now().format("%Y-%m-%d").to_string();
            let summary = repos.daily_summary().get_by_date(&today).await.ok().flatten();
            match summary {
                Some(s) => ProductivityDashboard {
                    total_minutes: s.total_minutes,
                    top_category: s.top_category.unwrap_or_else(|| "None".to_string()),
                    top_category_pct: s.top_category_pct.unwrap_or(0.0),
                    score: s.score.unwrap_or(0),
                },
                None => ProductivityDashboard {
                    total_minutes: 0,
                    top_category: "None".to_string(),
                    top_category_pct: 0.0,
                    score: 0,
                },
            }
        }
        None => ProductivityDashboard {
            total_minutes: 0,
            top_category: "None".to_string(),
            top_category_pct: 0.0,
            score: 0,
        },
    };

    Ok(DashboardData {
        focus,
        calendar,
        tasks,
        productivity,
    })
}
```

**Note:** The exact repo method names (`list_range`, `list_top_priority`, `get_by_date`) need to be verified against actual repo APIs. Adapt during implementation — the pattern is correct but method names may differ.

- [ ] **Step 5: Add `pub mod launcher` to handlers/mod.rs**

- [ ] **Step 6: Run `cargo build -p app-core`**

Expected: Compiles (may need to add missing search/list methods to repos — see note above).

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/ crates/feature-launcher/
git commit -m "feat(launcher): add LauncherSearchEngine and dashboard builder in app-core"
```

### Task 3.2: Add launcher fields to AppCore + initialization

**Files:**
- Modify: `crates/app-core/src/state.rs` (add launcher fields)
- Create: `crates/app-core/src/init/launcher.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add fields to AppCore**

In `state.rs`, add to `AppCore` struct:
```rust
// Launcher
pub launcher_search_engine: Option<Arc<handlers::launcher::LauncherSearchEngine>>,
pub launcher_app_index: Option<feature_launcher::search::AppIndex>,
pub launcher_script_runner: Option<feature_launcher::search::ScriptRunner>,
pub launcher_clipboard_repo: Option<feature_launcher::repos::ClipboardRepo>,
```

- [ ] **Step 2: Create init/launcher.rs**

```rust
use crate::handlers::launcher::LauncherSearchEngine;
use feature_launcher::{LauncherFeature, search::{AppIndex, ScriptRunner}, repos::{FrequencyRepo, ClipboardRepo}};
use storage::StoragePool;
use std::sync::Arc;

pub struct LauncherResult {
    pub search_engine: Arc<LauncherSearchEngine>,
    pub app_index: AppIndex,
    pub script_runner: ScriptRunner,
    pub clipboard_repo: ClipboardRepo,
}

pub async fn init_launcher(
    config: &config::Config,
    storage_pool: &StoragePool,
) -> common::Result<Option<LauncherResult>> {
    // Check if launcher feature is enabled
    let launcher_config = config.get_feature_config("launcher");
    let enabled = launcher_config
        .and_then(|c| c.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if !enabled {
        return Ok(None);
    }

    // Run migrations
    StoragePool::run_feature_migrations(
        storage_pool.inner(),
        &LauncherFeature::migrations_static(),
    ).await?;

    let pool = storage_pool.inner().clone();
    let frequency_repo = FrequencyRepo::new(pool.clone());
    let clipboard_repo = ClipboardRepo::new(pool.clone());
    let app_index = AppIndex::new();
    let script_runner = ScriptRunner::new();

    let search_engine = Arc::new(LauncherSearchEngine {
        app_index: app_index.clone(),
        script_runner: script_runner.clone(),
        frequency_repo,
        clipboard_repo: clipboard_repo.clone(),
    });

    // Start background indexing
    let idx = app_index.clone();
    tokio::spawn(async move {
        idx.index_applications().await;
    });

    // Discover scripts
    let scripts_dir = launcher_config
        .and_then(|c| c.get("scriptsDir"))
        .and_then(|v| v.as_str())
        .unwrap_or("~/.klyntbot/scripts");
    let scripts_dir = shellexpand::tilde(scripts_dir).to_string();
    let sr = script_runner.clone();
    let sd = scripts_dir.clone();
    tokio::spawn(async move {
        let scripts = ScriptRunner::discover(std::path::Path::new(&sd));
        sr.set_scripts(scripts);
    });

    Ok(Some(LauncherResult {
        search_engine,
        app_index,
        script_runner,
        clipboard_repo,
    }))
}
```

- [ ] **Step 3: Wire into init/mod.rs — call init_launcher during AppCore construction**

Add `pub mod launcher;` and call `init_launcher` in the main init sequence.

- [ ] **Step 4: Add AppCore handler methods**

In `state.rs` or a new `handlers/launcher/handlers.rs`, add methods on `AppCore`:

```rust
impl AppCore {
    pub async fn launcher_search(&self, query: String) -> Result<Vec<LauncherItem>, ApiError> {
        let engine = self.launcher_search_engine.as_ref()
            .ok_or(ApiError::feature_disabled("launcher"))?;
        let calendar_repo = self.productivity_repos.as_ref().map(|r| r.calendar_event());
        engine.search(&query, &self.repos.tasks, &self.note_repo, calendar_repo.as_ref())
            .await
            .map_err(ApiError::internal)
    }

    pub async fn launcher_dashboard(&self) -> Result<DashboardData, ApiError> {
        handlers::launcher::build_dashboard_data(
            self.focus_manager.as_deref(),
            self.productivity_repos.as_ref().map(|r| r.calendar_event()).as_ref(),
            &self.repos.tasks,
            self.productivity_repos.as_ref(),
        )
        .await
        .map_err(ApiError::internal)
    }

    pub async fn launcher_execute(&self, item_id: String, kind: String) -> Result<(), ApiError> {
        if let Some(engine) = &self.launcher_search_engine {
            engine.record_execution(&item_id, &kind).await.map_err(ApiError::internal)?;
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Run `cargo build -p app-core`**

Expected: Compiles

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/
git commit -m "feat(launcher): wire LauncherSearchEngine into AppCore with init"
```

### Task 3.3: Tauri commands

**Files:**
- Create: `crates/desktop/src/commands/launcher.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs` (register commands)

- [ ] **Step 1: Create launcher Tauri commands**

```rust
use crate::AppCore;
use feature_launcher::types::*;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn launcher_search(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<LauncherItem>, crate::ApiError> {
    state.launcher_search(query).await
}

#[tauri::command]
pub async fn launcher_execute(
    state: State<'_, Arc<AppCore>>,
    item_id: String,
    kind: String,
) -> Result<(), crate::ApiError> {
    state.launcher_execute(item_id, kind).await
}

#[tauri::command]
pub async fn launcher_dashboard(
    state: State<'_, Arc<AppCore>>,
) -> Result<DashboardData, crate::ApiError> {
    state.launcher_dashboard().await
}

#[tauri::command]
pub async fn launcher_clipboard_paste(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), crate::ApiError> {
    let repo = state.launcher_clipboard_repo.as_ref()
        .ok_or(crate::ApiError::feature_disabled("launcher"))?;
    let _entry = repo.get(id).await.map_err(crate::ApiError::internal)?
        .ok_or(crate::ApiError::not_found("clipboard entry"))?;
    // TODO: paste via CGEvent in a future task
    Ok(())
}

#[tauri::command]
pub async fn launcher_clipboard_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), crate::ApiError> {
    let repo = state.launcher_clipboard_repo.as_ref()
        .ok_or(crate::ApiError::feature_disabled("launcher"))?;
    repo.delete(id).await.map_err(crate::ApiError::internal)
}

#[tauri::command]
pub async fn launcher_clipboard_pin(
    state: State<'_, Arc<AppCore>>,
    id: i64,
    pinned: bool,
) -> Result<(), crate::ApiError> {
    let repo = state.launcher_clipboard_repo.as_ref()
        .ok_or(crate::ApiError::feature_disabled("launcher"))?;
    repo.pin(id, pinned).await.map_err(crate::ApiError::internal)
}

#[tauri::command]
pub async fn launcher_window_action(
    _state: State<'_, Arc<AppCore>>,
    action: WindowAction,
) -> Result<(), crate::ApiError> {
    // TODO: implement in window management task
    tracing::info!("Window action: {:?}", action);
    Ok(())
}

#[tauri::command]
pub async fn launcher_run_script(
    _state: State<'_, Arc<AppCore>>,
    path: String,
) -> Result<String, crate::ApiError> {
    feature_launcher::search::ScriptRunner::execute(std::path::Path::new(&path))
        .await
        .map_err(crate::ApiError::internal)
}

#[tauri::command]
pub async fn launcher_system_command(
    _state: State<'_, Arc<AppCore>>,
    action: SystemAction,
) -> Result<(), crate::ApiError> {
    feature_launcher::search::SystemCommands::execute(&action)
        .await
        .map_err(crate::ApiError::internal)
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "launcher_search",
    "launcher_execute",
    "launcher_dashboard",
    "launcher_clipboard_paste",
    "launcher_clipboard_delete",
    "launcher_clipboard_pin",
    "launcher_window_action",
    "launcher_run_script",
    "launcher_system_command",
];
```

- [ ] **Step 2: Register in mod.rs and main.rs**

Add `pub mod launcher;` to `commands/mod.rs`. Add all launcher commands to the `invoke_handler` in `main.rs`. Add `launcher::DEV_COMMANDS` to the dev server coverage list.

- [ ] **Step 3: Run `cargo build -p desktop`**

Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/
git commit -m "feat(launcher): add Tauri launcher commands with DEV_COMMANDS"
```

---

## Chunk 4: Frontend — Store, Types, Input, Dashboard

### Task 4.1: Frontend types and store

**Files:**
- Create: `desktop-ui/src/features/launcher/types.ts`
- Create: `desktop-ui/src/features/launcher/stores/launcherStore.ts`

- [ ] **Step 1: Create types.ts**

```typescript
export type LauncherMode = "dashboard" | "search" | "detail" | "chat";

export interface LauncherItem {
  id: string;
  title: string;
  subtitle?: string;
  icon?: string;
  kind: LauncherItemKind;
  score: number;
}

export type LauncherItemKind =
  | { type: "application"; path: string; running: boolean }
  | { type: "task"; taskId: string; status: string }
  | { type: "note"; noteId: string; preview: string }
  | { type: "clipboardEntry"; entryId: number; contentType: "text" | "image" | "file" }
  | { type: "systemCommand"; action: string }
  | { type: "script"; path: string; name: string }
  | { type: "calculator"; expression: string; result: number }
  | { type: "calendar"; eventId: string; startsAt: string }
  | { type: "aiChat"; query: string };

export interface DashboardData {
  focus: FocusDashboard | null;
  calendar: CalendarDashboard[];
  tasks: TaskDashboard[];
  productivity: ProductivityDashboard;
}

export interface FocusDashboard {
  taskName: string | null;
  elapsedSecs: number;
  targetSecs: number | null;
  sessionId: string;
}

export interface CalendarDashboard {
  eventId: string;
  title: string;
  startsAt: string;
  endsAt: string;
  minutesUntil: number;
}

export interface TaskDashboard {
  id: string;
  title: string;
  status: string;
  projectName: string | null;
}

export interface ProductivityDashboard {
  totalMinutes: number;
  topCategory: string;
  topCategoryPct: number;
  score: number;
}
```

- [ ] **Step 2: Create launcherStore.ts**

```typescript
import { create } from "zustand";
import type { LauncherMode, LauncherItem, DashboardData } from "../types";

interface LauncherState {
  mode: LauncherMode;
  query: string;
  results: LauncherItem[];
  selectedIndex: number;
  dashboard: DashboardData | null;
  queryHistory: string[];
  historyIndex: number;

  setMode: (mode: LauncherMode) => void;
  setQuery: (query: string) => void;
  setResults: (results: LauncherItem[]) => void;
  setSelectedIndex: (index: number) => void;
  setDashboard: (data: DashboardData) => void;
  moveSelection: (delta: number) => void;
  pushHistory: (query: string) => void;
  navigateHistory: (direction: "up" | "down") => void;
  reset: () => void;
}

export const useLauncherStore = create<LauncherState>((set, get) => ({
  mode: "dashboard",
  query: "",
  results: [],
  selectedIndex: 0,
  dashboard: null,
  queryHistory: [],
  historyIndex: -1,

  setMode: (mode) => set({ mode }),
  setQuery: (query) => {
    const mode = query.length > 0 ? "search" : "dashboard";
    set({ query, mode, selectedIndex: 0, historyIndex: -1 });
  },
  setResults: (results) => set({ results }),
  setSelectedIndex: (index) => set({ selectedIndex: index }),
  setDashboard: (data) => set({ dashboard: data }),
  moveSelection: (delta) => {
    const { selectedIndex, results } = get();
    const next = Math.max(0, Math.min(results.length - 1, selectedIndex + delta));
    set({ selectedIndex: next });
  },
  pushHistory: (query) => {
    if (!query.trim()) return;
    const { queryHistory } = get();
    // Deduplicate consecutive entries
    if (queryHistory[queryHistory.length - 1] === query) return;
    set({ queryHistory: [...queryHistory, query].slice(-50) });
  },
  navigateHistory: (direction) => {
    const { queryHistory, historyIndex } = get();
    if (queryHistory.length === 0) return;
    const newIndex =
      direction === "up"
        ? Math.min(historyIndex + 1, queryHistory.length - 1)
        : Math.max(historyIndex - 1, -1);
    if (newIndex === -1) {
      set({ historyIndex: -1, query: "" });
    } else {
      const idx = queryHistory.length - 1 - newIndex;
      set({ historyIndex: newIndex, query: queryHistory[idx] });
    }
  },
  reset: () =>
    set({
      mode: "dashboard",
      query: "",
      results: [],
      selectedIndex: 0,
      historyIndex: -1,
    }),
}));
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/
git commit -m "feat(launcher): add frontend types and Zustand store"
```

### Task 4.2: Launcher hooks

**Files:**
- Create: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`
- Create: `desktop-ui/src/features/launcher/hooks/useDashboardData.ts`

- [ ] **Step 1: Create useLauncherSearch hook**

```typescript
import { useCallback, useEffect, useRef } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

export function useLauncherSearch() {
  const query = useLauncherStore((s) => s.query);
  const setResults = useLauncherStore((s) => s.setResults);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  const search = useCallback(
    async (q: string) => {
      if (!q.trim() || q.startsWith("@")) {
        setResults([]);
        return;
      }
      try {
        const results = await ipc<LauncherItem[]>("launcher_search", { query: q });
        setResults(results);
      } catch (err) {
        console.error("Launcher search failed:", err);
        setResults([]);
      }
    },
    [setResults],
  );

  useEffect(() => {
    clearTimeout(timerRef.current);
    if (!query.trim()) {
      setResults([]);
      return;
    }
    timerRef.current = setTimeout(() => search(query), 50);
    return () => clearTimeout(timerRef.current);
  }, [query, search, setResults]);
}
```

- [ ] **Step 2: Create useDashboardData hook**

```typescript
import { useCallback, useEffect } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { useLauncherStore } from "../stores/launcherStore";
import type { DashboardData } from "../types";

export function useDashboardData() {
  const setDashboard = useLauncherStore((s) => s.setDashboard);

  const fetchDashboard = useCallback(async () => {
    try {
      const data = await ipc<DashboardData>("launcher_dashboard");
      setDashboard(data);
    } catch (err) {
      console.error("Dashboard fetch failed:", err);
    }
  }, [setDashboard]);

  // Fetch on mount + listen for Tauri events
  useEffect(() => {
    fetchDashboard();

    // Subscribe to update events
    let unlisteners: Array<() => void> = [];

    if (typeof window !== "undefined" && "__TAURI__" in window) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        const events = [
          "launcher:focus_update",
          "launcher:calendar_update",
          "launcher:tasks_update",
          "launcher:productivity_update",
        ];
        events.forEach((event) => {
          listen<DashboardData>(event, (e) => {
            setDashboard(e.payload);
          }).then((unlisten) => unlisteners.push(unlisten));
        });
      });
    }

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [fetchDashboard, setDashboard]);

  return { refetch: fetchDashboard };
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/
git commit -m "feat(launcher): add search and dashboard data hooks"
```

### Task 4.3: Dashboard widgets

**Files:**
- Create: `desktop-ui/src/features/launcher/components/Dashboard.tsx`
- Create: `desktop-ui/src/features/launcher/components/DashboardFocus.tsx`
- Create: `desktop-ui/src/features/launcher/components/DashboardCalendar.tsx`
- Create: `desktop-ui/src/features/launcher/components/DashboardTasks.tsx`
- Create: `desktop-ui/src/features/launcher/components/DashboardProductivity.tsx`

- [ ] **Step 1: Create DashboardFocus.tsx**

```tsx
import { Timer } from "lucide-react";
import type { FocusDashboard } from "../types";

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function DashboardFocus({ focus }: { focus: FocusDashboard }) {
  return (
    <div className="flex items-center gap-3 px-4 py-2.5 border-b border-border">
      <Timer size={16} className="text-brand shrink-0" />
      <span className="text-[13px] text-primary truncate flex-1">
        {focus.taskName ?? "Focus session"}
      </span>
      <span className="text-[13px] font-mono text-muted">
        {formatDuration(focus.elapsedSecs)}
      </span>
    </div>
  );
}
```

- [ ] **Step 2: Create DashboardCalendar.tsx**

```tsx
import { Calendar } from "lucide-react";
import type { CalendarDashboard } from "../types";

export function DashboardCalendar({ events }: { events: CalendarDashboard[] }) {
  if (events.length === 0) return null;
  return (
    <div className="border-b border-border">
      {events.map((event) => (
        <div key={event.eventId} className="flex items-center gap-3 px-4 py-2">
          <Calendar size={14} className="text-muted shrink-0" />
          <span className="text-[13px] text-primary truncate flex-1">{event.title}</span>
          <span className="text-[12px] text-muted">
            {event.minutesUntil <= 0
              ? "now"
              : event.minutesUntil < 60
                ? `in ${event.minutesUntil}m`
                : new Date(event.startsAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
          </span>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Create DashboardTasks.tsx**

```tsx
import { CheckSquare } from "lucide-react";
import type { TaskDashboard } from "../types";

export function DashboardTasks({ tasks }: { tasks: TaskDashboard[] }) {
  if (tasks.length === 0) return null;
  return (
    <div className="border-b border-border">
      {tasks.map((task) => (
        <div key={task.id} className="flex items-center gap-3 px-4 py-2">
          <CheckSquare size={14} className="text-muted shrink-0" />
          <span className="text-[13px] text-primary truncate flex-1">{task.title}</span>
          {task.projectName && (
            <span className="text-[11px] text-dim">{task.projectName}</span>
          )}
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Create DashboardProductivity.tsx**

```tsx
import type { ProductivityDashboard } from "../types";

function scoreColor(score: number): string {
  if (score >= 70) return "text-success";
  if (score >= 40) return "text-warning";
  return "text-destructive";
}

function formatMinutes(mins: number): string {
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h === 0) return `${m}m`;
  return `${h}h ${m}m`;
}

export function DashboardProductivity({ data }: { data: ProductivityDashboard }) {
  return (
    <div className="flex items-center gap-3 px-4 py-2 text-[12px] text-muted">
      <span>Today: {formatMinutes(data.totalMinutes)}</span>
      <span className="text-dim">·</span>
      <span>{data.topCategory} {Math.round(data.topCategoryPct)}%</span>
      <span className="text-dim">·</span>
      <span className={scoreColor(data.score)}>{data.score}</span>
    </div>
  );
}
```

- [ ] **Step 5: Create Dashboard.tsx container**

```tsx
import { useLauncherStore } from "../stores/launcherStore";
import { DashboardFocus } from "./DashboardFocus";
import { DashboardCalendar } from "./DashboardCalendar";
import { DashboardTasks } from "./DashboardTasks";
import { DashboardProductivity } from "./DashboardProductivity";

export function Dashboard() {
  const dashboard = useLauncherStore((s) => s.dashboard);
  if (!dashboard) return null;

  return (
    <div className="flex flex-col">
      {dashboard.focus && <DashboardFocus focus={dashboard.focus} />}
      <DashboardCalendar events={dashboard.calendar} />
      <DashboardTasks tasks={dashboard.tasks} />
      <DashboardProductivity data={dashboard.productivity} />
    </div>
  );
}
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/launcher/components/
git commit -m "feat(launcher): add dashboard widget components"
```

### Task 4.4: Search results UI

**Files:**
- Create: `desktop-ui/src/features/launcher/components/ResultItem.tsx`
- Create: `desktop-ui/src/features/launcher/components/ResultsList.tsx`

- [ ] **Step 1: Create ResultItem.tsx**

```tsx
import {
  AppWindow, CheckSquare, FileText, Clipboard, Terminal,
  FileCode, Calculator, Calendar, Sparkles,
} from "lucide-react";
import type { LauncherItem } from "../types";

const ICONS: Record<string, React.ComponentType<{ size?: number; className?: string }>> = {
  "app-window": AppWindow,
  "check-square": CheckSquare,
  "file-text": FileText,
  clipboard: Clipboard,
  terminal: Terminal,
  "file-code": FileCode,
  calculator: Calculator,
  calendar: Calendar,
  sparkles: Sparkles,
};

interface Props {
  item: LauncherItem;
  selected: boolean;
  onSelect: () => void;
  onMouseEnter: () => void;
}

export function ResultItem({ item, selected, onSelect, onMouseEnter }: Props) {
  const Icon = ICONS[item.icon ?? ""] ?? Terminal;

  return (
    <div
      className={`flex items-center gap-3 px-4 py-2 cursor-default transition-colors ${
        selected ? "bg-brand/10" : "hover:bg-surface-hover"
      }`}
      onClick={onSelect}
      onMouseEnter={onMouseEnter}
    >
      <Icon size={16} className={selected ? "text-brand" : "text-muted"} />
      <div className="flex flex-col flex-1 min-w-0">
        <span className="text-[13px] text-primary truncate">{item.title}</span>
        {item.subtitle && (
          <span className="text-[11px] text-dim truncate">{item.subtitle}</span>
        )}
      </div>
      {item.kind.type === "calculator" && (
        <span className="text-[11px] text-muted">⏎ copy</span>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create ResultsList.tsx**

```tsx
import { useLauncherStore } from "../stores/launcherStore";
import { ResultItem } from "./ResultItem";

interface Props {
  onExecute: (index: number) => void;
}

export function ResultsList({ onExecute }: Props) {
  const results = useLauncherStore((s) => s.results);
  const selectedIndex = useLauncherStore((s) => s.selectedIndex);
  const setSelectedIndex = useLauncherStore((s) => s.setSelectedIndex);

  if (results.length === 0) {
    return (
      <div className="px-4 py-6 text-center text-[13px] text-dim">
        No results found
      </div>
    );
  }

  return (
    <div className="max-h-[440px] overflow-y-auto">
      {results.map((item, index) => (
        <ResultItem
          key={item.id}
          item={item}
          selected={index === selectedIndex}
          onSelect={() => onExecute(index)}
          onMouseEnter={() => setSelectedIndex(index)}
        />
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/components/
git commit -m "feat(launcher): add search results list and result item components"
```

---

## Chunk 5: Frontend — LauncherPage, Input, Keyboard Navigation

### Task 5.1: LauncherInput component

**Files:**
- Create: `desktop-ui/src/features/launcher/components/LauncherInput.tsx`

- [ ] **Step 1: Create LauncherInput.tsx**

```tsx
import { Search } from "lucide-react";
import { useRef, useEffect } from "react";
import { useLauncherStore } from "../stores/launcherStore";

export function LauncherInput() {
  const query = useLauncherStore((s) => s.query);
  const setQuery = useLauncherStore((s) => s.setQuery);
  const inputRef = useRef<HTMLInputElement>(null);

  // Always focus the input when the launcher shows
  useEffect(() => {
    inputRef.current?.focus();
    const handleFocus = () => inputRef.current?.focus();
    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, []);

  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b border-border">
      <Search size={16} className="text-muted shrink-0" />
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search or type a command..."
        className="flex-1 bg-transparent text-[14px] text-primary placeholder:text-dim outline-none"
        spellCheck={false}
        autoComplete="off"
      />
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/launcher/components/LauncherInput.tsx
git commit -m "feat(launcher): add LauncherInput component"
```

### Task 5.2: Keyboard navigation hook

**Files:**
- Create: `desktop-ui/src/features/launcher/hooks/useKeyboardNavigation.ts`

- [ ] **Step 1: Create useKeyboardNavigation.ts**

```typescript
import { useCallback, useEffect } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

interface Options {
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
}

export function useKeyboardNavigation({ onEnterChat, onExpandToMain, onHide }: Options) {
  const mode = useLauncherStore((s) => s.mode);
  const query = useLauncherStore((s) => s.query);
  const results = useLauncherStore((s) => s.results);
  const selectedIndex = useLauncherStore((s) => s.selectedIndex);
  const setQuery = useLauncherStore((s) => s.setQuery);
  const setMode = useLauncherStore((s) => s.setMode);
  const moveSelection = useLauncherStore((s) => s.moveSelection);
  const navigateHistory = useLauncherStore((s) => s.navigateHistory);
  const pushHistory = useLauncherStore((s) => s.pushHistory);
  const reset = useLauncherStore((s) => s.reset);

  const executeItem = useCallback(
    async (item: LauncherItem) => {
      const kindStr = item.kind.type;
      // Record execution for frequency learning
      ipc("launcher_execute", { itemId: item.id, kind: kindStr }).catch(() => {});
      pushHistory(query);

      switch (item.kind.type) {
        case "application":
          ipc("launcher_run_script", { path: `open -a "${item.kind.path}"` }).catch(() => {});
          onHide();
          break;
        case "systemCommand":
          ipc("launcher_system_command", { action: item.kind.action }).catch(() => {});
          onHide();
          break;
        case "script":
          ipc("launcher_run_script", { path: item.kind.path }).catch(() => {});
          onHide();
          break;
        case "calculator":
          navigator.clipboard.writeText(String(item.kind.result));
          onHide();
          break;
        case "aiChat":
          onEnterChat(item.kind.query);
          break;
        case "clipboardEntry":
          ipc("launcher_clipboard_paste", { id: item.kind.entryId }).catch(() => {});
          onHide();
          break;
        case "task":
        case "note":
        case "calendar":
          // Open in main window
          onExpandToMain();
          break;
      }
    },
    [query, pushHistory, onEnterChat, onExpandToMain, onHide],
  );

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // ⌘/ — expand to main window
      if (e.metaKey && e.key === "/") {
        e.preventDefault();
        onExpandToMain();
        return;
      }

      if (mode === "dashboard" || mode === "search") {
        // Arrow navigation
        if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "j")) {
          e.preventDefault();
          moveSelection(1);
          return;
        }
        if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "k")) {
          e.preventDefault();
          if (mode === "dashboard" || (mode === "search" && selectedIndex === 0 && !query)) {
            navigateHistory("up");
          } else {
            moveSelection(-1);
          }
          return;
        }

        // Enter — execute selected
        if (e.key === "Enter") {
          e.preventDefault();
          if (mode === "search" && results[selectedIndex]) {
            executeItem(results[selectedIndex]);
          } else if (mode === "dashboard" && query.trim()) {
            // If @ prefix, go to chat
            if (query.startsWith("@")) {
              onEnterChat(query.slice(1).trim());
            }
          }
          return;
        }

        // Escape — layered dismissal
        if (e.key === "Escape") {
          e.preventDefault();
          if (query) {
            setQuery("");
          } else {
            onHide();
          }
          return;
        }
      }

      if (mode === "chat") {
        if (e.key === "Escape") {
          e.preventDefault();
          setMode("dashboard");
          setQuery("");
          return;
        }
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [
    mode, query, results, selectedIndex,
    moveSelection, navigateHistory, setQuery, setMode,
    executeItem, onEnterChat, onExpandToMain, onHide, reset,
  ]);
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/useKeyboardNavigation.ts
git commit -m "feat(launcher): add keyboard navigation hook with vim bindings"
```

### Task 5.3: Rewrite LauncherPage

**Files:**
- Modify: `desktop-ui/src/features/tray/pages/LauncherPage.tsx` (full rewrite)
- Move: `desktop-ui/src/features/tray/components/LauncherChat.tsx` → `desktop-ui/src/features/launcher/components/LauncherChat.tsx`

- [ ] **Step 1: Move LauncherChat.tsx**

Copy `features/tray/components/LauncherChat.tsx` to `features/launcher/components/LauncherChat.tsx`. Update imports to use `@features/chat/*` paths (already uses these). Delete the orphaned `features/chat/pages/LauncherChatPage.tsx`.

- [ ] **Step 2: Rewrite LauncherPage.tsx**

Replace the contents of `features/tray/pages/LauncherPage.tsx` with:

```tsx
import { useRef, useCallback, useState } from "react";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { emit } from "@tauri-apps/api/event";
import { isTauri } from "@shared/lib/utils";

import { useLauncherStore } from "@features/launcher/stores/launcherStore";
import { useLauncherSearch } from "@features/launcher/hooks/useLauncherSearch";
import { useDashboardData } from "@features/launcher/hooks/useDashboardData";
import { useKeyboardNavigation } from "@features/launcher/hooks/useKeyboardNavigation";
import { LauncherInput } from "@features/launcher/components/LauncherInput";
import { Dashboard } from "@features/launcher/components/Dashboard";
import { ResultsList } from "@features/launcher/components/ResultsList";
import { LauncherChat } from "@features/launcher/components/LauncherChat";

export default function LauncherPage() {
  const contentRef = useRef<HTMLDivElement>(null);
  const mode = useLauncherStore((s) => s.mode);
  const setMode = useLauncherStore((s) => s.setMode);
  const reset = useLauncherStore((s) => s.reset);

  const [chatSessionKey, setChatSessionKey] = useState("");
  const [chatInitialQuery, setChatInitialQuery] = useState<string | null>(null);

  useTransparentBackground({ nativeVibrancy: true });
  useWindowAutoResize(contentRef, { width: 660, maxHeight: 680 });
  useLauncherSearch();
  useDashboardData();

  const hideWindow = useCallback(async () => {
    if (isTauri) {
      await getCurrentWindow().hide();
    }
    reset();
  }, [reset]);

  const enterChat = useCallback(
    (query: string) => {
      setChatSessionKey(`launcher-${Date.now()}`);
      setChatInitialQuery(query);
      setMode("chat");
    },
    [setMode],
  );

  const expandToMain = useCallback(async () => {
    if (!isTauri) return;
    const mainWindow = await WebviewWindow.getByLabel("main");
    if (mainWindow) {
      await mainWindow.show();
      await mainWindow.setFocus();
      if (chatSessionKey) {
        await emit("open-chat", { sessionKey: chatSessionKey });
      }
    }
    await getCurrentWindow().hide();
    reset();
  }, [chatSessionKey, reset]);

  useKeyboardNavigation({
    onEnterChat: enterChat,
    onExpandToMain: expandToMain,
    onHide: hideWindow,
  });

  const executeItem = useCallback(
    (index: number) => {
      const results = useLauncherStore.getState().results;
      const item = results[index];
      if (!item) return;
      if (item.kind.type === "aiChat") {
        enterChat(item.kind.query);
      }
      // Other executions handled by keyboard nav hook
    },
    [enterChat],
  );

  return (
    <div ref={contentRef} className="glass-floating rounded-2xl overflow-hidden">
      {mode === "chat" ? (
        <LauncherChat
          sessionKey={chatSessionKey}
          initialQuery={chatInitialQuery}
          onBack={() => {
            setMode("dashboard");
            reset();
          }}
          onExpand={expandToMain}
        />
      ) : (
        <>
          <LauncherInput />
          {mode === "dashboard" && <Dashboard />}
          {mode === "search" && <ResultsList onExecute={executeItem} />}
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Update router if needed**

The route `/launcher` should still point to `LauncherPage`. Verify in `app/router.tsx` and update the import path if it changed.

- [ ] **Step 4: Run `bun run build` from desktop-ui to verify it compiles**

Run: `cd desktop-ui && bun run build`
Expected: Builds successfully

- [ ] **Step 5: Run `bun run lint:fix`**

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/
git commit -m "feat(launcher): rewrite LauncherPage with dashboard, search, and chat modes"
```

---

## Chunk 6: Window Management (native macOS)

### Task 6.1: Window management with AXUIElement

**Files:**
- Create: `crates/feature-launcher/src/window_mgmt/mod.rs`
- Create: `crates/feature-launcher/src/window_mgmt/accessibility.rs`
- Create: `crates/feature-launcher/src/window_mgmt/actions.rs`

- [ ] **Step 1: Implement accessibility.rs — AXUIElement FFI wrappers**

Follow the same raw FFI approach as `crates/feature-productivity/src/tracker/macos.rs`. Create wrappers for:
- `get_frontmost_window() -> Option<AXWindow>`
- `AXWindow::get_position() -> (f64, f64)`
- `AXWindow::get_size() -> (f64, f64)`
- `AXWindow::set_position(x, y)`
- `AXWindow::set_size(w, h)`
- `get_screen_frame() -> (f64, f64, f64, f64)` (using `CGMainDisplayID` + `CGDisplayBounds`)

- [ ] **Step 2: Implement actions.rs — snap commands with cycling**

```rust
use crate::types::WindowAction;
use std::collections::HashMap;
use std::time::Instant;
use parking_lot::Mutex;

struct LastAction {
    action: WindowAction,
    timestamp: Instant,
    cycle_index: usize,
}

pub struct WindowManager {
    last_actions: Mutex<HashMap<u32, LastAction>>, // window_id -> last action
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            last_actions: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn execute(&self, action: &WindowAction) -> common::Result<()> {
        use super::accessibility;

        let window = accessibility::get_frontmost_window()
            .ok_or(common::KlyntbotError::Internal("No frontmost window".into()))?;
        let screen = accessibility::get_screen_frame();
        let window_id = window.id();

        // Check for cycling (same action within 2s, same window)
        let cycle_index = {
            let mut last = self.last_actions.lock();
            let entry = last.get(&window_id);
            let idx = if let Some(prev) = entry {
                if std::mem::discriminant(&prev.action) == std::mem::discriminant(action)
                    && prev.timestamp.elapsed().as_secs() < 2
                {
                    (prev.cycle_index + 1) % 3
                } else {
                    0
                }
            } else {
                0
            };
            last.insert(window_id, LastAction {
                action: action.clone(),
                timestamp: Instant::now(),
                cycle_index: idx,
            });
            idx
        };

        let (x, y, w, h) = self.compute_frame(action, &screen, cycle_index);
        window.set_position(x, y);
        window.set_size(w, h);
        Ok(())
    }

    fn compute_frame(
        &self,
        action: &WindowAction,
        screen: &(f64, f64, f64, f64), // x, y, width, height
        cycle: usize,
    ) -> (f64, f64, f64, f64) {
        let (sx, sy, sw, sh) = *screen;
        let fractions = [0.5, 1.0 / 3.0, 2.0 / 3.0];
        let frac = fractions[cycle];

        match action {
            WindowAction::LeftHalf => (sx, sy, sw * frac, sh),
            WindowAction::RightHalf => (sx + sw * (1.0 - frac), sy, sw * frac, sh),
            WindowAction::TopHalf => (sx, sy, sw, sh * frac),
            WindowAction::BottomHalf => (sx, sy + sh * (1.0 - frac), sw, sh * frac),
            WindowAction::LeftThird => (sx, sy, sw / 3.0, sh),
            WindowAction::CenterThird => (sx + sw / 3.0, sy, sw / 3.0, sh),
            WindowAction::RightThird => (sx + sw * 2.0 / 3.0, sy, sw / 3.0, sh),
            WindowAction::Maximize => (sx, sy, sw, sh),
            WindowAction::Center => {
                let cw = sw * 0.6;
                let ch = sh * 0.7;
                (sx + (sw - cw) / 2.0, sy + (sh - ch) / 2.0, cw, ch)
            }
            WindowAction::Restore => {
                // TODO: track previous frame for restore
                let cw = sw * 0.6;
                let ch = sh * 0.7;
                (sx + (sw - cw) / 2.0, sy + (sh - ch) / 2.0, cw, ch)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn execute(&self, _action: &WindowAction) -> common::Result<()> {
        Err(common::KlyntbotError::Internal("Window management only supported on macOS".into()))
    }
}
```

- [ ] **Step 3: Create mod.rs**

```rust
#[cfg(target_os = "macos")]
pub mod accessibility;
pub mod actions;
pub use actions::WindowManager;
```

Add `pub mod window_mgmt;` to `lib.rs`.

- [ ] **Step 4: Wire WindowManager into AppCore and Tauri command**

Update `launcher_window_action` in `commands/launcher.rs` to use the actual `WindowManager`.

- [ ] **Step 5: Run `cargo build -p feature-launcher`**

Expected: Compiles

- [ ] **Step 6: Commit**

```bash
git add crates/feature-launcher/src/window_mgmt/
git commit -m "feat(launcher): add window management with AXUIElement and cycling"
```

---

## Chunk 7: Clipboard Monitoring (native macOS)

### Task 7.1: Clipboard monitor background task

**Files:**
- Create: `crates/feature-launcher/src/clipboard/monitor.rs`
- Modify: `crates/feature-launcher/src/clipboard/mod.rs`

- [ ] **Step 1: Implement clipboard monitor**

```rust
use crate::repos::ClipboardRepo;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;

pub struct ClipboardMonitor {
    repo: ClipboardRepo,
    max_entries: i64,
}

impl ClipboardMonitor {
    pub fn new(repo: ClipboardRepo, max_entries: i64) -> Self {
        Self { repo, max_entries }
    }

    /// Start monitoring clipboard changes. Runs until cancellation token is triggered.
    #[cfg(target_os = "macos")]
    pub async fn start(&self, cancel: CancellationToken) {
        use objc2::rc::Retained;
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::NSString;

        let mut last_change_count: i64 = -1;
        let mut tick = interval(Duration::from_millis(500));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tick.tick() => {
                    let current_count = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.changeCount() as i64
                    };

                    if current_count != last_change_count && last_change_count != -1 {
                        if let Some(content) = self.read_pasteboard() {
                            let source = self.get_frontmost_app_name();
                            if let Err(e) = self.repo.insert(
                                &content,
                                "text",
                                source.as_deref(),
                                None,
                            ).await {
                                tracing::error!("Failed to store clipboard entry: {}", e);
                            }
                            // Evict old entries
                            let _ = self.repo.evict_to_max(self.max_entries).await;
                        }
                    }
                    last_change_count = current_count;
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn read_pasteboard(&self) -> Option<String> {
        unsafe {
            let pb = objc2_app_kit::NSPasteboard::generalPasteboard();
            let string_type = objc2_foundation::NSString::from_str("public.utf8-plain-text");
            pb.stringForType(&string_type).map(|s| s.to_string())
        }
    }

    #[cfg(target_os = "macos")]
    fn get_frontmost_app_name(&self) -> Option<String> {
        unsafe {
            let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
            workspace.frontmostApplication()
                .and_then(|app| app.localizedName().map(|n| n.to_string()))
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn start(&self, cancel: CancellationToken) {
        cancel.cancelled().await;
    }
}
```

**Note:** The exact `objc2` API may vary. During implementation, check the actual `objc2-app-kit` and `objc2-foundation` crate APIs for the right method signatures. The pattern above is correct but method names may need adjustment.

- [ ] **Step 2: Add objc2 dependencies to Cargo.toml**

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-app-kit = { version = "0.2", features = ["NSPasteboard", "NSWorkspace", "NSRunningApplication"] }
objc2-foundation = { version = "0.2", features = ["NSString"] }
tokio-util = { version = "0.7", features = ["rt"] }
```

- [ ] **Step 3: Create clipboard/mod.rs**

```rust
pub mod monitor;
pub use monitor::ClipboardMonitor;
```

Add `pub mod clipboard;` to `lib.rs`.

- [ ] **Step 4: Wire into LauncherService init — start clipboard monitoring task**

In `init/launcher.rs`, after creating the clipboard repo:
```rust
let clipboard_monitor = ClipboardMonitor::new(clipboard_repo.clone(), 1000);
let cancel = shutdown_token.clone();
tokio::spawn(async move {
    clipboard_monitor.start(cancel).await;
});
```

- [ ] **Step 5: Run `cargo build -p feature-launcher`**

Expected: Compiles on macOS

- [ ] **Step 6: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add clipboard monitoring background task"
```

---

## Chunk 8: Integration Testing & Polish

### Task 8.1: Add search methods to existing repos if missing

**Files:**
- Modify: `crates/storage/src/repos/task.rs` (add `search` method if missing)
- Modify: `crates/storage/src/repos/note.rs` (add `search` method if missing)

- [ ] **Step 1: Check if TaskRepo and NoteRepo have search methods**

Read the actual files. If they don't have a `search(query, limit)` method, add one using `LIKE`:

```rust
pub async fn search(&self, query: &str, limit: i64) -> common::Result<Vec<TaskRow>> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<_, TaskRow>(
        "SELECT * FROM tasks WHERE title LIKE ? AND status != 'completed' ORDER BY priority DESC LIMIT ?"
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

Similar for NoteRepo.

- [ ] **Step 2: Run full workspace build**

Run: `cargo build --workspace`
Expected: Compiles

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 4: Run existing tests**

Run: `cargo nextest run --workspace`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "feat(launcher): add search methods to TaskRepo and NoteRepo"
```

### Task 8.2: Frontend build verification and lint

- [ ] **Step 1: Run frontend build**

Run: `cd desktop-ui && bun run build`
Expected: Builds

- [ ] **Step 2: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Run frontend tests**

Run: `cd desktop-ui && bun run test`

- [ ] **Step 4: Full Tauri dev test**

Run: `cargo tauri dev`
Expected: App launches, ⌥Space opens launcher with dashboard view, typing shows search results.

- [ ] **Step 5: Commit any fixes**

```bash
git add .
git commit -m "fix(launcher): address lint and build issues"
```

### Task 8.3: Launcher event emitting from backend

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/mod.rs`

- [ ] **Step 1: Add Tauri event emission for dashboard updates**

When productivity data changes, focus sessions start/end, or tasks are modified, emit `launcher:*` events so the frontend dashboard stays fresh. Wire into existing event bus or add emit calls to relevant handlers.

This is best done by adding a `LauncherEventEmitter` that subscribes to the existing `DomainEventBus` and re-emits as Tauri events:

```rust
pub struct LauncherEventEmitter;

impl LauncherEventEmitter {
    pub fn start(app_handle: tauri::AppHandle, /* existing event sources */) {
        // Subscribe to focus session changes → emit launcher:focus_update
        // Subscribe to task changes → emit launcher:tasks_update
        // Periodic timer → emit launcher:productivity_update every 5 min
        // Calendar refresh → emit launcher:calendar_update
    }
}
```

- [ ] **Step 2: Wire into desktop app startup in main.rs**

- [ ] **Step 3: Commit**

```bash
git add crates/
git commit -m "feat(launcher): add event emitting for real-time dashboard updates"
```

---

## CRITICAL CORRECTIONS (Read Before Implementing)

The code snippets above contain API mismatches that MUST be fixed during implementation. These corrections are based on actual codebase verification.

### Correction 1: FeatureMigration struct (Task 1.1, Step 3)

**Wrong:**
```rust
FeatureMigration { feature: "launcher", version: 1, sql: include_str!(...) }
```

**Correct (4 fields, all owned Strings):**
```rust
FeatureMigration {
    feature_name: "launcher".to_string(),
    version: 1,
    description: "Launcher tables: frequencies, clipboard history, FTS5".to_string(),
    sql: include_str!("../migrations/001_launcher_tables.sql").to_string(),
}
```

### Correction 2: ProductivityRepos field access (Tasks 3.1, 3.2)

**Wrong:** `repos.calendar_event()`, `repos.daily_summary()`
**Correct:** Direct field access:
- `repos.calendar_events` (type: `CalendarEventRepo`)
- `repos.summaries` (type: `DailySummaryRepo`)
- `repos.sessions` (type: `FocusSessionRepo`)
- `repos.events` (type: `ActivityEventRepo`)

### Correction 3: NoteRepo location (Task 3.1)

**Wrong:** `storage::repos::NoteRepo`
**Correct:** `feature_notes::repo::NoteRepo`

The `AppCore` field is `pub note_repo: NoteRepo` (from `feature_notes::repo`).

`NoteRow` fields: `id`, `notebook_id`, `title`, `body`, `body_html`, `pinned`, `archived`, `created_at`, `updated_at`.

Search method: `note_repo.search_notes(query)` — returns `Vec<NoteRow>`, no limit parameter.

### Correction 4: TaskRepo search method (Task 3.1)

**Wrong:** `repo.search(query, 5)`
**Correct:** `repo.search_by_keyword(query, Some(5))` — returns `Result<Vec<TaskRow>, StorageError>`

Access via: `self.repos.tasks` on `AppCore`.

`TaskRow` fields: `id`, `title`, `description`, `area_id`, `project_id`, `priority`, `due_date`, `tags`, `status`, `focused_at`, etc. **No `project_name` field** — you need to join or look up separately, or just use `project_id`.

### Correction 5: CalendarEventRepo methods (Task 3.1)

Methods that exist: `list_range(from, to)`, `list_for_date(date)` — both return `Vec<CalendarEvent>`.
**No `search()` method.** For calendar search, use `list_for_date` with today's date and filter by title in Rust.

`CalendarEvent` fields: `id`, `calendar_id`, `title`, `description`, `started_at` (String), `ended_at` (String), `location`, `attendees_count`, `is_recurring`, `source`, `external_uid`, `session_id`, `color`, `synced_at`.

### Correction 6: DailySummaryRepo method (Task 3.1 dashboard)

**Wrong:** `repos.daily_summary().get_by_date(&today)`
**Correct:** `repos.summaries.get(&today)` — returns `Option<DailySummary>`

`DailySummary` fields: `date`, `total_active_secs`, `productive_secs`, `neutral_secs`, `distracting_secs`, `focus_sessions_count`, `context_switches`, `top_apps: Vec<AppUsage>`, `top_categories: Vec<CategoryUsage>`, `productivity_score: Option<f64>`, `deep_work_blocks`, `deep_work_secs`.

**No `total_minutes`, `top_category`, `top_category_pct` fields.** Compute from:
- `total_minutes = summary.total_active_secs / 60`
- Top category from `summary.top_categories[0]` (Vec<CategoryUsage>)
- Score from `summary.productivity_score`

### Correction 7: FocusSession fields (Task 3.1 dashboard)

**Wrong:** `s.task_name`, `s.started_at` (String), `s.target_minutes`, `s.id`
**Correct:**
- `s.id` ✓
- `s.action_id` (not `task_name` — need to look up task title from action_id)
- `s.started_at` is `DateTime<Utc>` (not String)
- `s.target_mins: Option<i64>` (not `target_minutes`)

### Correction 8: ApiError constructors (Task 3.3)

**Wrong:** `ApiError::feature_disabled()`, `ApiError::not_found()`, `ApiError::internal()`
**Correct:** Only `ApiError::new(code, message)`:
```rust
ApiError::new("FEATURE_DISABLED", "Launcher feature is not enabled")
ApiError::new("NOT_FOUND", "Clipboard entry not found")
ApiError::new("INTERNAL", format!("Error: {}", e))
```

### Correction 9: App launching (Task 5.2 keyboard nav)

**Wrong:** `ipc("launcher_run_script", { path: 'open -a ...' })`
**Correct:** Add a dedicated `launcher_open_app` Tauri command:
```rust
#[tauri::command]
pub async fn launcher_open_app(path: String) -> Result<(), ApiError> {
    std::process::Command::new("open").arg("-a").arg(&path).spawn()
        .map_err(|e| ApiError::new("LAUNCH_FAILED", e.to_string()))?;
    Ok(())
}
```
Frontend calls: `ipc("launcher_open_app", { path: item.kind.path })`

### Correction 10: ScriptRunner needs Clone

Add `Arc<RwLock<>>` pattern (same as `AppIndex`) to make `ScriptRunner` cloneable:
```rust
#[derive(Clone)]
pub struct ScriptRunner {
    scripts: Arc<RwLock<Vec<ScriptEntry>>>,
}
```

### Correction 11: Missing features (implement after core chunks)

These spec features are not covered in Chunks 1-8 and need additional tasks:

1. **Tab → Detail/Actions panel** — `ResultActions.tsx` component showing secondary actions per item type. New `detail` mode in state machine. Tab key handler in keyboard nav.
2. **Permissions handling** — Permission check for Accessibility/Automation. In-launcher explanation card. Graceful degradation.
3. **Category display grouping** — Map 18 existing categories → 8 display groups for dashboard. Function in `app-core/handlers/launcher/dashboard.rs`.
4. **CSS variables** — Add `--launcher-widget-gap` and `--launcher-result-height` to `theme.css` and register in `@theme inline`.
5. **Productivity score customization** — Config option for which display groups count as productive.
6. **`shellexpand` dependency** — Add to `app-core/Cargo.toml` for tilde expansion in scripts dir path.
