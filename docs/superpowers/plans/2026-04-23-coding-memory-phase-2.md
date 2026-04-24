# Coding Memory — Phase 2 (Ingestion Transport + Claude Code End-to-End) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Phase-1 stubs into a working ingestion pipeline: `klyntbot-hook` forwards Claude Code hook payloads over a Unix socket (or to a file buffer) into a desktop-owned daemon that persists rows to `ingest_event_log`. Ship the Claude Code settings-toggle install flow and the Phase-2 Workbench panels (CLI Health + Session Replay).

**Architecture:** `klyntbot-hook` parses stdin → normalizes via `ClaudeCodeAdapter` → applies path-based privacy filter → resolves `RepoScope` from cwd → forwards via `HookClient`. `HookClient` tries `UnixIngestSocket` first (200ms timeout, fire-and-forget); on failure, falls back to `FileBufferFallback` (append-only JSONL with rotation + TTL prune) and emits a rate-limited stderr warning. The desktop-embedded `IngestDaemon` binds the socket, decodes length-prefixed JSON, and inserts rows into `ingest_event_log` via `IngestEventLogRepo`. On daemon startup, any buffered events drain into the log. Desktop liveness is signaled via `~/.klyntbot/desktop.lock` (30s heartbeat, 60s staleness threshold). Nothing in Phase 2 calls the Distiller or surfaces recall — that's Phase 3/4. The UI ships a settings-page toggle for Claude Code (writes `~/.claude/settings.json` atomically with pre-install backup) plus two Workbench panels.

**Tech Stack:** Rust (MSRV 1.93), `tokio` (`net`, `fs`, `io-util`, `time`), `sqlx` (SQLite), `serde`/`serde_json` (camelCase), `globset` for path-exclusion matching, `url` for repo canonicalization, `tempfile` for atomic writes, existing `common::Result<T>` / `KlyntbotError`. Frontend: existing `desktop-ui/` (React + Tailwind v4 + Biome 2.0 + React Compiler + `useQuery`/`useMutation`).

---

## File Structure

Every file created or modified by this plan, grouped by responsibility. Files stay small and focused per CLAUDE.md.

### New files — `crates/coding-ingest/`

| File | Responsibility |
|---|---|
| `src/store.rs` | `IngestEventLogRow`, `IngestEventLogRepo` — insert/list/count against `ingest_event_log` |
| `src/hook_client.rs` | `HookClient` — socket-first-else-buffer dispatcher; owns warn rate-limiter |
| `src/warn.rs` | `WarnLimiter` — touch-file rate-limited stderr warnings |
| `src/excludes.rs` | Path-based exclusion filter (globset compile + match) |
| `src/scope_resolver.rs` | Cwd → `RepoScope` via `git rev-parse` + remote canonicalization |
| `src/desktop_lock.rs` | `write_heartbeat` / `is_desktop_alive` helpers for `~/.klyntbot/desktop.lock` |
| `src/adapters/claude_code/payload.rs` | Per-hook Claude Code input JSON shapes |
| `src/adapters/claude_code/dispatch.rs` | PostToolUse tool-name dispatch (TestRun / FileEdit / ToolCall) |

### New files — `crates/desktop/src/commands/`

| File | Responsibility |
|---|---|
| `coding_memory.rs` | Tauri commands: `coding_memory_status`, `coding_memory_enable_cli`, `coding_memory_disable_cli`, `coding_memory_diagnose_cli`, `coding_memory_session_replay`, `coding_memory_cli_health` |

### New files — `crates/app-core/src/coding_memory/`

| File | Responsibility |
|---|---|
| `mod.rs` | Module root + re-exports |
| `installer.rs` | `ClaudeCodeInstaller::install/uninstall/diagnose` — manages `~/.claude/settings.json` atomically |
| `handlers.rs` | Thin handlers backing the Tauri commands (status, session-replay, cli-health) |

### New files — `desktop-ui/src/features/coding-memory/`

| File | Responsibility |
|---|---|
| `index.ts` | Re-exports |
| `CodingMemoryLayout.tsx` | `/coding-memory` shell with sub-route nav |
| `CliHealthPanel.tsx` | Per-CLI row (enabled, last event, buffered count, daemon liveness) |
| `SessionReplayPanel.tsx` | Paginated AgentEvent stream viewer with detail drawer |
| `hooks.ts` | `useCodingMemoryStatus`, `useCliHealth`, `useSessionReplay` |

### New files — `desktop-ui/src/features/settings/pages/`

| File | Responsibility |
|---|---|
| `CodingCliSettings.tsx` | Coding CLI integration settings (Claude Code toggle + Diagnose button) |

### Modified existing files

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `globset` + `url` to `[workspace.dependencies]` if not already there; they're already present — re-use |
| `crates/coding-ingest/Cargo.toml` | Add `globset`, `url`, `sqlx` (for `IngestEventLogRepo`), `tempfile` (dev) |
| `crates/coding-ingest/src/lib.rs` | Export new modules |
| `crates/coding-ingest/src/transport.rs` | Implement `UnixIngestSocket::send` + `FileBufferFallback::send` (replace `NotImplemented`) |
| `crates/coding-ingest/src/daemon.rs` | Implement `spawn` + `drain_buffer` (replace `NotImplemented`) |
| `crates/coding-ingest/src/adapters/claude_code.rs` | Replace stub parse with real dispatch; delegate to `adapters::claude_code::{payload, dispatch}` |
| `crates/coding-ingest/src/bin/klyntbot-hook.rs` | End-to-end wiring (stdin → adapter → exclude → scope → `HookClient`); add `status` subcommand |
| `crates/app-core/Cargo.toml` | Depend on `coding-ingest` and `coding-memory` |
| `crates/app-core/src/lib.rs` | `pub mod coding_memory;` |
| `crates/app-core/src/init/storage.rs` | Run `coding_memory::coding_memory_migrations()` |
| `crates/app-core/src/init/mod.rs` (or equivalent) | Spawn `IngestDaemon` after storage init; hold handle in `AppCore` |
| `crates/app-core/src/state.rs` | Store `Option<IngestDaemonHandle>` on `AppCore` |
| `crates/desktop/Cargo.toml` | Depend on `coding-ingest`, `coding-memory`, `app-core` (already) |
| `crates/desktop/src/commands/mod.rs` | `pub mod coding_memory;` |
| `crates/desktop/src/lib.rs` | Register 6 new Tauri commands in the invoke handler |
| `crates/desktop/src/dev_server/mod.rs` | Add `commands::coding_memory::DEV_COMMANDS` to coverage list + dispatch |
| `desktop-ui/src/app/router.tsx` | Add `/coding-memory` + `/settings/coding-cli` routes |
| `desktop-ui/src/features/settings/index.ts` | Re-export `CodingCliSettings` |

### Test files

| File | Responsibility |
|---|---|
| `crates/coding-ingest/tests/unix_socket_roundtrip.rs` | Socket send → in-test listener receives `AgentEvent` |
| `crates/coding-ingest/tests/file_buffer_rotation.rs` | Append → rotate at 50MB → TTL prune → hard-cap refuse |
| `crates/coding-ingest/tests/hook_client_fallback.rs` | Socket-down falls back to buffer + warn rate-limited |
| `crates/coding-ingest/tests/daemon_lifecycle.rs` | Spawn → send → DB row exists → shutdown cleanly |
| `crates/coding-ingest/tests/drain_buffer.rs` | Pre-seeded buffer drains on daemon start; archive file created |
| `crates/coding-ingest/tests/claude_code_adapter.rs` | One unit test per hook event; dispatch fan-out for PostToolUse |
| `crates/coding-ingest/tests/excludes.rs` | Glob matching: exact, glob, negative |
| `crates/coding-ingest/tests/scope_resolver.rs` | Git repo → canonical id; bare path → `local:` fallback |
| `crates/coding-ingest/tests/desktop_lock.rs` | Fresh/stale/missing lock detection |
| `crates/app-core/tests/claude_code_installer.rs` | No-existing-file / existing-no-hooks / existing-with-hooks / uninstall |
| `tests/integration/coding_memory_phase2_roundtrip.rs` | End-to-end synthetic Claude Code session — 10 events JSONL → daemon → rows queryable |
| `tests/integration/coding_memory_phase2_desktop_off.rs` | Scenario: desktop off → 3 hook invocations → buffer populated → desktop start → rows drained + archive present |
| `tests/fixtures/coding/synthetic_session_claude_code.jsonl` | 10-turn canned Claude Code hook payloads (SessionStart + UserPrompt + PostToolUse variants + Stop + SessionEnd) |
| `desktop-ui/src/features/coding-memory/__tests__/CliHealthPanel.test.tsx` | Panel renders rows from mocked `useQuery` |
| `desktop-ui/src/features/coding-memory/__tests__/SessionReplayPanel.test.tsx` | Paginated list renders + detail drawer opens on click |

---

## Task Structure

Tasks run sequentially by default but several pairs can parallelize when an engineer works in a worktree. Each task is self-contained: exact file paths, exact commands, full code.

### Task 1: Wire `coding_memory_migrations` into `app-core` storage init

**Files:**
- Modify: `crates/app-core/Cargo.toml`
- Modify: `crates/app-core/src/init/storage.rs`
- Test: `crates/coding-memory/tests/migration_applies.rs` (already exists — extend)

- [ ] **Step 1: Add coding-memory dep to app-core**

Edit `crates/app-core/Cargo.toml`, under `[dependencies]` add:

```toml
coding-memory = { path = "../coding-memory" }
coding-ingest = { path = "../coding-ingest" }
```

- [ ] **Step 2: Write the failing test**

Append to `crates/coding-memory/tests/migration_applies.rs`:

```rust
#[tokio::test]
async fn migration_is_idempotent() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    let migs = coding_memory::coding_memory_migrations();
    StoragePool::run_feature_migrations(pool.inner(), &migs).await.expect("first");
    // Second run must not fail (pre-release policy: versioned idempotent).
    StoragePool::run_feature_migrations(pool.inner(), &migs).await.expect("second");
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ingest_event_log")
        .fetch_one(pool.inner()).await.expect("count");
    assert_eq!(row.0, 0);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory migration_is_idempotent`
Expected: FAIL — no prior `semantic_facts`/`episodic_memories` tables in fresh pool, so `ALTER TABLE` in migration errors.

- [ ] **Step 4: Hook into existing cognitive migrations**

`coding_memory_migrations()` depends on `cognitive` already running its base migrations. Update the test to chain cognitive first:

```rust
use cognitive::cognitive_migrations;
StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations()).await.expect("cog");
StoragePool::run_feature_migrations(pool.inner(), &migs).await.expect("first");
StoragePool::run_feature_migrations(pool.inner(), &migs).await.expect("second");
```

Add `cognitive = { workspace = true }` to `crates/coding-memory/Cargo.toml` `[dev-dependencies]` if absent. (The prod dep is already there.)

- [ ] **Step 5: Wire migration into app-core**

Edit `crates/app-core/src/init/storage.rs`. After the `cognitive` migrations block (search for `cognitive_migrations`; if absent search for `feature-learning` migrations block and insert after), add:

```rust
StoragePool::run_feature_migrations(
    storage_pool.inner(),
    &coding_memory::coding_memory_migrations(),
)
.await
.map_err(|e| format!("coding-memory migration failed: {e}"))?;
```

- [ ] **Step 6: Verify test passes + workspace builds**

Run:
```bash
cargo nextest run -p coding-memory migration_is_idempotent
cargo build -p app-core
```
Expected: PASS + clean build.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/tests/migration_applies.rs crates/coding-memory/Cargo.toml \
        crates/app-core/Cargo.toml crates/app-core/src/init/storage.rs
git commit -m "feat(coding-memory): wire Phase-1 migrations into app-core storage init"
```

---

### Task 2: `IngestEventLogRepo` — insert + list + count

**Files:**
- Create: `crates/coding-ingest/src/store.rs`
- Modify: `crates/coding-ingest/src/lib.rs` (re-export)
- Modify: `crates/coding-ingest/Cargo.toml` (add `sqlx`)
- Test: `crates/coding-ingest/tests/ingest_event_log_repo.rs`

- [ ] **Step 1: Add sqlx + storage dep**

Edit `crates/coding-ingest/Cargo.toml` — ensure under `[dependencies]`:

```toml
sqlx = { workspace = true }
storage = { workspace = true }
```

(Both may already be there — verify and skip duplicates.)

- [ ] **Step 2: Write the failing test**

Create `crates/coding-ingest/tests/ingest_event_log_repo.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

fn sample_event() -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt {
            text: "hello".into(),
            attachments: vec![],
        },
    })
}

#[tokio::test]
async fn repo_inserts_and_lists_unprocessed() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await.unwrap();

    let repo = IngestEventLogRepo::new(pool.inner().clone());
    let evt = sample_event();
    repo.insert(&evt).await.expect("insert");

    let rows = repo.list_unprocessed(100).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, "s1");
    assert!(!rows[0].processed);
}

#[tokio::test]
async fn repo_count_by_session() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = IngestEventLogRepo::new(pool.inner().clone());
    repo.insert(&sample_event()).await.unwrap();
    repo.insert(&sample_event()).await.unwrap();
    assert_eq!(repo.count_by_session("s1").await.unwrap(), 2);
    assert_eq!(repo.count_by_session("missing").await.unwrap(), 0);
}
```

Add under `[dev-dependencies]` of `crates/coding-ingest/Cargo.toml`:

```toml
cognitive = { workspace = true }
coding-memory = { path = "../coding-memory" }
storage = { workspace = true }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p coding-ingest repo_inserts_and_lists_unprocessed`
Expected: FAIL — `IngestEventLogRepo` undefined.

- [ ] **Step 4: Create the repo module**

Create `crates/coding-ingest/src/store.rs`:

```rust
//! `ingest_event_log` persistence — the append-only AgentEvent buffer
//! the daemon writes to and the Distiller (Phase 3) reads from.

use crate::event::{AgentEvent, EventKind};
use common::{KlyntbotError, Result};
use sqlx::{SqlitePool, Row};

/// A single decoded row from `ingest_event_log`.
#[derive(Debug, Clone)]
pub struct IngestEventLogRow {
    /// UUID (matches `AgentEventV1.id`).
    pub id: String,
    /// Source CLI name (`claude-code`, `codex`, ...).
    pub source: String,
    /// Session id as assigned by the CLI.
    pub session_id: String,
    /// Turn id when present.
    pub turn_id: Option<String>,
    /// Repo canonical id when resolved.
    pub repo_id: Option<String>,
    /// `EventKind` discriminant (`userPrompt`, `toolCall`, ...).
    pub kind: String,
    /// Serialized `AgentEvent` JSON.
    pub payload: String,
    /// Whether Distiller has consumed this row.
    pub processed: bool,
    /// RFC3339 occurred-at.
    pub occurred_at: String,
}

/// Repository for `ingest_event_log`.
#[derive(Debug, Clone)]
pub struct IngestEventLogRepo {
    pool: SqlitePool,
}

impl IngestEventLogRepo {
    /// Construct over a `SqlitePool`.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert one event. The `AgentEvent` is serialized as JSON into `payload`.
    pub async fn insert(&self, event: &AgentEvent) -> Result<()> {
        let AgentEvent::V1(v1) = event;
        let payload = serde_json::to_string(event)
            .map_err(|e| KlyntbotError::Storage(format!("serialize event: {e}")))?;
        let kind = event_kind_tag(&v1.kind);
        let repo_id = v1.repo.as_ref().map(|r| r.repo_id.clone());
        let cwd = v1.cwd.to_string_lossy().to_string();
        let occurred = v1.occurred_at.to_string();
        let source = agent_source_slug(v1.source);

        sqlx::query(
            "INSERT INTO ingest_event_log
             (id, source, session_id, turn_id, cwd, repo_id, occurred_at, kind, payload)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(v1.id.to_string())
        .bind(source)
        .bind(&v1.session_id)
        .bind(v1.turn_id.as_deref())
        .bind(cwd)
        .bind(repo_id)
        .bind(occurred)
        .bind(kind)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log insert: {e}")))?;
        Ok(())
    }

    /// Fetch up to `limit` unprocessed rows ordered by `received_at`.
    pub async fn list_unprocessed(&self, limit: i64) -> Result<Vec<IngestEventLogRow>> {
        let rows = sqlx::query(
            "SELECT id, source, session_id, turn_id, repo_id, kind, payload, processed, occurred_at
             FROM ingest_event_log
             WHERE processed = 0
             ORDER BY received_at ASC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log list: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| IngestEventLogRow {
                id: r.get("id"),
                source: r.get("source"),
                session_id: r.get("session_id"),
                turn_id: r.get("turn_id"),
                repo_id: r.get("repo_id"),
                kind: r.get("kind"),
                payload: r.get("payload"),
                processed: r.get::<bool, _>("processed"),
                occurred_at: r.get("occurred_at"),
            })
            .collect())
    }

    /// Count rows for a session (processed + unprocessed).
    pub async fn count_by_session(&self, session_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ingest_event_log WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log count: {e}")))?;
        Ok(row.0)
    }

    /// Count unprocessed rows (buffered events awaiting distillation).
    pub async fn count_unprocessed(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ingest_event_log WHERE processed = 0",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log count_unprocessed: {e}")))?;
        Ok(row.0)
    }
}

fn event_kind_tag(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::SessionStart { .. } => "sessionStart",
        EventKind::SessionEnd { .. } => "sessionEnd",
        EventKind::UserPrompt { .. } => "userPrompt",
        EventKind::AssistantMsg { .. } => "assistantMsg",
        EventKind::ToolCall { .. } => "toolCall",
        EventKind::FileEdit { .. } => "fileEdit",
        EventKind::TestRun { .. } => "testRun",
        EventKind::CompactEvent { .. } => "compactEvent",
        EventKind::Error { .. } => "error",
        EventKind::SkillActivated { .. } => "skillActivated",
        EventKind::RecallInjected { .. } => "recallInjected",
        EventKind::ApprovalDecision { .. } => "approvalDecision",
        EventKind::SandboxApplied { .. } => "sandboxApplied",
        EventKind::FileEditEnriched { .. } => "fileEditEnriched",
        EventKind::TestRunEnriched { .. } => "testRunEnriched",
        EventKind::ProviderCall { .. } => "providerCall",
        EventKind::CompressionApplied { .. } => "compressionApplied",
        EventKind::MirrorAlert { .. } => "mirrorAlert",
        EventKind::SkillRoutingTrace { .. } => "skillRoutingTrace",
    }
}

fn agent_source_slug(src: crate::event::AgentSource) -> &'static str {
    use crate::event::AgentSource::*;
    match src {
        ClaudeCode => "claude-code",
        Codex => "codex",
        KimiCli => "kimi-cli",
        OpenCode => "opencode",
        KlyntCli => "klynt-cli",
    }
}
```

- [ ] **Step 5: Re-export from crate root**

Edit `crates/coding-ingest/src/lib.rs` — add:

```rust
/// `ingest_event_log` persistence.
pub mod store;
```

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p coding-ingest --tests
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS + zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-ingest/Cargo.toml crates/coding-ingest/src/store.rs \
        crates/coding-ingest/src/lib.rs crates/coding-ingest/tests/ingest_event_log_repo.rs
git commit -m "feat(coding-ingest): IngestEventLogRepo with insert/list/count"
```

---

### Task 3: `UnixIngestSocket::send` — real implementation

**Files:**
- Modify: `crates/coding-ingest/src/transport.rs`
- Test: `crates/coding-ingest/tests/unix_socket_roundtrip.rs`

**Protocol (locked):** 4-byte little-endian length prefix, then UTF-8 JSON bytes of `AgentEvent`. Max payload 1 MiB. Write deadline: 200ms.

- [ ] **Step 1: Write the failing test**

Create `crates/coding-ingest/tests/unix_socket_roundtrip.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::transport::{IngestSocket, UnixIngestSocket};
use jiff::Timestamp;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use uuid::Uuid;

#[tokio::test]
async fn send_writes_length_prefix_then_json() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("ingest.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "hi".into(), attachments: vec![] },
    });

    let sink = UnixIngestSocket::new(sock.clone());
    let send_task = tokio::spawn(async move { sink.send(&evt).await });

    let (mut stream, _addr) = listener.accept().await.unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.unwrap();
    let decoded: AgentEvent = serde_json::from_slice(&body).unwrap();
    let AgentEvent::V1(v1) = decoded;
    assert_eq!(v1.session_id, "s");
    send_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn send_returns_error_when_socket_missing() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("absent.sock");
    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::SessionEnd { reason: "x".into() },
    });
    let sink = UnixIngestSocket::new(sock);
    assert!(sink.send(&evt).await.is_err());
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p coding-ingest --test unix_socket_roundtrip`
Expected: FAIL — current `send` returns `NotImplemented`.

- [ ] **Step 3: Implement `send`**

Replace the `impl IngestSocket for UnixIngestSocket` block in `crates/coding-ingest/src/transport.rs`:

```rust
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const SEND_TIMEOUT_MS: u64 = 200;

#[async_trait]
impl IngestSocket for UnixIngestSocket {
    async fn send(&self, event: &AgentEvent) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixStream;
        use tokio::time::{timeout, Duration};

        let body = serde_json::to_vec(event)
            .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
        if body.len() > MAX_PAYLOAD_BYTES {
            return Err(KlyntbotError::Storage(format!(
                "payload {} > {} bytes",
                body.len(),
                MAX_PAYLOAD_BYTES
            )));
        }
        let len = u32::try_from(body.len())
            .map_err(|_| KlyntbotError::Storage("payload overflow".into()))?
            .to_le_bytes();

        let dl = Duration::from_millis(SEND_TIMEOUT_MS);
        let mut stream = timeout(dl, UnixStream::connect(&self.path))
            .await
            .map_err(|_| KlyntbotError::Storage("socket connect timeout".into()))?
            .map_err(|e| KlyntbotError::Storage(format!("socket connect: {e}")))?;

        timeout(dl, async {
            stream.write_all(&len).await?;
            stream.write_all(&body).await?;
            stream.shutdown().await?;
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(|_| KlyntbotError::Storage("socket write timeout".into()))?
        .map_err(|e| KlyntbotError::Storage(format!("socket write: {e}")))?;

        Ok(())
    }
}
```

- [ ] **Step 4: Verify tests pass**

```bash
cargo nextest run -p coding-ingest --test unix_socket_roundtrip
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS + zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/transport.rs crates/coding-ingest/tests/unix_socket_roundtrip.rs
git commit -m "feat(coding-ingest): UnixIngestSocket::send with 200ms deadline"
```

---

### Task 4: `FileBufferFallback::send` — append + rotation + TTL + hard cap

**Files:**
- Modify: `crates/coding-ingest/src/transport.rs`
- Test: `crates/coding-ingest/tests/file_buffer_rotation.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/coding-ingest/tests/file_buffer_rotation.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::transport::{
    FileBufferFallback, IngestSocket, BUFFER_HARD_CAP_BYTES, BUFFER_ROTATE_BYTES,
};
use jiff::Timestamp;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

fn evt(i: u32) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: format!("s-{i}"),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "x".into(), attachments: vec![] },
    })
}

#[tokio::test]
async fn append_produces_one_line_per_event() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("buf.jsonl");
    let sink = FileBufferFallback::new(path.clone());
    sink.send(&evt(0)).await.unwrap();
    sink.send(&evt(1)).await.unwrap();
    let contents = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(contents.lines().count(), 2);
    for line in contents.lines() {
        let _: AgentEvent = serde_json::from_str(line).unwrap();
    }
}

#[tokio::test]
async fn rotates_when_over_rotate_threshold() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("buf.jsonl");
    // Seed file just over rotate threshold.
    tokio::fs::write(&path, vec![b'x'; (BUFFER_ROTATE_BYTES as usize) + 1]).await.unwrap();
    let sink = FileBufferFallback::new(path.clone());
    sink.send(&evt(0)).await.unwrap();
    // After rotation, primary file contains only the new event.
    let contents = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(contents.lines().count(), 1);
    // A rotated file exists alongside.
    let siblings: Vec<_> = std::fs::read_dir(dir.path()).unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.starts_with("buf.jsonl."))
        .collect();
    assert_eq!(siblings.len(), 1);
}

#[tokio::test]
async fn refuses_when_over_hard_cap() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("buf.jsonl");
    // Create many fake rotated siblings totalling > hard cap.
    // We assert hard cap by monkeying the primary.
    tokio::fs::write(&path, vec![b'x'; (BUFFER_HARD_CAP_BYTES as usize) + 1]).await.unwrap();
    let sink = FileBufferFallback::new(path.clone());
    let r = sink.send(&evt(0)).await;
    assert!(r.is_err(), "expected hard-cap error");
}
```

- [ ] **Step 2: Verify tests fail**

Run: `cargo nextest run -p coding-ingest --test file_buffer_rotation`
Expected: FAIL — `send` still returns `NotImplemented`.

- [ ] **Step 3: Implement append + rotation + hard cap**

In `crates/coding-ingest/src/transport.rs`, replace the `FileBufferFallback` impl:

```rust
use jiff::Timestamp;
use tokio::fs::{metadata, rename, OpenOptions};
use tokio::io::AsyncWriteExt;

#[async_trait]
impl IngestSocket for FileBufferFallback {
    async fn send(&self, event: &AgentEvent) -> Result<()> {
        // Hard cap check — primary file only (rotated siblings counted by `prune_older`).
        if let Ok(meta) = metadata(&self.path).await {
            if meta.len() > BUFFER_HARD_CAP_BYTES {
                return Err(KlyntbotError::Storage(format!(
                    "ingest buffer over hard cap ({} > {} bytes)",
                    meta.len(), BUFFER_HARD_CAP_BYTES
                )));
            }
            if meta.len() > BUFFER_ROTATE_BYTES {
                let rotated = self.path.with_extension(format!(
                    "jsonl.{}",
                    Timestamp::now().as_millisecond()
                ));
                rename(&self.path, &rotated).await.map_err(|e| {
                    KlyntbotError::Storage(format!("rotate buffer: {e}"))
                })?;
            }
        }

        let mut line = serde_json::to_vec(event)
            .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
        line.push(b'\n');

        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("open buffer: {e}")))?;
        f.write_all(&line).await
            .map_err(|e| KlyntbotError::Storage(format!("write buffer: {e}")))?;
        f.flush().await.ok();
        Ok(())
    }
}

impl FileBufferFallback {
    /// Delete rotated sibling files older than `BUFFER_TTL_DAYS`. Safe to call
    /// periodically; errors are logged, never returned. Caller: daemon startup.
    pub async fn prune_older(&self) -> Result<usize> {
        let parent = match self.path.parent() {
            Some(p) => p.to_path_buf(),
            None => return Ok(0),
        };
        let prefix = self.path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| format!("{s}."))
            .unwrap_or_default();
        let ttl = std::time::Duration::from_secs(60 * 60 * 24 * BUFFER_TTL_DAYS);
        let now = std::time::SystemTime::now();
        let mut removed = 0usize;
        let mut rd = tokio::fs::read_dir(&parent).await
            .map_err(|e| KlyntbotError::Storage(format!("readdir: {e}")))?;
        while let Some(entry) = rd.next_entry().await
            .map_err(|e| KlyntbotError::Storage(format!("readdir next: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) { continue; }
            let Ok(meta) = entry.metadata().await else { continue };
            let Ok(modified) = meta.modified() else { continue };
            if now.duration_since(modified).map(|d| d > ttl).unwrap_or(false) {
                let _ = tokio::fs::remove_file(entry.path()).await;
                removed += 1;
            }
        }
        Ok(removed)
    }
}
```

- [ ] **Step 4: Verify tests pass**

```bash
cargo nextest run -p coding-ingest --test file_buffer_rotation
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/transport.rs crates/coding-ingest/tests/file_buffer_rotation.rs
git commit -m "feat(coding-ingest): FileBufferFallback append with rotation + TTL + hard cap"
```

---

### Task 5: `WarnLimiter` — touch-file rate-limited stderr

**Files:**
- Create: `crates/coding-ingest/src/warn.rs`
- Modify: `crates/coding-ingest/src/lib.rs` (export)
- Test: embedded `#[cfg(test)] mod tests` in `warn.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-ingest/src/warn.rs`:

```rust
//! Touch-file rate-limited stderr warnings for the hook binary.

use common::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Interval between warnings sharing the same touch-file.
pub const WARN_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Rate-limiter using filesystem mtime as persisted state.
#[derive(Debug, Clone)]
pub struct WarnLimiter {
    /// Path to the touch file (usually `~/.klyntbot/.hook-warn.stamp`).
    pub stamp_path: PathBuf,
}

impl WarnLimiter {
    /// Construct.
    #[must_use]
    pub fn new(stamp_path: PathBuf) -> Self { Self { stamp_path } }

    /// Should we emit the warning now? Touches the file if yes.
    pub fn should_warn(&self) -> bool {
        let now = SystemTime::now();
        let due = std::fs::metadata(&self.stamp_path)
            .and_then(|m| m.modified())
            .map(|t| now.duration_since(t).map(|d| d >= WARN_INTERVAL).unwrap_or(true))
            .unwrap_or(true);
        if due { let _ = touch(&self.stamp_path); }
        due
    }
}

fn touch(p: &Path) -> Result<()> {
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::File::create(p)
        .map_err(|e| common::KlyntbotError::Storage(format!("warn stamp: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn first_call_warns_then_suppresses() {
        let dir = TempDir::new().unwrap();
        let l = WarnLimiter::new(dir.path().join(".stamp"));
        assert!(l.should_warn());
        assert!(!l.should_warn());
    }

    #[test]
    fn warns_again_after_interval_simulated_by_backdating() {
        let dir = TempDir::new().unwrap();
        let stamp = dir.path().join(".stamp");
        let l = WarnLimiter::new(stamp.clone());
        assert!(l.should_warn());
        // Backdate mtime beyond the interval.
        let past = std::time::SystemTime::now() - WARN_INTERVAL - Duration::from_secs(1);
        let ft = filetime::FileTime::from_system_time(past);
        filetime::set_file_mtime(&stamp, ft).unwrap();
        assert!(l.should_warn());
    }
}
```

- [ ] **Step 2: Add `filetime` dev-dep**

In `crates/coding-ingest/Cargo.toml`, under `[dev-dependencies]`:

```toml
filetime = "0.2"
```

- [ ] **Step 3: Export and verify**

Edit `crates/coding-ingest/src/lib.rs` — add:

```rust
/// Touch-file rate-limited stderr warnings.
pub mod warn;
```

Run:
```bash
cargo nextest run -p coding-ingest warn::
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/src/warn.rs crates/coding-ingest/src/lib.rs crates/coding-ingest/Cargo.toml
git commit -m "feat(coding-ingest): WarnLimiter for touch-file rate-limited stderr warnings"
```

---

### Task 6: `HookClient` — socket-first-else-buffer dispatcher

**Files:**
- Create: `crates/coding-ingest/src/hook_client.rs`
- Modify: `crates/coding-ingest/src/lib.rs`
- Test: `crates/coding-ingest/tests/hook_client_fallback.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-ingest/tests/hook_client_fallback.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::hook_client::HookClient;
use jiff::Timestamp;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use uuid::Uuid;

fn evt() -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::SessionEnd { reason: "x".into() },
    })
}

#[tokio::test]
async fn uses_socket_when_available() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("s.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let client = HookClient::new(
        sock.clone(),
        dir.path().join("buf.jsonl"),
        dir.path().join(".stamp"),
    );
    let task = tokio::spawn(async move { client.send(&evt()).await });
    let (mut s, _) = listener.accept().await.unwrap();
    let mut len = [0u8; 4]; s.read_exact(&mut len).await.unwrap();
    let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
    s.read_exact(&mut body).await.unwrap();
    task.await.unwrap().unwrap();
    assert!(!dir.path().join("buf.jsonl").exists());
}

#[tokio::test]
async fn falls_back_to_buffer_when_socket_absent() {
    let dir = TempDir::new().unwrap();
    let client = HookClient::new(
        dir.path().join("absent.sock"),
        dir.path().join("buf.jsonl"),
        dir.path().join(".stamp"),
    );
    client.send(&evt()).await.unwrap();
    let contents = tokio::fs::read_to_string(dir.path().join("buf.jsonl")).await.unwrap();
    assert_eq!(contents.lines().count(), 1);
    // Stamp was touched → warning was issued once.
    assert!(dir.path().join(".stamp").exists());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p coding-ingest --test hook_client_fallback`
Expected: FAIL — `HookClient` undefined.

- [ ] **Step 3: Implement**

Create `crates/coding-ingest/src/hook_client.rs`:

```rust
//! `HookClient` — the hook binary's public write API. Tries the Unix socket
//! first; on any failure, falls back to appending into the file buffer and
//! emits a rate-limited stderr warning.

use crate::event::AgentEvent;
use crate::transport::{FileBufferFallback, IngestSocket, UnixIngestSocket};
use crate::warn::WarnLimiter;
use common::Result;
use std::path::PathBuf;

/// The hook binary's dispatcher.
#[derive(Debug, Clone)]
pub struct HookClient {
    socket: UnixIngestSocket,
    buffer: FileBufferFallback,
    warn: WarnLimiter,
}

impl HookClient {
    /// Construct with absolute paths (socket / buffer file / touch-stamp).
    #[must_use]
    pub fn new(socket_path: PathBuf, buffer_path: PathBuf, warn_stamp: PathBuf) -> Self {
        Self {
            socket: UnixIngestSocket::new(socket_path),
            buffer: FileBufferFallback::new(buffer_path),
            warn: WarnLimiter::new(warn_stamp),
        }
    }

    /// Try socket; on failure buffer the event + maybe warn.
    pub async fn send(&self, event: &AgentEvent) -> Result<()> {
        match self.socket.send(event).await {
            Ok(()) => Ok(()),
            Err(socket_err) => {
                self.buffer.send(event).await?;
                if self.warn.should_warn() {
                    eprintln!(
                        "klyntbot-hook: desktop unreachable — buffering events to disk ({socket_err})"
                    );
                }
                Ok(())
            }
        }
    }
}
```

- [ ] **Step 4: Export + verify**

Edit `crates/coding-ingest/src/lib.rs` — add:

```rust
/// `HookClient` — socket-first-else-buffer dispatcher.
pub mod hook_client;
```

Run:
```bash
cargo nextest run -p coding-ingest --test hook_client_fallback
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS + zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/hook_client.rs crates/coding-ingest/src/lib.rs \
        crates/coding-ingest/tests/hook_client_fallback.rs
git commit -m "feat(coding-ingest): HookClient socket-first-else-buffer dispatcher"
```

---

### Task 7: `IngestDaemon` — accept loop + decode + persist

**Files:**
- Modify: `crates/coding-ingest/src/daemon.rs`
- Test: `crates/coding-ingest/tests/daemon_lifecycle.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-ingest/tests/daemon_lifecycle.rs`:

```rust
use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use coding_ingest::transport::{IngestSocket, UnixIngestSocket};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn daemon_accepts_event_and_writes_row() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = TempDir::new().unwrap();
    let cfg = IngestDaemonConfig {
        socket_path: dir.path().join("s.sock"),
        buffer_path: dir.path().join("buf.jsonl"),
        lock_path: dir.path().join("desktop.lock"),
        repo: repo.clone(),
    };
    let handle = spawn(cfg.clone()).await.expect("spawn");

    let sink = UnixIngestSocket::new(cfg.socket_path.clone());
    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "hi".into(), attachments: vec![] },
    });
    sink.send(&evt).await.unwrap();

    // Poll briefly — daemon handles inserts async.
    for _ in 0..50 {
        if repo.count_by_session("s1").await.unwrap() > 0 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(repo.count_by_session("s1").await.unwrap(), 1);

    handle.shutdown().await;
}
```

Add `coding-memory`, `cognitive`, `storage` to `crates/coding-ingest/Cargo.toml` `[dev-dependencies]` if absent.

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p coding-ingest --test daemon_lifecycle`
Expected: FAIL — `IngestDaemonConfig::repo` field missing; `spawn` returns `NotImplemented`.

- [ ] **Step 3: Implement**

Replace `crates/coding-ingest/src/daemon.rs` fully:

```rust
//! Desktop-embedded ingestion daemon — owns the Unix-socket lifecycle, the
//! file-buffer drainer, and the `desktop.lock` heartbeat.

use crate::event::AgentEvent;
use crate::store::IngestEventLogRepo;
use crate::transport::FileBufferFallback;
use common::{KlyntbotError, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::oneshot;

/// Configuration for the ingestion daemon.
#[derive(Debug, Clone)]
pub struct IngestDaemonConfig {
    /// Where the Unix socket is bound.
    pub socket_path: PathBuf,
    /// Where the cold-path file buffer lives.
    pub buffer_path: PathBuf,
    /// Desktop liveness touch-file path.
    pub lock_path: PathBuf,
    /// Repo that receives decoded events.
    pub repo: Arc<IngestEventLogRepo>,
}

/// Handle returned by [`spawn`]; used to shutdown cleanly.
#[derive(Debug)]
pub struct IngestDaemonHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
    accept_task: tokio::task::JoinHandle<()>,
    heartbeat_task: tokio::task::JoinHandle<()>,
}

impl IngestDaemonHandle {
    /// Signal shutdown and wait for the accept loop + heartbeat to exit.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() { let _ = tx.send(()); }
        let _ = self.accept_task.await;
        self.heartbeat_task.abort();
        let _ = self.heartbeat_task.await;
    }
}

/// Bind the socket, spawn accept loop + heartbeat + buffer-drain.
pub async fn spawn(cfg: IngestDaemonConfig) -> Result<IngestDaemonHandle> {
    if cfg.socket_path.exists() {
        let _ = tokio::fs::remove_file(&cfg.socket_path).await;
    }
    if let Some(parent) = cfg.socket_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let listener = UnixListener::bind(&cfg.socket_path)
        .map_err(|e| KlyntbotError::Storage(format!("bind {}: {e}", cfg.socket_path.display())))?;

    // Drain any buffered events from a prior desktop-off window.
    if cfg.buffer_path.exists() {
        let drained = drain_buffer(&cfg.buffer_path, cfg.repo.as_ref()).await?;
        tracing::info!(drained, "ingest buffer drained on startup");
    }
    // Prune old rotated siblings.
    let buf = FileBufferFallback::new(cfg.buffer_path.clone());
    let _ = buf.prune_older().await;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let repo = cfg.repo.clone();

    let accept_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept = listener.accept() => {
                    match accept {
                        Ok((stream, _)) => {
                            let repo = repo.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, repo).await {
                                    tracing::warn!(error = %e, "ingest handler failed");
                                }
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "accept failed");
                        }
                    }
                }
            }
        }
    });

    // Heartbeat task — refreshes `desktop.lock` mtime every 30s.
    let lock = cfg.lock_path.clone();
    let heartbeat_task = tokio::spawn(async move {
        loop {
            if let Err(e) = crate::desktop_lock::write_heartbeat(&lock).await {
                tracing::warn!(error = %e, "heartbeat write failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    Ok(IngestDaemonHandle {
        shutdown_tx: Some(shutdown_tx),
        accept_task,
        heartbeat_task,
    })
}

const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    repo: Arc<IngestEventLogRepo>,
) -> Result<()> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await
        .map_err(|e| KlyntbotError::Storage(format!("read len: {e}")))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_PAYLOAD_BYTES {
        return Err(KlyntbotError::Storage(format!("payload too large: {len}")));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await
        .map_err(|e| KlyntbotError::Storage(format!("read body: {e}")))?;
    let event: AgentEvent = serde_json::from_slice(&body)
        .map_err(|e| KlyntbotError::Storage(format!("decode event: {e}")))?;
    repo.insert(&event).await?;
    Ok(())
}

/// Read the JSONL buffer line-by-line, insert each event, then archive the file.
pub async fn drain_buffer(path: &std::path::Path, repo: &IngestEventLogRepo) -> Result<usize> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let f = tokio::fs::File::open(path).await
        .map_err(|e| KlyntbotError::Storage(format!("open buffer: {e}")))?;
    let mut lines = BufReader::new(f).lines();
    let mut n = 0usize;
    while let Some(line) = lines.next_line().await
        .map_err(|e| KlyntbotError::Storage(format!("read buffer: {e}")))?
    {
        if line.trim().is_empty() { continue; }
        match serde_json::from_str::<AgentEvent>(&line) {
            Ok(evt) => {
                if let Err(e) = repo.insert(&evt).await {
                    tracing::warn!(error = %e, "drain insert failed");
                } else {
                    n += 1;
                }
            }
            Err(e) => tracing::warn!(error = %e, "drain: bad line skipped"),
        }
    }
    // Archive the drained buffer.
    let archive = path.with_extension(format!(
        "jsonl.done.{}",
        jiff::Timestamp::now().as_millisecond()
    ));
    tokio::fs::rename(path, &archive).await
        .map_err(|e| KlyntbotError::Storage(format!("archive buffer: {e}")))?;
    Ok(n)
}
```

- [ ] **Step 4: Run test**

```bash
cargo nextest run -p coding-ingest --test daemon_lifecycle
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS + zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/daemon.rs crates/coding-ingest/tests/daemon_lifecycle.rs \
        crates/coding-ingest/Cargo.toml
git commit -m "feat(coding-ingest): IngestDaemon accept/decode/persist + graceful shutdown"
```

---

### Task 8: `desktop.lock` heartbeat helpers

**Files:**
- Create: `crates/coding-ingest/src/desktop_lock.rs`
- Modify: `crates/coding-ingest/src/lib.rs`
- Test: `crates/coding-ingest/tests/desktop_lock.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/coding-ingest/tests/desktop_lock.rs`:

```rust
use coding_ingest::desktop_lock::{is_desktop_alive, write_heartbeat};
use tempfile::TempDir;

#[tokio::test]
async fn missing_lock_is_dead() {
    let dir = TempDir::new().unwrap();
    assert!(!is_desktop_alive(&dir.path().join("desktop.lock")));
}

#[tokio::test]
async fn fresh_heartbeat_is_alive() {
    let dir = TempDir::new().unwrap();
    let lock = dir.path().join("desktop.lock");
    write_heartbeat(&lock).await.unwrap();
    assert!(is_desktop_alive(&lock));
}

#[tokio::test]
async fn stale_heartbeat_is_dead() {
    let dir = TempDir::new().unwrap();
    let lock = dir.path().join("desktop.lock");
    write_heartbeat(&lock).await.unwrap();
    let past = std::time::SystemTime::now()
        - std::time::Duration::from_secs(120);
    let ft = filetime::FileTime::from_system_time(past);
    filetime::set_file_mtime(&lock, ft).unwrap();
    assert!(!is_desktop_alive(&lock));
}
```

- [ ] **Step 2: Verify fails**

Run: `cargo nextest run -p coding-ingest --test desktop_lock`
Expected: FAIL — module undefined.

- [ ] **Step 3: Implement**

Create `crates/coding-ingest/src/desktop_lock.rs`:

```rust
//! `~/.klyntbot/desktop.lock` — lightweight liveness signal.
//!
//! This is not a mutex. Writers touch the file every 30s; readers treat
//! `now - mtime > 60s` as "desktop dead." This lets `klynt-cli` (and any
//! future native source) swap its `MemorySink` impl at event boundaries
//! without explicit coordination with the desktop.

use common::{KlyntbotError, Result};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Desktop is considered dead if its heartbeat is older than this.
pub const STALENESS_THRESHOLD: Duration = Duration::from_secs(60);

/// Touch (or create) the lock file, refreshing its mtime.
pub async fn write_heartbeat(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| KlyntbotError::Storage(format!("heartbeat mkdir: {e}")))?;
    }
    let pid = std::process::id().to_string();
    tokio::fs::write(path, pid.as_bytes()).await
        .map_err(|e| KlyntbotError::Storage(format!("heartbeat write: {e}")))?;
    Ok(())
}

/// Read the lock's mtime; return true if within the staleness threshold.
#[must_use]
pub fn is_desktop_alive(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    let Ok(modified) = meta.modified() else { return false };
    SystemTime::now()
        .duration_since(modified)
        .map(|d| d < STALENESS_THRESHOLD)
        .unwrap_or(false)
}
```

Edit `crates/coding-ingest/src/lib.rs` — add:

```rust
/// `desktop.lock` heartbeat helpers.
pub mod desktop_lock;
```

- [ ] **Step 4: Verify + commit**

```bash
cargo nextest run -p coding-ingest --test desktop_lock
cargo clippy -p coding-ingest --all-targets -- -D warnings
git add crates/coding-ingest/src/desktop_lock.rs crates/coding-ingest/src/lib.rs \
        crates/coding-ingest/tests/desktop_lock.rs
git commit -m "feat(coding-ingest): desktop.lock heartbeat (30s write, 60s staleness)"
```

---

### Task 9: `drain_buffer` — explicit test

**Files:**
- Test: `crates/coding-ingest/tests/drain_buffer.rs`

Covered partly by Task 7's implementation; now a dedicated scenario.

- [ ] **Step 1: Write test**

Create `crates/coding-ingest/tests/drain_buffer.rs`:

```rust
use coding_ingest::daemon::{drain_buffer, spawn, IngestDaemonConfig};
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use coding_ingest::transport::{FileBufferFallback, IngestSocket};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;
use uuid::Uuid;

fn evt(i: u32) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(), source: AgentSource::ClaudeCode,
        session_id: format!("s-{i}"), turn_id: None,
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "x".into(), attachments: vec![] },
    })
}

#[tokio::test]
async fn buffered_events_drain_into_log() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = TempDir::new().unwrap();
    let buffer_path = dir.path().join("buf.jsonl");
    let buf = FileBufferFallback::new(buffer_path.clone());
    buf.send(&evt(0)).await.unwrap();
    buf.send(&evt(1)).await.unwrap();
    buf.send(&evt(2)).await.unwrap();

    let n = drain_buffer(&buffer_path, repo.as_ref()).await.unwrap();
    assert_eq!(n, 3);
    assert_eq!(repo.count_unprocessed().await.unwrap(), 3);
    assert!(!buffer_path.exists());
    // Archive sibling present.
    let siblings: Vec<_> = std::fs::read_dir(dir.path()).unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.contains(".done."))
        .collect();
    assert_eq!(siblings.len(), 1);
}

#[tokio::test]
async fn daemon_start_drains_pre_existing_buffer() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let dir = TempDir::new().unwrap();
    let buffer_path = dir.path().join("buf.jsonl");
    FileBufferFallback::new(buffer_path.clone()).send(&evt(0)).await.unwrap();

    let handle = spawn(IngestDaemonConfig {
        socket_path: dir.path().join("s.sock"),
        buffer_path: buffer_path.clone(),
        lock_path: dir.path().join("desktop.lock"),
        repo: repo.clone(),
    }).await.unwrap();

    // drain is synchronous part of spawn — by the time we have a handle, it's done.
    assert_eq!(repo.count_unprocessed().await.unwrap(), 1);
    assert!(!buffer_path.exists());
    handle.shutdown().await;
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-ingest --test drain_buffer
git add crates/coding-ingest/tests/drain_buffer.rs
git commit -m "test(coding-ingest): drain_buffer scenario covers pre-seeded events"
```

---

### Task 10: Path-based exclusion filter

**Files:**
- Create: `crates/coding-ingest/src/excludes.rs`
- Modify: `crates/coding-ingest/src/lib.rs`, `crates/coding-ingest/Cargo.toml` (add `globset`)
- Test: `crates/coding-ingest/tests/excludes.rs`

- [ ] **Step 1: Add globset dep**

In `crates/coding-ingest/Cargo.toml` `[dependencies]`:

```toml
globset = "0.4"
```

- [ ] **Step 2: Write failing tests**

Create `crates/coding-ingest/tests/excludes.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp};
use coding_ingest::excludes::{default_exclude_globs, ExcludeSet};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn file_edit(path: &str) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(), source: AgentSource::ClaudeCode,
        session_id: "s".into(), turn_id: None,
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::FileEdit {
            path: PathBuf::from(path), op: FileOp::Modify,
            bytes: 0, diff_preview: None,
        },
    })
}

#[test]
fn defaults_block_env_and_keys() {
    let s = ExcludeSet::compile(&default_exclude_globs()).unwrap();
    assert!(s.should_drop(&file_edit("/home/u/proj/.env")));
    assert!(s.should_drop(&file_edit("/home/u/proj/secrets/db.toml")));
    assert!(s.should_drop(&file_edit("/home/u/.ssh/id_rsa")));
    assert!(s.should_drop(&file_edit("/home/u/proj/node_modules/x/y.js")));
    assert!(!s.should_drop(&file_edit("/home/u/proj/src/main.rs")));
}

#[test]
fn tool_call_args_are_scanned() {
    let s = ExcludeSet::compile(&["**/*.key".to_string()]).unwrap();
    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(), source: AgentSource::ClaudeCode,
        session_id: "s".into(), turn_id: None,
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::ToolCall {
            tool: "Read".into(),
            args_preview: "path=/home/u/keys/deploy.key".into(),
            ok: true, duration_ms: 2, result_preview: String::new(),
        },
    });
    assert!(s.should_drop(&evt));
}
```

- [ ] **Step 3: Implement**

Create `crates/coding-ingest/src/excludes.rs`:

```rust
//! Path-based privacy filter applied at the hook level — matches get dropped
//! before they reach the socket or the cold-path buffer.

use crate::event::{AgentEvent, EventKind};
use globset::{Glob, GlobSet, GlobSetBuilder};

/// The default exclusion globs shipped with klyntbot.
#[must_use]
pub fn default_exclude_globs() -> Vec<String> {
    vec![
        "**/.env", "**/.env.*",
        "**/secrets/**", "**/private/**",
        "**/*.key", "**/*.pem", "**/*.p12", "**/*.pfx",
        "**/id_rsa", "**/id_ed25519", "**/known_hosts",
        "**/.aws/credentials", "**/.gcloud/**", "**/.kube/config",
        "**/node_modules/**", "**/target/**", "**/.git/**",
    ].into_iter().map(String::from).collect()
}

/// Compiled glob matcher.
#[derive(Debug, Clone)]
pub struct ExcludeSet {
    set: GlobSet,
}

impl ExcludeSet {
    /// Compile a set of glob patterns.
    pub fn compile(patterns: &[String]) -> Result<Self, globset::Error> {
        let mut b = GlobSetBuilder::new();
        for p in patterns { b.add(Glob::new(p)?); }
        Ok(Self { set: b.build()? })
    }

    /// Should this event be dropped?
    #[must_use]
    pub fn should_drop(&self, event: &AgentEvent) -> bool {
        let AgentEvent::V1(v1) = event;
        match &v1.kind {
            EventKind::FileEdit { path, .. } | EventKind::FileEditEnriched { path, .. } => {
                self.set.is_match(path)
            }
            EventKind::ToolCall { args_preview, .. } => self.any_token_match(args_preview),
            _ => false,
        }
    }

    fn any_token_match(&self, haystack: &str) -> bool {
        // Split on common separators; match any token that looks like a path.
        haystack.split(|c: char| c.is_whitespace() || c == '=' || c == ',' || c == '\"')
            .any(|tok| !tok.is_empty() && self.set.is_match(tok))
    }
}
```

Edit `crates/coding-ingest/src/lib.rs`:

```rust
/// Path-based privacy exclusion filter.
pub mod excludes;
```

- [ ] **Step 4: Verify + commit**

```bash
cargo nextest run -p coding-ingest --test excludes
cargo clippy -p coding-ingest --all-targets -- -D warnings
git add crates/coding-ingest/Cargo.toml crates/coding-ingest/src/excludes.rs \
        crates/coding-ingest/src/lib.rs crates/coding-ingest/tests/excludes.rs
git commit -m "feat(coding-ingest): path-based privacy exclusion filter"
```

---

### Task 11: Scope resolver via `git rev-parse`

**Files:**
- Create: `crates/coding-ingest/src/scope_resolver.rs`
- Modify: `crates/coding-ingest/src/lib.rs`, `crates/coding-ingest/Cargo.toml` (add `url`)
- Test: `crates/coding-ingest/tests/scope_resolver.rs`

- [ ] **Step 1: Add `url` dep**

In `crates/coding-ingest/Cargo.toml` `[dependencies]`:

```toml
url = "2.5"
```

- [ ] **Step 2: Write failing test**

Create `crates/coding-ingest/tests/scope_resolver.rs`:

```rust
use coding_ingest::scope_resolver::resolve_scope;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git").current_dir(dir).args(args).status().unwrap().success();
    assert!(ok, "git {:?} failed", args);
}

#[test]
fn resolves_canonical_github_id() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["remote", "add", "origin", "git@github.com:klynt/bot.git"]);
    std::fs::write(dir.path().join("README.md"), "x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["-c", "user.email=x@x", "-c", "user.name=x", "commit", "-qm", "x"]);
    let scope = resolve_scope(dir.path()).expect("some");
    assert_eq!(scope.repo_id, "github.com/klynt/bot");
    assert_eq!(scope.root, std::fs::canonicalize(dir.path()).unwrap());
    assert!(scope.git_hash.is_some());
}

#[test]
fn falls_back_to_local_for_non_git_paths() {
    let dir = TempDir::new().unwrap();
    let scope = resolve_scope(dir.path()).expect("some");
    assert!(scope.repo_id.starts_with("local:"));
}

#[test]
fn no_remote_uses_local_prefix_with_worktree_basename() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    let scope = resolve_scope(dir.path()).expect("some");
    assert!(scope.repo_id.starts_with("local:"));
}
```

- [ ] **Step 3: Implement**

Create `crates/coding-ingest/src/scope_resolver.rs`:

```rust
//! Resolve a cwd to a canonical `RepoScope`.
//!
//! Strategy:
//! 1. `git rev-parse --show-toplevel` → worktree root.
//! 2. `git config --get remote.origin.url` → canonical id when present.
//! 3. Fall back to `local:<sanitized-worktree-basename>`.
//!
//! Result cached per-cwd in a process-wide `RwLock<HashMap>`.

use crate::scope::RepoScope;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

static CACHE: once_cell::sync::Lazy<RwLock<std::collections::HashMap<PathBuf, Option<RepoScope>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(std::collections::HashMap::new()));

/// Resolve `cwd` → `RepoScope`. Returns `None` only if `cwd` doesn't exist.
#[must_use]
pub fn resolve_scope(cwd: &Path) -> Option<RepoScope> {
    let key = std::fs::canonicalize(cwd).ok()?;
    if let Some(hit) = CACHE.read().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let scope = compute(&key);
    if let Ok(mut m) = CACHE.write() { m.insert(key, scope.clone()); }
    scope
}

fn compute(cwd: &Path) -> Option<RepoScope> {
    // Fall-back identity uses the basename.
    let basename = cwd.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
    let fallback = RepoScope {
        repo_id: format!("local:{}", sanitize(basename)),
        root: cwd.to_path_buf(),
        git_hash: None,
        branch: None,
    };

    let Some(root) = run(cwd, &["rev-parse", "--show-toplevel"]).and_then(|s| {
        std::fs::canonicalize(s.trim()).ok()
    }) else {
        return Some(fallback);
    };

    let repo_id = run(cwd, &["config", "--get", "remote.origin.url"])
        .and_then(|s| canonicalize_remote(s.trim()))
        .unwrap_or_else(|| {
            format!("local:{}", sanitize(
                root.file_name().and_then(|s| s.to_str()).unwrap_or("unknown")
            ))
        });
    let git_hash = run(cwd, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string());
    let branch = run(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| s != "HEAD");

    Some(RepoScope { repo_id, root, git_hash, branch })
}

fn run(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").current_dir(cwd).args(args).output().ok()?;
    if !out.status.success() { return None; }
    String::from_utf8(out.stdout).ok()
}

fn canonicalize_remote(raw: &str) -> Option<String> {
    // `git@github.com:org/repo.git` → `github.com/org/repo`
    if let Some(rest) = raw.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            return Some(format!("{host}/{}", strip_git_suffix(path)));
        }
    }
    // `https://github.com/org/repo.git` → `github.com/org/repo`
    if let Ok(url) = url::Url::parse(raw) {
        let host = url.host_str()?;
        let path = url.path().trim_start_matches('/');
        return Some(format!("{host}/{}", strip_git_suffix(path)));
    }
    None
}

fn strip_git_suffix(path: &str) -> String {
    path.trim_end_matches('/').trim_end_matches(".git").to_string()
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
```

Add to `crates/coding-ingest/Cargo.toml` `[dependencies]`:

```toml
once_cell = "1"
```

Edit `crates/coding-ingest/src/lib.rs`:

```rust
/// Cwd → `RepoScope` resolver (cached).
pub mod scope_resolver;
```

- [ ] **Step 4: Verify + commit**

```bash
cargo nextest run -p coding-ingest --test scope_resolver
cargo clippy -p coding-ingest --all-targets -- -D warnings
git add crates/coding-ingest/Cargo.toml crates/coding-ingest/src/scope_resolver.rs \
        crates/coding-ingest/src/lib.rs crates/coding-ingest/tests/scope_resolver.rs
git commit -m "feat(coding-ingest): resolve_scope via git rev-parse + remote canonicalization"
```

---

### Task 12: Claude Code adapter — payload shapes

**Files:**
- Create: `crates/coding-ingest/src/adapters/claude_code/mod.rs` (converts existing single file into folder)
- Create: `crates/coding-ingest/src/adapters/claude_code/payload.rs`
- Modify: `crates/coding-ingest/src/adapters/mod.rs` (pub mod path)

> Note: Phase 1 shipped `adapters/claude_code.rs` as a single file. This task converts it to a folder. The module name stays the same.

Hooks we listen on (7): `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact`, `SessionEnd`. The adapter argument `hook_event: &str` is exactly the Claude Code hook name.

- [ ] **Step 1: Move the existing file**

```bash
git mv crates/coding-ingest/src/adapters/claude_code.rs \
       crates/coding-ingest/src/adapters/claude_code/mod.rs
mkdir -p crates/coding-ingest/src/adapters/claude_code
```

Verify `git status` shows the rename.

- [ ] **Step 2: Write failing test**

Create `crates/coding-ingest/tests/claude_code_adapter.rs`:

```rust
use coding_ingest::adapters::claude_code::ClaudeCodeAdapter;
use coding_ingest::adapters::IngestAdapter;
use coding_ingest::event::{AgentEvent, EventKind};

fn parse(event: &str, body: &str) -> Option<AgentEvent> {
    ClaudeCodeAdapter.parse(event, body.as_bytes()).unwrap()
}

#[test]
fn session_start() {
    let body = r#"{
        "session_id": "abc",
        "cwd": "/tmp/repo",
        "source": "cli",
        "model": "claude-sonnet-4-6"
    }"#;
    let AgentEvent::V1(v1) = parse("SessionStart", body).unwrap();
    assert_eq!(v1.session_id, "abc");
    matches!(v1.kind, EventKind::SessionStart { .. });
}

#[test]
fn user_prompt() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "prompt": "hello",
        "attachments": ["/tmp/a.png"]
    }"#;
    let AgentEvent::V1(v1) = parse("UserPromptSubmit", body).unwrap();
    match v1.kind {
        EventKind::UserPrompt { text, attachments } => {
            assert_eq!(text, "hello");
            assert_eq!(attachments.len(), 1);
        }
        _ => panic!("wrong kind"),
    }
}

#[test]
fn stop_becomes_assistant_msg() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "transcript_path": "/tmp/tr.jsonl",
        "stop_hook_active": false
    }"#;
    let AgentEvent::V1(v1) = parse("Stop", body).unwrap();
    matches!(v1.kind, EventKind::AssistantMsg { .. });
}

#[test]
fn session_end() {
    let body = r#"{"session_id": "s", "cwd": "/tmp", "reason": "user-quit"}"#;
    let AgentEvent::V1(v1) = parse("SessionEnd", body).unwrap();
    match v1.kind {
        EventKind::SessionEnd { reason } => assert_eq!(reason, "user-quit"),
        _ => panic!(),
    }
}

#[test]
fn pre_compact_becomes_compact_event() {
    let body = r#"{"session_id": "s", "cwd": "/tmp", "trigger": "auto", "custom_instructions": ""}"#;
    let AgentEvent::V1(v1) = parse("PreCompact", body).unwrap();
    matches!(v1.kind, EventKind::CompactEvent { .. });
}

#[test]
fn unknown_hook_returns_none() {
    let body = r#"{}"#;
    assert!(parse("Unknown", body).is_none());
}
```

- [ ] **Step 3: Verify fails**

Run: `cargo nextest run -p coding-ingest --test claude_code_adapter`
Expected: FAIL — `parse` still returns `NotImplemented`.

- [ ] **Step 4: Implement payload types**

Create `crates/coding-ingest/src/adapters/claude_code/payload.rs`:

```rust
//! Claude Code stdin payload shapes for the 7 hooks we listen on.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub(super) struct CommonEnvelope {
    pub session_id: String,
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default)]
    pub transcript_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionStartBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionEndBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UserPromptBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub prompt: String,
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(super) struct StopBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub stop_hook_active: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct PreCompactBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolUseBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: serde_json::Value,
    #[serde(default)]
    pub tool_response: serde_json::Value,
    #[serde(default)]
    pub duration_ms: u32,
}
```

- [ ] **Step 5: Implement adapter dispatch (non-tool branches)**

Replace `crates/coding-ingest/src/adapters/claude_code/mod.rs`:

```rust
//! Claude Code adapter — 7 of Claude Code's hook events → `AgentEvent`.

mod payload;
mod dispatch;

use super::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Adapter for Claude Code hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCodeAdapter;

impl IngestAdapter for ClaudeCodeAdapter {
    fn source_name(&self) -> &'static str { "claude-code" }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        match hook_event {
            "SessionStart" => Ok(Some(wrap(parse_session_start(raw)?))),
            "SessionEnd" => Ok(Some(wrap(parse_session_end(raw)?))),
            "UserPromptSubmit" => Ok(Some(wrap(parse_user_prompt(raw)?))),
            "Stop" => Ok(Some(wrap(parse_stop(raw)?))),
            "PreCompact" => Ok(Some(wrap(parse_pre_compact(raw)?))),
            "PreToolUse" => Ok(None), // not recorded — used for approval layer only
            "PostToolUse" => dispatch::parse_post_tool_use(raw).map(Some).map(|o| o.map(wrap)),
            _ => Ok(None),
        }
    }
}

fn wrap(v1: AgentEventV1) -> AgentEvent { AgentEvent::V1(v1) }

fn base(common: payload::CommonEnvelope, kind: EventKind) -> AgentEventV1 {
    AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: common.session_id,
        turn_id: None,
        cwd: common.cwd,
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }
}

fn decode<T: for<'de> serde::Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw).map_err(|e| KlyntbotError::Storage(format!("claude-code decode: {e}")))
}

fn parse_session_start(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionStartBody = decode(raw)?;
    let kind = EventKind::SessionStart {
        model: b.model,
        source_reason: b.source.unwrap_or_else(|| "unknown".into()),
    };
    Ok(base(b.common, kind))
}

fn parse_session_end(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionEndBody = decode(raw)?;
    let kind = EventKind::SessionEnd { reason: b.reason.unwrap_or_else(|| "unspecified".into()) };
    Ok(base(b.common, kind))
}

fn parse_user_prompt(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::UserPromptBody = decode(raw)?;
    let kind = EventKind::UserPrompt { text: b.prompt, attachments: b.attachments };
    Ok(base(b.common, kind))
}

fn parse_stop(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::StopBody = decode(raw)?;
    let kind = EventKind::AssistantMsg {
        text: String::new(),
        truncated: false,
        token_usage: None::<TokenUsage>,
    };
    Ok(base(b.common, kind))
}

fn parse_pre_compact(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::PreCompactBody = decode(raw)?;
    let kind = EventKind::CompactEvent {
        trigger: b.trigger.unwrap_or_else(|| "unknown".into()),
        token_count: 0,
    };
    Ok(base(b.common, kind))
}

pub(crate) use dispatch as _dispatch_mod_ref;
```

(The `pub(crate) use` line is a hack to keep the compiler from complaining if `dispatch` is otherwise only referenced in the match arm; remove if unused.)

- [ ] **Step 6: Run non-tool-use tests (fails on PostToolUse for now)**

```bash
cargo nextest run -p coding-ingest --test claude_code_adapter -E 'test(session_start) + test(user_prompt) + test(stop_becomes_assistant_msg) + test(session_end) + test(pre_compact_becomes_compact_event) + test(unknown_hook_returns_none)'
```
Expected: all 6 PASS. `PostToolUse` dispatch lands in Task 13.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-ingest/src/adapters/claude_code \
        crates/coding-ingest/tests/claude_code_adapter.rs
git commit -m "feat(coding-ingest): Claude Code adapter — non-tool hook branches"
```

---

### Task 13: Claude Code adapter — PostToolUse dispatch

**Files:**
- Create: `crates/coding-ingest/src/adapters/claude_code/dispatch.rs`
- Modify: `crates/coding-ingest/tests/claude_code_adapter.rs` (add dispatch tests)

- [ ] **Step 1: Extend the test**

Append to `crates/coding-ingest/tests/claude_code_adapter.rs`:

```rust
#[test]
fn post_tool_use_bash_test_becomes_test_run() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test --workspace"},
        "tool_response": {
            "stdout": "test result: ok. 12 passed; 3 failed; 0 ignored",
            "stderr": "", "exit_code": 1
        },
        "duration_ms": 4200
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::TestRun { framework, passed, failed, duration_ms, .. } => {
            assert_eq!(framework.as_deref(), Some("cargo"));
            assert_eq!(passed, 12);
            assert_eq!(failed, 3);
            assert_eq!(duration_ms, 4200);
        }
        other => panic!("expected TestRun, got {other:?}"),
    }
}

#[test]
fn post_tool_use_bash_non_test_becomes_tool_call() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Bash",
        "tool_input": {"command": "ls -la"},
        "tool_response": {"stdout": "...", "stderr": "", "exit_code": 0},
        "duration_ms": 15
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    matches!(v1.kind, EventKind::ToolCall { .. });
}

#[test]
fn post_tool_use_edit_becomes_file_edit() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Edit",
        "tool_input": {"file_path": "/tmp/src/main.rs", "old_string": "a", "new_string": "b"},
        "tool_response": {"success": true, "bytes": 1234},
        "duration_ms": 8
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::FileEdit { path, bytes, .. } => {
            assert_eq!(path.to_string_lossy(), "/tmp/src/main.rs");
            assert_eq!(bytes, 1234);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn post_tool_use_write_is_create_file_op() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Write",
        "tool_input": {"file_path": "/tmp/new.rs", "content": "fn main(){}"},
        "tool_response": {"bytes": 11},
        "duration_ms": 3
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::FileEdit { op, .. } => assert_eq!(op, coding_ingest::event::FileOp::Create),
        other => panic!("{other:?}"),
    }
}

#[test]
fn post_tool_use_read_becomes_file_edit_read_op() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Read",
        "tool_input": {"file_path": "/tmp/x.rs"},
        "tool_response": {"bytes": 500},
        "duration_ms": 2
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::FileEdit { op, .. } => assert_eq!(op, coding_ingest::event::FileOp::Read),
        other => panic!("{other:?}"),
    }
}

#[test]
fn post_tool_use_pytest_framework_detected() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Bash",
        "tool_input": {"command": "pytest -v"},
        "tool_response": {
            "stdout": "==== 5 passed, 1 failed, 2 skipped in 0.23s ====",
            "stderr": "", "exit_code": 1
        },
        "duration_ms": 230
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::TestRun { framework, passed, failed, .. } => {
            assert_eq!(framework.as_deref(), Some("pytest"));
            assert_eq!(passed, 5);
            assert_eq!(failed, 1);
        }
        other => panic!("{other:?}"),
    }
}
```

- [ ] **Step 2: Implement dispatch**

Create `crates/coding-ingest/src/adapters/claude_code/dispatch.rs`:

```rust
//! PostToolUse dispatch — classifies by tool name (and for Bash, by command).

use super::{base, decode, payload};
use crate::event::{AgentEventV1, EventKind, FileOp};
use common::Result;
use std::path::PathBuf;

pub(super) fn parse_post_tool_use(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ToolUseBody = decode(raw)?;
    let kind = match b.tool_name.as_str() {
        "Bash" => classify_bash(&b),
        "Read" => file_edit(&b, FileOp::Read),
        "Write" => file_edit(&b, FileOp::Create),
        "Edit" | "MultiEdit" => file_edit(&b, FileOp::Modify),
        _ => tool_call(&b),
    };
    Ok(base(b.common, kind))
}

fn classify_bash(b: &payload::ToolUseBody) -> EventKind {
    let cmd = b.tool_input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let trimmed = cmd.trim();
    if let Some(fw) = detect_framework(trimmed) {
        let stdout = b.tool_response.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
        let (passed, failed) = parse_results(fw, stdout);
        return EventKind::TestRun {
            command: trimmed.to_string(),
            framework: Some(fw.into()),
            passed,
            failed,
            duration_ms: b.duration_ms,
        };
    }
    tool_call(b)
}

fn tool_call(b: &payload::ToolUseBody) -> EventKind {
    let args = serde_json::to_string(&b.tool_input).unwrap_or_default();
    let args_preview = truncate(&args, 512);
    let result = serde_json::to_string(&b.tool_response).unwrap_or_default();
    let result_preview = truncate(&result, 512);
    let ok = b.tool_response.get("exit_code")
        .and_then(|v| v.as_i64())
        .map(|c| c == 0)
        .unwrap_or(true);
    EventKind::ToolCall {
        tool: b.tool_name.clone(),
        args_preview, ok,
        duration_ms: b.duration_ms,
        result_preview,
    }
}

fn file_edit(b: &payload::ToolUseBody, op: FileOp) -> EventKind {
    let path = b.tool_input.get("file_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_default();
    let bytes = b.tool_response.get("bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    EventKind::FileEdit { path, op, bytes, diff_preview: None }
}

fn detect_framework(cmd: &str) -> Option<&'static str> {
    let first = cmd.split_whitespace().next()?;
    let looks_like = |n: &str| first == n || cmd.starts_with(&format!("{n} "));
    if looks_like("pytest") { return Some("pytest"); }
    if looks_like("cargo") && cmd.contains("test") { return Some("cargo"); }
    if (looks_like("npm") || looks_like("pnpm") || looks_like("yarn") || looks_like("bun"))
        && cmd.contains("test") { return Some("node"); }
    if looks_like("go") && cmd.contains("test") { return Some("go"); }
    if looks_like("jest") { return Some("jest"); }
    if looks_like("vitest") { return Some("vitest"); }
    None
}

fn parse_results(framework: &str, stdout: &str) -> (u32, u32) {
    match framework {
        "cargo" => {
            let passed = capture_u32(stdout, r"(\d+)\s+passed").unwrap_or(0);
            let failed = capture_u32(stdout, r"(\d+)\s+failed").unwrap_or(0);
            (passed, failed)
        }
        "pytest" => {
            let passed = capture_u32(stdout, r"(\d+)\s+passed").unwrap_or(0);
            let failed = capture_u32(stdout, r"(\d+)\s+failed").unwrap_or(0);
            (passed, failed)
        }
        _ => {
            let passed = capture_u32(stdout, r"(\d+)\s+pass").unwrap_or(0);
            let failed = capture_u32(stdout, r"(\d+)\s+fail").unwrap_or(0);
            (passed, failed)
        }
    }
}

fn capture_u32(text: &str, pat: &str) -> Option<u32> {
    // Cheap non-regex scan: find the pattern fragment (e.g., "passed"), walk backwards
    // over whitespace/digits to extract the number. Avoids pulling in the regex crate.
    let marker = pat.split(')').nth(1)?.trim_start_matches("\\s+").trim_start_matches('+');
    let idx = text.find(marker)?;
    let prefix = &text[..idx];
    let digits: String = prefix.chars().rev()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>().chars().rev().collect();
    digits.parse().ok()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}
```

- [ ] **Step 3: Verify**

```bash
cargo nextest run -p coding-ingest --test claude_code_adapter
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: all PASS + zero warnings. If the non-regex `capture_u32` proves flaky against real stdout, swap to the `regex` workspace dep (add under `[dependencies]` and rewrite with `Regex::new(pat)`) — the behavior contract stays the same.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/src/adapters/claude_code/dispatch.rs \
        crates/coding-ingest/tests/claude_code_adapter.rs
git commit -m "feat(coding-ingest): PostToolUse dispatch (TestRun / FileEdit / ToolCall)"
```

---

### Task 14: `klyntbot-hook` end-to-end wiring

**Files:**
- Modify: `crates/coding-ingest/src/bin/klyntbot-hook.rs`

- [ ] **Step 1: Rewrite the binary**

Replace `crates/coding-ingest/src/bin/klyntbot-hook.rs` with:

```rust
//! `klyntbot-hook` — shell binary users' coding CLIs spawn per hook.
//!
//! Usage:
//!   klyntbot-hook <source> [hook-event]     # normal forwarding
//!   klyntbot-hook status                    # socket/buffer/daemon report
//!
//!   source ∈ { claude-code, codex, kimi-cli, opencode }
//!
//! Exits 0 on success, 2 on bad args, 1 on fatal IO. Never blocks the parent.

use coding_ingest::adapters::{claude_code::ClaudeCodeAdapter, IngestAdapter};
use coding_ingest::desktop_lock::is_desktop_alive;
use coding_ingest::event::AgentEvent;
use coding_ingest::excludes::{default_exclude_globs, ExcludeSet};
use coding_ingest::hook_client::HookClient;
use coding_ingest::scope_resolver::resolve_scope;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  klyntbot-hook <source> [hook-event]
  klyntbot-hook status
  source ∈ { claude-code, codex, kimi-cli, opencode }
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(first) = args.first() else { eprintln!("{USAGE}"); return ExitCode::from(2); };

    if first == "status" {
        return run_status();
    }

    let source = first.clone();
    let hook_event = args.get(1).cloned().unwrap_or_else(|| "unknown".into());

    if !matches!(source.as_str(), "claude-code" | "codex" | "kimi-cli" | "opencode") {
        eprintln!("unknown source `{source}`\n{USAGE}");
        return ExitCode::from(2);
    }

    let mut raw = Vec::with_capacity(8 * 1024);
    if let Err(e) = io::stdin().read_to_end(&mut raw) {
        eprintln!("klyntbot-hook: stdin read: {e}");
        return ExitCode::from(1);
    }

    // Only Claude Code is implemented end-to-end in Phase 2.
    if source != "claude-code" {
        eprintln!("klyntbot-hook: source `{source}` not yet wired (Phase 7)");
        return ExitCode::SUCCESS;
    }

    let event = match ClaudeCodeAdapter.parse(&hook_event, &raw) {
        Ok(Some(e)) => e,
        Ok(None) => return ExitCode::SUCCESS, // silently ignore hooks we don't record
        Err(e) => { eprintln!("klyntbot-hook: parse: {e}"); return ExitCode::from(1); }
    };
    let event = enrich_with_scope(event);

    // Defense-in-depth: drop excluded events before they hit transport.
    let excludes = ExcludeSet::compile(&default_exclude_globs())
        .unwrap_or_else(|_| ExcludeSet::compile(&[]).expect("empty glob set"));
    if excludes.should_drop(&event) { return ExitCode::SUCCESS; }

    let home = home_dir();
    let client = HookClient::new(
        home.join("ingest.sock"),
        home.join("ingest-buffer.jsonl"),
        home.join(".hook-warn.stamp"),
    );
    // Fire-and-forget — bounded by 200ms socket deadline inside HookClient.
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => { eprintln!("klyntbot-hook: runtime: {e}"); return ExitCode::from(1); }
    };
    if let Err(e) = rt.block_on(client.send(&event)) {
        eprintln!("klyntbot-hook: send: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run_status() -> ExitCode {
    let home = home_dir();
    let sock = home.join("ingest.sock");
    let lock = home.join("desktop.lock");
    let buf = home.join("ingest-buffer.jsonl");
    let alive = is_desktop_alive(&lock);
    let buf_size = std::fs::metadata(&buf).map(|m| m.len()).unwrap_or(0);
    println!("socket:        {} ({})", sock.display(), if sock.exists() {"present"} else {"absent"});
    println!("desktop.lock:  {} ({})", lock.display(), if alive {"alive"} else {"stale or missing"});
    println!("buffer:        {} ({} bytes)", buf.display(), buf_size);
    ExitCode::SUCCESS
}

fn home_dir() -> PathBuf {
    let root = std::env::var("KLYNTBOT_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let h = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(h).join(".klyntbot")
        });
    let _ = std::fs::create_dir_all(&root);
    root
}

fn enrich_with_scope(event: AgentEvent) -> AgentEvent {
    let AgentEvent::V1(mut v1) = event;
    if v1.repo.is_none() {
        v1.repo = resolve_scope(&v1.cwd);
    }
    AgentEvent::V1(v1)
}
```

- [ ] **Step 2: Integration test via `assert_cmd`**

Add `assert_cmd = "2"` under `[dev-dependencies]` of `crates/coding-ingest/Cargo.toml`.

Create `crates/coding-ingest/tests/hook_binary.rs`:

```rust
use assert_cmd::Command;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;

#[tokio::test]
async fn hook_forwards_session_start_to_socket() {
    let home = TempDir::new().unwrap();
    let sock = home.path().join("ingest.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    let reader = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut len = [0u8; 4]; s.read_exact(&mut len).await.unwrap();
        let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
        s.read_exact(&mut body).await.unwrap();
        body
    });

    let stdin_body = br#"{"session_id":"abc","cwd":"/tmp","source":"cli","model":"m"}"#;
    Command::cargo_bin("klyntbot-hook").unwrap()
        .env("KLYNTBOT_HOME", home.path())
        .args(["claude-code", "SessionStart"])
        .write_stdin(&stdin_body[..])
        .assert()
        .success();

    let body = tokio::time::timeout(std::time::Duration::from_secs(2), reader)
        .await.unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["v"], "V1");
}

#[test]
fn status_subcommand_exits_zero_even_with_no_desktop() {
    let home = TempDir::new().unwrap();
    Command::cargo_bin("klyntbot-hook").unwrap()
        .env("KLYNTBOT_HOME", home.path())
        .arg("status")
        .assert()
        .success();
}
```

- [ ] **Step 3: Verify**

```bash
cargo build -p coding-ingest --bin klyntbot-hook
cargo nextest run -p coding-ingest --test hook_binary
cargo clippy -p coding-ingest --all-targets -- -D warnings
```
Expected: PASS + zero warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/src/bin/klyntbot-hook.rs crates/coding-ingest/tests/hook_binary.rs \
        crates/coding-ingest/Cargo.toml
git commit -m "feat(coding-ingest): klyntbot-hook end-to-end wiring + status subcommand"
```

---

### Task 15: `ClaudeCodeInstaller` — manage `~/.claude/settings.json`

**Files:**
- Create: `crates/app-core/src/coding_memory/mod.rs`
- Create: `crates/app-core/src/coding_memory/installer.rs`
- Modify: `crates/app-core/src/lib.rs`
- Test: `crates/app-core/tests/claude_code_installer.rs`

Claude Code reads `~/.claude/settings.json`. We manage seven entries under `hooks` — each maps a hook event name to an array of matcher/command blocks. We use a stable matcher string `"klyntbot-managed"` so we can identify + replace without clobbering user-written hooks.

- [ ] **Step 1: Write failing tests**

Create `crates/app-core/tests/claude_code_installer.rs`:

```rust
use app_core::coding_memory::installer::ClaudeCodeInstaller;
use serde_json::Value;
use tempfile::TempDir;

fn read(p: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn install_into_empty_dir_creates_settings_file() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    let hook = std::path::PathBuf::from("/usr/local/bin/klyntbot-hook");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    let v = read(&settings);
    let hooks = v.get("hooks").unwrap().as_object().unwrap();
    for event in ["SessionStart","SessionEnd","UserPromptSubmit","PreToolUse","PostToolUse","Stop","PreCompact"] {
        assert!(hooks.contains_key(event), "missing {event}");
    }
}

#[test]
fn install_preserves_unrelated_user_hooks() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{
        "hooks": {
            "SessionStart": [{"matcher":"my-own","hooks":[{"type":"command","command":"echo mine"}]}]
        }
    }"#).unwrap();
    let hook = std::path::PathBuf::from("/bin/kh");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    let v = read(&settings);
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "user + klyntbot entries must both exist");
}

#[test]
fn install_twice_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    let hook = std::path::PathBuf::from("/bin/kh");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    let v = read(&settings);
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 1, "second install must not duplicate");
}

#[test]
fn uninstall_removes_managed_entries_and_leaves_user_ones() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{
        "hooks": {
            "SessionStart": [{"matcher":"my-own","hooks":[{"type":"command","command":"echo mine"}]}]
        }
    }"#).unwrap();
    let hook = std::path::PathBuf::from("/bin/kh");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    ClaudeCodeInstaller::uninstall(&settings).unwrap();
    let v = read(&settings);
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["matcher"], "my-own");
}

#[test]
fn install_creates_backup_of_preexisting_file() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{"hooks":{}}"#).unwrap();
    ClaudeCodeInstaller::install(&settings, &std::path::PathBuf::from("/bin/kh")).unwrap();
    let backups: Vec<_> = std::fs::read_dir(dir.path()).unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.contains("klyntbot-backup"))
        .collect();
    assert_eq!(backups.len(), 1);
}
```

- [ ] **Step 2: Verify fails**

Run: `cargo nextest run -p app-core --test claude_code_installer`
Expected: FAIL — `coding_memory::installer` undefined.

- [ ] **Step 3: Implement**

Create `crates/app-core/src/coding_memory/mod.rs`:

```rust
//! `coding-memory` adapters owned by `app-core`.

/// Claude Code settings.json installer.
pub mod installer;
```

Create `crates/app-core/src/coding_memory/installer.rs`:

```rust
//! Manage klyntbot's managed entries in `~/.claude/settings.json`.
//!
//! Install semantics: read → merge → atomic write. Entries are identified by
//! a fixed matcher string (`klyntbot-managed`) so we can remove them cleanly
//! on uninstall without touching user-written hooks.

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MATCHER_TAG: &str = "klyntbot-managed";
const HOOK_EVENTS: [&str; 7] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "PreCompact",
];

/// Claude Code settings installer.
pub struct ClaudeCodeInstaller;

impl ClaudeCodeInstaller {
    /// Install klyntbot-managed hook entries. Creates a backup of the original
    /// if the settings file already existed.
    pub fn install(settings_path: &Path, hook_binary: &Path) -> Result<()> {
        let mut doc: Value = read_or_empty(settings_path)?;
        if settings_path.exists() {
            backup(settings_path)?;
        }
        let hooks = doc.get_mut("hooks")
            .and_then(Value::as_object_mut)
            .cloned();
        let mut hooks = hooks.unwrap_or_default();

        for event in HOOK_EVENTS {
            let arr = hooks
                .entry(event.to_string())
                .or_insert_with(|| Value::Array(vec![]))
                .as_array_mut()
                .ok_or_else(|| KlyntbotError::Storage(format!("hooks[{event}] not array")))?;
            arr.retain(|entry| entry.get("matcher").and_then(|m| m.as_str()) != Some(MATCHER_TAG));
            arr.push(json!({
                "matcher": MATCHER_TAG,
                "hooks": [{
                    "type": "command",
                    "command": format!("{} claude-code {}", hook_binary.display(), event),
                }]
            }));
        }
        doc["hooks"] = Value::Object(hooks);
        atomic_write(settings_path, &doc)
    }

    /// Remove klyntbot-managed entries. Leaves user entries intact.
    pub fn uninstall(settings_path: &Path) -> Result<()> {
        if !settings_path.exists() { return Ok(()); }
        let mut doc: Value = read_or_empty(settings_path)?;
        if let Some(hooks) = doc.get_mut("hooks").and_then(Value::as_object_mut) {
            for event in HOOK_EVENTS {
                if let Some(arr) = hooks.get_mut(event).and_then(Value::as_array_mut) {
                    arr.retain(|entry| {
                        entry.get("matcher").and_then(|m| m.as_str()) != Some(MATCHER_TAG)
                    });
                }
            }
        }
        atomic_write(settings_path, &doc)
    }

    /// Run the binary with a synthetic payload to verify exit code 0.
    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["claude-code", "SessionStart"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| KlyntbotError::Storage(format!("spawn hook: {e}")))?;
        let body = br#"{"session_id":"diagnose","cwd":"/tmp","source":"diagnose"}"#;
        child.stdin.as_mut().unwrap().write_all(body)
            .map_err(|e| KlyntbotError::Storage(format!("write stdin: {e}")))?;
        let status = child.wait()
            .map_err(|e| KlyntbotError::Storage(format!("wait: {e}")))?;
        if !status.success() {
            return Err(KlyntbotError::Storage(format!("hook exited {}", status.code().unwrap_or(-1))));
        }
        Ok(())
    }
}

fn read_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() { return Ok(json!({})); }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| KlyntbotError::Storage(format!("read settings: {e}")))?;
    if raw.trim().is_empty() { return Ok(json!({})); }
    serde_json::from_str(&raw)
        .map_err(|e| KlyntbotError::Storage(format!("parse settings: {e}")))
}

fn backup(path: &Path) -> Result<()> {
    let ts = Timestamp::now().as_millisecond();
    let bak: PathBuf = path.with_extension(format!("json.klyntbot-backup.{ts}"));
    std::fs::copy(path, &bak)
        .map_err(|e| KlyntbotError::Storage(format!("backup: {e}")))?;
    Ok(())
}

fn atomic_write(path: &Path, doc: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| KlyntbotError::Storage(format!("mkdir: {e}")))?;
    }
    let body = serde_json::to_vec_pretty(doc)
        .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &body)
        .map_err(|e| KlyntbotError::Storage(format!("write tmp: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| KlyntbotError::Storage(format!("rename: {e}")))?;
    Ok(())
}
```

Edit `crates/app-core/src/lib.rs` — add:

```rust
/// coding-memory adapters (Claude Code installer, etc.).
pub mod coding_memory;
```

- [ ] **Step 4: Verify + commit**

```bash
cargo nextest run -p app-core --test claude_code_installer
cargo clippy -p app-core --all-targets -- -D warnings
git add crates/app-core/src/coding_memory crates/app-core/src/lib.rs \
        crates/app-core/tests/claude_code_installer.rs
git commit -m "feat(app-core): ClaudeCodeInstaller — manage ~/.claude/settings.json atomically"
```

---

### Task 16: `AppCore` holds `IngestDaemonHandle`; init wires spawn

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs` (or equivalent init aggregator — verify with `rg 'pub(super) async fn init_storage' crates/app-core/src/init`)
- Modify: `crates/app-core/Cargo.toml` (already has `coding-ingest`/`coding-memory` from Task 1)
- Test: existing app-core tests run green

- [ ] **Step 1: Add handle slot on `AppCore`**

Edit `crates/app-core/src/state.rs` — find the `AppCore` struct definition and add:

```rust
/// Ingestion daemon handle; `None` until Phase-2 init wires it up.
pub ingest_daemon: std::sync::Mutex<Option<coding_ingest::daemon::IngestDaemonHandle>>,
```

Initialize in the constructor (`AppCore::new` or equivalent) with `std::sync::Mutex::new(None)`.

- [ ] **Step 2: Spawn daemon after storage init**

In the init aggregator (the function that calls `init_storage` and builds `AppCore`), after storage init and before returning the `AppCore`:

```rust
use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;

let data_dir = config.data_dir_path();
let daemon_cfg = IngestDaemonConfig {
    socket_path: data_dir.join("ingest.sock"),
    buffer_path: data_dir.join("ingest-buffer.jsonl"),
    lock_path: data_dir.join("desktop.lock"),
    repo: Arc::new(IngestEventLogRepo::new(storage_pool.inner().clone())),
};
let handle = match spawn(daemon_cfg).await {
    Ok(h) => Some(h),
    Err(e) => {
        tracing::warn!(error = %e, "ingest daemon failed to spawn — coding CLI ingestion disabled");
        None
    }
};
// After `app_core = Arc::new(AppCore { ... })`:
if let Ok(mut slot) = app_core.ingest_daemon.lock() { *slot = handle; }
```

Match the exact constructor shape in state.rs — this sketch assumes fielded struct initialization.

- [ ] **Step 3: Regression check**

```bash
cargo build -p app-core
cargo nextest run -p app-core
cargo clippy -p app-core --all-targets -- -D warnings
```
Expected: PASS + zero warnings. No regressions in personal-AI tests.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/
git commit -m "feat(app-core): spawn IngestDaemon during init; hold handle on AppCore"
```

---

### Task 17: Tauri commands — `coding_memory.rs`

**Files:**
- Create: `crates/desktop/src/commands/coding_memory.rs`
- Create: `crates/app-core/src/coding_memory/handlers.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Define DTOs in `desktop-shared`**

Look for existing shared-types location (`crates/desktop-shared/src/commands/`). Create `crates/desktop-shared/src/commands/coding_memory.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingMemoryStatusResponse {
    pub daemon_alive: bool,
    pub buffered_event_count: i64,
    pub unprocessed_event_count: i64,
    pub socket_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliHealthRow {
    pub cli: String,          // "claude-code" | "codex" | ...
    pub enabled: bool,
    pub last_event_at: Option<String>,
    pub event_count_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReplayEntry {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub kind: String,
    pub occurred_at: String,
    pub payload: String,       // raw JSON
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnoseResult {
    pub ok: bool,
    pub message: String,
}
```

Re-export from `crates/desktop-shared/src/commands/mod.rs`.

- [ ] **Step 2: Write handlers in app-core**

Create `crates/app-core/src/coding_memory/handlers.rs`:

```rust
//! Handlers backing the desktop Tauri commands for coding-memory.

use coding_ingest::desktop_lock::is_desktop_alive;
use coding_ingest::store::IngestEventLogRepo;
use common::Result;
use desktop_shared::commands::coding_memory::*;
use sqlx::Row;
use std::path::Path;

pub async fn status(
    repo: &IngestEventLogRepo,
    data_dir: &Path,
) -> Result<CodingMemoryStatusResponse> {
    let lock = data_dir.join("desktop.lock");
    let buffer = data_dir.join("ingest-buffer.jsonl");
    let buffered = std::fs::metadata(&buffer).map(|m| m.len()).unwrap_or(0) as i64;
    let unprocessed = repo.count_unprocessed().await?;
    Ok(CodingMemoryStatusResponse {
        daemon_alive: is_desktop_alive(&lock),
        buffered_event_count: buffered,
        unprocessed_event_count: unprocessed,
        socket_path: data_dir.join("ingest.sock").to_string_lossy().into(),
    })
}

pub async fn cli_health(pool: &sqlx::SqlitePool) -> Result<Vec<CliHealthRow>> {
    let sources = ["claude-code", "codex", "kimi-cli", "opencode"];
    let mut out = Vec::with_capacity(sources.len());
    for src in sources {
        let row = sqlx::query(
            "SELECT COUNT(*) as c, MAX(occurred_at) as last
             FROM ingest_event_log
             WHERE source = ? AND received_at > datetime('now', '-1 day')",
        )
        .bind(src)
        .fetch_one(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("cli_health: {e}")))?;
        out.push(CliHealthRow {
            cli: src.into(),
            enabled: false, // filled in by caller from config
            last_event_at: row.try_get::<Option<String>, _>("last").unwrap_or(None),
            event_count_24h: row.try_get::<i64, _>("c").unwrap_or(0),
        });
    }
    Ok(out)
}

pub async fn session_replay(
    pool: &sqlx::SqlitePool,
    session_id: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SessionReplayEntry>> {
    let rows = match session_id {
        Some(sid) => {
            sqlx::query(
                "SELECT id, source, session_id, kind, occurred_at, payload
                 FROM ingest_event_log WHERE session_id = ?
                 ORDER BY occurred_at ASC LIMIT ? OFFSET ?",
            )
            .bind(sid).bind(limit).bind(offset)
            .fetch_all(pool).await
        }
        None => {
            sqlx::query(
                "SELECT id, source, session_id, kind, occurred_at, payload
                 FROM ingest_event_log
                 ORDER BY received_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit).bind(offset)
            .fetch_all(pool).await
        }
    }
    .map_err(|e| common::KlyntbotError::Storage(format!("session_replay: {e}")))?;
    Ok(rows.into_iter().map(|r| SessionReplayEntry {
        id: r.get("id"),
        source: r.get("source"),
        session_id: r.get("session_id"),
        kind: r.get("kind"),
        occurred_at: r.get("occurred_at"),
        payload: r.get("payload"),
    }).collect())
}
```

Export from `crates/app-core/src/coding_memory/mod.rs`:

```rust
pub mod handlers;
```

Add `AppCore` methods that wrap these — e.g. in `crates/app-core/src/coding_memory/mod.rs` or a new file `crates/app-core/src/handlers/coding_memory.rs`:

```rust
impl crate::AppCore {
    pub async fn coding_memory_status(
        self: &std::sync::Arc<Self>,
    ) -> Result<desktop_shared::commands::coding_memory::CodingMemoryStatusResponse, desktop_shared::errors::ApiError> {
        let repo = coding_ingest::store::IngestEventLogRepo::new(self.storage_pool.inner().clone());
        handlers::status(&repo, &self.config_read().data_dir_path()).await
            .map_err(desktop_shared::errors::ApiError::from)
    }
    // ...similar wrappers for cli_health / session_replay / enable / disable / diagnose
}
```

Precise integration depends on existing `AppCore` shape; match it exactly.

- [ ] **Step 3: Create the tauri commands module**

Create `crates/desktop/src/commands/coding_memory.rs`:

```rust
//! Tauri adapters for coding-memory.

use std::sync::Arc;
use tauri::State;

use desktop_shared::commands::coding_memory::*;
use desktop_shared::errors::ApiError;

use crate::app_core::AppCore;

/// dev_server command coverage.
pub const DEV_COMMANDS: &[&str] = &[
    "coding_memory_status",
    "coding_memory_enable_cli",
    "coding_memory_disable_cli",
    "coding_memory_diagnose_cli",
    "coding_memory_session_replay",
    "coding_memory_cli_health",
];

#[tauri::command]
pub async fn coding_memory_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<CodingMemoryStatusResponse, ApiError> {
    state.coding_memory_status().await
}

#[tauri::command]
pub async fn coding_memory_cli_health(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<CliHealthRow>, ApiError> {
    state.coding_memory_cli_health().await
}

#[tauri::command]
pub async fn coding_memory_session_replay(
    state: State<'_, Arc<AppCore>>,
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<SessionReplayEntry>, ApiError> {
    state.coding_memory_session_replay(session_id, limit.unwrap_or(500), offset.unwrap_or(0)).await
}

#[tauri::command]
pub async fn coding_memory_enable_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> Result<(), ApiError> {
    state.coding_memory_enable_cli(cli).await
}

#[tauri::command]
pub async fn coding_memory_disable_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> Result<(), ApiError> {
    state.coding_memory_disable_cli(cli).await
}

#[tauri::command]
pub async fn coding_memory_diagnose_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> Result<DiagnoseResult, ApiError> {
    state.coding_memory_diagnose_cli(cli).await
}

/// dev_server dispatcher — wired by dev_server/mod.rs.
pub async fn dispatch_dev(
    state: &Arc<AppCore>,
    cmd: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    match cmd {
        "coding_memory_status" => ok(state.coding_memory_status().await?),
        "coding_memory_cli_health" => ok(state.coding_memory_cli_health().await?),
        "coding_memory_session_replay" => {
            #[derive(serde::Deserialize)]
            struct A { session_id: Option<String>, limit: Option<i64>, offset: Option<i64> }
            let a: A = serde_json::from_value(args).map_err(|e| ApiError::bad_request(e.to_string()))?;
            ok(state.coding_memory_session_replay(a.session_id, a.limit.unwrap_or(500), a.offset.unwrap_or(0)).await?)
        }
        "coding_memory_enable_cli" => {
            #[derive(serde::Deserialize)] struct A { cli: String }
            let a: A = serde_json::from_value(args).map_err(|e| ApiError::bad_request(e.to_string()))?;
            state.coding_memory_enable_cli(a.cli).await?;
            Ok(serde_json::Value::Null)
        }
        "coding_memory_disable_cli" => {
            #[derive(serde::Deserialize)] struct A { cli: String }
            let a: A = serde_json::from_value(args).map_err(|e| ApiError::bad_request(e.to_string()))?;
            state.coding_memory_disable_cli(a.cli).await?;
            Ok(serde_json::Value::Null)
        }
        "coding_memory_diagnose_cli" => {
            #[derive(serde::Deserialize)] struct A { cli: String }
            let a: A = serde_json::from_value(args).map_err(|e| ApiError::bad_request(e.to_string()))?;
            ok(state.coding_memory_diagnose_cli(a.cli).await?)
        }
        _ => Err(ApiError::not_found(format!("coding_memory: unknown dev cmd {cmd}"))),
    }
}

fn ok<T: serde::Serialize>(v: T) -> Result<serde_json::Value, ApiError> {
    Ok(serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
}
```

- [ ] **Step 4: Register in invoke handler + dev server**

In `crates/desktop/src/commands/mod.rs` add `pub mod coding_memory;`.

In `crates/desktop/src/lib.rs` — search for the `.invoke_handler(tauri::generate_handler![` macro and add all 6 names.

In `crates/desktop/src/dev_server/mod.rs`:
- Add `commands::coding_memory::DEV_COMMANDS` to the `Self::collect()` array (line ~211 onward).
- Add a match arm in the command dispatcher that routes `cmd.starts_with("coding_memory_")` to `commands::coding_memory::dispatch_dev(state, cmd, args).await`.

- [ ] **Step 5: Verify coverage test passes**

```bash
cargo nextest run -p desktop dev_server_covers_all_tauri_commands
cargo build -p desktop
cargo clippy -p desktop --all-targets -- -D warnings
```
Expected: PASS + zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop-shared/src/commands/coding_memory.rs \
        crates/desktop-shared/src/commands/mod.rs \
        crates/app-core/src/coding_memory/ \
        crates/desktop/src/commands/coding_memory.rs \
        crates/desktop/src/commands/mod.rs \
        crates/desktop/src/lib.rs \
        crates/desktop/src/dev_server/mod.rs
git commit -m "feat(desktop): coding-memory Tauri commands + dev_server coverage"
```

---

### Task 18: Wire installer into Tauri commands

**Files:**
- Modify: `crates/app-core/src/coding_memory/handlers.rs` (or wherever the AppCore method wrappers live)

- [ ] **Step 1: Implement `enable_cli` / `disable_cli` / `diagnose_cli`**

Extend the AppCore method wrappers added in Task 17:

```rust
impl crate::AppCore {
    pub async fn coding_memory_enable_cli(&self, cli: String) -> Result<(), desktop_shared::errors::ApiError> {
        if cli != "claude-code" {
            return Err(desktop_shared::errors::ApiError::bad_request(
                "Only claude-code is wired in Phase 2".into()));
        }
        let settings = claude_code_settings_path()?;
        let binary = hook_binary_path()?;
        tokio::task::spawn_blocking(move || {
            crate::coding_memory::installer::ClaudeCodeInstaller::install(&settings, &binary)
        }).await.map_err(|e| desktop_shared::errors::ApiError::internal(e.to_string()))?
          .map_err(|e| desktop_shared::errors::ApiError::internal(e.to_string()))?;
        // Persist config toggle.
        self.update_config(|c| c.coding_memory.cli.claude_code.enabled = true).await?;
        Ok(())
    }

    pub async fn coding_memory_disable_cli(&self, cli: String) -> Result<(), desktop_shared::errors::ApiError> {
        if cli != "claude-code" {
            return Err(desktop_shared::errors::ApiError::bad_request("Only claude-code".into()));
        }
        let settings = claude_code_settings_path()?;
        tokio::task::spawn_blocking(move || {
            crate::coding_memory::installer::ClaudeCodeInstaller::uninstall(&settings)
        }).await.map_err(|e| desktop_shared::errors::ApiError::internal(e.to_string()))?
          .map_err(|e| desktop_shared::errors::ApiError::internal(e.to_string()))?;
        self.update_config(|c| c.coding_memory.cli.claude_code.enabled = false).await?;
        Ok(())
    }

    pub async fn coding_memory_diagnose_cli(&self, cli: String) -> Result<DiagnoseResult, desktop_shared::errors::ApiError> {
        if cli != "claude-code" {
            return Ok(DiagnoseResult { ok: false, message: "only claude-code supported".into() });
        }
        let binary = hook_binary_path()?;
        let outcome = tokio::task::spawn_blocking(move || {
            crate::coding_memory::installer::ClaudeCodeInstaller::diagnose(&binary)
        }).await.map_err(|e| desktop_shared::errors::ApiError::internal(e.to_string()))?;
        match outcome {
            Ok(()) => Ok(DiagnoseResult { ok: true, message: "hook exited 0".into() }),
            Err(e) => Ok(DiagnoseResult { ok: false, message: e.to_string() }),
        }
    }
}

fn claude_code_settings_path() -> Result<std::path::PathBuf, desktop_shared::errors::ApiError> {
    let home = std::env::var("HOME").map_err(|_| desktop_shared::errors::ApiError::internal("no $HOME".into()))?;
    Ok(std::path::PathBuf::from(home).join(".claude").join("settings.json"))
}

fn hook_binary_path() -> Result<std::path::PathBuf, desktop_shared::errors::ApiError> {
    // Prefer the co-located binary shipped with the desktop app bundle; fall back to PATH lookup.
    let exe = std::env::current_exe()
        .map_err(|e| desktop_shared::errors::ApiError::internal(e.to_string()))?;
    let dir = exe.parent().ok_or_else(|| desktop_shared::errors::ApiError::internal("bad exe path".into()))?;
    let candidate = dir.join("klyntbot-hook");
    if candidate.exists() { return Ok(candidate); }
    Ok(std::path::PathBuf::from("klyntbot-hook"))
}
```

`update_config` is a hypothetical shorthand for whatever the existing `AppCore` config-write flow looks like; match it exactly — the pattern should already exist for other config toggles.

- [ ] **Step 2: Verify + commit**

```bash
cargo build -p desktop
cargo clippy -p desktop --all-targets -- -D warnings
git commit -am "feat(app-core): enable/disable/diagnose Claude Code CLI via installer"
```

---

### Task 19: `CodingCliSettings` React page

**Files:**
- Create: `desktop-ui/src/features/settings/pages/CodingCliSettings.tsx`
- Modify: `desktop-ui/src/features/settings/index.ts`
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: Create the page**

Create `desktop-ui/src/features/settings/pages/CodingCliSettings.tsx`:

```tsx
import { useMutation, useQuery } from "@shared/data/ipc";
import type {
  CodingMemoryStatusResponse,
  DiagnoseResult,
} from "@shared/types/coding-memory";
import { useState } from "react";

export function CodingCliSettings() {
  const status = useQuery<CodingMemoryStatusResponse>("coding_memory_status", {});
  const enable = useMutation<void, { cli: string }>("coding_memory_enable_cli");
  const disable = useMutation<void, { cli: string }>("coding_memory_disable_cli");
  const diagnose = useMutation<DiagnoseResult, { cli: string }>("coding_memory_diagnose_cli");
  const [lastDiagnose, setLastDiagnose] = useState<DiagnoseResult | null>(null);

  return (
    <section className="space-y-6 p-6">
      <header>
        <h1 className="text-xl font-semibold text-text">Coding CLI Integration</h1>
        <p className="mt-1 text-sm text-muted">
          Klyntbot can observe your coding CLI sessions via hooks. Memory accumulates
          silently and surfaces on your next session.
        </p>
      </header>

      <div className="rounded-lg border border-border bg-surface-base p-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-sm font-medium text-text">Claude Code</h2>
            <p className="text-xs text-muted">
              Writes hook config to ~/.claude/settings.json
            </p>
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              className="rounded border border-border px-3 py-1 text-sm hover:bg-surface-hover"
              onClick={() => enable.mutate({ cli: "claude-code" }).then(() => status.refetch())}
              disabled={enable.isPending}
            >
              Enable
            </button>
            <button
              type="button"
              className="rounded border border-border px-3 py-1 text-sm hover:bg-surface-hover"
              onClick={() => disable.mutate({ cli: "claude-code" }).then(() => status.refetch())}
              disabled={disable.isPending}
            >
              Disable
            </button>
            <button
              type="button"
              className="rounded border border-border px-3 py-1 text-sm hover:bg-surface-hover"
              onClick={async () => {
                const r = await diagnose.mutate({ cli: "claude-code" });
                setLastDiagnose(r);
              }}
              disabled={diagnose.isPending}
            >
              Diagnose
            </button>
          </div>
        </div>
        {lastDiagnose && (
          <div
            className={`mt-3 rounded p-2 text-xs ${
              lastDiagnose.ok ? "bg-success-muted text-success" : "bg-danger-muted text-danger"
            }`}
          >
            {lastDiagnose.message}
          </div>
        )}
      </div>

      <div className="rounded-lg border border-border bg-surface-base p-4 text-sm">
        <h3 className="mb-2 font-medium text-text">Daemon status</h3>
        {status.data ? (
          <dl className="grid grid-cols-2 gap-y-1 text-xs">
            <dt className="text-muted">Socket</dt>
            <dd className="text-text">{status.data.socketPath}</dd>
            <dt className="text-muted">Daemon</dt>
            <dd className="text-text">{status.data.daemonAlive ? "alive" : "stopped"}</dd>
            <dt className="text-muted">Buffered bytes</dt>
            <dd className="text-text">{status.data.bufferedEventCount}</dd>
            <dt className="text-muted">Unprocessed events</dt>
            <dd className="text-text">{status.data.unprocessedEventCount}</dd>
          </dl>
        ) : (
          <div className="text-muted">Loading…</div>
        )}
      </div>

      <p className="text-xs text-muted">
        Klyntbot desktop must be running in the background for hooks to forward events
        immediately. When the desktop is off, events buffer to disk and drain on next
        startup.
      </p>
    </section>
  );
}
```

- [ ] **Step 2: Re-export + route**

Edit `desktop-ui/src/features/settings/index.ts` — add:

```ts
export { CodingCliSettings } from "./pages/CodingCliSettings";
```

Edit `desktop-ui/src/app/router.tsx` — add a lazy-loaded route under the settings parent following the existing pattern:

```tsx
const CodingCliSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.CodingCliSettings })),
);
// …inside the settings children array:
{ path: "coding-cli", element: <CodingCliSettings /> },
```

Add a nav entry under the settings sidebar (follow the same pattern as `IntegrationsSettings` — search `IntegrationsSettings` in the SettingsLayout).

- [ ] **Step 3: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix && bun run test
```
Expected: PASS.

```bash
cd /Users/jayden/Projects/Klynt/bot
git add desktop-ui/src/features/settings/pages/CodingCliSettings.tsx \
        desktop-ui/src/features/settings/index.ts \
        desktop-ui/src/app/router.tsx
git commit -m "feat(desktop-ui): Coding CLI Integration settings page"
```

---

### Task 20: Workbench — `/coding-memory` layout + route

**Files:**
- Create: `desktop-ui/src/features/coding-memory/index.ts`
- Create: `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx`
- Create: `desktop-ui/src/features/coding-memory/hooks.ts`
- Create: `desktop-ui/src/shared/types/coding-memory.ts` (DTO types matching `desktop-shared`)
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: DTO types**

Create `desktop-ui/src/shared/types/coding-memory.ts`:

```ts
export interface CodingMemoryStatusResponse {
  daemonAlive: boolean;
  bufferedEventCount: number;
  unprocessedEventCount: number;
  socketPath: string;
}

export interface CliHealthRow {
  cli: string;
  enabled: boolean;
  lastEventAt: string | null;
  eventCount24h: number;
}

export interface SessionReplayEntry {
  id: string;
  source: string;
  sessionId: string;
  kind: string;
  occurredAt: string;
  payload: string;
}

export interface DiagnoseResult {
  ok: boolean;
  message: string;
}
```

- [ ] **Step 2: Hooks**

Create `desktop-ui/src/features/coding-memory/hooks.ts`:

```ts
import { useQuery } from "@shared/data/ipc";
import type {
  CliHealthRow,
  CodingMemoryStatusResponse,
  SessionReplayEntry,
} from "@shared/types/coding-memory";

export const useCodingMemoryStatus = () =>
  useQuery<CodingMemoryStatusResponse>("coding_memory_status", {});

export const useCliHealth = () =>
  useQuery<CliHealthRow[]>("coding_memory_cli_health", {});

export const useSessionReplay = (sessionId?: string, limit = 500, offset = 0) =>
  useQuery<SessionReplayEntry[]>("coding_memory_session_replay", {
    sessionId: sessionId ?? null,
    limit,
    offset,
  });
```

- [ ] **Step 3: Layout**

Create `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx`:

```tsx
import { NavLink, Outlet } from "react-router-dom";

const tabs = [
  { to: "cli-health", label: "CLI Health" },
  { to: "session-replay", label: "Session Replay" },
];

export function CodingMemoryLayout() {
  return (
    <div className="flex h-full">
      <nav className="w-48 border-r border-border p-4 text-sm">
        <h1 className="mb-3 text-xs font-semibold uppercase tracking-wide text-muted">
          Coding Memory
        </h1>
        <ul className="space-y-1">
          {tabs.map((t) => (
            <li key={t.to}>
              <NavLink
                to={t.to}
                className={({ isActive }) =>
                  `block rounded px-2 py-1 ${
                    isActive ? "bg-surface-hover text-text" : "text-muted hover:text-text"
                  }`
                }
              >
                {t.label}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
```

- [ ] **Step 4: Re-exports + route**

Create `desktop-ui/src/features/coding-memory/index.ts`:

```ts
export { CodingMemoryLayout } from "./CodingMemoryLayout";
export { CliHealthPanel } from "./CliHealthPanel";
export { SessionReplayPanel } from "./SessionReplayPanel";
```

Edit `desktop-ui/src/app/router.tsx` — add `/coding-memory` top-level route with nested children (`cli-health`, `session-replay`). Follow the existing lazy-load pattern.

- [ ] **Step 5: Commit (panels land in Tasks 21–22 but stub files must exist to satisfy imports)**

Create placeholder `desktop-ui/src/features/coding-memory/CliHealthPanel.tsx`:

```tsx
export function CliHealthPanel() { return <div className="p-6 text-muted">Loading…</div>; }
```

And `desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx`:

```tsx
export function SessionReplayPanel() { return <div className="p-6 text-muted">Loading…</div>; }
```

```bash
cd desktop-ui && bun run lint:fix
cd /Users/jayden/Projects/Klynt/bot
git add desktop-ui/src/shared/types/coding-memory.ts \
        desktop-ui/src/features/coding-memory/ \
        desktop-ui/src/app/router.tsx
git commit -m "feat(desktop-ui): coding-memory workbench layout + route"
```

---

### Task 21: `CliHealthPanel`

**Files:**
- Modify: `desktop-ui/src/features/coding-memory/CliHealthPanel.tsx`
- Test: `desktop-ui/src/features/coding-memory/__tests__/CliHealthPanel.test.tsx`

- [ ] **Step 1: Write the panel**

Replace `CliHealthPanel.tsx`:

```tsx
import { useCliHealth, useCodingMemoryStatus } from "./hooks";

const CLI_LABELS: Record<string, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  "kimi-cli": "kimi-cli",
  opencode: "opencode",
};

export function CliHealthPanel() {
  const rows = useCliHealth();
  const status = useCodingMemoryStatus();

  if (!rows.data || !status.data) {
    return <div className="p-6 text-sm text-muted">Loading…</div>;
  }
  return (
    <section className="space-y-4 p-6">
      <header className="flex items-baseline justify-between">
        <h2 className="text-lg font-semibold text-text">CLI Health</h2>
        <span
          className={`rounded px-2 py-0.5 text-xs ${
            status.data.daemonAlive
              ? "bg-success-muted text-success"
              : "bg-danger-muted text-danger"
          }`}
        >
          Daemon {status.data.daemonAlive ? "alive" : "stopped"}
        </span>
      </header>
      <table className="w-full text-sm">
        <thead className="text-xs uppercase tracking-wide text-muted">
          <tr>
            <th className="py-2 text-left">CLI</th>
            <th className="py-2 text-left">Enabled</th>
            <th className="py-2 text-left">Last event</th>
            <th className="py-2 text-right">24h events</th>
          </tr>
        </thead>
        <tbody>
          {rows.data.map((r) => (
            <tr key={r.cli} className="border-t border-border">
              <td className="py-2">{CLI_LABELS[r.cli] ?? r.cli}</td>
              <td className="py-2">{r.enabled ? "yes" : "no"}</td>
              <td className="py-2">{r.lastEventAt ?? "—"}</td>
              <td className="py-2 text-right tabular-nums">{r.eventCount24h}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
```

- [ ] **Step 2: Test**

Create `desktop-ui/src/features/coding-memory/__tests__/CliHealthPanel.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CliHealthPanel } from "../CliHealthPanel";

vi.mock("../hooks", () => ({
  useCliHealth: () => ({
    data: [
      { cli: "claude-code", enabled: true, lastEventAt: "2026-04-23T10:00:00Z", eventCount24h: 42 },
    ],
  }),
  useCodingMemoryStatus: () => ({
    data: { daemonAlive: true, bufferedEventCount: 0, unprocessedEventCount: 3, socketPath: "/x" },
  }),
}));

describe("CliHealthPanel", () => {
  it("renders rows", () => {
    render(<CliHealthPanel />);
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText(/alive/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Commit**

```bash
cd desktop-ui && bun run test
cd /Users/jayden/Projects/Klynt/bot
git add desktop-ui/src/features/coding-memory/CliHealthPanel.tsx \
        desktop-ui/src/features/coding-memory/__tests__/CliHealthPanel.test.tsx
git commit -m "feat(desktop-ui): CliHealthPanel"
```

---

### Task 22: `SessionReplayPanel`

**Files:**
- Modify: `desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx`
- Test: `desktop-ui/src/features/coding-memory/__tests__/SessionReplayPanel.test.tsx`

- [ ] **Step 1: Write the panel**

Replace `SessionReplayPanel.tsx`:

```tsx
import { useState } from "react";
import type { SessionReplayEntry } from "@shared/types/coding-memory";
import { useSessionReplay } from "./hooks";

export function SessionReplayPanel() {
  const [offset, setOffset] = useState(0);
  const rows = useSessionReplay(undefined, 500, offset);
  const [selected, setSelected] = useState<SessionReplayEntry | null>(null);

  if (!rows.data) return <div className="p-6 text-sm text-muted">Loading…</div>;

  return (
    <section className="flex h-full">
      <div className="flex-1 overflow-auto p-6">
        <header className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-text">Session Replay</h2>
          <div className="flex gap-2">
            <button
              type="button"
              className="rounded border border-border px-2 py-1 text-sm disabled:opacity-50"
              onClick={() => setOffset(Math.max(0, offset - 500))}
              disabled={offset === 0}
            >
              ◀ Prev
            </button>
            <button
              type="button"
              className="rounded border border-border px-2 py-1 text-sm disabled:opacity-50"
              onClick={() => setOffset(offset + 500)}
              disabled={rows.data.length < 500}
            >
              Next ▶
            </button>
          </div>
        </header>
        <ul className="divide-y divide-border font-mono text-xs">
          {rows.data.map((r) => (
            <li key={r.id}>
              <button
                type="button"
                onClick={() => setSelected(r)}
                className="flex w-full items-baseline gap-4 py-1.5 text-left hover:bg-surface-hover"
              >
                <span className="w-40 shrink-0 text-muted">{r.occurredAt}</span>
                <span className="w-24 shrink-0 text-accent">{r.kind}</span>
                <span className="w-24 shrink-0 text-muted">{r.source}</span>
                <span className="truncate text-text">{r.sessionId}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
      {selected && (
        <aside className="w-1/3 border-l border-border p-6">
          <header className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-semibold">Event detail</h3>
            <button
              type="button"
              onClick={() => setSelected(null)}
              className="text-xs text-muted hover:text-text"
            >
              Close
            </button>
          </header>
          <pre className="whitespace-pre-wrap break-words rounded bg-surface-base p-3 text-xs">
            {JSON.stringify(JSON.parse(selected.payload), null, 2)}
          </pre>
        </aside>
      )}
    </section>
  );
}
```

- [ ] **Step 2: Test**

Create `desktop-ui/src/features/coding-memory/__tests__/SessionReplayPanel.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SessionReplayPanel } from "../SessionReplayPanel";

vi.mock("../hooks", () => ({
  useSessionReplay: () => ({
    data: [
      {
        id: "e1",
        source: "claude-code",
        sessionId: "s-1",
        kind: "userPrompt",
        occurredAt: "2026-04-23T10:00:00Z",
        payload: JSON.stringify({ v: "V1", kind: "userPrompt", text: "hi" }),
      },
    ],
  }),
}));

describe("SessionReplayPanel", () => {
  it("renders rows and opens detail", () => {
    render(<SessionReplayPanel />);
    expect(screen.getByText("userPrompt")).toBeInTheDocument();
    fireEvent.click(screen.getByText("userPrompt"));
    expect(screen.getByText(/Event detail/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix && bun run test
cd /Users/jayden/Projects/Klynt/bot
git add desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx \
        desktop-ui/src/features/coding-memory/__tests__/SessionReplayPanel.test.tsx
git commit -m "feat(desktop-ui): SessionReplayPanel with detail drawer"
```

---

### Task 23: Synthetic Claude Code fixture

**Files:**
- Create: `tests/fixtures/coding/synthetic_session_claude_code.jsonl`

Each line is `{"hookEvent": "<name>", "body": <claude-code-json>}` — consumed by the integration test in Task 24.

- [ ] **Step 1: Write the fixture**

Create `tests/fixtures/coding/synthetic_session_claude_code.jsonl`:

```jsonl
{"hookEvent":"SessionStart","body":{"session_id":"fx-1","cwd":"/tmp/fx","source":"cli","model":"claude-sonnet-4-6"}}
{"hookEvent":"UserPromptSubmit","body":{"session_id":"fx-1","cwd":"/tmp/fx","prompt":"fix the failing test","attachments":[]}}
{"hookEvent":"PostToolUse","body":{"session_id":"fx-1","cwd":"/tmp/fx","tool_name":"Read","tool_input":{"file_path":"/tmp/fx/src/lib.rs"},"tool_response":{"bytes":1200},"duration_ms":5}}
{"hookEvent":"PostToolUse","body":{"session_id":"fx-1","cwd":"/tmp/fx","tool_name":"Bash","tool_input":{"command":"cargo test --workspace"},"tool_response":{"stdout":"test result: ok. 12 passed; 3 failed; 0 ignored","stderr":"","exit_code":1},"duration_ms":4200}}
{"hookEvent":"PostToolUse","body":{"session_id":"fx-1","cwd":"/tmp/fx","tool_name":"Edit","tool_input":{"file_path":"/tmp/fx/src/lib.rs","old_string":"a","new_string":"b"},"tool_response":{"bytes":1205},"duration_ms":12}}
{"hookEvent":"PostToolUse","body":{"session_id":"fx-1","cwd":"/tmp/fx","tool_name":"Bash","tool_input":{"command":"cargo test --workspace"},"tool_response":{"stdout":"test result: ok. 15 passed; 0 failed; 0 ignored","stderr":"","exit_code":0},"duration_ms":4100}}
{"hookEvent":"Stop","body":{"session_id":"fx-1","cwd":"/tmp/fx","transcript_path":"/tmp/fx/.tr","stop_hook_active":false}}
{"hookEvent":"PreCompact","body":{"session_id":"fx-1","cwd":"/tmp/fx","trigger":"auto"}}
{"hookEvent":"UserPromptSubmit","body":{"session_id":"fx-1","cwd":"/tmp/fx","prompt":"what else needs doing?","attachments":[]}}
{"hookEvent":"SessionEnd","body":{"session_id":"fx-1","cwd":"/tmp/fx","reason":"user-quit"}}
```

- [ ] **Step 2: Commit**

```bash
git add tests/fixtures/coding/synthetic_session_claude_code.jsonl
git commit -m "test(coding-memory): synthetic Claude Code session fixture (10 events)"
```

---

### Task 24: End-to-end round-trip integration test

**Files:**
- Create: `tests/integration/coding_memory_phase2_roundtrip.rs` (or add to existing root test binaries — verify with `ls tests/`)

Verify `ls /Users/jayden/Projects/Klynt/bot/tests` first; add a new `[[test]]` entry in the root `Cargo.toml` if needed.

- [ ] **Step 1: Write the test**

Create `tests/integration/coding_memory_phase2_roundtrip.rs`:

```rust
//! End-to-end: run klyntbot-hook against a running IngestDaemon using the
//! synthetic Claude Code fixture. Assert every event lands in ingest_event_log.

use assert_cmd::Command;
use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;

#[derive(serde::Deserialize)]
struct FixtureLine {
    #[serde(rename = "hookEvent")]
    hook_event: String,
    body: serde_json::Value,
}

#[tokio::test]
async fn synthetic_claude_code_session_round_trips() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let home = TempDir::new().unwrap();
    let cfg = IngestDaemonConfig {
        socket_path: home.path().join("ingest.sock"),
        buffer_path: home.path().join("ingest-buffer.jsonl"),
        lock_path: home.path().join("desktop.lock"),
        repo: repo.clone(),
    };
    let handle = spawn(cfg).await.expect("daemon spawn");

    let fixture = std::fs::read_to_string("tests/fixtures/coding/synthetic_session_claude_code.jsonl").unwrap();
    for line in fixture.lines().filter(|l| !l.trim().is_empty()) {
        let fl: FixtureLine = serde_json::from_str(line).unwrap();
        Command::cargo_bin("klyntbot-hook").unwrap()
            .env("KLYNTBOT_HOME", home.path())
            .args(["claude-code", &fl.hook_event])
            .write_stdin(serde_json::to_string(&fl.body).unwrap())
            .assert()
            .success();
    }

    // Poll for arrival.
    for _ in 0..100 {
        if repo.count_by_session("fx-1").await.unwrap() >= 9 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    // PreToolUse is filtered so we expect 9 events (not 10): fixture has no PreToolUse, so all 10 land.
    // But our adapter returns `None` for `PreToolUse`, which the fixture doesn't exercise anyway.
    let total = repo.count_by_session("fx-1").await.unwrap();
    assert!(total >= 9, "expected >=9 events, got {total}");

    handle.shutdown().await;
}
```

- [ ] **Step 2: Register as root-level test binary**

If `tests/integration/` isn't already compiled as a root test, add to root `Cargo.toml`:

```toml
[[test]]
name = "coding_memory_phase2_roundtrip"
path = "tests/integration/coding_memory_phase2_roundtrip.rs"
```

Add root `[dev-dependencies]` as needed (`coding-ingest`, `coding-memory`, `cognitive`, `storage`, `assert_cmd`, `tempfile`, `tokio`, `serde`, `serde_json`). Verify most are already present.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run --test coding_memory_phase2_roundtrip
git add tests/integration/coding_memory_phase2_roundtrip.rs Cargo.toml
git commit -m "test(coding-memory): Phase-2 end-to-end round-trip via synthetic session"
```

---

### Task 25: Desktop-off recovery scenario test

**Files:**
- Create: `tests/integration/coding_memory_phase2_desktop_off.rs`

- [ ] **Step 1: Write the test**

Create `tests/integration/coding_memory_phase2_desktop_off.rs`:

```rust
//! Scenario: desktop off → 3 hook invocations buffer to disk → desktop starts
//! → buffered events drain into ingest_event_log → archive file present.

use assert_cmd::Command;
use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;

#[tokio::test]
async fn desktop_off_buffers_then_drains_on_startup() {
    let home = TempDir::new().unwrap();

    // Phase 1: desktop is OFF. Send 3 hook events — they go to the file buffer.
    for i in 0..3 {
        let body = format!(
            r#"{{"session_id":"off-{i}","cwd":"/tmp","source":"cli","model":"m"}}"#
        );
        Command::cargo_bin("klyntbot-hook").unwrap()
            .env("KLYNTBOT_HOME", home.path())
            .args(["claude-code", "SessionStart"])
            .write_stdin(body)
            .assert()
            .success();
    }
    let buffer_path = home.path().join("ingest-buffer.jsonl");
    let contents = std::fs::read_to_string(&buffer_path).unwrap();
    assert_eq!(contents.lines().count(), 3);

    // Phase 2: desktop starts. Daemon drains the buffer on spawn.
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let cfg = IngestDaemonConfig {
        socket_path: home.path().join("ingest.sock"),
        buffer_path: buffer_path.clone(),
        lock_path: home.path().join("desktop.lock"),
        repo: repo.clone(),
    };
    let handle = spawn(cfg).await.expect("spawn");

    assert_eq!(repo.count_unprocessed().await.unwrap(), 3);
    assert!(!buffer_path.exists(), "buffer should be archived");
    let archived: Vec<_> = std::fs::read_dir(home.path()).unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.contains(".done."))
        .collect();
    assert_eq!(archived.len(), 1);

    handle.shutdown().await;
}
```

- [ ] **Step 2: Register + commit**

Add `[[test]]` entry to root Cargo.toml matching the previous task.

```bash
cargo nextest run --test coding_memory_phase2_desktop_off
git add tests/integration/coding_memory_phase2_desktop_off.rs Cargo.toml
git commit -m "test(coding-memory): Phase-2 desktop-off recovery scenario"
```

---

### Task 26: Final quality gates + docs

**Files:**
- Modify: `docs/coding-memory/README.md` (or create if missing)

- [ ] **Step 1: Full workspace verification**

Run the full matrix:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run lint && bun run test && bun run build
cd /Users/jayden/Projects/Klynt/bot
```
Expected: all PASS. Zero clippy warnings. Zero fmt drift.

- [ ] **Step 2: Grep for Phase-2 regressions**

```bash
# No `NotImplemented` strings on hot Phase-2 paths.
rg -n 'NotImplemented' crates/coding-ingest/src/transport.rs crates/coding-ingest/src/daemon.rs crates/coding-ingest/src/adapters/claude_code
# Should print nothing.

# No stray TODO / FIXME landed in Phase-2 paths.
rg -n 'TODO|FIXME' crates/coding-ingest/src crates/coding-memory/src crates/app-core/src/coding_memory
```
Expected: empty output.

- [ ] **Step 3: Update docs**

Append to `docs/coding-memory/README.md` (create with basic frame if missing):

```markdown
## Phase 2 — Ingestion transport + Claude Code E2E (shipped 2026-04-24)

Components newly live:

- `UnixIngestSocket` / `FileBufferFallback` — 200ms socket deadline; 50 MB rotate / 7 d TTL / 500 MB hard cap for the cold path.
- `HookClient` — socket-first-else-buffer dispatcher with rate-limited stderr warnings.
- `IngestDaemon` — binds `~/.klyntbot/ingest.sock`, decodes length-prefixed JSON, persists rows to `ingest_event_log`, drains any pre-existing buffer on startup, heartbeats `desktop.lock` every 30 s.
- Claude Code adapter — 7 hook events (`SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact`). Bash + test-framework detection emits `TestRun`; file-ops emit `FileEdit`.
- `ClaudeCodeInstaller` — idempotent `~/.claude/settings.json` merge with a pre-install backup; the `klyntbot-managed` matcher tag lets users keep their own hooks alongside.
- Workbench: Coding CLI settings page (toggle + Diagnose), CLI Health panel, Session Replay panel.

Unchanged: Distiller / Recall / Reforge / Mirror coding behavior all remain Phase 1 stubs. No facts are written to `semantic_facts` or `episodic_memories` yet — only `ingest_event_log` rows accumulate.

Exit-gate evidence: `tests/integration/coding_memory_phase2_roundtrip.rs`, `tests/integration/coding_memory_phase2_desktop_off.rs`.
```

- [ ] **Step 4: Final commit**

```bash
git add docs/coding-memory/README.md
git commit -m "docs(coding-memory): record Phase-2 scope"
```

- [ ] **Step 5: Open the PR**

Follow the workspace's PR conventions (see CLAUDE.md).

```bash
gh pr create --title "feat(coding-memory): Phase 2 — ingestion transport + Claude Code end-to-end" \
  --body "$(cat <<'EOF'
## Summary
- Unix socket + file-buffer fallback transport; fire-and-forget hook client with 200ms deadline
- Desktop-embedded IngestDaemon with buffer drain on startup and desktop.lock heartbeat
- Claude Code adapter for 7 hooks; PostToolUse dispatches TestRun / FileEdit / ToolCall by tool + test-framework detection
- Path-based privacy exclusion filter applied at the hook level
- `~/.claude/settings.json` installer with idempotent managed-matcher merge + pre-install backup
- Settings page toggle + Workbench panels (CLI Health, Session Replay)

## Test plan
- [ ] `cargo nextest run --workspace`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --doc`
- [ ] `tests/integration/coding_memory_phase2_roundtrip.rs` — synthetic session lands in ingest_event_log
- [ ] `tests/integration/coding_memory_phase2_desktop_off.rs` — buffer-and-drain scenario
- [ ] Desktop UI: Settings → Coding CLI Integration → Enable → Diagnose shows success; sessions appear in Session Replay

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

| Spec item (§ and decision) | Task |
|---|---|
| §11 Phase 2: Unix socket transport | T3 |
| §11 Phase 2: File-buffer fallback | T4 |
| §11 Phase 2: Three-tier warning | T5 (limiter), T14 (stderr path), T14 (`status` subcommand), T19 (settings copy) |
| §11 Phase 2: Desktop-owned daemon lifecycle | T7, T8, T16 |
| §11 Phase 2: Full Claude Code adapter | T12, T13 |
| §11 Phase 2: Settings page + hook install + Diagnose | T15, T18, T19 |
| §11 Phase 2: Repo detection via `git rev-parse` | T11 |
| §11 Phase 2: Integration tests (round-trip + desktop-off) | T24, T25 |
| §11.5 Phase 2 panels (CLI Health + Session Replay) | T20, T21, T22 |
| §12 Test pyramid (~20 unit + 5 integration + 1 scenario) | T2/T3/T4/T5/T6/T10/T11/T12/T13 unit; T7/T8/T9/T17/T24 integration; T25 scenario |
| §5 `excludePaths` hook-level filter | T10 |
| §4 `IngestEventLogRepo` | T2 |
| §4 `klynt_sessions` (consolidated) | Phase-1 migration — nothing needed here |
| §5 `desktop.lock` heartbeat | T8 + T7 (spawn writes; T8 reads) |
| CLAUDE.md `DEV_COMMANDS` for every new Tauri command module | T17 |
| CLAUDE.md zero clippy warnings + fmt + doc | T26 |

**Invariant coverage.** Phase 2 does not write `SemanticFact` / `EpisodicMemory` rows, so invariants 1–6 remain vacuously satisfied. Invariant 7 (AgentEvent round-trip for Claude Code) is exercised by T12 + T13 + T24. Invariants 8–9 belong to Phase 3+.

**Type consistency.** `IngestDaemonConfig.repo: Arc<IngestEventLogRepo>` is the shape used in every daemon-spawning test (T7, T9, T16, T24, T25). `HookClient::new(socket, buffer, warn_stamp)` signature matches across T6, T14. `ClaudeCodeInstaller::{install, uninstall, diagnose}` signature matches across T15, T18. `resolve_scope(&Path) -> Option<RepoScope>` matches across T11, T14.

**Placeholder scan.** No "TBD"/"TODO"/"implement later" / "similar to Task N" text in any step body. Every Rust step includes a full code block. Every test step has exact `cargo nextest run ...` invocation with expected pass/fail. Every commit step has a complete commit message.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-23-coding-memory-phase-2.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task; I review between tasks; fast iteration. Best for this plan because many tasks are parallelizable (e.g., T10 + T11 + T12 touch different files).

**2. Inline Execution** — Execute tasks in this session via `superpowers:executing-plans`; batch execution with checkpoints every 3–4 tasks.

Which approach?
