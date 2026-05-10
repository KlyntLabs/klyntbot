# Coding Background Bash — Interactive Compute Design (Phase 2.3c)

**Date:** 2026-05-10
**Status:** Spec — ready for implementation plan
**Phase:** 2.3c of the long-running-task roadmap (`docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md`)
**Companion docs:**
- `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md` (Phase 2.3b — cross-run diff + episodic memory)
- `docs/superpowers/specs/2026-05-08-coding-background-bash-design.md` (Phase 2.3a — the foundation this builds on)
- `docs/superpowers/specs/2026-05-08-coding-plan-mode-design.md` (Phase 2.2 — `DynamicInjector` scaffold reused)
- `docs/superpowers/specs/2026-04-29-klynt-cognitive-architecture-design.md` (KCA — `EpisodicMemory`, `MirrorSignalSource`)

---

## 1. Goal & Scope

Phase 2.3a shipped "the LLM can run things in the background and get notified on completion." Phase 2.3b closed the cognitive loop ("the LLM learns from what those runs produced"). Phase 2.3c closes the **interactive** half: the LLM can drive interactive prompts via `coding_task_stdin`, while the user can attach an `xterm.js` terminal pane in `JobsPanel` to take over for credentials, MFA codes, or unexpected prompts. Handoff is cooperative — while the user is attached, an injected `<system-reminder>` tells the LLM to defer stdin to them.

All three sub-features (`tty: bool`, `coding_task_stdin`, `coding_task_resize`) ship together as production-grade affordances on top of the 2.3a/2.3b foundation. The `klynt-pty` crate's reserved `ChildHandle::Pty` slot is fulfilled here.

**2.3c explicit scope:**

1. **`bash` tool extension** — `BashArgs` gains `tty: Option<bool>`, `tty_rows: Option<u16>`, `tty_cols: Option<u16>` (in `crates/klynt-core/src/tools/bash.rs`). Default off; only valid with `run_in_background=true`.
2. **`klynt-pty` extension** — `ChildHandle::Pty` variant + `spawn_with_pty(cmd, rows, cols)`. Backed by `portable-pty = "0.8"` (the same crate wezterm + zellij use). Trait names already match the 2.3a forward-pointer.
3. **Two new tools in `feature-coding-bash::tools`:**
   - `coding_task_stdin(task_id, data, encoding)` — `approval_class = "sensitive"`, `approval_scope = "command"`.
   - `coding_task_resize(task_id, rows, cols)` — `approval_class = "safe"`.
4. **Supervisor extensions** — `JobSupervisor::write_stdin`, `resize`, `attach`, `detach`. `LiveJob` carries an optional `pty_master` and an `Arc<RwLock<AttachState>>`.
5. **Cooperative-handoff injector** — `BackgroundJobsInjector` reads `attached_user_at` per job and renders a per-turn `<system-reminder>` while any attach is live.
6. **`GateClassifier` ANSI-strip** — `vte`-backed sanitization runs before the existing failure regexes so colour codes don't break test/compile/lint extraction.
7. **Storage delta** — five new columns on `coding_background_jobs`: `tty`, `tty_rows`, `tty_cols`, `attached_user_at`, `attach_token`.
8. **Frontend attach UI** — `JobsPanel` row gains an `Attach` button on PTY jobs; new `AttachTerminal.tsx` (xterm.js + xterm-addon-fit) wired via a Tauri WebSocket bridge.
9. **macOS sandbox profile updates** — allow `/dev/ptmx` and `/dev/ttys[0-9]+` reads/writes for the sandboxed bash child.

**2.3c explicit non-goals:**

- **Windows ConPTY support.** `portable-pty` does abstract ConPTY; we just don't sandbox-test or build-verify it in this phase.
- **Multi-user attach.** Single live attach session per job; second concurrent attempt receives 409.
- **Remote PTY over SSH.** Local PTY only.
- **Terminal-session recording for replay** — the ring file already preserves the raw byte stream; future phase if `asciinema`-style export is wanted.
- **Frame-perfect history replay in `AttachTerminal`** — we send the ring's last 4 KB followed by live frames; xterm.js re-renders ANSI naturally. Older history requires `coding_task_output`.
- **Per-key approval gating on stdin.** Approval is per-call (the JSON args blob is the cache key); session grants apply normally.
- **Replacing the on-disk ring format** — PTY output goes into the same `RingFile` infrastructure.
- **New mirror tables** — `BackgroundJobSignalSource` (2.3b) handles attach episodes; nothing new in `mirror_*`.
- **Approval-class change for `bash`** — `tty=true` does not elevate the class.

---

## 2. Architecture Overview

The 2.3c changes touch three new wires in the agent runtime: register two new tools (`coding_task_stdin`, `coding_task_resize`), extend the `BackgroundJobsInjector` body to carry the cooperative handoff section, and mount one new axum route in `dev_server` for the WebSocket bridge. Everything else is internal to `feature-coding-bash`, `klynt-pty`, or the cognitive layer.

```
L0 platform-macos                # Verify seatbelt rules accept /dev/ptmx + dynamic /dev/ttys*
L1 klynt-sandbox/                # Profile rule update (additive — see §7)
                                 #   (allow file-read* file-write* (literal "/dev/ptmx"))
                                 #   (allow file-read* file-write* (regex #"^/dev/ttys[0-9]+$"))

L4 klynt-pty/                    # EXTENDED
       src/lib.rs                #   ChildHandle::Pty variant fulfilled
       src/pty_backend.rs        # NEW: portable-pty integration; sync→async adapters

L4 feature-coding-bash/          # EXTENDED
       src/spawner.rs            #   tty-aware path: dispatches Pty vs Process branches
       src/supervisor.rs         #   +pty_master per LiveJob; +write_stdin/+resize/+attach/+detach
       src/ring.rs               #   Single-stream merged-ring path for PTY mode
       src/gate.rs               #   +ansi_strip(&str) -> String via vte = "0.13"
       src/injector.rs           #   +cooperative_handoff section when attached_user_at present
       src/tools/coding_task_stdin.rs    # NEW
       src/tools/coding_task_resize.rs   # NEW
       src/attach/                       # NEW submodule
           mod.rs
           bridge.rs             #   PtyAttachBridge: WebSocket ↔ PTY bidirectional pump
           token.rs              #   16-byte url-safe attach tokens
       src/migrations.rs         #   FeatureMigration version 2→3: 5 new columns (pre-release drop+recreate)

L2 storage/src/repos/coding_background_jobs.rs
                                 # +tty, +tty_rows, +tty_cols, +attached_user_at, +attach_token on BashJobRow
                                 # +mark_attached / +clear_attached / +find_by_attach_token methods

L1 bus/src/domain_events.rs      # +BashJobEvent::AttachStarted/AttachEnded variants

L4 app-core/src/init/ai_pipeline.rs    # extend BashJob arm to translate the new variants

L7 desktop/src/commands/coding_jobs.rs # +coding_task_attach / coding_task_detach (klynt_command)
L7 desktop/src/dev_server/attach_ws.rs # NEW: axum WebSocket route /api/coding/jobs/:id/attach
L7 desktop/src/specta_builder.rs       # register coding_task_attach + coding_task_detach

UI desktop-ui/src/features/coding/
       components/JobsPanel.tsx           #   +Attach button on tty=1 rows (lazy-loads AttachTerminal)
       components/AttachTerminal.tsx      # NEW: xterm.js + addon-fit + addon-web-links
       hooks/useAttachSession.ts          # NEW: WS lifecycle + token retry
       state/attachStore.ts               # NEW: { activeAttach: Option<{ job_id, ws }> }
```

**Key design choices:**

- **`portable-pty` as the backend.** Cross-platform (Linux + Mac today, Windows ready), exposes the `MasterPty`/`Child` traits the 2.3a spec already named. Same crate wezterm + zellij + atuin use in production.
- **Single attach submodule inside `feature-coding-bash`.** All WebSocket-bridge / handoff logic lives in `feature-coding-bash::attach`. `desktop` only owns the Tauri command shell + axum route registration; the bridge is testable in isolation (no Tauri dependency).
- **`vte` for ANSI stripping in the gate.** Streaming, allocation-light, the same parser alacritty + wezterm use. Strips colour / cursor / OSC sequences before the existing failure-extraction regexes run.
- **Storage delta is additive.** Five new columns; pre-release we drop+recreate per `CLAUDE.md`. Post-release this becomes an `ALTER TABLE` migration.
- **No new `RecallDomain`.** Attach-lifecycle events publish via the existing `BashJobEvent` family; `BackgroundJobSignalSource` (2.3b) writes `kind="bash_job_attach"` episodes via the same translator pipeline that already routes `BashJob.*` events.
- **xterm.js code-splitting.** Dynamic `import("xterm")` inside `AttachTerminal` — the addon is lazy-loaded only when the user clicks Attach, keeping the main bundle small.

---

## 3. Tool Surface

### 3.1 Extended `bash` tool (klynt-core)

```rust
// crates/klynt-core/src/tools/bash.rs (extended)

#[derive(Debug, Clone, serde::Serialize, ToolParams)]
pub struct BashArgs {
    pub command: String,
    pub timeout_ms: Option<u64>,
    pub cwd: Option<String>,
    pub run_in_background: Option<bool>,
    pub description: Option<String>,
    pub silent_completion: Option<bool>,

    /// Allocate a PTY for the command. Only valid with run_in_background=true.
    /// Enables ANSI/colour passthrough and accepts stdin via coding_task_stdin.
    pub tty: Option<bool>,

    /// PTY rows. Default 24. Only valid when tty=true.
    pub tty_rows: Option<u16>,

    /// PTY cols. Default 80. Only valid when tty=true.
    pub tty_cols: Option<u16>,
}
```

Validation (in `BashTool::execute` before any spawn):

| Condition | Outcome |
|---|---|
| `tty=Some(true)` and `run_in_background ≠ Some(true)` | Reject: `"tty=true requires run_in_background=true"` |
| `tty_rows`/`tty_cols` set but `tty != Some(true)` | Reject: `"tty_rows/tty_cols require tty=true"` |
| `tty_rows ∉ [4, 200]` or `tty_cols ∉ [20, 400]` | Clamp to range; emit warning line in tool result |
| `run_in_background=true`, `tty=true`, `description` missing | Reject: 2.3a invariant unchanged |
| All other cases | Accept; defaults rows=24, cols=80 |

Tool result (background + tty):

```text
Started background PTY job bash-aB3kF7c2qR.
Description: gh auth login
TTY: 24 rows × 80 cols
Send stdin:    coding_task_stdin("bash-aB3kF7c2qR", "y\n")
Resize:        coding_task_resize("bash-aB3kF7c2qR", rows, cols)
Inspect output: coding_task_output("bash-aB3kF7c2qR")
Cancel:        coding_task_stop("bash-aB3kF7c2qR")

The user may attach via JobsPanel. While attached, defer stdin to them.
```

The non-PTY result line stays identical to 2.3a. The tool returns within ~100 ms regardless of underlying lifetime.

### 3.2 `coding_task_stdin`

```rust
// crates/feature-coding-bash/src/tools/coding_task_stdin.rs

#[derive(ToolParams)]
pub struct CodingTaskStdinArgs {
    #[param(required)]
    pub task_id: String,
    /// Bytes to send. UTF-8 if encoding="utf8" (default), or base64-decoded
    /// if encoding="base64".
    #[param(required)]
    pub data: String,
    /// "utf8" (default) or "base64". Use base64 for control characters
    /// (Ctrl-C "\x03", Ctrl-D "\x04", arrow keys "\x1b[A").
    pub encoding: Option<String>,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_stdin",
    description = "Send bytes to the stdin of a background PTY job.",
    params = "CodingTaskStdinArgs",
    allowed_channels = "coding_only",
    approval_class = "sensitive",
    approval_scope = "command"
)]
pub struct CodingTaskStdinTool;
```

Tool result: `"Sent 7 bytes to bash-aB3kF7c2qR."`.

Errors: job not found, job not PTY (`"job has no PTY; spawn with tty=true to enable stdin"`), terminal-state job (`"job is in state Failed; stdin ignored"`), encoding decode error.

The tool **does not block** when the user is attached — the cooperative reminder handles deferral. `approval_scope = "command"` means the JSON-encoded args blob keys persistent grants; identical repeated payloads (e.g. `"y\n"` to the same job) hit the cache.

### 3.3 `coding_task_resize`

```rust
// crates/feature-coding-bash/src/tools/coding_task_resize.rs

#[derive(ToolParams)]
pub struct CodingTaskResizeArgs {
    #[param(required)] pub task_id: String,
    #[param(required)] pub rows: u16,
    #[param(required)] pub cols: u16,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_resize",
    description = "Resize the PTY of a background job. Sends SIGWINCH to the child.",
    params = "CodingTaskResizeArgs",
    allowed_channels = "coding_only",
    approval_class = "safe"
)]
pub struct CodingTaskResizeTool;
```

`Safe` class — resize is metadata-only from a security perspective. Same clamps as the spawn-time bounds: rows `[4, 200]`, cols `[20, 400]`. Tool result: `"Resized bash-aB3kF7c2qR to 30 rows × 120 cols."`.

### 3.4 Tauri commands for the frontend (NOT LLM-facing)

```rust
#[klynt_command]
pub async fn coding_task_attach(thread_id: String, task_id: String) -> AttachHandle;

#[klynt_command]
pub async fn coding_task_detach(thread_id: String, task_id: String) -> ();
```

Returned `AttachHandle` shape:

```rust
pub struct AttachHandle {
    pub ws_url:    String,    // "ws://localhost:3456/api/coding/jobs/<id>/attach?token=…"
    pub rows:      u16,
    pub cols:      u16,
    pub tail_b64:  String,    // last 4 KB of ring file, base64 — primes xterm.js immediately
}
```

`coding_task_attach` issues a fresh 16-byte url-safe token (`base64::URL_SAFE_NO_PAD` encoding, 22-char output), persists it via `BashJobRepo::mark_attached`, and returns the URL the frontend should connect to. The WS handler verifies the token before opening the bridge. `mark_attached` is atomic — a second concurrent attach attempt receives `AttachError::AlreadyAttached`. `coding_task_detach` clears the attach state idempotently; the bridge also auto-detaches on WS close.

Both commands are added to `desktop_macros::klynt_collect_commands![...]` in `specta_builder.rs`. `cargo tauri dev` regenerates `desktop-ui/src/bindings.ts`.

---

## 4. Data Model

### 4.1 SQLite schema extension

Single in-place migration (pre-release; `FeatureMigration::version` 2 → 3, drop+recreate per `CLAUDE.md`):

```sql
-- crates/feature-coding-bash/src/migrations.rs (FeatureMigration { version: 3, ... })

CREATE TABLE coding_background_jobs (
    id                    TEXT PRIMARY KEY,
    session_id            TEXT NOT NULL,
    agent_id              TEXT NOT NULL,
    description           TEXT NOT NULL,
    command               TEXT NOT NULL,
    command_key           TEXT NOT NULL,                       -- 2.3b
    cwd                   TEXT NOT NULL,
    timeout_ms            INTEGER NOT NULL,
    silent_completion     INTEGER NOT NULL DEFAULT 0,

    tty                   INTEGER NOT NULL DEFAULT 0,          -- NEW: 0=Process, 1=Pty
    tty_rows              INTEGER,                              -- NEW: NULL when tty=0
    tty_cols              INTEGER,                              -- NEW: NULL when tty=0
    attached_user_at      TEXT,                                 -- NEW: RFC3339; NULL = not attached
    attach_token          TEXT,                                 -- NEW: 22-char url-safe; NULL when no live attach

    status                TEXT NOT NULL,
    exit_code             INTEGER,
    failure_kind          TEXT,
    failure_detail        TEXT,
    failure_extracted     TEXT,
    started_at            TEXT NOT NULL,
    finished_at           TEXT,
    total_bytes_emitted   INTEGER NOT NULL DEFAULT 0,
    bisect_count          INTEGER NOT NULL DEFAULT 0,
    log_path              TEXT NOT NULL,
    final_path            TEXT,
    last_polled_at        TEXT,
    last_seen_offset      INTEGER NOT NULL DEFAULT 0,

    CHECK (status IN ('Starting','Running','Completed','Failed','Cancelled','Lost')),
    CHECK (failure_kind IS NULL OR status IN ('Failed','Cancelled','Lost')),
    CHECK (tty IN (0, 1)),
    CHECK (tty = 0 OR (tty_rows IS NOT NULL AND tty_cols IS NOT NULL)),
    CHECK (attached_user_at IS NULL OR tty = 1),
    CHECK ((attached_user_at IS NULL) = (attach_token IS NULL)),
    FOREIGN KEY (session_id) REFERENCES coding_sessions(id) ON DELETE CASCADE
);

-- 2.3a + 2.3b indexes preserved.
CREATE INDEX idx_cbj_session_status ON coding_background_jobs(session_id, status);
CREATE INDEX idx_cbj_active        ON coding_background_jobs(status) WHERE status IN ('Starting','Running');
CREATE INDEX idx_cbj_session_command_key
    ON coding_background_jobs(session_id, command_key, started_at DESC);

-- NEW (2.3c)
CREATE INDEX idx_cbj_attached
    ON coding_background_jobs(session_id, attached_user_at)
    WHERE attached_user_at IS NOT NULL;
```

The two paired CHECK constraints are the load-bearing invariants: a job is either non-PTY (`tty=0`, rows/cols/attached/token all NULL) or PTY (`tty=1`, rows/cols mandatory, attach state independently nullable but always paired). The DB rejects malformed inserts; consumers can trust the row shape.

### 4.2 `BashJobRow` extension

```rust
// crates/storage/src/repos/coding_background_jobs.rs (extended)

pub struct BashJobRow {
    // ... all existing 2.3a + 2.3b fields ...
    pub tty:              bool,
    pub tty_rows:         Option<u16>,
    pub tty_cols:         Option<u16>,
    pub attached_user_at: Option<jiff::Timestamp>,
    pub attach_token:     Option<String>,
}

impl BashJobRepo {
    /// Mark a job as user-attached. Issues a fresh attach_token if `token` is None;
    /// otherwise persists the supplied token. Atomic: returns AttachError::AlreadyAttached
    /// if attached_user_at is already non-null.
    pub async fn mark_attached(
        &self, job_id: &str, token: Option<&str>,
    ) -> Result<String, AttachError>;

    /// Clear attached state. Always succeeds, even if already null.
    pub async fn clear_attached(&self, job_id: &str) -> Result<()>;

    /// Look up a job by attach token. Used by the WS handler at connection time.
    pub async fn find_by_attach_token(&self, token: &str) -> Result<Option<BashJobRow>>;
}
```

All other 2.3a/2.3b methods (`upsert`, `get`, `list_for_session`, `find_prior_by_command_key`, etc.) keep their signatures; their bodies extend to include the five new columns in the row mapping.

### 4.3 `BashJobEvent` extension (bus)

```rust
// crates/bus/src/domain_events.rs (extended)

pub enum BashJobEvent {
    Started   { /* … */ },
    Completed { /* … */ },
    Failed    { /* … */ },
    Cancelled { /* … */ },
    Lost      { /* … */ },

    AttachStarted {
        job_id:    String,
        thread_id: String,
        agent_id:  String,
        timestamp: jiff::Timestamp,
    },
    AttachEnded {
        job_id:      String,
        thread_id:   String,
        agent_id:    String,
        timestamp:   jiff::Timestamp,
        duration_ms: u64,
    },
}
```

The 2.3b accessor methods (`job_id()`, `thread_id()`, `agent_id()`) extend to cover the new variants. `ai_pipeline::translate` (2.3b) gains two more event-kind strings (`"BashJob.AttachStarted"`, `"BashJob.AttachEnded"`) with the same body shape. `BackgroundJobSignalSource` (2.3b) subscribes to these new kinds and writes one episode per attach lifecycle, `kind="bash_job_attach"`, importance 0.4.

### 4.4 `LiveJob` extension (in-memory)

```rust
// crates/feature-coding-bash/src/supervisor.rs (extended)

pub struct LiveJob {
    pub id: JobId,
    pub spec: JobSpec,
    pub started_at: jiff::Timestamp,
    // ... existing 2.3a/2.3b fields ...

    pub backend: ChildBackend,
    pub attach:  Arc<RwLock<AttachState>>,
}

pub enum ChildBackend {
    Process,
    Pty {
        master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
        child:  Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
        rows:   AtomicU16,
        cols:   AtomicU16,
    },
}

pub struct AttachState {
    pub user_at: Option<jiff::Timestamp>,
    pub token:   Option<String>,
    /// Outbound channel: PTY reader sends bytes; WS bridge forwards them.
    /// Some(_) iff a live websocket is attached.
    pub ws_tx:   Option<UnboundedSender<Vec<u8>>>,
}
```

`MasterPty` is `Send` but not `Sync` in `portable-pty 0.8`, so `Mutex` is mandatory. The mutex is held only briefly on `write_stdin` and `resize`; PTY output uses `try_clone_reader()` which yields an owned reader living in its own task. `AtomicU16` for rows/cols lets concurrent injectors render the size lock-free.

---

## 5. Components

### 5.1 `klynt-pty` extension

```rust
// crates/klynt-pty/src/lib.rs (extended; ChildHandle::Pty arm fulfilled)

pub enum ChildHandle {
    Process { child: tokio::process::Child },
    Pty {
        master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
        child:  Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
        pgid:   Option<u32>,
    },
}

/// Spawn `cmd` inside a PTY of (rows × cols). Mirrors spawn_with_pgrp's
/// contract but goes through portable-pty's PtySystem instead of tokio::Command.
pub fn spawn_with_pty(
    cmd: portable_pty::CommandBuilder,
    rows: u16,
    cols: u16,
) -> Result<BackgroundCommandHandle, PtyError> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system.openpty(portable_pty::PtySize {
        rows, cols, pixel_width: 0, pixel_height: 0,
    })?;
    let child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);                                          // close slave fd in parent

    let reader = pair.master.try_clone_reader()?;              // owned reader
    let writer = pair.master.take_writer()?;                   // owned writer (stdin)
    let pid = child.process_id();
    let pgid = pid.and_then(|p| {
        #[cfg(unix)] unsafe {
            let g = libc::getpgid(p as i32);
            if g < 0 { None } else { Some(g as u32) }
        }
        #[cfg(not(unix))] None
    });

    Ok(BackgroundCommandHandle {
        child: ChildHandle::Pty {
            master: Arc::new(Mutex::new(pair.master)),
            child:  Arc::new(Mutex::new(child)),
            pgid,
        },
        // PTY merges stdout+stderr at the kernel level — single reader, no stderr.
        stdout: Box::new(BlockingReaderToAsync::new(reader)) as _,
        stderr: None,
        stdin:  Some(Box::new(BlockingWriterToAsync::new(writer)) as _),
        pgid,
    })
}
```

`portable-pty` exposes blocking `Read`/`Write`. `BlockingReaderToAsync` / `BlockingWriterToAsync` are thin `tokio::task::spawn_blocking`-backed adapters in `klynt-pty::pty_backend` (~25 LOC each). The `cmd` argument is a `portable_pty::CommandBuilder`, not `tokio::process::Command` — the spawner converts via `feature-coding-bash::spawner::build_pty_command(spec)` with the same env (`GIT_EDITOR=true`, `PAGER=cat`, **`TERM=xterm-256color` instead of `dumb`**), the same cwd, and the same `setpgid` pre-exec on Unix.

### 5.2 `JobSupervisor` extensions

```rust
// crates/feature-coding-bash/src/supervisor.rs (extended)

impl JobSupervisor {
    async fn spawn(&self, spec: JobSpec) -> Result<JobView, JobError> {
        // ... existing prelude ...
        let handle = if spec.tty {
            spawner::spawn_pty(&spec).await?
        } else {
            spawner::spawn_process(&spec).await?
        };
        let backend = ChildBackend::from_handle(&handle, spec.tty_rows, spec.tty_cols);
        let live = Arc::new(LiveJob {
            id: id.clone(),
            spec,
            backend,
            attach: AttachState::default().into_rwlock(),
            // ...
        });
        // existing: spawn reader tasks, wait task, register in DashMap, persist row
        Ok(view)
    }

    pub async fn write_stdin(&self, id: &JobId, data: &[u8]) -> Result<usize, JobError> {
        let live = self.lookup(id)?;
        let ChildBackend::Pty { master, .. } = &live.backend else {
            return Err(JobError::NotPty);
        };
        let mut master = master.lock().await;
        let mut writer = master.take_writer().map_err(JobError::Pty)?;
        let n = data.len();
        let bytes = data.to_vec();
        tokio::task::spawn_blocking(move || writer.write_all(&bytes))
            .await
            .map_err(|e| JobError::Other(e.to_string()))??;
        Ok(n)
    }

    pub async fn resize(&self, id: &JobId, rows: u16, cols: u16) -> Result<(), JobError> {
        let live = self.lookup(id)?;
        let ChildBackend::Pty { master, rows: r, cols: c, .. } = &live.backend else {
            return Err(JobError::NotPty);
        };
        let mut master = master.lock().await;
        master.resize(portable_pty::PtySize {
            rows, cols, pixel_width: 0, pixel_height: 0,
        }).map_err(JobError::Pty)?;
        r.store(rows, Ordering::Relaxed);
        c.store(cols, Ordering::Relaxed);
        Ok(())
    }

    pub async fn attach(&self, id: &JobId) -> Result<AttachHandle, JobError> {
        let live = self.lookup(id)?;
        if !matches!(live.backend, ChildBackend::Pty { .. }) {
            return Err(JobError::NotPty);
        }
        let token = generate_attach_token();        // 16-byte url-safe
        self.repo.mark_attached(id.as_str(), Some(&token)).await?;
        {
            let mut state = live.attach.write().await;
            state.user_at = Some(jiff::Timestamp::now());
            state.token   = Some(token.clone());
            // ws_tx is set by the WS handler when the bridge connects
        }
        self.bus.publish_bash_job(BashJobEvent::AttachStarted { /* … */ });
        let tail = self.tail_b64(id, 4096).await?;
        Ok(AttachHandle { ws_url: format_ws_url(id, &token), rows, cols, tail_b64: tail })
    }

    pub async fn detach(&self, id: &JobId) -> Result<(), JobError> {
        let live = self.lookup(id)?;
        let started_at = {
            let mut state = live.attach.write().await;
            let ts = state.user_at.take();
            state.token  = None;
            state.ws_tx  = None;
            ts
        };
        self.repo.clear_attached(id.as_str()).await?;
        if let Some(ts) = started_at {
            let duration_ms = (jiff::Timestamp::now() - ts).as_millis() as u64;
            self.bus.publish_bash_job(BashJobEvent::AttachEnded {
                /* … duration_ms */
            });
        }
        Ok(())
    }

    /// Wire the outbound channel so the PTY reader task can fan output to the WS.
    pub async fn set_attach_channel(
        &self, id: &JobId, tx: UnboundedSender<Vec<u8>>,
    ) -> Result<(), JobError> {
        let live = self.lookup(id)?;
        live.attach.write().await.ws_tx = Some(tx);
        Ok(())
    }
}
```

`write_stdin` uses `spawn_blocking` because portable-pty's writer is sync. The mutex is held only for the duration of the writer take + write — the reader task has its own cloned handle and is never blocked by stdin.

### 5.3 PTY-mode `RingFile`

The 2.3a `RingFile` is fed by two reader tasks (stdout + stderr). PTY mode merges both at the kernel level, so we run a single reader. No `RingFile` API change is needed — the existing `append(&[u8])` accepts whatever the reader feeds it. The change is purely in `supervisor::start_readers`:

```rust
match &live.backend {
    ChildBackend::Process => {
        spawn_reader_task(stdout_pipe, ring.clone(), attach.clone(), "stdout");
        spawn_reader_task(stderr_pipe, ring.clone(), attach.clone(), "stderr");
    }
    ChildBackend::Pty { .. } => {
        spawn_reader_task(stdout_pipe, ring.clone(), attach.clone(), "pty");
    }
}
```

Each reader task takes a chunk, writes to the ring, then takes a read lock on `attach` and forks the bytes to `ws_tx` if `Some(_)`. Tokio's `RwLock` is the right primitive here — reads dominate (output chunks are frequent; attach state changes are rare).

### 5.4 `GateClassifier` ANSI strip

```rust
// crates/feature-coding-bash/src/gate.rs (extended)

use vte::{Parser, Perform};

#[derive(Default)]
struct AnsiStripPerform { out: String }

impl Perform for AnsiStripPerform {
    fn print(&mut self, c: char) { self.out.push(c); }
    fn execute(&mut self, b: u8) { if b == b'\n' || b == b'\t' { self.out.push(b as char); } }
    fn csi_dispatch(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {}
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {}
    fn esc_dispatch(&mut self, _: &[u8], _: bool, _: u8) {}
    fn hook(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
}

pub fn strip_ansi(input: &str) -> String {
    let cap = input.len().min(64 * 1024);
    let bounded = &input[input.len().saturating_sub(cap)..];
    let mut parser = Parser::new();
    let mut perform = AnsiStripPerform::default();
    for byte in bounded.bytes() { parser.advance(&mut perform, byte); }
    perform.out
}
```

Called once per gate-classifier invocation against the tail of the ring before the existing regex extractors run. The 64 KB tail cap matches the existing gate's read budget. `vte` is the same parser alacritty + wezterm use — battle-tested, allocation-light, streaming.

### 5.5 `BackgroundJobsInjector` cooperative-handoff section

```rust
// crates/feature-coding-bash/src/injector.rs (extended)

fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
    let active = self.supervisor.list(ctx.thread_id(), ctx.agent_chain(), true);
    let mut sections = Vec::new();
    if !active.is_empty() {
        sections.push(render_active_jobs_section(&active));      // existing 2.3a body
    }
    let attached: Vec<_> = active.iter()
        .filter_map(|j| j.attached_user_at.map(|ts| (j, ts)))
        .collect();
    if !attached.is_empty() {
        sections.push(render_attach_handoff_section(&attached));  // NEW (2.3c)
    }
    if sections.is_empty() { return vec![]; }
    let body = wrap_in_system_reminder(sections.join("\n\n"));
    vec![ContextUpdate {
        reason:   ContextUpdateReason::CodingJobsChanged,
        priority: ContextUpdatePriority::Standard,
        content:  Some(body),
        ..Default::default()
    }]
}
```

Rendered handoff section:

```xml
<system-reminder>
The user is currently attached to the following PTY jobs:
- bash-aB3kF7c2qR (gh auth login) — attached at 14:32 local
Defer stdin to the user. Do NOT call coding_task_stdin on these jobs while
attached. You may still observe their output via coding_task_output. The
attach indicator clears automatically when the user closes the panel.
</system-reminder>
```

### 5.6 `attach::PtyAttachBridge`

```rust
// crates/feature-coding-bash/src/attach/bridge.rs

pub struct PtyAttachBridge {
    job_id:     JobId,
    supervisor: Arc<dyn JobSupervisorHandle>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind")]
enum ControlFrame {
    #[serde(rename = "resize")]
    Resize { rows: u16, cols: u16 },
}

impl PtyAttachBridge {
    /// Bidirectional pump. Drives until the WS closes or the job terminates.
    pub async fn run<S>(&self, mut ws: WebSocketStream<S>) -> Result<(), AttachError>
    where S: AsyncRead + AsyncWrite + Unpin + Send + 'static {
        let (mut ws_tx, mut ws_rx) = ws.split();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.supervisor.set_attach_channel(&self.job_id, out_tx).await?;

        let id = self.job_id.clone();
        let supervisor = self.supervisor.clone();

        let outbound = async move {
            while let Some(bytes) = out_rx.recv().await {
                if ws_tx.send(WsMessage::Binary(bytes)).await.is_err() { break; }
            }
        };

        let inbound = async move {
            while let Some(msg) = ws_rx.next().await {
                match msg.map_err(AttachError::Ws)? {
                    WsMessage::Binary(bytes) => {
                        supervisor.write_stdin(&id, &bytes).await?;
                    }
                    WsMessage::Text(s) => {
                        // Text frames carry control messages (resize).
                        if let Ok(frame) = serde_json::from_str::<ControlFrame>(&s) {
                            match frame {
                                ControlFrame::Resize { rows, cols } => {
                                    supervisor.resize(&id, rows, cols).await?;
                                }
                            }
                        } else {
                            supervisor.write_stdin(&id, s.as_bytes()).await?;
                        }
                    }
                    WsMessage::Close(_) => break,
                    _ => {}
                }
            }
            Ok::<(), AttachError>(())
        };

        tokio::select! {
            _ = outbound => {}
            r = inbound  => { r?; }
        }
        self.supervisor.detach(&self.job_id).await?;
        Ok(())
    }
}
```

Fully testable without a real websocket: tests use `tokio::io::duplex()` + `tokio_tungstenite::WebSocketStream::from_raw_socket(...)` and drive frames manually. The framing is: binary frames are stdin; text frames are tried as JSON `ControlFrame` first and fall through to stdin if the parse fails (so xterm.js's default text-mode `onData` keystrokes go to stdin without the frontend having to opt into binary mode for input). The bridge never touches PTY internals — every operation goes through `JobSupervisor` methods.

### 5.7 attach-token issuance

```rust
// crates/feature-coding-bash/src/attach/token.rs

pub fn generate_attach_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}

pub fn tokens_eq_constant_time(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
```

22-char output. Single-use per attach session. The WS handler uses the constant-time compare against the row's `attach_token`.

### 5.8 Frontend: `AttachTerminal.tsx`

```typescript
// desktop-ui/src/features/coding/components/AttachTerminal.tsx

import { useEffect, useRef } from "react";
import { invoke } from "@/api/client";

export function AttachTerminal({ jobId, threadId }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    let cleanup: (() => void) | null = null;

    (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([
        import("xterm"),
        import("xterm-addon-fit"),
      ]);
      const term = new Terminal({
        fontFamily: 'var(--ff-mono, "SF Mono", monospace)',
        fontSize: 13.5,
        cursorBlink: true,
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      if (!ref.current || cancelled) { term.dispose(); return; }
      term.open(ref.current);
      fit.fit();

      const handle = await invoke<AttachHandle>("coding_task_attach",
        { threadId, taskId: jobId });
      if (cancelled) { term.dispose(); return; }

      // Prime with last 4 KB of ring tail
      term.write(atob(handle.tail_b64));

      const ws = new WebSocket(handle.ws_url);
      ws.binaryType = "arraybuffer";
      ws.onmessage = (e) => term.write(new Uint8Array(e.data as ArrayBuffer));
      ws.onclose = () => term.write("\r\n[detached]\r\n");

      term.onData((s) => {
        if (ws.readyState === WebSocket.OPEN) ws.send(s);
      });
      term.onResize(({ rows, cols }) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ kind: "resize", rows, cols }));
        }
      });

      cleanup = () => {
        ws.close();
        term.dispose();
        invoke("coding_task_detach", { threadId, taskId: jobId }).catch(() => {});
      };
    })();

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [jobId, threadId]);

  return <div className="coding-jobs-panel__attach-term" ref={ref} />;
}
```

xterm.js + xterm-addon-fit are dynamically imported so the main bundle stays small; the addons load only when the user opens the panel.

### 5.9 axum WebSocket route

```rust
// crates/desktop/src/dev_server/attach_ws.rs

#[derive(serde::Deserialize)]
struct AttachQuery { token: String }

pub fn route() -> Router<AppState> {
    Router::new().route("/api/coding/jobs/:id/attach", get(handler))
}

async fn handler(
    Path(id): Path<String>,
    Query(q): Query<AttachQuery>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let Ok(Some(row)) = state.bash_repo.find_by_attach_token(&q.token).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !tokens_eq_constant_time(row.attach_token.as_deref().unwrap_or(""), &q.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if row.id != id { return StatusCode::FORBIDDEN.into_response(); }

    ws.on_upgrade(move |socket| async move {
        let bridge = PtyAttachBridge::new(
            JobId::from_str(&row.id).expect("valid job id"),
            state.supervisor.clone(),
        );
        let _ = bridge.run(socket).await;
    })
}
```

Mounted alongside the existing `dev_server` routes. Production builds use the same Tauri-managed localhost server (default port 3456; `KLYNTBOT_DEV_PORT` env override).

### 5.10 Migration

`feature-coding-bash::migrations::FeatureMigration::version` bumps 2 → 3. The SQL drops the existing table and recreates with the schema in §4. Pre-release: zero-data migration. Post-release this becomes a multi-statement `ALTER TABLE` migration (out of scope here).

---

## 6. Data Flow

### 6.1 PTY spawn flow

```
LLM calls bash(command="gh auth login", run_in_background=true, tty=true,
                description="OAuth login")
   │
   ▼
BashTool::execute (klynt-core)
   │  · validate: tty=true requires run_in_background=true ✔
   │  · clamp tty_rows/tty_cols to [4,200] / [20,400]
   │  · ApprovalGate::check (Destructive) — already gated; user grant cached
   │
   ▼
JobSupervisor::spawn(spec { tty: true, tty_rows: 24, tty_cols: 80 })
   │
   ├── spawner::spawn_pty(spec)
   │     ├── build_pty_command(spec) → portable_pty::CommandBuilder
   │     │     · cwd, env (GIT_EDITOR=true, PAGER=cat, TERM=xterm-256color)
   │     │     · setpgid pre_exec hook on Unix
   │     ├── klynt_pty::spawn_with_pty(cmd, 24, 80)
   │     │     · pty_system.openpty()  → master + slave pair
   │     │     · slave.spawn_command(cmd)
   │     │     · drop slave fd
   │     │     · master.try_clone_reader() / take_writer()
   │     │     · capture pgid via getpgid(child.process_id())
   │     └── return BackgroundCommandHandle { ChildHandle::Pty, single stdout, no stderr, stdin Some, pgid }
   │
   ├── construct LiveJob { backend: ChildBackend::Pty { master, child, rows=24, cols=80 } }
   ├── spawn_reader_task(merged stream → ring + ws_tx if attached)
   ├── repo.upsert(BashJobRow { tty=1, tty_rows=24, tty_cols=80, attached_user_at=NULL, … })
   ├── bus.publish_bash_job(BashJobEvent::Started { … })
   └── return JobView { id: bash-aB3kF7c2qR, tty: true, tty_rows: 24, tty_cols: 80 }
   ▼
Tool result (~100 ms): Started background PTY job bash-aB3kF7c2qR. …
```

### 6.2 LLM stdin flow

```
LLM (per-turn injector reminded of bash-aB3kF7c2qR; sees prompt "Username:")
   │
   ▼
LLM calls coding_task_stdin(task_id, "myuser\n")
   │
   ▼
CodingTaskStdinTool::execute
   │  · ApprovalGate::check (Sensitive, scope=command, payload-keyed)
   │  · decode if encoding="base64"; else UTF-8 bytes
   │
   ▼
JobSupervisor::write_stdin(id, &bytes)
   │  · lookup live job; verify ChildBackend::Pty
   │  · master.lock() → take_writer() → spawn_blocking { writer.write_all(bytes) }
   │
   ├── PTY child sees the bytes on its stdin (kernel-level)
   ├── child responds; output flows through merged stream → reader task → ring file
   ├── if attached: same bytes also fanned to ws_tx → user sees them in xterm.js
   └── return Ok(7) — 7 bytes written
   ▼
Tool result: Sent 7 bytes to bash-aB3kF7c2qR.
```

If the user is attached when the LLM sends stdin, **both writes interleave** at the byte level on the PTY master. The cooperative reminder is the safeguard — the LLM is told not to do this.

### 6.3 LLM resize flow

```
LLM calls coding_task_resize(task_id, 30, 120)
   │
   ▼
JobSupervisor::resize(id, 30, 120)
   ├── master.lock() → master.resize(PtySize { rows: 30, cols: 120, ... })
   │   · portable-pty issues TIOCSWINSZ ioctl
   │   · kernel sends SIGWINCH to the foreground process group
   ├── store rows/cols into ChildBackend::Pty atomics
   └── (no DB update — runtime metadata; reflected via coding_task_list)
   ▼
Tool result: Resized bash-aB3kF7c2qR to 30 rows × 120 cols.
```

### 6.4 User attach flow (cooperative handoff)

```
User clicks [Attach] on the JobsPanel row for bash-aB3kF7c2qR
   │
   ▼
React: invoke("coding_task_attach", { thread_id, task_id })
   │
   ▼
desktop::commands::coding_task_attach (klynt_command)
   │
   ▼
AppCore::coding_task_attach handler
   ├── JobSupervisor::attach(id)
   │     ├── lookup live; verify ChildBackend::Pty
   │     ├── token = generate_attach_token()
   │     ├── repo.mark_attached(id, Some(&token))
   │     │     ↳ if AlreadyAttached: returns 409 (UI shows toast)
   │     ├── live.attach.write() { user_at = now(); token = Some(...) }
   │     ├── bus.publish_bash_job(BashJobEvent::AttachStarted { … })
   │     │     ↳ ai_pipeline::translate → AiSignal "BashJob.AttachStarted"
   │     │     ↳ BackgroundJobSignalSource writes attach episode (kind=bash_job_attach, importance 0.4)
   │     ├── tail = read last 4 KB of ring file, base64
   │     └── return AttachHandle { ws_url, rows, cols, tail_b64 }
   │
   ▼
Frontend: open WebSocket to ws_url; xterm.js writes tail; bind input/output
   │
   ▼
WS handler (axum) verifies token via repo.find_by_attach_token; opens
PtyAttachBridge::run(socket)
   ├── set live.attach.write().ws_tx = Some(out_tx)
   ├── Out loop: out_rx.recv() → ws.send(Binary)   (PTY output → user)
   └── In loop:  ws.recv() → write_stdin / resize   (user input → PTY)

Meanwhile, on the LLM side:
   Next iteration's BackgroundJobsInjector.collect() detects attached_user_at →
   renders the cooperative-handoff <system-reminder>. LLM reads, defers stdin.

When the user closes the panel:
   ├── Frontend: ws.close() + invoke("coding_task_detach")
   ├── PtyAttachBridge::run loop exits; calls supervisor.detach
   ├── JobSupervisor::detach
   │     ├── live.attach.write() { user_at = None; token = None; ws_tx = None }
   │     ├── repo.clear_attached(id)
   │     ├── bus.publish_bash_job(BashJobEvent::AttachEnded { duration_ms })
   │     │     ↳ BackgroundJobSignalSource writes second episode
   │     └── Ok(())
   └── Next-iteration injector body drops the handoff section. LLM resumes normal stdin.
```

### 6.5 Restart recovery

`reconcile_on_startup` (2.3a) is extended with two changes:

1. Any row with `attached_user_at = Some(_)` at startup has its attach state cleared (`UPDATE coding_background_jobs SET attached_user_at = NULL, attach_token = NULL`). The WS bridge died with the previous Tauri process; the attach is stale.
2. Orphan PTY rows go through the same `Lost` classification as Process orphans. If the orphan also had attach state, a second `bash_job_attach` episode with `kind="bash_job_attach_lost"`, importance 0.5, is written alongside the standard `bash_job` Lost episode.

Lost rows are still excluded from `find_prior_by_command_key` (2.3b) — they have no reliable final output to diff against.

### 6.6 Subagent inheritance

Identical to 2.3a/2.3b. `JobSupervisorHandle` doesn't differentiate by backend. A subagent that calls `bash(tty=true, run_in_background=true)` gets a PTY job tagged with the subagent's `agent_id`. The injector's `agent_chain` walk surfaces both the active-job listing and any attach state to the parent's prompt boundary, so parents see the cooperative reminder for jobs spawned by descendants. Episodic memories carry `actor_id = agent_id`. Frontend attach is rooted at the thread, so a user can attach to a subagent-spawned job from `JobsPanel` regardless of which agent in the chain spawned it.

### 6.7 Thread cleanup

`reap_session` already kills processes via pgrp, finalizes rows, and the SQLite cascade clears the table. PTY jobs follow the same path with one defensive addition: `reap_session` calls `JobSupervisor::detach` on each job before `kill_process_group`, so any live attach websocket gets a clean close frame instead of a TCP RST. The bridge tolerates RSTs cleanly regardless — `detach()` is idempotent.

---

## 7. Approval & Concurrency

**Approval surface (no new approval classes):**

| Tool | Class | Scope | Notes |
|---|---|---|---|
| `bash` (with `tty=true`) | `Destructive` | `command` | Same gate as non-PTY bash; no extra approval purely for `tty=true` |
| `coding_task_stdin` | `Sensitive` | `command` | Each unique JSON-encoded args blob is its own grant; session-grants apply |
| `coding_task_resize` | `Safe` | — | No prompt; auto-allow |
| `coding_task_attach` (Tauri) | n/a (user action) | — | Not LLM-facing |
| `coding_task_detach` (Tauri) | n/a | — | Not LLM-facing |

**Concurrency:**

- The 2.3a cap of 6 active jobs per `(session_id, agent_chain)` is unchanged. PTY jobs count the same as Process jobs.
- One live attach per job — `BashJobRepo::mark_attached` is atomic (`UPDATE … WHERE attached_user_at IS NULL` and checks rowcount). Second concurrent attempt receives `AttachError::AlreadyAttached`; UI surfaces as a toast.
- `JobSupervisor`'s `DashMap<JobId, LiveJob>` continues as the single source of truth for live state.

**Sandbox (macOS seatbelt):**

The current `klynt-sandbox` profile does not allow the dynamic PTY slave device. Add two rules:

```scheme
(allow file-read* file-write* (literal "/dev/ptmx"))
(allow file-read* file-write* (regex #"^/dev/ttys[0-9]+$"))
```

Minimum-privilege: grants the bash child process the ability to open its own PTY pair but no other tty (a sandboxed process cannot, for example, write into the user's terminal sessions outside the agent). Verified in CI by an integration test that spawns a sandboxed `tty=true` bash, writes `"echo hello\n"`, and asserts the output contains `hello`. Without the rules, the test fails with `EPERM` on the slave open — that's the regression net.

Linux has no sandbox today; PTY works without configuration.

---

## 8. Subagent Inheritance

Unchanged from 2.3a/2.3b mechanics. Detail in §6.6: subagents spawn PTY jobs tagged with their own `agent_id`; the injector's `agent_chain` walk surfaces both the active-job listing and any attach state to the parent's prompt boundary. Episodic memories carry `actor_id = agent_id`. Frontend attach is rooted at the thread, not the agent.

---

## 9. Recovery & Restart

The 2.3a/2.3b recovery flow gains two changes (§6.5):

1. `attached_user_at` and `attach_token` are cleared for any row at startup. The WS bridge died with the previous process; the attach state is stale.
2. Orphan PTY rows that had attach state get a second `bash_job_attach_lost` episode (importance 0.5) written alongside the standard Lost episode. The cognitive layer can later answer "did we lose anything mid-attach last week?"

Lost PTY rows behave the same as Lost Process rows for diff lookup (excluded from `find_prior_by_command_key`).

---

## 10. Error Handling

The principle from 2.3a/2.3b carries: **errors surface as tool results, HTTP responses, or logs — never as Rust panics.**

| Failure | Symptom | Response |
|---|---|---|
| `tty=true` without `run_in_background=true` | LLM misuse | Tool result: `Err("tty=true requires run_in_background=true")` |
| `tty_rows`/`tty_cols` out of range | LLM passes 0 or 9999 | Clamp to `[4,200]`/`[20,400]`; result includes warning |
| `coding_task_stdin` on non-PTY job | LLM forgets `tty=true` | `Err("job has no PTY; spawn with tty=true to enable stdin")` |
| `coding_task_stdin` on terminated job | Race with completion | `Err("job is in state Failed; stdin ignored")` |
| `coding_task_stdin` encoding decode failure | Bad base64 | `Err("invalid base64 payload: …")` |
| `coding_task_attach` on non-PTY job | UI bug or stale row | 400 from Tauri command; UI toast `"job has no PTY"` |
| `coding_task_attach` already attached | Two windows / stale state | 409; UI toast `"another window owns this attach"` |
| WS connect with bad/missing token | Stale URL or attack | `401 Unauthorized` from axum handler; xterm.js shows `"[attach failed: unauthorized]"` |
| WS disconnects mid-input | Network blip / panel close | `PtyAttachBridge::run` exits inbound loop; `detach()` runs idempotently; reader continues writing to ring (panel-less mode) |
| PTY writer write fails (EPIPE) | Child exited mid-stdin | `JobError::ChildGone`; tool result: `"job is in state Failed; stdin ignored"` |
| `vte` parser hits unterminated escape | Pathological output | `strip_ansi` caps input at 64 KB tail; larger input truncates from head |
| portable-pty backend init failure | macOS sandbox missing rules | `JobError::Pty(format!("openpty: {e}"))`; tool result reflects |
| Subagent's `JobSupervisorHandle` is `None` | Misconfig | `JobError::Disabled`; mirrors 2.3a behaviour |
| `mark_attached` race between two attach calls | Split-brain UI | Atomic UPDATE; loser receives `AttachError::AlreadyAttached`; detach + retry resolves |

---

## 11. Testing Strategy

### 11.1 Unit tests (inline `#[cfg(test)] mod tests`)

| File | Coverage |
|---|---|
| `klynt-pty/src/lib.rs` | `spawn_with_pty` returns Pty handle; `try_clone_reader` works; resize updates kernel size (TIOCGWINSZ readback); kill via pgrp on Unix; tolerates already-exited children |
| `feature-coding-bash/src/spawner.rs` | `build_pty_command` sets `TERM=xterm-256color`; env passthrough; cwd respected |
| `feature-coding-bash/src/gate.rs` | `strip_ansi` removes CSI / OSC / DCS / RIS; preserves printable + `\n`/`\t`; cap at 64 KB; cargo coloured output classifies as TestFailure correctly |
| `feature-coding-bash/src/intelligence/...` (extended) | command_key for tty jobs; diff handles tty transitions cleanly |
| `feature-coding-bash/src/attach/token.rs` | 22-char output; high entropy; constant-time compare |
| `feature-coding-bash/src/attach/bridge.rs` | duplex-stream test: bytes in → write_stdin called; bytes out → ws frames sent; close → detach called; resize JSON dispatched correctly |
| `feature-coding-bash/src/injector.rs` | renders cooperative section iff any active job has `attached_user_at`; suppresses when empty; respects `agent_chain` |
| `storage/src/repos/coding_background_jobs.rs` | `mark_attached` atomic + AlreadyAttached variant; `find_by_attach_token` returns row; `clear_attached` idempotent; PTY-row CHECK constraints enforce paired NULLs |

Total: ~35 test cases, all sub-second, no I/O beyond `connect_in_memory()`.

### 11.2 Integration tests (`crates/feature-coding-bash/tests/`)

| File | Scenario |
|---|---|
| `interactive_pty_echo.rs` | Spawn `bash -c 'read x; echo $x'` with tty=true; `coding_task_stdin("hello\n")`; assert ring contains echoed `hello` |
| `interactive_resize_sigwinch.rs` | Spawn `bash -c 'while true; do echo ${LINES}x${COLUMNS}; sleep 0.5; done'`; resize to 30×120; tail of ring contains `30x120` |
| `interactive_ansi_in_gate.rs` | Spawn cargo-test fixture with `--color=always`; failure classifies as TestFailure; `failed_test_names` extracted correctly |
| `interactive_attach_handoff.rs` | Spawn tty=true; attach via in-process axum bridge; LLM iteration triggers injector; captured prompt contains cooperative reminder |
| `interactive_attach_episode.rs` | Attach + detach; `episodic_memories` rows for `bash_job_attach` (start + end), importance 0.4, duration_ms in metadata |
| `interactive_attach_already_attached.rs` | Two parallel `coding_task_attach` calls; one succeeds, second returns `AlreadyAttached` |
| `interactive_attach_ws_unauthorized.rs` | Connect with bad token; receives 401; supervisor state unchanged |
| `interactive_lost_on_restart.rs` | Insert tty=1 row with `attached_user_at`, fake `.log` file; call `reconcile_on_startup`; row goes Lost; attach cleared; two episodes written |
| `interactive_cancel_pty.rs` | Spawn tty=true `bash -c 'sleep 60'`; `coding_task_stop`; pgrp killed within 2s; row Cancelled |
| `interactive_subagent_pty.rs` | Subagent spawns tty=true; episode `actor_id` matches subagent's agent_id |
| `interactive_stdin_during_attach.rs` | Spawn + attach + LLM calls `coding_task_stdin`; both writes interleave on master; verify both observed in ring |

All use `StoragePool::connect_in_memory()` + an in-process `axum::Router::into_service()` bound to `tokio::io::duplex()` for WS testing. Real local PTYs (`/dev/ptmx`) on the test platform.

### 11.3 Frontend tests

| File | Coverage |
|---|---|
| `AttachTerminal.test.tsx` | Mount → `coding_task_attach` invoked; tail written to xterm.js; binary WS message renders; user keystroke sent as binary; resize sends JSON control frame; unmount → ws.close + `coding_task_detach` |
| `JobsPanel.test.tsx` (extended) | Attach button visible only on tty=1 rows; click opens AttachTerminal panel |

`xterm.js` is mocked at the module level via `vi.mock("xterm", ...)`; byte-level rendering is verified by Rust-side integration tests.

### 11.4 Manual smoke checklist

Run before merge:

1. Coding thread; `bash command="bash -c 'echo -n Username:; read u; echo got $u'" run_in_background=true tty=true description="auth probe"`. Inspect `coding_task_output` — see `Username:`. Call `coding_task_stdin("alice\n")`. Inspect output — see `got alice`.
2. Open `JobsPanel`; click `Attach` on the same job. Type `whoami` + Enter; see your username. Close panel; verify next LLM iteration's reminders no longer mention attach.
3. Trigger LLM stdin while attached; observe both writes interleave in xterm.js. Verify cooperative reminder appears in next iteration.
4. `bash command="cargo nextest run -p feature-coding-bash --color=always" run_in_background=true tty=true`; let it fail. Completion `<system-reminder>` shows `failure_kind=TestFailure` with `failed_test_names` correctly extracted (ANSI was stripped pre-classification).
5. Force-kill Tauri while attached. Restart. Inspect `episodic_memories` — see `bash_job_attach_lost` episode + `bash_job` Lost episode for the same job.
6. macOS: confirm the new sandbox rules pass. Without them, the integration tests fail with EPERM.

### 11.5 Migration / rollout

Pre-release per `CLAUDE.md`. `FeatureMigration::version` 2 → 3; SQL drops + recreates `coding_background_jobs` with five new columns + the new index. No backfill script. Existing dev databases lose bash-job history on first run — acceptable; documented in PR description.

---

## 12. Future Phases

After 2.3c, all 9 game-changer pillars are met. Opportunistic items beyond:

- **Windows ConPTY support.** `portable-pty` already abstracts ConPTY; only needed work is sandbox parity (windows-fundamentals doesn't have a seatbelt analogue today; rely on AppContainer if/when relevant) + CI lane. Trigger: a Windows-using contributor.
- **Asciinema-style recording.** The ring file is already a near-perfect terminal log; a `coding_task_export_cast(task_id) -> String` tool could emit `.cast` format for replay-in-browser. Trigger: a user requests "let me share the terminal session of that flow."
- **Multiple concurrent attach (read-only observers).** Single writer + N read-only WS observers. Useful for screen-share-style debugging. Trigger: pair-coding workflows.
- **Remote PTY over SSH.** Replace `portable-pty::native_pty_system()` with an SSH-tunnelled equivalent. Adjacent to `kaos` design space in kimi-cli. Trigger: explicit "remote agent" scope.
- **Smart input replay.** Episodic memories of `bash_job_attach` carry the input transcript; a future tool could propose `coding_task_stdin` payloads matching past similar prompts. Trigger: user reports the LLM keeps guessing wrong on auth flows.

---

## 13. Game-Changer Scorecard

| Pillar | 2.3a | 2.3b | **2.3c** |
|---|---|---|---|
| Never forgets (per-turn injector) | ✅ | ✅ | ✅ |
| Push-on-completion (no polling) | ✅ | ✅ | ✅ |
| Structured failure extraction | ✅ | ✅ | ✅ (ANSI-aware) |
| Tauri-restart recovery (Lost) | ✅ | ✅ | ✅ (incl. attach cleanup) |
| Subagent inheritance | ✅ | ✅ | ✅ |
| Cross-run output diffing | ❌ | ✅ | ✅ |
| Plan-mode auto-affordance | ❌ | ✅ | ✅ |
| Episodic memory of past failures | ❌ | ✅ | ✅ (incl. attach episodes) |
| Interactive (PTY/stdin/resize/attach) | ❌ | ❌ | **✅** |

**9 of 9 pillars after 2.3c.** Loop closed.

---

## Appendix A — Verification commands

```bash
# Tests
cargo nextest run -p klynt-pty
cargo nextest run -p feature-coding-bash
cargo nextest run -p storage -E 'test(coding_background_jobs)'
cargo nextest run -E 'test(interactive_)'

# Lint + format
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check

# Doctest
cargo test --workspace --doc

# Frontend
cd desktop-ui && bun run typecheck && bun run test -- AttachTerminal JobsPanel

# Manual
cargo tauri dev
# Then run the §11.4 smoke checklist
```

---

## Appendix B — Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| `portable-pty` version drift breaks public API | Low | Pin to `0.8`; integration tests catch silently-broken behaviour |
| `vte` parser memory blow-up on pathological output | Low | Cap input to 64 KB tail per gate call; matches existing gate budget |
| macOS sandbox rejects PTY open | Medium pre-release; Low after first ship | Spec the rules in §7; integration test fails loudly without them |
| WebSocket token leaked via clipboard | Very low | 16-byte random; single-attach; constant-time compare; rotated per attach |
| LLM ignores cooperative reminder and stdins during attach | Medium | Accept — cooperative protocol; user sees both writes in xterm.js; can `coding_task_stop` if it gets bad |
| xterm.js bundle size grows desktop-ui | Low | Code-split via dynamic `import("xterm")` in `AttachTerminal`; lazy-loaded only when user clicks Attach |
| Two attach windows fight over the same job | Low | DB-level atomic; second attempt rejected with 409 |
| Reader task forwards bytes to a closed `ws_tx` | Low | `UnboundedSender::send` returns `Err` on closed receiver; reader logs and clears `ws_tx` |
| `attach_token` row leftover after crash gives stale auth window | Low | `reconcile_on_startup` clears `attached_user_at` + `attach_token` for any row at startup; tokens are useless without a live bridge |
| `coding_task_stdin` payload blob is enormous (whole file paste) | Medium | Approval prompt makes it visible; LLM rate-limited by approval class |
| `vte` doesn't strip a new escape sequence pytest emits | Low | Gate falls back to OtherFailure if extraction fails — never panics |
| Backfill needed post-release | Pre-release N/A | Documented as known limitation; first post-release migration must add the five columns + a backfill that infers `tty` from existing rows (default 0) |

---

## Appendix C — Out-of-scope confirmations

For clarity, these are explicitly NOT in 2.3c:

- New mirror tables — episodic-only, reuses 2.3b's `BackgroundJobSignalSource`
- Windows native build — Mac/Linux only
- Multi-user / observer attach
- Remote / SSH PTY
- `.cast` recording export
- Theme matching system theme in xterm.js — uses `ds-tokens.css` palette; no live theme switching
- Replacing the on-disk ring file format
- Approval-class changes for the existing `bash` tool — `tty=true` does not elevate the class
- Frontend changes outside `JobsPanel` + `AttachTerminal` — no sidebar / settings additions
- Linux seccomp profile parity — Linux has no agent-side sandbox today; no asymmetric work introduced by this phase
