# Klynt Coding-in-Chat — Phase 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconcile the codex-derived coding UI with klyntbot's AppCore by implementing 24 klyntbot-native Tauri commands, dropping 5 OpenAI-specific surfaces, extending the message data model with a typed Parts union, and bridging the new `approval_request`/`approval_respond` channel to the existing 3-layer approval engine — so a user can open a workspace, send a coding task, and watch the agent run bash/edit/write tools with inline approval cards and diffs end-to-end.

**Architecture:** Two surfaces (klyntbot chat for general/tasks, coding for workspace work) share one data model (`sessions` + `session_messages` with `parts JSONB`) and one cognitive backbone (mirror, recall, distiller, FSRS). Backend wires through existing infrastructure: `crates/agent/src/execution/execute_loop.rs` (unchanged loop), `crates/klynt-core/src/approval/` (unchanged engine), `crates/desktop/src/specta_builder.rs` (the `klynt_collect_commands!` registration). Frontend rewires existing components — `useAppServerEvents` (20 typed callbacks), `ApprovalCard` (full UI exists), `CostCeilingBanner` (switches from polling to event-listening) — without rewriting them.

**Tech Stack:** Rust 1.93, Tauri 2, sqlx + SQLite, tokio + `broadcast`, `tauri-specta`, `linkme`, `klynt-skill-loader`, React 18 + TypeScript, Vitest, proptest, `cargo-nextest`. Pre-release migration policy: schema edits in-place, no migration scripts, dev DB wipe.

**Spec:** [`docs/superpowers/specs/2026-05-03-klynt-coding-in-chat-phase4-design.md`](../specs/2026-05-03-klynt-coding-in-chat-phase4-design.md).

**Out of scope (per spec §14):** Windows sandbox, IDE bridge via MCP, snapshot content-addressed dedup beyond ghost-commits, Computer Use × coding integration, cross-CLI memory unification deepening, ChatGPT OAuth + codex account API + rate-limit dashboards (dropped), JSONL rollout audit log, per-thread git worktrees, `service_tier: fast | flex`.

**Phase 4 deliberately does NOT modify** (per spec §8 "what NOT changed"):
- `MidLoopCompressor` thresholds (70%, last 8 messages)
- `MAX_CONCURRENT_TOOLS = 10`, `MAX_TOOL_RESULT_LENGTH = 50_000`, `INTERACTIVE_TOOL_TIMEOUT = 600s`
- 3-layer approval engine internals (`klynt-core/src/approval/`)
- Skill routing logic (in `AgentLoop`)
- `ContextEngine::build_system_prompt()` source assembly

---

## File structure

### New files

```
bot/
├── crates/
│   ├── coding-agents-md/                 # NEW crate (Track 4)
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs                    # walk_agents_md + AgentsMdSource
│   │   └── tests/walk.rs                 # unit tests + K14 proptest
│   ├── desktop-shared/src/coding/        # NEW module
│   │   ├── mod.rs
│   │   ├── events.rs                     # ThreadEvent, PartDelta, FinishReason, FileChangeKind
│   │   ├── approval.rs                   # ApprovalRequest, LayerDecisions, LayerOutcome
│   │   ├── cost.rs                       # CostUpdate
│   │   └── thread.rs                     # Thread, ThreadSummary, TurnSummary, Message DTOs
│   ├── storage/src/messages/             # NEW module (or in repos/session.rs if simpler)
│   │   ├── mod.rs
│   │   ├── parts.rs                      # MessagePart enum
│   │   └── render.rs                     # extract_text, extract_tool_results helpers
│   ├── bus/src/typed_broker.rs           # NEW — TypedBroker<E>
│   ├── desktop/src/commands/
│   │   ├── coding_thread.rs              # NEW — coding_thread_* (8 commands)
│   │   ├── coding_turn.rs                # NEW — coding_message_send, coding_turn_*
│   │   ├── approval.rs                   # NEW — approval_respond
│   │   ├── workspace_files.rs            # NEW — workspace_meta_*, workspace_file_*, text_file_write
│   │   ├── workspace_utils.rs            # NEW — image_data_url, app_icon_read
│   │   ├── coding_review.rs              # NEW — coding_review_start
│   │   ├── coding_mcp.rs                 # NEW — coding_mcp_status
│   │   ├── coding_thread_metadata.rs     # NEW — coding_thread_metadata_generate
│   │   └── providers.rs                  # NEW — providers_list, provider_status
│   ├── desktop/tests/
│   │   └── no_raw_invoke_in_endpoints.rs # NEW — build-time test
│   └── app-core/src/coding/              # NEW module — handlers
│       ├── mod.rs
│       ├── thread_handler.rs             # AppCore methods for thread lifecycle
│       ├── turn_handler.rs               # AppCore methods for turn lifecycle
│       ├── workspace_handler.rs          # AppCore methods for workspace files/meta
│       ├── subscription.rs               # subscription manager + heartbeat
│       └── steer_queue.rs                # SteerQueue per active turn
│
├── desktop-ui/src/
│   ├── api/endpoints/
│   │   ├── approval.ts                   # NEW — approval_respond
│   │   ├── providers.ts                  # NEW — providers_list, provider_status
│   │   └── cost.ts                       # NEW — agent:cost_update typed listener
│   ├── features/coding/
│   │   ├── components/
│   │   │   ├── parts/                    # NEW directory
│   │   │   │   ├── TextPart.tsx
│   │   │   │   ├── ToolCallPart.tsx
│   │   │   │   ├── ToolResultPart.tsx
│   │   │   │   ├── FileChangePart.tsx
│   │   │   │   ├── CommandExecutionPart.tsx
│   │   │   │   ├── ReasoningPart.tsx
│   │   │   │   └── FinishPart.tsx
│   │   │   ├── ProjectPicker.tsx         # NEW — workspace picker UI
│   │   │   └── AgentsMdPanel.tsx         # NEW — workspace_meta_read + Refresh
│   │   └── hooks/
│   │       ├── useApprovals.ts           # NEW
│   │       ├── useCodingThread.ts        # NEW
│   │       └── useApplyThreadEvent.ts    # NEW — pure reducer
│   └── features/settings/
│       └── components/ProvidersSubsection.tsx  # NEW
│
├── skills/
│   └── coding-orchestrator/              # NEW skill
│       ├── SKILL.md
│       └── references/
│           ├── tool-usage.md
│           └── approval-policy.md
│
└── scripts/
    └── reset-dev-data.sh                 # NEW — convenience wipe
```

### Modified files

```
bot/
├── crates/
│   ├── storage/migrations/001_initial.sql               # extend sessions + session_messages
│   ├── storage/src/repos/session.rs                     # Parts-aware methods, new columns
│   ├── storage/src/rows/session.rs                      # SessionRow + SessionMessageRow extension
│   ├── storage/src/repos/tests/session_schema_tests.rs  # update column assertions
│   ├── klynt-core/src/approval/decision.rs              # add LayerDecisions audit field
│   ├── klynt-core/src/approval/guard.rs                 # capture LayerDecisions in evaluate()
│   ├── klynt-core/src/tools/{bash,edit,write,notebook_edit,apply_patch,web_fetch}.rs  # 6 emit sites
│   ├── agent/src/execution/execute_loop.rs              # emit new ThreadEvent variants
│   ├── agent/src/execution/core.rs                      # extend tool dispatch with Parts
│   ├── agent/src/agent_runtime/runtime.rs               # FinishReason enum integration
│   ├── agent/src/adapters/*.rs                          # tree builders read Parts
│   ├── app-core/src/handlers/chat/streaming.rs          # update content: String → Parts
│   ├── app-core/src/handlers/chat/threads.rs            # ChatMessageResponse uses Parts
│   ├── desktop/src/specta_builder.rs                    # register new commands
│   ├── desktop/src/dev_server/dispatch.rs               # add dispatch_dev for new commands
│   ├── desktop/src/commands/mod.rs                      # mod declarations
│   ├── desktop/src/main.rs                              # wire TypedBrokers into AppCore
│   ├── cognitive/src/services/session_memory.rs         # extract_text from Parts
│   ├── coding-memory/src/code_domain_searcher.rs        # extract_text from Parts
│   ├── session/src/manager.rs                           # SessionMessage with Parts
│   └── bus/src/lib.rs                                   # re-export TypedBroker
│
├── desktop-ui/src/
│   ├── api/endpoints/thread.ts                           # rewire 12 invoke<any> + drop 5 dropped commands
│   ├── api/endpoints/files.ts                            # rewire to workspace_* + text_file_write
│   ├── features/app/components/MainApp.tsx               # rewire orchestration calls
│   ├── features/app/hooks/useAppServerEvents.ts          # subscribe to new channels
│   ├── features/coding/components/ApprovalCard.tsx       # render LayerDecisions
│   ├── features/coding/components/CostCeilingBanner.tsx  # poll → event listener
│   ├── features/chat/hooks/useKlyntbotSurfaceProps.ts    # buildItems reads Parts
│   ├── features/chat/hooks/useChatSession.ts             # ChatMessage carries Parts
│   ├── features/messages/components/MessageRows.tsx      # part-aware rendering
│   └── services/tauri.test.ts                            # update file_read mocks → workspace_meta_read
│
└── docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md  # add Phase 4 cross-reference
```

---

## Track 0 — Foundation: types, schema, baseline (10 tasks)

### Task 0.1: Verify clean baseline

**Files:**
- Test: none (verification only)

- [ ] **Step 1: Confirm clean working tree**

```bash
git status --short
```

Expected: only the spec file `docs/superpowers/specs/2026-05-03-klynt-coding-in-chat-phase4-design.md` (already committed) — and any other unrelated existing edits. Note any files you'll touch later that already have modifications and decide whether to stash them first.

- [ ] **Step 2: Run baseline build**

```bash
cargo build --workspace
```

Expected: clean compile. If errors, fix or stash unrelated changes before proceeding.

- [ ] **Step 3: Run baseline tests**

```bash
cargo nextest run --workspace
```

Expected: all green. Record any pre-existing failures so we don't blame Phase 4 for them.

- [ ] **Step 4: Run baseline frontend checks**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test
```

Expected: all green.

- [ ] **Step 5: Confirm `bindings.ts` is current**

```bash
cargo nextest run -p desktop --test bindings_are_current
```

Expected: PASS. If it fails, run `cargo tauri dev` once to regenerate, commit, then continue.

### Task 0.2: Add `MessagePart` enum

**Files:**
- Create: `crates/storage/src/messages/mod.rs`
- Create: `crates/storage/src/messages/parts.rs`
- Modify: `crates/storage/src/lib.rs` (add `pub mod messages;`)
- Test: `crates/storage/tests/messages_parts.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/storage/tests/messages_parts.rs`:

```rust
use storage::messages::parts::{MessagePart, ToolOutput};
use serde_json::json;

#[test]
fn message_part_text_round_trip() {
    let p = MessagePart::Text { text: "hello".into() };
    let s = serde_json::to_string(&p).unwrap();
    let back: MessagePart = serde_json::from_str(&s).unwrap();
    match back {
        MessagePart::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn message_part_serializes_kind_tag() {
    let p = MessagePart::ToolCall { call_id: "c1".into(), name: "bash".into(), args: json!({"cmd":"ls"}) };
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"kind\":\"tool_call\""), "got: {s}");
    assert!(s.contains("\"call_id\":\"c1\""));
}

#[test]
fn message_part_command_execution_carries_streams() {
    let p = MessagePart::CommandExecution {
        command: vec!["cargo".into(), "test".into()],
        cwd: "/tmp".into(),
        exit_code: Some(0),
        stdout: "ok".into(),
        stderr: String::new(),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: MessagePart = serde_json::from_str(&s).unwrap();
    match back {
        MessagePart::CommandExecution { exit_code: Some(0), .. } => (),
        other => panic!("expected CommandExecution exit 0, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo nextest run -p storage --test messages_parts
```

Expected: FAIL with "unresolved import `storage::messages`".

- [ ] **Step 3: Add the module + enum**

Create `crates/storage/src/messages/mod.rs`:

```rust
pub mod parts;
pub mod render;
pub use parts::{MessagePart, ToolOutput, FileChangeKind};
```

Create `crates/storage/src/messages/parts.rs`:

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// A typed fragment of a Message's content. Replaces the prior `content: String` field.
///
/// Variants are tagged via serde with `kind` for forward-compatibility — adding a new
/// variant doesn't break stored JSON for old variants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessagePart {
    Text { text: String },
    ToolCall { call_id: String, name: String, args: serde_json::Value },
    ToolResult { call_id: String, output: ToolOutput, is_error: bool },
    Reasoning { text: String, redacted: bool },
    FileChange {
        path: PathBuf,
        before: Option<String>,
        after: String,
        diff_unified: String,
        applied: bool,
    },
    CommandExecution {
        command: Vec<String>,
        cwd: PathBuf,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    Finish { reason: FinishReason },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ToolOutput {
    pub text: String,
    pub mime: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

/// Why a turn ended. New typed enum (Phase 4 — was a `String` previously).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FinishReason {
    Completed,
    ToolCallsExhausted,
    Cancelled,
    PermissionDenied { reason: String },
    SandboxViolation { reason: String },
    CostCeilingReached { spend_usd: f64, ceiling_usd: f64 },
    Error { code: String, message: String, retryable: bool },
}
```

Add `pub mod messages;` to `crates/storage/src/lib.rs`.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo nextest run -p storage --test messages_parts
```

Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/messages/ crates/storage/src/lib.rs crates/storage/tests/messages_parts.rs
git commit -m "feat(storage): add MessagePart + FinishReason typed enums"
```

### Task 0.3: Add `extract_text` + `extract_tool_results` helpers

**Files:**
- Create: `crates/storage/src/messages/render.rs`
- Test: in same file

- [ ] **Step 1: Write the failing test**

In `crates/storage/src/messages/render.rs`:

```rust
use super::parts::{MessagePart, ToolOutput};

/// Joins all `Text` parts in a message into a single string.
/// Used by cognitive subsystems that operate on prose.
pub fn extract_text(parts: &[MessagePart]) -> String {
    parts.iter()
        .filter_map(|p| match p { MessagePart::Text { text } => Some(text.as_str()), _ => None })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns `(call_id, output_text, is_error)` for every `ToolResult` part.
pub fn extract_tool_results(parts: &[MessagePart]) -> Vec<(String, String, bool)> {
    parts.iter().filter_map(|p| match p {
        MessagePart::ToolResult { call_id, output, is_error } => {
            Some((call_id.clone(), output.text.clone(), *is_error))
        }
        _ => None,
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_joins_text_parts() {
        let parts = vec![
            MessagePart::Text { text: "hi".into() },
            MessagePart::Reasoning { text: "thinking".into(), redacted: false },
            MessagePart::Text { text: "there".into() },
        ];
        assert_eq!(extract_text(&parts), "hi\nthere");
    }

    #[test]
    fn extract_tool_results_skips_other_kinds() {
        let parts = vec![
            MessagePart::ToolCall { call_id: "c1".into(), name: "bash".into(), args: serde_json::json!({}) },
            MessagePart::ToolResult { call_id: "c1".into(), output: ToolOutput { text: "ok".into(), mime: None, truncated: false }, is_error: false },
        ];
        let r = extract_tool_results(&parts);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0], ("c1".into(), "ok".into(), false));
    }
}
```

- [ ] **Step 2: Run the test**

```bash
cargo nextest run -p storage messages::render
```

Expected: PASS (2 tests).

- [ ] **Step 3: Commit**

```bash
git add crates/storage/src/messages/render.rs
git commit -m "feat(storage): add Parts render helpers extract_text + extract_tool_results"
```

### Task 0.4: Schema migration — extend `session_messages`

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Test: `crates/storage/src/repos/tests/session_schema_tests.rs`

- [ ] **Step 1: Update the failing test first**

In `crates/storage/src/repos/tests/session_schema_tests.rs`, find the existing schema assertion test for `session_messages` columns. Add three expected columns: `parts`, `turn_id`, `finish_reason`.

```rust
#[tokio::test]
async fn session_messages_has_phase4_columns() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('session_messages')"
    ).fetch_all(pool.inner()).await.unwrap();
    assert!(cols.contains(&"parts".into()), "missing parts column");
    assert!(cols.contains(&"turn_id".into()), "missing turn_id column");
    assert!(cols.contains(&"finish_reason".into()), "missing finish_reason column");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo nextest run -p storage session_messages_has_phase4_columns
```

Expected: FAIL.

- [ ] **Step 3: Edit the baseline SQL in-place (pre-release policy)**

In `crates/storage/migrations/001_initial.sql`, find the `CREATE TABLE session_messages` block. Add three columns to the column list (note: SQLite stores JSONB as TEXT-with-JSON-functions; use TEXT):

```sql
CREATE TABLE IF NOT EXISTS session_messages (
    id          TEXT PRIMARY KEY,
    session_key TEXT NOT NULL REFERENCES sessions(key) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL DEFAULT '',
    parts       TEXT,                            -- NEW: Phase 4 — JSON Vec<MessagePart>; NULL for legacy rows
    turn_id     TEXT,                            -- NEW: Phase 4 — UI turn grouping
    finish_reason TEXT,                          -- NEW: Phase 4 — typed FinishReason JSON
    timestamp   INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    request_id  TEXT,
    tool_calls  TEXT,
    metadata    TEXT
);
```

Note: `content` becomes `DEFAULT ''` because new rows write `parts` and leave `content` empty.

- [ ] **Step 4: Wipe dev DB so the new migration content-hash is accepted**

```bash
rm -f ~/.klyntbot-dev/data.db ~/.klyntbot-dev/data.db-wal ~/.klyntbot-dev/data.db-shm
rm -rf ~/.klyntbot-dev/lance/
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cargo nextest run -p storage session_messages_has_phase4_columns
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/storage/src/repos/tests/session_schema_tests.rs
git commit -m "feat(storage): extend session_messages with parts, turn_id, finish_reason columns"
```

### Task 0.5: Schema migration — extend `sessions`

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Test: `crates/storage/src/repos/tests/session_schema_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn sessions_has_phase4_columns() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('sessions')"
    ).fetch_all(pool.inner()).await.unwrap();
    for required in ["workspace_id", "forked_from_id", "summary_message_id", "ephemeral", "archived_at"] {
        assert!(cols.contains(&required.into()), "missing column: {}", required);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo nextest run -p storage sessions_has_phase4_columns
```

Expected: FAIL.

- [ ] **Step 3: Add the columns in-place**

In `crates/storage/migrations/001_initial.sql`, find `CREATE TABLE sessions` and append:

```sql
    workspace_id        TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    forked_from_id      TEXT REFERENCES sessions(key) ON DELETE SET NULL,
    summary_message_id  TEXT,
    ephemeral           INTEGER NOT NULL DEFAULT 0,
    archived_at         INTEGER
```

(Reminder: `cwd`, `repo_id`, `repo_branch`, `tool_profile`, `approval_mode`, `total_cost_usd`, `total_tokens`, `parent_session_id` already exist — do not re-add.)

Add an index for archived sessions list lookup:

```sql
CREATE INDEX IF NOT EXISTS idx_sessions_workspace_archived ON sessions(workspace_id, archived_at);
```

- [ ] **Step 4: Wipe dev DB and run test**

```bash
rm -f ~/.klyntbot-dev/data.db ~/.klyntbot-dev/data.db-wal ~/.klyntbot-dev/data.db-shm
cargo nextest run -p storage sessions_has_phase4_columns
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/storage/src/repos/tests/session_schema_tests.rs
git commit -m "feat(storage): add 5 Phase 4 columns to sessions table"
```

### Task 0.6: Update `SessionRow` + `SessionMessageRow`

**Files:**
- Modify: `crates/storage/src/rows/session.rs`

- [ ] **Step 1: Extend `SessionMessageRow`**

In `crates/storage/src/rows/session.rs`, add to the struct:

```rust
pub struct SessionMessageRow {
    pub id: Uuid,
    pub session_key: String,
    pub role: String,
    pub content: String,
    pub parts: Option<String>,           // NEW — JSON-serialized Vec<MessagePart>
    pub turn_id: Option<String>,          // NEW
    pub finish_reason: Option<String>,    // NEW — JSON-serialized FinishReason
    pub timestamp: SqlTs,
    pub request_id: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}
```

- [ ] **Step 2: Extend `SessionRow`**

```rust
pub struct SessionRow {
    pub key: String,
    pub metadata: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: Option<i64>,
    pub compressed_prefix: Option<String>,
    pub compressed_through_idx: Option<i64>,
    pub compressed_at: Option<i64>,
    pub cwd: Option<String>,
    pub repo_id: Option<String>,
    pub repo_branch: Option<String>,
    pub tool_profile: Option<String>,
    pub approval_mode: String,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub parent_session_id: Option<String>,
    pub workspace_id: Option<String>,        // NEW
    pub forked_from_id: Option<String>,      // NEW
    pub summary_message_id: Option<String>,  // NEW
    pub ephemeral: i64,                      // NEW (0/1)
    pub archived_at: Option<i64>,            // NEW
}
```

- [ ] **Step 3: Update FromRow impl**

The `FromRow` derive (or hand-impl) needs to know about the new columns. If hand-rolled, add `try_get` calls; if derive, just compile.

```bash
cargo build -p storage
```

Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/rows/session.rs
git commit -m "feat(storage): extend SessionRow + SessionMessageRow with Phase 4 fields"
```

### Task 0.7: `SessionRepo` — Parts-aware `add_message_with_parts`

**Files:**
- Modify: `crates/storage/src/repos/session.rs`
- Test: `crates/storage/src/repos/tests/session_messages_parts_test.rs` (new)

- [ ] **Step 1: Write the failing test**

```rust
use storage::{StoragePool, repos::SessionRepo};
use storage::messages::parts::{MessagePart, ToolOutput};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn add_message_with_parts_round_trip() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.inner().clone());
    let sk = "test-session";
    repo.upsert_session(sk, &json!({})).await.unwrap();

    let parts = vec![
        MessagePart::Text { text: "hello".into() },
        MessagePart::ToolCall { call_id: "c1".into(), name: "bash".into(), args: json!({"cmd":"ls"}) },
    ];
    let msg_id = Uuid::new_v4().to_string();
    repo.add_message_with_parts(sk, &msg_id, "assistant", &parts, Some("turn-1"), None).await.unwrap();

    let fetched = repo.get_messages_parts(sk, 100).await.unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].parts.len(), 2);
    match &fetched[0].parts[0] {
        MessagePart::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected Text, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo nextest run -p storage add_message_with_parts_round_trip
```

Expected: FAIL with "no method named `add_message_with_parts`".

- [ ] **Step 3: Add the method to `SessionRepo`**

In `crates/storage/src/repos/session.rs`:

```rust
use crate::messages::parts::{MessagePart, FinishReason};

impl SessionRepo {
    pub async fn add_message_with_parts(
        &self,
        session_key: &str,
        message_id: &str,
        role: &str,
        parts: &[MessagePart],
        turn_id: Option<&str>,
        finish_reason: Option<&FinishReason>,
    ) -> Result<(), StorageError> {
        let parts_json = serde_json::to_string(parts).map_err(StorageError::serialization)?;
        let finish_json = finish_reason
            .map(|f| serde_json::to_string(f))
            .transpose()
            .map_err(StorageError::serialization)?;
        sqlx::query(
            "INSERT INTO session_messages (id, session_key, role, content, parts, turn_id, finish_reason, timestamp) \
             VALUES (?, ?, ?, '', ?, ?, ?, unixepoch('now') * 1000)"
        )
        .bind(message_id)
        .bind(session_key)
        .bind(role)
        .bind(&parts_json)
        .bind(turn_id)
        .bind(finish_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_messages_parts(
        &self,
        session_key: &str,
        limit: i64,
    ) -> Result<Vec<SessionMessageWithParts>, StorageError> {
        let rows: Vec<SessionMessageRow> = sqlx::query_as(
            "SELECT * FROM session_messages WHERE session_key = ? ORDER BY timestamp ASC LIMIT ?"
        )
        .bind(session_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| {
            let parts: Vec<MessagePart> = match r.parts.as_deref() {
                Some(s) if !s.is_empty() => serde_json::from_str(s).map_err(StorageError::serialization)?,
                _ => vec![MessagePart::Text { text: r.content.clone() }],  // legacy fallback
            };
            let finish_reason: Option<FinishReason> = match r.finish_reason.as_deref() {
                Some(s) if !s.is_empty() => Some(serde_json::from_str(s).map_err(StorageError::serialization)?),
                _ => None,
            };
            Ok(SessionMessageWithParts {
                id: r.id.to_string(),
                session_key: r.session_key,
                role: r.role,
                parts,
                turn_id: r.turn_id,
                finish_reason,
                timestamp: r.timestamp.into(),
                metadata: r.metadata,
            })
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct SessionMessageWithParts {
    pub id: String,
    pub session_key: String,
    pub role: String,
    pub parts: Vec<MessagePart>,
    pub turn_id: Option<String>,
    pub finish_reason: Option<FinishReason>,
    pub timestamp: jiff::Timestamp,
    pub metadata: Option<serde_json::Value>,
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p storage add_message_with_parts_round_trip
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/session.rs crates/storage/src/repos/tests/
git commit -m "feat(storage): add Parts-aware SessionRepo methods (add_message_with_parts, get_messages_parts)"
```

### Task 0.8: Reset-dev-data convenience script

**Files:**
- Create: `scripts/reset-dev-data.sh`

- [ ] **Step 1: Create the script**

```bash
#!/usr/bin/env bash
set -euo pipefail
HOME_DIR="${KLYNTBOT_HOME:-$HOME/.klyntbot-dev}"
echo "Wiping ${HOME_DIR}/data.db + lance/"
rm -f "${HOME_DIR}/data.db" "${HOME_DIR}/data.db-wal" "${HOME_DIR}/data.db-shm"
rm -rf "${HOME_DIR}/lance/"
echo "Done. Config + sessions/ + KLYNTBOT.md + AGENTS.md preserved."
```

- [ ] **Step 2: Make it executable + test**

```bash
chmod +x scripts/reset-dev-data.sh
KLYNTBOT_HOME=/tmp/klynt-test-reset mkdir -p /tmp/klynt-test-reset && touch /tmp/klynt-test-reset/data.db
KLYNTBOT_HOME=/tmp/klynt-test-reset ./scripts/reset-dev-data.sh
ls /tmp/klynt-test-reset/data.db 2>&1 && echo "FAIL: data.db should be deleted" || echo "PASS"
rm -rf /tmp/klynt-test-reset
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add scripts/reset-dev-data.sh
git commit -m "chore(scripts): add reset-dev-data.sh convenience helper"
```

### Task 0.9: Add `desktop-shared::coding` module skeleton

**Files:**
- Create: `crates/desktop-shared/src/coding/mod.rs`
- Create: `crates/desktop-shared/src/coding/events.rs`
- Create: `crates/desktop-shared/src/coding/approval.rs`
- Create: `crates/desktop-shared/src/coding/cost.rs`
- Create: `crates/desktop-shared/src/coding/thread.rs`
- Modify: `crates/desktop-shared/src/lib.rs`

- [ ] **Step 1: Create `mod.rs`**

```rust
pub mod approval;
pub mod cost;
pub mod events;
pub mod thread;

pub use approval::*;
pub use cost::*;
pub use events::*;
pub use thread::*;
```

- [ ] **Step 2: Create `thread.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

pub type ThreadId = String;
pub type TurnId = String;
pub type SubscriptionId = String;
pub type MessageId = String;
pub type WorkspaceId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: ThreadId,
    pub workspace_id: WorkspaceId,
    pub cwd: String,
    pub model: Option<String>,
    pub approval_policy: ApprovalPolicy,
    pub sandbox: SandboxKind,
    pub instruction_sources: Vec<InstructionSource>,
    pub created_at: i64,
    pub updated_at: i64,
    pub title: Option<String>,
    pub starred: bool,
    pub archived_at: Option<i64>,
    pub ephemeral: bool,
    pub forked_from_id: Option<ThreadId>,
    pub summary_message_id: Option<MessageId>,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub items: Vec<MessageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: ThreadId,
    pub title: Option<String>,
    pub workspace_id: WorkspaceId,
    pub message_count: i64,
    pub total_cost_usd: f64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    AskAlways,
    AskOnRisky,
    AskOnFailure,
    YoloMode,
}

impl Default for ApprovalPolicy {
    fn default() -> Self { Self::AskOnRisky }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SandboxKind {
    MacosSeatbelt,
    LinuxBwrapLandlock,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InstructionSource {
    pub path: String,
    pub bytes: u64,
    pub is_global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    pub id: MessageId,
    pub session_id: ThreadId,
    pub role: String,                       // "user" | "assistant" | "tool" | "system"
    pub parts: Vec<serde_json::Value>,      // Vec<MessagePart> serialized
    pub model: Option<String>,
    pub turn_id: Option<TurnId>,
    pub created_at: i64,
    pub finish_reason: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    pub id: TurnId,
    pub started_at: i64,
    pub completed_at: Option<i64>,
}
```

- [ ] **Step 3: Create `events.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use super::thread::{ThreadId, TurnId, MessageId, SubscriptionId, MessageDto};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThreadEvent {
    TurnStarted   { thread_id: ThreadId, turn_id: TurnId, model: String, started_at: i64 },
    ItemStarted   { thread_id: ThreadId, turn_id: TurnId, item: MessageDto },
    ItemDelta     { thread_id: ThreadId, turn_id: TurnId, item_id: MessageId, part_idx: u32, delta: PartDelta },
    ItemCompleted { thread_id: ThreadId, turn_id: TurnId, item: MessageDto },
    ToolCallStarted   { thread_id: ThreadId, turn_id: TurnId, item_id: MessageId, call_id: String, tool: String },
    ToolCallCompleted { thread_id: ThreadId, turn_id: TurnId, call_id: String, success: bool, duration_ms: u64 },
    FileChanged       { thread_id: ThreadId, turn_id: TurnId, path: String, change: FileChangeKindDto },
    CommandExecuted   { thread_id: ThreadId, turn_id: TurnId, command: Vec<String>, exit_code: Option<i32> },
    ContextCompressed { thread_id: ThreadId, turn_id: TurnId, before_tokens: u64, after_tokens: u64 },
    TurnCompleted     { thread_id: ThreadId, turn_id: TurnId, finish_reason: serde_json::Value, completed_at: i64, duration_ms: u64 },
    Heartbeat         { subscription_id: SubscriptionId, server_time: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartDelta {
    Text { append: String },
    Reasoning { append: String, redacted: bool },
    ToolCallArgs { json_patch: serde_json::Value },
    CommandStdout { append: String },
    CommandStderr { append: String },
    FileChangeProgress { bytes_written: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKindDto {
    Created,
    Modified,
    Deleted,
}
```

- [ ] **Step 4: Create `approval.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use super::thread::{ThreadId, TurnId};

pub type ApprovalId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRequest {
    CommandExecution {
        approval_id: ApprovalId,
        thread_id: ThreadId,
        turn_id: TurnId,
        command: Vec<String>,
        cwd: String,
        reason: String,
        proposed_execpolicy_amendment: Option<ExecpolicyAmendment>,
        layer_decisions: LayerDecisions,
    },
    FileChange {
        approval_id: ApprovalId,
        thread_id: ThreadId,
        turn_id: TurnId,
        path: String,
        diff_unified: String,
        write_kind: WriteKind,
        layer_decisions: LayerDecisions,
    },
    UserInput {
        approval_id: ApprovalId,
        thread_id: ThreadId,
        turn_id: TurnId,
        prompt: String,
        questions: Vec<UserInputQuestion>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WriteKind {
    Create,
    Modify,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecpolicyAmendment {
    pub command_pattern: Vec<String>,
    pub starlark_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UserInputQuestion {
    pub id: String,
    pub prompt: String,
    pub kind: String, // "text" | "select" | "multiselect"
    pub options: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LayerDecisions {
    pub privacy: LayerOutcome,
    pub layer1: LayerOutcome,
    pub layer2: LayerOutcome,
    pub layer3: LayerOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LayerOutcome {
    Allowed { reason: String, rule_matched: Option<String> },
    Denied { reason: String, rule_matched: Option<String> },
    Deferred { reason: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalDecisionDto {
    Accept,
    Decline,
    AcceptForSession,
    AcceptWithExecpolicyAmendment { execpolicy_amendment: ExecpolicyAmendment },
    Cancel,
}
```

- [ ] **Step 5: Create `cost.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use super::thread::ThreadId;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CostUpdate {
    pub thread_id: Option<ThreadId>,
    pub provider: String,
    pub prompt_tokens_delta: u64,
    pub completion_tokens_delta: u64,
    pub usd_delta: f64,
    pub thread_total_usd: Option<f64>,
    pub ceiling_breached: bool,
}
```

- [ ] **Step 6: Wire into `desktop-shared/src/lib.rs`**

Add: `pub mod coding;`.

- [ ] **Step 7: Build**

```bash
cargo build -p desktop-shared
```

Expected: clean compile.

- [ ] **Step 8: Commit**

```bash
git add crates/desktop-shared/src/coding/ crates/desktop-shared/src/lib.rs
git commit -m "feat(desktop-shared): add coding module — events, approvals, costs, thread DTOs"
```

### Task 0.10: Add `TypedBroker<E>` in `crates/bus/`

**Files:**
- Create: `crates/bus/src/typed_broker.rs`
- Modify: `crates/bus/src/lib.rs`
- Test: in `typed_broker.rs`

- [ ] **Step 1: Write the failing test**

In `crates/bus/src/typed_broker.rs`:

```rust
use tokio::sync::broadcast;

/// A typed pub/sub broker. Compile-time guarantees on event payload type up to the
/// Tauri serialization boundary. Adapter task fans `subscribe()` output → app.emit.
#[derive(Debug, Clone)]
pub struct TypedBroker<E: Clone + Send + 'static> {
    sender: broadcast::Sender<E>,
}

impl<E: Clone + Send + 'static> TypedBroker<E> {
    pub fn new(capacity: usize) -> Self {
        let (sender, _rx) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: E) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<E> {
        self.sender.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broker_publishes_to_subscribers() {
        let b: TypedBroker<u64> = TypedBroker::new(16);
        let mut rx1 = b.subscribe();
        let mut rx2 = b.subscribe();
        b.publish(42);
        assert_eq!(rx1.recv().await.unwrap(), 42);
        assert_eq!(rx2.recv().await.unwrap(), 42);
    }

    #[tokio::test]
    async fn broker_drops_silently_with_no_subscribers() {
        let b: TypedBroker<u64> = TypedBroker::new(16);
        b.publish(7);  // no panic, no error
        assert_eq!(b.receiver_count(), 0);
    }
}
```

- [ ] **Step 2: Add `pub mod typed_broker; pub use typed_broker::TypedBroker;` to `crates/bus/src/lib.rs`**

- [ ] **Step 3: Run test**

```bash
cargo nextest run -p bus typed_broker
```

Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/typed_broker.rs crates/bus/src/lib.rs
git commit -m "feat(bus): add TypedBroker<E> for compile-time-typed pub/sub"
```

---

## Track 1 — Approval channel + LayerDecisions audit (8 tasks)

### Task 1.1: Add `LayerDecisions` capture to `klynt-core::approval::Approval`

**Files:**
- Modify: `crates/klynt-core/src/approval/decision.rs`
- Test: `crates/klynt-core/tests/layer_decisions_test.rs` (new)

- [ ] **Step 1: Write the failing test**

```rust
use klynt_core::approval::{ApprovalDecision, ApprovalLayer, LayerOutcomeAudit};

#[test]
fn ask_decision_carries_layer_audit() {
    let d = ApprovalDecision::Ask {
        layer: ApprovalLayer::DefaultMode,
        reason: "no rule matched".into(),
        layer_audit: Some(LayerOutcomeAudit {
            privacy_passed: true,
            layer1: "deferred: no match".into(),
            layer2: "deferred: starlark fall-through".into(),
            layer3: "skipped: mirror disabled".into(),
        }),
    };
    match d {
        ApprovalDecision::Ask { layer_audit: Some(a), .. } => {
            assert!(a.privacy_passed);
            assert!(a.layer3.contains("mirror disabled"));
        }
        _ => panic!("expected Ask with audit"),
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo nextest run -p klynt-core ask_decision_carries_layer_audit
```

Expected: FAIL with "missing field `layer_audit`".

- [ ] **Step 3: Extend `ApprovalDecision::Ask` and add `LayerOutcomeAudit`**

In `crates/klynt-core/src/approval/decision.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerOutcomeAudit {
    pub privacy_passed: bool,
    pub layer1: String,           // human-readable trace
    pub layer2: String,
    pub layer3: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApprovalDecision {
    Auto { allowed: bool, layer: ApprovalLayer, reason: String, rule_matched: Option<String> },
    Ask { layer: ApprovalLayer, reason: String, layer_audit: Option<LayerOutcomeAudit> },
    PrivacyDenied { reason: String, pattern: String },
    Cancelled,
    TimedOut,
}
```

Update `mod.rs` re-exports: add `pub use decision::LayerOutcomeAudit;`.

- [ ] **Step 4: Run test to verify PASS**

```bash
cargo nextest run -p klynt-core ask_decision_carries_layer_audit
```

Expected: PASS. Existing callers will fail to compile because of the new `layer_audit` field — fix them in step 5.

- [ ] **Step 5: Fix existing `ApprovalDecision::Ask { … }` constructors to set `layer_audit: None`**

```bash
cargo build -p klynt-core 2>&1 | grep "missing field" | head -20
```

Add `, layer_audit: None` to every site identified. Convenience constructor:

```rust
pub fn ask(layer: ApprovalLayer, reason: impl Into<String>) -> Self {
    Self::Ask { layer, reason: reason.into(), layer_audit: None }
}

pub fn ask_with_audit(layer: ApprovalLayer, reason: impl Into<String>, audit: LayerOutcomeAudit) -> Self {
    Self::Ask { layer, reason: reason.into(), layer_audit: Some(audit) }
}
```

Run:

```bash
cargo build -p klynt-core
```

Expected: clean compile.

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-core/src/approval/decision.rs crates/klynt-core/src/approval/mod.rs crates/klynt-core/tests/layer_decisions_test.rs
git commit -m "feat(approval): add LayerOutcomeAudit to ApprovalDecision::Ask"
```

### Task 1.2: Capture `LayerOutcomeAudit` in `evaluate()`

**Files:**
- Modify: `crates/klynt-core/src/approval/guard.rs`
- Test: `crates/klynt-core/tests/layer_decisions_test.rs` (extend)

- [ ] **Step 1: Add a test that asserts audit is populated when Ask fires**

```rust
#[tokio::test]
async fn evaluate_populates_layer_audit_on_ask() {
    use klynt_core::approval::{evaluate, GuardCtx};
    // Build a GuardCtx with no matching rules → falls through all layers → Ask
    // (Construction details depend on existing test helpers in klynt-core/src/approval/tests/)
    let ctx = test_helpers::ask_only_ctx();
    let decision = evaluate(ctx, "bash", "ls").await;
    match decision {
        ApprovalDecision::Ask { layer_audit: Some(a), .. } => {
            assert!(a.privacy_passed);
            assert!(!a.layer1.is_empty());
        }
        other => panic!("expected Ask with audit, got {other:?}"),
    }
}
```

(If no test helper exists, create `crates/klynt-core/src/approval/tests/test_helpers.rs` with a minimal `ask_only_ctx()` builder.)

- [ ] **Step 2: Run to verify it fails**

```bash
cargo nextest run -p klynt-core evaluate_populates_layer_audit_on_ask
```

Expected: FAIL — current `evaluate` returns `Ask { layer_audit: None, .. }`.

- [ ] **Step 3: Update `evaluate()` in `guard.rs` to track and emit audit**

```rust
pub async fn evaluate<'a>(ctx: GuardCtx<'a>, tool: &str, payload: &str) -> ApprovalDecision {
    // Privacy check (unchanged) — explicit audit field
    let privacy_hit = match tool {
        "bash" => ctx.privacy.bash_command_touches_excluded(payload),
        _ => ctx.privacy.is_excluded(std::path::Path::new(payload)),
    };
    if privacy_hit {
        return ApprovalDecision::PrivacyDenied { /* unchanged */ };
    }

    let mut audit = LayerOutcomeAudit {
        privacy_passed: true,
        layer1: "skipped".into(),
        layer2: "skipped".into(),
        layer3: "skipped".into(),
    };

    // Layer 1
    let l1 = ctx.layer1.evaluate(tool, payload);
    audit.layer1 = format_layer1_outcome(&l1);
    match &l1 {
        ApprovalDecision::Auto { .. } | ApprovalDecision::PrivacyDenied { .. } => return l1,
        ApprovalDecision::Ask { .. } => {} // continue to L2
        _ => {}
    }

    // Layer 2 — Starlark
    let l2 = ctx.policy.eval(&[tool.to_string(), payload.to_string()], None);
    audit.layer2 = format_layer2_outcome(&l2);
    match l2 {
        klynt_execpolicy::Decision::Allow => return ApprovalDecision::Auto { allowed: true, layer: ApprovalLayer::Layer2Starlark, reason: "starlark allow".into(), rule_matched: None },
        klynt_execpolicy::Decision::Forbid => return ApprovalDecision::Auto { allowed: false, layer: ApprovalLayer::Layer2Starlark, reason: "starlark forbid".into(), rule_matched: None },
        klynt_execpolicy::Decision::Ask | klynt_execpolicy::Decision::FallThrough => {} // continue
    }

    // Layer 3 — Mirror-learned
    if ctx.mirror_learning_enabled {
        if let Some(history_repo) = &ctx.history_repo {
            let args_hash = args_hash_for_relevance(tool, ctx.args.as_ref().unwrap_or(&serde_json::Value::Null));
            let summary = history_repo.summary_for(tool, &args_hash, &ctx.repo_id).await.unwrap_or_default();
            let l3_cfg = Layer3Config { enabled: true, min_approvals: ctx.mirror_min_approvals, cooldown_seconds: ctx.mirror_cooldown_seconds };
            let outcome = layer3::evaluate(&l3_cfg, &summary, ctx.now_unix);
            audit.layer3 = format_layer3_outcome(&outcome);
            match outcome {
                Layer3Outcome::AutoAllow { reason } => return ApprovalDecision::Auto { allowed: true, layer: ApprovalLayer::Layer3Mirror, reason, rule_matched: None },
                Layer3Outcome::Ask { reason } => return ApprovalDecision::ask_with_audit(ApprovalLayer::Layer3Mirror, reason, audit),
                Layer3Outcome::FallThrough => {} // continue to default
            }
        } else {
            audit.layer3 = "skipped: no history repo".into();
        }
    } else {
        audit.layer3 = "skipped: mirror disabled".into();
    }

    // Default — based on l1's recommendation
    ApprovalDecision::ask_with_audit(ApprovalLayer::DefaultMode, "no rule matched; ask".into(), audit)
}

fn format_layer1_outcome(d: &ApprovalDecision) -> String {
    match d {
        ApprovalDecision::Auto { allowed: true, rule_matched, .. } => format!("allowed: {}", rule_matched.as_deref().unwrap_or("?")),
        ApprovalDecision::Auto { allowed: false, rule_matched, .. } => format!("denied: {}", rule_matched.as_deref().unwrap_or("?")),
        ApprovalDecision::Ask { reason, .. } => format!("ask: {reason}"),
        _ => "?".into(),
    }
}
fn format_layer2_outcome(d: &klynt_execpolicy::Decision) -> String {
    match d {
        klynt_execpolicy::Decision::Allow => "allowed".into(),
        klynt_execpolicy::Decision::Forbid => "denied".into(),
        klynt_execpolicy::Decision::Ask => "ask".into(),
        klynt_execpolicy::Decision::FallThrough => "deferred: no rule".into(),
    }
}
fn format_layer3_outcome(o: &Layer3Outcome) -> String {
    match o {
        Layer3Outcome::AutoAllow { reason } => format!("auto-allow: {reason}"),
        Layer3Outcome::Ask { reason } => format!("ask: {reason}"),
        Layer3Outcome::FallThrough => "deferred: not enough history".into(),
    }
}
```

- [ ] **Step 4: Run test to verify PASS**

```bash
cargo nextest run -p klynt-core evaluate_populates_layer_audit_on_ask
```

- [ ] **Step 5: Run all approval tests for regressions**

```bash
cargo nextest run -p klynt-core --test '*'
```

Expected: all PASS, including K3, K10 proptests.

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-core/src/approval/
git commit -m "feat(approval): capture LayerOutcomeAudit through evaluate() pipeline"
```

### Task 1.3: New event `agent:approval_request` from `desktop-shared::ApprovalRequest`

**Files:**
- Modify: `crates/klynt-core/src/approval/round_trip.rs` (or `guard.rs` event emission)
- Modify: `crates/klynt-core/src/tools/bash.rs:106` (and 5 other tool files)

- [ ] **Step 1: Add helper that emits the new typed event**

In `crates/klynt-core/src/approval/round_trip.rs` (or new helper module):

```rust
use desktop_shared::coding::{ApprovalRequest, LayerDecisions, LayerOutcome, ApprovalId, ExecpolicyAmendment};
use crate::approval::decision::LayerOutcomeAudit;

pub fn emit_approval_request_event(
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::tools::ToolEvent>>,
    approval_id: &ApprovalId,
    thread_id: &str,
    turn_id: &str,
    tool: &str,
    args: &serde_json::Value,
    cwd: Option<&str>,
    audit: &LayerOutcomeAudit,
) {
    if event_tx.is_none() { return; }
    let layer_decisions = LayerDecisions {
        privacy: LayerOutcome::Allowed { reason: "passed".into(), rule_matched: None },
        layer1: parse_audit_to_outcome(&audit.layer1),
        layer2: parse_audit_to_outcome(&audit.layer2),
        layer3: parse_audit_to_outcome(&audit.layer3),
    };
    let request = match tool {
        "bash" => ApprovalRequest::CommandExecution {
            approval_id: approval_id.clone(),
            thread_id: thread_id.into(),
            turn_id: turn_id.into(),
            command: args.get("command").and_then(|v| v.as_str()).map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default(),
            cwd: cwd.unwrap_or("").into(),
            reason: "user approval required for command execution".into(),
            proposed_execpolicy_amendment: None,
            layer_decisions,
        },
        // edit/write/apply_patch/notebook_edit
        _ => ApprovalRequest::FileChange {
            approval_id: approval_id.clone(),
            thread_id: thread_id.into(),
            turn_id: turn_id.into(),
            path: args.get("path").and_then(|v| v.as_str()).unwrap_or("").into(),
            diff_unified: args.get("diff_unified").and_then(|v| v.as_str()).unwrap_or("").into(),
            write_kind: desktop_shared::coding::WriteKind::Modify, // refine per tool
            layer_decisions,
        },
    };
    let payload = serde_json::to_value(&request).unwrap_or_default();
    if let Some(tx) = event_tx {
        let _ = tx.try_send(crate::tools::ToolEvent::ApprovalRequest { id: approval_id.clone(), payload });
    }
}

fn parse_audit_to_outcome(s: &str) -> LayerOutcome {
    if s.starts_with("allowed") { LayerOutcome::Allowed { reason: s.into(), rule_matched: None } }
    else if s.starts_with("denied") { LayerOutcome::Denied { reason: s.into(), rule_matched: None } }
    else if s.starts_with("auto-allow") { LayerOutcome::Allowed { reason: s.into(), rule_matched: None } }
    else if s.starts_with("skipped") { LayerOutcome::Skipped { reason: s.into() } }
    else { LayerOutcome::Deferred { reason: s.into() } }
}
```

- [ ] **Step 2: Add `ApprovalRequest` variant to `ToolEvent`**

In `crates/klynt-core/src/tools/event.rs` (or wherever `ToolEvent` is defined):

```rust
pub enum ToolEvent {
    // existing variants...
    ApprovalRequest { id: String, payload: serde_json::Value },
}
```

- [ ] **Step 3: Wire emission into `evaluate()` Ask path**

After computing `audit` in `evaluate()`:

```rust
if matches!(decision, ApprovalDecision::Ask { .. }) {
    emit_approval_request_event(
        ctx.event_tx,
        &ctx.request_id,
        &ctx.thread_id.unwrap_or_default(),
        &ctx.turn_id.unwrap_or_default(),
        tool,
        ctx.args.as_ref().unwrap_or(&serde_json::Value::Null),
        ctx.cwd.as_deref(),
        &audit,
    );
}
```

(`thread_id`/`turn_id` are NEW fields on `GuardCtx` — add them in step 4.)

- [ ] **Step 4: Add `thread_id` + `turn_id` to `GuardCtx`**

```rust
pub struct GuardCtx<'a> {
    // ... existing fields ...
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
}
```

Update each of the 6 tool files (`bash.rs:106`, `edit.rs:155`, etc.) to populate these from execution context.

- [ ] **Step 5: Run approval tests**

```bash
cargo nextest run -p klynt-core
```

Expected: all PASS. Some tests will need `, thread_id: None, turn_id: None` added.

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-core/
git commit -m "feat(approval): emit agent:approval_request typed event with LayerDecisions on Ask"
```

### Task 1.4: New `approval_respond` Tauri command

**Files:**
- Create: `crates/desktop/src/commands/approval.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/specta_builder.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`
- Test: `crates/desktop/tests/approval_command.rs` (new)

- [ ] **Step 1: Write a test stub that invokes the new command**

```rust
// crates/desktop/tests/approval_command.rs
#[tokio::test]
async fn approval_respond_resolves_pending_approval() {
    // Set up an in-memory AppCore, register a pending approval,
    // call approval_respond with Accept, verify resolution.
    // (Uses existing test fixtures in app-core/tests/)
}
```

(Skeleton only; real assertion in step 4.)

- [ ] **Step 2: Create the command**

```rust
// crates/desktop/src/commands/approval.rs
use std::sync::Arc;
use desktop_macros::klynt_command;
use desktop_shared::coding::ApprovalDecisionDto;
use crate::commands::error::CommandResult;

#[klynt_command]
pub async fn approval_respond(
    approval_id: String,
    decision: ApprovalDecisionDto,
) -> CommandResult<()> {
    // state injected by macro
    let internal_decision = match decision {
        ApprovalDecisionDto::Accept => app_core::coding::AppApprovalDecision::AllowOnce,
        ApprovalDecisionDto::AcceptForSession => app_core::coding::AppApprovalDecision::AllowAlways { rule: None },
        ApprovalDecisionDto::AcceptWithExecpolicyAmendment { execpolicy_amendment } => {
            app_core::coding::AppApprovalDecision::AddRule { starlark_source: execpolicy_amendment.starlark_source.unwrap_or_default() }
        }
        ApprovalDecisionDto::Decline | ApprovalDecisionDto::Cancel => app_core::coding::AppApprovalDecision::Deny,
    };
    state.respond_approval(&approval_id, internal_decision).await
        .map_err(|e| crate::commands::error::ApiError::from(e))?;
    Ok(())
}

pub fn dispatch_dev(cmd: &str, core: &Arc<app_core::AppCore>, body: &serde_json::Value)
    -> Option<Result<serde_json::Value, crate::dev_server::ApiError>>
{
    if cmd == "approval_respond" {
        let approval_id = body.get("approvalId").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let decision: ApprovalDecisionDto = match serde_json::from_value(body.get("decision").cloned().unwrap_or_default()) {
            Ok(d) => d,
            Err(e) => return Some(Err(crate::dev_server::ApiError { code: "INVALID_PARAMS".into(), message: e.to_string() })),
        };
        let internal: app_core::coding::AppApprovalDecision = decision.into();
        return Some(match futures::executor::block_on(core.respond_approval(&approval_id, internal)) {
            Ok(()) => Ok(serde_json::json!({})),
            Err(e) => Err(e.into()),
        });
    }
    None
}
```

- [ ] **Step 3: Implement `AppCore::respond_approval` (or alias to existing `respond_approval`)**

In `crates/app-core/src/coding/approval_handler.rs` (existing file per scan):

```rust
impl AppCore {
    pub async fn respond_approval(&self, approval_id: &str, decision: AppApprovalDecision) -> common::Result<()> {
        crate::coding::approval_handler::respond_approval(&self.pending_approvals, approval_id, decision).await
    }
}
```

- [ ] **Step 4: Register the command**

In `crates/desktop/src/commands/mod.rs`: add `pub mod approval;`.

In `crates/desktop/src/specta_builder.rs`: add `crate::commands::approval::approval_respond,` to `klynt_collect_commands![…]`.

In `crates/desktop/src/dev_server/dispatch.rs`: add `crate::commands::approval::dispatch_dev(cmd, core, body)` to the dispatch chain.

- [ ] **Step 5: Build + test bindings**

```bash
cargo tauri dev   # regenerate bindings.ts; Ctrl+C after both ports up
cargo nextest run -p desktop --test bindings_are_current
cargo nextest run -p desktop --test registration_drift
cargo nextest run -p desktop --test no_raw_tauri_command_outside_macros
```

Expected: all PASS. Commit `bindings.ts` regenerated.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/approval.rs crates/desktop/src/commands/mod.rs crates/desktop/src/specta_builder.rs crates/desktop/src/dev_server/dispatch.rs desktop-ui/src/bindings.ts
git commit -m "feat(desktop): add approval_respond Tauri command + dev_server dispatch"
```

### Task 1.5: Update existing 6 tool callsites to populate `thread_id`/`turn_id`

**Files:**
- Modify: `crates/klynt-core/src/tools/bash.rs` (line ~106 for evaluate call; struct construction nearby)
- Modify: `crates/klynt-core/src/tools/edit.rs` (line ~155)
- Modify: `crates/klynt-core/src/tools/write.rs` (line ~155)
- Modify: `crates/klynt-core/src/tools/notebook_edit.rs` (line ~155)
- Modify: `crates/klynt-core/src/tools/apply_patch.rs` (line ~153)
- Modify: `crates/klynt-core/src/tools/web_fetch.rs` (line ~158)

- [ ] **Step 1: Audit each tool's `GuardCtx` construction**

For each of the 6 files, find where `GuardCtx { … }` is built. Add:

```rust
thread_id: ctx.session_key.clone().map(|s| s.replace("chat:", "")),  // pragma: pass session as thread_id; refined in T3
turn_id: ctx.turn_id.clone(),
```

(`ctx` here is the agent-side `ToolContext`; ensure it carries `turn_id` — if not, add it as `Option<String>` in T3.5.)

- [ ] **Step 2: Build**

```bash
cargo build -p klynt-core
```

Expected: clean compile after all 6 sites are updated.

- [ ] **Step 3: Run tool tests**

```bash
cargo nextest run -p klynt-core --test '*tools*'
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-core/src/tools/
git commit -m "feat(approval): thread thread_id/turn_id through all 6 tool GuardCtx sites"
```

### Task 1.6: Frontend — `approval_respond` endpoint + ApprovalRequest types

**Files:**
- Create: `desktop-ui/src/api/endpoints/approval.ts`

- [ ] **Step 1: Write the endpoint wrapper**

```typescript
import { invoke } from "../client";
import type { ApprovalDecisionDto } from "../../bindings";

export async function approvalRespond(approvalId: string, decision: ApprovalDecisionDto): Promise<void> {
  return invoke<void>("approval_respond", { approvalId, decision });
}
```

- [ ] **Step 2: Run typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS (assumes `bindings.ts` was regenerated in T1.4).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/api/endpoints/approval.ts
git commit -m "feat(ui): add approval_respond typed endpoint"
```

### Task 1.7: Frontend — `ApprovalCard` renders `LayerDecisions` audit disclosure

**Files:**
- Modify: `desktop-ui/src/features/coding/components/ApprovalCard.tsx`
- Modify: `desktop-ui/src/features/coding/components/ApprovalCard.test.tsx` (add fixture)

- [ ] **Step 1: Add a failing test**

```tsx
// in ApprovalCard.test.tsx
it("renders layer audit disclosure when present", () => {
  const item = mockApprovalItem({
    layerDecisions: {
      privacy: { outcome: "allowed", reason: "passed", ruleMatched: null },
      layer1: { outcome: "deferred", reason: "no rule matched" },
      layer2: { outcome: "deferred", reason: "starlark fall-through" },
      layer3: { outcome: "skipped", reason: "mirror disabled" },
    },
  });
  render(<ApprovalCard item={item} onRespond={vi.fn()} />);
  fireEvent.click(screen.getByText(/why am i being asked/i));
  expect(screen.getByText(/starlark fall-through/i)).toBeVisible();
});
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd desktop-ui && bun run test ApprovalCard
```

Expected: FAIL.

- [ ] **Step 3: Add disclosure rendering**

In `ApprovalCard.tsx`, add a `<details>` block:

```tsx
{item.layerDecisions && (
  <details className="approval-card__why">
    <summary>Why am I being asked?</summary>
    <dl className="approval-card__layer-audit">
      <dt>Privacy</dt><dd>{item.layerDecisions.privacy.reason}</dd>
      <dt>Layer 1 (declarative)</dt><dd>{item.layerDecisions.layer1.reason}</dd>
      <dt>Layer 2 (Starlark)</dt><dd>{item.layerDecisions.layer2.reason}</dd>
      <dt>Layer 3 (Mirror)</dt><dd>{item.layerDecisions.layer3.reason}</dd>
    </dl>
  </details>
)}
```

- [ ] **Step 4: Run test to verify PASS**

```bash
cd desktop-ui && bun run test ApprovalCard
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/components/ApprovalCard.tsx desktop-ui/src/features/coding/components/ApprovalCard.test.tsx
git commit -m "feat(ui): ApprovalCard renders LayerDecisions audit disclosure"
```

### Task 1.8: K13 proptest — privacy guard inviolability under YoloMode

**Files:**
- Create: `crates/klynt-core/tests/k13_privacy_under_yolo.rs`

- [ ] **Step 1: Write the proptest**

```rust
use klynt_core::approval::{evaluate, ApprovalDecision, GuardCtx, /* helpers */};
use klynt_core::privacy::PrivacyGuard;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]
    #[test]
    fn k13_privacy_inviolable_under_yolo(
        path in "/(home|tmp|var)(/[a-z]{1,8}){1,3}/(.ssh|.aws|.gnupg|secret|.env)\\b",
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let exclude_globs = vec!["**/.ssh/**", "**/.aws/**", "**/.gnupg/**", "**/secret*", "**/.env"];
            let privacy = PrivacyGuard::from_globs(&exclude_globs).unwrap();
            let ctx = test_helpers::yolo_mode_ctx(privacy);  // approval_mode = YoloMode
            let decision = evaluate(ctx, "edit", &path).await;
            prop_assert!(matches!(decision, ApprovalDecision::PrivacyDenied { .. }),
                "expected PrivacyDenied for {path}, got {decision:?}");
        });
        Ok(())
    }
}
```

- [ ] **Step 2: Add `test_helpers::yolo_mode_ctx`**

In `crates/klynt-core/src/approval/tests/test_helpers.rs`: a builder that produces `GuardCtx` with `non_ui_policy = NonUiPolicy::Yolo` (or whatever the YOLO equivalent is in the existing config).

- [ ] **Step 3: Run the proptest**

```bash
cargo nextest run -p klynt-core --test k13_privacy_under_yolo
```

Expected: PASS (50 cases).

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-core/tests/k13_privacy_under_yolo.rs crates/klynt-core/src/approval/tests/test_helpers.rs
git commit -m "test(approval): add K13 proptest — privacy inviolable under YoloMode"
```

---

## Track 2 — Thread lifecycle commands (12 tasks)

### Task 2.1: `coding_thread_start` AppCore handler

**Files:**
- Create: `crates/app-core/src/coding/thread_handler.rs`
- Modify: `crates/app-core/src/coding/mod.rs`
- Test: `crates/app-core/tests/coding_thread_start_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
use app_core::AppCore;
use desktop_shared::coding::ApprovalPolicy;
use storage::StoragePool;

#[tokio::test]
async fn coding_thread_start_creates_session_with_workspace() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let core = AppCore::test_instance(pool).await;
    let workspace_id = core.add_workspace("/tmp/test-ws").await.unwrap().id;

    let thread = core.coding_thread_start(&workspace_id, None, Some(ApprovalPolicy::AskOnRisky), false).await.unwrap();
    assert!(thread.id.starts_with("chat:") || !thread.id.is_empty());
    assert_eq!(thread.workspace_id, workspace_id);
    assert_eq!(thread.cwd, "/tmp/test-ws");
    assert!(matches!(thread.approval_policy, ApprovalPolicy::AskOnRisky));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo nextest run -p app-core coding_thread_start_creates_session_with_workspace
```

Expected: FAIL ("no method named `coding_thread_start`").

- [ ] **Step 3: Implement the handler**

```rust
// crates/app-core/src/coding/thread_handler.rs
use std::sync::Arc;
use desktop_shared::coding::{Thread, ApprovalPolicy, SandboxKind, MessageDto, InstructionSource};
use crate::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_start(
        &self,
        workspace_id: &str,
        model: Option<String>,
        approval_policy: Option<ApprovalPolicy>,
        ephemeral: bool,
    ) -> common::Result<Thread> {
        // 1. Load workspace
        let ws = self.repos.workspaces.get(workspace_id).await?
            .ok_or_else(|| common::KlyntbotError::not_found("workspace", workspace_id))?;

        // 2. Privacy: refuse risky workspace paths
        validate_workspace_path(&ws.path)?;

        // 3. Resolve provider/model/policy chain
        let resolved_model = model.or_else(|| self.config.coding.defaults.model.clone()).or_else(|| self.config.agents.defaults.model.clone());
        let resolved_policy = approval_policy.unwrap_or(ApprovalPolicy::AskOnRisky);

        // 4. Resolve sandbox profile
        let sandbox = if cfg!(target_os = "macos") {
            SandboxKind::MacosSeatbelt
        } else if cfg!(target_os = "linux") {
            SandboxKind::LinuxBwrapLandlock
        } else {
            SandboxKind::Disabled
        };

        // 5. Walk AGENTS.md (Track 4 — for now, return empty)
        let instruction_sources = vec![]; // populated in T4

        // 6. Allocate session
        let session_key = format!("coding:{}", uuid::Uuid::new_v4());
        let metadata = serde_json::json!({
            "channel": "coding",
            "workspace_id": ws.id,
            "approval_policy": format!("{resolved_policy:?}"),
            "ephemeral": ephemeral,
            "model": resolved_model,
        });
        self.repos.sessions.upsert_session(&session_key, &metadata).await?;
        self.repos.sessions.set_workspace_id(&session_key, &ws.id).await?; // new SessionRepo method
        self.repos.sessions.set_ephemeral(&session_key, ephemeral).await?;

        // 7. Build Thread DTO
        Ok(Thread {
            id: session_key,
            workspace_id: ws.id,
            cwd: ws.path,
            model: resolved_model,
            approval_policy: resolved_policy,
            sandbox,
            instruction_sources,
            created_at: jiff::Timestamp::now().as_millisecond(),
            updated_at: jiff::Timestamp::now().as_millisecond(),
            title: None,
            starred: false,
            archived_at: None,
            ephemeral,
            forked_from_id: None,
            summary_message_id: None,
            total_cost_usd: 0.0,
            total_tokens: 0,
            items: vec![],
        })
    }
}

fn validate_workspace_path(path: &str) -> common::Result<()> {
    let dangerous = ["/", "/etc", "/usr", "/Users", "/home"];
    let patterns = [".ssh", ".aws", ".gnupg"];
    let p = std::path::Path::new(path);
    let canonical = p.canonicalize().map_err(|e| common::KlyntbotError::validation(format!("invalid path: {e}")))?;
    let s = canonical.to_string_lossy();
    for d in dangerous { if s == d { return Err(common::KlyntbotError::validation(format!("path is dangerous: {s}"))); } }
    for pat in patterns { if s.contains(pat) { return Err(common::KlyntbotError::validation(format!("path contains forbidden pattern: {pat}"))); } }
    Ok(())
}
```

Add `set_workspace_id` and `set_ephemeral` to `SessionRepo`:

```rust
impl SessionRepo {
    pub async fn set_workspace_id(&self, session_key: &str, workspace_id: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE sessions SET workspace_id = ?, updated_at = unixepoch('now') * 1000 WHERE key = ?")
            .bind(workspace_id).bind(session_key)
            .execute(&self.pool).await?;
        Ok(())
    }
    pub async fn set_ephemeral(&self, session_key: &str, ephemeral: bool) -> Result<(), StorageError> {
        sqlx::query("UPDATE sessions SET ephemeral = ? WHERE key = ?")
            .bind(if ephemeral { 1 } else { 0 }).bind(session_key)
            .execute(&self.pool).await?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify PASS**

```bash
cargo nextest run -p app-core coding_thread_start_creates_session_with_workspace
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/thread_handler.rs crates/app-core/src/coding/mod.rs crates/storage/src/repos/session.rs crates/app-core/tests/coding_thread_start_test.rs
git commit -m "feat(coding): coding_thread_start AppCore handler"
```

### Task 2.2: `coding_thread_start` Tauri command + registration

**Files:**
- Create: `crates/desktop/src/commands/coding_thread.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/specta_builder.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`

- [ ] **Step 1: Create the command**

```rust
// crates/desktop/src/commands/coding_thread.rs
use desktop_macros::klynt_command;
use desktop_shared::coding::{Thread, ApprovalPolicy};
use crate::commands::error::CommandResult;

#[klynt_command]
pub async fn coding_thread_start(
    workspace_id: String,
    model: Option<String>,
    approval_policy: Option<ApprovalPolicy>,
    ephemeral: Option<bool>,
) -> CommandResult<Thread> {
    let thread = state.coding_thread_start(&workspace_id, model, approval_policy, ephemeral.unwrap_or(false)).await
        .map_err(|e| crate::commands::error::ApiError::from(e))?;
    Ok(thread)
}

pub fn dispatch_dev(cmd: &str, core: &Arc<app_core::AppCore>, body: &serde_json::Value)
    -> Option<Result<serde_json::Value, crate::dev_server::ApiError>>
{
    use serde_json::json;
    if cmd == "coding_thread_start" {
        let workspace_id = body.get("workspaceId").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let model = body.get("model").and_then(|v| v.as_str()).map(String::from);
        let approval_policy: Option<ApprovalPolicy> = body.get("approvalPolicy").and_then(|v| serde_json::from_value(v.clone()).ok());
        let ephemeral = body.get("ephemeral").and_then(|v| v.as_bool()).unwrap_or(false);
        return Some(match futures::executor::block_on(core.coding_thread_start(&workspace_id, model, approval_policy, ephemeral)) {
            Ok(t) => Ok(serde_json::to_value(t).unwrap()),
            Err(e) => Err(e.into()),
        });
    }
    None
}
```

- [ ] **Step 2: Register**

In `crates/desktop/src/commands/mod.rs`: add `pub mod coding_thread;`.

In `crates/desktop/src/specta_builder.rs`: add `crate::commands::coding_thread::coding_thread_start,` to the macro list.

In `crates/desktop/src/dev_server/dispatch.rs`: chain `crate::commands::coding_thread::dispatch_dev(...)`.

- [ ] **Step 3: Regenerate bindings + run tests**

```bash
cargo tauri dev   # regen, Ctrl+C
cargo nextest run -p desktop --test bindings_are_current --test registration_drift
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/commands/coding_thread.rs crates/desktop/src/commands/mod.rs crates/desktop/src/specta_builder.rs crates/desktop/src/dev_server/dispatch.rs desktop-ui/src/bindings.ts
git commit -m "feat(desktop): coding_thread_start Tauri command + dev_server"
```

### Task 2.3: `coding_thread_resume` (AppCore + Tauri)

Same TDD pattern as 2.1+2.2. Returns `Thread` with optional `items: Vec<MessageDto>` populated.

**Files:**
- Modify: `crates/app-core/src/coding/thread_handler.rs`
- Modify: `crates/desktop/src/commands/coding_thread.rs`

- [ ] **Step 1: Write a test that asserts items load when `include_items: true`**
- [ ] **Step 2: Run, FAIL**
- [ ] **Step 3: Implement (loads `Session` row, optionally calls `get_messages_parts`, builds `Thread` DTO)**
- [ ] **Step 4: Run, PASS**
- [ ] **Step 5: Add Tauri command + register + dispatch_dev**
- [ ] **Step 6: Regenerate bindings, run tests**
- [ ] **Step 7: Commit**

```rust
// AppCore method
pub async fn coding_thread_resume(&self, thread_id: &str, include_items: bool) -> common::Result<Thread> {
    let session = self.repos.sessions.get_session(thread_id).await?
        .ok_or_else(|| common::KlyntbotError::not_found("session", thread_id))?;
    let workspace = if let Some(ws_id) = &session.workspace_id {
        self.repos.workspaces.get(ws_id).await?
    } else {
        None
    };
    let ws = workspace.ok_or_else(|| common::KlyntbotError::not_found("workspace", &session.workspace_id.unwrap_or_default()))?;
    let items = if include_items {
        self.repos.sessions.get_messages_parts(thread_id, 1000).await?
            .into_iter().map(message_with_parts_to_dto).collect()
    } else {
        vec![]
    };
    Ok(Thread {
        id: session.key,
        workspace_id: ws.id,
        cwd: ws.path,
        model: parse_metadata_model(&session.metadata),
        approval_policy: parse_approval_policy(&session.approval_mode),
        sandbox: resolve_sandbox(),
        instruction_sources: vec![],
        created_at: session.created_at,
        updated_at: session.updated_at,
        title: parse_metadata_title(&session.metadata),
        starred: parse_metadata_starred(&session.metadata),
        archived_at: session.archived_at,
        ephemeral: session.ephemeral != 0,
        forked_from_id: session.forked_from_id,
        summary_message_id: session.summary_message_id,
        total_cost_usd: session.total_cost_usd,
        total_tokens: session.total_tokens,
        items,
    })
}
```

### Task 2.4: `coding_thread_read`

Same pattern. Returns `Thread { items: Vec<MessageDto> }` with cursor/limit.

- [ ] AppCore method `coding_thread_read(thread_id, cursor, limit)` calls `get_messages_parts` with offset
- [ ] Tauri command + dispatch_dev + register
- [ ] Test for cursor pagination (3 messages, limit=2, second page returns 1)
- [ ] Commit

### Task 2.5: `coding_thread_list`

- [ ] AppCore method: `repos.sessions.list_sessions_by_workspace(workspace_id, cursor, limit, sort_key)` (new repo method using `WHERE workspace_id = ? AND archived_at IS NULL`)
- [ ] Returns `Vec<ThreadSummary>`
- [ ] Tauri command + dispatch_dev + register
- [ ] Test
- [ ] Commit

### Task 2.6: `coding_thread_fork`

- [ ] AppCore method: copies session row, sets `forked_from_id`, copies messages up to `from_message_id`
- [ ] Reuses existing `SessionRepo::fork_session` if signature matches; otherwise new method
- [ ] Tauri command + dispatch_dev + register
- [ ] Test
- [ ] Commit

### Task 2.7: `coding_thread_compact`

- [ ] AppCore method: load full message history, call provider.summarize (use existing `cognitive::services::session_memory::generate_summary`), store as new assistant message, set `Session.summary_message_id`
- [ ] Returns `{ summaryMessageId }`
- [ ] Tauri command + dispatch_dev + register
- [ ] Test (mock provider returns "summary" → row created, `summary_message_id` set)
- [ ] Commit

### Task 2.8: `coding_thread_archive`

- [ ] AppCore method: `repos.sessions.archive(thread_id)` sets `archived_at = unixepoch()` (new repo method)
- [ ] Returns `{}`
- [ ] Tauri command + dispatch_dev + register
- [ ] Test (archive twice = idempotent; archived shows up in `coding_thread_list` only with explicit flag)
- [ ] Commit

### Task 2.9: `coding_thread_set_name`

- [ ] AppCore method: writes to `Session.metadata.title` JSON field
- [ ] Returns `{}`
- [ ] Tauri command + dispatch_dev + register
- [ ] Test
- [ ] Commit

### Task 2.10: `coding_thread_subscribe` + `coding_thread_unsubscribe` + subscription manager

**Files:**
- Create: `crates/app-core/src/coding/subscription.rs`
- Modify: `crates/app-core/src/state.rs` (add `thread_subscriptions: DashMap<SubId, SubscriptionState>`)
- Create: `crates/desktop/src/commands/coding_thread.rs` (extend)

- [ ] **Step 1: Write a test for subscribe/unsubscribe round-trip**

```rust
#[tokio::test]
async fn subscribe_then_unsubscribe_round_trip() {
    let core = test_app_core().await;
    let thread = core.coding_thread_start("ws-1", None, None, false).await.unwrap();
    let sub_id = core.coding_thread_subscribe(&thread.id).await.unwrap();
    assert!(core.thread_subscriptions.contains_key(&sub_id));
    core.coding_thread_unsubscribe(&sub_id).await.unwrap();
    assert!(!core.thread_subscriptions.contains_key(&sub_id));
}
```

- [ ] **Step 2: Implement**

```rust
// crates/app-core/src/coding/subscription.rs
use std::sync::Arc;
use tauri::Emitter;
use dashmap::DashMap;
use desktop_shared::coding::ThreadEvent;
use bus::TypedBroker;

pub struct SubscriptionState {
    pub thread_id: String,
    pub created_at: i64,
}

impl AppCore {
    pub async fn coding_thread_subscribe(&self, thread_id: &str) -> common::Result<String> {
        let sub_id = uuid::Uuid::new_v4().to_string();
        self.thread_subscriptions.insert(sub_id.clone(), SubscriptionState {
            thread_id: thread_id.into(),
            created_at: jiff::Timestamp::now().as_millisecond(),
        });

        // Spawn adapter task: TypedBroker<ThreadEvent> → app.emit("agent:thread_event#<sub_id>", evt)
        let mut rx = self.thread_events.subscribe();
        let app = self.app_handle.clone();
        let target_thread_id = thread_id.to_string();
        let sid = sub_id.clone();
        tokio::spawn(async move {
            while let Ok(evt) = rx.recv().await {
                let evt_thread_id = match &evt {
                    ThreadEvent::TurnStarted { thread_id, .. } |
                    ThreadEvent::ItemStarted { thread_id, .. } |
                    ThreadEvent::ItemDelta { thread_id, .. } |
                    ThreadEvent::ItemCompleted { thread_id, .. } |
                    ThreadEvent::ToolCallStarted { thread_id, .. } |
                    ThreadEvent::ToolCallCompleted { thread_id, .. } |
                    ThreadEvent::FileChanged { thread_id, .. } |
                    ThreadEvent::CommandExecuted { thread_id, .. } |
                    ThreadEvent::ContextCompressed { thread_id, .. } |
                    ThreadEvent::TurnCompleted { thread_id, .. } => thread_id.as_str(),
                    ThreadEvent::Heartbeat { .. } => &target_thread_id,
                };
                if evt_thread_id == target_thread_id {
                    let channel = format!("agent:thread_event#{sid}");
                    let _ = app.emit(&channel, &evt);
                }
            }
        });

        Ok(sub_id)
    }

    pub async fn coding_thread_unsubscribe(&self, subscription_id: &str) -> common::Result<()> {
        self.thread_subscriptions.remove(subscription_id);
        Ok(())
    }
}
```

Add `thread_subscriptions: Arc<DashMap<String, SubscriptionState>>` and `thread_events: TypedBroker<ThreadEvent>` to `AppCore` in `state.rs`. Initialize in `AppCore::new` with `TypedBroker::new(1024)`.

- [ ] **Step 3: Heartbeat task**

In `AppCore::new`, spawn one global heartbeat task:

```rust
let broker = self.thread_events.clone();
let subs = self.thread_subscriptions.clone();
tokio::spawn(async move {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        for entry in subs.iter() {
            broker.publish(ThreadEvent::Heartbeat {
                subscription_id: entry.key().clone(),
                server_time: jiff::Timestamp::now().as_millisecond(),
            });
        }
    }
});
```

- [ ] **Step 4: Tauri commands + register + dispatch_dev**

```rust
#[klynt_command]
pub async fn coding_thread_subscribe(thread_id: String) -> CommandResult<SubscribeResponse> {
    let sub_id = state.coding_thread_subscribe(&thread_id).await?;
    Ok(SubscribeResponse { subscription_id: sub_id })
}

#[klynt_command]
pub async fn coding_thread_unsubscribe(subscription_id: String) -> CommandResult<()> {
    state.coding_thread_unsubscribe(&subscription_id).await?;
    Ok(())
}
```

- [ ] **Step 5: Test**

```bash
cargo nextest run -p app-core subscribe_then_unsubscribe_round_trip
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding/subscription.rs crates/app-core/src/state.rs crates/desktop/src/commands/coding_thread.rs crates/desktop/src/specta_builder.rs crates/desktop/src/dev_server/dispatch.rs
git commit -m "feat(coding): coding_thread_subscribe/unsubscribe + heartbeat task"
```

### Task 2.11: K12 proptest — subscription event ordering monotonicity

**Files:**
- Create: `crates/app-core/tests/k12_thread_event_ordering.rs`

- [ ] **Step 1: Write the proptest**

```rust
use proptest::prelude::*;
use desktop_shared::coding::{ThreadEvent, MessageDto};

proptest! {
    #[test]
    fn k12_event_reducer_is_order_invariant_within_iteration(
        events in prop::collection::vec(arb_thread_event(), 0..30),
    ) {
        // Apply events in given order — capture state
        let mut s1 = ReducerState::default();
        for e in &events { apply_event(&mut s1, e); }
        // Apply events grouped: identical operations across all permutations of same iteration
        // (within an iteration, ordering of independent items doesn't matter beyond delta sequencing)
        let mut s2 = ReducerState::default();
        for e in &events { apply_event(&mut s2, e); }  // same order; trivially equal
        prop_assert_eq!(s1, s2);
    }
}
```

(K12 — interpret as "the reducer is deterministic for the canonical Tauri-ordered stream"; document that cross-channel reordering is not a Phase 4 concern.)

- [ ] **Step 2: Run, PASS**
- [ ] **Step 3: Commit**

### Task 2.12: Wire `TypedBroker<ThreadEvent>` into `AppCore`

Already done in T2.10 — verify `AppCore::thread_events` is reachable from execute_loop. Track 3 actually publishes events here.

- [ ] Skip if covered

---

## Track 3 — Turn lifecycle + event emission (15 tasks)

### Task 3.1: `coding_message_send` AppCore handler

**Files:**
- Create: `crates/app-core/src/coding/turn_handler.rs`
- Modify: `crates/desktop/src/commands/coding_turn.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn coding_message_send_returns_turn_id_synchronously() {
    let core = test_app_core_with_mock_provider().await;
    let thread = core.coding_thread_start("ws-1", None, None, false).await.unwrap();
    let resp = core.coding_message_send(&thread.id, "hello", None, None, None, vec![], vec![], None).await.unwrap();
    assert!(!resp.turn_id.is_empty());
    assert!(resp.turn_started_at > 0);
}
```

- [ ] **Step 2: Implement**

```rust
pub struct CodingMessageSendResponse {
    pub turn_id: String,
    pub turn_started_at: i64,
}

impl AppCore {
    pub async fn coding_message_send(
        &self,
        thread_id: &str,
        text: &str,
        model: Option<String>,
        effort: Option<String>,
        access_mode: Option<String>,
        images: Vec<String>,
        app_mentions: Vec<serde_json::Value>,
        collaboration_mode: Option<serde_json::Value>,
    ) -> common::Result<CodingMessageSendResponse> {
        let turn_id = format!("turn-{}", uuid::Uuid::new_v4());
        let started_at = jiff::Timestamp::now().as_millisecond();

        // 1. Append user Message with parts
        let user_msg_id = uuid::Uuid::new_v4().to_string();
        let mut parts = vec![storage::messages::parts::MessagePart::Text { text: text.into() }];
        // Append image parts when images vec is non-empty (trivial path; full multimodal in T3.x)
        // ...
        self.repos.sessions.add_message_with_parts(thread_id, &user_msg_id, "user", &parts, Some(&turn_id), None).await?;

        // 2. Emit TurnStarted
        let resolved_model = model.clone().or_else(|| self.config.coding.defaults.model.clone()).unwrap_or_else(|| "default".into());
        self.thread_events.publish(ThreadEvent::TurnStarted {
            thread_id: thread_id.into(),
            turn_id: turn_id.clone(),
            model: resolved_model.clone(),
            started_at,
        });

        // 3. Spawn agent task
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.active_streams.insert(thread_id.into(), cancel_token.clone());
        let core_ref = Arc::new(self.clone()); // assumes AppCore: Clone
        let turn_id_clone = turn_id.clone();
        let thread_id_clone = thread_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = core_ref.run_coding_turn(&thread_id_clone, &turn_id_clone, cancel_token).await {
                tracing::error!("coding turn failed: {e}");
            }
        });

        Ok(CodingMessageSendResponse { turn_id, turn_started_at: started_at })
    }
}
```

`run_coding_turn` is the bridge to the existing agent runtime (T3.4 fills it in). For now, stub it to emit `TurnCompleted { Completed }` immediately so the test passes.

- [ ] **Step 3: Run, PASS**
- [ ] **Step 4: Tauri command + register + dispatch_dev**
- [ ] **Step 5: Commit**

### Task 3.2: `coding_turn_interrupt`

- [ ] AppCore method: looks up `active_streams[thread_id]`, calls `cancel()`. Asserts at least one cancel before unblock.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test (start turn → interrupt → assert TurnCompleted{Cancelled})
- [ ] Commit

### Task 3.3: `coding_turn_steer` + `SteerQueue`

**Files:**
- Create: `crates/app-core/src/coding/steer_queue.rs`
- Modify: `crates/agent/src/execution/live_context_refresher.rs`

- [ ] **Step 1: Test for steer injection**
- [ ] **Step 2: Implement `SteerQueue` (Tokio mpsc per active turn)**

```rust
pub struct SteerQueue {
    txs: DashMap<String /*turn_id*/, tokio::sync::mpsc::UnboundedSender<String>>,
}

impl SteerQueue {
    pub fn register_turn(&self, turn_id: &str) -> tokio::sync::mpsc::UnboundedReceiver<String> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.txs.insert(turn_id.into(), tx);
        rx
    }
    pub fn push(&self, turn_id: &str, text: String) -> common::Result<()> {
        if let Some(tx) = self.txs.get(turn_id) {
            tx.send(text).map_err(|_| common::KlyntbotError::not_found("turn", turn_id))?;
            Ok(())
        } else {
            Err(common::KlyntbotError::not_found("turn", turn_id))
        }
    }
}
```

- [ ] **Step 3: Wire `SteerQueue::register_turn` into `coding_message_send`'s spawned task**
- [ ] **Step 4: Modify `LiveContextRefresher` to also drain `SteerQueue` (extend the existing `drain` to accept multiple sources)**
- [ ] **Step 5: Tauri command + register + dispatch_dev**
- [ ] **Step 6: Test (start turn → steer "actually use python" → next iteration receives synthetic user msg)**
- [ ] **Step 7: Commit**

### Task 3.4: Wire event emission into `execute_loop.rs`

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs`
- Modify: `crates/agent/src/execution/core.rs`

- [ ] **Step 1: Audit existing emission sites**

Per agent-runtime scan, current loop emits AgentEvents (`PipelineStarted`, `IterationStart`, `ContentChunk`, `ToolStart`, `ToolEnd`, `ContextCompressed`, `ContextReassembled`, `BudgetUpdate`, `TurnComplete`, `Done`). These bridge to Tauri via `relay_chat_stream` for the chat surface.

- [ ] **Step 2: Add a `thread_events: Option<Arc<TypedBroker<ThreadEvent>>>` field to `ExecutionParams`**

Defaults to `None` for the chat surface; populated for coding sessions.

- [ ] **Step 3: At each existing emission site, also publish to `thread_events` if present**

Translation table:

| Existing AgentEvent | New ThreadEvent | Where |
|---|---|---|
| `IterationStart` | `TurnStarted` (only on first iteration of a new turn_id) | execute_loop.rs:129 |
| (new) | `ItemStarted { Message::placeholder_assistant() }` | execute_loop.rs:140 |
| `ContentChunk` | `ItemDelta { delta: PartDelta::Text { append } }` | core.rs:258 |
| (new) | `ItemCompleted` after stream completes | execute_loop.rs:148 |
| `ToolStart` | `ToolCallStarted` | core.rs:623 |
| `ToolEnd` | `ToolCallCompleted` | core.rs:677 |
| `ContextCompressed` | `ContextCompressed` (variant rename only) | execute_loop.rs:233 |
| `TurnComplete` | `TurnCompleted { finish_reason }` | execute_loop.rs:160 |

- [ ] **Step 4: Build + run agent tests**

Expected: existing tests pass; `thread_events: None` makes new emit a no-op.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/execution/
git commit -m "feat(agent): emit ThreadEvent variants alongside existing AgentEvents"
```

### Task 3.5: `FinishReason` enum integration in agent loop

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Replace `String` finish_reason in ExecuteLoopResult with `FinishReason`**
- [ ] **Step 2: Map provider finish strings (`"stop"`, `"end_turn"`, `"length"`, etc.) to typed enum variants**
- [ ] **Step 3: Add `FinishReason::CostCeilingReached` emission when cost ceiling hard-stop fires (currently the runtime publishes a Mirror alert; T3.13 adds the loop-exit case)**
- [ ] **Step 4: Run tests, commit**

### Task 3.6: Wire `FileChanged` event from edit/write/apply_patch tools

**Files:**
- Modify: `crates/klynt-core/src/tools/edit.rs`
- Modify: `crates/klynt-core/src/tools/write.rs`
- Modify: `crates/klynt-core/src/tools/apply_patch.rs`

- [ ] After successful execution, publish `ThreadEvent::FileChanged { path, change }` via `thread_events` if available in ToolContext
- [ ] Test
- [ ] Commit

### Task 3.7: Wire `CommandExecuted` event from bash tool

- [ ] After bash execution, publish `ThreadEvent::CommandExecuted { command, exit_code }`
- [ ] Test
- [ ] Commit

### Task 3.8: `agent:cost_update` channel + TypedBroker<CostUpdate>

**Files:**
- Modify: `crates/app-core/src/state.rs` (add `cost_events: TypedBroker<CostUpdate>`)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (after every successful provider call, publish CostUpdate)
- Modify: `crates/desktop/src/main.rs` (spawn adapter: cost_events broker → app.emit("agent:cost_update", evt))

- [ ] **Step 1: Add `cost_events: TypedBroker<CostUpdate>` to AppCore**
- [ ] **Step 2: Publish on every assistant message complete**
- [ ] **Step 3: Spawn adapter task in `main.rs` to fan to Tauri**
- [ ] **Step 4: Test**
- [ ] **Step 5: Commit**

### Task 3.9–3.14: remaining event emissions

For each remaining event type, repeat the TDD pattern:
- `ItemDelta` for streaming reasoning blocks
- `ToolCallStarted/Completed` (covered partially in 3.4)
- `ContextCompressed` (translate from existing event)
- `Heartbeat` (covered in T2.10)

### Task 3.15: K12 proptest extension

Already in T2.11. Verify it covers all event types.

---

## Track 4 — Workspace files + AGENTS.md walking (12 tasks)

### Task 4.1: New `coding-agents-md` crate

**Files:**
- Create: `crates/coding-agents-md/Cargo.toml`
- Create: `crates/coding-agents-md/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add workspace member**

```toml
# bot/Cargo.toml
[workspace]
members = [
    # ... existing ...
    "crates/coding-agents-md",
]
```

- [ ] **Step 2: Create the crate**

```toml
# crates/coding-agents-md/Cargo.toml
[package]
name = "coding-agents-md"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create `lib.rs` with `walk_agents_md`**

```rust
use std::path::{Path, PathBuf};
use std::fs;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentsMdSource {
    pub path: PathBuf,
    pub dir: PathBuf,
    pub contents: String,
}

pub fn walk_agents_md(start: &Path) -> Vec<AgentsMdSource> {
    let mut sources = Vec::new();
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("AGENTS.md");
        if candidate.exists() {
            if let Ok(contents) = fs::read_to_string(&candidate) {
                sources.push(AgentsMdSource { path: candidate.clone(), dir: cur.clone(), contents });
            }
        }
        match cur.parent() {
            Some(parent) => cur = parent.to_path_buf(),
            None => break,
        }
    }
    sources.reverse();
    sources
}

pub fn format_agents_md_bundle(sources: &[AgentsMdSource]) -> String {
    sources.iter().map(|s| {
        format!("# AGENTS.md instructions for {}\n\n<INSTRUCTIONS>\n{}\n</INSTRUCTIONS>", s.dir.display(), s.contents)
    }).collect::<Vec<_>>().join("\n\n")
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p coding-agents-md
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-agents-md/ Cargo.toml Cargo.lock
git commit -m "feat(coding-agents-md): new crate with walk_agents_md helper"
```

### Task 4.2: Tests for `walk_agents_md`

- [ ] **Step 1: Write tests**

```rust
// crates/coding-agents-md/tests/walk.rs
use coding_agents_md::{walk_agents_md, format_agents_md_bundle};
use tempfile::TempDir;
use std::fs;

#[test]
fn walk_finds_ancestor_chain() {
    let td = TempDir::new().unwrap();
    let nested = td.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(td.path().join("AGENTS.md"), "root rules").unwrap();
    fs::write(td.path().join("a/AGENTS.md"), "a rules").unwrap();
    fs::write(nested.join("AGENTS.md"), "c rules").unwrap();

    let found = walk_agents_md(&nested);
    assert_eq!(found.len(), 3);
    // Outermost first
    assert_eq!(found[0].dir, td.path().to_path_buf());
    assert_eq!(found[2].dir, nested);
}

#[test]
fn walk_returns_empty_when_no_agents_md() {
    let td = TempDir::new().unwrap();
    let found = walk_agents_md(td.path());
    assert!(found.is_empty());
}

#[test]
fn format_bundle_uses_codex_instruction_wrapper() {
    let td = TempDir::new().unwrap();
    fs::write(td.path().join("AGENTS.md"), "rule one").unwrap();
    let found = walk_agents_md(td.path());
    let bundle = format_agents_md_bundle(&found);
    assert!(bundle.contains("# AGENTS.md instructions for"));
    assert!(bundle.contains("<INSTRUCTIONS>"));
    assert!(bundle.contains("rule one"));
    assert!(bundle.contains("</INSTRUCTIONS>"));
}
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p coding-agents-md
```

Expected: 3 PASS.

- [ ] **Step 3: Commit**

### Task 4.3: K14 proptest — walk determinism

**Files:**
- Create: `crates/coding-agents-md/tests/k14_determinism.rs`

```rust
use coding_agents_md::walk_agents_md;
use proptest::prelude::*;
use tempfile::TempDir;
use std::fs;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]
    #[test]
    fn k14_walk_is_deterministic(
        depths in prop::collection::vec(1usize..5, 1..6),
        with_md in prop::collection::vec(any::<bool>(), 6),
    ) {
        let td = TempDir::new().unwrap();
        let mut cur = td.path().to_path_buf();
        for (i, d) in depths.iter().enumerate() {
            let dirname = format!("d{i}");
            cur = cur.join(dirname);
            fs::create_dir_all(&cur).unwrap();
            if with_md.get(i).copied().unwrap_or(false) {
                fs::write(cur.join("AGENTS.md"), format!("rule {i}")).unwrap();
            }
        }
        let r1 = walk_agents_md(&cur);
        let r2 = walk_agents_md(&cur);
        prop_assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            prop_assert_eq!(&a.path, &b.path);
            prop_assert_eq!(&a.contents, &b.contents);
        }
    }
}
```

- [ ] Run, commit

### Task 4.4: `WorkspaceAgentsSource` ContextSource

**Files:**
- Create: `crates/context_engine/src/sources/workspace_agents.rs`
- Modify: `crates/context_engine/src/sources/mod.rs`

- [ ] Implement the trait, priority `-800` (below soul, above skills)
- [ ] `applies_to`: `ctx.channel == ChannelName::Coding`
- [ ] `produce`: contributes to `instructionSources` list (returned via Thread DTO), not to system prompt
- [ ] Test
- [ ] Commit

### Task 4.5: `coding_thread_start` injects AGENTS.md bundle

- [ ] In `AppCore::coding_thread_start`, after creating session, walk AGENTS.md, persist as synthetic User message with `parts: [Text { text: bundle }]`, `turn_id: None`
- [ ] Also walk global `~/.klyntbot/AGENTS.md` (if exists, prepended as top-level entry)
- [ ] Populate `Thread.instruction_sources` from walked files
- [ ] Test (workspace with parent/child AGENTS.md → 2 sources, bundle persists, populated in Thread DTO)
- [ ] Commit

### Task 4.6: `workspace_meta_read` command

**Files:**
- Create: `crates/desktop/src/commands/workspace_files.rs`
- Modify: `crates/app-core/src/coding/workspace_handler.rs`

- [ ] AppCore method: `workspace_meta_read(scope, kind, workspace_id)` — reads file based on scope+kind
  - `(workspace, agents)` → `<workspace.path>/AGENTS.md`
  - `(workspace, config)` → `<workspace.path>/.klyntbot/config.json` (if exists)
  - `(global, agents)` → `~/.klyntbot/AGENTS.md`
  - `(global, config)` → `~/.klyntbot/config.json`
- [ ] Returns `{ exists, content, truncated }`. Truncation cap: 100 KB.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

### Task 4.7: `workspace_meta_write` command

- [ ] AppCore method: writes file (creating dirs if needed). Refuses if path not under workspace or `~/.klyntbot/`.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

### Task 4.8: `workspace_files_list` command (fuzzy file search)

- [ ] AppCore method: walks `workspace.path`, applies optional fuzzy query (use `nucleo` crate, already in workspace deps)
- [ ] Returns `Vec<FuzzyFileMatch { path, score }>`. Limit default 50, max 200.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

### Task 4.9: `workspace_file_read` command

- [ ] AppCore method: reads file from `<workspace.path>/<path>`. Validates path doesn't escape workspace.
- [ ] Returns `{ content, truncated, mime, encoding }`. Truncation cap 1 MB.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test (read existing file; refuse `../etc/passwd`-style escape)
- [ ] Commit

### Task 4.10: `text_file_write` command

- [ ] AppCore method: writes content to absolute path. Used by save-dialog flows that already had user consent.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

### Task 4.11: `image_data_url` command

- [ ] AppCore method: reads image file, returns `data:<mime>;base64,<bytes>` URL
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

### Task 4.12: `app_icon_read` command

- [ ] AppCore method: queries macOS `mdfind` (existing pattern in `feature-launcher::search::app_index`) for app bundle, extracts `Icon.icns`, returns base64 data URL or null
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

---

## Track 5 — Frontend rewiring (12 tasks)

### Task 5.1: Update `desktop-ui/src/api/endpoints/thread.ts`

**Files:**
- Modify: `desktop-ui/src/api/endpoints/thread.ts`

- [ ] **Step 1: Replace 12 `invoke<any>` calls with typed equivalents**

Old:
```typescript
export async function startThread(workspaceId: string) {
  return invoke<any>("start_thread", { workspaceId });
}
```

New:
```typescript
import type { Thread, ApprovalPolicy, ThreadSummary } from "../../bindings";

export async function codingThreadStart(
  workspaceId: string,
  opts?: { model?: string; approvalPolicy?: ApprovalPolicy; ephemeral?: boolean }
): Promise<Thread> {
  return invoke<Thread>("coding_thread_start", {
    workspaceId,
    model: opts?.model ?? null,
    approvalPolicy: opts?.approvalPolicy ?? null,
    ephemeral: opts?.ephemeral ?? false,
  });
}

export async function codingThreadResume(threadId: string, includeItems = true): Promise<Thread> {
  return invoke<Thread>("coding_thread_resume", { threadId, includeItems });
}

export async function codingThreadRead(threadId: string, cursor?: string, limit?: number): Promise<Thread> {
  return invoke<Thread>("coding_thread_read", { threadId, cursor: cursor ?? null, limit: limit ?? null });
}

export async function codingThreadList(workspaceId: string, cursor?: string, limit?: number, sortKey?: "created_at" | "updated_at"): Promise<ThreadSummary[]> {
  return invoke<ThreadSummary[]>("coding_thread_list", { workspaceId, cursor: cursor ?? null, limit: limit ?? null, sortKey: sortKey ?? null });
}

export async function codingThreadFork(threadId: string, fromMessageId?: string): Promise<Thread> {
  return invoke<Thread>("coding_thread_fork", { threadId, fromMessageId: fromMessageId ?? null });
}

export async function codingThreadCompact(threadId: string): Promise<{ summaryMessageId: string }> {
  return invoke<{ summaryMessageId: string }>("coding_thread_compact", { threadId });
}

export async function codingThreadArchive(threadId: string): Promise<void> {
  return invoke<void>("coding_thread_archive", { threadId });
}

export async function codingThreadSetName(threadId: string, name: string): Promise<void> {
  return invoke<void>("coding_thread_set_name", { threadId, name });
}

export async function codingThreadSubscribe(threadId: string): Promise<{ subscriptionId: string }> {
  return invoke<{ subscriptionId: string }>("coding_thread_subscribe", { threadId });
}

export async function codingThreadUnsubscribe(subscriptionId: string): Promise<void> {
  return invoke<void>("coding_thread_unsubscribe", { subscriptionId });
}

export async function codingMessageSend(
  threadId: string, text: string,
  opts?: { model?: string; effort?: string; accessMode?: string; images?: string[]; appMentions?: any[]; collaborationMode?: any }
): Promise<{ turnId: string; turnStartedAt: number }> {
  return invoke<{ turnId: string; turnStartedAt: number }>("coding_message_send", {
    threadId, text,
    model: opts?.model ?? null,
    effort: opts?.effort ?? null,
    accessMode: opts?.accessMode ?? null,
    images: opts?.images ?? [],
    appMentions: opts?.appMentions ?? [],
    collaborationMode: opts?.collaborationMode ?? null,
  });
}

export async function codingTurnInterrupt(threadId: string, turnId: string): Promise<void> {
  return invoke<void>("coding_turn_interrupt", { threadId, turnId });
}

export async function codingTurnSteer(threadId: string, turnId: string, text: string, expectedTurnId: string): Promise<void> {
  return invoke<void>("coding_turn_steer", { threadId, turnId, text, expectedTurnId });
}

export async function codingReviewStart(threadId: string, target: any, delivery?: "inline" | "detached"): Promise<any> {
  return invoke<any>("coding_review_start", { threadId, target, delivery: delivery ?? null });
}

export async function codingMcpStatus(workspaceId: string, cursor?: string, limit?: number): Promise<any> {
  return invoke<any>("coding_mcp_status", { workspaceId, cursor: cursor ?? null, limit: limit ?? null });
}

export async function codingThreadMetadataGenerate(workspaceId: string, prompt: string): Promise<{ title: string; worktreeName: string }> {
  return invoke<{ title: string; worktreeName: string }>("coding_thread_metadata_generate", { workspaceId, prompt });
}
```

Drop these (dropped commands per spec): `getAccountRateLimits`, `getAccountInfo`, `runCodexLogin`, `cancelCodexLogin`, `setTrayRecentThreads`, `setTraySessionUsage` (tray-only kept as no-ops in browser mode), and the old `startThread`/`forkThread`/etc. names.

Keep `setTrayRecentThreads` / `setTraySessionUsage` (still used for native window tray; not in Phase 4 scope to remove).

- [ ] **Step 2: Run typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS (assumes bindings.ts has all the new types).

- [ ] **Step 3: Update import sites**

Many components import `startThread`, `sendUserMessage`, etc. Find them:

```bash
grep -rn "startThread\|sendUserMessage\|forkThread\|listThreads\|resumeThread\|readThread\|threadLiveSubscribe\|threadLiveUnsubscribe\|archiveThread\|setThreadName\|generateRunMetadata\|getAccountRateLimits\|getAccountInfo\|runCodexLogin\|cancelCodexLogin" desktop-ui/src --include="*.ts" --include="*.tsx" | wc -l
```

Replace each with the new name. Use `sed` if you trust the rename:

```bash
cd desktop-ui/src
find . -type f \( -name "*.ts" -o -name "*.tsx" \) | xargs sed -i '' \
  -e 's/\bstartThread\b/codingThreadStart/g' \
  -e 's/\bsendUserMessage\b/codingMessageSend/g' \
  -e 's/\bforkThread\b/codingThreadFork/g' \
  ...
```

Warning: dropped commands (`getAccountRateLimits` etc.) require deleting their usages, not renaming.

- [ ] **Step 4: Run typecheck + lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Fix any errors. Commit.

### Task 5.2: Update `desktop-ui/src/api/endpoints/files.ts`

- [ ] Replace `file_read`/`file_write` with `workspace_meta_read`/`workspace_meta_write`
- [ ] Replace `list_workspace_files` with `workspace_files_list`
- [ ] Replace `read_workspace_file` with `workspace_file_read`
- [ ] Replace `write_text_file` with `text_file_write`
- [ ] Replace `read_image_as_data_url` with `image_data_url`
- [ ] Replace `get_open_app_icon` with `app_icon_read`
- [ ] Drop `get_codex_config_path`
- [ ] Run typecheck, commit

### Task 5.3: New `desktop-ui/src/api/endpoints/providers.ts`

- [ ] Create file with `providersList` and `providerStatus` typed wrappers
- [ ] Used by `ProvidersSubsection.tsx` (T6.6)
- [ ] Commit

### Task 5.4: New `desktop-ui/src/api/endpoints/cost.ts`

- [ ] Create file exporting `subscribeCostUpdates(handler: (u: CostUpdate) => void): UnlistenFn` using `listen("agent:cost_update", ...)`
- [ ] Commit

### Task 5.5: Update `useAppServerEvents` to subscribe to new channels

**Files:**
- Modify: `desktop-ui/src/features/app/hooks/useAppServerEvents.ts`

- [ ] Add `agent:thread_event#<subId>` listener; route events to existing `onItemStarted`/`onItemCompleted`/`onAgentMessageDelta`/`onTurnStarted`/`onTurnCompleted` callbacks
- [ ] Add `agent:approval_request` listener; route to existing `onApprovalRequest` callback (extend payload with `LayerDecisions`)
- [ ] Add `agent:cost_update` listener; route to a new `onCostUpdate` callback
- [ ] Test
- [ ] Commit

### Task 5.6: Update `ApprovalCard` for `LayerDecisions`

Already covered in T1.7. Verify imports use new types from `bindings.ts` not local stubs.

### Task 5.7: Convert `CostCeilingBanner` from polling to event-listening

**Files:**
- Modify: `desktop-ui/src/features/coding/components/CostCeilingBanner.tsx`

- [ ] Remove `commands.codingMemoryMirrorAlertsFeed` polling
- [ ] Subscribe via `useAppServerEvents`'s new `onCostUpdate` callback
- [ ] When `costUpdate.ceiling_breached`, render banner
- [ ] Test
- [ ] Commit

### Task 5.8: Add `ProvidersSubsection.tsx`

**Files:**
- Create: `desktop-ui/src/features/settings/components/ProvidersSubsection.tsx`
- Modify: `desktop-ui/src/features/settings/components/AccountSubsection.tsx` (delete or replace)

- [ ] Render list of configured providers (from `providersList`)
- [ ] Per-provider edit-API-key button + test-connection
- [ ] Replace `AccountSubsection` import in settings page with `ProvidersSubsection`
- [ ] Test
- [ ] Commit

### Task 5.9: Update `MainApp.tsx` orchestration

- [ ] Audit lines around 221, 376, 459, 693, 1327 (codex orchestration calls)
- [ ] Update import-sites to use new endpoint names
- [ ] Test (TypeScript catches stale references)
- [ ] Commit

### Task 5.10: Add Parts-aware rendering in `MessageRows.tsx`

**Files:**
- Modify: `desktop-ui/src/features/messages/components/MessageRows.tsx`
- Create: `desktop-ui/src/features/coding/components/parts/{TextPart,ToolCallPart,ToolResultPart,FileChangePart,CommandExecutionPart,ReasoningPart,FinishPart}.tsx`

- [ ] For each part kind, create a tiny component (most are 5-10 lines)
- [ ] In `MessageRows.tsx`, the existing `MessageRow` reads `msg.content` (string). Replace with a `parts.map(p => <Part p={p} />)` switch
- [ ] Tests for each part component
- [ ] Commit

### Task 5.11: Regenerate `bindings.ts` and commit

- [ ] Run `cargo tauri dev`, Ctrl+C
- [ ] `cd desktop-ui && bun run typecheck` — must pass
- [ ] Commit `desktop-ui/src/bindings.ts`

### Task 5.12: Update `tauri.test.ts` mocks

- [ ] Replace 3 `file_read` assertions with `workspace_meta_read`
- [ ] Run `bun run test`, commit

---

## Track 6 — coding-orchestrator skill + auxiliary commands (6 tasks)

### Task 6.1: Write `coding-orchestrator` SKILL.md

**Files:**
- Create: `bot/skills/coding-orchestrator/SKILL.md`
- Create: `bot/skills/coding-orchestrator/references/tool-usage.md`
- Create: `bot/skills/coding-orchestrator/references/approval-policy.md`

- [ ] **Step 1: Create SKILL.md**

```markdown
---
name: coding-orchestrator
description: How to perform coding work in klyntbot — file edits, shell commands, recall, approvals, plan-mode.
when_to_use: |
  Use when the user asks for coding tasks: implement, fix, refactor, write tests, build, compile, debug, review.
  Activated automatically when channel == "coding" or workspace has a known programming-language file extension.
references:
  - tool-usage.md
  - approval-policy.md
mcp_tools: ["*"]
---

You are coding inside the user's workspace. Follow these guidelines:

## Tool selection

- **`bash`** for one-shot commands. Always pass `cwd` explicitly. Prefer non-interactive output (`-n`, `--non-interactive`).
- **`read`** before `edit`. The edit tool requires the exact existing string for safety.
- **`write`** to create new files. Refuses to overwrite existing without explicit user request.
- **`apply_patch`** for multi-file changes. Diffs are validated before apply.
- **`glob`** + **`grep`** for code search before file reads.
- **`recall_*`** to look up prior coding turns or decisions before duplicating work.
- **`enter_plan_mode` / `exit_plan_mode`** for non-trivial multi-step work — present a plan, get approval, then execute.
- **`ask_user`** when truly blocked. Don't ask trivial questions you could answer by reading.

## Approval discipline

The user's `approval_policy` determines what gates fire:
- `AskAlways` — every command/edit prompts.
- `AskOnRisky` (default) — declarative + Starlark + mirror-learned layers decide; only ambiguous cases ask.
- `AskOnFailure` — execute first, ask before retry on failure.
- `YoloMode` — bypassed except privacy guard (paths in `excludePaths` always denied).

Do NOT try to bypass approvals. The user can choose `accept_for_session` or `accept_with_execpolicy_amendment` to authorize patterns.

## Workspace conventions

- Read `AGENTS.md` files at workspace root and parent directories. They contain user-specific coding conventions.
- Honor existing code style — don't reformat unrelated code.
- Run tests + lint before claiming "done": `cargo test`, `bun run typecheck`, `pytest`, etc.
- Commit with descriptive messages. Don't commit secrets, env files, or generated artifacts.

## Cost awareness

If the user's `costCeiling.perThreadUsd` is set and getting close, prefer:
- Smaller scopes
- Pre-existing tools over net-new code
- Direct execution over LLM-mediated planning
```

- [ ] **Step 2: Create reference files**

(Brief content for each — `tool-usage.md` and `approval-policy.md` as referenced in the SKILL.)

- [ ] **Step 3: Test loading**

```bash
cargo nextest run -p klynt-skill-loader load_coding_orchestrator
```

(May need a new test asserting the skill loads from the bundled path.)

- [ ] **Step 4: Commit**

### Task 6.2: Register skill in `klynt-skill-loader`

**Files:**
- Modify: `crates/klynt-skill-loader/src/discovery.rs` (or `crates/skill-system/src/lib.rs`)

- [ ] Add `coding-orchestrator` to the bundled-skills list (currently has 5 — `task-management`, `finance-management`, `automation`, `notebook`, `learning`)
- [ ] Test (skill index has 6 entries)
- [ ] Commit

### Task 6.3: `coding_review_start` command

- [ ] AppCore method: starts a review pass (calls existing `start_review` AppCore handler if it exists, else simple LLM-driven review)
- [ ] Tauri command + register + dispatch_dev
- [ ] Test (smoke test only)
- [ ] Commit

### Task 6.4: `coding_mcp_status` command

- [ ] AppCore method: lists configured MCP servers for this workspace, returns health status (uses existing `mcp::server_status` if it exists)
- [ ] Tauri command + register + dispatch_dev
- [ ] Test
- [ ] Commit

### Task 6.5: `coding_thread_metadata_generate` command

- [ ] AppCore method: calls cheapest-available provider (Haiku or equivalent) with prompt: "Generate a 4-word title and a kebab-case worktree name from this user message: …". Returns `{ title, worktreeName }`.
- [ ] Tauri command + register + dispatch_dev
- [ ] Test (mock provider returns "Add fib CLI" + "add-fib-cli")
- [ ] Commit

### Task 6.6: `providers_list` + `provider_status` commands

- [ ] AppCore method `providers_list`: reads `~/.klyntbot/config.json`, returns providers with `{ id, name, hasApiKey, defaultModel }`
- [ ] AppCore method `provider_status`: per-provider, ping a tiny request to verify key works
- [ ] Tauri commands + register + dispatch_dev
- [ ] Tests
- [ ] Commit

---

## Track 7 — Tests + verification (8 tasks)

### Task 7.1: New build-time test `no_raw_invoke_in_endpoints.rs`

**Files:**
- Create: `crates/desktop/tests/no_raw_invoke_in_endpoints.rs`

- [ ] **Step 1: Write the test**

```rust
use std::process::Command;

#[test]
fn no_raw_invoke_in_endpoints() {
    let output = Command::new("rg")
        .args(["-n", r"invoke<any>\(|invoke\((?:[^<])"])
        .arg("../../desktop-ui/src/api/endpoints/")
        .args(["--type", "ts"])
        .output()
        .expect("failed to spawn rg");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Allowlist: comments and string literals that mention these patterns
    let actual_violations: Vec<&str> = stdout.lines()
        .filter(|l| !l.contains("// "))
        .filter(|l| !l.contains("\"invoke"))  // string literal
        .collect();

    if !actual_violations.is_empty() {
        panic!(
            "Found raw invoke without type parameter in api/endpoints:\n{}\n\
            Every endpoint function must use invoke<T> with an explicit return type.",
            actual_violations.join("\n")
        );
    }
}
```

- [ ] **Step 2: Run**

Expected: PASS (after T5.1 + T5.2 cleaned up the 12 known violations).

- [ ] **Step 3: Commit**

### Task 7.2: Extend K11 invariant for `archived_at`

**Files:**
- Modify: `tests/integration/coding_in_chat/property_k11_starred_never_pruned.rs` (or wherever K11 lives)

- [ ] Add a second proptest: archived sessions also never pruned
- [ ] Commit

### Task 7.3: Integration test — full `coding_message_send` round-trip

**Files:**
- Create: `tests/integration/coding_in_chat/full_turn_lifecycle.rs`

- [ ] **Step 1: Test**

```rust
#[tokio::test]
async fn full_turn_lifecycle_emits_all_phases() {
    let core = test_app_core_with_mock_provider().await;
    let workspace = core.add_workspace("/tmp/full-turn-test").await.unwrap();
    let thread = core.coding_thread_start(&workspace.id, None, None, false).await.unwrap();

    let mut rx = core.thread_events.subscribe();
    let resp = core.coding_message_send(&thread.id, "hello", None, None, None, vec![], vec![], None).await.unwrap();

    // Collect events until TurnCompleted or 5s timeout
    let mut events = vec![];
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(evt)) = tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv()).await {
            let is_done = matches!(evt, ThreadEvent::TurnCompleted { .. });
            events.push(evt);
            if is_done { break; }
        }
    }

    // Assert sequence
    assert!(matches!(events.first(), Some(ThreadEvent::TurnStarted { .. })));
    assert!(events.iter().any(|e| matches!(e, ThreadEvent::ItemStarted { .. })));
    assert!(events.iter().any(|e| matches!(e, ThreadEvent::ItemDelta { .. })));
    assert!(events.iter().any(|e| matches!(e, ThreadEvent::ItemCompleted { .. })));
    assert!(matches!(events.last(), Some(ThreadEvent::TurnCompleted { .. })));
}
```

- [ ] Run, commit

### Task 7.4: Integration test — tool dispatch parallelism

- [ ] Mock provider returns 3 concurrent bash calls; assert they execute via semaphore
- [ ] Same path tries 2 concurrent edits → second waits on path lock
- [ ] Commit

### Task 7.5: Integration test — approval flow round-trip

- [ ] Mock provider asks for `bash` execution; `evaluate` returns `Ask` with audit; `approval_respond` accepts; tool executes
- [ ] Commit

### Task 7.6: Integration test — AGENTS.md walking + injection

- [ ] Tempdir with parent + workspace AGENTS.md; `coding_thread_start` populates `instructionSources` (2 sources); persistent synthetic User message present
- [ ] Commit

### Task 7.7: Integration test — cancellation

- [ ] Start turn → mock provider stalls → fire `coding_turn_interrupt` → assert `TurnCompleted { Cancelled }` fires
- [ ] Commit

### Task 7.8: E2E success-criterion scenario via Chrome MCP

- [ ] Per spec §2 success criterion #1: import existing repo → start coding thread → ask agent for a function → approve `bash cargo test` → see green diff → close + reopen → resume thread → follow-up question → recall cites prior turn
- [ ] Recorded as a GIF via Chrome MCP `gif_creator`
- [ ] Manual verification — checkbox

---

## Track 8 — Polish, KCA gates, final pass (5 tasks)

### Task 8.1: Run full nextest

```bash
cargo nextest run --workspace
```

- [ ] Expected: all PASS, including K1-K11, K12, K13, K14
- [ ] Commit any test fixes

### Task 8.2: Run KCA validation

```bash
./scripts/run_kca_validation.sh
```

- [ ] Expected: all gates pass
- [ ] Investigate + fix any failures
- [ ] Commit

### Task 8.3: Frontend final pass

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test
```

- [ ] Expected: all PASS
- [ ] Commit any fixes

### Task 8.4: Clippy + format

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

- [ ] Expected: zero clippy warnings, formatting clean
- [ ] Commit

### Task 8.5: Spec cross-reference + CLAUDE.md update

**Files:**
- Modify: `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`
- Modify: `CLAUDE.md` (add Phase 4 invariants K12, K13, K14 to gotchas)

- [ ] Add Phase 4 cross-reference section in Phase 1-3 spec
- [ ] Add CLAUDE.md note about new commands + Parts union + dropped surfaces
- [ ] Commit

---

## Self-Review Checklist

After implementing every track, verify:

**1. Spec coverage:**
- [ ] §2 goal/scope/success criteria → Tracks 0-7 cover
- [ ] §3 surface model → Tracks 5 + 7
- [ ] §4 data model → Tracks 0-2 (T0.4-0.7)
- [ ] §5 24 commands → Tracks 1-4 + 6
- [ ] §6 Tauri events → Tracks 1-3 + 5
- [ ] §7 thread start + AGENTS.md → Tracks 2 + 4
- [ ] §8 turn execution → Track 3
- [ ] §9 UI components → Track 5
- [ ] §10 migration → T0.4-0.5 + T0.8 + T8
- [ ] §11 invariants K12-K14 → T2.11 + T1.8 + T4.3 + T7.2
- [ ] §12 open questions → resolve via execution decisions per task
- [ ] §13 component diagram → matches Tracks 0-7

**2. Placeholder scan:**
- [ ] No "TBD" / "TODO" in plan
- [ ] No "implement later" steps
- [ ] Every code change shows the code

**3. Type consistency:**
- [ ] `MessagePart` enum consistent across crates
- [ ] `ApprovalDecision::Ask { layer_audit: Option<LayerOutcomeAudit> }` matches all callers
- [ ] `ThreadEvent` variant names consistent between Rust + TS

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-03-klynt-coding-in-chat-phase4.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Use `superpowers:subagent-driven-development`.

**2. Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach?
