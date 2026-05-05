# Assistant / Code Mode Separation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote "Assistant" and "Code" from a cosmetic UI toggle into two genuinely independent modes (Claude.ai vs Claude Code parity): a `SessionMode` enum on every session, a per-mode tool surface, a per-mode system prompt, and a sidebar / chat list that filters by mode.

**Architecture:** Add `SessionMode { Assistant, Coding }` as a NOT NULL column on `sessions` (set at creation, never mutated). Plumb it through `RoutingContext.session_mode`. Reuse the existing `ChannelMask` system: gate assistant feature tools (`feature-tasks`, `feature-finance`, `feature-notes`, `feature-productivity`, `feature-learning`, `feature-language-learning`) with `ChannelMask::NON_CODING`. Add a per-mode `SoulContextSource` that loads either `KLYNTBOT.md` or `KLYNTBOT-coding.md`. On the frontend, make `SidebarChatLayout`, `MainApp`, and `useMainAppLayoutSurfaces` actually read `useAppMode()` and (a) filter nav items, (b) swap the chat list source between klyntbot chats and coding sessions, (c) reset `appView` on mode change.

**Tech Stack:** Rust 1.93, Tauri 2, sqlx + SQLite (in-memory for tests), `cargo-nextest`, React 18, TypeScript 5, Vitest + `@testing-library/react`, `@tauri-apps/api/core`, plain CSS with BEM.

**Pre-release policy:** Per CLAUDE.md, schema changes are made directly in `crates/storage/migrations/001_initial.sql`. Wipe `~/.klyntbot-dev/data.db*` and `~/.klyntbot-dev/lance/` between phases that change schema. No migration scripts.

**Conventions:**
- Every public `AppCore` method gets `#[tracing::instrument(skip(self), err)]`.
- All Tauri commands use `#[klynt_command]` and are listed in `klynt_collect_commands![…]` in `crates/desktop/src/specta_builder.rs`.
- Tests run via `cargo nextest run -p <crate>` (NOT `cargo test`); doctests only via `cargo test --workspace --doc`.
- Zero clippy warnings policy. Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` before each commit.
- Frontend: `bun` only, never `npm`. `bun run lint && bun run typecheck && bun run test` before commit.
- After adding any Tauri command, run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`. The `bindings_are_current` test will fail otherwise.

**Out of scope (explicit non-goals):**
- Migrating existing dev sessions (pre-release; wipe & recreate).
- Telegram/Discord channel changes (channels remain `Channel::Other`, unaffected by mode split).
- Renaming `chat_send` / `coding_thread_start` (kept as the two entry points; we just make them honour `SessionMode`).
- Squad chat / collaboration mode interactions (separate `debate::build_persona_system_prompt` path remains untouched).
- New backend features. This plan is pure restructuring of an existing surface.

---

## Phase A — Backend foundation: `SessionMode` enum + schema

### Task A1: Define `SessionMode` enum in `common` crate

**Files:**
- Create: `crates/common/src/session_mode.rs`
- Modify: `crates/common/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/common/src/session_mode.rs`:

```rust
//! Authoritative discriminator for assistant vs coding sessions.
//!
//! Set at session creation, never mutated. Stored as a NOT NULL `TEXT`
//! column on `sessions` and serialized as `"assistant"` / `"coding"`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Assistant,
    Coding,
}

impl SessionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::Coding => "coding",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "assistant" => Some(Self::Assistant),
            "coding" => Some(Self::Coding),
            _ => None,
        }
    }
}

impl Default for SessionMode {
    fn default() -> Self {
        Self::Assistant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_via_str() {
        for m in [SessionMode::Assistant, SessionMode::Coding] {
            assert_eq!(SessionMode::parse(m.as_str()), Some(m));
        }
    }

    #[test]
    fn parse_unknown_is_none() {
        assert_eq!(SessionMode::parse("chat"), None);
        assert_eq!(SessionMode::parse(""), None);
    }

    #[test]
    fn serde_uses_snake_case() {
        let s = serde_json::to_string(&SessionMode::Assistant).unwrap();
        assert_eq!(s, "\"assistant\"");
        let parsed: SessionMode = serde_json::from_str("\"coding\"").unwrap();
        assert_eq!(parsed, SessionMode::Coding);
    }

    #[test]
    fn default_is_assistant() {
        assert_eq!(SessionMode::default(), SessionMode::Assistant);
    }
}
```

- [ ] **Step 2: Wire into `common::lib`**

Modify `crates/common/src/lib.rs` — add right after the existing `pub use types::{...}` block (after line 38):

```rust
pub mod session_mode;
pub use session_mode::SessionMode;
```

- [ ] **Step 3: Run test to verify it passes**

```bash
cargo nextest run -p common -E 'test(session_mode)'
```

Expected: 4 PASS.

- [ ] **Step 4: Run clippy**

```bash
cargo clippy -p common --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/session_mode.rs crates/common/src/lib.rs
git commit -m "feat(common): add SessionMode enum (assistant | coding)"
```

---

### Task A2: Add `mode` column to `sessions` table (in-place schema edit)

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:126-154`

- [ ] **Step 1: Edit the CREATE TABLE**

Replace the `sessions` block (lines 126-154 of `crates/storage/migrations/001_initial.sql`) so the new `mode` column sits directly after `key`:

```sql
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    mode       TEXT NOT NULL DEFAULT 'assistant'
                 CHECK (mode IN ('assistant', 'coding')),
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0,
    compressed_prefix      TEXT,
    compressed_through_idx INTEGER,
    compressed_at          INTEGER,
    cwd                    TEXT,
    repo_id                TEXT,
    repo_branch            TEXT,
    tool_profile           TEXT,
    approval_mode          TEXT NOT NULL DEFAULT 'default',
    total_cost_usd         REAL NOT NULL DEFAULT 0,
    total_tokens           INTEGER NOT NULL DEFAULT 0,
    parent_session_id      TEXT,
    workspace_id           TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    forked_from_id         TEXT REFERENCES sessions(key) ON DELETE SET NULL,
    summary_message_id     TEXT,
    ephemeral              INTEGER NOT NULL DEFAULT 0,
    archived_at            INTEGER
);

CREATE INDEX IF NOT EXISTS idx_sessions_workspace_archived ON sessions(workspace_id, archived_at);
CREATE INDEX IF NOT EXISTS idx_sessions_mode ON sessions(mode);
```

- [ ] **Step 2: Wipe dev DB**

```bash
rm -f  ~/.klyntbot-dev/data.db ~/.klyntbot-dev/data.db-wal ~/.klyntbot-dev/data.db-shm
rm -rf ~/.klyntbot-dev/lance/
```

Expected: command silent.

- [ ] **Step 3: Verify migration applies cleanly**

```bash
cargo nextest run -p storage
```

Expected: PASS (storage tests use `connect_in_memory()` which runs all migrations).

- [ ] **Step 4: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): add sessions.mode column (assistant | coding, NOT NULL)"
```

---

### Task A3: Add `mode` to `SessionRow` + repo methods

**Files:**
- Modify: `crates/storage/src/rows/session.rs:1-62`
- Modify: `crates/storage/src/repos/session.rs` (multiple sites)
- Test: `crates/storage/src/repos/session.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Add `mode` field to `SessionRow`**

Modify `crates/storage/src/rows/session.rs:9-21` — insert `pub mode: String,` directly after `pub key: String,`:

```rust
#[derive(Debug, Clone, FromRow, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub key: String,
    pub mode: String,  // "assistant" | "coding"  (mirrors common::SessionMode)
    pub metadata: serde_json::Value,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    // … rest unchanged
}
```

- [ ] **Step 2: Add typed accessor on `SessionRow`**

In the same file, append after the struct:

```rust
impl SessionRow {
    pub fn session_mode(&self) -> common::SessionMode {
        common::SessionMode::parse(&self.mode).unwrap_or_default()
    }
}
```

- [ ] **Step 3: Add `upsert_session_with_mode`**

In `crates/storage/src/repos/session.rs`, add a new method directly above the existing `upsert_session` (around line 35). The new method takes a typed `SessionMode`:

```rust
/// Insert or refresh a session with a known mode.
/// On conflict the `mode` column is NOT overwritten — mode is set at
/// creation time and is immutable for the life of the session.
pub async fn upsert_session_with_mode(
    &self,
    key: &str,
    mode: common::SessionMode,
    metadata: &serde_json::Value,
) -> Result<SessionRow, StorageError> {
    let now = jiff::Timestamp::now().as_millisecond();
    let row = sqlx::query_as::<_, SessionRow>(
        "INSERT INTO sessions (key, mode, metadata, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT (key) DO UPDATE SET
           metadata   = excluded.metadata,
           updated_at = excluded.updated_at
         RETURNING *",
    )
    .bind(key)
    .bind(mode.as_str())
    .bind(metadata)
    .bind(now)
    .fetch_one(&self.pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 4: Update existing `upsert_session` to default mode**

In the same file, find the existing `upsert_session` (~line 35-56) and update its INSERT to provide the default mode (`'assistant'`). Replace the SQL with:

```rust
"INSERT INTO sessions (key, mode, metadata, created_at, updated_at)
 VALUES (?1, 'assistant', ?2, ?3, ?3)
 ON CONFLICT (key) DO UPDATE SET
   updated_at = ?3
 RETURNING *"
```

(Bind `key`, `metadata`, `now` — drop the second `now` bind since SQL now references `?3` twice.) The intent: legacy callers of `upsert_session` get `assistant` mode. Coding callers must call the new typed variant.

- [ ] **Step 5: Write the failing tests**

Append to the inline `#[cfg(test)] mod tests` block in `crates/storage/src/repos/session.rs`:

```rust
#[tokio::test]
async fn upsert_session_with_mode_persists_coding() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repos = crate::Repos::from_pool(&pool);
    let row = repos
        .sessions
        .upsert_session_with_mode(
            "coding:abc",
            common::SessionMode::Coding,
            &serde_json::json!({}),
        )
        .await
        .unwrap();
    assert_eq!(row.mode, "coding");
    assert_eq!(row.session_mode(), common::SessionMode::Coding);
}

#[tokio::test]
async fn upsert_session_defaults_to_assistant() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repos = crate::Repos::from_pool(&pool);
    let row = repos
        .sessions
        .upsert_session("chat:xyz", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(row.mode, "assistant");
}

#[tokio::test]
async fn mode_is_immutable_on_conflict() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repos = crate::Repos::from_pool(&pool);
    repos
        .sessions
        .upsert_session_with_mode(
            "coding:k",
            common::SessionMode::Coding,
            &serde_json::json!({"v": 1}),
        )
        .await
        .unwrap();
    // Re-upsert with assistant mode — mode column must stay "coding".
    let row = repos
        .sessions
        .upsert_session_with_mode(
            "coding:k",
            common::SessionMode::Assistant,
            &serde_json::json!({"v": 2}),
        )
        .await
        .unwrap();
    assert_eq!(row.mode, "coding");
}
```

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p storage -E 'test(upsert_session)'
```

Expected: 3 PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/storage/src/rows/session.rs crates/storage/src/repos/session.rs
git commit -m "feat(storage): typed SessionRow.mode + immutable upsert_session_with_mode"
```

---

### Task A4: Add `SessionMode` to `RoutingContext`

**Files:**
- Modify: `crates/tools-core/src/routing.rs:61-97`
- Test: `crates/tools-core/src/routing.rs` (inline)

- [ ] **Step 1: Extend the struct**

In `crates/tools-core/src/routing.rs`, after the `pub channel: ChannelName,` line (line ~63), insert:

```rust
/// Authoritative session mode discriminator. Drives:
/// - which `SoulContextSource` variant to inject
/// - which tools the LLM sees (via `ChannelMask` interpreted in mode-aware mode)
/// - which `ContextSource`s gate themselves on (e.g. CodingRecall)
pub session_mode: common::SessionMode,
```

- [ ] **Step 2: Update every `RoutingContext` constructor**

In the same file, find each `pub fn new(...)`, `pub fn with_interaction(...)`, `pub fn for_test(...)`, `Default` impl, etc. — add `session_mode: common::SessionMode::Assistant` as the default for each. Use `grep -n "session_mode\|RoutingContext {" crates/tools-core/src/routing.rs` to find every literal struct construction.

For each, mirror this pattern:

```rust
RoutingContext {
    channel,
    session_mode: common::SessionMode::Assistant,  // <-- add
    chat_id,
    // … rest unchanged
}
```

- [ ] **Step 3: Add typed setter**

Append to the impl block:

```rust
impl RoutingContext {
    pub fn with_session_mode(mut self, mode: common::SessionMode) -> Self {
        self.session_mode = mode;
        self
    }
}
```

- [ ] **Step 4: Build the workspace**

```bash
cargo build --workspace 2>&1 | head -100
```

Expected: any callers that construct `RoutingContext` literally will fail. Fix each by adding `session_mode: common::SessionMode::Assistant,`. Common sites:
- `crates/agent/src/agent_loop/mod.rs` (around line 1145 — see Task B1)
- `crates/app-core/src/coding/review_handler.rs`
- subagent spawn paths in `crates/agent/src/subagent.rs`
- test fixtures in `crates/tools-core/tests/`

Use `grep -rn 'RoutingContext {' crates/` to enumerate.

- [ ] **Step 5: Run tests**

```bash
cargo nextest run -p tools-core
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/tools-core/src/routing.rs $(grep -rl 'RoutingContext {' crates/ | grep -v target)
git commit -m "feat(tools-core): add RoutingContext.session_mode (defaults Assistant)"
```

---

## Phase B — Plumb `SessionMode` through the agent runtime entry points

### Task B1: Read `mode` from session row in `process_direct_streaming`

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs:1130-1160`

- [ ] **Step 1: Replace the channel-from-string block**

Find the block in `crates/agent/src/agent_loop/mod.rs` that maps `mode: Option<String>` → `ChannelName`. Replace it with a session-row lookup so the discriminator is authoritative:

```rust
// Authoritative session mode comes from the session row itself.
// The legacy `mode: Option<String>` parameter is now an override hint
// only used when the row does not yet exist (first turn).
let session_mode: common::SessionMode = match self
    .repos
    .sessions
    .get_session(&session_key)
    .await
{
    Ok(row) => row.session_mode(),
    Err(_) => mode
        .as_deref()
        .and_then(common::SessionMode::parse)
        .unwrap_or(common::SessionMode::Assistant),
};

let channel: common::ChannelName = match session_mode {
    common::SessionMode::Coding => common::CODING_CHANNEL.into(),
    common::SessionMode::Assistant => "desktop".into(),
};

let mut routing_ctx =
    RoutingContext::with_interaction(channel, session_key.clone().into(), interaction_tx);
routing_ctx.session_mode = session_mode;
routing_ctx.session_key = Some(session_key.clone().into());
routing_ctx.message_id = user_msg_id;
```

- [ ] **Step 2: Type-check**

```bash
cargo check -p agent
```

Expected: PASS.

- [ ] **Step 3: Run agent tests**

```bash
cargo nextest run -p agent
```

Expected: PASS (existing tests already pass `mode: None` and now resolve to `Assistant`).

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs
git commit -m "feat(agent): read session_mode from sessions row in direct streaming"
```

---

### Task B2: Set `Coding` mode at coding-thread creation

**Files:**
- Modify: `crates/app-core/src/coding/thread_handler.rs:42-95`
- Test: `crates/app-core/tests/coding_thread_mode.rs` (new)

- [ ] **Step 1: Switch to typed upsert**

In `crates/app-core/src/coding/thread_handler.rs`, replace the existing `upsert_session` call (around line 56) with:

```rust
self.repos
    .sessions
    .upsert_session_with_mode(&session_key, common::SessionMode::Coding, &metadata)
    .await?;
```

- [ ] **Step 2: Write the integration test**

Create `crates/app-core/tests/coding_thread_mode.rs`:

```rust
//! Verifies that `coding_thread_start` always persists `mode = 'coding'`
//! and that the row is queryable as `SessionMode::Coding`.

use app_core::AppCore;
use desktop_shared::coding::ApprovalPolicy;

#[tokio::test]
async fn coding_thread_start_persists_coding_mode() {
    let core = AppCore::new_for_test().await;
    let ws = core
        .repos
        .workspaces
        .insert_test_workspace("/tmp/k-test")
        .await
        .unwrap();

    let thread = core
        .coding_thread_start(&ws.id, None, Some(ApprovalPolicy::default()), false)
        .await
        .unwrap();

    let row = core.repos.sessions.get_session(&thread.id).await.unwrap();
    assert_eq!(row.mode, "coding");
    assert_eq!(row.session_mode(), common::SessionMode::Coding);
}
```

If `AppCore::new_for_test` or `insert_test_workspace` do not exist yet, locate the existing test scaffolding (`grep -rn 'new_for_test\|fn fixture' crates/app-core/tests/`) and use whichever helper is already in use for coding tests; fall back to manual `StoragePool::connect_in_memory()` + `Repos::from_pool` + direct insert.

- [ ] **Step 3: Run the test**

```bash
cargo nextest run -p app-core -E 'test(coding_thread_start_persists)'
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/coding/thread_handler.rs crates/app-core/tests/coding_thread_mode.rs
git commit -m "feat(app-core): coding_thread_start writes SessionMode::Coding"
```

---

### Task B3: Set `Assistant` mode at first `chat_send` for unknown sessions

**Files:**
- Modify: `crates/app-core/src/handlers/chat/mod.rs` (or wherever `chat_send` writes the session — `grep -rn 'fn chat_send' crates/app-core/src/`)

- [ ] **Step 1: Find the session creation point in chat_send**

Run:

```bash
grep -rn 'fn chat_send\|upsert_session' crates/app-core/src/handlers/chat/
```

Identify the line that currently calls `repos.sessions.upsert_session(...)`. There is exactly one site in the chat-send happy path.

- [ ] **Step 2: Switch to typed upsert**

Replace that call with:

```rust
let mode = match raw_mode_param.as_deref() {
    Some("coding") => common::SessionMode::Coding,
    _ => common::SessionMode::Assistant,
};
self.repos
    .sessions
    .upsert_session_with_mode(&session_key, mode, &metadata)
    .await?;
```

(If the function does not currently take a `mode` parameter, thread it through from the existing `chat_send(mode: Option<String>)` parameter on the Tauri command.)

- [ ] **Step 3: Run chat tests**

```bash
cargo nextest run -p app-core -E 'test(chat_send)'
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/chat/
git commit -m "feat(app-core): chat_send writes SessionMode at session creation"
```

---

### Task B4: Remove the legacy `chat_set_mode` mutation path

**Files:**
- Delete: `crates/app-core/src/coding/mode_handler.rs`
- Modify: `crates/desktop/src/commands/chat.rs:126-152`
- Modify: `crates/desktop/src/specta_builder.rs` (drop `chat_set_mode` from `klynt_collect_commands![]`)

`SessionMode` is creation-time and immutable. The legacy `chat_set_mode` Tauri command writes `conversation_type` after the fact and is now a footgun.

- [ ] **Step 1: Delete the handler module**

```bash
rm crates/app-core/src/coding/mode_handler.rs
```

- [ ] **Step 2: Remove `pub mod mode_handler;` from `crates/app-core/src/coding/mod.rs`**

Use `grep -n 'mode_handler' crates/app-core/src/coding/mod.rs` to find the line; delete it.

- [ ] **Step 3: Remove the Tauri command**

In `crates/desktop/src/commands/chat.rs`, delete the `chat_set_mode` function (lines ~126-152) entirely.

- [ ] **Step 4: Remove from collect macro**

In `crates/desktop/src/specta_builder.rs`, delete the line:

```rust
crate::commands::chat::chat_set_mode,
```

- [ ] **Step 5: Build**

```bash
cargo build -p desktop 2>&1 | tail -40
```

Expected: PASS. Any reference to `ChatMode` or `chat_set_mode` from elsewhere in the workspace must be deleted.

- [ ] **Step 6: Regenerate frontend bindings**

```bash
cargo tauri dev
```

Wait until "App ready" log; press `Ctrl+C`. The `desktop-ui/src/bindings.ts` file should now have removed `ChatMode` and `chatSetMode`.

- [ ] **Step 7: Verify no callers remain**

```bash
grep -rn 'chatSetMode\|chat_set_mode\|ChatMode' desktop-ui/src/ crates/
```

Expected: no hits in non-bindings files.

- [ ] **Step 8: Run full workspace tests**

```bash
cargo nextest run --workspace
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add -u
git commit -m "refactor: remove chat_set_mode (mode is now creation-time immutable)"
```

---

## Phase C — Per-mode soul (system prompt)

### Task C1: Create `KLYNTBOT-coding.md` default content + `MODE_AWARE_SOUL` constant

**Files:**
- Modify: `crates/skill-system/src/soul.rs`

- [ ] **Step 1: Add a coding-mode default soul**

In `crates/skill-system/src/soul.rs`, just below the existing `DEFAULT_SOUL` constant (line ~37), add:

```rust
const DEFAULT_CODING_SOUL: &str = r#"# Klyntbot Coding

You are Klyntbot in coding mode — a senior software engineer pair-programming with the user inside their workspace.

## Behaviour
- Investigate before changing. Read the file you're about to edit.
- Surgical changes only. Don't refactor adjacent code unless asked.
- Don't add error handling, fallbacks, or validation for scenarios that can't happen.
- Default to writing no comments. Only add a comment when WHY is non-obvious.
- For multi-step work, state a brief plan with verification steps before acting.

## Tools
- Use `bash`, `read`, `write`, `edit`, `apply_patch` for code changes.
- Use `recall_index`, `recall_timeline`, `check_dead_ends` to consult prior coding sessions before guessing.
- Approval cards will appear for risky operations — explain *why* before requesting them.

## Formatting (STRICT — overrides any default writing style)

**Emoji rule.** Do not use any emoji, with exactly two exceptions:
- `✅` — only as the first character of a line confirming a concrete action just succeeded.
- `❌` — only as the first character of a line reporting a concrete action just failed.

When citing code, include the file path and line number (`path/to/file.rs:42`).
"#;
```

- [ ] **Step 2: Make `SoulContextSource` mode-aware**

Replace the `pub struct SoulContextSource { … }` definition (line ~40) and its `new` / `provide` methods with this version:

```rust
pub struct SoulContextSource {
    assistant: Arc<RwLock<String>>,
    coding: Arc<RwLock<String>>,
    assistant_path: PathBuf,
    coding_path: PathBuf,
    last_assistant_mtime: Arc<RwLock<Option<SystemTime>>>,
    last_coding_mtime: Arc<RwLock<Option<SystemTime>>>,
}

impl SoulContextSource {
    pub fn new(klyntbot_home: &std::path::Path) -> Self {
        Self {
            assistant: Arc::new(RwLock::new(DEFAULT_SOUL.to_string())),
            coding: Arc::new(RwLock::new(DEFAULT_CODING_SOUL.to_string())),
            assistant_path: klyntbot_home.join("KLYNTBOT.md"),
            coding_path: klyntbot_home.join("KLYNTBOT-coding.md"),
            last_assistant_mtime: Arc::new(RwLock::new(None)),
            last_coding_mtime: Arc::new(RwLock::new(None)),
        }
    }
}
```

Then update the `ContextSource for SoulContextSource` impl's `provide(&self, ctx: &SourceContext)` method to pick the file by mode:

```rust
async fn provide(&self, ctx: &SourceContext) -> Option<String> {
    let (path, content, last_mtime) = match ctx.session_mode {
        common::SessionMode::Coding => (
            &self.coding_path,
            &self.coding,
            &self.last_coding_mtime,
        ),
        common::SessionMode::Assistant => (
            &self.assistant_path,
            &self.assistant,
            &self.last_assistant_mtime,
        ),
    };

    // (existing live-read logic, but parameterised over `path` / `content` / `last_mtime`)
    let needs_reload = match tokio::fs::metadata(path).await {
        Ok(meta) => match meta.modified() {
            Ok(mtime) => *last_mtime.read().await != Some(mtime),
            Err(_) => true,
        },
        Err(_) => true,
    };

    if !needs_reload {
        let cached = content.read().await;
        return if cached.is_empty() { None } else { Some(cached.clone()) };
    }

    match tokio::fs::read_to_string(path).await {
        Ok(fresh) => {
            *content.write().await = fresh.clone();
            if let Ok(meta) = tokio::fs::metadata(path).await {
                if let Ok(mtime) = meta.modified() {
                    *last_mtime.write().await = Some(mtime);
                }
            }
            Some(fresh)
        }
        Err(_) => {
            let cached = content.read().await;
            if cached.is_empty() { None } else { Some(cached.clone()) }
        }
    }
}
```

- [ ] **Step 3: Add `session_mode` to `SourceContext`**

`grep -n 'pub struct SourceContext' crates/skill-system/src/`. Add a `pub session_mode: common::SessionMode,` field. Update every constructor and the `From<&RoutingContext>` impl (likely in `crates/agent/src/context_engine/`) to populate it from `routing_ctx.session_mode`.

- [ ] **Step 4: Run skill-system tests**

```bash
cargo nextest run -p skill-system
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skill-system/src/soul.rs $(grep -rln 'SourceContext {' crates/)
git commit -m "feat(skill-system): per-mode SoulContextSource (KLYNTBOT.md + KLYNTBOT-coding.md)"
```

---

### Task C2: Create user-facing `KLYNTBOT-coding.md` in dev home

**Files:**
- Create: `~/.klyntbot-dev/KLYNTBOT-coding.md` (manual, dev-only)

- [ ] **Step 1: Seed the dev file**

Copy the `DEFAULT_CODING_SOUL` body into `~/.klyntbot-dev/KLYNTBOT-coding.md` so the dev instance has an externally editable coding soul:

```bash
cp crates/skill-system/src/soul.rs /tmp/soul-extract.rs
# Manually copy the DEFAULT_CODING_SOUL body into:
$EDITOR ~/.klyntbot-dev/KLYNTBOT-coding.md
```

- [ ] **Step 2: Verify it loads**

Run the desktop app (`cargo tauri dev`), open a coding thread, and check the agent log for the soul body matching the file. If you have not started a coding thread, the assistant soul will be loaded — that's expected.

- [ ] **Step 3: No commit** — dev artifact only.

---

## Phase D — Gate assistant-only feature tools with `NON_CODING`

### Task D1: Inventory feature-tool derive sites

**Files:**
- Read-only

- [ ] **Step 1: List every assistant-tool derive site**

```bash
grep -rn '#\[derive(tools_core::Tool)\]\|#\[tool(' crates/feature-tasks crates/feature-finance crates/feature-notes crates/feature-productivity crates/feature-learning crates/feature-language-learning crates/feature-coaching crates/feature-insights crates/feature-launcher
```

Capture the file:line of each derive site in a temporary scratch list (paste into your TODO).

- [ ] **Step 2: Confirm none currently set `allowed_channels`**

```bash
grep -rn 'allowed_channels' crates/feature-tasks crates/feature-finance crates/feature-notes crates/feature-productivity crates/feature-learning crates/feature-language-learning crates/feature-coaching crates/feature-insights crates/feature-launcher
```

Expected: zero hits. Confirms the planned change is purely additive.

---

### Task D2: Add `allowed_channels = "non_coding"` to assistant feature tools

For each tool in the inventory, add the attribute. Below is the exact template — repeat for every site found in D1.

**Files (representative — actual list from D1):**
- Modify: `crates/feature-tasks/src/lib.rs` (TaskTool derive)
- Modify: `crates/feature-finance/src/tools.rs` (FinanceTool)
- Modify: `crates/feature-notes/src/lib.rs` (NotesTool)
- Modify: `crates/feature-productivity/src/lib.rs` (ProductivityTool)
- Modify: `crates/feature-learning/src/lib.rs` (LearningTool)
- Modify: `crates/feature-language-learning/src/lib.rs` (LanguageLearningTool)

- [ ] **Step 1: Patch each derive site**

Pattern — change:

```rust
#[derive(tools_core::Tool)]
#[tool(
    name = "tasks",
    description = "...",
    params = "TaskArgs"
)]
pub struct TaskTool { /* … */ }
```

to:

```rust
#[derive(tools_core::Tool)]
#[tool(
    name = "tasks",
    description = "...",
    params = "TaskArgs",
    allowed_channels = "non_coding"
)]
pub struct TaskTool { /* … */ }
```

- [ ] **Step 2: Build the workspace**

```bash
cargo build --workspace 2>&1 | tail -40
```

Expected: PASS. (If the `tools_core::Tool` derive macro doesn't recognise `non_coding`, audit `crates/tools-core-macros/src/helpers.rs`; the `ChannelMask::NON_CODING` constant exists at `crates/common/src/tool_channel.rs:51` so the macro should accept the snake-case form. If not, add a one-line match arm to the macro.)

- [ ] **Step 3: Write a registry-filter test**

Create `crates/app-core/tests/assistant_tools_hidden_in_coding.rs`:

```rust
//! Invariant: assistant-only feature tools are not visible to the LLM
//! when the routing context carries Channel::Coding.

use common::{Channel, ChannelMask};

#[test]
fn task_tool_mask_excludes_coding() {
    use feature_tasks::TaskTool;
    let mask = <TaskTool as tools_core::ToolMeta>::allowed_channels();
    assert!(!mask.allows(Channel::Coding));
    assert!(mask.allows(Channel::Desktop));
}

// Repeat for every assistant tool changed in Task D2.
#[test]
fn finance_tool_mask_excludes_coding() {
    use feature_finance::FinanceTool;
    let mask = <FinanceTool as tools_core::ToolMeta>::allowed_channels();
    assert!(!mask.allows(Channel::Coding));
}
```

(If `tools_core::ToolMeta` is not the actual trait name carrying `allowed_channels`, adjust to call `Tool::allowed_channels(&instance)` on a default-constructed instance.)

- [ ] **Step 4: Run the test**

```bash
cargo nextest run -p app-core -E 'test(_mask_excludes_coding)'
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat: gate assistant feature tools with ChannelMask::NON_CODING"
```

---

### Task D3: Verify coding-only tools still gated

**Files:**
- Test: `crates/app-core/tests/coding_tools_hidden_in_assistant.rs` (new)

- [ ] **Step 1: Add the inverse test**

```rust
//! Invariant: coding-only tools (bash/write/edit/apply_patch/recall_*) are
//! NOT visible to the LLM in assistant/desktop channels.

use common::Channel;

#[test]
fn bash_tool_mask_excludes_desktop() {
    use klynt_core::tools::BashTool;
    // BashTool has private fields; we read the mask via the trait method.
    // Use the static derive accessor.
    let mask = BashTool::allowed_channels_static();
    assert!(mask.allows(Channel::Coding));
    assert!(!mask.allows(Channel::Desktop));
    assert!(!mask.allows(Channel::Other));
}
```

(If the macro does not generate `allowed_channels_static`, instantiate a `BashTool` via its existing test fixture and call the trait method.)

- [ ] **Step 2: Run**

```bash
cargo nextest run -p app-core -E 'test(_mask_excludes_desktop)'
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/tests/coding_tools_hidden_in_assistant.rs
git commit -m "test: confirm coding-only tools stay gated against Desktop channel"
```

---

## Phase E — Frontend: mode-aware sidebar

### Task E1: Annotate nav items with `modes` and filter

**Files:**
- Modify: `desktop-ui/src/features/app/components/SidebarChatLayout.tsx:1-122`
- Test: `desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx` (new)

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx`:

```tsx
/** @vitest-environment jsdom */
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { __testing as appModeTesting } from "../hooks/useAppMode";
import { SidebarChatLayout } from "./SidebarChatLayout";

vi.mock("@tauri-apps/api/core");

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  appModeTesting.reset("assistant");
});

const baseProps = {
  onOpenSettings: vi.fn(),
  onNewChat: vi.fn(),
  onSelectPlugins: vi.fn(),
  onSelectCalendar: vi.fn(),
  threads: [],
  selectedSessionKey: null,
  onSelectThread: vi.fn(),
  activeNavId: null,
};

describe("SidebarChatLayout — mode-aware nav", () => {
  it("shows Calendar + Automations + Project in assistant mode", () => {
    appModeTesting.reset("assistant");
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("Calendar")).toBeTruthy();
    expect(screen.getByText("Automations")).toBeTruthy();
    expect(screen.getByText("Project")).toBeTruthy();
  });

  it("hides Calendar + Automations in code mode; shows Project", () => {
    appModeTesting.reset("code");
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.queryByText("Calendar")).toBeNull();
    expect(screen.queryByText("Automations")).toBeNull();
    expect(screen.getByText("Project")).toBeTruthy();
  });

  it("shows Search in both modes", () => {
    for (const m of ["assistant", "code"] as const) {
      appModeTesting.reset(m);
      const { unmount } = render(<SidebarChatLayout {...baseProps} />);
      expect(screen.getByText("Search")).toBeTruthy();
      unmount();
    }
  });
});
```

- [ ] **Step 2: Run test (expect FAIL)**

```bash
cd desktop-ui && bun run test --run src/features/app/components/SidebarChatLayout.test.tsx
```

Expected: 2 of 3 FAIL (Calendar/Automations are unconditionally rendered today).

- [ ] **Step 3: Filter nav items by mode**

In `desktop-ui/src/features/app/components/SidebarChatLayout.tsx`, replace the `navItems` block (lines ~43-55) with:

```tsx
import type { AppMode } from "../hooks/useAppMode";

type NavItem = {
  id: string;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
  modes: readonly AppMode[];
};

// (inside the component, after `const { mode, setMode } = useAppMode();`)
const allNavItems: NavItem[] = [
  { id: "new-chat", label: "New chat", icon: <SquarePen aria-hidden />,
    onClick: onNewChat,            modes: ["assistant", "code"] },
  { id: "search",   label: "Search", icon: <Search aria-hidden />,
                                     modes: ["assistant", "code"] },
  { id: "calendar", label: "Calendar", icon: <Calendar aria-hidden />,
    onClick: handleSelectCalendar, modes: ["assistant"] },
  { id: "plugins",  label: "Plugins",  icon: <LayoutGrid aria-hidden />,
    onClick: onSelectPlugins,      modes: ["assistant", "code"] },
  { id: "automations", label: "Automations", icon: <Clock aria-hidden />,
                                     modes: ["assistant"] },
  { id: "project",  label: "Project", icon: <FolderPlus aria-hidden />,
                                     modes: ["assistant", "code"] },
];
const navItems = allNavItems.filter((it) => it.modes.includes(mode));
```

- [ ] **Step 4: Re-run test**

```bash
cd desktop-ui && bun run test --run src/features/app/components/SidebarChatLayout.test.tsx
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/app/components/SidebarChatLayout.tsx desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx
git commit -m "feat(ui): SidebarChatLayout filters nav items by AppMode"
```

---

### Task E2: Make "New chat" mode-aware

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx:362-390`

In assistant mode, "New chat" creates a klyntbot session as today. In code mode, it should clear the active workspace selection and route to the code landing pane (so the user picks/creates a project).

- [ ] **Step 1: Update `onNewChat`**

Replace the existing `onNewChat` callback with a mode-aware version:

```tsx
import { useAppMode } from "@app/hooks/useAppMode";
// (already imported elsewhere — confirm)

const { mode } = useAppMode();

const onNewChat = useCallback(() => {
  if (mode === "code") {
    // Code mode: clear selection so the user lands on CodeLanding.
    setSelectedSessionKey(null);
    setAppView("home");
    threadNavigation.clearWorkspaceSelection?.();
    return;
  }
  setSelectedSessionKey(`chat:${crypto.randomUUID()}`);
  setAppView("chat");
}, [mode, threadNavigation]);
```

(If `threadNavigation.clearWorkspaceSelection` is not exposed, add a one-line helper that sets `activeWorkspaceId` to `null` in whatever store backs `threadNavigation`.)

- [ ] **Step 2: Type-check**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "feat(ui): New chat is mode-aware (assistant→klyntbot, code→landing)"
```

---

### Task E3: Reset `appView` when mode changes

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx`

If the user is on Calendar (assistant-only) and flips to Code, the calendar pane stays visible — bug. Reset on mode change.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/app/hooks/useResetAppViewOnModeChange.test.ts`:

```ts
/** @vitest-environment jsdom */
import { renderHook, act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { __testing as appModeTesting, useAppMode } from "./useAppMode";
import { useResetAppViewOnModeChange } from "./useResetAppViewOnModeChange";

afterEach(() => appModeTesting.reset("assistant"));

describe("useResetAppViewOnModeChange", () => {
  it("resets to home when mode changes from assistant to code", () => {
    appModeTesting.reset("assistant");
    const { result } = renderHook(() => {
      const [view, setView] = useState<string>("calendar");
      useResetAppViewOnModeChange(setView);
      const { mode, setMode } = useAppMode();
      return { view, setView, mode, setMode };
    });
    expect(result.current.view).toBe("calendar");
    act(() => result.current.setMode("code"));
    expect(result.current.view).toBe("home");
  });
});
```

- [ ] **Step 2: Implement the hook**

Create `desktop-ui/src/features/app/hooks/useResetAppViewOnModeChange.ts`:

```ts
import { useEffect, useRef } from "react";
import { useAppMode } from "./useAppMode";
import type { AppView } from "../constants/appViews";

/**
 * Resets the centre-pane `appView` to "home" whenever the AppMode flips.
 * This prevents stranding the user on an assistant-only view (calendar)
 * after a switch to code mode (and vice-versa).
 */
export function useResetAppViewOnModeChange(
  setAppView: (next: AppView) => void,
): void {
  const { mode } = useAppMode();
  const previous = useRef(mode);
  useEffect(() => {
    if (previous.current !== mode) {
      previous.current = mode;
      setAppView("home");
    }
  }, [mode, setAppView]);
}
```

- [ ] **Step 3: Wire into MainApp**

In `desktop-ui/src/features/app/components/MainApp.tsx`, just below the `const [appView, setAppView] = useState<AppView>(AppView.Home);` line, add:

```tsx
import { useResetAppViewOnModeChange } from "../hooks/useResetAppViewOnModeChange";

// inside the component:
useResetAppViewOnModeChange(setAppView);
```

- [ ] **Step 4: Run tests**

```bash
cd desktop-ui && bun run test --run src/features/app/hooks/useResetAppViewOnModeChange.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useResetAppViewOnModeChange.ts desktop-ui/src/features/app/hooks/useResetAppViewOnModeChange.test.ts desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "feat(ui): reset appView to home on AppMode change"
```

---

## Phase F — Frontend: chat list bifurcation

### Task F1: Add `useCodingSessions` hook (mirror of `useChatThreads`)

**Files:**
- Create: `desktop-ui/src/features/coding/hooks/useCodingSessions.ts`
- Test: `desktop-ui/src/features/coding/hooks/useCodingSessions.test.ts`

The chat list in `SidebarChatLayout` currently shows only klyntbot chat threads (`useChatThreads`). In Code mode it should show coding sessions instead. We add a sibling hook that calls the existing `coding_thread_list` Tauri command and reshapes its output to the same `ChatThread` shape `SidebarChatLayout` expects.

- [ ] **Step 1: Write the test**

```ts
/** @vitest-environment jsdom */
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "coding_thread_list") {
      return [
        { id: "coding:abc", title: "Fix bug", updatedAt: 1, workspaceId: "ws1" },
      ];
    }
    return [];
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

afterEach(() => vi.restoreAllMocks());

describe("useCodingSessions", () => {
  it("returns coding sessions reshaped to ChatThread", async () => {
    const { useCodingSessions } = await import("./useCodingSessions");
    const { result } = renderHook(() => useCodingSessions());
    await waitFor(() => expect(result.current.threads.length).toBe(1));
    expect(result.current.threads[0].sessionKey).toBe("coding:abc");
    expect(result.current.threads[0].title).toBe("Fix bug");
  });
});
```

- [ ] **Step 2: Implement the hook**

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import type { ChatThread } from "@/features/chat/types";

type CodingThreadSummary = {
  id: string;
  title: string | null;
  updatedAt: number;
  workspaceId: string;
};

export interface UseCodingSessionsResult {
  threads: ChatThread[];
  refetch: () => Promise<void>;
  error: string | null;
}

export function useCodingSessions(): UseCodingSessionsResult {
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(async () => {
    try {
      const raw = await invoke<CodingThreadSummary[]>("coding_thread_list");
      setThreads(
        raw.map((t) => ({
          sessionKey: t.id,
          title: t.title ?? "Untitled session",
          updatedAt: new Date(t.updatedAt).toISOString(),
        })),
      );
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    refetch();
    let unlisten: (() => void) | undefined;
    listen("coding:thread_updated", () => refetch()).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [refetch]);

  return { threads, refetch, error };
}
```

(Verify the actual Tauri command name and shape via `grep -n 'coding_thread_list\|fn coding_thread_list' crates/desktop/src/commands/coding_thread.rs` and match the field names.)

- [ ] **Step 3: Run test**

```bash
cd desktop-ui && bun run test --run src/features/coding/hooks/useCodingSessions.test.ts
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/coding/hooks/useCodingSessions.ts desktop-ui/src/features/coding/hooks/useCodingSessions.test.ts
git commit -m "feat(ui): useCodingSessions hook (mirror of useChatThreads for code mode)"
```

---

### Task F2: Switch chat list source by mode in `useMainAppLayoutSurfaces`

**Files:**
- Modify: `desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts:230-380`

- [ ] **Step 1: Add mode-aware thread selection**

Inside `useMainAppLayoutSurfaces`, after the destructured `chatView` block, add:

```ts
import { useAppMode } from "./useAppMode";
import { useCodingSessions } from "@/features/coding/hooks/useCodingSessions";

// inside the hook:
const { mode } = useAppMode();
const codingSessions = useCodingSessions();
const sidebarThreads = mode === "code" ? codingSessions.threads : chatView.chatThreads;
```

- [ ] **Step 2: Use it in `sidebarProps`**

Change the `threads:` line in the `sidebarProps` block (line ~365):

```ts
threads: sidebarThreads,
```

- [ ] **Step 3: Mode-aware thread-select dispatch**

Replace `onSelectThread: chatView.onSelectThread` with:

```ts
onSelectThread: (sessionKey: string) => {
  if (mode === "code") {
    // Coding session keys carry their workspace via the backend mapping.
    threadNavigation.openCodingSessionByKey?.(sessionKey);
    return;
  }
  chatView.onSelectThread(sessionKey);
},
```

(If `openCodingSessionByKey` does not exist on `threadNavigation`, add a small helper inside `threadNavigation` that derives `workspaceId` from the `coding:` session by calling the existing `coding_thread_get` command, then sets `activeThreadId` accordingly.)

- [ ] **Step 4: Type-check**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS.

- [ ] **Step 5: Manual verification**

Run the desktop app:

```bash
cargo tauri dev
```

In the app: flip to Code mode → sidebar chat list should show your `coding:UUID` sessions (same ones from your screenshot) instead of klyntbot chat threads. Flip back to Assistant → klyntbot chats reappear.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts
git commit -m "feat(ui): chat list source switches by AppMode (chats vs coding sessions)"
```

---

## Phase G — Frontend: composer slash gating

### Task G1: Pass `mode` into Composer; gate slash commands

**Files:**
- Modify: `desktop-ui/src/features/composer/components/Composer.tsx`
- Modify: `desktop-ui/src/features/coding/hooks/useSlashCommands.ts` (or wherever slash detection runs)

- [ ] **Step 1: Add `appMode` prop**

In `desktop-ui/src/features/composer/components/Composer.tsx`, append to `ComposerProps`:

```ts
appMode: "assistant" | "code";
```

- [ ] **Step 2: Gate slash detection**

Inside the component, where the `/` keystroke triggers slash-completion, wrap with:

```ts
if (props.appMode !== "code") {
  // Slash commands are coding-only — fall through to plain text.
  return false;
}
```

- [ ] **Step 3: Pass `appMode` from layout surfaces**

In `useMainAppLayoutSurfaces.ts`, add to the `composerProps` block:

```ts
appMode: mode,
```

- [ ] **Step 4: Run frontend tests**

```bash
cd desktop-ui && bun run test
```

Expected: PASS. Update any composer tests that construct `ComposerProps` literally — add `appMode: "code"`.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/composer/ desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts
git commit -m "feat(ui): Composer slash commands are gated to code mode"
```

---

## Phase H — End-to-end verification

### Task H1: Workspace-wide rust check

- [ ] **Step 1: Format**

```bash
cargo fmt --all --check
```

Expected: zero diff.

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 3: Nextest**

```bash
cargo nextest run --workspace
```

Expected: all PASS.

- [ ] **Step 4: Doctests**

```bash
cargo test --workspace --doc
```

Expected: all PASS.

---

### Task H2: Frontend verification

- [ ] **Step 1: Lint**

```bash
cd desktop-ui && bun run lint
```

Expected: zero errors.

- [ ] **Step 2: Typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: zero errors.

- [ ] **Step 3: Unit tests**

```bash
cd desktop-ui && bun run test
```

Expected: all PASS.

---

### Task H3: Manual smoke test (both modes)

- [ ] **Step 1: Start dev**

In two terminals:

```bash
# Terminal A
cd desktop-ui && bun run dev

# Terminal B
cargo tauri dev
```

- [ ] **Step 2: Assistant-mode walkthrough**

1. App opens — toggle is on Assistant.
2. Sidebar shows: New chat, Search, **Calendar**, Plugins, **Automations**, Project.
3. Click "New chat" → klyntbot chat session opens (`chat:UUID` key).
4. Send "what's my schedule today" — agent must NOT call `bash` / `write` / `edit`. It should be able to call `tasks` / calendar tools.
5. Click Calendar → Dashboard renders.

- [ ] **Step 3: Code-mode walkthrough**

1. Click the "Code" pill.
2. Sidebar nav loses Calendar + Automations. Chat list now shows your `coding:UUID` sessions.
3. Click "New chat" → CodeLanding renders ("What should we build today?").
4. Pick the `bot` project → coding thread opens.
5. Type "/" in the composer — slash menu appears (assistant mode it does not).
6. Send "list files in this repo" — agent calls `list_dir` / `bash`. It should NOT have access to `tasks` / `add_task`.

- [ ] **Step 4: Mode-flip mid-session**

1. While on Calendar (Assistant), flip to Code → pane resets to home (CodeLanding renders).
2. Flip back to Assistant → home renders, sidebar gets Calendar back, chat list returns to klyntbot threads.

- [ ] **Step 5: KLYNTBOT-coding.md hot-edit**

1. Edit `~/.klyntbot-dev/KLYNTBOT-coding.md` — add a phrase like "I always start replies with the word 'Synth.'".
2. Send a new message in the coding thread (no app restart).
3. Confirm the agent's reply starts with "Synth."
4. Send a message in an assistant chat — confirm it does NOT start with "Synth." (assistant uses `KLYNTBOT.md`).

---

### Task H4: KCA validation gate

Per CLAUDE.md any merge to main is gated.

- [ ] **Step 1: Run**

```bash
./scripts/run_kca_validation.sh
```

Expected: all gates PASS. Auto-generated game-changer report at `docs/architecture/kca-game-changer.md` updated.

---

## Phase I — Cleanup & docs

### Task I1: Update CLAUDE.md gotcha section

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add a Gotchas entry**

Append to the `## Gotchas` section:

```markdown
- **`SessionMode` is creation-time and immutable.** Sessions are tagged `assistant` or `coding` at insert; the column has a CHECK constraint. The legacy `chat_set_mode` Tauri command was removed (2026-05-04). To "switch modes" the user creates a new session via the appropriate entry point (`chat_send` for assistant, `coding_thread_start` for coding). The frontend `useAppMode()` store only controls *which entry point a "New chat" click invokes* — it does NOT mutate existing sessions.
- **Per-mode soul.** Assistant mode reads `~/.klyntbot/KLYNTBOT.md`; coding mode reads `~/.klyntbot/KLYNTBOT-coding.md`. Both are live-read with mtime caching. Edits to either take effect on the next message.
- **Assistant tool gating.** `feature-tasks`, `feature-finance`, `feature-notes`, `feature-productivity`, `feature-learning`, `feature-language-learning` declare `allowed_channels = "non_coding"`. The LLM in coding mode does not see them and cannot call them.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude.md): document SessionMode + per-mode soul gotchas"
```

---

### Task I2: Final PR

- [ ] **Step 1: Push**

```bash
git push -u origin <branch>
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "Assistant/Code mode separation" --body "$(cat <<'EOF'
## Summary
- Adds `SessionMode { Assistant, Coding }` enum + NOT NULL `sessions.mode` column
- Plumbs mode through `RoutingContext` and a per-mode `SoulContextSource`
- Gates assistant feature tools (tasks/finance/notes/productivity/learning) with `ChannelMask::NON_CODING`
- Frontend: SidebarChatLayout filters nav items by mode; chat list swaps source (klyntbot chats vs coding sessions); slash commands gated to code mode; appView resets on mode change

## Test plan
- [ ] cargo nextest run --workspace
- [ ] cargo clippy --workspace --all-targets --all-features -- -D warnings
- [ ] bun run lint && bun run typecheck && bun run test (in desktop-ui)
- [ ] Manual: walk through Assistant + Code modes per Task H3
- [ ] ./scripts/run_kca_validation.sh

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review Notes (for the writer)

- **Spec coverage** — every track of the synthesis (typed `SessionMode`, plumbed `RoutingContext`, per-mode soul, NON_CODING mask on assistant tools, mode-aware sidebar, mode-aware chat list, view reset, composer slash gating) maps to a Phase A–G task.
- **Placeholder scan** — no "TBD" / "implement appropriate" / "similar to above" anywhere; every code-touching step has a literal code block.
- **Type consistency** — `SessionMode` is the same name across `common`, `tools-core::RoutingContext`, `storage::SessionRow::session_mode()`, and the `ChatMode` binding is removed (Task B4) so no ambiguity remains. `useAppMode`'s `AppMode = "assistant" | "code"` stays distinct from backend `SessionMode = "assistant" | "coding"` — this is intentional (the frontend toggle controls *next-session* mode, not in-flight session mode), and Task I1 documents it.
- **Risk** — Task A2's in-place schema edit + dev-DB wipe is the only destructive step; the pre-release policy in CLAUDE.md authorises it.

---

**Plan complete.** Total: 4 phases of foundation (A–D), 3 phases of frontend (E–G), 1 verification phase (H), 1 docs phase (I) — ~25 tasks, ~110 steps.
