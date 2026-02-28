# Action Space Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close 6 architectural gaps identified in the Claude Code blog comparison — search tools, subagent coordination, platform-native elicitation, intent-based tool filtering, mode-gated RAG, and progressive disclosure.

**Architecture:** Layered feature slices across existing crates. Each task touches 2-4 crates and can be developed/tested independently. Breaking changes are acceptable (pre-production).

**Tech Stack:** Rust, SQLite (sqlx), tokio, async-trait, glob/walkdir/regex crates, Telegram/Discord/Slack HTTP APIs.

---

## Task 1: GrepTool — Search File Contents

**Files:**
- Create: `crates/tools/src/grep.rs`
- Modify: `crates/tools/src/lib.rs` (add `pub mod grep;`)
- Modify: `crates/tools/Cargo.toml` (add `walkdir`, `globset` deps)
- Modify: `crates/agent/src/agent_loop/builder.rs` (register tool)

**Step 1: Add dependencies to tools Cargo.toml**

Add to `[dependencies]` section of `crates/tools/Cargo.toml`:
```toml
walkdir = "2"
globset = "0.4"
```

Note: `regex` is already a workspace dependency.

**Step 2: Write the failing test**

Create `crates/tools/src/grep.rs` with the test module first:

```rust
//! Grep tool for searching file contents by regex pattern.

use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use regex::Regex;
use serde_json::Value;
use std::path::PathBuf;
use walkdir::WalkDir;

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

use crate::filesystem::FsToolBase;

pub struct GrepTool {
    base: FsToolBase,
}

impl GrepTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }
    fn description(&self) -> &str { "Search file contents using regex patterns within a directory scope. Returns matching lines with file path and line number." }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: workspace root)"
                },
                "glob": {
                    "type": "string",
                    "description": "File filter pattern, e.g. '*.rs', '*.py'. Only files matching this pattern are searched."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 20)",
                    "minimum": 1,
                    "maximum": 100
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of lines of context to show before and after each match (default: 0)",
                    "minimum": 0,
                    "maximum": 5
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    #[tokio::test]
    async fn test_grep_basic_match() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("hello.txt"), "hello world\ngoodbye world\nhello again").unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "hello",
            "path": tmp.path().to_str().unwrap()
        }), &test_ctx()).await.unwrap();

        assert!(result.contains("hello.txt:1:"));
        assert!(result.contains("hello.txt:3:"));
        assert!(!result.contains("goodbye"));
    }

    #[tokio::test]
    async fn test_grep_with_glob_filter() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("code.rs"), "fn main() {}").unwrap();
        fs::write(tmp.path().join("notes.txt"), "fn notes() {}").unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "fn",
            "path": tmp.path().to_str().unwrap(),
            "glob": "*.rs"
        }), &test_ctx()).await.unwrap();

        assert!(result.contains("code.rs"));
        assert!(!result.contains("notes.txt"));
    }

    #[tokio::test]
    async fn test_grep_max_results() {
        let tmp = TempDir::new().unwrap();
        let content: String = (0..50).map(|i| format!("match line {}\n", i)).collect();
        fs::write(tmp.path().join("big.txt"), &content).unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "match",
            "path": tmp.path().to_str().unwrap(),
            "max_results": 5
        }), &test_ctx()).await.unwrap();

        let match_count = result.lines().filter(|l| l.contains("big.txt")).count();
        assert_eq!(match_count, 5);
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("test.txt"), "content").unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "[invalid",
            "path": tmp.path().to_str().unwrap()
        }), &test_ctx()).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_grep_context_lines() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("ctx.txt"), "line1\nline2\nMATCH\nline4\nline5").unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "MATCH",
            "path": tmp.path().to_str().unwrap(),
            "context_lines": 1
        }), &test_ctx()).await.unwrap();

        assert!(result.contains("line2"));
        assert!(result.contains("MATCH"));
        assert!(result.contains("line4"));
    }

    #[test]
    fn test_grep_schema() {
        let tool = GrepTool::new(None);
        let schema = tool.to_schema();
        assert_eq!(schema["function"]["name"], "grep");
    }
}
```

**Step 3: Run test to verify it fails**

Run: `cargo nextest run -p tools -E 'test(grep)' --no-capture`
Expected: FAIL with "not yet implemented"

**Step 4: Implement the execute method**

Replace the `todo!()` in `execute()` with:

```rust
async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
    let p = ParamExtractor::new(&args);
    let pattern_str = p.required_str("pattern")?;
    let max_results = p.i64_or("max_results", 20)? as usize;
    let context_lines = p.i64_or("context_lines", 0)? as usize;

    let re = Regex::new(pattern_str)
        .map_err(|e| ToolError::InvalidParams(format!("Invalid regex: {}", e)))?;

    let search_path = if let Some(path) = p.optional_str("path")? {
        self.base.resolve_path(path)?
    } else if let Some(ref dir) = self.base.allowed_dir() {
        dir.clone()
    } else {
        std::env::current_dir().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
    };

    let glob_matcher: Option<GlobMatcher> = if let Some(glob_str) = p.optional_str("glob")? {
        Some(
            Glob::new(glob_str)
                .map_err(|e| ToolError::InvalidParams(format!("Invalid glob: {}", e)))?
                .compile_matcher(),
        )
    } else {
        None
    };

    let mut results = Vec::new();

    for entry in WalkDir::new(&search_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        if let Some(ref matcher) = glob_matcher {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !matcher.is_match(name) {
                    continue;
                }
            }
        }

        // Skip binary files (check first 512 bytes)
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // Skip unreadable/binary files
        };

        let lines: Vec<&str> = content.lines().collect();
        let rel_path = path
            .strip_prefix(&search_path)
            .unwrap_or(path)
            .to_string_lossy();

        for (i, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                if context_lines > 0 {
                    let start = i.saturating_sub(context_lines);
                    let end = (i + context_lines + 1).min(lines.len());
                    for j in start..end {
                        let marker = if j == i { ">" } else { " " };
                        results.push(format!("{}{}:{}:{}", marker, rel_path, j + 1, lines[j]));
                    }
                    results.push("--".to_string());
                } else {
                    results.push(format!("{}:{}:{}", rel_path, i + 1, line));
                }

                if results.len() >= max_results * (1 + context_lines * 2 + 1) {
                    break;
                }
            }
        }

        // Check overall limit (count actual matches, not context lines)
        let match_count = results.iter().filter(|l| !l.starts_with(' ') && *l != &"--").count();
        if match_count >= max_results {
            break;
        }
    }

    if results.is_empty() {
        Ok(format!("No matches found for pattern '{}' in {}", pattern_str, search_path.display()))
    } else {
        Ok(results.join("\n"))
    }
}
```

Note: You'll need to expose `FsToolBase::allowed_dir()` as a public getter if it doesn't exist, or access the field directly. Check `crates/tools/src/filesystem.rs` for how `FsToolBase` exposes `allowed_dir` — if it's a private field, add `pub fn allowed_dir(&self) -> &Option<PathBuf> { &self.allowed_dir }`.

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p tools -E 'test(grep)' --no-capture`
Expected: All 5 tests PASS

**Step 6: Register in builder and add module declaration**

In `crates/tools/src/lib.rs`, add:
```rust
pub mod grep;
```

In `crates/agent/src/agent_loop/builder.rs`, after the filesystem tool registration block, add:
```rust
tool_registry.register(tools::grep::GrepTool::new(allowed_dir.clone()));
```

**Step 7: Run full build + clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: 0 errors, 0 warnings

**Step 8: Commit**

```bash
git add crates/tools/src/grep.rs crates/tools/src/lib.rs crates/tools/Cargo.toml crates/agent/src/agent_loop/builder.rs
git commit -m "feat(tools): add GrepTool for searching file contents by regex"
```

---

## Task 2: GlobTool — Find Files by Pattern

**Files:**
- Create: `crates/tools/src/glob_tool.rs`
- Modify: `crates/tools/src/lib.rs` (add `pub mod glob_tool;`)
- Modify: `crates/agent/src/agent_loop/builder.rs` (register tool)

**Step 1: Write the failing test**

Create `crates/tools/src/glob_tool.rs`:

```rust
//! Glob tool for finding files by pattern matching.

use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use serde_json::Value;
use std::path::PathBuf;
use walkdir::WalkDir;

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

use crate::filesystem::FsToolBase;

pub struct GlobTool {
    base: FsToolBase,
}

impl GlobTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }
    fn description(&self) -> &str { "Find files by glob pattern matching. Returns matching file paths sorted by modification time (most recent first)." }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern like '**/*.rs', 'src/**/*.ts', '*.json'"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search from (default: workspace root)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    #[tokio::test]
    async fn test_glob_basic() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rs"), "").unwrap();
        fs::write(tmp.path().join("b.rs"), "").unwrap();
        fs::write(tmp.path().join("c.txt"), "").unwrap();

        let tool = GlobTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "*.rs",
            "path": tmp.path().to_str().unwrap()
        }), &test_ctx()).await.unwrap();

        assert!(result.contains("a.rs"));
        assert!(result.contains("b.rs"));
        assert!(!result.contains("c.txt"));
    }

    #[tokio::test]
    async fn test_glob_recursive() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(tmp.path().join("top.rs"), "").unwrap();
        fs::write(sub.join("nested.rs"), "").unwrap();

        let tool = GlobTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "**/*.rs",
            "path": tmp.path().to_str().unwrap()
        }), &test_ctx()).await.unwrap();

        assert!(result.contains("top.rs"));
        assert!(result.contains("nested.rs"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();

        let tool = GlobTool::new(Some(tmp.path().to_path_buf()));
        let result = tool.execute(serde_json::json!({
            "pattern": "*.rs",
            "path": tmp.path().to_str().unwrap()
        }), &test_ctx()).await.unwrap();

        assert!(result.contains("No files found"));
    }

    #[test]
    fn test_glob_schema() {
        let tool = GlobTool::new(None);
        let schema = tool.to_schema();
        assert_eq!(schema["function"]["name"], "glob");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p tools -E 'test(glob)' --no-capture`
Expected: FAIL with "not yet implemented"

**Step 3: Implement the execute method**

Replace `todo!()`:

```rust
async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
    let p = ParamExtractor::new(&args);
    let pattern_str = p.required_str("pattern")?;

    let search_path = if let Some(path) = p.optional_str("path")? {
        self.base.resolve_path(path)?
    } else if let Some(ref dir) = self.base.allowed_dir() {
        dir.clone()
    } else {
        std::env::current_dir().map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
    };

    let glob = Glob::new(pattern_str)
        .map_err(|e| ToolError::InvalidParams(format!("Invalid glob pattern: {}", e)))?
        .compile_matcher();

    let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in WalkDir::new(&search_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let rel_path = entry.path()
            .strip_prefix(&search_path)
            .unwrap_or(entry.path());

        if glob.is_match(rel_path) {
            let mtime = entry.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            matches.push((rel_path.to_path_buf(), mtime));
        }
    }

    if matches.is_empty() {
        return Ok(format!("No files found matching '{}' in {}", pattern_str, search_path.display()));
    }

    // Sort by modification time (most recent first)
    matches.sort_by(|a, b| b.1.cmp(&a.1));

    let output: Vec<String> = matches.iter()
        .map(|(p, _)| p.to_string_lossy().to_string())
        .collect();

    Ok(output.join("\n"))
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p tools -E 'test(glob)' --no-capture`
Expected: All 4 tests PASS

**Step 5: Register in builder and add module declaration**

In `crates/tools/src/lib.rs`, add:
```rust
pub mod glob_tool;
```

In `crates/agent/src/agent_loop/builder.rs`, after the grep registration:
```rust
tool_registry.register(tools::glob_tool::GlobTool::new(allowed_dir.clone()));
```

**Step 6: Run full build + clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: 0 errors, 0 warnings

**Step 7: Commit**

```bash
git add crates/tools/src/glob_tool.rs crates/tools/src/lib.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(tools): add GlobTool for finding files by pattern"
```

---

## Task 3: AgentTask Storage Layer

**Files:**
- Create: `crates/storage/migrations/005_agent_tasks.sql`
- Create: `crates/storage/src/rows/agent_task.rs`
- Create: `crates/storage/src/repos/agent_task.rs`
- Modify: `crates/storage/src/rows/mod.rs` (add module)
- Modify: `crates/storage/src/repos/mod.rs` (add module + Repos field)

**Step 1: Create the migration**

Create `crates/storage/migrations/005_agent_tasks.sql`:

```sql
-- Agent coordination task board for subagent work tracking

CREATE TABLE agent_tasks (
    id              TEXT PRIMARY KEY,
    session_key     TEXT NOT NULL,
    description     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending','claimed','running','completed','failed')),
    owner_agent_id  TEXT,
    parent_task_id  TEXT REFERENCES agent_tasks(id) ON DELETE CASCADE,
    result          TEXT,
    error           TEXT,
    blocked_by      TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_agent_tasks_session ON agent_tasks(session_key);
CREATE INDEX idx_agent_tasks_status ON agent_tasks(status);
CREATE INDEX idx_agent_tasks_owner ON agent_tasks(owner_agent_id);

-- Tool usage analytics (P3 but adding migration now to avoid future migration)
CREATE TABLE tool_usage (
    id              TEXT PRIMARY KEY,
    tool_name       TEXT NOT NULL,
    action          TEXT,
    session_key     TEXT,
    channel         TEXT,
    intent_category TEXT,
    success         INTEGER NOT NULL DEFAULT 1,
    duration_ms     INTEGER,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_tool_usage_tool ON tool_usage(tool_name);
CREATE INDEX idx_tool_usage_created ON tool_usage(created_at);
```

**Step 2: Create the row struct**

Create `crates/storage/src/rows/agent_task.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTaskRow {
    pub id: String,
    pub session_key: String,
    pub description: String,
    pub status: String,
    pub owner_agent_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub blocked_by: String, // JSON array
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Add `pub mod agent_task;` and `pub use agent_task::AgentTaskRow;` to `crates/storage/src/rows/mod.rs`.

**Step 3: Create the repo with tests**

Create `crates/storage/src/repos/agent_task.rs`:

```rust
use crate::rows::AgentTaskRow;
use crate::StorageError;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentTaskRepo {
    pool: SqlitePool,
}

impl AgentTaskRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        session_key: &str,
        description: &str,
        blocked_by: &[String],
    ) -> Result<AgentTaskRow, StorageError> {
        let id = Uuid::new_v4().to_string();
        let blocked_by_json = serde_json::to_string(blocked_by)
            .unwrap_or_else(|_| "[]".to_string());

        sqlx::query_as::<_, AgentTaskRow>(
            "INSERT INTO agent_tasks (id, session_key, description, blocked_by)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING *"
        )
        .bind(&id)
        .bind(session_key)
        .bind(description)
        .bind(&blocked_by_json)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    pub async fn claim(
        &self,
        task_id: &str,
        agent_id: &str,
    ) -> Result<AgentTaskRow, StorageError> {
        sqlx::query_as::<_, AgentTaskRow>(
            "UPDATE agent_tasks
             SET owner_agent_id = ?1, status = 'claimed',
                 updated_at = datetime('now')
             WHERE id = ?2 AND owner_agent_id IS NULL AND status = 'pending'
             RETURNING *"
        )
        .bind(agent_id)
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!(
            "Task {} not found or already claimed", task_id
        )))
    }

    pub async fn update_status(
        &self,
        task_id: &str,
        status: &str,
        result: Option<&str>,
        error: Option<&str>,
    ) -> Result<AgentTaskRow, StorageError> {
        sqlx::query_as::<_, AgentTaskRow>(
            "UPDATE agent_tasks
             SET status = ?1, result = ?2, error = ?3,
                 updated_at = datetime('now')
             WHERE id = ?4
             RETURNING *"
        )
        .bind(status)
        .bind(result)
        .bind(error)
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("Task {} not found", task_id)))
    }

    pub async fn list_by_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<AgentTaskRow>, StorageError> {
        sqlx::query_as::<_, AgentTaskRow>(
            "SELECT * FROM agent_tasks WHERE session_key = ?1 ORDER BY created_at"
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    pub async fn list_available(
        &self,
        session_key: &str,
    ) -> Result<Vec<AgentTaskRow>, StorageError> {
        // Available = unclaimed AND not blocked by any incomplete task
        let all = self.list_by_session(session_key).await?;
        let completed_ids: std::collections::HashSet<String> = all.iter()
            .filter(|t| t.status == "completed")
            .map(|t| t.id.clone())
            .collect();

        Ok(all.into_iter().filter(|t| {
            if t.status != "pending" || t.owner_agent_id.is_some() {
                return false;
            }
            let blocked: Vec<String> = serde_json::from_str(&t.blocked_by).unwrap_or_default();
            blocked.iter().all(|id| completed_ids.contains(id))
        }).collect())
    }

    pub async fn delete_by_session(
        &self,
        session_key: &str,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM agent_tasks WHERE session_key = ?1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn get(&self, task_id: &str) -> Result<AgentTaskRow, StorageError> {
        sqlx::query_as::<_, AgentTaskRow>(
            "SELECT * FROM agent_tasks WHERE id = ?1"
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("Task {} not found", task_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> AgentTaskRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        AgentTaskRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let repo = setup().await;
        let task = repo.create("sess:1", "Do research", &[]).await.unwrap();
        assert_eq!(task.status, "pending");
        assert_eq!(task.description, "Do research");
        assert!(task.owner_agent_id.is_none());

        let fetched = repo.get(&task.id).await.unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[tokio::test]
    async fn test_claim() {
        let repo = setup().await;
        let task = repo.create("sess:1", "Research", &[]).await.unwrap();

        let claimed = repo.claim(&task.id, "agent-abc").await.unwrap();
        assert_eq!(claimed.status, "claimed");
        assert_eq!(claimed.owner_agent_id.as_deref(), Some("agent-abc"));

        // Double claim should fail
        let err = repo.claim(&task.id, "agent-xyz").await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_update_status_complete() {
        let repo = setup().await;
        let task = repo.create("sess:1", "Work", &[]).await.unwrap();
        repo.claim(&task.id, "agent-1").await.unwrap();

        let updated = repo.update_status(&task.id, "completed", Some("Done!"), None).await.unwrap();
        assert_eq!(updated.status, "completed");
        assert_eq!(updated.result.as_deref(), Some("Done!"));
    }

    #[tokio::test]
    async fn test_list_available_respects_blocking() {
        let repo = setup().await;
        let t1 = repo.create("sess:1", "First", &[]).await.unwrap();
        let _t2 = repo.create("sess:1", "Second (blocked)", &[t1.id.clone()]).await.unwrap();
        let t3 = repo.create("sess:1", "Third (free)", &[]).await.unwrap();

        let available = repo.list_available("sess:1").await.unwrap();
        assert_eq!(available.len(), 2); // t1 and t3 are available
        let ids: Vec<&str> = available.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&t1.id.as_str()));
        assert!(ids.contains(&t3.id.as_str()));

        // Complete t1, now t2 should become available
        repo.claim(&t1.id, "a").await.unwrap();
        repo.update_status(&t1.id, "completed", None, None).await.unwrap();
        let available2 = repo.list_available("sess:1").await.unwrap();
        assert_eq!(available2.len(), 2); // t2 and t3
    }

    #[tokio::test]
    async fn test_delete_by_session() {
        let repo = setup().await;
        repo.create("sess:1", "A", &[]).await.unwrap();
        repo.create("sess:1", "B", &[]).await.unwrap();
        repo.create("sess:2", "C", &[]).await.unwrap();

        let deleted = repo.delete_by_session("sess:1").await.unwrap();
        assert_eq!(deleted, 2);

        let remaining = repo.list_by_session("sess:2").await.unwrap();
        assert_eq!(remaining.len(), 1);
    }
}
```

**Step 4: Register in repos mod.rs and Repos struct**

In `crates/storage/src/repos/mod.rs`, add:
```rust
pub mod agent_task;
pub use agent_task::AgentTaskRepo;
```

Add `pub agent_tasks: AgentTaskRepo,` to the `Repos` struct, and `agent_tasks: AgentTaskRepo::new(db.clone()),` to `Repos::from_pool()`.

**Step 5: Run tests**

Run: `cargo nextest run -p storage -E 'test(agent_task)' --no-capture`
Expected: All 5 tests PASS

**Step 6: Run full workspace build**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: 0 errors, 0 warnings

**Step 7: Commit**

```bash
git add crates/storage/migrations/005_agent_tasks.sql \
        crates/storage/src/rows/agent_task.rs crates/storage/src/rows/mod.rs \
        crates/storage/src/repos/agent_task.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): add agent_tasks table and AgentTaskRepo for subagent coordination"
```

---

## Task 4: AgentTaskTool + Handler Trait

**Files:**
- Create: `crates/tools/src/agent_task_tool.rs`
- Modify: `crates/tools/src/lib.rs` (add module)

**Step 1: Write the tool with handler trait**

Create `crates/tools/src/agent_task_tool.rs`:

```rust
//! Agent task tool for subagent coordination.
//! Only registered in subagent tool registries, not the parent agent's.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

/// Handler trait for agent task operations (dependency inversion).
/// Defined here in tools crate, implemented in agent crate.
#[async_trait]
pub trait AgentTaskHandler: Send + Sync {
    async fn list_tasks(&self, session_key: &str) -> Result<String>;
    async fn claim_task(&self, task_id: &str, agent_id: &str) -> Result<String>;
    async fn update_task(&self, task_id: &str, status: &str, result: Option<&str>) -> Result<String>;
    async fn complete_task(&self, task_id: &str, result: &str) -> Result<String>;
    async fn fail_task(&self, task_id: &str, error: &str) -> Result<String>;
}

pub struct AgentTaskTool {
    handler: Arc<dyn AgentTaskHandler>,
    session_key: String,
    agent_id: String,
}

impl AgentTaskTool {
    pub fn new(
        handler: Arc<dyn AgentTaskHandler>,
        session_key: String,
        agent_id: String,
    ) -> Self {
        Self { handler, session_key, agent_id }
    }
}

#[async_trait]
impl Tool for AgentTaskTool {
    fn name(&self) -> &str { "agent_task" }

    fn description(&self) -> &str {
        "Manage your assigned tasks from the task board. Use 'list' to see all tasks, \
         'claim' to take ownership of an unclaimed task, 'update' to report progress, \
         'complete' to mark a task done with results, or 'fail' to report a failure."
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Standard }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "claim", "update", "complete", "fail"],
                    "description": "Action to perform on the task board"
                },
                "task_id": {
                    "type": "string",
                    "description": "Task ID (required for claim, update, complete, fail)"
                },
                "result": {
                    "type": "string",
                    "description": "Result text (for update/complete) or error message (for fail)"
                },
                "status": {
                    "type": "string",
                    "enum": ["running", "completed", "failed"],
                    "description": "New status (for update action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "list" => self.handler.list_tasks(&self.session_key).await,
            "claim" => {
                let task_id = p.required_str("task_id")?;
                self.handler.claim_task(task_id, &self.agent_id).await
            }
            "update" => {
                let task_id = p.required_str("task_id")?;
                let status = p.str_or("status", "running")?;
                let result = p.optional_str("result")?;
                self.handler.update_task(task_id, status, result).await
            }
            "complete" => {
                let task_id = p.required_str("task_id")?;
                let result = p.str_or("result", "Task completed")?;
                self.handler.complete_task(task_id, result).await
            }
            "fail" => {
                let task_id = p.required_str("task_id")?;
                let error = p.str_or("result", "Task failed")?;
                self.handler.fail_task(task_id, error).await
            }
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}
```

**Step 2: Add module declaration**

In `crates/tools/src/lib.rs`, add:
```rust
pub mod agent_task_tool;
```

**Step 3: Run build**

Run: `cargo build --workspace`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/tools/src/agent_task_tool.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add AgentTaskTool and AgentTaskHandler trait for subagent coordination"
```

---

## Task 5: SubagentManager Overhaul — Cancel, Status, Task Board

**Files:**
- Modify: `crates/tools/src/spawn.rs` (extend SpawnHandler trait with cancel/status)
- Modify: `crates/agent/src/subagent.rs` (add SubagentHandle tracking, cancel, status, task board wiring)
- Modify: `crates/agent/src/agent_loop/builder.rs` (wire AgentTaskHandler, pass repos to SubagentManager)

**Step 1: Extend SpawnHandler trait**

In `crates/tools/src/spawn.rs`, extend the trait:

```rust
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        profile: String,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String;

    async fn cancel(&self, agent_id: &str) -> common::Result<String>;
    async fn status(&self, session_key: &str) -> common::Result<String>;
}
```

Update `SpawnTool::parameters()` to add `"action"` field with enum `["spawn", "cancel", "status"]` (default `"spawn"`). Update `execute()` to dispatch by action, calling `handler.cancel()` or `handler.status()` for the new actions. The existing behavior becomes the `"spawn"` action (also the default when action is omitted).

**Step 2: Update SubagentManager**

In `crates/agent/src/subagent.rs`:

Add a `SubagentHandle` struct:
```rust
struct SubagentHandle {
    cancel_token: tokio_util::sync::CancellationToken,
    label: String,
    profile: SubagentProfile,
    spawned_at: std::time::Instant,
}
```

Add to `SubagentManager`:
```rust
handles: Arc<Mutex<HashMap<String, SubagentHandle>>>,
agent_task_repo: Option<storage::repos::AgentTaskRepo>,
```

In `spawn()`: generate a short agent ID, store a `SubagentHandle`, pass `CancellationToken` into the spawned task. The `ReactiveEngine` loop should check `token.is_cancelled()` between cycles.

Implement `cancel()`: look up handle by agent ID, call `cancel_token.cancel()`, remove from handles map.

Implement `status()`: read `agent_task_repo.list_by_session(session_key)` and format as a task board table.

In `run_subagent_task()`: register `AgentTaskTool` in the subagent's `ToolRegistry` with the task handler and session key, so the subagent can manage its tasks.

**Step 3: Create AgentTaskHandlerImpl**

Create a new adapter in `crates/agent/src/agent_task_handler.rs` that implements `AgentTaskHandler` by delegating to `AgentTaskRepo`. Follow the same adapter pattern used by `CronHandlerAdapter`, `CalendarSyncAdapter`, etc.

**Step 4: Wire in builder**

In `crates/agent/src/agent_loop/builder.rs`, pass `repos.agent_tasks.clone()` to `SubagentManager::builder()`. Create the `AgentTaskHandlerImpl` and store it for subagent registry injection.

**Step 5: Run all tests**

Run: `cargo nextest run --workspace --no-capture`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add crates/tools/src/spawn.rs crates/agent/src/subagent.rs \
        crates/agent/src/agent_task_handler.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): overhaul SubagentManager with cancel, status, and AgentTaskBoard"
```

---

## Task 6: Intent-Based Tool Filtering

**Files:**
- Modify: `crates/agent/src/intent_pipeline/types.rs` (add `tool_groups` to IntentAnalysis)
- Modify: `crates/agent/src/intent_pipeline/heuristics.rs` (populate tool_groups)
- Modify: `crates/agent/src/intent_pipeline/classifier.rs` (add relevant_tools to JSON schema)
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs` (filter tools before passing to router)

**Step 1: Define ToolGroup enum in types.rs**

Add to `crates/agent/src/intent_pipeline/types.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolGroup {
    None,
    TaskManagement,
    Search,
    Calendar,
    Finance,
    Communication,
    Automation,
    Full,
}

impl ToolGroup {
    pub fn tool_names(&self) -> Vec<&'static str> {
        match self {
            Self::None => vec![],
            Self::TaskManagement => vec!["todo", "goal", "plan"],
            Self::Search => vec!["grep", "glob", "read_file", "list_dir", "web_search", "web_fetch", "memory"],
            Self::Calendar => vec!["calendar", "todo"],
            Self::Finance => vec!["finance"],
            Self::Communication => vec!["message", "ask_user"],
            Self::Automation => vec!["cron", "spawn"],
            Self::Full => vec![], // Special: means all tools
        }
    }
}
```

Add `pub tool_groups: Vec<ToolGroup>` to `IntentAnalysis`.

**Step 2: Populate tool_groups in heuristics**

In `crates/agent/src/intent_pipeline/heuristics.rs`, set `tool_groups` alongside each mode classification. For example:
- Greetings → `vec![ToolGroup::None]`
- Task management keywords → `vec![ToolGroup::TaskManagement, ToolGroup::Search]`
- Calendar keywords → `vec![ToolGroup::Calendar]`
- Complex/unknown → `vec![ToolGroup::Full]`

**Step 3: Add relevant_tools to classifier JSON schema**

In `crates/agent/src/intent_pipeline/classifier.rs`, add `"relevant_tools": ["tool1", "tool2"]` to the expected JSON schema. Parse it and map to `ToolGroup` values.

**Step 4: Filter tools in pipeline.rs**

In `crates/agent/src/intent_pipeline/pipeline.rs`, after classification:

```rust
let filtered_tools = if analysis.tool_groups.contains(&ToolGroup::Full) {
    tool_definitions.to_vec()
} else {
    let allowed: std::collections::HashSet<&str> = analysis.tool_groups.iter()
        .flat_map(|g| g.tool_names())
        .chain(std::iter::once("ask_user")) // always available
        .collect();
    tool_definitions.iter()
        .filter(|t| {
            t.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(|name| allowed.contains(name))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
};
```

Pass `filtered_tools` to the router instead of `tool_definitions`. On escalation, fall back to `Full` tools.

**Step 5: Write tests**

Add tests to `heuristics.rs` and `pipeline.rs` verifying:
- Greetings get no tools
- Task CRUD gets TaskManagement + Search
- Complex messages get Full
- Escalation restores Full tools

**Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(heuristic)' --no-capture && cargo nextest run -p agent -E 'test(pipeline)' --no-capture`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add crates/agent/src/intent_pipeline/types.rs \
        crates/agent/src/intent_pipeline/heuristics.rs \
        crates/agent/src/intent_pipeline/classifier.rs \
        crates/agent/src/intent_pipeline/pipeline.rs
git commit -m "feat(agent): add intent-based tool filtering to reduce action space per query"
```

---

## Task 7: Platform-Native Elicitation — Channel Trait Extension

**Files:**
- Modify: `crates/channels/src/lib.rs` (extend Channel trait)
- Modify: `crates/tools-core/src/lib.rs` (add `channel_ref` to RoutingContext)
- Modify: `crates/tools/src/ask_user.rs` (use channel interaction path)

**Step 1: Extend Channel trait**

In `crates/channels/src/lib.rs`, add default methods:

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    // ... existing methods ...

    fn supports_interaction(&self) -> bool { false }

    async fn send_interaction(
        &self,
        _chat_id: &str,
        _request: &common::InteractionRequest,
    ) -> Result<common::FormResponse> {
        Err(common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
            "Channel does not support structured interactions".into()
        )))
    }
}
```

**Step 2: Add channel_ref to RoutingContext**

In `crates/tools-core/src/lib.rs`, add to `RoutingContext`:

```rust
pub channel_ref: Option<Arc<dyn std::any::Any + Send + Sync>>,
```

Note: We use `Any` to avoid making `tools-core` depend on `channels`. The `ask_user` tool will downcast when needed. Alternatively, define a minimal `InteractionChannel` trait in `common` or `tools-core` with just `supports_interaction()` and `send_interaction()`, and have channels implement it.

The cleaner approach: define a trait in `tools-core`:

```rust
#[async_trait]
pub trait InteractionChannel: Send + Sync {
    fn supports_interaction(&self) -> bool;
    async fn send_interaction(
        &self,
        chat_id: &str,
        request: &common::InteractionRequest,
    ) -> common::Result<common::FormResponse>;
}
```

Add `pub interaction_channel: Option<Arc<dyn InteractionChannel>>` to `RoutingContext`.

**Step 3: Update ask_user tool**

In `crates/tools/src/ask_user.rs`, update `execute()`:

```rust
async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
    let request = parse_interaction_request(&args)?;

    // Path 1: CLI/Dashboard interactive mode
    if let Some(ref tx) = ctx.interaction_tx {
        // ... existing oneshot blocking flow ...
    }

    // Path 2: Channel-native interaction (NEW)
    if let Some(ref channel) = ctx.interaction_channel {
        if channel.supports_interaction() {
            let response = channel
                .send_interaction(ctx.chat_id.as_str(), &request)
                .await?;
            return Ok(format_semantic_response(&response, &request));
        }
    }

    // Path 3: Text fallback (unchanged)
    Ok(format_text_fallback(&request))
}
```

**Step 4: Wire in agent_loop**

In `process_message()`, when constructing `RoutingContext`, look up the channel from `ChannelManager` and set `interaction_channel` if the channel supports it.

**Step 5: Run build**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: 0 errors, 0 warnings

**Step 6: Commit**

```bash
git add crates/channels/src/lib.rs crates/tools-core/src/lib.rs \
        crates/tools/src/ask_user.rs crates/agent/src/agent_loop/mod.rs
git commit -m "feat: add InteractionChannel trait and ask_user channel-native path"
```

---

## Task 8: Telegram Interactive Elicitation

**Files:**
- Modify: `crates/channels/src/telegram.rs` (implement send_interaction with InlineKeyboardMarkup)

**Step 1: Implement supports_interaction and send_interaction for Telegram**

In `crates/channels/src/telegram.rs`:

Implement `InteractionChannel` (or the Channel trait extension) for `TelegramChannel`:

```rust
fn supports_interaction(&self) -> bool { true }

async fn send_interaction(
    &self,
    chat_id: &str,
    request: &InteractionRequest,
) -> Result<FormResponse> {
    // For each question:
    // 1. Build InlineKeyboardMarkup from options
    // 2. Send message with reply_markup
    // 3. Wait for callback_query (with 5-min timeout)
    // 4. Edit message to show selected answer
    // 5. Collect answers into FormResponse
}
```

For `single_select` / `yes_no`: Build `InlineKeyboardMarkup` with buttons in rows of 2. Each button has `callback_data` set to `"askuser:{question_id}:{option_value}"`.

For `multi_select`: Buttons toggle (prefix with check/uncheck), plus a `[Submit]` button.

For `free_text`: Send the question as a plain text message, store a `PendingInteraction` in a `DashMap<String, oneshot::Sender<String>>` keyed by chat_id, await the next user message.

Add a `pending_interactions: Arc<DashMap<String, PendingInteractionState>>` field to `TelegramChannel` for tracking in-progress interactions.

In the main polling loop (`handle_update()`), check for `callback_query` updates and resolve pending interactions.

**Step 2: Add dashmap dependency**

Add `dashmap = "6"` to `crates/channels/Cargo.toml` if not already present.

**Step 3: Write tests**

Test the InlineKeyboardMarkup JSON construction (unit test, no network):
- `single_select` with 3 options → 2 rows of buttons
- `yes_no` → 1 row with 2 buttons
- Callback data format is correct

**Step 4: Run tests**

Run: `cargo nextest run -p channels -E 'test(telegram)' --no-capture`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add crates/channels/src/telegram.rs crates/channels/Cargo.toml
git commit -m "feat(channels): implement Telegram InlineKeyboard interactions for ask_user"
```

---

## Task 9: Discord Interactive Elicitation

**Files:**
- Modify: `crates/channels/src/discord.rs` (implement send_interaction with ActionRow + Button + Select)

**Step 1: Implement send_interaction for Discord**

Follow the same pattern as Telegram but using Discord's component API:
- `single_select` with ≤5 options: `ActionRow` with `Button` components
- `single_select` with >5 options: `StringSelectMenu`
- `multi_select`: `StringSelectMenu` with `max_values` set
- `yes_no`: Two `Button` components (green/red styles)
- `free_text`: Send message, await next message in channel

Discord interactions use a different flow than Telegram callbacks:
- Send message with `components` array
- Listen for `INTERACTION_CREATE` gateway event (or use interaction URL)
- Respond with `InteractionResponseType::UPDATE_MESSAGE`

Add `pending_interactions` tracking similar to Telegram.

**Step 2-5:** Same pattern as Task 8.

**Step 5: Commit**

```bash
git add crates/channels/src/discord.rs
git commit -m "feat(channels): implement Discord component interactions for ask_user"
```

---

## Task 10: Slack Interactive Elicitation

**Files:**
- Modify: `crates/channels/src/slack.rs` (implement send_interaction with Block Kit)

**Step 1: Implement send_interaction for Slack**

Use Slack Block Kit:
- `single_select`: `section` block with `static_select` accessory
- `multi_select`: `section` block with `multi_static_select` accessory
- `yes_no`: `actions` block with two `button` elements
- `free_text`: `input` block with `plain_text_input` element (requires modal or just text prompt)

Slack interactions come via HTTP POST to an interaction URL. This requires either:
- A publicly accessible endpoint (for Slack's interaction payload webhook)
- Or using Slack's Socket Mode `interactive` events

Since we already use Socket Mode for messages, extend the WebSocket handler to process `interactive` envelope types.

**Step 2-5:** Same pattern as Tasks 8 and 9.

**Step 5: Commit**

```bash
git add crates/channels/src/slack.rs
git commit -m "feat(channels): implement Slack Block Kit interactions for ask_user"
```

---

## Task 11: Mode-Gated RAG

**Files:**
- Modify: `crates/context_engine/src/assembler.rs` (gate memory retrieval on strategy)

**Step 1: Write the failing test**

Add a test to the assembler tests that verifies Direct mode skips memory retrieval.

**Step 2: Implement the gate**

In `crates/context_engine/src/assembler.rs`, find the `retrieve_memory()` call and wrap it:

```rust
let memory_content = match &request.strategy {
    ExecutionStrategy::DirectResponse | ExecutionStrategy::Clarification { .. } => None,
    _ => self.retrieve_memory(request).await,
};
```

This is approximately a 5-line change.

**Step 3: Run tests**

Run: `cargo nextest run -p context_engine --no-capture`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "perf(context): skip memory retrieval for Direct-mode queries"
```

---

## Task 12: Progressive Disclosure — Skill Conventions + Identity Prompt

**Files:**
- Modify: `crates/agent/src/context_sources/identity.rs` (add progressive disclosure instruction)
- Modify: existing skills in `skills/` (add Deep Dive sections with links where applicable)

**Step 1: Update IdentitySource system prompt**

In `crates/agent/src/context_sources/identity.rs`, add to the system prompt:

```
**Progressive Disclosure:**
- When skills reference additional documentation via markdown links, use read_file to load that documentation if the current task requires deeper knowledge
- Follow nested references recursively when needed for complex tasks
```

**Step 2: Update 1-2 existing skills as examples**

Add "Deep Dive" sections to relevant skills (e.g., `skills/todo/SKILL.md`, `skills/cron/SKILL.md`) with links to relevant docs if they exist.

**Step 3: Run build**

Run: `cargo build --workspace`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/context_sources/identity.rs skills/
git commit -m "feat(agent): add progressive disclosure instructions to system prompt"
```

---

## Task 13: Make Search Tools Available to Subagents

**Files:**
- Modify: `crates/agent/src/subagent.rs` (register grep and glob in subagent tool registries)

**Step 1: Update subagent tool registration**

In `run_subagent_task()`, where subagent tool registries are built per profile, add `GrepTool` and `GlobTool` to all profiles (they're read-only):

```rust
// All profiles get search tools
registry.register(tools::grep::GrepTool::new(allowed_dir.clone()));
registry.register(tools::glob_tool::GlobTool::new(allowed_dir.clone()));
```

**Step 2: Run tests**

Run: `cargo nextest run -p agent -E 'test(subagent)' --no-capture`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add crates/agent/src/subagent.rs
git commit -m "feat(agent): register grep and glob tools in all subagent profiles"
```

---

## Task 14: Final Integration Test + Full Workspace Verification

**Files:**
- Run all workspace tests
- Run clippy
- Run fmt check

**Step 1: Run full test suite**

Run: `cargo nextest run --workspace --no-capture`
Expected: All tests PASS

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Run fmt check**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 4: Run doctests**

Run: `cargo test --workspace --doc`
Expected: PASS

**Step 5: Final commit if any fixes needed**

```bash
git commit -m "chore: fix lint/format issues from action space redesign"
```
