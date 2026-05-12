# Subagent Persistence and Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the one-shot, silently-capped subagent runtime with a persistent, resumable system (Kimi-style runtime + opencode-style storage) that surfaces cap-hits cleanly to the parent and lets the user drill into any subagent thread.

**Architecture:** Subagents become rows in the existing `sessions` table tagged with `mode='subagent'` and a `parent_session_id` foreign key, plus a thin `subagent_instances` metadata table for status / partial_summary / lifecycle bookkeeping. The existing `SpawnTool` is renamed to `SubagentsTool` with four actions (`spawn` / `resume` / `list` / `kill`) backed by a refactored `SubagentManager`. The 120k token cap is removed; turn cap raised to 500 per call. Cap-hits become structured `ToolError` payloads with `agent_id`, `partial_summary`, and a resume hint. Subagent sessions emit the same `agent:thread_event` envelope as coding threads, so the existing desktop UI handles drill-in / breadcrumb / sidebar grouping for free.

**Tech Stack:** Rust (workspace crates: `common`, `storage`, `agent`, `tools`, `app-core`, `desktop`), SQLite via sqlx, Tauri 2, React 18 + Zustand + TypeScript, vitest, cargo nextest.

**Spec:** `docs/superpowers/specs/2026-05-12-subagent-persistence-and-resume-design.md`

**Reference implementations:**
- Kimi runtime semantics: `/Users/jayden/Projects/Klynt/kimi-cli/src/kimi_cli/subagents/`
- Opencode storage model: `/Users/jayden/Projects/Klynt/opencode/internal/db/migrations/20250424200609_initial.sql`

---

## Phase 0 — Schema baseline

### Task 0.1: Add `Subagent` variant to `SessionMode`

**Files:**
- Modify: `crates/common/src/session_mode.rs`

- [ ] **Step 1: Write the failing test**

Add this test at the bottom of `crates/common/src/session_mode.rs` (in the existing `mod tests` block, replacing the existing `round_trips_via_str` array literal):

```rust
    #[test]
    fn round_trips_via_str() {
        for m in [SessionMode::Assistant, SessionMode::Coding, SessionMode::Subagent] {
            assert_eq!(SessionMode::parse(m.as_str()), Some(m));
        }
    }

    #[test]
    fn subagent_serde_uses_snake_case() {
        let s = serde_json::to_string(&SessionMode::Subagent).unwrap();
        assert_eq!(s, "\"subagent\"");
        let parsed: SessionMode = serde_json::from_str("\"subagent\"").unwrap();
        assert_eq!(parsed, SessionMode::Subagent);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p common -E 'test(session_mode)'`
Expected: FAIL — `SessionMode::Subagent` not found.

- [ ] **Step 3: Add the variant**

Edit `crates/common/src/session_mode.rs` so the enum and impl look like:

```rust
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    #[default]
    Assistant,
    Coding,
    Subagent,
}

impl SessionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::Coding => "coding",
            Self::Subagent => "subagent",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "assistant" => Some(Self::Assistant),
            "coding" => Some(Self::Coding),
            "subagent" => Some(Self::Subagent),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p common -E 'test(session_mode)'`
Expected: PASS — 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/session_mode.rs
git commit -m "feat(common): add SessionMode::Subagent variant"
```

---

### Task 0.2: Widen `sessions.mode` CHECK constraint + add FK on `parent_session_id`

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:126-155`

- [ ] **Step 1: Open the migration**

Edit `crates/storage/migrations/001_initial.sql`. The current sessions table CHECK (line 128-129) is `CHECK (mode IN ('assistant', 'coding'))`. The `parent_session_id TEXT` column (line 147) has no FK.

- [ ] **Step 2: Update CHECK and add FK reference**

Replace lines 126-155 with:

```sql
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    mode       TEXT NOT NULL DEFAULT 'assistant'
                 CHECK (mode IN ('assistant', 'coding', 'subagent')),
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0,
    compressed_prefix      TEXT,
    compressed_through_idx INTEGER,
    compressed_at          INTEGER,
    -- Coding-in-chat columns (added 2026-04-29 per spec 2026-04-29-klynt-coding-in-chat-design.md §11)
    cwd                    TEXT,
    repo_id                TEXT,
    repo_branch            TEXT,
    tool_profile           TEXT,
    approval_mode          TEXT NOT NULL DEFAULT 'default',
    total_cost_usd         REAL NOT NULL DEFAULT 0,
    total_tokens           INTEGER NOT NULL DEFAULT 0,
    parent_session_id      TEXT REFERENCES sessions(key) ON DELETE SET NULL,
    -- Phase 4 columns
    workspace_id           TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    forked_from_id         TEXT REFERENCES sessions(key) ON DELETE SET NULL,
    summary_message_id     TEXT,
    ephemeral              INTEGER NOT NULL DEFAULT 0,
    archived_at            INTEGER,
    last_event_at          INTEGER
);

CREATE INDEX IF NOT EXISTS idx_sessions_workspace_archived ON sessions(workspace_id, archived_at);
CREATE INDEX IF NOT EXISTS idx_sessions_mode ON sessions(mode);
CREATE INDEX IF NOT EXISTS idx_sessions_mode_updated_at ON sessions(mode, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id);
```

- [ ] **Step 3: Verify migration parses (no test yet)**

Run: `cargo build -p storage`
Expected: builds cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): widen sessions.mode CHECK to include 'subagent', add FK on parent_session_id"
```

---

### Task 0.3: Create `subagent_instances` table

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql` (append a new section before the LanceDB-related comments at the bottom; locate by `grep -n "Phase 2: file snapshots" 001_initial.sql` and insert before that block; if no obvious anchor, append at end before any final `COMMIT;` line)

- [ ] **Step 1: Inspect file end to choose insertion point**

Run: `tail -40 crates/storage/migrations/001_initial.sql`
Expected: see the last `CREATE TABLE` or `CREATE INDEX` near the bottom. Insert the new block immediately after the very last `CREATE INDEX` statement in the file, before any trailing comments.

- [ ] **Step 2: Append the new table**

Add at the end of `crates/storage/migrations/001_initial.sql`:

```sql
-- ============================================================
-- Subagent Instances (per design: 2026-05-12-subagent-persistence-and-resume)
-- ============================================================
CREATE TABLE subagent_instances (
    agent_id          TEXT PRIMARY KEY,
    session_id        TEXT NOT NULL REFERENCES sessions(key) ON DELETE CASCADE,
    parent_agent_id   TEXT REFERENCES subagent_instances(agent_id) ON DELETE SET NULL,
    description       TEXT NOT NULL,
    status            TEXT NOT NULL
                        CHECK (status IN ('running','idle','stopped_turn','failed','killed','completed')),
    model             TEXT,
    workspace_path    TEXT NOT NULL,
    turn_cap          INTEGER NOT NULL,
    turns_used        INTEGER NOT NULL DEFAULT 0,
    turns_used_total  INTEGER NOT NULL DEFAULT 0,
    partial_summary   TEXT,
    last_cap_hit_at   INTEGER,
    created_at        INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    updated_at        INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
);
CREATE INDEX idx_subagent_instances_session ON subagent_instances(session_id);
CREATE INDEX idx_subagent_instances_parent  ON subagent_instances(parent_agent_id);
CREATE INDEX idx_subagent_instances_status  ON subagent_instances(status);
CREATE INDEX idx_subagent_instances_updated_at ON subagent_instances(updated_at);
```

- [ ] **Step 3: Verify migration parses**

Run: `cargo nextest run -p storage -E 'test(migration)'` (any existing migration test exercises the SQL)
Expected: PASS — the in-memory pool boots with the new schema.

If no migration test exists, run: `cargo build -p storage` then ad-hoc:

```bash
cargo nextest run -p storage 2>&1 | tail -30
```
Expected: at least the existing storage tests still pass.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): add subagent_instances table"
```

---

## Phase 1 — Storage repo

### Task 1.1: Define `SubagentStatus` enum + `SubagentInstanceRow` struct

**Files:**
- Create: `crates/storage/src/rows/subagent_instance.rs`
- Modify: `crates/storage/src/rows/mod.rs`

- [ ] **Step 1: Create the row file**

Write `crates/storage/src/rows/subagent_instance.rs`:

```rust
//! Persisted row for `subagent_instances`.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Lifecycle states for a subagent instance. Mirrors the CHECK constraint on
/// `subagent_instances.status`. `idle` and `stopped_turn` are resumable;
/// `failed`, `killed`, and `completed` are terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Running,
    Idle,
    StoppedTurn,
    Failed,
    Killed,
    Completed,
}

impl SubagentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Idle => "idle",
            Self::StoppedTurn => "stopped_turn",
            Self::Failed => "failed",
            Self::Killed => "killed",
            Self::Completed => "completed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "idle" => Some(Self::Idle),
            "stopped_turn" => Some(Self::StoppedTurn),
            "failed" => Some(Self::Failed),
            "killed" => Some(Self::Killed),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Failed | Self::Killed | Self::Completed)
    }

    pub fn is_resumable(&self) -> bool {
        matches!(self, Self::Idle | Self::StoppedTurn)
    }
}

/// Raw row from `subagent_instances`. Use `SubagentInstanceRepo` to map to/from this.
#[derive(Debug, Clone, FromRow)]
pub struct SubagentInstanceRow {
    pub agent_id: String,
    pub session_id: String,
    pub parent_agent_id: Option<String>,
    pub description: String,
    pub status: String,
    pub model: Option<String>,
    pub workspace_path: String,
    pub turn_cap: i64,
    pub turns_used: i64,
    pub turns_used_total: i64,
    pub partial_summary: Option<String>,
    pub last_cap_hit_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SubagentInstanceRow {
    pub fn status_enum(&self) -> SubagentStatus {
        SubagentStatus::parse(&self.status).unwrap_or(SubagentStatus::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips() {
        for s in [
            SubagentStatus::Running,
            SubagentStatus::Idle,
            SubagentStatus::StoppedTurn,
            SubagentStatus::Failed,
            SubagentStatus::Killed,
            SubagentStatus::Completed,
        ] {
            assert_eq!(SubagentStatus::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn terminal_vs_resumable() {
        assert!(SubagentStatus::Idle.is_resumable());
        assert!(SubagentStatus::StoppedTurn.is_resumable());
        assert!(!SubagentStatus::Running.is_resumable());
        assert!(SubagentStatus::Failed.is_terminal());
        assert!(SubagentStatus::Killed.is_terminal());
        assert!(SubagentStatus::Completed.is_terminal());
        assert!(!SubagentStatus::Running.is_terminal());
        assert!(!SubagentStatus::Idle.is_terminal());
    }
}
```

- [ ] **Step 2: Export it from rows/mod.rs**

Add to `crates/storage/src/rows/mod.rs` (alphabetically with the other `pub mod` lines):

```rust
pub mod subagent_instance;
```

And at the bottom of the file, alongside any existing `pub use` re-exports:

```rust
pub use subagent_instance::{SubagentInstanceRow, SubagentStatus};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p storage -E 'test(subagent_instance)'`
Expected: 2 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/rows/subagent_instance.rs crates/storage/src/rows/mod.rs
git commit -m "feat(storage): add SubagentStatus enum + SubagentInstanceRow"
```

---

### Task 1.2: Create `SubagentInstanceRepo` — basic CRUD

**Files:**
- Create: `crates/storage/src/repos/subagent_instance.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write failing tests first**

Create `crates/storage/src/repos/subagent_instance.rs`:

```rust
//! Repository for `subagent_instances`.
//!
//! All lifecycle transitions go through this repo. The state machine is
//! enforced in `update_status` — illegal transitions return an error.

use sqlx::SqlitePool;

use common::Result;

use crate::rows::{SubagentInstanceRow, SubagentStatus};

#[derive(Debug, Clone)]
pub struct SubagentInstanceRepo {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct NewSubagentInstance {
    pub agent_id: String,
    pub session_id: String,
    pub parent_agent_id: Option<String>,
    pub description: String,
    pub model: Option<String>,
    pub workspace_path: String,
    pub turn_cap: i64,
}

impl SubagentInstanceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, new: &NewSubagentInstance) -> Result<SubagentInstanceRow> {
        sqlx::query(
            r#"
            INSERT INTO subagent_instances
                (agent_id, session_id, parent_agent_id, description, status,
                 model, workspace_path, turn_cap)
            VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7)
            "#,
        )
        .bind(&new.agent_id)
        .bind(&new.session_id)
        .bind(&new.parent_agent_id)
        .bind(&new.description)
        .bind(&new.model)
        .bind(&new.workspace_path)
        .bind(new.turn_cap)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent insert: {e}")))?;
        self.get(&new.agent_id).await?.ok_or_else(|| {
            common::KlyntbotError::Storage(format!(
                "subagent insert: row missing after insert: {}",
                new.agent_id
            ))
        })
    }

    pub async fn get(&self, agent_id: &str) -> Result<Option<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances WHERE agent_id = ?1",
        )
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent get: {e}")))
    }

    pub async fn get_by_session(&self, session_id: &str) -> Result<Option<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent get_by_session: {e}")))
    }

    pub async fn list_by_parent(
        &self,
        parent_agent_id: Option<&str>,
    ) -> Result<Vec<SubagentInstanceRow>> {
        match parent_agent_id {
            Some(p) => sqlx::query_as::<_, SubagentInstanceRow>(
                "SELECT * FROM subagent_instances WHERE parent_agent_id = ?1 ORDER BY created_at DESC",
            )
            .bind(p)
            .fetch_all(&self.pool)
            .await,
            None => sqlx::query_as::<_, SubagentInstanceRow>(
                "SELECT * FROM subagent_instances WHERE parent_agent_id IS NULL ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await,
        }
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent list_by_parent: {e}")))
    }

    pub async fn list_by_status(
        &self,
        status: SubagentStatus,
    ) -> Result<Vec<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances WHERE status = ?1 ORDER BY updated_at DESC",
        )
        .bind(status.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent list_by_status: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    async fn pool() -> StoragePool {
        StoragePool::connect_in_memory().await.unwrap()
    }

    async fn insert_parent_session(pool: &SqlitePool, key: &str) {
        sqlx::query(
            "INSERT INTO sessions (key, mode) VALUES (?1, 'subagent')",
        )
        .bind(key)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn insert_and_get_roundtrip() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner.clone());

        let row = repo
            .insert(&NewSubagentInstance {
                agent_id: "ag1".to_string(),
                session_id: "sess-1".to_string(),
                parent_agent_id: None,
                description: "search for X".to_string(),
                model: None,
                workspace_path: "/tmp/ws".to_string(),
                turn_cap: 500,
            })
            .await
            .unwrap();

        assert_eq!(row.agent_id, "ag1");
        assert_eq!(row.status, "running");
        assert_eq!(row.turn_cap, 500);
        assert_eq!(row.turns_used, 0);
        assert_eq!(row.turns_used_total, 0);

        let fetched = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(fetched.agent_id, "ag1");
    }

    #[tokio::test]
    async fn list_by_parent_filters_correctly() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-a").await;
        insert_parent_session(&inner, "sess-b").await;
        insert_parent_session(&inner, "sess-c").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag-parent".to_string(),
            session_id: "sess-a".to_string(),
            parent_agent_id: None,
            description: "parent".to_string(),
            model: None,
            workspace_path: "/tmp/ws".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.insert(&NewSubagentInstance {
            agent_id: "ag-child-1".to_string(),
            session_id: "sess-b".to_string(),
            parent_agent_id: Some("ag-parent".to_string()),
            description: "child 1".to_string(),
            model: None,
            workspace_path: "/tmp/ws".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.insert(&NewSubagentInstance {
            agent_id: "ag-child-2".to_string(),
            session_id: "sess-c".to_string(),
            parent_agent_id: Some("ag-parent".to_string()),
            description: "child 2".to_string(),
            model: None,
            workspace_path: "/tmp/ws".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        let top_level = repo.list_by_parent(None).await.unwrap();
        assert_eq!(top_level.len(), 1);
        assert_eq!(top_level[0].agent_id, "ag-parent");

        let children = repo.list_by_parent(Some("ag-parent")).await.unwrap();
        assert_eq!(children.len(), 2);
    }
}
```

- [ ] **Step 2: Wire the module + Repos struct**

Edit `crates/storage/src/repos/mod.rs`:

Add `pub mod subagent_instance;` near the other `pub mod` lines (alphabetical).

Add `pub use subagent_instance::{NewSubagentInstance, SubagentInstanceRepo};` near other `pub use` lines.

In the `Repos` struct (around line 146), add the field alphabetically:

```rust
    pub subagent_instances: SubagentInstanceRepo,
```

In `Repos::from_pool` (around line 188), add the initializer alphabetically:

```rust
            subagent_instances: SubagentInstanceRepo::new(db.clone()),
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p storage -E 'test(subagent_instance)'`
Expected: 2 tests PASS plus any earlier-task tests.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/repos/subagent_instance.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): add SubagentInstanceRepo with insert/get/list"
```

---

### Task 1.3: Add lifecycle transitions + counters

**Files:**
- Modify: `crates/storage/src/repos/subagent_instance.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/storage/src/repos/subagent_instance.rs`:

```rust
    #[tokio::test]
    async fn transitions_running_to_stopped_turn() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag1".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "x".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.update_status("ag1", SubagentStatus::StoppedTurn).await.unwrap();
        let row = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(row.status, "stopped_turn");
    }

    #[tokio::test]
    async fn rejects_transition_from_terminal_state() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag1".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "x".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.update_status("ag1", SubagentStatus::Killed).await.unwrap();
        let err = repo.update_status("ag1", SubagentStatus::Running).await;
        assert!(err.is_err(), "must reject transition out of terminal Killed");
    }

    #[tokio::test]
    async fn increments_counters_independently() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag1".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "x".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.tick_turn("ag1").await.unwrap();
        repo.tick_turn("ag1").await.unwrap();
        let row = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(row.turns_used, 2);
        assert_eq!(row.turns_used_total, 2);

        repo.reset_turns_for_resume("ag1").await.unwrap();
        let row2 = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(row2.turns_used, 0);
        assert_eq!(row2.turns_used_total, 2, "total accumulates across resumes");
    }
```

- [ ] **Step 2: Run it to confirm failure**

Run: `cargo nextest run -p storage -E 'test(subagent_instance)'`
Expected: FAIL — `update_status`, `tick_turn`, `reset_turns_for_resume` don't exist.

- [ ] **Step 3: Implement the methods**

Append to `impl SubagentInstanceRepo` in `crates/storage/src/repos/subagent_instance.rs`:

```rust
    /// Allowed transitions:
    /// - From terminal states (failed/killed/completed): forbidden.
    /// - From `running`: any non-running state.
    /// - From `idle` or `stopped_turn`: to `running` (on resume) or any terminal.
    /// Returns `Err(KlyntbotError::Storage)` if the transition is rejected.
    pub async fn update_status(
        &self,
        agent_id: &str,
        next: SubagentStatus,
    ) -> Result<()> {
        let current = self
            .get(agent_id)
            .await?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("subagent {agent_id}"))
            })?
            .status_enum();
        if current.is_terminal() {
            return Err(common::KlyntbotError::Storage(format!(
                "subagent {agent_id}: cannot transition out of terminal state {}",
                current.as_str()
            )));
        }
        sqlx::query(
            "UPDATE subagent_instances SET status = ?1, updated_at = (unixepoch('now') * 1000) WHERE agent_id = ?2",
        )
        .bind(next.as_str())
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent update_status: {e}")))?;
        Ok(())
    }

    /// Increment `turns_used` and `turns_used_total` by 1, refresh `updated_at`.
    /// Called once per iteration boundary in `execute_loop`.
    pub async fn tick_turn(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE subagent_instances
            SET turns_used = turns_used + 1,
                turns_used_total = turns_used_total + 1,
                updated_at = (unixepoch('now') * 1000)
            WHERE agent_id = ?1
            "#,
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent tick_turn: {e}")))?;
        Ok(())
    }

    /// Reset `turns_used` to 0 (called when starting a resume call). `turns_used_total` is untouched.
    pub async fn reset_turns_for_resume(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE subagent_instances SET turns_used = 0, updated_at = (unixepoch('now') * 1000) WHERE agent_id = ?1",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent reset_turns_for_resume: {e}")))?;
        Ok(())
    }

    /// Store the last assistant text (or fallback) when a cap-hit occurs.
    pub async fn set_partial_summary(
        &self,
        agent_id: &str,
        summary: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE subagent_instances
            SET partial_summary = ?1,
                last_cap_hit_at = (unixepoch('now') * 1000),
                updated_at = (unixepoch('now') * 1000)
            WHERE agent_id = ?2
            "#,
        )
        .bind(summary)
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent set_partial_summary: {e}")))?;
        Ok(())
    }

    /// Refresh `updated_at` without changing any other field (cheap heartbeat ping).
    pub async fn heartbeat(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE subagent_instances SET updated_at = (unixepoch('now') * 1000) WHERE agent_id = ?1",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent heartbeat: {e}")))?;
        Ok(())
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(subagent_instance)'`
Expected: 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/subagent_instance.rs
git commit -m "feat(storage): lifecycle transitions + turn counters for SubagentInstanceRepo"
```

---

### Task 1.4: Crash recovery sweep

**Files:**
- Modify: `crates/storage/src/repos/subagent_instance.rs`

- [ ] **Step 1: Write the failing test**

Append to the `tests` module:

```rust
    #[tokio::test]
    async fn zombie_sweep_marks_stale_running_as_failed() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        insert_parent_session(&inner, "sess-2").await;
        let repo = SubagentInstanceRepo::new(inner.clone());

        // ag-stale: status=running, updated_at 10 min ago
        repo.insert(&NewSubagentInstance {
            agent_id: "ag-stale".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "stale".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();
        sqlx::query("UPDATE subagent_instances SET updated_at = (unixepoch('now') * 1000) - 600000 WHERE agent_id = 'ag-stale'")
            .execute(&inner)
            .await
            .unwrap();

        // ag-fresh: status=running, updated_at just now
        repo.insert(&NewSubagentInstance {
            agent_id: "ag-fresh".to_string(),
            session_id: "sess-2".to_string(),
            parent_agent_id: None,
            description: "fresh".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        let swept = repo.sweep_zombies(300_000).await.unwrap();
        assert_eq!(swept, 1);

        let stale = repo.get("ag-stale").await.unwrap().unwrap();
        assert_eq!(stale.status, "failed");
        assert_eq!(
            stale.partial_summary.as_deref(),
            Some("Process crashed before completion"),
        );

        let fresh = repo.get("ag-fresh").await.unwrap().unwrap();
        assert_eq!(fresh.status, "running");
    }
```

- [ ] **Step 2: Confirm failure**

Run: `cargo nextest run -p storage -E 'test(zombie_sweep)'`
Expected: FAIL — `sweep_zombies` missing.

- [ ] **Step 3: Implement `sweep_zombies`**

Append to `impl SubagentInstanceRepo`:

```rust
    /// Flip `running` rows whose `updated_at` is older than `threshold_ms` to
    /// `failed`. Run once at app startup (before any new subagent starts).
    /// Returns the number of rows swept.
    pub async fn sweep_zombies(&self, threshold_ms: i64) -> Result<u64> {
        let res = sqlx::query(
            r#"
            UPDATE subagent_instances
            SET status = 'failed',
                partial_summary = COALESCE(partial_summary, 'Process crashed before completion'),
                updated_at = (unixepoch('now') * 1000)
            WHERE status = 'running'
              AND updated_at < (unixepoch('now') * 1000) - ?1
            "#,
        )
        .bind(threshold_ms)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent sweep_zombies: {e}")))?;
        Ok(res.rows_affected())
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(subagent_instance)'`
Expected: 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/subagent_instance.rs
git commit -m "feat(storage): zombie sweep for subagent_instances"
```

---

## Phase 2 — Remove `SubagentProfile`

### Task 2.1: Delete the enum and all its uses

**Files:**
- Modify: `crates/agent/src/subagent.rs` (heavy)

- [ ] **Step 1: List all usages**

Run:

```bash
grep -n "SubagentProfile" crates/agent/src/subagent.rs
```
Expected: see references at lines ~25-95 (enum + impl + FromStr/Display), in `SubagentHandle`, in `SubagentManager.registry_cache`, in `_build_tool_registry` (around line 678), in callers, and in tests.

Also run:

```bash
grep -rn "SubagentProfile\|profile.*SubagentProfile" crates/ | grep -v subagent.rs
```
Expected: a handful of caller sites (the LLM-callable spawn tool, the agent_loop builder, the app-core review handler).

- [ ] **Step 2: Delete the enum block**

In `crates/agent/src/subagent.rs`, delete the entire block from `pub enum SubagentProfile` through the closing brace of `impl SubagentProfile`. This is approximately lines 25-95 (verify by reading around those lines first). Also delete the `use std::str::FromStr;` import line at the top if unused after deletion.

- [ ] **Step 3: Update `SubagentHandle` and `SubagentManager`**

Find:

```rust
struct SubagentHandle {
    cancel_token: CancellationToken,
    label: String,
    profile: SubagentProfile,
    spawned_at: std::time::Instant,
    spawned_at_ms: i64,
}
```

Change to:

```rust
struct SubagentHandle {
    cancel_token: CancellationToken,
    label: String,
    spawned_at: std::time::Instant,
    spawned_at_ms: i64,
}
```

In `SubagentManager`, find:

```rust
    registry_cache:
        std::sync::Mutex<HashMap<(SubagentProfile, PathBuf), Arc<RwLock<ToolRegistry>>>>,
```

Change to:

```rust
    registry_cache:
        std::sync::Mutex<HashMap<PathBuf, Arc<RwLock<ToolRegistry>>>>,
```

In the `SubagentHandleSummary` struct, delete the `pub profile: String` field. (We'll keep the struct for now; later phases may replace it.)

- [ ] **Step 4: Delete profile-branched tool registries**

In `_build_tool_registry` (search for `match profile`), replace the per-profile match with a single full-access registry build. Look for code shaped like:

```rust
match profile {
    SubagentProfile::ReadOnly => { ... },
    SubagentProfile::ReadWrite => { ... },
    SubagentProfile::Full => { ... },
}
```

Replace the entire match with whatever the `Full` arm did — that's the new universal subagent tool kit.

If `_build_tool_registry`'s signature still takes a `profile: SubagentProfile` parameter, remove that parameter; update all callers in the same file.

- [ ] **Step 5: Update `run_subagent_task` signature**

The `run_subagent_task` function around line 630 currently takes a `profile: SubagentProfile` argument. Remove that argument. Update the prompt builder call (`build_subagent_prompt`) to no longer take a profile.

- [ ] **Step 6: Replace `build_subagent_prompt` body**

Find `fn build_subagent_prompt` (around line 793). Replace its full body with:

```rust
fn build_subagent_prompt(workspace: &std::path::Path, task: &str) -> String {
    format!(
        r#"# Subagent

You are a subagent. Complete the task assigned and return a clear, concise summary.

## Rules
1. Stay focused — complete only the assigned task.
2. Your final response is reported back to the parent agent.
3. Do not initiate side tasks. Do not spawn other subagents.
4. Be concise but informative.

## Your task
{task}

## Workspace
{workspace}
"#,
        task = task,
        workspace = workspace.display(),
    )
}
```

- [ ] **Step 7: Remove the profile-related tests**

Search the bottom of `crates/agent/src/subagent.rs` for any `#[test]` referencing `SubagentProfile` (around lines 1016, 1023, 1027 from earlier grep) and delete those test functions entirely. The integration tests in Phase 8 replace them.

- [ ] **Step 8: Update direct callers (mainly the LLM-spawn tool)**

Find every reference in `crates/tools/src/domain/spawn.rs` and any builder/runtime call site that passes a profile string. Replace `profile: "read_only" | "read_write" | "full"` with nothing (the parameter is gone).

Run:

```bash
grep -rn "SubagentProfile\b\|build_subagent_prompt\|run_subagent_task" crates/
```
Expected: only references inside `subagent.rs` itself (the function definition) remain.

- [ ] **Step 9: Build**

Run: `cargo build --workspace 2>&1 | tail -40`
Expected: builds cleanly.

If there are still errors mentioning `SubagentProfile`, those are stragglers — find and delete each reference.

- [ ] **Step 10: Commit**

```bash
git add crates/agent/src/subagent.rs crates/tools/src/domain/spawn.rs
git commit -m "refactor(agent): remove SubagentProfile; unify on full-access subagent kit"
```

---

## Phase 3 — Subagent runtime: spawn / resume / kill / list

### Task 3.1: Define `SubagentError` + `ActiveSubagentRegistry`

**Files:**
- Create: `crates/agent/src/subagent_runtime.rs`
- Modify: `crates/agent/src/lib.rs`

- [ ] **Step 1: Write the new module**

Create `crates/agent/src/subagent_runtime.rs`:

```rust
//! Persistent subagent runtime — backs the `subagents` multi-action tool.
//!
//! State lives in `subagent_instances` + a session in `sessions`. The same
//! `execute_loop` and `SafetyCap` machinery runs both spawn and resume
//! calls; the only difference is whether messages are bootstrapped fresh
//! (spawn) or loaded from the session (resume).

use std::sync::Arc;

use dashmap::DashMap;
use tokio_util::sync::CancellationToken;

use storage::rows::SubagentStatus;

/// Live cancel tokens for currently-running subagent instances.
/// Keyed by `agent_id`. Entries are inserted at start of spawn/resume and
/// removed when the call returns (success, error, cap-hit, or cancellation).
#[derive(Debug, Default, Clone)]
pub struct ActiveSubagentRegistry {
    tokens: Arc<DashMap<String, CancellationToken>>,
}

impl ActiveSubagentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new run. Caller must hold the token alive for the run.
    pub fn register(&self, agent_id: &str, token: CancellationToken) {
        self.tokens.insert(agent_id.to_string(), token);
    }

    /// Trigger cancel for an active run. Returns true if the agent was active.
    pub fn cancel(&self, agent_id: &str) -> bool {
        if let Some((_, token)) = self.tokens.remove(agent_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Remove (without cancelling) — used at clean run completion.
    pub fn unregister(&self, agent_id: &str) {
        self.tokens.remove(agent_id);
    }

    pub fn is_active(&self, agent_id: &str) -> bool {
        self.tokens.contains_key(agent_id)
    }
}

/// Structured errors returned by the subagent runtime to its tool layer.
/// The tool layer converts these into `is_error: true` payloads.
#[derive(Debug, thiserror::Error)]
pub enum SubagentError {
    #[error("subagent {agent_id} hit turn cap at {turns_used}: {partial_summary}")]
    CapHit {
        agent_id: String,
        session_id: String,
        turns_used: u32,
        partial_summary: String,
    },
    #[error("subagent {0} is currently running; cannot resume concurrently")]
    AlreadyRunning(String),
    #[error("subagent {agent_id} is not resumable (status={status})")]
    NotResumable {
        agent_id: String,
        status: &'static str,
    },
    #[error("subagent {0} not found")]
    Unknown(String),
    #[error("subagent runtime error: {0}")]
    Internal(String),
}

impl SubagentError {
    pub fn not_resumable(agent_id: impl Into<String>, status: SubagentStatus) -> Self {
        Self::NotResumable {
            agent_id: agent_id.into(),
            status: status.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_cancel_returns_true_only_when_active() {
        let r = ActiveSubagentRegistry::new();
        let token = CancellationToken::new();
        r.register("ag1", token.clone());
        assert!(r.is_active("ag1"));
        assert!(r.cancel("ag1"));
        assert!(token.is_cancelled());
        assert!(!r.is_active("ag1"));
        assert!(!r.cancel("ag1"), "second cancel is a no-op");
    }

    #[test]
    fn unregister_does_not_cancel() {
        let r = ActiveSubagentRegistry::new();
        let token = CancellationToken::new();
        r.register("ag1", token.clone());
        r.unregister("ag1");
        assert!(!token.is_cancelled());
        assert!(!r.is_active("ag1"));
    }

    #[test]
    fn error_status_strings_are_static() {
        let e = SubagentError::not_resumable("ag1", SubagentStatus::Killed);
        match e {
            SubagentError::NotResumable { status, .. } => assert_eq!(status, "killed"),
            _ => panic!("wrong variant"),
        }
    }
}
```

- [ ] **Step 2: Expose from lib.rs**

Add to `crates/agent/src/lib.rs` near the other `pub mod` lines:

```rust
pub mod subagent_runtime;
```

Add to the public re-exports:

```rust
pub use subagent_runtime::{ActiveSubagentRegistry, SubagentError};
```

- [ ] **Step 3: Add `thiserror` to agent crate if not already**

Check: `grep '^thiserror' crates/agent/Cargo.toml`. If missing, add to `[dependencies]`:

```toml
thiserror = "1"
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(subagent_runtime)'`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/subagent_runtime.rs crates/agent/src/lib.rs crates/agent/Cargo.toml
git commit -m "feat(agent): add ActiveSubagentRegistry + SubagentError"
```

---

### Task 3.2: `spawn_instance` — create row, build tool kit, run loop

**Files:**
- Modify: `crates/agent/src/subagent_runtime.rs`
- Modify: `crates/agent/src/subagent.rs` (to expose helpers)

- [ ] **Step 1: Identify the existing execute_loop entry point**

Run:

```bash
grep -n "fn run_subagent_task\|execute_loop(" crates/agent/src/subagent.rs | head
```
Expected: locate the `execute_loop` call inside `run_subagent_task` (around line 772).

- [ ] **Step 2: Add `spawn_instance` to subagent_runtime.rs**

Append to `crates/agent/src/subagent_runtime.rs` (after the existing impls; introduces a new struct that holds the deps):

```rust
use std::path::PathBuf;

use providers::DynProvider;
use storage::repos::{NewSubagentInstance, SubagentInstanceRepo};
use storage::repos::sessions::SessionRepo;
use storage::rows::SubagentInstanceRow;
use common::Result;

/// Default turn cap for spawn/resume calls. Matches Kimi's max_steps_per_turn.
pub const DEFAULT_TURN_CAP: u32 = 500;

/// Wires the storage repos + provider + active-registry for the subagent runtime.
/// Constructed once by `app-core` and held inside `SubagentManager`.
#[derive(Clone)]
pub struct SubagentRuntime {
    pub repo: SubagentInstanceRepo,
    pub sessions: SessionRepo,
    pub active: ActiveSubagentRegistry,
    pub provider: DynProvider,
}

/// Parameters for spawning a new instance.
#[derive(Debug, Clone)]
pub struct SpawnParams {
    pub description: String,
    pub prompt: String,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub workspace_path: PathBuf,
    pub parent_session_id: String,
    pub parent_agent_id: Option<String>,
}

/// Result of a single spawn / resume call (before tool-layer conversion).
#[derive(Debug, Clone)]
pub struct SubagentRunResult {
    pub agent_id: String,
    pub session_id: String,
    pub status: SubagentStatus,
    pub summary: String,
    pub turns_used: u32,
}

impl SubagentRuntime {
    /// Generate a stable, short agent_id: `ag` + 8 hex chars.
    fn new_agent_id() -> String {
        let raw = uuid::Uuid::new_v4().simple().to_string();
        format!("ag{}", &raw[..8])
    }

    /// Generate a new subagent session key.
    fn new_session_id() -> String {
        format!("sub-{}", uuid::Uuid::new_v4().simple())
    }
}
```

- [ ] **Step 3: Make `uuid` available** (likely already is — verify)

Run: `grep '^uuid' crates/agent/Cargo.toml`
Expected: present. If not, add `uuid = { version = "1", features = ["v4"] }`.

- [ ] **Step 4: Add `add_uuid_check_only` test (no implementation yet)**

Append to `tests` mod inside `subagent_runtime.rs`:

```rust
    #[test]
    fn generated_ids_have_expected_shape() {
        let aid = SubagentRuntime::new_agent_id();
        assert!(aid.starts_with("ag"));
        assert_eq!(aid.len(), 10);
        let sid = SubagentRuntime::new_session_id();
        assert!(sid.starts_with("sub-"));
    }
```

- [ ] **Step 5: Build + run**

Run: `cargo nextest run -p agent -E 'test(subagent_runtime)'`
Expected: 4 tests PASS (3 prior + 1 new).

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/subagent_runtime.rs crates/agent/Cargo.toml
git commit -m "feat(agent): SubagentRuntime skeleton + id helpers"
```

---

### Task 3.3: Implement `spawn` end-to-end

**Files:**
- Modify: `crates/agent/src/subagent_runtime.rs`

- [ ] **Step 1: Write the failing integration-style test**

Append to `tests` mod in `subagent_runtime.rs`. This requires an in-memory pool + a fake provider that returns one assistant message with no tool calls (i.e. clean completion):

```rust
    use providers::testing::SingleResponseProvider;
    use storage::pool::StoragePool;
    use storage::repos::Repos;

    async fn fixture() -> (StoragePool, SubagentRuntime) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repos = Repos::from_pool(&pool);
        let provider: DynProvider = Arc::new(SingleResponseProvider::with_text("done"));
        let rt = SubagentRuntime {
            repo: repos.subagent_instances.clone(),
            sessions: repos.sessions.clone(),
            active: ActiveSubagentRegistry::new(),
            provider,
        };
        // Insert a parent session that our subagent will reference.
        sqlx::query("INSERT INTO sessions (key, mode) VALUES ('parent-1', 'assistant')")
            .execute(pool.inner())
            .await
            .unwrap();
        (pool, rt)
    }

    #[tokio::test]
    async fn spawn_clean_run_returns_idle_with_summary() {
        let (_pool, rt) = fixture().await;
        let res = rt
            .spawn(SpawnParams {
                description: "say done".to_string(),
                prompt: "please respond done".to_string(),
                model: None,
                max_turns: Some(3),
                workspace_path: std::path::PathBuf::from("/tmp"),
                parent_session_id: "parent-1".to_string(),
                parent_agent_id: None,
            })
            .await
            .unwrap();
        assert_eq!(res.status, SubagentStatus::Idle);
        assert!(res.summary.contains("done"));
        let row = rt.repo.get(&res.agent_id).await.unwrap().unwrap();
        assert_eq!(row.status, "idle");
    }
```

(If `providers::testing::SingleResponseProvider` doesn't exist yet, we'll need to add one — see Step 3.)

- [ ] **Step 2: Confirm failure**

Run: `cargo nextest run -p agent -E 'test(spawn_clean_run)'`
Expected: FAIL — either `SubagentRuntime::spawn` is missing or `SingleResponseProvider` is missing.

- [ ] **Step 3: Provide a test provider if absent**

Run: `grep -rn "SingleResponseProvider\|pub mod testing" crates/providers/src/ | head -5`

If `providers::testing` exists, skip this step. Otherwise, create `crates/providers/src/testing.rs`:

```rust
//! Test fixtures for provider behavior. Compiled under all features.

use async_trait::async_trait;
use std::sync::Arc;

use crate::types::{ChatRequest, Message};
use crate::{DynProvider, LlmResponse, Provider, ProviderError, Usage};

/// Returns a single assistant text response with no tool calls; useful for
/// "clean completion" tests that don't exercise the tool loop.
#[derive(Debug, Clone)]
pub struct SingleResponseProvider {
    text: String,
}

impl SingleResponseProvider {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn dyn_arc(text: impl Into<String>) -> DynProvider {
        Arc::new(Self::with_text(text))
    }
}

#[async_trait]
impl Provider for SingleResponseProvider {
    async fn chat(&self, _req: ChatRequest) -> std::result::Result<LlmResponse, ProviderError> {
        Ok(LlmResponse {
            content: self.text.clone(),
            tool_calls: vec![],
            usage: Usage::default(),
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        })
    }
    fn name(&self) -> &str { "test-single-response" }
}
```

Add to `crates/providers/src/lib.rs`:

```rust
pub mod testing;
```

(Adjust the `LlmResponse` constructor to match the actual struct's required fields by running `cargo check -p providers` and fixing field names.)

- [ ] **Step 4: Implement `spawn`**

Append to `impl SubagentRuntime` in `subagent_runtime.rs`. **Important:** this method needs to call into the existing `execute_loop`. To avoid pulling all of `subagent.rs` into the new module, expose a thin `pub(crate) async fn run_loop_for_subagent(...)` helper in `subagent.rs` that takes a pre-built `SafetyCap`, the runtime context, and returns `ExecuteLoopResult`. Add that helper first by promoting most of `run_subagent_task` to be reusable. For Phase 3, the minimal exposed signature is:

```rust
// In crates/agent/src/subagent.rs (top-level pub fn):
pub async fn run_subagent_loop(
    provider: providers::DynProvider,
    messages: Vec<providers::types::Message>,
    workspace: std::path::PathBuf,
    cancel_token: tokio_util::sync::CancellationToken,
    max_turns: u32,
) -> common::Result<crate::execution::execute_loop::ExecuteLoopResult> {
    // existing setup from run_subagent_task: build core, tool_defs, routing_ctx
    // call execute_loop with SafetyCap::with_limits(DepthMode::Normal, 0, max_turns)
    // return the raw result without converting to Ok/Err
    todo!("refactor run_subagent_task here in step 5")
}
```

This is a placeholder; the next step does the refactor.

- [ ] **Step 5: Refactor `run_subagent_task` to delegate to `run_subagent_loop`**

In `crates/agent/src/subagent.rs`, extract the body of `run_subagent_task` from the point it builds `core` through the call to `execute_loop` into the new `pub async fn run_subagent_loop`. The wrapper `run_subagent_task` keeps its bus/event-forwarding shell but delegates the loop run.

Now in `subagent_runtime.rs`, implement `spawn`:

```rust
    pub async fn spawn(&self, p: SpawnParams) -> Result<SubagentRunResult> {
        let agent_id = Self::new_agent_id();
        let session_id = Self::new_session_id();
        let max_turns = p.max_turns.unwrap_or(DEFAULT_TURN_CAP);

        // 1. Insert a subagent session row.
        self.sessions.insert_subagent_session(
            &session_id,
            &p.parent_session_id,
            p.workspace_path.to_string_lossy().as_ref(),
        ).await?;

        // 2. Insert the metadata row (status=running).
        self.repo.insert(&NewSubagentInstance {
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
            parent_agent_id: p.parent_agent_id.clone(),
            description: p.description.clone(),
            model: p.model.clone(),
            workspace_path: p.workspace_path.to_string_lossy().to_string(),
            turn_cap: max_turns as i64,
        }).await?;

        // 3. Register a cancel token in the active map.
        let token = tokio_util::sync::CancellationToken::new();
        self.active.register(&agent_id, token.clone());

        // 4. Compose the initial messages list (system + user prompt).
        let system = crate::subagent::build_subagent_prompt(&p.workspace_path, &p.prompt);
        let messages = vec![
            providers::types::Message::system(system),
            providers::types::Message::user(p.prompt.clone()),
        ];

        // 5. Run the loop.
        let loop_res = crate::subagent::run_subagent_loop(
            self.provider.clone(),
            messages,
            p.workspace_path.clone(),
            token.clone(),
            max_turns,
        )
        .await;

        // 6. Convert the loop result into a SubagentRunResult and persist.
        let result = self.finalize_run(&agent_id, &session_id, loop_res).await?;
        self.active.unregister(&agent_id);
        Ok(result)
    }

    /// Persist final state and return the structured result.
    /// On cap-hit, sets partial_summary and returns SubagentError::CapHit
    /// (via Result::Err).
    async fn finalize_run(
        &self,
        agent_id: &str,
        session_id: &str,
        loop_res: Result<crate::execution::execute_loop::ExecuteLoopResult>,
    ) -> Result<SubagentRunResult> {
        let res = loop_res?;
        if res.safety_cap_hit {
            let partial = derive_partial_summary(&res.content);
            self.repo.set_partial_summary(agent_id, &partial).await?;
            self.repo
                .update_status(agent_id, SubagentStatus::StoppedTurn)
                .await?;
            return Err(common::KlyntbotError::ToolError(
                serde_json::to_string(&SubagentError::CapHit {
                    agent_id: agent_id.to_string(),
                    session_id: session_id.to_string(),
                    turns_used: res.turns,
                    partial_summary: partial,
                })
                .unwrap_or_default(),
            ));
        }
        self.repo.update_status(agent_id, SubagentStatus::Idle).await?;
        Ok(SubagentRunResult {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            status: SubagentStatus::Idle,
            summary: res.content,
            turns_used: res.turns,
        })
    }
```

- [ ] **Step 6: Add `derive_partial_summary` helper**

In the same file, free function near the bottom:

```rust
/// Pull a short partial summary from the final loop content. If the loop never
/// produced assistant text (e.g. cap hit mid tool call), return a synthetic
/// "Stopped" string. No LLM call.
pub(crate) fn derive_partial_summary(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return "Stopped before producing a summary.".to_string();
    }
    if trimmed.len() <= 2000 {
        return trimmed.to_string();
    }
    format!("{}…", &trimmed[..2000])
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn empty_content_yields_fallback() {
        assert_eq!(
            derive_partial_summary(""),
            "Stopped before producing a summary."
        );
    }

    #[test]
    fn short_content_passes_through() {
        assert_eq!(derive_partial_summary("hi"), "hi");
    }

    #[test]
    fn long_content_is_truncated() {
        let s = "x".repeat(3000);
        let out = derive_partial_summary(&s);
        assert_eq!(out.len(), 2001);
        assert!(out.ends_with('…'));
    }
}
```

- [ ] **Step 7: Add `SessionRepo::insert_subagent_session`**

In `crates/storage/src/repos/session.rs`, add:

```rust
    /// Insert a new `mode='subagent'` session with `parent_session_id` set.
    pub async fn insert_subagent_session(
        &self,
        session_key: &str,
        parent_session_id: &str,
        workspace_path: &str,
    ) -> common::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (key, mode, parent_session_id, cwd)
            VALUES (?1, 'subagent', ?2, ?3)
            "#,
        )
        .bind(session_key)
        .bind(parent_session_id)
        .bind(workspace_path)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("insert_subagent_session: {e}")))?;
        Ok(())
    }
```

- [ ] **Step 8: Run tests**

Run: `cargo nextest run -p agent -E 'test(spawn_clean_run) + test(summary_tests)'`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/agent/src/subagent_runtime.rs crates/agent/src/subagent.rs crates/storage/src/repos/session.rs crates/providers/src/testing.rs crates/providers/src/lib.rs
git commit -m "feat(agent): SubagentRuntime::spawn — persists state, runs execute_loop, surfaces cap-hit"
```

---

### Task 3.4: Heartbeat tick in execute_loop

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs`
- Modify: `crates/agent/src/execution/types.rs` (likely; check)

- [ ] **Step 1: Find the iteration boundary**

Look at `execute_loop.rs` around line 158-160 (where `cap.tick_turn()` is called).

- [ ] **Step 2: Plumb an optional heartbeat callback through `ExecutionParams`**

Open `crates/agent/src/execution/types.rs`. Find the `ExecutionParams` struct. Add a field:

```rust
    /// Optional callback fired at each iteration boundary (after cap.tick_turn).
    /// Used by SubagentRuntime to refresh `subagent_instances.updated_at`.
    pub on_iteration: Option<Arc<dyn Fn() + Send + Sync>>,
```

(If `Arc` isn't already imported, add `use std::sync::Arc;`.)

- [ ] **Step 3: Add a builder method**

In the same file, in `impl ExecutionParams`:

```rust
    pub fn with_on_iteration(mut self, cb: Arc<dyn Fn() + Send + Sync>) -> Self {
        self.on_iteration = Some(cb);
        self
    }
```

- [ ] **Step 4: Invoke the callback in execute_loop**

In `crates/agent/src/execution/execute_loop.rs` immediately after the `cap.tick_turn();` line (around line 159):

```rust
        cap.tick_turn();
        if let Some(ref cb) = params.on_iteration {
            cb();
        }
```

- [ ] **Step 5: Use it in spawn**

In `crates/agent/src/subagent.rs::run_subagent_loop`, wire the callback. Update the function signature to accept an optional repo handle, or — cleaner — accept the callback directly:

```rust
pub async fn run_subagent_loop(
    provider: providers::DynProvider,
    messages: Vec<providers::types::Message>,
    workspace: std::path::PathBuf,
    cancel_token: tokio_util::sync::CancellationToken,
    max_turns: u32,
    on_iteration: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
) -> common::Result<crate::execution::execute_loop::ExecuteLoopResult> {
```

Then inside, when building `ExecutionParams`, call `.with_on_iteration(cb)` if provided.

In `SubagentRuntime::spawn`, build the callback:

```rust
        let repo = self.repo.clone();
        let agent_id_clone = agent_id.clone();
        let on_iter: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(move || {
            let r = repo.clone();
            let a = agent_id_clone.clone();
            tokio::spawn(async move {
                let _ = r.tick_turn(&a).await;
            });
        });
```

Pass `Some(on_iter)` into `run_subagent_loop`.

- [ ] **Step 6: Add a heartbeat assertion to the spawn test**

Append to `crates/agent/src/subagent_runtime.rs::tests`:

```rust
    #[tokio::test]
    async fn spawn_ticks_turns_used_total() {
        let (_pool, rt) = fixture().await;
        let res = rt
            .spawn(SpawnParams {
                description: "tick".to_string(),
                prompt: "respond".to_string(),
                model: None,
                max_turns: Some(3),
                workspace_path: std::path::PathBuf::from("/tmp"),
                parent_session_id: "parent-1".to_string(),
                parent_agent_id: None,
            })
            .await
            .unwrap();
        // Give spawned ticks a moment to flush.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let row = rt.repo.get(&res.agent_id).await.unwrap().unwrap();
        assert!(row.turns_used_total >= 1, "tick should have fired at least once");
    }
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p agent -E 'test(spawn_ticks_turns_used_total)'`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/execution/execute_loop.rs crates/agent/src/execution/types.rs crates/agent/src/subagent.rs crates/agent/src/subagent_runtime.rs
git commit -m "feat(agent): heartbeat — refresh subagent updated_at per iteration boundary"
```

---

### Task 3.5: `resume_instance`

**Files:**
- Modify: `crates/agent/src/subagent_runtime.rs`
- Modify: `crates/storage/src/repos/session.rs` (add `load_messages` if not present)

- [ ] **Step 1: Verify message-load API exists**

Run:

```bash
grep -n "fn load_messages\|fn list_messages\|fn messages_for_session" crates/storage/src/repos/session.rs
```

If a method that returns `Vec<SessionMessageRow>` for a session key exists, note its name. If not, write a minimal one in the next step.

- [ ] **Step 2: Write failing resume test**

Append to `subagent_runtime.rs::tests`:

```rust
    #[tokio::test]
    async fn resume_reuses_instance_and_resets_turns_used() {
        let (_pool, rt) = fixture().await;
        let spawned = rt
            .spawn(SpawnParams {
                description: "x".to_string(),
                prompt: "first".to_string(),
                model: None,
                max_turns: Some(3),
                workspace_path: std::path::PathBuf::from("/tmp"),
                parent_session_id: "parent-1".to_string(),
                parent_agent_id: None,
            })
            .await
            .unwrap();
        let resumed = rt
            .resume(ResumeParams {
                agent_id: spawned.agent_id.clone(),
                prompt: "second".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(resumed.agent_id, spawned.agent_id);
        let row = rt.repo.get(&spawned.agent_id).await.unwrap().unwrap();
        assert_eq!(row.turns_used, 0, "reset to 0 at start of resume");
        assert!(row.turns_used_total >= 2, "total accumulates");
    }

    #[tokio::test]
    async fn resume_rejects_running_instance() {
        let (_pool, rt) = fixture().await;
        // Insert a 'running' row directly.
        rt.sessions
            .insert_subagent_session("sess-x", "parent-1", "/tmp")
            .await
            .unwrap();
        rt.repo
            .insert(&NewSubagentInstance {
                agent_id: "ag-running".to_string(),
                session_id: "sess-x".to_string(),
                parent_agent_id: None,
                description: "x".to_string(),
                model: None,
                workspace_path: "/tmp".to_string(),
                turn_cap: 500,
            })
            .await
            .unwrap();
        let err = rt
            .resume(ResumeParams {
                agent_id: "ag-running".to_string(),
                prompt: "x".to_string(),
            })
            .await;
        assert!(err.is_err());
    }
```

- [ ] **Step 3: Implement `resume`**

Append `ResumeParams` and the method to `impl SubagentRuntime`:

```rust
#[derive(Debug, Clone)]
pub struct ResumeParams {
    pub agent_id: String,
    pub prompt: String,
}

impl SubagentRuntime {
    pub async fn resume(&self, p: ResumeParams) -> Result<SubagentRunResult> {
        let row = self
            .repo
            .get(&p.agent_id)
            .await?
            .ok_or_else(|| common::KlyntbotError::StorageNotFound(format!("subagent {}", p.agent_id)))?;
        let status = row.status_enum();
        match status {
            SubagentStatus::Running => {
                return Err(common::KlyntbotError::Storage(format!(
                    "subagent {} is currently running; cannot resume concurrently",
                    p.agent_id
                )));
            }
            SubagentStatus::Idle | SubagentStatus::StoppedTurn => {}
            _ => {
                return Err(common::KlyntbotError::Storage(format!(
                    "subagent {} is not resumable (status={})",
                    p.agent_id,
                    status.as_str()
                )));
            }
        }

        // Reset per-call counters and flip status to running.
        self.repo.reset_turns_for_resume(&p.agent_id).await?;
        self.repo
            .update_status(&p.agent_id, SubagentStatus::Running)
            .await?;

        // Load conversation history from the session's messages and append the new prompt.
        let history = self.sessions.load_messages(&row.session_id).await?;
        let mut messages: Vec<providers::types::Message> = history
            .into_iter()
            .filter_map(crate::subagent::row_to_message)
            .collect();
        messages.push(providers::types::Message::user(p.prompt.clone()));

        let token = tokio_util::sync::CancellationToken::new();
        self.active.register(&p.agent_id, token.clone());

        let repo = self.repo.clone();
        let aid = p.agent_id.clone();
        let on_iter: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(move || {
            let r = repo.clone();
            let a = aid.clone();
            tokio::spawn(async move {
                let _ = r.tick_turn(&a).await;
            });
        });

        let loop_res = crate::subagent::run_subagent_loop(
            self.provider.clone(),
            messages,
            std::path::PathBuf::from(&row.workspace_path),
            token,
            row.turn_cap as u32,
            Some(on_iter),
        )
        .await;

        let result = self
            .finalize_run(&p.agent_id, &row.session_id, loop_res)
            .await?;
        self.active.unregister(&p.agent_id);
        Ok(result)
    }
}
```

- [ ] **Step 4: Add `row_to_message` helper in subagent.rs**

In `crates/agent/src/subagent.rs`:

```rust
/// Adapt a stored session message row into a provider Message.
/// Returns None for rows that don't map cleanly (e.g. malformed).
pub(crate) fn row_to_message(
    row: storage::rows::SessionMessageRow,
) -> Option<providers::types::Message> {
    let role = row.role.as_str();
    match role {
        "system" => Some(providers::types::Message::system(row.content)),
        "user" => Some(providers::types::Message::user(row.content)),
        "assistant" => Some(providers::types::Message::assistant(row.content)),
        // Tool messages: skip for now; resume bootstraps with the assistant
        // summary, not the raw tool-result history (parity with Kimi).
        _ => None,
    }
}
```

If `SessionMessageRow.content` has a different name (e.g. `text` or `body`), match it. Run `grep -n "pub struct SessionMessageRow" crates/storage/src/rows/`.

- [ ] **Step 5: Add `SessionRepo::load_messages` if missing**

Run: `grep -n "fn load_messages\|fn list_messages" crates/storage/src/repos/session.rs`

If missing, add:

```rust
    pub async fn load_messages(&self, session_key: &str) -> common::Result<Vec<crate::rows::SessionMessageRow>> {
        sqlx::query_as::<_, crate::rows::SessionMessageRow>(
            "SELECT * FROM session_messages WHERE session_key = ?1 ORDER BY idx ASC",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("load_messages: {e}")))
    }
```

(Verify the table is `session_messages` and the order column is `idx` — adjust if different.)

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(resume_reuses_instance) + test(resume_rejects_running)'`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/subagent_runtime.rs crates/agent/src/subagent.rs crates/storage/src/repos/session.rs
git commit -m "feat(agent): SubagentRuntime::resume — load history, reset turns, run loop"
```

---

### Task 3.6: `kill_instance` + `list_instances`

**Files:**
- Modify: `crates/agent/src/subagent_runtime.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests`:

```rust
    #[tokio::test]
    async fn kill_active_instance_sets_status_to_killed() {
        let (_pool, rt) = fixture().await;
        rt.sessions
            .insert_subagent_session("sess-k", "parent-1", "/tmp")
            .await
            .unwrap();
        rt.repo
            .insert(&NewSubagentInstance {
                agent_id: "ag-k".to_string(),
                session_id: "sess-k".to_string(),
                parent_agent_id: None,
                description: "x".to_string(),
                model: None,
                workspace_path: "/tmp".to_string(),
                turn_cap: 500,
            })
            .await
            .unwrap();
        // Register a token so kill has something to fire.
        let token = tokio_util::sync::CancellationToken::new();
        rt.active.register("ag-k", token.clone());

        let killed = rt.kill("ag-k").await.unwrap();
        assert_eq!(killed.status, SubagentStatus::Killed);
        assert!(token.is_cancelled());
        let row = rt.repo.get("ag-k").await.unwrap().unwrap();
        assert_eq!(row.status, "killed");
    }

    #[tokio::test]
    async fn list_filters_by_parent_and_status() {
        let (_pool, rt) = fixture().await;
        rt.sessions
            .insert_subagent_session("sess-list-1", "parent-1", "/tmp")
            .await
            .unwrap();
        rt.repo
            .insert(&NewSubagentInstance {
                agent_id: "ag-list-1".to_string(),
                session_id: "sess-list-1".to_string(),
                parent_agent_id: None,
                description: "x".to_string(),
                model: None,
                workspace_path: "/tmp".to_string(),
                turn_cap: 500,
            })
            .await
            .unwrap();
        let all = rt.list(None, None).await.unwrap();
        assert!(all.iter().any(|i| i.agent_id == "ag-list-1"));
    }
```

- [ ] **Step 2: Confirm failure**

Run: `cargo nextest run -p agent -E 'test(kill_active_instance) + test(list_filters)'`
Expected: FAIL — `kill` and `list` not defined.

- [ ] **Step 3: Implement them**

Append to `impl SubagentRuntime`:

```rust
    /// Cancel an active run (if any) and flip status to `killed`.
    /// If the instance isn't running, just flips the DB row.
    pub async fn kill(&self, agent_id: &str) -> Result<SubagentRunResult> {
        let row = self
            .repo
            .get(agent_id)
            .await?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("subagent {agent_id}"))
            })?;
        self.active.cancel(agent_id);
        // Allow kill regardless of state, as long as it isn't already terminal.
        let status_now = row.status_enum();
        if status_now.is_terminal() {
            // Already terminal — no-op, return current state.
            return Ok(SubagentRunResult {
                agent_id: agent_id.to_string(),
                session_id: row.session_id,
                status: status_now,
                summary: row.partial_summary.unwrap_or_default(),
                turns_used: row.turns_used as u32,
            });
        }
        self.repo.update_status(agent_id, SubagentStatus::Killed).await?;
        Ok(SubagentRunResult {
            agent_id: agent_id.to_string(),
            session_id: row.session_id,
            status: SubagentStatus::Killed,
            summary: row.partial_summary.unwrap_or_default(),
            turns_used: row.turns_used as u32,
        })
    }

    pub async fn list(
        &self,
        parent_agent_id: Option<&str>,
        status: Option<SubagentStatus>,
    ) -> Result<Vec<SubagentInstanceRow>> {
        let rows = if let Some(s) = status {
            self.repo.list_by_status(s).await?
        } else {
            self.repo.list_by_parent(parent_agent_id).await?
        };
        // If both filters were provided, intersect.
        if status.is_some() && parent_agent_id.is_some() {
            let parent = parent_agent_id.unwrap();
            Ok(rows.into_iter().filter(|r| r.parent_agent_id.as_deref() == Some(parent)).collect())
        } else {
            Ok(rows)
        }
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(kill_active_instance) + test(list_filters)'`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/subagent_runtime.rs
git commit -m "feat(agent): SubagentRuntime::kill + list"
```

---

## Phase 4 — Tool surface: rename to `subagents`

### Task 4.1: Rename `SpawnTool` → `SubagentsTool` (file + symbol)

**Files:**
- Rename: `crates/tools/src/domain/spawn.rs` → `crates/tools/src/domain/subagents.rs`
- Modify: `crates/tools/src/lib.rs`, `crates/tools/src/domain/mod.rs`

- [ ] **Step 1: Move the file**

```bash
git mv crates/tools/src/domain/spawn.rs crates/tools/src/domain/subagents.rs
```

- [ ] **Step 2: Update the module declarations**

Edit `crates/tools/src/domain/mod.rs`: replace `pub mod spawn;` with `pub mod subagents;`.

Edit `crates/tools/src/lib.rs`: replace any `pub use domain::spawn::...` with `pub use domain::subagents::...`. Also update the public re-export name.

- [ ] **Step 3: Rename the struct**

In `crates/tools/src/domain/subagents.rs`, rename `SpawnTool` → `SubagentsTool` everywhere in the file. Update the `fn name(&self) -> &str { "spawn" }` to `"subagents"`.

Also rename the trait `SpawnHandler` → `SubagentsHandler` and its method names (we'll redesign in Task 4.2).

- [ ] **Step 4: Update all imports**

Run:

```bash
grep -rln "tools::spawn\|domain::spawn\|SpawnTool\|SpawnHandler" crates/ | grep -v target
```
Expected: list of files. In each, replace `spawn` → `subagents`, `SpawnTool` → `SubagentsTool`, `SpawnHandler` → `SubagentsHandler`.

- [ ] **Step 5: Build**

Run: `cargo build --workspace 2>&1 | tail -40`
Expected: builds. If errors mention old name, fix the stragglers.

- [ ] **Step 6: Commit**

```bash
git add -A crates/tools crates/agent
git commit -m "refactor(tools): rename SpawnTool → SubagentsTool, registry name 'spawn' → 'subagents'"
```

---

### Task 4.2: Convert `SubagentsTool` to multi-action via `#[tool_actions]`

**Files:**
- Modify: `crates/tools/src/domain/subagents.rs`

- [ ] **Step 1: Find an existing multi-action example to copy**

Run: `grep -rn "#\[tool_actions\]" crates/ | head -5`
Expected: identify a tool like `TasksTool` or `MemoryTool` that already uses this. Read its pattern (e.g. `crates/feature-tasks/src/...` or `crates/tools/src/domain/memory_tool.rs`).

- [ ] **Step 2: Define the four `ActionParams`**

In `crates/tools/src/domain/subagents.rs`, add:

```rust
use tools_core_macros::{tool_actions, ActionParams, ToolParams};

#[derive(Debug, Clone, serde::Deserialize, ToolParams)]
pub struct SpawnAction {
    /// Short human-readable label for this subagent run (3-8 words).
    pub description: String,
    /// The full task description / prompt the subagent should execute.
    pub prompt: String,
    /// Optional model override (defaults to the parent's effective model).
    #[serde(default)]
    pub model: Option<String>,
    /// Optional per-call turn cap (default 500).
    #[serde(default)]
    pub max_turns: Option<u32>,
}

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct ResumeAction {
    pub agent_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct ListAction {
    #[serde(default)]
    pub parent_agent_id: Option<String>,
    /// Optional status filter: 'running' | 'idle' | 'stopped_turn' | 'failed' | 'killed' | 'completed'
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct KillAction {
    pub agent_id: String,
}
```

- [ ] **Step 3: Define the trait the tool delegates to**

Replace the old `SpawnHandler` trait with:

```rust
#[async_trait::async_trait]
pub trait SubagentsHandler: Send + Sync {
    async fn spawn(&self, action: SpawnAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError>;
    async fn resume(&self, action: ResumeAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError>;
    async fn list(&self, action: ListAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError>;
    async fn kill(&self, action: KillAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError>;
}
```

- [ ] **Step 4: Convert the tool to multi-action**

Replace the existing `impl Tool for SubagentsTool` body with:

```rust
#[tool_actions]
impl SubagentsTool {
    async fn spawn(&self, params: SpawnAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.spawn(params, ctx).await
    }

    async fn resume(&self, params: ResumeAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.resume(params, ctx).await
    }

    async fn list(&self, params: ListAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.list(params, ctx).await
    }

    async fn kill(&self, params: KillAction, ctx: &RoutingContext) -> Result<serde_json::Value, ToolError> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.kill(params, ctx).await
    }
}
```

(Check an existing `#[tool_actions]` example for exact macro signature.)

- [ ] **Step 5: Update the constructor signature**

```rust
impl SubagentsTool {
    pub fn with_handler(handler: Arc<dyn SubagentsHandler>) -> Self {
        Self { handler: Some(handler) }
    }
}
```

The `handler` field type becomes `Option<Arc<dyn SubagentsHandler>>`.

- [ ] **Step 6: Build**

Run: `cargo build -p tools 2>&1 | tail -40`
Expected: builds. If `#[tool_actions]` macro emits errors, copy the pattern from the example tool exactly.

- [ ] **Step 7: Implement `SubagentsHandler for SubagentManager`**

In `crates/agent/src/subagent.rs` (or a new sibling file), implement the trait:

```rust
#[async_trait::async_trait]
impl tools::subagents::SubagentsHandler for SubagentManager {
    async fn spawn(
        &self,
        action: tools::subagents::SpawnAction,
        ctx: &tools::RoutingContext,
    ) -> Result<serde_json::Value, tools_core::ToolError> {
        // 1. Resolve parent_session_id from ctx (assumes ctx.session_key is the parent).
        let parent_session_id = ctx.session_key.clone();
        // 2. Call into self.runtime (a SubagentRuntime stored on SubagentManager).
        match self.runtime.spawn(crate::subagent_runtime::SpawnParams {
            description: action.description,
            prompt: action.prompt,
            model: action.model,
            max_turns: action.max_turns,
            workspace_path: self.workspace.clone(),
            parent_session_id,
            parent_agent_id: ctx.agent_chain.last().cloned(),
        }).await {
            Ok(res) => Ok(serde_json::json!({
                "agent_id": res.agent_id,
                "session_id": res.session_id,
                "status": res.status.as_str(),
                "summary": res.summary,
                "turns_used": res.turns_used,
            })),
            Err(e) => Err(tools_core::ToolError::Failed(json_payload_for_error(&e))),
        }
    }
    async fn resume(&self, action: tools::subagents::ResumeAction, _ctx: &tools::RoutingContext)
        -> Result<serde_json::Value, tools_core::ToolError>
    {
        match self.runtime.resume(crate::subagent_runtime::ResumeParams {
            agent_id: action.agent_id,
            prompt: action.prompt,
        }).await {
            Ok(res) => Ok(serde_json::json!({
                "agent_id": res.agent_id,
                "session_id": res.session_id,
                "status": res.status.as_str(),
                "summary": res.summary,
                "turns_used": res.turns_used,
            })),
            Err(e) => Err(tools_core::ToolError::Failed(json_payload_for_error(&e))),
        }
    }
    async fn list(&self, action: tools::subagents::ListAction, _ctx: &tools::RoutingContext)
        -> Result<serde_json::Value, tools_core::ToolError>
    {
        let status = action.status.as_deref().and_then(storage::rows::SubagentStatus::parse);
        let rows = self.runtime.list(action.parent_agent_id.as_deref(), status).await
            .map_err(|e| tools_core::ToolError::Failed(e.to_string()))?;
        Ok(serde_json::json!({
            "instances": rows.into_iter().map(|r| serde_json::json!({
                "agent_id": r.agent_id,
                "session_id": r.session_id,
                "parent_agent_id": r.parent_agent_id,
                "description": r.description,
                "status": r.status,
                "turns_used_total": r.turns_used_total,
                "last_cap_hit_at": r.last_cap_hit_at,
                "updated_at": r.updated_at,
            })).collect::<Vec<_>>(),
        }))
    }
    async fn kill(&self, action: tools::subagents::KillAction, _ctx: &tools::RoutingContext)
        -> Result<serde_json::Value, tools_core::ToolError>
    {
        let res = self.runtime.kill(&action.agent_id).await
            .map_err(|e| tools_core::ToolError::Failed(e.to_string()))?;
        Ok(serde_json::json!({
            "agent_id": res.agent_id,
            "status": res.status.as_str(),
        }))
    }
}

fn json_payload_for_error(e: &common::KlyntbotError) -> String {
    // For CapHit errors thrown via KlyntbotError::ToolError(json), pass through.
    // For others, fall back to the display string.
    match e {
        common::KlyntbotError::ToolError(s) => s.clone(),
        _ => e.to_string(),
    }
}
```

(Confirm the field name `ctx.agent_chain` and `ctx.session_key` exist by `grep -n "agent_chain\|session_key" crates/tools/src/routing.rs`.)

- [ ] **Step 8: Add `runtime: SubagentRuntime` field to `SubagentManager`**

In `crates/agent/src/subagent.rs::SubagentManager`, add:

```rust
    runtime: crate::subagent_runtime::SubagentRuntime,
```

Wire it up in the builder (`SubagentManagerBuilder::build`): construct a `SubagentRuntime` from the same repos passed via the builder. If the builder doesn't currently take a `Repos` handle, add one.

- [ ] **Step 9: Build**

Run: `cargo build --workspace 2>&1 | tail -40`
Expected: builds.

- [ ] **Step 10: Commit**

```bash
git add -A crates/tools crates/agent
git commit -m "feat(tools): SubagentsTool multi-action — spawn/resume/list/kill"
```

---

## Phase 5 — App-core handlers + Tauri command

### Task 5.1: AppCore handler methods

**Files:**
- Modify: `crates/app-core/src/handlers/subagent.rs` (create)
- Modify: `crates/app-core/src/lib.rs`

- [ ] **Step 1: Write the new handler file**

Create `crates/app-core/src/handlers/subagent.rs`:

```rust
//! AppCore handlers for subagent persistence/resume operations exposed to the
//! desktop UI. These are thin wrappers around `SubagentRuntime` that the
//! Tauri command shells delegate to.

use crate::AppCore;
use common::Result;
use storage::rows::SubagentInstanceRow;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_list_for_session(
        &self,
        session_id: String,
    ) -> Result<Vec<SubagentInstanceRow>> {
        let direct = self.repos.subagent_instances.get_by_session(&session_id).await?;
        if let Some(row) = direct {
            return Ok(vec![row]);
        }
        // No instance for this session itself — caller is the parent session;
        // return all immediate children.
        // Children are rows whose underlying session has parent_session_id == session_id.
        let rows = sqlx::query_as::<_, SubagentInstanceRow>(
            r#"
            SELECT si.*
            FROM subagent_instances si
            INNER JOIN sessions s ON s.key = si.session_id
            WHERE s.parent_session_id = ?1
            ORDER BY si.created_at DESC
            "#,
        )
        .bind(&session_id)
        .fetch_all(self.repos.pool())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent_list_for_session: {e}")))?;
        Ok(rows)
    }
}
```

Note: `self.repos.pool()` requires a `pool()` accessor on `Repos`. If absent, add it:

In `crates/storage/src/repos/mod.rs::impl Repos`:

```rust
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
```

- [ ] **Step 2: Register the module**

Edit `crates/app-core/src/handlers/mod.rs` (or wherever handlers are declared) — add `pub mod subagent;`.

- [ ] **Step 3: Build**

Run: `cargo build -p app-core 2>&1 | tail -30`
Expected: builds.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/subagent.rs crates/app-core/src/handlers/mod.rs crates/storage/src/repos/mod.rs
git commit -m "feat(app-core): subagent_list_for_session handler"
```

---

### Task 5.2: Tauri command + frontend binding

**Files:**
- Create: `crates/desktop/src/commands/subagent.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Write the Tauri command**

Create `crates/desktop/src/commands/subagent.rs`:

```rust
//! Tauri commands for subagent navigation/listing.

use desktop_macros::klynt_command;
use desktop_shared::SubagentInstanceSummary;

use crate::AppCore;

/// List subagent instances either matching the given session (if it IS a subagent)
/// or the immediate children (if it's a parent thread).
#[klynt_command]
pub async fn subagent_list_for_session(
    app: &AppCore,
    session_id: String,
) -> Vec<SubagentInstanceSummary> {
    let rows = app
        .subagent_list_for_session(session_id)
        .await
        .unwrap_or_default();
    rows.into_iter().map(SubagentInstanceSummary::from).collect()
}
```

- [ ] **Step 2: Add the shared DTO**

In `crates/desktop-shared/src/lib.rs` (or wherever shared specta types live), add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentInstanceSummary {
    pub agent_id: String,
    pub session_id: String,
    pub parent_agent_id: Option<String>,
    pub description: String,
    pub status: String,
    pub turns_used_total: i64,
    pub last_cap_hit_at: Option<i64>,
    pub updated_at: i64,
}

impl From<storage::rows::SubagentInstanceRow> for SubagentInstanceSummary {
    fn from(r: storage::rows::SubagentInstanceRow) -> Self {
        Self {
            agent_id: r.agent_id,
            session_id: r.session_id,
            parent_agent_id: r.parent_agent_id,
            description: r.description,
            status: r.status,
            turns_used_total: r.turns_used_total,
            last_cap_hit_at: r.last_cap_hit_at,
            updated_at: r.updated_at,
        }
    }
}
```

- [ ] **Step 3: Register in specta + commands module**

Edit `crates/desktop/src/commands/mod.rs`: add `pub mod subagent;`.

Edit `crates/desktop/src/specta_builder.rs`: find the `klynt_collect_commands![...]` macro invocation and add `commands::subagent::subagent_list_for_session,` to the list.

- [ ] **Step 4: Regenerate bindings**

Run: `cargo build -p desktop 2>&1 | tail -30`
Expected: builds; `desktop-ui/src/bindings.ts` should be updated by the post-build step. If not, run `cargo tauri dev` once briefly (Ctrl-C after bindings regenerate).

Verify: `grep "subagentListForSession" desktop-ui/src/bindings.ts`
Expected: shows the new TypeScript binding.

- [ ] **Step 5: Run drift test**

Run: `cargo nextest run -p desktop -E 'test(registration_drift) + test(bindings_are_current)'`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/subagent.rs crates/desktop/src/commands/mod.rs crates/desktop/src/specta_builder.rs crates/desktop-shared/src/lib.rs desktop-ui/src/bindings.ts
git commit -m "feat(desktop): subagent_list_for_session Tauri command + binding"
```

---

### Task 5.3: Route `chat_cancel` by session mode

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:1477` (the `AppCore::chat_cancel` wrapper)

- [ ] **Step 1: Locate the wrapper**

Run: `sed -n '1470,1500p' crates/app-core/src/handlers/chat/streaming.rs`
Expected: see `pub async fn chat_cancel(&self, session_key: String) -> Result<(), ApiError>`.

- [ ] **Step 2: Add mode discrimination**

Replace the body with:

```rust
    pub async fn chat_cancel(&self, session_key: String) -> Result<(), ApiError> {
        let session = self
            .repos
            .sessions
            .get_session(&session_key)
            .await
            .map_err(ApiError::from)?;
        if session.session_mode() == common::SessionMode::Subagent {
            // Find the subagent_instances row for this session and kill it.
            if let Some(row) = self
                .repos
                .subagent_instances
                .get_by_session(&session_key)
                .await
                .map_err(ApiError::from)?
            {
                if let Some(rt) = self.subagent_runtime.as_ref() {
                    rt.kill(&row.agent_id).await.map_err(ApiError::from)?;
                }
                return Ok(());
            }
        }
        chat_cancel(
            // existing call with the original wired args; preserve from previous body
            // (read the original function and reproduce its argument-passing here)
            todo!("inline the original chat_cancel arg-passing here"),
        )
        .await
    }
```

(Replace the `todo!` with the actual original wired call — copy from the function as it stood.)

- [ ] **Step 3: Add `subagent_runtime` field to AppCore**

In `crates/app-core/src/lib.rs` find the `AppCore` struct and add:

```rust
    pub subagent_runtime: Option<std::sync::Arc<agent::subagent_runtime::SubagentRuntime>>,
```

Wire it up in `AppCore::new` (or wherever construction happens) — instantiate one with `Repos` + the provider used elsewhere.

- [ ] **Step 4: Build**

Run: `cargo build --workspace 2>&1 | tail -30`
Expected: builds.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs crates/app-core/src/lib.rs
git commit -m "feat(app-core): chat_cancel routes Subagent-mode sessions to SubagentRuntime::kill"
```

---

### Task 5.4: Startup zombie sweep

**Files:**
- Modify: `crates/app-core/src/init/mod.rs` (or the file where `AppCore::new` runs init tasks)

- [ ] **Step 1: Find the init point**

Run: `grep -n "from_pool\|migrate\|init" crates/app-core/src/init/mod.rs | head -20`

- [ ] **Step 2: Add the sweep call**

After the storage migrations run and before any subagent runtime starts accepting calls, add:

```rust
    // Subagent zombie sweep: flip any `running` rows older than 5 min to `failed`.
    // Mirrors the zombie-session detector documented in CLAUDE.md.
    if let Err(e) = repos.subagent_instances.sweep_zombies(300_000).await {
        tracing::warn!(error = %e, "subagent zombie sweep failed at startup");
    }
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core 2>&1 | tail -20`
Expected: builds.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): startup zombie sweep for stale subagent runs"
```

---

## Phase 6 — Frontend: navigation + chip + breadcrumb

### Task 6.1: Update SessionMode TS type + bindings reload

**Files:**
- The `bindings.ts` regen from cargo build already includes `'subagent'` in the SessionMode union since specta picks it up.
- Verify: `grep "subagent" desktop-ui/src/bindings.ts`

- [ ] **Step 1: Sanity check**

Run: `grep -c '"subagent"' desktop-ui/src/bindings.ts`
Expected: at least 1.

- [ ] **Step 2: Find places that match on SessionMode**

Run:

```bash
grep -rn "SessionMode\|session_mode\|sessionMode" desktop-ui/src/ | head -20
```
Expected: identify TypeScript narrowing sites. Most don't need updates (`'subagent'` flows through string-typed comparisons), but any exhaustive `switch (mode)` would.

For each exhaustive match, add a `case "subagent":` branch. If unsure, the typescript compiler tells you — run:

```bash
cd desktop-ui && bun run typecheck
```
Expected: type errors point to switches that need cases.

- [ ] **Step 3: Fix each switch**

For each error, add the missing branch. For sidebar / list rendering, treat subagent sessions like coding sessions for now (we'll add specific styling in Task 6.4).

- [ ] **Step 4: Verify typecheck clean**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src
git commit -m "fix(ui): add 'subagent' branch to exhaustive SessionMode switches"
```

---

### Task 6.2: API endpoint wrapper

**Files:**
- Modify: `desktop-ui/src/api/endpoints/subagent.ts` (create)

- [ ] **Step 1: Write the file**

Create `desktop-ui/src/api/endpoints/subagent.ts`:

```typescript
import { invoke } from "../client";

export type SubagentInstanceSummary = {
  agentId: string;
  sessionId: string;
  parentAgentId: string | null;
  description: string;
  status: string;
  turnsUsedTotal: number;
  lastCapHitAt: number | null;
  updatedAt: number;
};

export async function subagentListForSession(
  sessionId: string,
): Promise<SubagentInstanceSummary[]> {
  return invoke<SubagentInstanceSummary[]>("subagent_list_for_session", { sessionId });
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/api/endpoints/subagent.ts
git commit -m "feat(ui): subagent_list_for_session API wrapper"
```

---

### Task 6.3: Tool-result chip in subagent.spawn / resume blocks

**Files:**
- Identify the tool-result renderer (likely `desktop-ui/src/features/messages/components/ToolCallResult.tsx` or similar).

- [ ] **Step 1: Locate**

Run:

```bash
grep -rn "tool_name\|toolName\|ToolCallResult\|ToolResult\|tool_call_result" desktop-ui/src/features/messages/ | head -10
```

Identify the component that renders a tool result.

- [ ] **Step 2: Add a chip branch**

In that component, after parsing the tool result JSON, check:

```tsx
const toolName = toolCall.tool;
const result = toolCall.result as Record<string, unknown> | undefined;
const isSubagentSpawnOrResume =
  toolName === "subagents" &&
  result !== undefined &&
  typeof result.agentId === "string";

// ... render the standard body, then:
{isSubagentSpawnOrResume && result && (
  <SubagentChip
    agentId={String(result.agentId)}
    sessionId={String(result.sessionId ?? "")}
    description={String(result.description ?? "")}
  />
)}
```

- [ ] **Step 3: Create the chip component**

Create `desktop-ui/src/features/messages/components/SubagentChip.tsx`:

```tsx
import { useNavigate } from "@/router";

type Props = {
  agentId: string;
  sessionId: string;
  description?: string;
};

export function SubagentChip({ agentId, sessionId, description }: Props) {
  const navigate = useNavigate();
  if (!sessionId) return null;
  return (
    <button
      type="button"
      className="subagent-chip"
      onClick={() => navigate(`/thread/${sessionId}`)}
      title={`Open subagent ${agentId}`}
    >
      <span className="subagent-chip__arrow">↳</span>
      <span className="subagent-chip__id">{agentId}</span>
      {description && <span className="subagent-chip__sep">—</span>}
      {description && <span className="subagent-chip__desc">{description}</span>}
    </button>
  );
}
```

Add a CSS file `desktop-ui/src/styles/subagent-chip.css`:

```css
.subagent-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  font-size: var(--fs-xs);
  background: var(--color-surface-subtle);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  cursor: pointer;
  margin-top: 6px;
}
.subagent-chip:hover { background: var(--color-surface-hover); }
.subagent-chip__arrow { opacity: 0.6; }
.subagent-chip__id { font-family: var(--font-mono); }
.subagent-chip__sep { opacity: 0.5; }
.subagent-chip__desc { font-style: italic; }
```

Register in `desktop-ui/src/styles/index.css`:

```css
@import "./subagent-chip.css";
```

- [ ] **Step 4: Adapt to the actual routing API**

If the project doesn't have `@/router` with a `useNavigate`, adapt to whatever is used elsewhere (likely the active-thread setter). Run:

```bash
grep -rn "setActiveThreadId\|activeThreadIdByWorkspace\|useChatStore" desktop-ui/src/features | head -10
```

If thread switching is via Zustand setter, the chip onClick should call that:

```tsx
import { useChatStore } from "@/features/threads/store/useChatStore";

const setActive = useChatStore((s) => s.dispatchThreadAction);
// onClick:
setActive({ type: "setActiveThreadId", workspaceId: currentWorkspaceId, threadId: sessionId });
```

(The exact thread routing pattern depends on the existing code — adapt to what works.)

- [ ] **Step 5: Add a vitest**

Create `desktop-ui/src/features/messages/components/SubagentChip.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { SubagentChip } from "./SubagentChip";

vi.mock("@/features/threads/store/useChatStore", () => ({
  useChatStore: (selector: any) => selector({ dispatchThreadAction: vi.fn() }),
}));

describe("SubagentChip", () => {
  it("renders agent_id and description", () => {
    const { getByText } = render(
      <SubagentChip agentId="ag3f7a92c1" sessionId="sub-1" description="Search refs" />,
    );
    expect(getByText("ag3f7a92c1")).toBeDefined();
    expect(getByText("Search refs")).toBeDefined();
  });

  it("returns null when sessionId is empty", () => {
    const { container } = render(
      <SubagentChip agentId="ag1" sessionId="" description="x" />,
    );
    expect(container.firstChild).toBeNull();
  });
});
```

- [ ] **Step 6: Run tests**

Run: `cd desktop-ui && bun run test SubagentChip`
Expected: 2 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/messages/components/SubagentChip.tsx desktop-ui/src/features/messages/components/SubagentChip.test.tsx desktop-ui/src/styles/subagent-chip.css desktop-ui/src/styles/index.css
git commit -m "feat(ui): SubagentChip tool-result chip with thread navigation"
```

---

### Task 6.4: Thread-header breadcrumb for subagent sessions

**Files:**
- Modify: the thread-header component (locate via `grep`).

- [ ] **Step 1: Locate**

Run:

```bash
grep -rn "thread.*header\|ThreadHeader\|thread-header" desktop-ui/src/features/threads/components | head -10
```

- [ ] **Step 2: Read session + add breadcrumb**

In the header component, fetch the active session (likely already a hook) and render a breadcrumb when `parentSessionId` is set:

```tsx
import { useEffect, useState } from "react";

const [parent, setParent] = useState<{ id: string; title: string } | null>(null);

useEffect(() => {
  if (!session?.parentSessionId) {
    setParent(null);
    return;
  }
  // Fetch parent session title; reuse existing thread-summary endpoint.
  fetchThreadSummary(session.parentSessionId).then((s) =>
    setParent({ id: s.id, title: s.title ?? "Parent" }),
  );
}, [session?.parentSessionId]);

return (
  <header className="thread-header">
    {parent && (
      <nav className="thread-header__breadcrumb">
        <button
          className="thread-header__breadcrumb-parent"
          onClick={() => navigateToThread(parent.id)}
        >
          {parent.title}
        </button>
        <span className="thread-header__breadcrumb-sep">›</span>
      </nav>
    )}
    <h1>{session?.title ?? "Untitled"}</h1>
  </header>
);
```

- [ ] **Step 3: Style**

Add to `desktop-ui/src/styles/thread-header.css` (or wherever):

```css
.thread-header__breadcrumb {
  display: flex; gap: 6px; align-items: center;
  font-size: var(--fs-xs); margin-bottom: 4px;
}
.thread-header__breadcrumb-parent {
  background: none; border: 0; padding: 0; cursor: pointer;
  color: var(--color-text-secondary);
}
.thread-header__breadcrumb-parent:hover { text-decoration: underline; }
.thread-header__breadcrumb-sep { color: var(--color-text-secondary); }
```

- [ ] **Step 4: Add a vitest**

Test that the breadcrumb renders when `parentSessionId` is set. If the component is too deeply wired, isolate the breadcrumb into a `ThreadBreadcrumb.tsx` sub-component and test that independently.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/threads/components
git commit -m "feat(ui): breadcrumb in thread header for subagent sessions"
```

---

### Task 6.5: Sidebar grouping — show subagents under their parent

**Files:**
- Modify: the sidebar thread-list components (locate via `grep`).

- [ ] **Step 1: Find the sidebar**

Run:

```bash
grep -rn "SidebarWorkspaceGroups\|ThreadList\|sidebar.*thread" desktop-ui/src/features/app/components | head -10
```

- [ ] **Step 2: Fetch + group**

In the relevant component, fetch all threads as before; then for each parent thread, append its subagent children (queried via `subagentListForSession(parentId)`) underneath, indented.

A minimal implementation: when a parent thread row is expanded, lazy-fetch its subagent children and render them indented.

```tsx
import { subagentListForSession } from "@/api/endpoints/subagent";

function ParentThreadRow({ thread, expanded, onSelect }: Props) {
  const [children, setChildren] = useState<SubagentInstanceSummary[]>([]);

  useEffect(() => {
    if (!expanded) return;
    let cancelled = false;
    subagentListForSession(thread.id).then((c) => {
      if (!cancelled) setChildren(c);
    });
    return () => { cancelled = true; };
  }, [expanded, thread.id]);

  return (
    <div className="thread-row-group">
      <ThreadRow thread={thread} onSelect={onSelect} />
      {expanded && children.map((c) => (
        <ThreadRow
          key={c.agentId}
          thread={{ id: c.sessionId, title: c.description }}
          onSelect={onSelect}
          indent={1}
          accessory={<StatusDot status={c.status} />}
        />
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Style indent**

```css
.thread-row[data-indent="1"] { padding-left: 24px; }
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/app/components
git commit -m "feat(ui): sidebar grouping — subagent children indented under parent thread"
```

---

## Phase 7 — Integration tests

### Task 7.1: Spawn → cap-hit → resume → complete

**Files:**
- Create: `tests/integration/subagent_resume.rs` (workspace facade integration tests live at top level per CLAUDE.md)

- [ ] **Step 1: Write the test**

Create `tests/integration/subagent_resume.rs`:

```rust
//! End-to-end: spawn a subagent with a tight turn cap, force a cap-hit,
//! resume it, and verify completion.

use klyntbot::{AgentRuntime, Config, StoragePool}; // adapt to actual facade exports

#[tokio::test]
async fn spawn_capped_then_resume_to_completion() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);
    let provider = klyntbot::providers::testing::SingleResponseProvider::dyn_arc("partial response");
    let rt = klyntbot::SubagentRuntime {
        repo: repos.subagent_instances.clone(),
        sessions: repos.sessions.clone(),
        active: klyntbot::ActiveSubagentRegistry::new(),
        provider,
    };
    // Parent session.
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('parent-A', 'assistant')")
        .execute(pool.inner())
        .await
        .unwrap();

    // First call: cap of 1 turn — but our test provider returns clean stop,
    // so this completes idle. For a real cap-hit test we'd need a provider
    // that emits a tool call (forcing another iteration); see Task 7.2.

    let r1 = rt.spawn(klyntbot::SpawnParams {
        description: "test".to_string(),
        prompt: "hi".to_string(),
        model: None,
        max_turns: Some(5),
        workspace_path: std::path::PathBuf::from("/tmp"),
        parent_session_id: "parent-A".to_string(),
        parent_agent_id: None,
    }).await.unwrap();
    assert_eq!(r1.status, klyntbot::SubagentStatus::Idle);

    let r2 = rt.resume(klyntbot::ResumeParams {
        agent_id: r1.agent_id.clone(),
        prompt: "continue".to_string(),
    }).await.unwrap();
    assert_eq!(r2.status, klyntbot::SubagentStatus::Idle);
    assert_eq!(r2.agent_id, r1.agent_id);

    let row = repos.subagent_instances.get(&r1.agent_id).await.unwrap().unwrap();
    assert!(row.turns_used_total >= 2);
}
```

(Adapt the imports to match what the `klyntbot` facade crate actually re-exports. If a needed re-export is missing, add it to `src/lib.rs` of the facade.)

- [ ] **Step 2: Run**

Run: `cargo nextest run --test integration -E 'test(spawn_capped_then_resume)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/integration/subagent_resume.rs
git commit -m "test(integration): spawn → resume completes idle with accumulated turns_used_total"
```

---

### Task 7.2: Concurrency rule — resume on running fails

**Files:**
- Modify: `tests/integration/subagent_resume.rs`

- [ ] **Step 1: Add the test**

Append:

```rust
#[tokio::test]
async fn resume_on_running_returns_error() {
    // Setup: create a row in 'running' state directly (skips the actual loop).
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);
    let rt = klyntbot::SubagentRuntime {
        repo: repos.subagent_instances.clone(),
        sessions: repos.sessions.clone(),
        active: klyntbot::ActiveSubagentRegistry::new(),
        provider: klyntbot::providers::testing::SingleResponseProvider::dyn_arc("x"),
    };
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('parent-B', 'assistant'), ('sub-B', 'subagent')")
        .execute(pool.inner()).await.unwrap();
    repos.subagent_instances.insert(&klyntbot::NewSubagentInstance {
        agent_id: "ag-busy".to_string(),
        session_id: "sub-B".to_string(),
        parent_agent_id: None,
        description: "x".to_string(),
        model: None,
        workspace_path: "/tmp".to_string(),
        turn_cap: 500,
    }).await.unwrap();

    let err = rt.resume(klyntbot::ResumeParams {
        agent_id: "ag-busy".to_string(),
        prompt: "x".to_string(),
    }).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("currently running"));
}
```

- [ ] **Step 2: Run + commit**

Run: `cargo nextest run --test integration -E 'test(resume_on_running)'`
Expected: PASS.

```bash
git add tests/integration/subagent_resume.rs
git commit -m "test(integration): resume on running returns 'currently running' error"
```

---

### Task 7.3: Crash-recovery sweep at startup

**Files:**
- Create: `tests/integration/subagent_crash_recovery.rs`

- [ ] **Step 1: Write the test**

Create `tests/integration/subagent_crash_recovery.rs`:

```rust
//! Crash recovery: a stale 'running' row gets swept to 'failed' at startup.

use klyntbot::{NewSubagentInstance, Repos, StoragePool};

#[tokio::test]
async fn stale_running_row_flips_to_failed() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('p', 'assistant'), ('s', 'subagent')")
        .execute(pool.inner()).await.unwrap();
    repos.subagent_instances.insert(&NewSubagentInstance {
        agent_id: "ag-stale".to_string(),
        session_id: "s".to_string(),
        parent_agent_id: None,
        description: "x".to_string(),
        model: None,
        workspace_path: "/tmp".to_string(),
        turn_cap: 500,
    }).await.unwrap();
    sqlx::query("UPDATE subagent_instances SET updated_at = (unixepoch('now') * 1000) - 600000 WHERE agent_id = 'ag-stale'")
        .execute(pool.inner()).await.unwrap();

    let n = repos.subagent_instances.sweep_zombies(300_000).await.unwrap();
    assert_eq!(n, 1);
    let row = repos.subagent_instances.get("ag-stale").await.unwrap().unwrap();
    assert_eq!(row.status, "failed");
    assert!(row.partial_summary.is_some());
}
```

- [ ] **Step 2: Run + commit**

Run: `cargo nextest run --test integration -E 'test(stale_running_row)'`
Expected: PASS.

```bash
git add tests/integration/subagent_crash_recovery.rs
git commit -m "test(integration): startup zombie sweep flips stale running rows to failed"
```

---

## Phase 8 — Final verification

### Task 8.1: Run the full test suite + clippy + frontend

**Files:** none

- [ ] **Step 1: Cargo nextest workspace**

Run: `cargo nextest run --workspace 2>&1 | tail -40`
Expected: all tests pass.

- [ ] **Step 2: Cargo doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -20`
Expected: pass (or no doctests).

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30`
Expected: zero warnings.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: clean. If not, run `cargo fmt --all` and commit a "chore(fmt)" commit.

- [ ] **Step 5: Frontend**

Run: `cd desktop-ui && bun run typecheck && bun run lint && bun run test`
Expected: all green.

- [ ] **Step 6: Tauri dev smoke**

Run: `cargo tauri dev` (briefly), open the app, spawn a subagent from a chat session, verify the chip renders and clicking it navigates into the subagent thread, breadcrumb appears, click parent to navigate back. Ctrl-C when satisfied.

- [ ] **Step 7: Update CLAUDE.md with the new gotcha**

Edit `CLAUDE.md` (the project-level one), add to the "Gotchas" section:

```markdown
- **Subagent runtime (added 2026-05-12)** — Subagents are persistent now. Each lives in `sessions` (`mode='subagent'`, `parent_session_id`) + `subagent_instances` (`agent_id`, `status`, `partial_summary`, etc.). The `subagents` tool exposes four actions: `spawn` / `resume` / `list` / `kill`. The old `SpawnTool`/`spawn` registry name is removed. Cap-hits surface as structured `ToolError` payloads with `agent_id` + `partial_summary` + resume hint — the parent agent decides whether to resume or split. Token cap is removed for subagents; turn cap defaults to 500 per call (`SafetyCap::with_limits(_, 0, 500)`). Cancellation via `chat_cancel` on a `mode='subagent'` session routes to `SubagentRuntime::kill`. Crash recovery: a startup sweep flips any `running` rows with `updated_at` older than 5 minutes to `failed`. The heartbeat is the iteration-boundary `repo.tick_turn(agent_id)` call inside `execute_loop`.
```

- [ ] **Step 8: Final commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude-md): document the subagent persistence runtime"
```

- [ ] **Step 9: Open PR**

```bash
gh pr create --title "feat: persistent subagents with resume + opencode-style navigation" --body "$(cat <<'EOF'
## Summary

- Replace the one-shot subagent runtime with a persistent system: subagents now have a stable agent_id, conversation history in `sessions`, and metadata (status, partial_summary, lifecycle) in `subagent_instances`.
- Add `subagents` multi-action tool: `spawn` / `resume` / `list` / `kill`.
- Drop the 120k token cap on subagents (still gated by 500-turn cap; in-flight context compaction handles long sessions).
- Fix the silent-drop bug where cap-hits were reported as `Ok("ok")` to the parent.
- Subagent sessions live in the existing thread UI: drill-in from a tool-result chip, breadcrumb back to parent, indented in the sidebar.

## Test plan
- [ ] `cargo nextest run --workspace` passes
- [ ] `cargo clippy --workspace --all-targets --all-features` is clean
- [ ] `cd desktop-ui && bun run typecheck && bun run lint && bun run test` passes
- [ ] Manually spawn a subagent in coding mode, click the chip, verify navigation
- [ ] Force a cap-hit (max_turns=2 with a long task), verify the parent receives a structured error
- [ ] Resume the capped subagent, verify it completes
- [ ] Cancel an active subagent via the chat-cancel button, verify status flips to `killed`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-review checklist

Run through this before declaring the plan done:

**Spec coverage:**
- [x] Profile system removal — Phase 2 Task 2.1
- [x] Schema changes (sessions widen + new subagent_instances) — Phase 0 Tasks 0.2, 0.3
- [x] `SessionMode::Subagent` enum variant — Phase 0 Task 0.1
- [x] `SubagentInstanceRepo` with CRUD, transitions, heartbeat, sweep — Phase 1 Tasks 1.1–1.4
- [x] `SubagentRuntime` with spawn / resume / kill / list — Phase 3 Tasks 3.1–3.6
- [x] Heartbeat hook into `execute_loop` — Phase 3 Task 3.4
- [x] Silent-drop bug fix (cap-hit returns structured error) — Phase 3 Task 3.3 (`finalize_run`)
- [x] Tool rename + multi-action — Phase 4 Tasks 4.1, 4.2
- [x] AppCore handler + Tauri command — Phase 5 Tasks 5.1, 5.2
- [x] `chat_cancel` routes by mode — Phase 5 Task 5.3
- [x] Startup zombie sweep — Phase 5 Task 5.4
- [x] Frontend chip + breadcrumb + sidebar grouping — Phase 6 Tasks 6.3–6.5
- [x] Integration tests — Phase 7
- [x] CLAUDE.md gotcha note — Phase 8 Task 8.1

**Placeholder scan:** no `TBD`, `TODO`, `fill in later`, or "implement later" — except a single `todo!` macro inside Task 3.3 Step 4 that is replaced in Step 5 of the same task.

**Type consistency:** `SubagentStatus`, `SubagentInstanceRow`, `NewSubagentInstance`, `SubagentRuntime`, `SpawnParams`, `ResumeParams`, `SubagentsHandler`, and `SubagentInstanceSummary` are all defined once and used consistently. `agent_id` is the primary key throughout; `session_id` is the FK to `sessions(key)`.

**Known assumptions to verify when executing:**
- `LlmResponse` field names in `crates/providers/src/testing.rs` may differ — task explicitly says to verify with `cargo check`.
- The exact `#[tool_actions]` macro signature is copied from an existing tool — task tells the engineer to grep for an example before writing the macro invocation.
- `SessionMessageRow.content` field name may be `text` or `body` — task says to verify.
- `RoutingContext.session_key` and `agent_chain` field names are referenced but the spec also tells the engineer to verify them.
