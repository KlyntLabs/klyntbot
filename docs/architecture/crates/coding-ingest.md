# Crate: `coding-ingest`

> **Status:** 🟡 In Progress — `kimi_cli` + `opencode` hook adapters are "registered but poll-only" stubs in the hook CLI
> **Subsystem:** [09 — Coding Mode](../subsystems/09-coding-mode.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The event ingestion spine — `AgentEvent::V1`, 5 adapters, the `klyntbot-hook` binary, the daemon, and the Unix socket

---

## TL;DR

The crate that turns external CLI events into a unified `AgentEvent` stream. Five adapters (`claude_code`, `codex`, `kimi_cli`, `opencode`, `git_post_commit`) implement `IngestAdapter`. Two are hook-driven (`claude_code`, `codex`); two are poll-only (`kimi_cli`, `opencode`) — registered in the hook CLI but short-circuit with `"poll-only (Phase 7)"`. One (`git_post_commit`) is hook-driven via `.git/hooks/post-commit`.

`AgentEvent::V1` has 22 `EventKind` variants (10 base + 10 klynt-cli-only + 2 background-job). The cross-CLI normalization invariant (**Inv 7**: `parse(serialize(event)) == event`) is enforced by a 64-case proptest at `tests/cross_cli_normalization.rs`.

The `klyntbot-hook` binary and the `desktop --hook` short-circuit share `hook_cli::run()` — sub-10ms hot path that creates a fresh `new_current_thread` Tokio runtime per invocation.

---

## Module map

```
crates/coding-ingest/src/
├── lib.rs                      ← Re-exports + IngestAdapter trait
├── event.rs                    ← AgentEvent::V1 + 22 EventKind variants + AgentSource
├── hook_cli.rs                 ← run(args) — sub-10ms hot path (shared by klyntbot-hook + desktop --hook)
├── hook_client.rs              ← HookClient — Unix socket + buffer fallback
├── daemon.rs                   ← Ingest daemon (event log writer + dispatcher)
├── exclude_set.rs              ← Path/event filter
├── repo_scope.rs               ← RepoScope enrichment
├── repos.rs                    ← IngestEventLogRepo (storage layer)
│
├── adapters/
│   ├── mod.rs                  ← Re-exports
│   ├── claude_code/            ← Hook-driven (stdin JSON)
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   └── …
│   ├── codex/                  ← Hook-driven (stdin JSON) + poll fallback
│   │   ├── mod.rs              ← Note: legacy dispatch + payload modules retained as dead code
│   │   └── …
│   ├── kimi_cli/               ← Poll-only (Phase 7) — hook short-circuits
│   │   ├── mod.rs
│   │   ├── mapper.rs
│   │   ├── poller.rs
│   │   └── wire_file.rs
│   ├── opencode/               ← Poll-only (Phase 7) — hook short-circuits
│   │   ├── mod.rs
│   │   ├── poller.rs           ← OpencodePoller — SQLite diff on time_created
│   │   └── normalize.rs
│   └── git_post_commit.rs      ← Hook-driven (.git/hooks/post-commit)
│
└── bins/
    └── klyntbot-hook.rs        ← Standalone binary; wraps hook_cli::run
```

---

## Public API surface

### `IngestAdapter` trait

```rust
pub trait IngestAdapter: Send + Sync {
    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>>;
}
```

Stateless. The hook CLI passes (`hook_event` name, raw stdin bytes) and gets back `Option<AgentEvent>` — `None` means "no event to emit for this hook invocation."

### `AgentEvent::V1`

```rust
pub enum AgentEvent {
    V1(AgentEventV1),
}

pub struct AgentEventV1 {
    pub id: Uuid,
    pub source: AgentSource,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub cwd: PathBuf,
    pub repo: Option<RepoScope>,
    pub occurred_at: Timestamp,
    pub kind: EventKind,
}

pub enum AgentSource {
    ClaudeCode,
    Codex,
    KimiCli,
    OpenCode,
    KlyntCli,                    // emitted by Klynt's own coding mode
}

pub struct RepoScope {
    pub repo_root: PathBuf,
    pub repo_id: String,         // hashed/canonicalized repo identifier
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
}
```

### 22 `EventKind` variants

```rust
pub enum EventKind {
    // 10 base — emitted by all 5 adapters
    SessionStart { model: Option<String>, source_reason: String },
    SessionEnd { summary: Option<String> },
    UserPrompt { content: String },
    AssistantMsg { content: String, model: Option<String> },
    ToolCall { tool_name: String, args_preview: String, result_preview: Option<String>, duration_ms: u64 },
    FileEdit { path: PathBuf, edit_kind: FileEditKind },
    TestRun { kind: String, passed: u32, failed: u32, duration_ms: u64 },
    CompactEvent { tokens_before: u32, tokens_after: u32 },
    Error { message: String },
    GitCommit { commit_hash: String, parent_hash: Option<String>, repo_root: PathBuf, changed_files: Vec<PathBuf> },

    // 10 klynt-cli-only — high-resolution Klynt-specific events
    SkillActivated { skill_name: String, reason: SkillActivationReason },
    RecallInjected { domain: RecallDomain, item_count: u32, total_tokens: u32 },
    ApprovalDecision { tool: String, class: ApprovalClass, decision: String, decided_by: String },
    SandboxApplied { policy: String, allowed_paths: Vec<PathBuf>, network: bool },
    FileEditEnriched { path: PathBuf, lang: Option<String>, symbols_changed: Vec<String> },
    TestRunEnriched { kind: String, passed: u32, failed: u32, failures: Vec<String>, command: String },
    ProviderCall { provider: String, model: String, usage: ProviderUsage },
    CompressionApplied { compressor: String, tokens_before: u32, tokens_after: u32, ratio: f32 },
    MirrorAlert { alert_kind: String, payload: serde_json::Value },
    SkillRoutingTrace { candidates: Vec<String>, selected: Option<String> },

    // 2 background jobs — coding-bash specific
    BackgroundJobLifecycle { job_id: String, phase: BashJobPhase },
    BackgroundJobOutputBisect { job_id: String, start: u64, end: u64, summary: String },
}

pub enum FileEditKind { Create, Update, Delete, Rename }
pub enum SkillActivationReason { ProjectScope, PathMatch, UserRequest, … }

pub struct ProviderUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_usd: Option<f64>,
}

pub enum BashJobPhase {
    Started, Running, AwaitingApproval, Completed { exit: i32 },
    Failed { kind: String }, Cancelled { reason: String },
}
```

**Why Klynt produces 10 extra variants:** Internal observability is much richer than what external CLIs expose. Skill activation traces, approval decisions, sandbox policies applied, cached compression ratios, mirror alerts — all surface to cognitive at ≥5x the volume of external CLI adapter outputs. Per the deprecated 2026-04-23 spec §1.

### `hook_cli::run`

```rust
/// Sub-10ms hot path. Called by both klyntbot-hook binary and desktop --hook.
pub fn run(args: &[String]) -> Result<()>;
```

Dispatch on `args[0]`:

| `args[0]` | Behavior |
|---|---|
| `"status"` | `run_status()` — diagnostic output |
| `"context"` | `run_context()` — current ingest state |
| `"git-post-commit"` | `run_git_post_commit()` — git hook handler |
| `"claude-code"` | Parse stdin via `claude_code::adapter`; send to socket |
| `"codex"` | Parse stdin via `codex::adapter`; send to socket |
| `"kimi-cli"` | **Short-circuit** — print `"kimi-cli is poll-only (Phase 7)"`, exit 0 |
| `"opencode"` | **Short-circuit** — print `"opencode is poll-only (Phase 7)"`, exit 0 |
| anything else | Print USAGE; exit 1 |

The function creates a **fresh `new_current_thread` Tokio runtime per invocation** — single-threaded, minimum overhead. Designed to return before Claude Code's hook timeout (~3s typical, target sub-10ms).

### `HookClient` — Unix socket + buffer fallback

```rust
pub struct HookClient { /* opaque */ }

impl HookClient {
    pub fn new() -> Self;

    /// Try Unix socket first; on failure, append to buffer file.
    pub async fn send(&self, event: AgentEvent) -> Result<()>;
}

// Socket path: $KLYNTBOT_HOOK_SOCKET OR ~/.klyntbot/ingest.sock
// Buffer path: ~/.klyntbot/ingest-buffer.jsonl
```

The daemon, on startup, reads buffered events from `ingest-buffer.jsonl` and processes them. Events sent while the daemon is down aren't lost.

### `OpencodePoller`

```rust
pub struct OpencodePoller {
    db_path: PathBuf,
    poll_interval: Duration,
    tx: mpsc::UnboundedSender<AgentEvent>,
    repo: IngestEventLogRepo,
}

impl OpencodePoller {
    pub fn new(
        db_path: PathBuf,
        poll_interval: Duration,
        tx: mpsc::UnboundedSender<AgentEvent>,
        repo: IngestEventLogRepo,
    ) -> Self;

    /// Spawn as detached tokio task.
    pub fn start(self) -> JoinHandle<()>;
}
```

Polls opencode's SQLite DB (`message` + `part` tables), diffing by `time_created`. Pushes events via `mpsc::UnboundedSender`. Writes to `IngestEventLogRepo`.

### `IngestEventLogRepo`

```rust
pub struct IngestEventLogRepo { pool: StoragePool }

impl IngestEventLogRepo {
    pub fn new(pool: StoragePool) -> Self;

    pub async fn write_event(&self, event: &AgentEvent) -> Result<i64, StorageError>;
    pub async fn list_recent(&self, limit: u32) -> Result<Vec<AgentEvent>, StorageError>;
    pub async fn find_by_session(&self, session_id: &str) -> Result<Vec<AgentEvent>, StorageError>;
    pub async fn find_by_repo(&self, repo_id: &str, since: Timestamp) -> Result<Vec<AgentEvent>, StorageError>;
}
```

### Ingest daemon

```rust
pub struct IngestDaemon { /* opaque */ }

impl IngestDaemon {
    pub fn new(
        repo: IngestEventLogRepo,
        distiller: Arc<coding_memory::Distiller>,
        config: DaemonConfig,
    ) -> Self;

    pub fn start(self) -> JoinHandle<()>;
}

pub struct DaemonConfig {
    pub socket_path: PathBuf,        // default: ~/.klyntbot/ingest.sock
    pub buffer_path: PathBuf,        // default: ~/.klyntbot/ingest-buffer.jsonl
    pub poll_buffer_interval: Duration,
    // ...
}
```

The daemon:
1. Listens on the Unix socket for `AgentEvent` JSONs
2. Periodically drains the buffer file
3. Writes events to `IngestEventLogRepo`
4. Forwards events to `distiller.accept_event(event)`
5. Optionally publishes to `DomainEventBus` for other subscribers

### `RepoScope` enrichment

```rust
pub fn enrich_with_repo_scope(event: &mut AgentEvent, cwd: &Path) -> Result<()>;
```

Walks up from `cwd` to find `.git/`, computes `repo_id` (hashed canonical path), reads current branch + commit SHA. Sets `event.repo = Some(RepoScope { ... })`.

### `ExcludeSet`

```rust
pub struct ExcludeSet { /* glob set */ }

impl ExcludeSet {
    pub fn from_config(config: &CodingIngestConfig) -> Self;
    pub fn matches(&self, path: &Path) -> bool;
}
```

Filters out events for excluded paths (e.g., `node_modules/`, `target/`, `.venv/`).

---

## Internals

### `hook_cli::run` execution flow

```rust
pub fn run(args: &[String]) -> Result<()> {
    // 1. Dispatch on argv[1]
    match args.first().map(|s| s.as_str()) {
        Some("status") => return run_status(),
        Some("context") => return run_context(),
        Some("git-post-commit") => return run_git_post_commit(),
        Some("kimi-cli") => { eprintln!("kimi-cli is poll-only (Phase 7)"); return Ok(()); }
        Some("opencode") => { eprintln!("opencode is poll-only (Phase 7)"); return Ok(()); }
        Some(source) if matches!(source, "claude-code" | "codex") => { /* fall through */ }
        _ => { eprintln!("{USAGE}"); std::process::exit(1); }
    }

    // 2. Build the per-invocation Tokio runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        // 3. Read stdin
        let mut raw = Vec::new();
        std::io::stdin().read_to_end(&mut raw)?;

        // 4. Pick adapter
        let adapter: Box<dyn IngestAdapter> = match source {
            "claude-code" => Box::new(ClaudeCodeAdapter),
            "codex"       => Box::new(CodexAdapter),
            _ => unreachable!(),
        };

        // 5. Parse
        let hook_event = std::env::var("HOOK_EVENT").unwrap_or_default();
        let Some(mut event) = adapter.parse(&hook_event, &raw)? else { return Ok(()); };

        // 6. Enrich + filter
        let cwd = std::env::current_dir()?;
        enrich_with_repo_scope(&mut event, &cwd)?;
        let exclude_set = ExcludeSet::from_config(&config);
        if let EventKind::FileEdit { path, .. } = &event.kind {
            if exclude_set.matches(path) { return Ok(()); }
        }

        // 7. Send to daemon (socket; fallback to buffer)
        HookClient::new().send(event).await?;
        Ok(())
    })
}
```

### Why kimi-cli + opencode short-circuit

`kimi_cli::WireFile` polls `~/.kimi/sessions/<hash>/<uuid>/wire.jsonl`. `opencode::OpencodePoller` polls opencode's SQLite DB. Both have their own state files that the hook CLI doesn't see — hook-driven ingestion would miss most events. The CLI keeps them in the USAGE string to make the dispatch shape uniform, but short-circuits with a polite message.

### `desktop --hook` shares `hook_cli::run`

`crates/desktop/src/main.rs` first statement (after `pre_main_hardening`):

```rust
let raw_args: Vec<String> = std::env::args().collect();
if raw_args.get(1).map(|s| s.as_str()) == Some("--hook") {
    let hook_args = &raw_args[2..];
    std::process::exit(match coding_ingest::hook_cli::run(hook_args) {
        Ok(_) => 0,
        Err(e) => { eprintln!("{e}"); 1 }
    });
}
```

This makes the desktop binary triple-mode (Tauri app / MCP serve / hook ingest) with sub-10ms hot-path latency for the hook case.

### Codex adapter has dead modules

`crates/coding-ingest/src/adapters/codex/mod.rs:8` notes:
> The legacy `dispatch` and `payload` modules below are retained as dead

These are kept for migration reference but not exercised. Could be removed in cleanup.

### Cross-CLI normalization invariant (Inv 7)

The proptest at `tests/cross_cli_normalization.rs` runs 64 cases asserting:

```rust
proptest! {
    #[test]
    fn parse_serialize_round_trip(event in agent_event_strategy()) {
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(event, parsed);
    }
}
```

Covers all 5 `AgentSource` variants × 9 base `EventKind` variants. Labeled **Inv 7**. If you change `AgentEvent::V1` shape, this test fails — by design.

### Adapter parsing — claude_code example

`claude_code::adapter` parses stdin JSON per the Claude Code hook schema:
- `HOOK_EVENT=PreToolUse` → maybe emit `ToolCall { result_preview: None, ... }`
- `HOOK_EVENT=PostToolUse` → emit `ToolCall { result_preview: Some(...), duration_ms: ... }`
- `HOOK_EVENT=UserPromptSubmit` → emit `UserPrompt`
- `HOOK_EVENT=Stop` → emit `AssistantMsg`
- `HOOK_EVENT=SessionEnd` → emit `SessionEnd`
- etc.

Adapter returns `None` for hook events that don't map to an `AgentEvent` (e.g., `PreCompact` may be observability-only).

### Kimi adapter `mapper.rs:265` TODO

```rust
// crates/coding-ingest/src/adapters/kimi_cli/mapper.rs:265
// TODO(distiller): attach token usage to the prior AssistantMsg row.
```

Kimi-CLI emits token usage as a separate event after `AssistantMsg`. Current mapper doesn't back-attach the usage to the `AssistantMsg` row — token accounting incomplete for the Kimi adapter.

### `OpencodePoller` SQLite diff

```rust
// Pseudocode
loop {
    let last_seen = self.repo.last_opencode_time_created().await?;
    let new_rows: Vec<OpencodeMessage> = sqlx::query_as!(
        OpencodeMessage,
        "SELECT * FROM message WHERE time_created > ? ORDER BY time_created ASC",
        last_seen
    ).fetch_all(&opencode_db_pool).await?;

    for msg in new_rows {
        let event = opencode::normalize::to_agent_event(msg)?;
        self.tx.send(event)?;
    }

    tokio::time::sleep(self.poll_interval).await;
}
```

Idempotency via `time_created` cursor in `IngestEventLogRepo`.

---

## Workflows

### Claude Code session → AgentEvent

```
1. User invokes Claude Code in their terminal
2. Claude Code runs ~/.config/claude-code/hooks/pre-tool-use.sh:
   #!/bin/sh
   exec klyntbot-hook claude-code <<< "$HOOK_PAYLOAD"
   (HOOK_EVENT env var is set by Claude Code)
3. klyntbot-hook binary calls hook_cli::run(["claude-code"])
4. hook_cli::run:
   - Read stdin → raw bytes
   - ClaudeCodeAdapter.parse(hook_event, raw) → Option<AgentEvent>
   - enrich_with_repo_scope(event, cwd)
   - ExcludeSet filter
   - HookClient::send(event)
     - Try Unix socket at ~/.klyntbot/ingest.sock
     - On socket failure: append to ~/.klyntbot/ingest-buffer.jsonl
   - Exit
5. (Meanwhile) IngestDaemon reads from socket / buffer:
   - IngestEventLogRepo::write_event
   - distiller.accept_event(event)  ← non-blocking fire-and-forget
   - (Optional) publish to DomainEventBus
```

### Opencode session → AgentEvent

```
1. User opens an opencode session
2. opencode writes message + part rows to its local SQLite (~/.local/share/opencode/)
3. OpencodePoller (running as part of IngestDaemon):
   - Polls opencode SQLite every N seconds
   - Diffs by time_created
   - For each new row:
     - opencode::normalize::to_agent_event → AgentEvent
     - tx.send(event)
4. Receiver task: IngestEventLogRepo::write_event + distiller.accept_event
```

### Git post-commit → AgentEvent

```
1. User commits in a git repo with the post-commit hook installed
2. .git/hooks/post-commit executes:
   #!/bin/sh
   exec klyntbot-hook git-post-commit
3. klyntbot-hook binary calls hook_cli::run(["git-post-commit"])
4. run_git_post_commit():
   - Reads HEAD ref, current branch, commit metadata
   - Constructs AgentEvent::V1 with EventKind::FileEdit / EventKind::Error per changed file
   - HookClient::send each event
5. Daemon processes as usual
```

### `desktop --hook` (the integrated path)

```
1. Claude Code (or git hook) invokes:
   /Applications/KlyntBot.app/Contents/MacOS/KlyntBot --hook claude-code
2. desktop main():
   - raw_args[1] == "--hook" → call coding_ingest::hook_cli::run(&raw_args[2..])
   - Skip pre_main_hardening (no need for hook path)
   - Skip mimalloc init, Cli::parse, run_desktop_app
   - Exit with hook_cli::run result
3. Sub-10ms total.
```

---

## Testing approach

### Test an adapter

```rust
let adapter = ClaudeCodeAdapter;
let raw = b"{\"type\":\"tool_use\",\"name\":\"read\",\"input\":{...}}";

std::env::set_var("HOOK_EVENT", "PreToolUse");
let event = adapter.parse("PreToolUse", raw).unwrap();
assert!(matches!(event.unwrap().kind, EventKind::ToolCall { .. }));
```

### Test the cross-CLI normalization invariant

```rust
proptest! {
    #[test]
    fn round_trip(event in agent_event_strategy()) {
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(event, parsed);
    }
}
```

Run with `cargo nextest run -p coding-ingest --test cross_cli_normalization`.

### Mock the hook client

```rust
struct CapturingClient { events: Arc<Mutex<Vec<AgentEvent>>> }

#[async_trait]
impl HookClientLike for CapturingClient {
    async fn send(&self, event: AgentEvent) -> Result<()> {
        self.events.lock().await.push(event);
        Ok(())
    }
}
```

### Test the poller

```rust
let opencode_db = create_temp_opencode_db().await;
seed_opencode_db(&opencode_db, vec![/* test messages */]).await;

let (tx, mut rx) = mpsc::unbounded_channel();
let poller = OpencodePoller::new(
    opencode_db.path().to_path_buf(),
    Duration::from_millis(10),
    tx,
    repo,
);
let handle = poller.start();

let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
    .await.unwrap().unwrap();
assert_eq!(event.v1().source, AgentSource::OpenCode);

handle.abort();
```

### Skip the daemon for unit tests

Daemon is a long-running task. For unit tests, instantiate `IngestEventLogRepo` directly and call its methods.

---

## Extension points

### Add a new ingest adapter

1. Create `crates/coding-ingest/src/adapters/<my_cli>/mod.rs`.
2. Implement `IngestAdapter::parse`.
3. Add variant to `AgentSource` enum.
4. If hook-driven: add dispatch arm in `hook_cli::run`.
5. If poll-only: implement a daemon (see `OpencodePoller`); short-circuit in `hook_cli::run` with `"<name> is poll-only (Phase N)"`.
6. **Update the cross-CLI normalization proptest** to cover the new `AgentSource`.

### Add an `EventKind` variant

1. Add to the enum in `event.rs`.
2. Update all adapters that should emit it.
3. Update the proptest to cover round-trip serialization.
4. Update `coding-memory::Distiller` Phase A / Phase C if the new variant has special meaning.
5. Update MCP recall tools that filter by `EventKind`.

### Add a hook event handler

If a CLI introduces a new hook event (e.g., Claude Code adds `PreSubagent`):
1. Update the adapter's `parse` method to dispatch on the new `HOOK_EVENT` value.
2. Map to an existing `EventKind` if possible; add a new one only if necessary.

### Customize the daemon

`DaemonConfig` is the seam — adjust paths, poll intervals, buffer behavior.

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| Default socket path | `~/.klyntbot/ingest.sock` (or `$KLYNTBOT_HOOK_SOCKET`) | `hook_client.rs` |
| Default buffer path | `~/.klyntbot/ingest-buffer.jsonl` | `hook_client.rs` |
| `OpencodePoller` default interval | per-config | `adapters/opencode/poller.rs` |
| Cross-CLI proptest cases | `64` | `tests/cross_cli_normalization.rs` |
| `kimi-cli` / `opencode` short-circuit message | `"<name> is poll-only (Phase 7)"` | `hook_cli.rs` |

---

## Open questions

- **`kimi-cli` + `opencode` listed in hook USAGE but short-circuit.** Either remove from USAGE or implement the hook path.
- **`codex` legacy `dispatch` + `payload` modules** are dead code retained for migration. Remove in cleanup.
- **`kimi_cli::mapper.rs:265` TODO** — token usage not back-attached to `AssistantMsg`. Implement.
- **Cross-CLI proptest is 64 cases** — light. Increase if Inv 7 needs more confidence.
- **`AgentEvent::V1` is the only version** — when `V2` is added, need a migration story for stored events.
- **`OpencodePoller` polls indefinitely** — no backpressure if `mpsc::UnboundedSender` consumer can't keep up. Use bounded channel?
- **`HookClient` buffer fallback** — buffer is never trimmed; grows unboundedly if daemon stays down. Add log rotation.
- **`KlyntCli` source has 10 extra `EventKind` variants** — coupling to internal Klynt state. If protocol evolves, external CLIs must ignore unknown variants gracefully.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #1 + #3 + #9 for specifics.

---

## Cross-references

- [Subsystem 09 — Coding Mode](../subsystems/09-coding-mode.md) (parent)
- [`crates/coding-memory.md`](./coding-memory.md) — consumes the `AgentEvent` stream
- [`crates/storage.md`](./storage.md) — `IngestEventLogRepo`
- [`crates/desktop.md`](./desktop.md) *(planned)* — `--hook` short-circuit
- [Subsystem 14 — Validation](../subsystems/14-validation.md) — cross-CLI normalization proptest is part of merge gate
