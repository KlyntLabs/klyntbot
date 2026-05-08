# Coding Background Bash Design (Phase 2.3)

**Date:** 2026-05-08
**Status:** Spec — ready for implementation plan
**Phase:** 2.3 of the long-running-task roadmap (`docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md`)
**Companion docs:**
- `docs/superpowers/specs/2026-05-08-coding-plan-mode-design.md` (Phase 2.2 — `DynamicInjector` scaffold reused here)
- `docs/superpowers/specs/2026-05-07-coding-todowrite-design.md` (Phase 2.1 — feature-package + injector pattern reused here)
- `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md` (`CodingApprovalPolicy` enum, extended trivially)

---

## 1. Goal & Scope

Background bash unblocks **long-turn coding tests**: the LLM starts a long command (e.g. `cargo nextest run --workspace`), continues working on edits and reasoning, polls the job's output as needed, and reacts to the structured failure result when the job completes. The job survives across LLM iterations, across user-cancelled turns, and across Tauri app restarts.

This document specifies **Phase 2.3a — Cognitive Background**, the load-bearing release that makes the feature shippable. Phases 2.3b and 2.3c are sketched at the end as forward-pointers.

**2.3a explicit scope:**

- `bash` tool gains `run_in_background: bool`, `description: string?`, `silent_completion: bool`
- Three new tools: `coding_task_list`, `coding_task_output`, `coding_task_stop`
- `JobSupervisor` (in-memory) + SQLite-persisted spec rows + on-disk ring-buffered output
- `BackgroundJobsInjector` per-turn reminder via the existing `InjectorRegistry`
- Push-on-completion: completion of a job immediately enqueues a rich `ContextUpdate`
- `GateClassifier` with 8 `FailureKind` variants and **structured extraction** (file/line/col, test name, port number)
- Tauri-restart recovery (orphans → `Lost`, partial output preserved)
- `JobsPanel` sidebar UI with last-3-lines live preview
- Concurrency cap = 6 per `(session, agent)`; subagents inherit
- Process-group + SIGTERM(2 s)→SIGKILL cancellation

**2.3a explicit non-goals:**

- PTY support (`tty: bool`) — deferred to 2.3c
- Interactive stdin (`coding_task_stdin`) — deferred to 2.3c
- Plan-mode auto-affordances — deferred to 2.3b
- Output diffing across runs — deferred to 2.3b
- Episodic memory of past failures (`BackgroundJobSignalSource`) — deferred to 2.3b
- Job dependency graphs (`parent_job_id`) — deferred indefinitely; the LLM sequences calls
- Job priority — deferred indefinitely; cap is enforced at spawn so no eviction is needed
- Estimated duration (`estimated_duration_ms`) — deferred indefinitely; no rigorous use found

---

## 2. Architecture Overview

Two new crates plus extensions to four existing ones, following Klynt's 9-layer dependency rule:

```
L4 (NEW)  feature-coding-bash/
              src/lib.rs                  # FeaturePackage impl
              src/supervisor.rs           # JobSupervisor (in-memory live processes)
              src/spawner.rs              # spawn_command (sandbox + pgrp + GIT_EDITOR)
              src/ring.rs                 # RingFile (append, bisect, cursor read)
              src/gate.rs                 # GateClassifier (regex + structured extraction)
              src/injector.rs             # BackgroundJobsInjector : DynamicInjector
              src/render.rs               # XML rendering for the system-reminder body
              src/tools/bash.rs           # MOVED from klynt-core (extended schema)
              src/tools/coding_task_list.rs
              src/tools/coding_task_output.rs
              src/tools/coding_task_stop.rs

L4 (NEW)  klynt-pty/                      # placeholder crate; non-PTY only in 2.3a
              src/lib.rs                  # ChildHandle::Process variant only

L2  storage/src/repos/coding_background_jobs.rs   # BashJobRow + BashJobRepo
        # NO numbered SQL file — feature crates own their migrations via FeatureMigration

L4  feature-coding-bash/src/migrations.rs         # FeatureMigration { feature_name: "feature_coding_bash", version: 1, sql: "CREATE TABLE …" }
L4  feature-coding-bash/src/view.rs               # BashJobView (specta::Type) — lives in feature crate, not desktop-shared

L1  bus/src/domain_events.rs              # JobStarted/JobCompleted/JobFailed/JobLost variants
L1  bus/src/context_updates.rs            # ContextUpdateReason::CodingJobsChanged

L3  agent/src/subagent.rs                 # SubagentManager carries job_supervisor: Arc<dyn JobSupervisorHandle>
L4  app-core/src/handlers/coding_jobs.rs  # Tauri handler shells (delegate to JobSupervisor)
L4  app-core/src/handlers/coding_threads.rs   # extend on-thread-delete to call reap_session
L4  app-core/src/init/ai_pipeline.rs      # Wire JobSupervisor into RoutingContext + InjectorRegistry

L7  desktop/src/commands/coding_jobs.rs   # Tauri command shells (klynt_command)
L7  desktop/src/specta_builder.rs         # register 4 new commands

UI  desktop-ui/src/features/coding/
        state/jobsStore.ts                # Zustand store
        hooks/useThreadJobs.ts            # subscribe to coding:job_event
        components/JobsPanel.tsx
        components/JobsPanel.test.tsx
        components/JobBadge.tsx           # spinner + count
```

**Key design choices:**

- **Feature package, not just a tool family.** `feature-coding-bash` owns a tool family + SQLite migration + `DynamicInjector` + supervisor + gate classifier + event publication. Mirrors `feature-coding-todo` (Phase 2.1) and `feature-coding-plan` (Phase 2.2).
- **Two-tier durability.** In-memory `JobSupervisor` for live process control; SQLite for spec rows; on-disk files for output. The split mirrors DeepSeek-TUI's `ShellManager` + `TaskManager` partition, adapted to Klynt's storage stack.
- **Dependency-inverted handle.** `JobSupervisorHandle` trait lives in `tools-core`; the concrete `feature_coding_bash::JobSupervisor` implements it and is injected via `RoutingContext`. Same pattern as `SpawnHandler` and `CronHandler`.
- **`klynt-pty` exists in 2.3a but only exposes the `Process` branch.** The PTY branch is added in 2.3c without re-touching `feature-coding-bash`.

---

## 3. Tool Surface

### 3.1 Extended `bash` tool

```rust
#[derive(ToolParams)]
pub struct BashArgs {
    pub command: String,
    pub timeout_ms: Option<u64>,
    pub cwd: Option<String>,

    /// When true, returns immediately with a job_id; output is read via `coding_task_output`.
    pub run_in_background: Option<bool>,

    /// Required when run_in_background=true. A short human-readable label.
    /// Shown in the JobsPanel and the per-turn injector reminder.
    pub description: Option<String>,

    /// When true, skip the auto-injected completion notification on this job.
    /// Default false; suppress only for chatty long-running probes (e.g. dev server).
    pub silent_completion: Option<bool>,
}
```

Validation: when `run_in_background=true`, `description` is required and must be 1–120 chars.

Foreground behavior (`run_in_background` absent or false) is unchanged from today.

Background behavior: returns a tool result of the shape

```text
Started background job bash-aB3kF7c2qR.
Description: cargo nextest run --workspace
Inspect output:    coding_task_output("bash-aB3kF7c2qR")
Cancel:            coding_task_stop("bash-aB3kF7c2qR")

This job will auto-notify on completion. The active job list is reminded each turn.
```

The tool returns within ~100 ms regardless of how long the underlying command runs.

### 3.2 `coding_task_list`

```rust
#[derive(ToolParams)]
pub struct CodingTaskListArgs {
    pub active_only: Option<bool>,    // default true
}
```

Returns up to 20 jobs scoped to the calling `(session_id, agent_chain)` — i.e. anything spawned by any agent in the same chain — ordered by `started_at DESC`. See §10.

Tool result format (one block per job):

```text
bash-aB3kF7c2qR  Running   4m 12s    3.2 MB    cargo nextest run --workspace
                 last polled 47s ago, last seen offset 2_048_000

bash-9Qx2pLm8wT  Completed 1h 8m     412 KB   bun run dev
                 exit_code=0  gate=Passed
```

### 3.3 `coding_task_output`

```rust
#[derive(ToolParams)]
pub struct CodingTaskOutputArgs {
    pub task_id: String,
    pub since_offset: Option<u64>,    // default 0
    pub block: Option<bool>,          // default false
    pub timeout_ms: Option<u64>,      // default 30_000; ignored if block=false
}
```

Returns up to 50 KB of new output bytes since `since_offset`, plus structured metadata. When `block=true` and there's no new output, waits up to `timeout_ms` for new bytes (via `Notify` on the `RingFile`).

Tool result body: the literal new bytes (cleaned of control chars except `\n\t\r`, then byte-truncated to 50 KB if needed via the existing `klynt_truncation` policy used by tool results elsewhere).

Tool result metadata (returned as a JSON tail block the LLM can read):

```json
{
  "task_id": "bash-aB3kF7c2qR",
  "status": "Running",
  "new_offset": 2_080_768,
  "total_bytes_emitted": 2_080_768,
  "bisect_generation": 0,
  "bisect_occurred_since": false,
  "bytes_returned": 32_768,
  "exit_code": null,
  "gate_kind": null,
  "failure_extracted": null
}
```

When `bisect_occurred_since=true` (the consumer's `since_offset` was below the current `bisect_low_water`), the body returns the head-segment bytes plus a `[--- bisect: N bytes truncated from the middle ---]` marker plus the tail-segment from the prior `since_offset` mapped to the new layout. The next valid `since_offset` is the returned `new_offset`.

### 3.4 `coding_task_stop`

```rust
#[derive(ToolParams)]
pub struct CodingTaskStopArgs {
    pub task_id: String,
    pub reason: Option<String>,    // default "Stopped by LLM"
}
```

`approval_class = "sensitive"` — explicitly *not* destructive. Killing a runaway process is de-escalation; gating it behind approval would invert the safety incentive.

Tool result body:

```text
Stopped bash-aB3kF7c2qR (reason: stale build).
Status: Cancelled  exit_code=-15 (SIGTERM)  ran 4m 38s, emitted 3.2 MB.
gate=Cancelled  final summary at coding_task_output("bash-aB3kF7c2qR")
```

---

## 4. Data Model

### 4.1 SQLite schema

`crates/storage/migrations/NNN_coding_background_jobs.sql`:

```sql
CREATE TABLE coding_background_jobs (
    id                    TEXT PRIMARY KEY,
    session_id            TEXT NOT NULL,
    agent_id              TEXT NOT NULL,
    description           TEXT NOT NULL,
    command               TEXT NOT NULL,
    cwd                   TEXT NOT NULL,
    timeout_ms            INTEGER NOT NULL,
    silent_completion     INTEGER NOT NULL DEFAULT 0,
    status                TEXT NOT NULL,
    exit_code             INTEGER,
    failure_kind          TEXT,
    failure_detail        TEXT,
    failure_extracted     TEXT,                                -- JSON; see §8
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
    FOREIGN KEY (session_id) REFERENCES coding_sessions(id) ON DELETE CASCADE
);

CREATE INDEX idx_cbj_session_status ON coding_background_jobs(session_id, status);
CREATE INDEX idx_cbj_active        ON coding_background_jobs(status) WHERE status IN ('Starting','Running');
```

`status` lifecycle: `Starting` → `Running` → one of `Completed`/`Failed`/`Cancelled`/`Lost`. Terminal states are immutable.

### 4.2 File layout under the data dir

```
~/.klyntbot/                              # or ~/.klyntbot-dev when KLYNTBOT_HOME is set
├── data.db
├── sessions/
├── workspace/
├── lance/
└── jobs/                                 # NEW
    ├── bash-aB3kF7c2qR.log               # running, ≤4 MB ring with bisect overflow
    ├── bash-aB3kF7c2qR.log.tmp           # transient during bisect rewrite (atomic rename target)
    ├── bash-9Qx2pLm8wT.final             # completed, ≤256 KB head+tail summary
    └── …
```

Naming: `{id}.log` while running, `{id}.final` after completion. Bisect rewrites use `{id}.log.tmp` + atomic `rename(2)` so a Tauri crash mid-bisect leaves the prior log intact. Background GC sweep at supervisor init removes orphan `.log`/`.final` files (any without a matching SQLite row).

### 4.3 Rust types

```rust
pub struct JobSpec {
    pub session_id:        String,
    pub agent_id:          String,
    pub description:       String,
    pub command:           String,
    pub cwd:               PathBuf,
    pub timeout_ms:        u64,                         // 1..=86_400_000
    pub silent_completion: bool,
}

pub struct JobView {
    pub id:                  JobId,
    pub session_id:          String,
    pub agent_id:            String,
    pub description:         String,
    pub command:             String,
    pub cwd:                 PathBuf,
    pub status:              JobStatus,
    pub started_at:          jiff::Timestamp,
    pub finished_at:         Option<jiff::Timestamp>,
    pub exit_code:           Option<i32>,
    pub gate_result:         Option<GateResult>,
    pub failure_extracted:   Option<serde_json::Value>,
    pub total_bytes_emitted: u64,
    pub bisect_generation:   u64,
    pub last_polled_at:      Option<jiff::Timestamp>,
    pub last_seen_offset:    u64,
}

pub struct JobId(String);                                // "bash-{10 base32 chars}"

pub enum JobStatus { Starting, Running, Completed, Failed, Cancelled, Lost }
```

### 4.4 `RoutingContext` extension

`crates/tools-core/src/routing.rs`:

```rust
pub struct RoutingContext {
    // ... existing fields ...
    pub workspace_cwd:  Option<PathBuf>,                  // NEW; captured at thread spawn
    pub agent_chain:    Vec<String>,                      // NEW; root → … → self (always non-empty,
                                                          //      [0] = root_agent_id, last = current agent_id)
    pub job_supervisor: Option<Arc<dyn JobSupervisorHandle>>,   // NEW; injected in app-core init,
                                                          //      flows through SubagentManager.coding_policies-style sharing
}

// NOTE: `RoutingContext` already implements `bus::InjectorContext` (routing.rs:181-194).
// We extend the impl to also expose agent_chain via a new method on the trait if needed.

#[async_trait]
pub trait JobSupervisorHandle: Send + Sync {
    async fn spawn(&self, spec: JobSpec) -> Result<JobView, JobError>;
    async fn output_delta(&self, id: &JobId, since: u64, block: bool, timeout_ms: u64)
        -> Result<RingRead, JobError>;
    async fn stop(&self, id: &JobId, reason: &str) -> Result<JobView, JobError>;
    fn list(&self, session_id: &str, agent_chain: &[String], active_only: bool) -> Vec<JobView>;
}
```

The trait stays narrow — only what tools need. `list` takes the full agent chain so subagents see jobs spawned anywhere in their chain (see §10). Lifecycle methods (`reap_session`, `reconcile_on_startup`) are on the concrete struct, called by `AppCore`.

### 4.5 New event variants

`crates/coding-ingest/src/event.rs`:

```rust
pub enum EventKind {
    // ... existing 19 ...
    BackgroundJobLifecycle {
        job_id:       String,
        phase:        JobPhase,                          // Started|Stopped|Completed|Failed|Lost
        exit_code:    Option<i32>,
        failure_kind: Option<String>,
        gate_summary: Option<String>,
    },
    BackgroundJobOutputBisect {
        job_id:        String,
        bisect_gen:    u64,
        dropped_bytes: u64,
    },
}
```

Lifecycle events flow through the existing `Translator` → `MemorySink` pipeline. **No per-byte stdout chunks** are ingested — the `.log`/`.final` files are the source of truth for output.

`crates/bus/src/context_updates.rs`:

```rust
pub enum ContextUpdateReason {
    // ... existing ...
    CodingJobsChanged,
}
```

---

## 5. Components

### 5.1 `JobSupervisor`

```rust
pub struct JobSupervisor {
    jobs:             DashMap<JobId, Arc<LiveJob>>,
    repo:             CodingBackgroundJobsRepo,
    bus:              Arc<DomainEventBus>,
    update_queue:     Arc<ContextUpdateQueue>,
    data_dir:         PathBuf,
    cap_per_session:  usize,              // = 6
    sandbox:          Arc<MacOsSeatbeltRunner>,
}

impl JobSupervisor {
    pub async fn spawn(&self, spec: JobSpec) -> Result<JobView, SpawnError>;
    pub async fn output_delta(&self, id: &JobId, since: u64, block: bool, timeout_ms: u64)
        -> Result<RingRead, JobError>;
    pub async fn stop(&self, id: &JobId, reason: &str) -> Result<JobView, JobError>;
    pub fn list(&self, session_id: &str, agent_chain: &[String], active_only: bool) -> Vec<JobView>;

    pub async fn reap_session(&self, session_id: &str) -> Result<usize, JobError>;
    pub async fn reconcile_on_startup(&self) -> Result<usize, JobError>;
}
```

The `Arc<LiveJob>` pattern: `JobSupervisor::spawn` constructs the `LiveJob` and inserts it into the `DashMap`. `stop` and `output_delta` look up by id and clone the `Arc`, then operate on it without holding the map lock. Removal from the `DashMap` happens in the wait task after finalize.

### 5.2 `LiveJob`

```rust
struct LiveJob {
    id:         JobId,
    spec:       JobSpec,
    child:      Mutex<ChildHandle>,                 // ChildHandle::Process(_) only in 2.3a
    pgid:       Option<u32>,
    ring:       Arc<RingFile>,
    cancel:     CancellationToken,
    state:      AtomicU8,                            // Running=0, Stopping=1, Completed=2
    started_at: jiff::Timestamp,
}
```

`Mutex<ChildHandle>` is held only briefly during `wait()` poll cycles; concurrent stops grab it momentarily to read `pgid` then release before sending the signal.

### 5.3 `RingFile`

```rust
pub struct RingFile {
    path:               PathBuf,
    writer:             tokio::sync::Mutex<tokio::io::BufWriter<tokio::fs::File>>,
    bytes_written:      AtomicU64,                   // cumulative; never decreases
    bisect_low_water:   AtomicU64,
    bisect_generation:  AtomicU64,
    cap_bytes:          u64,                         // 4 * 1024 * 1024
    notify:             tokio::sync::Notify,
}

impl RingFile {
    pub async fn create(path: PathBuf, cap_bytes: u64) -> std::io::Result<Self>;
    pub async fn append(&self, bytes: &[u8]) -> std::io::Result<()>;
    pub async fn read_delta(&self, since_offset: u64) -> std::io::Result<RingRead>;
    pub async fn finalize(&self) -> std::io::Result<PathBuf>;   // → {id}.final ≤ 256 KB
    pub fn notify_waiters(&self);                                // for block=true polls
}

pub struct RingRead {
    pub bytes:                 Vec<u8>,
    pub new_offset:            u64,
    pub bisect_generation:     u64,
    pub bisect_occurred_since: bool,
    pub total_bytes_emitted:   u64,
}
```

**Bisect on overflow:** when `bytes_written - bisect_low_water > cap_bytes`, the writer:

1. Closes the current `BufWriter`.
2. Reads first 1.5 MB and last 2.5 MB of the file.
3. Writes them to `{path}.tmp` separated by a `\n[--- bisect: {N} bytes truncated from the middle ---]\n` marker, where `N` is the count of dropped bytes.
4. `rename({path}.tmp, {path})`.
5. Updates `bisect_low_water = bytes_written - 4 MB + marker_len`, increments `bisect_generation`.
6. Reopens the writer in append mode.

The `bisect_low_water` is the lowest cumulative-byte-offset that's still mapped to a byte in the file. `read_delta(since)` checks `since < bisect_low_water` and sets `bisect_occurred_since=true` when it triggers the head-segment + marker + tail-segment fallback path.

**Finalize:** at job completion, computes a 256 KB head+tail summary (96 KB head + marker + 160 KB tail), writes to `{id}.final`, deletes `{id}.log`. The `final_path` column on the SQLite row is set to the `.final` path; subsequent `output_delta` reads on a completed job serve from this file.

### 5.4 `BackgroundJobsInjector`

```rust
pub struct BackgroundJobsInjector {
    supervisor: Arc<JobSupervisor>,
}

#[async_trait]
impl DynamicInjector for BackgroundJobsInjector {
    fn name(&self) -> &str { "background-jobs" }

    async fn collect(&self, ctx: &InjectionContext) -> Vec<ContextUpdate> {
        let active = self.supervisor.list(&ctx.session_id, &ctx.agent_chain, true);
        if active.is_empty() { return vec![]; }
        let body = render_active_jobs_reminder(&active);
        vec![ContextUpdate {
            reason:   ContextUpdateReason::CodingJobsChanged,
            priority: ContextUpdatePriority::Standard,
            body,
            ..Default::default()
        }]
    }
}

fn render_active_jobs_reminder(jobs: &[JobView]) -> String { /* see below */ }
```

**Note:** the injector is invoked via `LiveContextRefresher::inject_pending_with_ctx` (NOT `inject_pending`) — the `_with_ctx` variant is what calls `InjectorRegistry::collect_all`. Verify the call site in `execute_loop` already uses the `_with_ctx` form (Phase 2.2 should have switched it; if not, that's a prerequisite step in the plan).

Body format (as a `<system-reminder>` block):

```xml
<system-reminder>
You have 2 background jobs running in this thread:
- bash-aB3kF7c2qR: cargo nextest run --workspace (started 4m 12s ago, 3.2 MB output, last poll 47s ago)
- bash-9Qx2pLm8wT: bun run dev (started 1h 8m ago, 412 KB output, never polled)

Inspect output with coding_task_output(task_id, since_offset).
Cancel with coding_task_stop(task_id).
Completed jobs auto-notify in this thread; you do not need to poll for completion.
</system-reminder>
```

Registered in the `InjectorRegistry` alongside `PlanModeInjector` (Phase 2.2). Drained by `LiveContextRefresher` between iterations.

### 5.5 `GateClassifier`

See §8 for taxonomy and extraction logic.

### 5.6 Tools

```rust
#[derive(Tool)]                                  // approval_class = "destructive", channel = "coding_only"
pub struct BashTool { /* extended; takes JobSupervisor handle */ }

#[derive(Tool)]                                  // approval_class = "safe"
pub struct CodingTaskListTool;

#[derive(Tool)]                                  // approval_class = "safe"
pub struct CodingTaskOutputTool;

#[derive(Tool)]                                  // approval_class = "sensitive"
pub struct CodingTaskStopTool;
```

All four are registered via `feature_coding_bash::FeaturePackage::tools()`. The `bash` tool relocates from `klynt-core` to `feature-coding-bash` to consolidate the family.

---

## 6. Data Flow

### 6.1 Spawn

```
LLM tool call: bash(command, run_in_background=true, description="…")
   │
   ▼
BashTool::execute(args, ctx)
   │
   ├── ApprovalGate::check(ApprovalClass::Destructive, command)        ← unchanged from foreground
   ├── if !args.run_in_background → existing synchronous path (unchanged)
   │
   └── else: branch to background path
          │
          ▼
       JobSupervisor::spawn(JobSpec { … })
          │
          ├── check cap → reject if 6 already active in (session_id, agent_chain)
          ├── allocate JobId + log_path
          ├── INSERT row (status="Starting")
          ├── RingFile::create(log_path, 4 MB)
          ├── spawner::spawn_command(spec, sandbox) → SpawnedJob {
          │       child: ChildHandle::Process(_),
          │       stdout: Box<dyn AsyncRead>,
          │       stderr: Box<dyn AsyncRead>,
          │       pgid: Some(pid),
          │   }
          ├── tokio::spawn drain_reader(stdout, ring.clone(), cancel.clone())
          ├── tokio::spawn drain_reader(stderr, ring.clone(), cancel.clone())
          ├── tokio::spawn wait_task(child, ring, supervisor, job_id, cancel)
          ├── DashMap.insert(job_id, Arc::new(LiveJob))
          ├── UPDATE row status="Running"
          ├── publish JobStarted on DomainEventBus
          ├── enqueue ContextUpdate(CodingJobsChanged) — non-blocking
          ├── emit Tauri event "coding:job_event" → frontend
          └── return JobView
```

### 6.2 Iteration boundary (mid-job)

```
execute_loop iteration N completes
   │
   ▼
LiveContextRefresher::collect()
   │
   ├── drain ContextUpdateQueue (reasons: existing + CodingJobsChanged)
   ├── InjectorRegistry::collect_all(ctx)
   │     ├── PlanModeInjector → maybe a system-reminder
   │     └── BackgroundJobsInjector → if any active jobs, emit one update
   │
   └── merge into next iteration's prompt as Message::ContextUpdate
```

### 6.3 Output poll

```
LLM tool call: coding_task_output(task_id, since_offset, block?)
   │
   ▼
CodingTaskOutputTool::execute(args, ctx)
   │
   ├── lookup JobView in JobSupervisor.list (by id) — preferred path
   │     OR fallback: SELECT FROM coding_background_jobs WHERE id=?
   │     OR final-file path if status=Completed/Failed/Cancelled/Lost
   │
   ├── RingFile::read_delta(since_offset)
   │     ├── if since < bisect_low_water → returns bisect_occurred_since=true
   │     │     bytes = head_segment[since..] || marker || tail_segment
   │     ├── else → seek + read to EOF (capped to 50 KB)
   │     └── if block && len(bytes)==0 && status==Running:
   │           ring.notify.notified().await with timeout
   │           re-read on wake
   │
   ├── UPDATE coding_background_jobs SET last_polled_at=now,
   │                                     last_seen_offset=max(last_seen_offset, new_offset)
   │
   └── return tool result with body bytes + JSON metadata trailer
```

### 6.4 Completion (auto-push)

```
wait_task observes child exit
   │
   ▼
finalize_job(live_job, exit_status)
   │
   ├── cancel reader tasks (so they don't hang on closed pipes)
   ├── RingFile::finalize() → {id}.final (≤256 KB)
   ├── delete {id}.log
   ├── GateClassifier::classify(head, tail, exit_code, command, was_timeout)
   │     → GateResult { kind, detail, extracted: serde_json::Value }
   ├── UPDATE row SET
   │     status, exit_code, finished_at, failure_kind, failure_detail,
   │     failure_extracted, final_path, total_bytes_emitted
   ├── DashMap.remove(job_id)                                       ← LiveJob dropped
   ├── publish JobCompleted/JobFailed/JobCancelled on DomainEventBus
   ├── if !spec.silent_completion:
   │     enqueue ContextUpdate(CodingJobsChanged, body=completion_notification)
   │       body includes: gate result + extracted struct + last 80 lines + read-pointer
   │       priority = UpdatePriority::High (so the LLM sees it on the very next iteration even under
   │       token-budget pressure)
   └── emit Tauri event "coding:job_event" → frontend updates JobsPanel
```

The completion ContextUpdate body:

```xml
<system-reminder>
Background job bash-aB3kF7c2qR completed.
Description: cargo nextest run --workspace
Status: Failed  Exit: 101  Ran: 4m 38s  Output: 3.2 MB

Failure kind: TestFailure
Detail: "test result: FAILED. 1 passed; 3 failed; 0 ignored"
Extracted:
  test_name: tests::session_persistence::reload_active_thread
  n_failed: 3
  n_passed: 1

Last 80 lines of output below.
For more, call coding_task_output("bash-aB3kF7c2qR", since_offset=…).

[--- last 80 lines verbatim ---]
</system-reminder>
```

### 6.5 Restart recovery

```
AppCore::init() → JobSupervisor::reconcile_on_startup()
   │
   ├── SELECT * FROM coding_background_jobs WHERE status IN ('Starting','Running')
   ├── for each orphan row:
   │     ├── if {id}.final exists → presume previous run finalized but did not commit row update
   │     │     parse final + reclassify gate → mark Completed/Failed appropriately
   │     ├── else if {id}.log exists → status="Lost", failure_kind=Lost,
   │     │     failure_detail="Klynt restarted while job was running",
   │     │     finished_at=now, preserve {id}.log (LLM can still read partial output)
   │     │     enqueue ContextUpdate(CodingJobsChanged, body=lost_notification)
   │     └── else → status="Lost", no log to preserve
   │
   └── return count of reconciled rows
```

Lost jobs are *not* auto-resumed. The LLM, on next thread open, sees the Lost notification in its first iteration's reminder and decides whether to relaunch. This is honest about partial knowledge — kimi-cli does the same.

### 6.6 Thread cleanup

```
on coding_thread_delete(session_id):
   │
   ├── JobSupervisor::reap_session(session_id)
   │     ├── SELECT id FROM coding_background_jobs WHERE session_id=? AND status IN ('Starting','Running')
   │     ├── for each id: kill_process_group(pgid, SIGTERM)
   │     ├── sleep 2s
   │     ├── for each still-alive: kill_process_group(pgid, SIGKILL)
   │     └── for each: finalize_job(reason=ThreadDeleted)
   │
   └── DELETE FROM coding_sessions WHERE id=?    ← cascades to coding_background_jobs
```

The `.log`/`.final` files for a deleted thread are GC'd by the orphan-file sweep at the next supervisor init.

---

## 7. Platform Details

### 7.1 Non-PTY spawn (the only path in 2.3a)

```rust
let mut cmd = tokio::process::Command::new("/bin/bash");
cmd.arg("-c").arg(&spec.command)
   .current_dir(&spec.cwd)
   .env("GIT_EDITOR", "true")               // prevent interactive git editor blocks
   .env("PAGER", "cat")                      // prevent paging
   .env("TERM", "dumb")                      // strip ANSI in non-PTY path
   .stdout(Stdio::piped())
   .stderr(Stdio::piped())
   .stdin(Stdio::null());                    // EOF on read; prevents `ssh`/REPL hangs

#[cfg(unix)]
unsafe {
    cmd.pre_exec(|| {
        libc::setpgid(0, 0);                                    // own process group
        #[cfg(target_os = "linux")]
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);     // best-effort orphan cleanup
        Ok(())
    });
}

// On macOS, MacOsSeatbeltRunner wraps with `/usr/bin/sandbox-exec -p <policy>` envelope
// via a new helper `MacOsSeatbeltRunner::build_sandboxed_command(command) -> std::process::Command`
// that returns a fully-configured Command with the sandbox wrapping intact.

let child = cmd.spawn()?;
let pgid = unix_only::getpgid(child.id().expect("pid set"))?;
```

Three env vars and `Stdio::null()` are the difference between "background bash works" and "background bash hangs forever on `ssh`, `git commit`, `npm config`, etc." Each one is a one-line fix to a class of hang.

### 7.2 Sandbox interaction

`MacOsSeatbeltRunner::run_command` (`crates/klynt-sandbox/src/seatbelt.rs:51-109`) builds a `sandbox-exec`-wrapped Command, awaits completion, and returns a single `CommandOutput { stdout, exit_code }` where stdout and stderr are **merged**. Background bash needs separate streams plus a kept-alive child handle.

Refactor (additive, no breaking change to foreground):

```rust
impl MacOsSeatbeltRunner {
    // NEW — reusable command builder. Lines 59-68 of run_command extracted.
    pub fn build_sandboxed_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
    ) -> Result<tokio::process::Command, SandboxError>;

    // existing run_command unchanged — still merges stdout+stderr, still awaits exit.
}
```

`feature-coding-bash::spawner` calls `build_sandboxed_command` to get a Command, then:
1. Sets `pre_exec` for `setpgid(0, 0)` (Linux: also `PR_SET_PDEATHSIG`)
2. Adds `GIT_EDITOR=true`, `PAGER=cat`, `TERM=dumb` env vars
3. Sets `Stdio::null()` for stdin
4. Spawns and **keeps the `tokio::process::Child` handle**
5. Reads stdout and stderr separately into the `RingFile` (two reader tasks)

The `CommandOutput` type is **not modified**. Foreground bash continues to use the merged-stream path. Background bash bypasses `CommandOutput` entirely and uses the new `BackgroundCommandHandle { child, stdout: ChildStdout, stderr: ChildStderr, pgid: Option<u32> }` returned by `feature-coding-bash::spawner::spawn_background_command`.

### 7.3 Cancel propagation

| Trigger | Source | Behavior |
|---|---|---|
| LLM `coding_task_stop(id)` | tool call | `JobSupervisor::stop` → SIGTERM to pgid, 2 s grace, SIGKILL if alive, then `finalize_job(reason=Cancelled)` |
| User cancels turn | `coding_turn_interrupt` | LLM iteration aborts; **background jobs are NOT killed** |
| Thread deletion | `coding_thread_delete` | `JobSupervisor::reap_session` → SIGTERM all, 2 s grace, SIGKILL all, finalize each |

Process-group kill on Unix:

```rust
#[cfg(unix)]
fn kill_process_group(pgid: u32, signal: libc::c_int) -> std::io::Result<()> {
    unsafe {
        if libc::kill(-(pgid as i32), signal) < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
```

The negative `pgid` argument signals the entire process group — `make` and its `cc` grandchildren die together. opencode's `pgrep -P`-only approach is the buggy alternative we're explicitly avoiding.

### 7.4 Reader task

```rust
async fn drain_reader<R: AsyncRead + Unpin>(
    mut reader: R,
    ring: Arc<RingFile>,
    cancel: CancellationToken,
) -> std::io::Result<()> {
    let mut buf = [0u8; 8192];                          // 8 KB chunks
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Ok(()),
            n = reader.read(&mut buf) => match n? {
                0 => return Ok(()),                     // EOF
                n => {
                    ring.append(&buf[..n]).await?;
                    ring.notify_waiters();
                }
            }
        }
    }
}
```

`8 KB` chosen empirically: large enough that `notify_waiters` doesn't fire thousands of times per build; small enough that fast-changing state is observable to `block=true` polls. Cancellation here means "stop reading", not "kill child" — those are separate.

---

## 8. Gate Classification

### 8.1 Taxonomy

```rust
pub enum FailureKind {
    CompileError,           // language compiler reports a hard error
    TestFailure,            // test runner reports failed tests
    LintFailure,            // linter aborts due to error-level findings
    NetworkBindFailure,     // EADDRINUSE / address already in use
    Timeout,                // wall-clock > timeout_ms
    Cancelled,              // explicit stop or thread deletion
    Lost,                   // reconciled-on-startup orphan
    Other(String),          // exit_code != 0 and no above signature matched
}

pub enum GateResult {
    Passed,
    Failed { kind: FailureKind, detail: String, extracted: serde_json::Value },
}
```

### 8.2 Detection order

Most-specific first. Each detector is a small struct implementing a `Detector` trait; they run in sequence and the first match wins.

1. `CompileError` — Rust `error\[E\d{4}\]:`, TS `error TS\d{4}:`, top-level `^error: ` from cargo, Python `SyntaxError:`, `ImportError:`, JS `Uncaught SyntaxError:`.
2. `TestFailure` — `test result: FAILED\. \d+ passed; \d+ failed`, `\d+ tests? failed`, `FAILED \(failures=\d+\)`, `\d+ passing.*\d+ failing`.
3. `LintFailure` — clippy lints followed by `error: aborting due to`, `\d+ problems? \(\d+ errors?\)` from eslint, tsc `error TS\d{4}` without an accompanying compile crash.
4. `NetworkBindFailure` — `address already in use`, `EADDRINUSE`, `bind: address already in use`.
5. `Timeout` — synthesized when wall-clock > `timeout_ms`.
6. `Cancelled` — synthesized when stop was called or thread deleted.
7. `Lost` — synthesized only by `reconcile_on_startup`.
8. `Other(String)` — fallback, includes exit code and any short stderr signal.

### 8.3 Structured extraction

For each `FailureKind`, a regex-with-captures pulls structured fields into `failure_extracted`:

| Kind | Extracted JSON |
|---|---|
| `CompileError` | `{ "file": "src/lib.rs", "line": 42, "col": 7, "diagnostic_code": "E0277", "diagnostic_message": "the trait bound `T: Foo` is not satisfied" }` |
| `TestFailure` | `{ "test_name": "tests::session_persistence::reload_active_thread", "n_failed": 3, "n_passed": 1, "n_ignored": 0 }` (when multiple, the first failed test) |
| `LintFailure` | `{ "tool": "clippy", "rule": "clippy::needless_clone", "file": "…", "line": …, "n_errors": 2 }` |
| `NetworkBindFailure` | `{ "port": 3000, "address": "127.0.0.1", "process_hint": null }` |
| `Timeout` | `{ "timeout_ms": 600000, "elapsed_ms": 612345 }` |
| `Cancelled` | `{ "reason": "Stopped by LLM", "elapsed_ms": 278123 }` |
| `Lost` | `{ "elapsed_at_loss_ms": null, "log_preserved": true }` |
| `Other` | `{ "exit_code": 137, "signal_hint": "SIGKILL (likely OOM)" }` (when applicable) |

Storage: serialized to the `failure_extracted` TEXT column. Surfaced both in `JobView` and in the completion notification body so the LLM can react with structured tool calls (e.g. `read_file(path=src/lib.rs, line=42, lines=20)`).

### 8.4 Per-detector size

Each detector is ≤30 LOC of regex + capture-extraction + struct construction. Tested against fixtures under `crates/feature-coding-bash/tests/fixtures/`:

```
fixtures/
├── cargo_compile_error.txt
├── cargo_test_failed.txt
├── tsc_compile_error.txt
├── vitest_failure.txt
├── pytest_failure.txt
├── clippy_aborting.txt
├── eslint_errors.txt
├── eaddrinuse.txt
└── timeout_synth.txt
```

Each fixture is a real captured output from running the corresponding tool. The classification test runs the detector and asserts the exact `FailureKind` + `failure_extracted` JSON.

---

## 9. Approval & Concurrency

### 9.1 Approval

Background `bash` (`run_in_background=true`) shares the existing `Destructive` approval class with foreground `bash`. Persistent grants in `approval_grants` are keyed on `(tool_name="bash", command_pattern, session_id)`. A user who approves `cargo nextest *` for the session covers both foreground and background invocations.

`coding_task_list` and `coding_task_output`: `Safe` — no approval gate.

`coding_task_stop`: `Sensitive`. Per the unified-permission-gate spec, `Sensitive` gates on first use per-thread but auto-allows after that. The deliberate de-escalation principle: gating kill behind approval would perversely reward leaving runaway processes alive.

### 9.2 Plan-mode interaction

`CodingApprovalPolicy::PlanMode` (Phase 2.2) classifies all non-plan-file writes as Destructive. Background bash inherits this — in plan mode, `bash run_in_background=true` is rejected by the policy *unless* the command writes only to the plan file (which is essentially impossible for shell commands), so for v1 the practical answer is: **background bash is unavailable in plan mode**.

The plan-mode reminder body (from `crates/feature-coding-bash/src/render.rs`) includes a note: "Plan mode is active. Background jobs cannot be spawned. Use `/plan-exit` to ratify before running tests." This is rendered by `PlanModeInjector` which already knows about plan mode.

### 9.3 Concurrency cap

Cap = 6 active jobs per `(session_id, agent_chain)`. The chain — root → … → self — is carried on `RoutingContext` (§4.4); the cap counts every active job whose `agent_id` falls anywhere in the chain. A subagent and its parent therefore share the cap: parent has 5 active, subagent can spawn 1 more, the 7th call (from either) is rejected.

When the LLM calls `bash run_in_background=true` and the cap is exceeded, the tool returns an error tool result:

```text
Cannot spawn: 6 background jobs already active in this thread.
Active: bash-A, bash-B, bash-C, bash-D, bash-E, bash-F.
Use coding_task_stop(task_id) to free a slot, or wait for an active job to complete.
```

The cap is enforced at the `JobSupervisor::spawn` entry point, before any SQLite write or process spawn. No race window exists because the `DashMap.entry` API serializes inserts.

---

## 10. Subagent Inheritance

### 10.1 Job visibility

Subagents see jobs spawned by anyone in their agent chain. Specifically:

- `coding_task_list` returns rows where `session_id = ctx.session_id` AND `agent_id IN ctx.agent_chain` for `active_only=true`.
- `coding_task_output` and `coding_task_stop` permit any agent in the chain to read/stop any job in the chain.

The chain is carried on `RoutingContext.agent_chain` (§4.4): a `Vec<String>` ordered root → … → self. `SubagentManager` populates it when constructing the subagent's RoutingContext by appending the new agent's id to the parent's chain.

### 10.2 Cap sharing

Cap 6 is enforced over the whole `(session_id, agent_chain)` set — i.e. shared by every agent in the chain. Rationale: the user cares about how much the agent "consumed" in their thread, not how much each individual subagent did. The cap query is `SELECT COUNT(*) FROM coding_background_jobs WHERE session_id=? AND agent_id IN (chain) AND status IN ('Starting','Running')` — `IN` over a small list, so the existing index `idx_cbj_session_status` is sufficient.

### 10.3 Plan-policy snapshot

The Phase 2.2 `SubagentManager.plan_policy_snapshot` already forwards plan-mode state. We add `job_supervisor: Arc<dyn JobSupervisorHandle>` to the snapshot; subagents get the same handle the parent has, so they can see and operate on the parent's jobs. No separate registry per subagent.

---

## 11. Recovery & Restart

### 11.1 At supervisor init

`JobSupervisor::reconcile_on_startup()` is called once during `AppCore::init`, before any thread is opened.

```
SELECT id, log_path, final_path, started_at FROM coding_background_jobs
WHERE status IN ('Starting','Running');

for each row:
  if final_path is set and file exists:
    parse + reclassify gate → status=Completed/Failed (rare; means crash between finalize and row update)
  elif log_path file exists:
    status = "Lost"
    failure_kind = "Lost"
    failure_detail = "Klynt restarted while job was running"
    finished_at = now()
    enqueue ContextUpdate(CodingJobsChanged, body=lost_notification)
    DO NOT delete log_path — preserve for LLM inspection
  else:
    status = "Lost"
    no log to preserve
```

### 11.2 Orphan file sweep

After `reconcile_on_startup`, sweep `~/.klyntbot/jobs/`:

- For each `.log` and `.final` file, check if the corresponding `id` exists in SQLite. If not, unlink the file. (Catches: leftover from a row that was deleted while the file was open in another process; transient `.log.tmp` files from a crashed bisect.)
- For each `.log.tmp` file: unlink. These are always transient.

### 11.3 At thread open

When the user opens a thread that has Lost jobs, the `BackgroundJobsInjector` does NOT include Lost jobs in its active list (they're terminal). But the *first* iteration after thread open produces a one-shot notification (via the reconcile-time `ContextUpdate` that was queued at restart) that the LLM sees.

---

## 12. Error Handling

| Failure | Symptom | Klynt's response |
|---|---|---|
| Disk full during ring append | `RingFile::append` returns `ENOSPC` | Mark job `Failed`, `failure_kind=Other("disk full: <io_err>")`, kill child, surface via Tauri event + injector |
| Tauri crashes mid-job | Process orphaned with `setpgid` group | On restart: status=`Lost`, `.log` preserved, LLM reads partial output |
| Bisect occurs while a `coding_task_output` poll is in flight | Cursor below `bisect_low_water` | `read_delta` returns `bisect_occurred_since=true`; LLM sees flag and treats prior cursor as invalid |
| Concurrency cap exceeded | LLM calls `bash run_in_background=true` with 6 already active | Tool returns `is_error=true` with cap-exceeded message; no row written, no process spawned |
| `coding_task_output` of a deleted thread's job | Race: thread deleted between LLM tool calls | Tool returns `is_error=true` "Task not found or thread deleted" |
| Sandbox policy violation | Child exits with sandbox-specific exit code | Standard exit-code path; `failure_kind=Other("sandbox denied: <op>")` extracted from stderr if the policy emitted a message |
| Bisect rename failure | `rename .tmp → .log` returns IO error | Mark job `Failed`, `failure_kind=Other("log rotation failed: <io_err>")`, kill child, `.tmp` GC'd at startup |
| Process spawn failure | `Command::spawn` returns error | Tool call fails synchronously (no row written, no orphan). The LLM sees a normal tool error |
| `description` missing when `run_in_background=true` | Schema validation | Tool call fails synchronously with "description required when run_in_background=true" |
| `timeout_ms` exceeds 86_400_000 | Schema validation | Capped to 86_400_000 (24 h) with a warning in the tool result |

The principle: **errors surface to the LLM as tool results, not as Rust panics or silent state.** Every failure mode produces something the LLM can read and react to.

---

## 13. Testing Strategy

### 13.1 Inline unit tests

| File | Coverage |
|---|---|
| `feature-coding-bash/src/ring.rs` | Append below cap (no bisect); append over cap (bisect runs, generation increments); read_delta with cursor below low_water (`bisect_occurred_since=true`); concurrent append+read; finalize produces ≤256 KB; rename atomicity |
| `feature-coding-bash/src/gate.rs` | One test per `FailureKind` × per detector × per fixture: classification matches; structured extraction populates expected fields |
| `feature-coding-bash/src/supervisor.rs` | Spawn enforces cap; reap_session kills all session jobs; reconcile_on_startup marks orphans Lost |
| `feature-coding-bash/src/injector.rs` | `collect` returns 0 when no active jobs; returns 1 update with all active jobs listed; respects (session, agent_chain) scoping |
| `feature-coding-bash/src/render.rs` | Active-jobs reminder body shape; completion notification body shape; lost notification body shape |
| `klynt-pty/src/lib.rs` | spawn returns SpawnedJob; non-PTY honors `GIT_EDITOR=true`/`PAGER=cat`/`TERM=dumb`/`Stdio::null()`; pgid is captured |
| `storage/src/repos/coding_background_jobs.rs` | insert/update/list_active; cascade delete on session removal; index hits |
| `approval/src/coding_policy.rs` (extend) | `coding_task_stop` classified as `Sensitive`; `coding_task_list`/`output` classified `Safe`; `bash` with `run_in_background=true` classified `Destructive`; plan-mode rejection of background bash |

### 13.2 Integration tests

| File | Scenario |
|---|---|
| `crates/feature-coding-bash/tests/bg_smoke.rs` | Spawn `echo hello; sleep 1; echo world`; poll twice with cursor-delta; second poll returns only " world\n"; child completes; gate=Passed; final ≤256 KB |
| `bg_cancel.rs` | Spawn `sleep 30`; call stop with `reason="user"`; SIGTERM observed, 2 s grace, SIGKILL fires (use a sleeper that ignores SIGTERM); status=Cancelled |
| `bg_concurrency_cap.rs` | Spawn 6 jobs successfully; 7th fails with cap-reached error; stop one; 7th now succeeds |
| `bg_thread_cleanup.rs` | Spawn 2 jobs in session A; delete session A; both processes dead; rows gone; `.log` files swept |
| `bg_recovery.rs` | Insert a fake `Running` row + fake `.log` file; run reconcile_on_startup; status=Lost; `.log` preserved; ContextUpdate emitted |
| `bg_subagent_inheritance.rs` | Parent spawns job A; subagent calls `coding_task_list` and sees A; subagent calls `coding_task_stop(A)` and succeeds |
| `bg_gate_classification.rs` | Run `cargo build` against broken toy crate → `failure_kind=CompileError` with extracted file/line; failing test → `TestFailure` with extracted test_name; bind a port twice → `NetworkBindFailure` with extracted port |
| `bg_push_on_completion.rs` | Spawn `false` (immediate fail); without polling, observe ContextUpdateQueue receives a CodingJobsChanged update with body containing the gate result |
| `bg_silent_completion.rs` | Spawn `true` with `silent_completion=true`; observe NO ContextUpdate is enqueued, only a Tauri event |
| `bg_bisect_during_poll.rs` | Spawn a chatty job that produces 5 MB of output; poll at offset 2 MB; trigger bisect; subsequent poll returns `bisect_occurred_since=true` |
| `bg_disk_full.rs` (gated by `feature = "fault-injection"`) | Inject `ENOSPC` on `RingFile::append`; observe job marked Failed with `Other("disk full: …")` and child killed |

### 13.3 Frontend tests

`desktop-ui/src/features/coding/components/JobsPanel.test.tsx`:

- Renders empty state when no jobs
- Renders 6 jobs in started-desc order
- Updates row on `coding:job_event` Tauri event (status transition, byte count, gate result)
- Last-3-lines preview updates as bytes arrive (mocked event stream)
- Clicking "Stop" invokes `coding_task_stop` Tauri command with the right id
- Clicking job row opens an inline output viewer with cursor advance via `coding_task_output`

`desktop-ui/src/features/coding/state/jobsStore.test.ts`:

- Subscribe/unsubscribe symmetry
- Optimistic update on user-initiated stop
- Reconciliation when Tauri event arrives after optimistic update

### 13.4 Manual smoke checklist

Documented in the implementation plan; run at end of each PR:

1. Open coding thread; run `bash` with `run_in_background=true, command="cargo nextest run -p agent"`. Observe job appears in JobsPanel within 1 s.
2. Continue chatting for 2 minutes; observe each LLM reply gets a per-turn reminder via the injector.
3. Job completes; observe completion notification appears in the next LLM iteration with gate result.
4. Force-kill Tauri mid-job; restart; observe Lost notification on next thread open.
5. Spawn 7 jobs; observe 7th rejected with cap message.
6. Delete the thread mid-job; observe processes are terminated and JobsPanel clears.

---

## 14. Future Phases

### Phase 2.3b — "Execution Intelligence" (~3 days)

- **Plan-mode integration:** when plan mode is active and a `TodoItem` matches verification patterns (`Run`, `Test`, `Check`, `Verify`), the `BackgroundJobsInjector` adds a one-line affordance: "TodoItem 'Run integration tests' looks like a background-job candidate; spawn with `bash(command=…, run_in_background=true)` after ratification." Surface only — the LLM still initiates.
- **Output diffing across runs:** on `JobCompleted` for a job whose `command` matches a previously-completed job (same `session_id`, same normalized command), compute a structured diff of `failure_extracted` (e.g. "new test failures: A, B; still failing: C; resolved: D"). Inject diff in completion notification.
- **Episodic memory:** `BackgroundJobSignalSource : MirrorSignalSource` — the cognitive layer subscribes to `JobCompleted` events. Distillation extracts patterns like "test X fails when commit-set Y is present" and stores in `episodic_memories`. Mirrors `TodoSignalSource` from Phase 2.1.

### Phase 2.3c — "Interactive Compute" (~3-4 days, when needed)

- PTY support: `tty: bool` flag on `bash` activates `klynt-pty::ChildHandle::Pty` branch (currently a placeholder). Fixed 80×24 in initial cut.
- Interactive stdin: new tool `coding_task_stdin(task_id, data)` (`Sensitive`). Requires `interactive: bool` on the original spawn.
- TTY resize: `coding_task_resize(task_id, rows, cols)`. Triggers SIGWINCH in the PTY.
- Trigger criterion: not built speculatively. Built when at least one user request demands `npm init`-style interactive flows or visible TTY-aware command output (e.g. `cargo` colors).

---

## 15. Game-Changer Scorecard

| Pillar | 2.3a | 2.3b | 2.3c |
|---|---|---|---|
| Never forgets (per-turn injector) | ✅ | ✅ | ✅ |
| Push-on-completion (no polling required) | ✅ | ✅ | ✅ |
| Structured failure extraction (file/line/test_name/port) | ✅ | ✅ | ✅ |
| Tauri-restart recovery (honest Lost status) | ✅ | ✅ | ✅ |
| Subagent inheritance | ✅ | ✅ | ✅ |
| Cross-run output diffing | ❌ | ✅ | ✅ |
| Plan-mode auto-affordance | ❌ | ✅ | ✅ |
| Episodic memory of past failures | ❌ | ✅ | ✅ |
| Interactive (PTY/stdin/resize) | ❌ | ❌ | ✅ |

**2.3a alone clears 5 of the 9 pillars** — including the three load-bearing claims of "never forgets / understands failure / pushes on completion." 2.3b adds the cross-run intelligence that turns the feature from "useful" to "remembers what you've been doing." 2.3c is purely interactive-shell territory and stays optional until demand surfaces.

---

## Appendix A — Verification commands

After implementation:

```bash
# Tests
cargo nextest run -p feature-coding-bash
cargo nextest run -p storage -E 'test(coding_background_jobs)'
cargo nextest run -E 'test(bg_)'
cd desktop-ui && bun run test -- JobsPanel jobsStore

# Lint + format
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check

# Doctest
cargo test --workspace --doc

# Manual
cargo tauri dev
# Then run the §13.4 smoke checklist
```

## Appendix B — Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| Bisect during a poll-in-flight produces incorrect bytes | Low | `bisect_generation` counter + `bisect_occurred_since` flag in `RingRead` |
| SQLite WAL contention with 6 active jobs writing status updates | Low | Status updates are coarse (transitions only), not per-byte |
| `sandbox-exec` blocks PTY device access on macOS | N/A in 2.3a | Defer to 2.3c when PTY ships; investigate at that point |
| Subagent spawns push parent thread over cap | Medium | Cap is `(session, agent_chain)`-scoped; rejection at spawn entry |
| User force-kills Tauri while ring-buffer write is in flight | Low | `BufWriter` is flushed on each append; partial writes mean the last <8 KB might be lost; `.log` is still readable |
| Reconcile-on-startup misclassifies a successfully-finalized job as Lost | Very low | Check for `.final` file existence before marking Lost; sweep .log only when no .final present |
| LLM ignores the per-turn reminder and never polls | Low | Push-on-completion ensures terminal events are always delivered; reminder is for in-progress awareness only |
| Disk fills with `.final` files over time | Medium | Future: GC `.final` files older than N days; for 2.3a, tolerated (256 KB × 1000 jobs = 256 MB, acceptable) |
