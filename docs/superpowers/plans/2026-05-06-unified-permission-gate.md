# Unified Permission Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Klynt's two partial permission systems (coding-mode 3-layer guard + latent `PermissionLevel`) with a single pre-execution `ApprovalGate` that fires for every tool call across every mode and channel.

**Architecture:** A new L3 crate `crates/approval/` owns `ApprovalClass` / `ApprovalScope` / `ApprovalChannel` / `ApprovalGate`. The `Tool` trait in `tools-core` gains `approval_class(&Value) -> ApprovalClass` and `approval_scope(&Value) -> ApprovalScope` with safe defaults. `ExecutionCore::run_cycle` calls `ApprovalGate::check` once per tool call **before** `PreToolUse` dispatch. Persistent decisions live in a fresh `approval_grants` table (drop-and-replace of existing `coding_approval_history` — pre-release, no migration). The legacy 3-layer guard becomes a `CodingApprovalPolicy` plug-in consulted only for coding-mode tools whose class depends on shell/path inspection.

**Tech Stack:** Rust 1.93 stable, tokio, async-trait, sqlx (SQLite via `StoragePool`), serde, tracing. Frontend: Tauri 2 + React (existing modal generalized).

**Spec reference:** `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md`

---

## File Structure

### Created
- `crates/approval/Cargo.toml` — new L3 crate manifest
- `crates/approval/src/lib.rs` — re-exports
- `crates/approval/src/class.rs` — `ApprovalClass`, `ApprovalScope`, `ApprovalDecision`, `ApprovalLifetime`
- `crates/approval/src/request.rs` — `ApprovalRequest`, `ApprovalContext`
- `crates/approval/src/channel.rs` — `ApprovalChannel` trait + `ApprovalCapabilities`
- `crates/approval/src/grants.rs` — `ApprovalGrantsRepo` (CRUD over `approval_grants` table)
- `crates/approval/src/gate.rs` — `ApprovalGate::check` decision flow
- `crates/approval/src/policy.rs` — `ClassifyHook` trait
- `crates/approval/src/coding_policy.rs` — `CodingApprovalPolicy` (port of layer1/layer3)
- `crates/approval/src/observability.rs` — activity-log emitter
- `crates/approval/tests/gate_flow.rs` — integration test for the gate
- `crates/approval/tests/coding_policy_parity.rs` — snapshot tests vs old guard
- `desktop-ui/src/features/approvals/ApprovalModal.tsx` — generalized modal (rename of coding modal)

### Modified
- `crates/tools-core/src/lib.rs` — add `Tool::approval_class`, `Tool::approval_scope`, `DEFAULT_APPROVAL` const
- `crates/tools-core/src/permissions.rs` — delete `PermissionLevel` (latent, unused) **OR** keep as separate concept (decision: delete, see Task 2)
- `crates/storage/migrations/001_initial.sql` — drop `coding_approval_history` block; add `approval_grants` table
- `crates/storage/src/lib.rs` and `crates/storage/src/repos/mod.rs` — wire `ApprovalGrantsRepo`
- `crates/agent/src/execution/core.rs:700-742` — call `ApprovalGate::check` before `tool.execute`
- `crates/agent/Cargo.toml` — add `approval` dep
- `crates/klynt-core/src/approval/mod.rs` — gut; re-export from new crate (transitional shim, then deleted)
- `crates/klynt-core/src/tools/{bash,edit,web_fetch,notebook_edit,apply_patch}.rs` — remove inline `evaluate(GuardCtx, ...)` calls; tools now declare `approval_class` only
- `crates/channels/src/telegram/mod.rs` — `TelegramApprovalChannel` impl
- `crates/desktop/src/commands/approvals.rs` — `DesktopApprovalChannel` (port modal IPC)
- `crates/mcp/src/server/handler.rs` — `McpApprovalChannel` (structured error)
- `desktop-ui/src/styles/index.css` — import new approvals CSS
- For each domain feature (`feature-tasks`, `feature-notes`, `feature-finance`, `feature-productivity`, `feature-learning`, `feature-language-learning`, `feature-coaching`, `feature-insights`): override `approval_class` per tool action

### Deleted (after migration)
- `crates/klynt-core/src/approval/{decision,guard,host_cache,layer1,layer3,matcher,round_trip}.rs` — moved/ported into `crates/approval/`
- `crates/tools-core/src/permissions.rs` — `PermissionLevel` retired (replaced by `ApprovalClass`)

---

## Phase 1 — Crate skeleton

### Task 1: Create `crates/approval/` skeleton

**Files:**
- Create: `crates/approval/Cargo.toml`
- Create: `crates/approval/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create the crate manifest**

`crates/approval/Cargo.toml`:

```toml
[package]
name = "approval"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sqlx = { workspace = true, features = ["sqlite", "runtime-tokio"] }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["sync", "time"] }
tokio-util = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true, features = ["v4", "serde"] }
jiff = { workspace = true }

common = { path = "../common" }
tools-core = { path = "../tools-core" }
storage = { path = "../storage" }
bus = { path = "../bus" }
activity-log = { path = "../activity-log" }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "test-util"] }
```

- [ ] **Step 2: Stub `lib.rs`**

```rust
//! Unified approval gate — pre-tool-execution permission system.
//!
//! See: docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md

pub mod channel;
pub mod class;
pub mod coding_policy;
pub mod gate;
pub mod grants;
pub mod observability;
pub mod policy;
pub mod request;

pub use channel::{ApprovalCapabilities, ApprovalChannel};
pub use class::{ApprovalClass, ApprovalDecision, ApprovalLifetime, ApprovalScope};
pub use gate::{ApprovalGate, GateOutcome};
pub use grants::{ApprovalGrantsRepo, GrantRow};
pub use policy::ClassifyHook;
pub use request::{ApprovalContext, ApprovalRequest};
```

- [ ] **Step 3: Add to workspace**

In root `Cargo.toml`, append `"crates/approval"` to `members`. Verify other workspace members are listed alphabetically; insert in the right position.

- [ ] **Step 4: Create empty stub files so workspace compiles**

For each module listed in `lib.rs`, create `crates/approval/src/<name>.rs` containing only `//! TODO: see plan task N`. (Replaces `pub mod` errors during incremental work.)

- [ ] **Step 5: Verify workspace builds**

Run: `cargo build -p approval`
Expected: Builds with warnings about empty modules; no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/approval Cargo.toml
git commit -m "feat(approval): create empty crate skeleton at L3"
```

---

### Task 2: Define `ApprovalClass`, `ApprovalScope`, `ApprovalDecision`, `ApprovalLifetime`

**Files:**
- Modify: `crates/approval/src/class.rs`
- Test: `crates/approval/src/class.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing tests**

Append to `crates/approval/src/class.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_serializes_to_lowercase_kebab() {
        assert_eq!(serde_json::to_string(&ApprovalClass::Safe).unwrap(), "\"safe\"");
        assert_eq!(serde_json::to_string(&ApprovalClass::Sensitive).unwrap(), "\"sensitive\"");
        assert_eq!(serde_json::to_string(&ApprovalClass::Destructive).unwrap(), "\"destructive\"");
        assert_eq!(serde_json::to_string(&ApprovalClass::Admin).unwrap(), "\"admin\"");
    }

    #[test]
    fn lifetime_once_is_default() {
        assert_eq!(ApprovalLifetime::default(), ApprovalLifetime::Once);
    }

    #[test]
    fn scope_tool_action_has_no_resource() {
        let s = ApprovalScope::ToolAction;
        assert!(matches!(s, ApprovalScope::ToolAction));
    }

    #[test]
    fn scope_resource_carries_key() {
        let s = ApprovalScope::ToolActionResource("path/to/file".into());
        if let ApprovalScope::ToolActionResource(k) = s {
            assert_eq!(k, "path/to/file");
        } else {
            panic!("wrong variant");
        }
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p approval class::tests`
Expected: FAIL — types don't exist.

- [ ] **Step 3: Implement**

Replace `crates/approval/src/class.rs` content:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalClass {
    Safe,
    Sensitive,
    Destructive,
    Admin,
}

impl ApprovalClass {
    pub fn requires_prompt_on_remote(&self) -> bool {
        matches!(self, Self::Destructive | Self::Admin)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApprovalScope {
    ToolAction,
    ToolActionResource(String),
}

impl ApprovalScope {
    pub fn resource_key(&self) -> Option<&str> {
        match self {
            Self::ToolAction => None,
            Self::ToolActionResource(k) => Some(k.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalLifetime {
    #[default]
    Once,
    Session,
    Forever,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ApprovalDecision {
    Once,
    Session,
    Forever,
    Decline { reason: String },
    Cancel,
}

#[cfg(test)]
mod tests { /* from Step 1 */ }
```

(Paste the test module from Step 1 verbatim.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p approval class::tests`
Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/approval/src/class.rs
git commit -m "feat(approval): add ApprovalClass, Scope, Decision, Lifetime"
```

---

### Task 3: Define `ApprovalRequest` + `ApprovalContext`

**Files:**
- Modify: `crates/approval/src/request.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_remote_predicate() {
        let local = ApprovalContext {
            mode: AgentMode::Coding,
            channel: ChannelKind::Desktop,
            session_id: "s1".into(),
            user_id: None,
        };
        assert!(!local.is_remote());
        let remote = ApprovalContext { channel: ChannelKind::Telegram, ..local };
        assert!(remote.is_remote());
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p approval request::tests`

- [ ] **Step 3: Implement**

```rust
use crate::class::{ApprovalClass, ApprovalScope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Assistant,
    Coding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    Desktop,
    Telegram,
    Discord,
    Slack,
    Email,
    Mcp,
}

impl ChannelKind {
    pub fn is_remote(&self) -> bool {
        !matches!(self, Self::Desktop)
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalContext {
    pub mode: AgentMode,
    pub channel: ChannelKind,
    pub session_id: String,
    pub user_id: Option<String>,
}

impl ApprovalContext {
    pub fn is_remote(&self) -> bool {
        self.channel.is_remote()
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub action: Option<String>,
    pub args: Value,
    pub class: ApprovalClass,
    pub scope: ApprovalScope,
    pub ctx: ApprovalContext,
}

#[cfg(test)]
mod tests { /* from Step 1 */ }
```

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p approval request::tests`

- [ ] **Step 5: Commit**

```bash
git add crates/approval/src/request.rs
git commit -m "feat(approval): add ApprovalRequest and ApprovalContext"
```

---

### Task 4: Define `ApprovalChannel` trait + `ApprovalCapabilities`

**Files:**
- Modify: `crates/approval/src/channel.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::ApprovalClass;
    use std::collections::HashSet;

    struct DummyChannel;

    #[async_trait::async_trait]
    impl ApprovalChannel for DummyChannel {
        async fn request(&self, _r: crate::ApprovalRequest) -> crate::ApprovalDecision {
            crate::ApprovalDecision::Once
        }
        fn capabilities(&self) -> ApprovalCapabilities {
            ApprovalCapabilities {
                supports_inline: true,
                supports_classes: HashSet::from([ApprovalClass::Destructive]),
            }
        }
    }

    #[tokio::test]
    async fn dummy_channel_returns_once() {
        let c = DummyChannel;
        assert!(c.capabilities().supports_inline);
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p approval channel::tests`

- [ ] **Step 3: Implement**

```rust
use crate::class::ApprovalClass;
use crate::request::ApprovalRequest;
use crate::ApprovalDecision;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ApprovalCapabilities {
    pub supports_inline: bool,
    pub supports_classes: HashSet<ApprovalClass>,
}

#[async_trait::async_trait]
pub trait ApprovalChannel: Send + Sync {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision;
    fn capabilities(&self) -> ApprovalCapabilities;
}

#[cfg(test)]
mod tests { /* from Step 1 */ }
```

- [ ] **Step 4: Run tests — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/approval/src/channel.rs
git commit -m "feat(approval): add ApprovalChannel trait"
```

---

## Phase 2 — Storage

### Task 5: Drop existing coding-grant SQL, add `approval_grants`

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:860-880` (drop the `coding_approval_history` block)
- Modify: `crates/storage/migrations/001_initial.sql` (append new schema)

- [ ] **Step 1: Read current block**

Run: `sed -n '855,910p' crates/storage/migrations/001_initial.sql`

Note line numbers of the `coding_approval_history` table + indexes. Confirm no other code references it after Task 12 ports the lookups.

- [ ] **Step 2: Replace the block**

Delete lines 860-906 (or whichever contain `coding_approval_history` + its indexes + comment). Append at the end of the file:

```sql
-- Unified approval grants (replaces coding_approval_history; pre-release, no migration).
CREATE TABLE IF NOT EXISTS approval_grants (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    class           TEXT NOT NULL CHECK (class IN ('safe','sensitive','destructive','admin')),
    tool_name       TEXT NOT NULL,
    action          TEXT,
    resource_key    TEXT,
    lifetime        TEXT NOT NULL CHECK (lifetime IN ('session','forever')),
    session_id      TEXT,
    granted_at      INTEGER NOT NULL,
    expires_at      INTEGER,
    UNIQUE (class, tool_name, action, resource_key, lifetime, session_id)
);

CREATE INDEX IF NOT EXISTS idx_approval_grants_lookup
    ON approval_grants(tool_name, action, class, resource_key, session_id);
```

- [ ] **Step 3: Verify migration applies**

Run: `cargo nextest run -p storage`
Expected: All storage tests PASS (the in-memory pool runs migrations on connect).

- [ ] **Step 4: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): replace coding_approval_history with approval_grants"
```

---

### Task 6: Implement `ApprovalGrantsRepo`

**Files:**
- Modify: `crates/approval/src/grants.rs`
- Modify: `crates/storage/src/repos/mod.rs` — re-export
- Test: `crates/approval/tests/grants_repo.rs`

- [ ] **Step 1: Write failing integration test**

`crates/approval/tests/grants_repo.rs`:

```rust
use approval::{ApprovalClass, ApprovalGrantsRepo, ApprovalLifetime, GrantRow};
use storage::StoragePool;

#[tokio::test]
async fn insert_and_find_session_grant() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool.clone());

    let row = GrantRow {
        class: ApprovalClass::Destructive,
        tool_name: "bash".into(),
        action: None,
        resource_key: Some("rm -rf /tmp/x".into()),
        lifetime: ApprovalLifetime::Session,
        session_id: Some("sess-1".into()),
        granted_at: 1_700_000_000,
        expires_at: None,
    };
    repo.insert(&row).await.unwrap();

    let found = repo.find(
        ApprovalClass::Destructive,
        "bash",
        None,
        Some("rm -rf /tmp/x"),
        Some("sess-1"),
    ).await.unwrap();
    assert!(found.is_some(), "session grant should be found");

    let other_session = repo.find(
        ApprovalClass::Destructive,
        "bash",
        None,
        Some("rm -rf /tmp/x"),
        Some("sess-2"),
    ).await.unwrap();
    assert!(other_session.is_none(), "should not match different session");
}

#[tokio::test]
async fn forever_grant_matches_any_session() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool);
    repo.insert(&GrantRow {
        class: ApprovalClass::Sensitive,
        tool_name: "notes".into(),
        action: Some("delete".into()),
        resource_key: None,
        lifetime: ApprovalLifetime::Forever,
        session_id: None,
        granted_at: 1,
        expires_at: None,
    }).await.unwrap();

    // Forever lookup uses session_id = None
    let found = repo.find_forever(ApprovalClass::Sensitive, "notes", Some("delete"), None)
        .await.unwrap();
    assert!(found.is_some());
}
```

- [ ] **Step 2: Run — expect FAIL (compile error)**

Run: `cargo test -p approval --test grants_repo`

- [ ] **Step 3: Implement repo**

`crates/approval/src/grants.rs`:

```rust
use crate::class::{ApprovalClass, ApprovalLifetime};
use common::Result;
use sqlx::Row;
use storage::StoragePool;

#[derive(Debug, Clone)]
pub struct GrantRow {
    pub class: ApprovalClass,
    pub tool_name: String,
    pub action: Option<String>,
    pub resource_key: Option<String>,
    pub lifetime: ApprovalLifetime,
    pub session_id: Option<String>,
    pub granted_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Clone)]
pub struct ApprovalGrantsRepo {
    pool: StoragePool,
}

impl ApprovalGrantsRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &GrantRow) -> Result<()> {
        let class = serde_json::to_string(&row.class)?.trim_matches('"').to_string();
        let lifetime = serde_json::to_string(&row.lifetime)?.trim_matches('"').to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO approval_grants
             (class, tool_name, action, resource_key, lifetime, session_id, granted_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(class)
        .bind(&row.tool_name)
        .bind(&row.action)
        .bind(&row.resource_key)
        .bind(lifetime)
        .bind(&row.session_id)
        .bind(row.granted_at)
        .bind(row.expires_at)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn find(
        &self,
        class: ApprovalClass,
        tool: &str,
        action: Option<&str>,
        resource: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Option<GrantRow>> {
        let class_s = serde_json::to_string(&class)?.trim_matches('"').to_string();
        let row = sqlx::query(
            "SELECT class, tool_name, action, resource_key, lifetime, session_id, granted_at, expires_at
             FROM approval_grants
             WHERE class = ? AND tool_name = ?
               AND (action IS ? OR action = ?)
               AND (resource_key IS ? OR resource_key = ?)
               AND ((lifetime = 'forever' AND session_id IS NULL)
                 OR (lifetime = 'session' AND session_id = ?))
             LIMIT 1",
        )
        .bind(class_s)
        .bind(tool)
        .bind(action)
        .bind(action)
        .bind(resource)
        .bind(resource)
        .bind(session_id)
        .fetch_optional(self.pool.inner())
        .await?;
        row.map(row_to_grant).transpose()
    }

    pub async fn find_forever(
        &self,
        class: ApprovalClass,
        tool: &str,
        action: Option<&str>,
        resource: Option<&str>,
    ) -> Result<Option<GrantRow>> {
        self.find(class, tool, action, resource, None).await
    }

    pub async fn purge_session(&self, session_id: &str) -> Result<u64> {
        let res = sqlx::query("DELETE FROM approval_grants WHERE lifetime = 'session' AND session_id = ?")
            .bind(session_id)
            .execute(self.pool.inner())
            .await?;
        Ok(res.rows_affected())
    }
}

fn row_to_grant(row: sqlx::sqlite::SqliteRow) -> Result<GrantRow> {
    let class_s: String = row.try_get("class")?;
    let lifetime_s: String = row.try_get("lifetime")?;
    Ok(GrantRow {
        class: serde_json::from_str(&format!("\"{class_s}\""))?,
        tool_name: row.try_get("tool_name")?,
        action: row.try_get("action")?,
        resource_key: row.try_get("resource_key")?,
        lifetime: serde_json::from_str(&format!("\"{lifetime_s}\""))?,
        session_id: row.try_get("session_id")?,
        granted_at: row.try_get("granted_at")?,
        expires_at: row.try_get("expires_at")?,
    })
}
```

If `StoragePool::inner()` is not the actual accessor, run `grep -n "impl StoragePool" crates/storage/src/lib.rs` and adjust to the real method (it may be `as_ref()`, `pool()`, or expose `&SqlitePool` directly).

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo nextest run -p approval`
Expected: 2 grant tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/approval
git commit -m "feat(approval): add ApprovalGrantsRepo with session/forever lookup"
```

---

## Phase 3 — Tool trait extension

### Task 7: Add `approval_class` / `approval_scope` to `Tool` trait with safe defaults

**Files:**
- Modify: `crates/tools-core/src/lib.rs`
- Test: inline in `crates/tools-core/src/lib.rs`

- [ ] **Step 1: Locate the `Tool` trait**

Run: `grep -n "pub trait Tool" crates/tools-core/src/lib.rs`

Note the line where the trait body ends.

- [ ] **Step 2: Add a failing test**

Append to `crates/tools-core/src/lib.rs`:

```rust
#[cfg(test)]
mod approval_defaults_tests {
    use super::*;
    use approval::{ApprovalClass, ApprovalScope};
    use serde_json::Value;

    struct StubTool;
    #[async_trait::async_trait]
    impl Tool for StubTool {
        // ... fill the existing required methods minimally; copy from another stub in the file
    }

    #[test]
    fn default_class_is_safe() {
        let t = StubTool;
        assert_eq!(t.approval_class(&Value::Null), ApprovalClass::Safe);
    }

    #[test]
    fn default_scope_is_tool_action() {
        let t = StubTool;
        assert!(matches!(t.approval_scope(&Value::Null), ApprovalScope::ToolAction));
    }
}
```

(If `tools-core` already has a stub Tool in tests, reuse it instead of writing a new one.)

- [ ] **Step 3: Run — expect FAIL**

Run: `cargo test -p tools-core approval_defaults_tests`
Expected: compile error — `approval_class` not on trait.

- [ ] **Step 4: Add a thin re-export of `ApprovalClass`/`ApprovalScope`**

Because `tools-core` is L1 and `approval` is L3, we can't depend on `approval` from `tools-core`. **Move `class.rs` types to `tools-core` instead** — they are tiny, stable enums.

Revise plan: in `crates/tools-core/src/approval_class.rs`, paste the contents of `crates/approval/src/class.rs` (the enum definitions only, without tests). Then in `crates/approval/src/class.rs`, replace the body with `pub use tools_core::approval_class::*;`.

- [ ] **Step 5: Wire trait defaults**

In `crates/tools-core/src/lib.rs`, inside `pub trait Tool { ... }`, add:

```rust
const DEFAULT_APPROVAL: crate::approval_class::ApprovalClass =
    crate::approval_class::ApprovalClass::Safe;

fn approval_class(&self, _args: &serde_json::Value) -> crate::approval_class::ApprovalClass {
    Self::DEFAULT_APPROVAL
}

fn approval_scope(&self, _args: &serde_json::Value) -> crate::approval_class::ApprovalScope {
    crate::approval_class::ApprovalScope::ToolAction
}
```

- [ ] **Step 6: Add `pub mod approval_class;` to `tools-core/src/lib.rs`**

- [ ] **Step 7: Run tests — PASS**

Run: `cargo test -p tools-core`
Expected: All `tools-core` tests pass, including the new ones.

- [ ] **Step 8: Workspace build**

Run: `cargo build --workspace`
Expected: builds; existing tool implementations get default `Safe` class for free.

- [ ] **Step 9: Commit**

```bash
git add crates/tools-core crates/approval
git commit -m "feat(tools-core): add approval_class and approval_scope to Tool trait"
```

---

### Task 8: Retire latent `PermissionLevel`

**Files:**
- Delete: `crates/tools-core/src/permissions.rs`
- Modify: `crates/tools-core/src/lib.rs` (remove `pub mod permissions`)
- Modify: callers found via grep

- [ ] **Step 1: Find every reference**

Run: `grep -rn "PermissionLevel\|ToolPermissions" crates/ --include="*.rs"`

Capture the list. Each reference will be deleted (declarative, never enforced).

- [ ] **Step 2: Delete the module**

```bash
git rm crates/tools-core/src/permissions.rs
```

- [ ] **Step 3: Remove the `pub mod permissions;` line and any re-exports in `crates/tools-core/src/lib.rs`**

- [ ] **Step 4: Delete callers**

For each file from Step 1 (e.g., `crates/tools/src/permissions.rs`, `crates/tools/src/registry.rs`, etc.), delete:
- imports of `PermissionLevel` / `ToolPermissions`
- struct fields holding them
- methods that set or check them

Run: `cargo build --workspace` after each file to surface the next breakage; iterate.

- [ ] **Step 5: Verify workspace builds**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: 0 errors, 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(tools-core): retire latent PermissionLevel in favor of ApprovalClass"
```

---

## Phase 4 — The gate

### Task 9: Implement `ApprovalGate::check` decision flow

**Files:**
- Modify: `crates/approval/src/gate.rs`
- Test: `crates/approval/tests/gate_flow.rs`

- [ ] **Step 1: Write failing integration test**

`crates/approval/tests/gate_flow.rs`:

```rust
use approval::{
    ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalContext, ApprovalDecision,
    ApprovalGate, ApprovalGrantsRepo, ApprovalRequest, ApprovalScope, AgentMode, ChannelKind,
    GateOutcome,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use storage::StoragePool;

#[derive(Default, Clone)]
struct StubChannel {
    decisions: Arc<Mutex<Vec<ApprovalDecision>>>,
    requested: Arc<Mutex<u32>>,
}

#[async_trait::async_trait]
impl ApprovalChannel for StubChannel {
    async fn request(&self, _r: ApprovalRequest) -> ApprovalDecision {
        *self.requested.lock().unwrap() += 1;
        self.decisions.lock().unwrap().pop().unwrap_or(ApprovalDecision::Once)
    }
    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([
                ApprovalClass::Sensitive,
                ApprovalClass::Destructive,
                ApprovalClass::Admin,
            ]),
        }
    }
}

fn ctx() -> ApprovalContext {
    ApprovalContext {
        mode: AgentMode::Coding,
        channel: ChannelKind::Desktop,
        session_id: "sess-1".into(),
        user_id: None,
    }
}

#[tokio::test]
async fn safe_class_auto_allows_without_prompt() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool);
    let chan = StubChannel::default();
    let gate = ApprovalGate::new(repo, Arc::new(chan.clone()));

    let req = ApprovalRequest {
        tool_name: "notes".into(),
        action: Some("read".into()),
        args: serde_json::json!({}),
        class: ApprovalClass::Safe,
        scope: ApprovalScope::ToolAction,
        ctx: ctx(),
    };
    let out = gate.check(req).await.unwrap();
    assert!(matches!(out, GateOutcome::Allow));
    assert_eq!(*chan.requested.lock().unwrap(), 0);
}

#[tokio::test]
async fn destructive_session_grant_persists_for_session() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool);
    let chan = StubChannel::default();
    chan.decisions.lock().unwrap().push(ApprovalDecision::Session);
    let gate = ApprovalGate::new(repo, Arc::new(chan.clone()));

    let make_req = || ApprovalRequest {
        tool_name: "notes".into(),
        action: Some("delete".into()),
        args: serde_json::json!({"id":"n1"}),
        class: ApprovalClass::Destructive,
        scope: ApprovalScope::ToolAction,
        ctx: ctx(),
    };

    let out1 = gate.check(make_req()).await.unwrap();
    assert!(matches!(out1, GateOutcome::Allow));
    assert_eq!(*chan.requested.lock().unwrap(), 1);

    // Second call: grant cached, channel must NOT be invoked.
    let out2 = gate.check(make_req()).await.unwrap();
    assert!(matches!(out2, GateOutcome::Allow));
    assert_eq!(*chan.requested.lock().unwrap(), 1);
}

#[tokio::test]
async fn decline_returns_deny() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool);
    let chan = StubChannel::default();
    chan.decisions.lock().unwrap().push(ApprovalDecision::Decline { reason: "no".into() });
    let gate = ApprovalGate::new(repo, Arc::new(chan));

    let req = ApprovalRequest {
        tool_name: "bash".into(),
        action: None,
        args: serde_json::json!({"cmd":"rm"}),
        class: ApprovalClass::Destructive,
        scope: ApprovalScope::ToolAction,
        ctx: ctx(),
    };
    let out = gate.check(req).await.unwrap();
    assert!(matches!(out, GateOutcome::Deny { .. }));
}

#[tokio::test]
async fn cancel_propagates() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool);
    let chan = StubChannel::default();
    chan.decisions.lock().unwrap().push(ApprovalDecision::Cancel);
    let gate = ApprovalGate::new(repo, Arc::new(chan));

    let req = ApprovalRequest {
        tool_name: "bash".into(), action: None,
        args: serde_json::json!({}),
        class: ApprovalClass::Destructive,
        scope: ApprovalScope::ToolAction,
        ctx: ctx(),
    };
    let out = gate.check(req).await.unwrap();
    assert!(matches!(out, GateOutcome::Cancel));
}

#[tokio::test]
async fn remote_channel_auto_allows_sensitive_when_capabilities_omit_it() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ApprovalGrantsRepo::new(pool);
    let chan = StubChannel::default(); // capabilities exclude Safe (default)
    let gate = ApprovalGate::new(repo, Arc::new(chan.clone()));

    let mut c = ctx();
    c.channel = ChannelKind::Telegram;
    let req = ApprovalRequest {
        tool_name: "notes".into(), action: Some("update".into()),
        args: serde_json::json!({}),
        class: ApprovalClass::Safe,
        scope: ApprovalScope::ToolAction,
        ctx: c,
    };
    let out = gate.check(req).await.unwrap();
    assert!(matches!(out, GateOutcome::Allow));
    assert_eq!(*chan.requested.lock().unwrap(), 0);
}
```

- [ ] **Step 2: Run — expect FAIL (gate not implemented)**

Run: `cargo test -p approval --test gate_flow`

- [ ] **Step 3: Implement gate**

`crates/approval/src/gate.rs`:

```rust
use crate::{
    channel::ApprovalChannel,
    class::{ApprovalClass, ApprovalDecision, ApprovalLifetime, ApprovalScope},
    grants::{ApprovalGrantsRepo, GrantRow},
    request::ApprovalRequest,
};
use common::Result;
use jiff::Timestamp;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum GateOutcome {
    Allow,
    Deny { reason: String },
    Cancel,
}

pub struct ApprovalGate {
    grants: ApprovalGrantsRepo,
    channel: Arc<dyn ApprovalChannel>,
}

impl ApprovalGate {
    pub fn new(grants: ApprovalGrantsRepo, channel: Arc<dyn ApprovalChannel>) -> Self {
        Self { grants, channel }
    }

    pub async fn check(&self, req: ApprovalRequest) -> Result<GateOutcome> {
        let resource = match &req.scope {
            ApprovalScope::ToolAction => None,
            ApprovalScope::ToolActionResource(k) => Some(k.clone()),
        };

        // Remote-channel auto-allow for non-prompted classes.
        let caps = self.channel.capabilities();
        if req.ctx.is_remote() && !caps.supports_classes.contains(&req.class) {
            tracing::debug!(
                tool = %req.tool_name, class = ?req.class, channel = ?req.ctx.channel,
                "approval: remote auto-allow (class not in channel capabilities)"
            );
            return Ok(GateOutcome::Allow);
        }

        // Existing-grant lookup.
        if let Some(_existing) = self.grants.find(
            req.class,
            &req.tool_name,
            req.action.as_deref(),
            resource.as_deref(),
            Some(&req.ctx.session_id),
        ).await? {
            return Ok(GateOutcome::Allow);
        }

        // Prompt the channel.
        let decision = self.channel.request(req.clone()).await;
        match decision {
            ApprovalDecision::Once => Ok(GateOutcome::Allow),
            ApprovalDecision::Session => {
                self.persist(&req, resource.as_deref(), ApprovalLifetime::Session).await?;
                Ok(GateOutcome::Allow)
            }
            ApprovalDecision::Forever => {
                self.persist(&req, resource.as_deref(), ApprovalLifetime::Forever).await?;
                Ok(GateOutcome::Allow)
            }
            ApprovalDecision::Decline { reason } => Ok(GateOutcome::Deny { reason }),
            ApprovalDecision::Cancel => Ok(GateOutcome::Cancel),
        }
    }

    async fn persist(
        &self,
        req: &ApprovalRequest,
        resource: Option<&str>,
        lifetime: ApprovalLifetime,
    ) -> Result<()> {
        let now = Timestamp::now().as_second();
        let session_id = match lifetime {
            ApprovalLifetime::Session => Some(req.ctx.session_id.clone()),
            ApprovalLifetime::Forever => None,
            ApprovalLifetime::Once => return Ok(()),
        };
        self.grants.insert(&GrantRow {
            class: req.class,
            tool_name: req.tool_name.clone(),
            action: req.action.clone(),
            resource_key: resource.map(str::to_string),
            lifetime,
            session_id,
            granted_at: now,
            expires_at: None,
        }).await
    }
}
```

Make `ApprovalRequest` derive `Clone` (Task 3 update).

- [ ] **Step 4: Run tests — PASS**

Run: `cargo nextest run -p approval --test gate_flow`
Expected: 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/approval
git commit -m "feat(approval): implement ApprovalGate decision flow"
```

---

### Task 10: Activity-log observability

**Files:**
- Modify: `crates/approval/src/observability.rs`
- Modify: `crates/approval/src/gate.rs` (call into emitter)

- [ ] **Step 1: Read activity-log API**

Run: `grep -n "pub fn\|pub struct\|pub enum" crates/activity-log/src/lib.rs | head -30`

Identify the entry point for emitting a row (likely `ActivityLog::emit` or similar).

- [ ] **Step 2: Implement emitter**

`crates/approval/src/observability.rs`:

```rust
use crate::{class::ApprovalClass, request::ApprovalRequest, ApprovalDecision};
use activity_log::ActivityLog;
use std::sync::Arc;

pub struct ApprovalAuditor {
    log: Arc<ActivityLog>,
}

impl ApprovalAuditor {
    pub fn new(log: Arc<ActivityLog>) -> Self { Self { log } }

    pub async fn record(
        &self,
        req: &ApprovalRequest,
        decision: &str,
        lifetime: Option<&str>,
    ) {
        let payload = serde_json::json!({
            "kind": "approval",
            "tool": req.tool_name,
            "action": req.action,
            "class": req.class,
            "decision": decision,
            "lifetime": lifetime,
            "channel": format!("{:?}", req.ctx.channel),
            "session_id": req.ctx.session_id,
        });
        if let Err(e) = self.log.append("approval", payload).await {
            tracing::warn!(error = %e, "approval auditor: append failed");
        }
    }
}
```

(Adapt `self.log.append` to the actual method.)

- [ ] **Step 3: Plumb into gate**

Add `auditor: Option<Arc<ApprovalAuditor>>` field on `ApprovalGate`. Call `auditor.record(&req, "allow", Some("session"))` etc. at each terminal branch.

- [ ] **Step 4: Add unit test with mock log; verify a row is emitted per decision**

(Use a `Mutex<Vec<Value>>` mock or feature-flag the auditor with a no-op default.)

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p approval`

- [ ] **Step 6: Commit**

```bash
git add crates/approval
git commit -m "feat(approval): emit activity-log row per gate decision"
```

---

## Phase 5 — Coding policy port

### Task 11: Define `ClassifyHook` trait

**Files:**
- Modify: `crates/approval/src/policy.rs`

- [ ] **Step 1: Implement**

```rust
use crate::class::{ApprovalClass, ApprovalScope};
use serde_json::Value;

pub trait ClassifyHook: Send + Sync {
    /// Inspect tool args and override the static class. Returning `None` means "use the tool's declared class".
    fn classify(&self, tool: &str, action: Option<&str>, args: &Value) -> Option<ApprovalClass>;

    /// Inspect args to derive a per-resource scope. Default returns `None` (tool's declared scope wins).
    fn scope(&self, _tool: &str, _action: Option<&str>, _args: &Value) -> Option<ApprovalScope> {
        None
    }
}
```

- [ ] **Step 2: Wire into gate**

Add `Vec<Arc<dyn ClassifyHook>>` to `ApprovalGate`. In `check`, after computing the static class+scope, run each hook and let the **last non-None** override.

Update `ApprovalGate::new` signature to accept hooks; provide a `with_classify_hooks` builder method.

- [ ] **Step 3: Add a unit test**

`gate_flow.rs`:

```rust
#[tokio::test]
async fn classify_hook_can_promote_class() {
    struct Promote;
    impl approval::ClassifyHook for Promote {
        fn classify(&self, _t: &str, _a: Option<&str>, _v: &serde_json::Value)
            -> Option<approval::ApprovalClass> { Some(approval::ApprovalClass::Destructive) }
    }
    // ...build gate with this hook; assert a Safe-declared call gets prompted
}
```

- [ ] **Step 4: Run — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/approval
git commit -m "feat(approval): add ClassifyHook trait for runtime class overrides"
```

---

### Task 12: Port `Layer1` and `Layer3` into `CodingApprovalPolicy`

**Files:**
- Create: `crates/approval/src/coding_policy.rs`
- Reference: `crates/klynt-core/src/approval/layer1.rs`, `crates/klynt-core/src/approval/layer3.rs`, `crates/klynt-core/src/approval/matcher.rs`

- [ ] **Step 1: Copy module bodies**

```bash
cp crates/klynt-core/src/approval/layer1.rs crates/approval/src/coding_policy/layer1.rs
cp crates/klynt-core/src/approval/layer3.rs crates/approval/src/coding_policy/layer3.rs
cp crates/klynt-core/src/approval/matcher.rs crates/approval/src/coding_policy/matcher.rs
cp crates/klynt-core/src/approval/host_cache.rs crates/approval/src/coding_policy/host_cache.rs
```

(Adjust import paths inside each file from `crate::approval::*` → `crate::coding_policy::*`.)

- [ ] **Step 2: Create `crates/approval/src/coding_policy/mod.rs`**

```rust
pub mod host_cache;
pub mod layer1;
pub mod layer3;
pub mod matcher;

use crate::class::{ApprovalClass, ApprovalScope};
use crate::policy::ClassifyHook;
use serde_json::Value;
use std::sync::Arc;

pub struct CodingApprovalPolicy {
    layer1: Arc<layer1::Layer1>,
    // layer3 details...
}

impl CodingApprovalPolicy {
    pub fn new(layer1: Arc<layer1::Layer1>) -> Self { Self { layer1 } }
}

impl ClassifyHook for CodingApprovalPolicy {
    fn classify(&self, tool: &str, _action: Option<&str>, args: &Value) -> Option<ApprovalClass> {
        let payload = match tool {
            "bash" => args.get("command").and_then(|v| v.as_str())?.to_string(),
            "edit" | "write" | "apply_patch" | "notebook_edit" => {
                args.get("file_path").and_then(|v| v.as_str())?.to_string()
            }
            "web_fetch" => args.get("url").and_then(|v| v.as_str())?.to_string(),
            _ => return None,
        };
        match self.layer1.evaluate(tool, &payload) {
            // Map old ApprovalDecision::Auto { allowed: true } → Safe
            // Map ApprovalDecision::Ask → Destructive (was the prompt branch)
            // Map ApprovalDecision::Auto { allowed: false } → still Destructive (denial happens at channel)
            d if matches!(d, crate::coding_policy::layer1::Auto { allowed: true, .. }) => Some(ApprovalClass::Safe),
            _ => Some(ApprovalClass::Destructive),
        }
    }

    fn scope(&self, tool: &str, _action: Option<&str>, args: &Value) -> Option<ApprovalScope> {
        let resource = match tool {
            "bash" => args.get("command")?.as_str()?.to_string(),
            "edit" | "write" | "apply_patch" => args.get("file_path")?.as_str()?.to_string(),
            _ => return None,
        };
        Some(ApprovalScope::ToolActionResource(resource))
    }
}
```

(The mapping above is illustrative — read the real `ApprovalDecision` variants from `decision.rs` and produce the equivalent `ApprovalClass`.)

- [ ] **Step 3: Verify it compiles in isolation**

Run: `cargo build -p approval`
Expected: builds.

- [ ] **Step 4: Commit**

```bash
git add crates/approval
git commit -m "feat(approval): port coding 3-layer guard as CodingApprovalPolicy"
```

---

### Task 13: Parity snapshot tests vs old guard

**Files:**
- Create: `crates/approval/tests/coding_policy_parity.rs`
- Create: `crates/approval/tests/fixtures/coding_calls.json`

- [ ] **Step 1: Build a fixture corpus**

Add ~30 historically-known shell/edit calls to `fixtures/coding_calls.json`:

```json
[
  {"tool":"bash","args":{"command":"ls -la"},"expected":"safe"},
  {"tool":"bash","args":{"command":"rm -rf /tmp/x"},"expected":"destructive"},
  {"tool":"bash","args":{"command":"git status"},"expected":"safe"},
  {"tool":"edit","args":{"file_path":"/etc/hosts"},"expected":"destructive"},
  {"tool":"edit","args":{"file_path":"/Users/x/proj/src/main.rs"},"expected":"destructive"},
  {"tool":"web_fetch","args":{"url":"https://api.github.com"},"expected":"destructive"}
  // ... 24 more
]
```

(Source the corpus from real session logs if available; otherwise hand-pick across categories: read-only shell, mutating shell, edits inside repo, edits to system paths, web fetch by host.)

- [ ] **Step 2: Write the test**

```rust
#[test]
fn coding_policy_matches_legacy_classifier_on_corpus() {
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(
        include_str!("fixtures/coding_calls.json")
    ).unwrap();

    let layer1 = std::sync::Arc::new(approval::coding_policy::layer1::Layer1::default());
    let policy = approval::coding_policy::CodingApprovalPolicy::new(layer1);

    for f in fixtures {
        let tool = f["tool"].as_str().unwrap();
        let args = &f["args"];
        let expected: approval::ApprovalClass =
            serde_json::from_value(f["expected"].clone()).unwrap();
        let got = approval::ClassifyHook::classify(&policy, tool, None, args)
            .unwrap_or(approval::ApprovalClass::Safe);
        assert_eq!(got, expected, "tool={tool}, args={args}");
    }
}
```

- [ ] **Step 3: Run — confirm parity**

Run: `cargo nextest run -p approval --test coding_policy_parity`

If a fixture mismatches, either fix the mapping in `coding_policy/mod.rs` or update the fixture (with a code comment justifying the change).

- [ ] **Step 4: Commit**

```bash
git add crates/approval/tests
git commit -m "test(approval): parity snapshots for CodingApprovalPolicy vs legacy guard"
```

---

## Phase 6 — Wire into execution

### Task 14: Inject `ApprovalGate` into `ExecutionCore`

**Files:**
- Modify: `crates/agent/src/execution/core.rs:528-742`
- Modify: `crates/agent/Cargo.toml` (add `approval` dep)

- [ ] **Step 1: Add the dep**

In `crates/agent/Cargo.toml`, under `[dependencies]`:

```toml
approval = { path = "../approval" }
```

- [ ] **Step 2: Add the field to `ExecutionCore`**

Locate the `ExecutionCore` struct (find with `grep -n "pub struct ExecutionCore" crates/agent/src/execution/core.rs`). Add:

```rust
pub approval_gate: Arc<approval::ApprovalGate>,
```

Update every constructor / builder call site to take and pass it.

- [ ] **Step 3: Insert the gate call**

At `crates/agent/src/execution/core.rs:734` (the `tool.execute(args, &ctx)` line), wrap with the gate. Find the surrounding closure starting around line 711:

```rust
let exec_result = tokio::time::timeout(timeout_dur, async {
    let tool = {
        let reg = registry.read().await;
        reg.prepare(&name, &args, &ctx)?
    };

    // === NEW: approval gate ===
    let req = approval::ApprovalRequest {
        tool_name: name.clone(),
        action: extract_action(&args),
        args: args.clone(),
        class: tool.approval_class(&args),
        scope: tool.approval_scope(&args),
        ctx: approval::ApprovalContext {
            mode: ctx.mode_into(),                   // see helper below
            channel: ctx.channel_into(),
            session_id: ctx.session_id.clone(),
            user_id: ctx.user_id.clone(),
        },
    };
    match self.approval_gate.check(req).await? {
        approval::GateOutcome::Allow => {}
        approval::GateOutcome::Deny { reason } => {
            return Err(common::KlyntbotError::permission_denied(reason));
        }
        approval::GateOutcome::Cancel => {
            return Err(common::KlyntbotError::cancelled("user cancelled approval"));
        }
    }
    // === END NEW ===

    if let Some(ref chain) = interceptor_chain {
        chain.check(&name, &args, None).await?;
    }
    tool.execute(args, &ctx).await
})
.await;
```

`extract_action` reads `args["action"].as_str()` if present (multi-action tools); returns `None` otherwise.

`ctx.mode_into()` / `ctx.channel_into()` are tiny conversion helpers from the existing `RoutingContext` types to the new `AgentMode` / `ChannelKind` enums. Add them next to `RoutingContext`.

- [ ] **Step 4: Add error variants**

In `crates/common/src/error.rs`, ensure `KlyntbotError::permission_denied(String)` and `cancelled(impl Into<String>)` exist. If not, add them.

- [ ] **Step 5: Build**

Run: `cargo build -p agent`
Expected: compiles after constructor sites are updated.

- [ ] **Step 6: Add an integration test**

`crates/agent/tests/approval_gate_integration.rs`:

```rust
// Build an ExecutionCore with a stub ApprovalGate, register a fake tool that
// declares `Destructive`, run one cycle, assert the channel was prompted and
// the tool did NOT execute when the channel returns Decline.
```

(Pattern after existing integration tests in `tests/integration/`.)

- [ ] **Step 7: Run — PASS**

Run: `cargo nextest run -p agent approval_gate_integration`

- [ ] **Step 8: Commit**

```bash
git add crates/agent
git commit -m "feat(agent): invoke ApprovalGate in ExecutionCore::run_cycle"
```

---

### Task 15: Remove inline `evaluate(GuardCtx, ...)` from coding tools

**Files:**
- Modify: `crates/klynt-core/src/tools/bash.rs`
- Modify: `crates/klynt-core/src/tools/edit.rs`
- Modify: `crates/klynt-core/src/tools/web_fetch.rs`
- Modify: `crates/klynt-core/src/tools/notebook_edit.rs`
- Modify: `crates/klynt-core/src/tools/apply_patch.rs`

- [ ] **Step 1: For each file, do the following**

Pattern in each file (e.g. `bash.rs`):

```rust
// BEFORE
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
// ... inside execute():
let decision = evaluate(GuardCtx { ... }, "bash", &command).await;
match decision { ... }

// AFTER
// (delete the import; delete the `evaluate` call; the gate runs above us in ExecutionCore)
```

Also implement `Tool::approval_class` for each:

```rust
fn approval_class(&self, _args: &Value) -> ApprovalClass {
    ApprovalClass::Destructive  // bash / edit / apply_patch / notebook_edit
}
fn approval_scope(&self, args: &Value) -> ApprovalScope {
    if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
        ApprovalScope::ToolActionResource(cmd.to_string())
    } else {
        ApprovalScope::ToolAction
    }
}
```

`web_fetch` declares `Sensitive` (network read) and `ToolActionResource(url)`.

- [ ] **Step 2: Build**

Run: `cargo build -p klynt-core`

- [ ] **Step 3: Run tool tests**

Run: `cargo nextest run -p klynt-core`
Expected: PASS. (Existing approval tests in `crates/klynt-core/tests/` may need migration to `crates/approval/tests/` — see Task 18.)

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-core
git commit -m "refactor(klynt-core): remove inline approval calls from coding tools"
```

---

## Phase 7 — Channel adapters

### Task 16: Desktop `ApprovalChannel`

**Files:**
- Create: `crates/desktop/src/approval/mod.rs`
- Modify: `crates/desktop/src/lib.rs` (`pub mod approval;`)
- Modify: `desktop-ui/src/features/approvals/ApprovalModal.tsx` (rename of the coding modal)

- [ ] **Step 1: Identify the existing coding modal**

Run: `grep -rln "approval_respond\|ApprovalCard" desktop-ui/src crates/desktop/src`

- [ ] **Step 2: Generalize the modal**

Rename `desktop-ui/src/features/coding/ApprovalCard.tsx` → `desktop-ui/src/features/approvals/ApprovalModal.tsx`. Replace `Privacy/Layer1/Layer2/Layer3` strings with `Safe/Sensitive/Destructive/Admin`. Buttons: `[Once] [Session] [Always] [Decline] [Cancel]`. Props: `{ tool, action, class, args, onDecide(decision) }`.

- [ ] **Step 3: Implement the channel**

`crates/desktop/src/approval/mod.rs`:

```rust
use approval::{ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest};
use std::collections::HashSet;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

pub struct DesktopApprovalChannel {
    app: AppHandle,
    pending: Arc<Mutex<std::collections::HashMap<String, oneshot::Sender<ApprovalDecision>>>>,
}

impl DesktopApprovalChannel {
    pub fn new(app: AppHandle) -> Self {
        Self { app, pending: Arc::new(Mutex::new(Default::default())) }
    }

    pub async fn respond(&self, request_id: &str, decision: ApprovalDecision) {
        if let Some(tx) = self.pending.lock().await.remove(request_id) {
            let _ = tx.send(decision);
        }
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for DesktopApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);

        let _ = self.app.emit("approval:request", serde_json::json!({
            "id": id,
            "tool": req.tool_name,
            "action": req.action,
            "class": req.class,
            "args": req.args,
        }));

        match tokio::time::timeout(std::time::Duration::from_secs(600), rx).await {
            Ok(Ok(d)) => d,
            _ => ApprovalDecision::Cancel,
        }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([
                ApprovalClass::Safe,
                ApprovalClass::Sensitive,
                ApprovalClass::Destructive,
                ApprovalClass::Admin,
            ]),
        }
    }
}
```

- [ ] **Step 4: Add `approval_respond` Tauri command**

In `crates/desktop/src/commands/approvals.rs`, add a `#[klynt_command]` that calls `DesktopApprovalChannel::respond`. Register in `klynt_collect_commands![...]` (per CLAUDE.md gotcha).

- [ ] **Step 5: Frontend listens for `approval:request`**

In `desktop-ui/src/features/approvals/`, add a hook:

```tsx
useEffect(() => {
  const un = listen<ApprovalReq>('approval:request', (e) => setPending(e.payload));
  return () => { un.then(f => f()); };
}, []);
```

Render `<ApprovalModal />` when `pending` is set; on click, `invoke('approval_respond', { id, decision })`.

- [ ] **Step 6: Run dev app, manually trigger a Destructive call, verify modal appears**

```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

Trigger via a coding-mode `bash rm -rf /tmp/test`. Expected: modal appears with `[Once] [Session] [Always] [Decline]`.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop desktop-ui
git commit -m "feat(desktop): DesktopApprovalChannel with generalized modal UI"
```

---

### Task 17: Telegram `ApprovalChannel` with inline buttons

**Files:**
- Modify: `crates/channels/src/telegram/mod.rs`
- Create: `crates/channels/src/telegram/approval.rs`

- [ ] **Step 1: Implement**

```rust
use approval::{ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest};
use std::collections::HashSet;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

pub struct TelegramApprovalChannel {
    bot: Bot,
    chat_id: ChatId,
    pending: Arc<Mutex<std::collections::HashMap<String, oneshot::Sender<ApprovalDecision>>>>,
}

#[async_trait::async_trait]
impl ApprovalChannel for TelegramApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        let id = Uuid::new_v4().to_string();
        let kb = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("Once",     format!("appr:{id}:once")),
            InlineKeyboardButton::callback("Session",  format!("appr:{id}:session")),
            InlineKeyboardButton::callback("Always",   format!("appr:{id}:forever")),
            InlineKeyboardButton::callback("Decline",  format!("appr:{id}:decline")),
        ]]);
        let body = format!(
            "🔐 *{}* approval needed\nTool: `{}`\nAction: `{}`",
            format!("{:?}", req.class),
            req.tool_name,
            req.action.as_deref().unwrap_or("-"),
        );
        let _ = self.bot.send_message(self.chat_id, body).reply_markup(kb).await;

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        match tokio::time::timeout(std::time::Duration::from_secs(600), rx).await {
            Ok(Ok(d)) => d,
            _ => ApprovalDecision::Cancel,
        }
    }
    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([ApprovalClass::Destructive, ApprovalClass::Admin]),
        }
    }
}
```

- [ ] **Step 2: Wire callback handler**

In the existing telegram callback router, parse `appr:<id>:<verb>` and call `pending.lock().remove(id).send(decision)`.

- [ ] **Step 3: Manual smoke test**

Run the bot, trigger `notes.delete` from Telegram, confirm buttons appear.

- [ ] **Step 4: Commit**

```bash
git add crates/channels
git commit -m "feat(channels): TelegramApprovalChannel with inline buttons"
```

---

### Task 18: MCP `ApprovalChannel` with structured error

**Files:**
- Modify: `crates/mcp/src/server/handler.rs`
- Create: `crates/mcp/src/server/approval.rs`

- [ ] **Step 1: Implement**

```rust
use approval::{ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest};
use std::collections::HashSet;

pub struct McpApprovalChannel;

#[async_trait::async_trait]
impl ApprovalChannel for McpApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        // No interactive surface inside an MCP transport — return Decline with a structured reason
        // so the JSON-RPC error wrapper can carry it back to the caller.
        ApprovalDecision::Decline {
            reason: serde_json::json!({
                "code": "approval-required",
                "tool": req.tool_name,
                "action": req.action,
                "class": req.class,
                "message": "User approval required; not interactive over MCP.",
            }).to_string(),
        }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: false,
            // Empty set => Safe/Sensitive auto-allow per gate logic; Destructive/Admin route here & decline.
            supports_classes: HashSet::from([ApprovalClass::Destructive, ApprovalClass::Admin]),
        }
    }
}
```

- [ ] **Step 2: Wire into MCP server handler**

In `handler.rs`, when the gate returns `Deny { reason }` and `reason` parses as JSON containing `"code":"approval-required"`, return a JSON-RPC error response with that JSON as the `data` field. Otherwise treat as a normal denial.

- [ ] **Step 3: Wire-format test**

```rust
#[tokio::test]
async fn destructive_call_over_mcp_returns_approval_required_error() {
    // Spin up the MCP server in-process, call a Destructive tool, assert
    // response.error.data.code == "approval-required".
}
```

- [ ] **Step 4: Run — PASS**

Run: `cargo nextest run -p mcp`

- [ ] **Step 5: Commit**

```bash
git add crates/mcp
git commit -m "feat(mcp): McpApprovalChannel returns structured approval-required error"
```

---

### Task 19: Fallback channel (block + ask user to approve on desktop)

**Files:**
- Modify: `crates/approval/src/channel.rs`

- [ ] **Step 1: Add `BlockingFallbackChannel`**

```rust
pub struct BlockingFallbackChannel {
    message: String,
}

#[async_trait::async_trait]
impl ApprovalChannel for BlockingFallbackChannel {
    async fn request(&self, _req: ApprovalRequest) -> ApprovalDecision {
        ApprovalDecision::Decline { reason: self.message.clone() }
    }
    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: false,
            supports_classes: std::collections::HashSet::from([
                ApprovalClass::Destructive, ApprovalClass::Admin,
            ]),
        }
    }
}
```

Use in Discord/Slack/Email channel wiring until they get real impls. Default message: `"Action requires approval. Open Klynt on desktop to confirm."`.

- [ ] **Step 2: Commit**

```bash
git add crates/approval
git commit -m "feat(approval): blocking fallback channel for unimplemented surfaces"
```

---

## Phase 8 — Domain tool annotations

### Task 20: Annotate domain feature tools with `approval_class`

**Files:** all `crates/feature-*/src/**.rs` containing `#[derive(Tool)]`.

For each feature crate, identify the public tool struct(s) and override `approval_class`. **Do this per-action** for multi-action tools.

| Tool / Action | Class |
|---|---|
| `notes.read|list|search` | Safe |
| `notes.create|update` | Sensitive |
| `notes.delete|delete_all` | Destructive |
| `tasks.read|list` | Safe |
| `tasks.create|update|complete` | Sensitive |
| `tasks.delete|bulk_delete` | Destructive |
| `finance.read|list|forecast` | Safe |
| `finance.transaction.create|update` | Sensitive |
| `finance.transaction.delete|account.delete` | Destructive |
| `productivity.*read*` | Safe |
| `productivity.*update|create*` | Sensitive |
| `learning.*read*` | Safe |
| `learning.deck.delete` | Destructive |
| `language-learning.*read*` | Safe |
| `language-learning.*record*` | Sensitive |
| `memory.read` | Safe |
| `memory.write|forget` | Sensitive / Destructive (per action) |
| `okr.*read*` | Safe |
| `okr.delete` | Destructive |
| `agent` (delegate) | Sensitive |
| `cron.*read*` | Safe |
| `cron.create|update|delete` | Destructive (creates persistent automation) |
| `mirror.*` | Safe (read-only) |
| `spawn` | Admin |

- [ ] **Step 1: For each feature crate, edit each tool**

Example for `feature-notes/src/lib.rs`:

```rust
fn approval_class(&self, args: &Value) -> ApprovalClass {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
    match action {
        "read" | "list" | "search" => ApprovalClass::Safe,
        "create" | "update" => ApprovalClass::Sensitive,
        "delete" | "delete_all" => ApprovalClass::Destructive,
        _ => ApprovalClass::Sensitive,
    }
}
```

- [ ] **Step 2: For each crate, run its tests**

Run: `cargo nextest run -p feature-notes` (then each in turn).

- [ ] **Step 3: Workspace clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`

- [ ] **Step 4: Commit per feature crate (or one combined commit)**

```bash
git add crates/feature-notes
git commit -m "feat(feature-notes): declare approval_class per action"
# repeat for each
```

---

## Phase 9 — Mode & remote auto-allow logic

### Task 21: Wire `CodingApprovalPolicy` only for coding mode

**Files:**
- Modify: `crates/app-core/src/init/agent.rs` (or wherever `ExecutionCore` is built)

- [ ] **Step 1: Build the gate per-mode**

```rust
let mut hooks: Vec<Arc<dyn ClassifyHook>> = Vec::new();
if is_coding_mode {
    hooks.push(Arc::new(CodingApprovalPolicy::new(layer1.clone())));
}
let gate = Arc::new(ApprovalGate::new(grants_repo, channel).with_classify_hooks(hooks));
```

- [ ] **Step 2: Verify assistant-mode session does NOT consult coding policy**

Add an integration test that asserts a `bash` tool registered in assistant mode falls back to the static class (not the layer1 inspection).

- [ ] **Step 3: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): inject CodingApprovalPolicy only when mode is coding"
```

---

### Task 22: Session purge on session end

**Files:**
- Modify: `crates/session/src/lifecycle.rs` (or wherever sessions terminate)

- [ ] **Step 1: Locate session-end hook**

Run: `grep -rn "fn end_session\|session_close\|on_session_end" crates/session crates/app-core`

- [ ] **Step 2: Call `grants.purge_session(session_id)` in the end hook**

- [ ] **Step 3: Test**

Add a test:

```rust
#[tokio::test]
async fn session_grant_evicted_when_session_ends() {
    // Insert a Session-lifetime grant; call session-end hook; assert find() returns None.
}
```

- [ ] **Step 4: Run — PASS, commit**

```bash
git add -A
git commit -m "feat(session): purge approval_grants on session end"
```

---

## Phase 10 — Cleanup & polish

### Task 23: Delete legacy `klynt-core::approval` module

**Files:**
- Delete: `crates/klynt-core/src/approval/{decision,guard,host_cache,layer1,layer3,matcher,round_trip}.rs`
- Modify: `crates/klynt-core/src/approval/mod.rs` → delete the file
- Modify: `crates/klynt-core/src/lib.rs` → remove `pub mod approval;`

- [ ] **Step 1: Confirm no callers**

Run: `grep -rn "klynt_core::approval\|klynt-core/.*approval" crates/ --include="*.rs"`

If any remain, port them (they should now use `approval::*`).

- [ ] **Step 2: Delete**

```bash
git rm -r crates/klynt-core/src/approval
```

- [ ] **Step 3: Update `lib.rs`**

Remove `pub mod approval;` from `crates/klynt-core/src/lib.rs`.

- [ ] **Step 4: Workspace build + clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(klynt-core): delete legacy approval module (moved to crates/approval)"
```

---

### Task 24: Migrate legacy approval tests

**Files:**
- Move: `crates/klynt-core/tests/approval_guard.rs` → `crates/approval/tests/legacy_guard.rs`
- Move: `crates/klynt-core/tests/channel_aware_approval.rs` → `crates/approval/tests/channel_fallback.rs`
- Move: `crates/klynt-core/tests/k13_privacy_under_yolo.rs` → `crates/approval/tests/privacy_under_yolo.rs`

- [ ] **Step 1: For each test file, port imports**

Replace `klynt_core::approval::*` → `approval::*`. Adjust test bodies to the new types (`ApprovalClass` instead of `ApprovalLayer`, etc.).

- [ ] **Step 2: Run each**

Run: `cargo nextest run -p approval`

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test(approval): port legacy approval tests to new crate"
```

---

### Task 25: Run KCA validation gates

**Files:** none (validation only).

- [ ] **Step 1: Workspace lint**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 2: Workspace tests**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: 0 failures.

- [ ] **Step 3: KCA gates**

Run: `./scripts/run_kca_validation.sh`
Expected: all gates pass.

- [ ] **Step 4: Commit (if any auto-fmt)**

```bash
git add -A
git commit -m "chore: fmt + clippy after permission gate landing" --allow-empty
```

---

### Task 26: Update docs

**Files:**
- Modify: `CLAUDE.md` — add a "Approval gate" subsection under "Architecture"
- Modify: `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md` — flip `Status: Draft` → `Status: Implemented`

- [ ] **Step 1: Append to `CLAUDE.md`**

```markdown
### Approval gate

Every tool call passes through `approval::ApprovalGate::check` (`crates/approval/`) before
`PreToolUse` hooks fire. Tools declare `approval_class` (Safe/Sensitive/Destructive/Admin)
on the `Tool` trait; coding-mode shell/edit/web_fetch get runtime classification via
`CodingApprovalPolicy`. Persistent grants live in the `approval_grants` table; remote
channels (Telegram, MCP, etc.) implement `ApprovalChannel`. Desktop modal at
`desktop-ui/src/features/approvals/ApprovalModal.tsx`.
```

- [ ] **Step 2: Update spec status line**

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md docs
git commit -m "docs: document unified approval gate"
```

---

## Self-Review

**Spec coverage:**
- §4.1 layering → Task 14 (gate call site in `ExecutionCore::run_cycle`)
- §4.2 crate placement → Task 1 (`crates/approval/` at L3)
- §4.3 coding-mode integration → Tasks 11, 12, 21 (`ClassifyHook` + `CodingApprovalPolicy` + per-mode wiring)
- §5.1 tool API → Task 7 (`approval_class`/`approval_scope` on `Tool`)
- §5.2 grants table → Task 5
- §5.3 gate decision flow → Task 9
- §6.1 channel trait → Task 4
- §6.2 per-channel strategy → Tasks 16 (desktop), 17 (telegram), 18 (mcp), 19 (fallback)
- §6.3 remote auto-allow Sensitive → Task 9 (test `remote_channel_auto_allows_sensitive_when_capabilities_omit_it`)
- §7 mode integration → Task 21
- §8 migration plan → Tasks 1, 5, 7, 14, 16-19, 20
- §9 testing → Tasks 9, 13, 14, 18, 21, 22, 24
- §10 open questions → parked (no tasks needed)
- §11 observability → Task 10

**Placeholder scan:** No "TBD", "implement later", or vague "add error handling" steps. Each code step has a complete code block.

**Type consistency:** `ApprovalClass`, `ApprovalScope`, `ApprovalDecision`, `ApprovalLifetime`, `ApprovalRequest`, `ApprovalContext`, `ApprovalChannel`, `ApprovalGate`, `GateOutcome`, `ApprovalGrantsRepo`, `GrantRow`, `ClassifyHook`, `CodingApprovalPolicy` — names are consistent across all tasks. `tools-core` owns the enum types (Task 7 Step 4 revision); `approval` re-exports them.

**Risk callouts engineers should know:**
1. Task 7 Step 4 reverses the spec's "types live in `approval`" placement — `tools-core` (L1) cannot depend on `approval` (L3), so the enums *must* live in `tools-core` and be re-exported. Do not skip this.
2. `StoragePool::inner()` may not be the actual accessor — check before pasting Task 6 code.
3. `KlyntbotError::permission_denied` / `cancelled` may not exist — Task 14 Step 4 says to add them.
4. The `desktop-macros::klynt_collect_commands![...]` array must be updated when adding `approval_respond` (Task 16 Step 4) per CLAUDE.md gotcha — the `bindings_are_current` test will fail until `cargo tauri dev` regenerates `bindings.ts`.
