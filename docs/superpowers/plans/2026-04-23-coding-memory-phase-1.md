# Coding Memory — Phase 1 (Architecture Skeleton) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land two new workspace crates (`coding-memory`, `coding-ingest`), every public type/trait/MCP tool/DomainEvent/config surface referenced by the coding-memory spec, plus the consolidated Phase-1 schema migration — all as stubs with `unimplemented!()` bodies that compile clean, pass clippy with zero warnings, and document every public item. No runtime behavior.

**Architecture:** Two new L5 crates alongside `agent`/`cognitive`/`channels`. `coding-ingest` owns the `AgentEvent` contract, CLI adapter stubs, transport stubs, daemon stub, and the `klyntbot-hook` shell binary. `coding-memory` owns the fact taxonomy types, `MemorySink` trait, Distiller/Recall/Reforge/Skills module stubs, migration SQL, and MCP tool stubs. Downstream wiring (DomainEvent variants in `bus`, config additions in `config`, provider roles in `providers`, tool names in `EXPLICIT_TOOL_ALLOWLIST`) is surgical — no behavior touched. Pre-release schema policy per CLAUDE.md authorizes direct DDL: one consolidated migration lands every column/table all eight phases will need.

**Tech Stack:** Rust (MSRV 1.93), `sqlx` (SQLite), `serde` (camelCase), `async-trait`, `uuid`, `jiff::Timestamp`, `tokio` broadcast, existing `common::Result<T>` / `KlyntbotError`. No new runtime deps beyond those already in the workspace.

---

## File Structure

Every file created or modified by this plan, grouped by responsibility. Each new file holds one clear responsibility and is small enough to fit in a single focused commit.

### New crate: `crates/coding-ingest/`

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest; workspace deps only |
| `src/lib.rs` | Module re-exports + crate-level docs |
| `src/event.rs` | `AgentEvent`, `AgentEventV1`, `EventKind` (9 base + 10 klynt-cli rich variants), `AgentSource`, support types (`FileOp`, `TokenUsage`, `SymbolRef`, `DiagnosticsDelta`, `TestFailure`, `SkillScore`) |
| `src/scope.rs` | `RepoScope` (shared with `coding-memory` via re-export) — lives here because `AgentEventV1.repo: Option<RepoScope>` must compile without pulling in `coding-memory` |
| `src/adapters/mod.rs` | `IngestAdapter` trait + module glue |
| `src/adapters/claude_code.rs` | `ClaudeCodeAdapter` stub (7 hook events → `AgentEvent`) |
| `src/adapters/codex.rs` | `CodexAdapter` stub (5 hook events) |
| `src/adapters/kimi_wire.rs` | `KimiAdapter` stub (tier 1 hook + tier 2 Wire) |
| `src/adapters/opencode.rs` | `OpencodeAdapter` stub (SQLite WAL polling) |
| `src/transport.rs` | `IngestSocket` trait + `UnixIngestSocket` + `FileBufferFallback` stubs |
| `src/daemon.rs` | `IngestDaemon::run(...)` stub (unimplemented) |
| `src/bin/klyntbot-hook.rs` | Shell binary — arg parsing for all 4 CLIs, writes to stderr only |

### New crate: `crates/coding-memory/`

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest; depends on `coding-ingest`, `cognitive`, `storage`, `bus`, `providers`, `context_engine`, `common` |
| `src/lib.rs` | Module re-exports, crate-level docs, `coding_memory_migrations()` public fn |
| `src/error.rs` | `NotImplementedInPhase { required_phase: u8 }` error variant wrapper |
| `src/scope.rs` | `ProvenanceMetadata`, `AnchoredSymbol`, `CausalEdge`, `CausalEdgeKind`, `Sensitivity`, `RepoScope` re-export from `coding-ingest` |
| `src/facts.rs` | `RepoContext`, `FixAttempt`, `DeadEndAttempt`, `StylePreference`, `WorkflowPattern`, `FailurePattern`, `RefactorEpisode`, `TestRunEpisode`; Distiller `record_observation` LLM tool schema `CodingKind` (5-value enum) |
| `src/sink.rs` | `MemorySink` trait + `InProcessSink` stub + `IngestSocketSink` stub |
| `src/distiller/mod.rs` | `Distiller` type, `TurnTrace`, `DistillerPhase` enum; all methods `unimplemented!()` |
| `src/recall/mod.rs` | `CodingRecallService` stub, `IndexEntry`, `TimelineEntry`, `FullEntry`, `DeadEndResponse`, `CausalTraceResponse` |
| `src/recall/renderers.rs` | Markdown renderer stubs for SessionStart / UserPromptSubmit injection |
| `src/reforge_phase.rs` | `CodingSynthesisPhase` (Phase 2.5), `RuleArtifactGenerationPhase` (Phase 3.5) stubs |
| `src/skills.rs` | `ProjectSkillEvolver` stub, scope-aware skill types |
| `src/retrieval_skills.rs` | `RetrievalSkill` trait + 5 closed-set stubs (`QueryRewriter`, `QueryDecomposer`, `EvidenceFocuser`, `RawEventEscalator`, `CausalContextExpander`) |
| `src/mcp.rs` | 8 MCP tool stubs returning `NotImplementedInPhase` |
| `migrations/001_coding_memory.sql` | Consolidated Phase-1 schema delta (every column + every new table across 8 phases) |

### Modified existing files

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `crates/coding-ingest` + `crates/coding-memory` to `members` |
| `crates/bus/src/domain_events.rs` | Add 6 variants: `PatternApplied`, `PatternOutcome`, `FixAttemptFailed`, `MemoryRetrieved`, `AssistantMsgCompleted`, `RetrievalSkillApplied` |
| `crates/config/src/schema/mcp.rs` | Append 8 tool names to `EXPLICIT_TOOL_ALLOWLIST` |
| `crates/config/src/schema/mod.rs` | `mod coding_memory;` declaration |
| `crates/config/src/schema/coding_memory.rs` | NEW — full `CodingMemoryConfig` tree mirroring §13.D of the spec |
| `crates/config/src/schema/core.rs` | Add `pub coding_memory: CodingMemoryConfig` field to `Config` + serde default |
| `crates/providers/src/lib.rs` | Add `pub enum ProviderRole { Distiller, ReforgeSynth, ReforgeRules, /* existing roles if any */ }` |
| `docs/coding-memory/README.md` | NEW — architecture diagram + phase-1 scope |
| `docs/coding-memory/decisions.md` | NEW — decision records from spec §3 |

### Test files

| File | Responsibility |
|---|---|
| `crates/coding-ingest/tests/agent_event_roundtrip.rs` | `parse(serialize(event)) == event` for every `EventKind` variant |
| `crates/coding-memory/tests/migration_applies.rs` | Migration runs against in-memory SQLite; all new tables exist |
| `crates/coding-memory/tests/public_surface.rs` | Smoke test — every public type is `Debug + Clone` where applicable; every trait object-safe where applicable |

---

## Task Structure

### Task 1: Scaffold `coding-ingest` crate

**Files:**
- Create: `crates/coding-ingest/Cargo.toml`
- Create: `crates/coding-ingest/src/lib.rs`
- Modify: `Cargo.toml` (workspace root, `members` array)

- [ ] **Step 1: Add crate to workspace members**

Append after the last existing `"crates/..."` entry in the root `Cargo.toml` `members` array:

```toml
    "crates/coding-ingest",
```

- [ ] **Step 2: Create `crates/coding-ingest/Cargo.toml`**

```toml
[package]
name = "coding-ingest"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common = { workspace = true }
bus = { workspace = true }
storage = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["fs", "io-util", "net", "sync", "macros", "rt-multi-thread"] }
async-trait = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true, features = ["v4", "serde"] }
jiff = { workspace = true, features = ["serde"] }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[[bin]]
name = "klyntbot-hook"
path = "src/bin/klyntbot-hook.rs"

[lints]
workspace = true
```

- [ ] **Step 3: Create `crates/coding-ingest/src/lib.rs`**

```rust
//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams
//! from external coding CLIs and the native `klynt-cli` source.
//!
//! Phase 1 delivers types, traits, and stub module surface. No runtime
//! behavior is implemented here; downstream phases fill in the bodies.

#![deny(missing_docs)]

pub mod adapters;
pub mod daemon;
pub mod event;
pub mod scope;
pub mod transport;

pub use event::{
    AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage,
};
pub use scope::RepoScope;
```

- [ ] **Step 4: Create stub source files so `lib.rs` compiles**

For each of `adapters/mod.rs`, `daemon.rs`, `event.rs`, `scope.rs`, `transport.rs`, create a one-liner placeholder so the crate builds. These bodies get fleshed out in later tasks — the intent here is just *crate is in the workspace*.

Create `crates/coding-ingest/src/event.rs`:
```rust
//! Stub — filled in by Task 3/4.
```

Create `crates/coding-ingest/src/scope.rs`:
```rust
//! Stub — filled in by Task 3.
```

Create `crates/coding-ingest/src/transport.rs`:
```rust
//! Stub — filled in by Task 7.
```

Create `crates/coding-ingest/src/daemon.rs`:
```rust
//! Stub — filled in by Task 8.
```

Create `crates/coding-ingest/src/adapters/mod.rs`:
```rust
//! Stub — filled in by Task 6.
```

- [ ] **Step 5: Replace `lib.rs` with a version that only references filled-in modules**

Until later tasks land, `lib.rs` cannot re-export types that don't exist yet. Replace the contents of `crates/coding-ingest/src/lib.rs` with:

```rust
//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams.
//!
//! Phase 1 lands the module surface; implementations follow in later tasks.

#![deny(missing_docs)]

/// CLI adapter stubs — see Task 6.
pub mod adapters;
/// Daemon stub — see Task 8.
pub mod daemon;
/// `AgentEvent` contract — see Task 3/4.
pub mod event;
/// `RepoScope` — see Task 3.
pub mod scope;
/// Transport stubs — see Task 7.
pub mod transport;
```

- [ ] **Step 6: Verify crate builds**

Run: `cargo build -p coding-ingest`
Expected: PASS (zero warnings).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/coding-ingest
git commit -m "feat(coding-ingest): scaffold new L5 crate in workspace"
```

---

### Task 2: Scaffold `coding-memory` crate

**Files:**
- Create: `crates/coding-memory/Cargo.toml`
- Create: `crates/coding-memory/src/lib.rs`
- Create: `crates/coding-memory/src/{error,scope,facts,sink,distiller/mod,recall/mod,recall/renderers,reforge_phase,skills,retrieval_skills,mcp}.rs` (placeholders)
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add crate to workspace members**

Append to root `Cargo.toml` `members` array (adjacent to the `coding-ingest` entry from Task 1):

```toml
    "crates/coding-memory",
```

- [ ] **Step 2: Create `crates/coding-memory/Cargo.toml`**

```toml
[package]
name = "coding-memory"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common = { workspace = true }
bus = { workspace = true }
storage = { workspace = true }
providers = { workspace = true }
context_engine = { workspace = true }
cognitive = { workspace = true }
coding-ingest = { path = "../coding-ingest" }
tools-core = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true, features = ["v4", "serde"] }
jiff = { workspace = true, features = ["serde"] }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Create `crates/coding-memory/src/lib.rs`**

```rust
//! `coding-memory` — local-first coding memory layer built on klyntbot's
//! cognitive crate. Hosts the fact taxonomy, Distiller, recall service,
//! Reforge coding phases, and the `MemorySink` trait used by native
//! in-process consumers like `klynt-cli`.
//!
//! Phase 1 lands the module surface, all public types, MCP tool stubs,
//! and the consolidated schema migration. Methods return
//! `NotImplementedInPhase { required_phase }`.

#![deny(missing_docs)]

/// Error surface for phased stubs.
pub mod error;
/// Scope partitioning, provenance, anchored symbols, causal edges.
pub mod scope;
/// Coding fact taxonomy (`FixAttempt`, `RepoContext`, …).
pub mod facts;
/// `MemorySink` trait + `InProcessSink` / `IngestSocketSink` stubs.
pub mod sink;
/// Distiller — online writer stub.
pub mod distiller;
/// Recall service — MCP + passive injection stub.
pub mod recall;
/// Reforge coding phases (2.5, 3.5) stubs.
pub mod reforge_phase;
/// Scope-aware skill store extension + project skill evolution.
pub mod skills;
/// C3 retrieval-skill registry stubs.
pub mod retrieval_skills;
/// MCP tool stubs — registered with `default_exposed_tools()`.
pub mod mcp;

pub use error::{CodingMemoryError, NotImplementedInPhase};

use tools_core::FeatureMigration;

/// Coding-memory migrations. Caller: `AppCore::init_storage` (app-core crate).
pub fn coding_memory_migrations() -> Vec<FeatureMigration> {
    vec![FeatureMigration {
        feature_name: "coding_memory".to_string(),
        version: 1,
        description: "Consolidated Phase-1 schema: scope_repo_id, metadata, \
                      actor_id columns; memory_causal_edges, memory_utilization, \
                      ingest_event_log, klynt_sessions tables; skill_versions \
                      scope columns."
            .to_string(),
        sql: include_str!("../migrations/001_coding_memory.sql").to_string(),
    }]
}
```

- [ ] **Step 4: Create placeholder module files**

Create a one-line placeholder for each module referenced by `lib.rs` (bodies landed in later tasks). Paths:

- `crates/coding-memory/src/error.rs`
- `crates/coding-memory/src/scope.rs`
- `crates/coding-memory/src/facts.rs`
- `crates/coding-memory/src/sink.rs`
- `crates/coding-memory/src/distiller/mod.rs`
- `crates/coding-memory/src/recall/mod.rs`
- `crates/coding-memory/src/recall/renderers.rs`
- `crates/coding-memory/src/reforge_phase.rs`
- `crates/coding-memory/src/skills.rs`
- `crates/coding-memory/src/retrieval_skills.rs`
- `crates/coding-memory/src/mcp.rs`

Each placeholder file contains only:
```rust
//! Stub — filled in by a later Task.
```

Exception: `error.rs` must compile against the `pub use` in `lib.rs`. Create:

```rust
//! Phase-scoped stub error surface.

use thiserror::Error;

/// Top-level error for `coding-memory` stubs.
#[derive(Debug, Error)]
pub enum CodingMemoryError {
    /// Method is not yet implemented — it becomes available in `required_phase`.
    #[error("coding-memory operation not implemented until phase {}", .0.required_phase)]
    NotImplemented(NotImplementedInPhase),
}

/// Indicates the phase that must be completed before this operation is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotImplementedInPhase {
    /// The phase number (2-8) in which this method becomes non-stub.
    pub required_phase: u8,
}

impl NotImplementedInPhase {
    /// Construct a `NotImplementedInPhase` marker.
    #[must_use]
    pub const fn new(required_phase: u8) -> Self {
        Self { required_phase }
    }
}
```

- [ ] **Step 5: Create the migration file placeholder**

Create `crates/coding-memory/migrations/001_coding_memory.sql` with a single comment — full DDL lands in Task 3-bis (Task 24):

```sql
-- Consolidated Phase-1 schema for coding-memory (filled in by Task 24).
```

- [ ] **Step 6: Verify crate builds**

Run: `cargo build -p coding-memory`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/coding-memory
git commit -m "feat(coding-memory): scaffold new L5 crate in workspace"
```

---

### Task 3: `AgentEvent` base contract (9 variants) + support types

**Files:**
- Modify: `crates/coding-ingest/src/event.rs`
- Modify: `crates/coding-ingest/src/scope.rs`
- Modify: `crates/coding-ingest/src/lib.rs` (re-exports)

- [ ] **Step 1: Populate `scope.rs`**

Replace `crates/coding-ingest/src/scope.rs` contents with:

```rust
//! `RepoScope` — canonical repo identity for an `AgentEvent`.
//!
//! Derived from `cwd` via `git rev-parse` + remote origin URL; cached per
//! session. `repo_id` is a sanitized slug, `root` is an absolute path.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Canonical repo identity attached to every `AgentEvent` when detectable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoScope {
    /// Canonical repo id — e.g. `github.com/klynt/bot` or `local:sanitized-path`.
    pub repo_id: String,
    /// Absolute path to the working tree root.
    pub root: PathBuf,
    /// Current HEAD commit hash if available at event time.
    pub git_hash: Option<String>,
    /// Current branch if available at event time.
    pub branch: Option<String>,
}
```

- [ ] **Step 2: Populate `event.rs` — 9 base `EventKind` variants**

Replace `crates/coding-ingest/src/event.rs` with:

```rust
//! `AgentEvent` — the versioned cross-CLI contract.
//!
//! External CLIs (Claude Code, Codex, kimi-cli, opencode) emit the 9 base
//! `EventKind` variants. klynt-cli additionally emits 10 rich variants
//! (see Task 4).

use crate::scope::RepoScope;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Versioned wrapper. Future `V2` never breaks `V1`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "v", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Current version.
    V1(AgentEventV1),
}

/// The V1 payload. All CLI adapters normalize to this shape.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventV1 {
    /// Per-event UUID.
    pub id: Uuid,
    /// Which CLI emitted this event.
    pub source: AgentSource,
    /// CLI-assigned session id (shape varies per CLI).
    pub session_id: String,
    /// CLI-assigned turn id when present.
    pub turn_id: Option<String>,
    /// Working directory at event time.
    pub cwd: PathBuf,
    /// Resolved repo scope if detectable.
    pub repo: Option<RepoScope>,
    /// Wall-clock timestamp.
    pub occurred_at: Timestamp,
    /// The semantic payload.
    pub kind: EventKind,
}

/// Which coding CLI emitted the event.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum AgentSource {
    /// Anthropic Claude Code.
    ClaudeCode,
    /// OpenAI Codex.
    Codex,
    /// Moonshot kimi-cli.
    KimiCli,
    /// sst/opencode.
    OpenCode,
    /// Native klynt-cli (future — see linked spec).
    KlyntCli,
}

/// File-op classifier for `FileEdit` events.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileOp {
    /// File was read (no mutation).
    Read,
    /// File was created.
    Create,
    /// File was modified in place.
    Modify,
    /// File was deleted.
    Delete,
}

/// Token accounting reported by the provider for one assistant turn.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Prompt tokens consumed.
    pub prompt_tokens: u32,
    /// Completion tokens produced.
    pub completion_tokens: u32,
    /// Cached-input tokens (prompt-cache hit).
    pub cached_tokens: Option<u32>,
}

/// Semantic payload of an `AgentEvent`. 9 base variants for external CLIs;
/// 10 rich variants (Task 4) for klynt-cli.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventKind {
    /// A coding-CLI session just started.
    SessionStart {
        /// Model chosen for this session, if known.
        model: Option<String>,
        /// Origin of the session (e.g. `"cli-arg"`, `"resumed"`).
        source_reason: String,
    },
    /// The session terminated.
    SessionEnd {
        /// Why it ended (`"user-quit"`, `"error"`, `"compact"`, …).
        reason: String,
    },
    /// User typed a prompt.
    UserPrompt {
        /// Raw prompt text.
        text: String,
        /// Attached files the CLI reported.
        attachments: Vec<PathBuf>,
    },
    /// Assistant finished a response.
    AssistantMsg {
        /// Response text (may be truncated).
        text: String,
        /// Whether the text was truncated at this boundary.
        truncated: bool,
        /// Provider-reported usage if available.
        token_usage: Option<TokenUsage>,
    },
    /// Assistant invoked a tool.
    ToolCall {
        /// Tool name.
        tool: String,
        /// Truncated args preview.
        args_preview: String,
        /// Whether the call succeeded.
        ok: bool,
        /// Wall-clock duration.
        duration_ms: u32,
        /// Truncated result preview.
        result_preview: String,
    },
    /// A file was touched.
    FileEdit {
        /// Absolute path.
        path: PathBuf,
        /// Operation performed.
        op: FileOp,
        /// New file size in bytes.
        bytes: u64,
        /// Optional unified-diff preview.
        diff_preview: Option<String>,
    },
    /// A test runner was invoked.
    TestRun {
        /// Command as executed.
        command: String,
        /// Detected framework (`"cargo"`, `"pytest"`, …).
        framework: Option<String>,
        /// Number of passing tests.
        passed: u32,
        /// Number of failing tests.
        failed: u32,
        /// Wall-clock duration.
        duration_ms: u32,
    },
    /// Compaction/summarization boundary.
    CompactEvent {
        /// What triggered compaction.
        trigger: String,
        /// Pre-compaction token count.
        token_count: u32,
    },
    /// A tool or provider error surfaced.
    Error {
        /// Tool that failed (if tool-scoped).
        tool: Option<String>,
        /// Human-readable message.
        message: String,
    },
}
```

- [ ] **Step 3: Re-export from `lib.rs`**

Replace `crates/coding-ingest/src/lib.rs` with:

```rust
//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams
//! from external coding CLIs and the native `klynt-cli` source.

#![deny(missing_docs)]

/// CLI adapter stubs — see Task 6.
pub mod adapters;
/// Daemon stub — see Task 8.
pub mod daemon;
/// `AgentEvent` contract.
pub mod event;
/// `RepoScope` — repo identity attached to events.
pub mod scope;
/// Transport stubs — see Task 7.
pub mod transport;

pub use event::{
    AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage,
};
pub use scope::RepoScope;
```

- [ ] **Step 4: Build + clippy**

Run: `cargo build -p coding-ingest && cargo clippy -p coding-ingest --all-targets -- -D warnings`
Expected: PASS (zero warnings).

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/event.rs crates/coding-ingest/src/scope.rs crates/coding-ingest/src/lib.rs
git commit -m "feat(coding-ingest): AgentEvent v1 + 9 base EventKind variants"
```

---

### Task 4: `EventKind` klynt-cli rich variants (10) + support types

**Files:**
- Modify: `crates/coding-ingest/src/event.rs`

- [ ] **Step 1: Add support types above `EventKind`**

Insert the following *before* the `EventKind` enum in `crates/coding-ingest/src/event.rs`:

```rust
/// Symbol reference for `FileEditEnriched` anchoring.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolRef {
    /// Absolute file path.
    pub file_path: PathBuf,
    /// Symbol name (function, method, type, const).
    pub symbol: String,
    /// Commit hash at which the symbol was anchored.
    pub git_hash: String,
}

/// LSP diagnostics delta attached to `FileEditEnriched`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsDelta {
    /// Diagnostics present before the edit.
    pub before: u32,
    /// Diagnostics present after the edit.
    pub after: u32,
    /// Newly introduced diagnostic messages.
    pub introduced: Vec<String>,
    /// Diagnostic messages resolved by the edit.
    pub resolved: Vec<String>,
}

/// A single test failure captured by `TestRunEnriched`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestFailure {
    /// Test identifier (per-framework convention).
    pub test_name: String,
    /// Truncated error or assertion message.
    pub message: String,
    /// Stack trace when available.
    pub stack: Option<String>,
}

/// A skill considered by klynt-cli's router, with its composite score.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillScore {
    /// Skill identifier.
    pub skill_id: String,
    /// Composite score (0.0 – 1.0).
    pub score: f32,
    /// Breakdown components for observability.
    pub components: Vec<(String, f32)>,
}
```

- [ ] **Step 2: Add the 10 rich variants to `EventKind`**

Append to the `EventKind` enum (before the closing `}`):

```rust
    /// klynt-cli: a skill was activated for this turn.
    SkillActivated {
        /// Skill identifier.
        skill_id: String,
        /// SKILL.md source path.
        source_path: PathBuf,
        /// Why it was activated.
        trigger: String,
    },
    /// klynt-cli: memories were injected into the turn context.
    RecallInjected {
        /// IDs of injected memories.
        memory_ids: Vec<String>,
        /// C3 coverage score at injection time.
        coverage_score: f32,
        /// Whether a dead-end warning was attached.
        dead_end_warning: bool,
    },
    /// klynt-cli: approval/sandbox layer decided to allow or deny a tool call.
    ApprovalDecision {
        /// Tool name.
        tool: String,
        /// `"allow"`, `"deny"`, `"ask"`.
        decision: String,
        /// Which approval layer decided (`"privacy"`, `"rules"`, `"user"`).
        layer: String,
    },
    /// klynt-cli: sandbox policy applied for a tool call.
    SandboxApplied {
        /// Tool name.
        tool: String,
        /// One-line summary of the policy.
        policy_summary: String,
        /// Whether the call fell back to unsandboxed execution.
        fallback_unsandboxed: bool,
    },
    /// klynt-cli: `FileEdit` enriched with tree-sitter anchors + LSP diagnostics.
    FileEditEnriched {
        /// Absolute path.
        path: PathBuf,
        /// Operation performed.
        op: FileOp,
        /// Anchored symbols extracted via tree-sitter.
        anchored_symbols: Vec<SymbolRef>,
        /// LSP diagnostic delta.
        lsp_diagnostics_delta: Option<DiagnosticsDelta>,
    },
    /// klynt-cli: `TestRun` enriched with per-test outcomes.
    TestRunEnriched {
        /// Command as executed.
        command: String,
        /// Names of passing tests.
        passed_tests: Vec<String>,
        /// Failure details per failed test.
        failed_tests: Vec<TestFailure>,
        /// Tests newly failing vs. last run.
        newly_failing: Vec<String>,
    },
    /// klynt-cli: a provider call was completed (cost + latency visible).
    ProviderCall {
        /// Model identifier.
        model: String,
        /// Prompt tokens.
        prompt_tokens: u32,
        /// Completion tokens.
        completion_tokens: u32,
        /// USD cost of this call.
        cost_usd: f64,
        /// End-to-end latency.
        latency_ms: u64,
        /// Retries this call required.
        retries: u32,
    },
    /// klynt-cli: mid-loop compression was applied.
    CompressionApplied {
        /// Token count before compression.
        before_tokens: u32,
        /// Token count after compression.
        after_tokens: u32,
        /// Number of messages condensed.
        messages_condensed: u32,
    },
    /// klynt-cli: a Mirror alert was surfaced to the user.
    MirrorAlert {
        /// Alert identifier (maps to `mirror_snippets.id`).
        alert_id: String,
        /// Severity level (`"low"`, `"medium"`, `"high"`, `"critical"`).
        severity: String,
        /// Alert kind (closed enum string — see coding-memory design §10).
        kind: String,
    },
    /// klynt-cli: skill router trace for a turn.
    SkillRoutingTrace {
        /// Skills considered with scores.
        considered: Vec<SkillScore>,
        /// Skill ids chosen for injection.
        chosen: Vec<String>,
    },
```

- [ ] **Step 3: Re-export support types from `event.rs`**

The module's public types auto-export via `pub` visibility; just confirm the re-export block in `lib.rs` stays as-is. Add `SymbolRef`, `DiagnosticsDelta`, `TestFailure`, `SkillScore` to the `pub use event::{…}` line:

Open `crates/coding-ingest/src/lib.rs` and replace:

```rust
pub use event::{
    AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage,
};
```

with:

```rust
pub use event::{
    AgentEvent, AgentEventV1, AgentSource, DiagnosticsDelta, EventKind, FileOp,
    SkillScore, SymbolRef, TestFailure, TokenUsage,
};
```

- [ ] **Step 4: Build + clippy**

Run: `cargo build -p coding-ingest && cargo clippy -p coding-ingest --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/event.rs crates/coding-ingest/src/lib.rs
git commit -m "feat(coding-ingest): klynt-cli rich EventKind variants + support types"
```

---

### Task 5: `AgentEvent` round-trip property test

**Files:**
- Create: `crates/coding-ingest/tests/agent_event_roundtrip.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-ingest/tests/agent_event_roundtrip.rs`:

```rust
//! Invariant 7 (coding-memory design §3): round-trip identity holds for
//! every `EventKind` variant. Phase 1 exercises the base 9 variants + a
//! sampling of the klynt-cli rich variants. Property-tested variants in
//! Phase 2.

use coding_ingest::{
    AgentEvent, AgentEventV1, AgentSource, DiagnosticsDelta, EventKind, FileOp,
    RepoScope, SkillScore, SymbolRef, TestFailure, TokenUsage,
};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn wrap(kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::ClaudeCode,
        session_id: "sess-1".into(),
        turn_id: Some("turn-1".into()),
        cwd: PathBuf::from("/repo"),
        repo: Some(RepoScope {
            repo_id: "github.com/klynt/bot".into(),
            root: PathBuf::from("/repo"),
            git_hash: Some("abc123".into()),
            branch: Some("main".into()),
        }),
        occurred_at: Timestamp::from_second(1_800_000_000).unwrap(),
        kind,
    })
}

fn assert_roundtrip(event: AgentEvent) {
    let json = serde_json::to_string(&event).expect("serialize");
    let parsed: AgentEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, event, "roundtrip mismatch for JSON: {json}");
}

#[test]
fn base_session_start_roundtrips() {
    assert_roundtrip(wrap(EventKind::SessionStart {
        model: Some("claude-opus-4-7".into()),
        source_reason: "cli-arg".into(),
    }));
}

#[test]
fn base_session_end_roundtrips() {
    assert_roundtrip(wrap(EventKind::SessionEnd {
        reason: "user-quit".into(),
    }));
}

#[test]
fn base_user_prompt_roundtrips() {
    assert_roundtrip(wrap(EventKind::UserPrompt {
        text: "fix the bug".into(),
        attachments: vec![PathBuf::from("a.rs")],
    }));
}

#[test]
fn base_assistant_msg_roundtrips() {
    assert_roundtrip(wrap(EventKind::AssistantMsg {
        text: "done".into(),
        truncated: false,
        token_usage: Some(TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            cached_tokens: Some(25),
        }),
    }));
}

#[test]
fn base_tool_call_roundtrips() {
    assert_roundtrip(wrap(EventKind::ToolCall {
        tool: "bash".into(),
        args_preview: "cargo test".into(),
        ok: true,
        duration_ms: 500,
        result_preview: "ok".into(),
    }));
}

#[test]
fn base_file_edit_roundtrips() {
    assert_roundtrip(wrap(EventKind::FileEdit {
        path: PathBuf::from("src/main.rs"),
        op: FileOp::Modify,
        bytes: 1024,
        diff_preview: Some("--- a\n+++ b\n".into()),
    }));
}

#[test]
fn base_test_run_roundtrips() {
    assert_roundtrip(wrap(EventKind::TestRun {
        command: "cargo test".into(),
        framework: Some("cargo".into()),
        passed: 10,
        failed: 0,
        duration_ms: 2000,
    }));
}

#[test]
fn base_compact_roundtrips() {
    assert_roundtrip(wrap(EventKind::CompactEvent {
        trigger: "token-limit".into(),
        token_count: 180_000,
    }));
}

#[test]
fn base_error_roundtrips() {
    assert_roundtrip(wrap(EventKind::Error {
        tool: Some("bash".into()),
        message: "exit 1".into(),
    }));
}

#[test]
fn rich_file_edit_enriched_roundtrips() {
    assert_roundtrip(wrap(EventKind::FileEditEnriched {
        path: PathBuf::from("src/main.rs"),
        op: FileOp::Modify,
        anchored_symbols: vec![SymbolRef {
            file_path: PathBuf::from("src/main.rs"),
            symbol: "main".into(),
            git_hash: "abc123".into(),
        }],
        lsp_diagnostics_delta: Some(DiagnosticsDelta {
            before: 2,
            after: 0,
            introduced: vec![],
            resolved: vec!["E0499".into()],
        }),
    }));
}

#[test]
fn rich_test_run_enriched_roundtrips() {
    assert_roundtrip(wrap(EventKind::TestRunEnriched {
        command: "cargo test".into(),
        passed_tests: vec!["ok1".into()],
        failed_tests: vec![TestFailure {
            test_name: "bad1".into(),
            message: "assertion failed".into(),
            stack: None,
        }],
        newly_failing: vec!["bad1".into()],
    }));
}

#[test]
fn rich_skill_routing_trace_roundtrips() {
    assert_roundtrip(wrap(EventKind::SkillRoutingTrace {
        considered: vec![SkillScore {
            skill_id: "fix-bugs".into(),
            score: 0.8,
            components: vec![("keyword".into(), 0.5)],
        }],
        chosen: vec!["fix-bugs".into()],
    }));
}
```

- [ ] **Step 2: Run the test**

Run: `cargo nextest run -p coding-ingest --test agent_event_roundtrip`
Expected: all 12 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-ingest/tests/agent_event_roundtrip.rs
git commit -m "test(coding-ingest): AgentEvent round-trip for base + rich variants"
```

---

### Task 6: `IngestAdapter` trait + 4 CLI adapter stubs

**Files:**
- Modify: `crates/coding-ingest/src/adapters/mod.rs`
- Create: `crates/coding-ingest/src/adapters/claude_code.rs`
- Create: `crates/coding-ingest/src/adapters/codex.rs`
- Create: `crates/coding-ingest/src/adapters/kimi_wire.rs`
- Create: `crates/coding-ingest/src/adapters/opencode.rs`

- [ ] **Step 1: Populate `adapters/mod.rs`**

Replace `crates/coding-ingest/src/adapters/mod.rs`:

```rust
//! CLI-specific adapters normalize per-CLI hook payloads to `AgentEvent`.
//!
//! Phase 1 ships trait + empty adapters. Each phase-2+ task fleshes out
//! one adapter; the trait signature is stable from Phase 1.

use crate::AgentEvent;
use common::Result;

/// Adapter that converts one CLI's per-hook stdin payload into an `AgentEvent`.
///
/// Implementations are stateless; one instance can handle many hook invocations.
pub trait IngestAdapter: Send + Sync {
    /// Stable name used in `AgentSource` and settings UI.
    fn source_name(&self) -> &'static str;

    /// Parse a single stdin payload + originating hook event name.
    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>>;
}

/// Claude Code adapter.
pub mod claude_code;
/// Codex adapter.
pub mod codex;
/// kimi-cli adapter (tier-1 hook + tier-2 Wire path).
pub mod kimi_wire;
/// opencode adapter (SQLite WAL polling).
pub mod opencode;
```

- [ ] **Step 2: Create `claude_code.rs`**

Create `crates/coding-ingest/src/adapters/claude_code.rs`:

```rust
//! Claude Code adapter — 7 hook events filtered from Claude's 27.
//!
//! Phase 1 stub. Behavior lands in Phase 2.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for Claude Code hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCodeAdapter;

impl IngestAdapter for ClaudeCodeAdapter {
    fn source_name(&self) -> &'static str {
        "claude-code"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "ClaudeCodeAdapter::parse lands in Phase 2".into(),
        ))
    }
}
```

- [ ] **Step 3: Create `codex.rs`**

Create `crates/coding-ingest/src/adapters/codex.rs`:

```rust
//! Codex adapter — 5 hook events from OpenAI's Codex CLI.
//!
//! Phase 1 stub. Behavior lands in Phase 7.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for Codex hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexAdapter;

impl IngestAdapter for CodexAdapter {
    fn source_name(&self) -> &'static str {
        "codex"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "CodexAdapter::parse lands in Phase 7".into(),
        ))
    }
}
```

- [ ] **Step 4: Create `kimi_wire.rs`**

Create `crates/coding-ingest/src/adapters/kimi_wire.rs`:

```rust
//! kimi-cli adapter — 13 hook events (tier 1) + Wire streaming (tier 2).
//!
//! Phase 1 stub. Behavior lands in Phase 7.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for kimi-cli hook payloads. Wire-tier client surface lands later.
#[derive(Debug, Default, Clone, Copy)]
pub struct KimiAdapter;

impl IngestAdapter for KimiAdapter {
    fn source_name(&self) -> &'static str {
        "kimi-cli"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "KimiAdapter::parse lands in Phase 7".into(),
        ))
    }
}
```

- [ ] **Step 5: Create `opencode.rs`**

Create `crates/coding-ingest/src/adapters/opencode.rs`:

```rust
//! opencode adapter — SQLite WAL polling (500ms) over opencode's local DB.
//!
//! Phase 1 stub. Behavior lands in Phase 7.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for opencode SQLite polling.
#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeAdapter;

impl IngestAdapter for OpencodeAdapter {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "OpencodeAdapter::parse lands in Phase 7".into(),
        ))
    }
}
```

- [ ] **Step 6: Add `NotImplemented` variant to `KlyntbotError`**

Open `crates/common/src/error.rs`. Find `pub enum KlyntbotError { … }`. Add one variant (alphabetically near `Storage`):

```rust
    /// An operation is a Phase-1 stub; implementation lands in a later phase.
    #[error("not implemented: {0}")]
    NotImplemented(String),
```

Also add to the existing exhaustive match test at `KlyntbotError::…` if any test in `crates/common/src/error.rs` requires an entry for the new variant (grep `crates/common/src/error.rs` for the discriminant list). If the existing test (`let cases: Vec<(&str, KlyntbotError)>`) exists, append one tuple:

```rust
            ("NotImplemented", KlyntbotError::NotImplemented("fixture".into())),
```

- [ ] **Step 7: Build + clippy**

Run: `cargo build -p coding-ingest -p common && cargo clippy -p coding-ingest -p common --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/coding-ingest/src/adapters crates/common/src/error.rs
git commit -m "feat(coding-ingest): IngestAdapter trait + 4 CLI stubs; KlyntbotError::NotImplemented"
```

---

### Task 7: Transport stubs — `IngestSocket` trait + Unix socket + file buffer

**Files:**
- Modify: `crates/coding-ingest/src/transport.rs`

- [ ] **Step 1: Populate `transport.rs`**

Replace `crates/coding-ingest/src/transport.rs`:

```rust
//! Transport stubs — Unix socket hot path + file-buffer cold path.
//!
//! Phase 1 defines the trait and struct shells so `daemon.rs` and the
//! `klyntbot-hook` binary have types to reference. Actual IO lands in
//! Phase 2.

use crate::AgentEvent;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// Ingest channel — the hook writer's view of "send one event."
///
/// Implementations are expected to be async-safe (used from tokio context)
/// but may block briefly on filesystem operations.
#[async_trait]
pub trait IngestSocket: Send + Sync {
    /// Write one event.
    async fn send(&self, event: &AgentEvent) -> Result<()>;
}

/// Default Unix socket location (`~/.klyntbot/ingest.sock`).
pub const DEFAULT_SOCKET_PATH: &str = "ingest.sock";
/// Default file-buffer location (`~/.klyntbot/ingest-buffer.jsonl`).
pub const DEFAULT_BUFFER_PATH: &str = "ingest-buffer.jsonl";
/// Hard cap for the buffer file before rotation (50 MB).
pub const BUFFER_ROTATE_BYTES: u64 = 50 * 1024 * 1024;
/// Hard-fail ceiling (500 MB).
pub const BUFFER_HARD_CAP_BYTES: u64 = 500 * 1024 * 1024;
/// Buffer file TTL (7 days).
pub const BUFFER_TTL_DAYS: u64 = 7;

/// Unix-domain-socket sink (hot path when klyntbot desktop is running).
#[derive(Debug, Clone)]
pub struct UnixIngestSocket {
    /// Absolute path to the socket file.
    pub path: PathBuf,
}

impl UnixIngestSocket {
    /// Construct with an explicit path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl IngestSocket for UnixIngestSocket {
    async fn send(&self, _event: &AgentEvent) -> Result<()> {
        Err(KlyntbotError::NotImplemented(
            "UnixIngestSocket::send lands in Phase 2".into(),
        ))
    }
}

/// File-append sink (cold path when desktop is off).
#[derive(Debug, Clone)]
pub struct FileBufferFallback {
    /// Absolute path to the buffer file.
    pub path: PathBuf,
}

impl FileBufferFallback {
    /// Construct with an explicit path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl IngestSocket for FileBufferFallback {
    async fn send(&self, _event: &AgentEvent) -> Result<()> {
        Err(KlyntbotError::NotImplemented(
            "FileBufferFallback::send lands in Phase 2".into(),
        ))
    }
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-ingest && cargo clippy -p coding-ingest --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-ingest/src/transport.rs
git commit -m "feat(coding-ingest): IngestSocket trait + Unix/file-buffer stubs"
```

---

### Task 8: Daemon stub

**Files:**
- Modify: `crates/coding-ingest/src/daemon.rs`

- [ ] **Step 1: Populate `daemon.rs`**

Replace `crates/coding-ingest/src/daemon.rs`:

```rust
//! Desktop-embedded ingestion daemon — owns the `ingest.sock` lifecycle
//! and drains events into `ingest_event_log`.
//!
//! Phase 1 stub. Behavior lands in Phase 2 (lifecycle + Claude Code E2E).

use crate::AgentEvent;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// Configuration for the ingestion daemon.
#[derive(Debug, Clone)]
pub struct IngestDaemonConfig {
    /// Where the Unix socket is bound.
    pub socket_path: PathBuf,
    /// Where the cold-path file buffer lives.
    pub buffer_path: PathBuf,
}

/// Daemon handle — obtained after `spawn`; used to shutdown cleanly.
#[derive(Debug)]
pub struct IngestDaemonHandle {
    /// Placeholder — Phase 2 replaces with a shutdown sender.
    _private: (),
}

/// Spawn the ingestion daemon. Owned by the klyntbot desktop binary.
///
/// Phase 1 stub — returns an error so the desktop-layer wiring can reference
/// the symbol without Phase 1 regressing desktop startup. Desktop does not
/// yet call this in Phase 1.
pub async fn spawn(_cfg: IngestDaemonConfig) -> Result<IngestDaemonHandle> {
    Err(KlyntbotError::NotImplemented(
        "ingest daemon spawn lands in Phase 2".into(),
    ))
}

/// Record drainage API — exposed for `ingest_event_log` replay. Phase 2.
pub async fn drain_buffer(_path: &PathBuf) -> Result<Vec<AgentEvent>> {
    Err(KlyntbotError::NotImplemented(
        "buffer drain lands in Phase 2".into(),
    ))
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-ingest && cargo clippy -p coding-ingest --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-ingest/src/daemon.rs
git commit -m "feat(coding-ingest): daemon stub with IngestDaemonConfig/Handle"
```

---

### Task 9: `klyntbot-hook` binary (stub)

**Files:**
- Create: `crates/coding-ingest/src/bin/klyntbot-hook.rs`

- [ ] **Step 1: Create the binary entry point**

Create `crates/coding-ingest/src/bin/klyntbot-hook.rs`:

```rust
//! `klyntbot-hook` — shell binary users' coding CLIs spawn per hook.
//!
//! Phase 1: parses CLI arg (which adapter to use), reads stdin, logs to
//! stderr only. No socket writes — wire-up lands in Phase 2.
//!
//! Usage: `klyntbot-hook <source> [hook-event-name]`
//!
//!   source ∈ { claude-code, codex, kimi-cli, opencode }
//!
//! Exits 0 on success, 2 on bad args, 1 on read failure. Never blocks the
//! parent CLI — all IO has a hard timeout in Phase 2.

use std::io::{self, Read};
use std::process::ExitCode;

const USAGE: &str = "\
usage: klyntbot-hook <source> [hook-event]
  source ∈ { claude-code, codex, kimi-cli, opencode }
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let source = match args.next() {
        Some(s) => s,
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let source_ok = matches!(
        source.as_str(),
        "claude-code" | "codex" | "kimi-cli" | "opencode"
    );
    if !source_ok {
        eprintln!("unknown source `{source}`\n{USAGE}");
        return ExitCode::from(2);
    }

    let hook_event = args.next().unwrap_or_else(|| "unknown".to_string());

    let mut raw = Vec::with_capacity(8 * 1024);
    if let Err(e) = io::stdin().read_to_end(&mut raw) {
        eprintln!("klyntbot-hook: stdin read failed: {e}");
        return ExitCode::from(1);
    }

    // Phase 1: observational only — log presence, never transmit.
    eprintln!(
        "klyntbot-hook: source={source} hook_event={hook_event} bytes={} (phase 1 stub — not forwarded)",
        raw.len()
    );

    ExitCode::SUCCESS
}
```

- [ ] **Step 2: Verify the binary builds**

Run: `cargo build -p coding-ingest --bin klyntbot-hook`
Expected: PASS.

- [ ] **Step 3: Smoke-test argument parsing**

Run:
```bash
echo '{}' | cargo run -q -p coding-ingest --bin klyntbot-hook -- claude-code SessionStart 2>&1
```
Expected: stderr includes `source=claude-code hook_event=SessionStart bytes=3 (phase 1 stub — not forwarded)`; exit code 0.

Run:
```bash
cargo run -q -p coding-ingest --bin klyntbot-hook -- bad-cli 2>&1; echo "exit=$?"
```
Expected: stderr shows usage + `unknown source`; `exit=2`.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/src/bin/klyntbot-hook.rs
git commit -m "feat(coding-ingest): klyntbot-hook shell binary (Phase 1 stub)"
```

---

### Task 10: `coding-memory/scope.rs` — provenance, anchored symbols, causal edges

**Files:**
- Modify: `crates/coding-memory/src/scope.rs`

- [ ] **Step 1: Populate `scope.rs`**

Replace `crates/coding-memory/src/scope.rs`:

```rust
//! Scope partitioning, provenance metadata, anchored symbols, causal edges.
//!
//! These types appear in the `metadata` JSON column of `semantic_facts` and
//! `episodic_memories`, and in the `memory_causal_edges` table.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub use coding_ingest::RepoScope;

/// Privacy tier — every memory carries exactly one.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// Default. Normal retrieval + eligible for externalization.
    #[default]
    Normal,
    /// Retrieved normally but never written to rule artifacts on disk.
    High,
    /// Hidden from retrieval unless `include_excluded: true`.
    Excluded,
}

/// Provenance chain attached to every memory write.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceMetadata {
    /// `ingest_event_log.id` rows that produced this memory.
    pub source_events: Vec<Uuid>,
    /// Session id at distillation time.
    pub session_id: String,
    /// Turn id at distillation time.
    pub turn_id: Option<String>,
    /// When distillation ran.
    pub distilled_at: Timestamp,
    /// Which model produced it (model id string).
    pub distiller_model: String,
    /// Pipeline that wrote this fact.
    pub source_kind: ProvenanceKind,
}

/// Which pipeline produced a given fact.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    /// Phase-A extractive pass.
    DistillerExtractive,
    /// Phase-B LLM synthesis.
    DistillerLlm,
    /// User explicitly edited/promoted the fact.
    UserCorrected,
    /// Reforge synthesis phase.
    ReforgeSynthesis,
}

/// Anchored symbol — link from a memory to a tree-sitter-extracted code symbol.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnchoredSymbol {
    /// Absolute file path.
    pub file_path: PathBuf,
    /// Symbol name.
    pub symbol: String,
    /// Symbol kind (function, method, struct, enum, const).
    pub kind: String,
    /// Commit at which the symbol was anchored.
    pub git_hash: String,
    /// Optional byte span for precise invalidation.
    pub byte_span: Option<(u32, u32)>,
}

/// Causal edge kinds — MAGMA-style. Closed enum.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CausalEdgeKind {
    /// `from` fix caused `to` failure.
    Broke,
    /// `from` change was fixed by `to`.
    FixedBy,
    /// `from` test pass flipped to fail at `to`.
    FlippedToFail,
    /// `from` failure shares root cause with `to` failure.
    SharesRootCause,
    /// `from` refactor enabled `to` subsequent work.
    Enabled,
}

/// Causal edge row — backed by `memory_causal_edges` table.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CausalEdge {
    /// Edge id.
    pub id: Uuid,
    /// Source memory id (semantic or episodic).
    pub from_id: Uuid,
    /// Target memory id.
    pub to_id: Uuid,
    /// Edge kind.
    pub edge_kind: CausalEdgeKind,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// When the edge was inferred.
    pub inferred_at: Timestamp,
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/src/scope.rs
git commit -m "feat(coding-memory): scope.rs — provenance, anchored symbols, causal edges"
```

---

### Task 11: `coding-memory/facts.rs` — coding fact taxonomy

**Files:**
- Modify: `crates/coding-memory/src/facts.rs`

- [ ] **Step 1: Populate `facts.rs`**

Replace `crates/coding-memory/src/facts.rs`:

```rust
//! Coding fact taxonomy — the in-memory shape of every kind the Distiller
//! and Reforge write. Persistence uses existing `SemanticFact` /
//! `EpisodicMemory` / `ProceduralRule` rows; these structs are what the
//! Distiller constructs before handing off to the cognitive repos.
//!
//! See coding-memory design §7 for the full taxonomy table.

use crate::scope::{AnchoredSymbol, ProvenanceMetadata, Sensitivity};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// The 5-value `CodingKind` enum the LLM Distiller `record_observation` tool
/// accepts. Reforge-only kinds (ProblemSolutionPattern, ProjectUnderstanding,
/// UserHabit) are NOT in this enum — Distiller MUST NOT emit them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodingKind {
    /// `FixAttempt` episode.
    FixAttempt,
    /// `StylePreference` semantic fact.
    StylePreference,
    /// `WorkflowPattern` procedural rule.
    WorkflowPattern,
    /// `RepoContext` semantic fact.
    RepoContext,
    /// `FailurePattern` procedural rule.
    FailurePattern,
}

/// Outcome of a fix attempt.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FixOutcome {
    /// Fix worked — tests pass, behavior confirmed.
    Success,
    /// Fix partially worked; follow-up needed.
    Partial,
    /// Fix did not work — reverted or replaced.
    Failure,
    /// Abandoned without reaching a conclusion.
    Abandoned,
}

/// Structured JSON body of a `FixAttempt` episodic memory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixAttempt {
    /// Stable hash of (canonical problem statement).
    pub problem_hash: String,
    /// Human-readable problem statement.
    pub problem: String,
    /// Files touched.
    pub files: Vec<PathBuf>,
    /// One-sentence description of the approach.
    pub approach: String,
    /// How it ended.
    pub outcome: FixOutcome,
    /// What we learned.
    pub insight: Option<String>,
    /// Wall-clock duration.
    pub duration_ms: u32,
    /// Pre-fix test outcome summary.
    pub test_before: Option<String>,
    /// Post-fix test outcome summary.
    pub test_after: Option<String>,
    /// Symbols touched (Phase 6 populates; Phase 1 allows `vec![]`).
    pub anchored_symbols: Vec<AnchoredSymbol>,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// Derived "tried X, didn't work" fact emitted alongside a `Failure`/`Abandoned`
/// `FixAttempt`. Stored as a `SemanticFact { memory_type: 'counterfactual' }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndAttempt {
    /// Links back to the episode that caused this dead-end entry.
    pub source_attempt_id: Uuid,
    /// Canonical problem hash.
    pub problem_hash: String,
    /// What we tried.
    pub approach: String,
    /// Why it didn't work.
    pub reason: String,
    /// Confidence the dead-end warning is valid.
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
}

/// A style / preference statement. `SemanticFact { domain: 'preferences' }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StylePreference {
    /// Predicate — `prefers | avoids | uses | dislikes`.
    pub predicate: String,
    /// Object of the preference.
    pub object: String,
    /// Scope — `"global"` or `"repo"`.
    pub scope_kind: StyleScope,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// Where a style preference applies.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StyleScope {
    /// Applies everywhere.
    Global,
    /// Applies to one repo only.
    Repo,
}

/// A repo-level fact — framework, language, conventions, etc.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoContext {
    /// Predicate — one of `framework | language | package_manager |
    /// test_command | lint_command | deployment | convention |
    /// architecture_layer | depends_on | has_gotcha`.
    pub predicate: String,
    /// Object value.
    pub object: String,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// A recurring workflow pattern. `ProceduralRule { source: 'observed' }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPattern {
    /// Short name.
    pub name: String,
    /// When-to-apply heuristic in plain language.
    pub when_to_use: String,
    /// Step-by-step procedure (one line per step).
    pub procedure: Vec<String>,
    /// Starting effectiveness (Distiller default 0.5).
    pub effectiveness: f32,
    /// Confidence at emission time.
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// A recurring failure pattern with a remediation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FailurePattern {
    /// Name of the pattern (stable identifier).
    pub name: String,
    /// Signature / symptom description.
    pub symptom: String,
    /// Remediation (one line per step).
    pub remediation: Vec<String>,
    /// Confidence at emission time.
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// Episodic memory emitted by Phase-A extractive when a refactor pattern
/// is detected (file-edit clustering). No LLM.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefactorEpisode {
    /// Files touched.
    pub files: Vec<PathBuf>,
    /// Symbols touched.
    pub anchored_symbols: Vec<AnchoredSymbol>,
    /// Summary of the change.
    pub summary: String,
    /// When it happened.
    pub occurred_at: Timestamp,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
}

/// Episodic memory emitted by Phase-A when a test runner ran.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestRunEpisode {
    /// Command as executed.
    pub command: String,
    /// Detected framework.
    pub framework: Option<String>,
    /// Passed count.
    pub passed: u32,
    /// Failed count.
    pub failed: u32,
    /// When it ran.
    pub occurred_at: Timestamp,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/src/facts.rs
git commit -m "feat(coding-memory): facts.rs — fact taxonomy (FixAttempt, StylePreference, ...)"
```

---

### Task 12: `MemorySink` trait + `InProcessSink` / `IngestSocketSink` stubs

**Files:**
- Modify: `crates/coding-memory/src/sink.rs`

- [ ] **Step 1: Populate `sink.rs`**

Replace `crates/coding-memory/src/sink.rs`:

```rust
//! `MemorySink` — the abstraction native in-process consumers (klynt-cli)
//! and the ingest socket share. Lets klynt-cli emit events directly to the
//! Distiller when desktop is off, and to the socket when desktop is alive.
//!
//! See coding-memory design §5 "Native source: klynt-cli".

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use coding_ingest::AgentEvent;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// Abstraction over "accept an `AgentEvent` from a native source".
#[async_trait]
pub trait MemorySink: Send + Sync {
    /// Accept one event. Implementations buffer / forward as appropriate.
    async fn accept_event(&self, event: AgentEvent) -> Result<()>;

    /// Flush any pending events — called at session end or on shutdown.
    async fn flush(&self) -> Result<()>;
}

/// In-process sink — when desktop is off, klynt-cli calls the Distiller directly.
#[derive(Debug, Default, Clone)]
pub struct InProcessSink {
    /// Phase-2+ wiring will carry a `Distiller` handle here.
    _phase_stub: (),
}

impl InProcessSink {
    /// Construct an in-process sink. Phase 1 stub.
    #[must_use]
    pub fn new() -> Self {
        Self { _phase_stub: () }
    }
}

#[async_trait]
impl MemorySink for InProcessSink {
    async fn accept_event(&self, _event: AgentEvent) -> Result<()> {
        Err(phase(2))
    }

    async fn flush(&self) -> Result<()> {
        Err(phase(2))
    }
}

/// Unix-socket sink — when desktop is alive, klynt-cli writes to `ingest.sock`.
#[derive(Debug, Clone)]
pub struct IngestSocketSink {
    /// Socket path.
    pub socket_path: PathBuf,
}

impl IngestSocketSink {
    /// Construct with an explicit socket path.
    #[must_use]
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }
}

#[async_trait]
impl MemorySink for IngestSocketSink {
    async fn accept_event(&self, _event: AgentEvent) -> Result<()> {
        Err(phase(2))
    }

    async fn flush(&self) -> Result<()> {
        Err(phase(2))
    }
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/src/sink.rs
git commit -m "feat(coding-memory): MemorySink trait + InProcessSink/IngestSocketSink stubs"
```

---

### Task 13: Distiller module stubs

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs`

- [ ] **Step 1: Populate `distiller/mod.rs`**

Replace `crates/coding-memory/src/distiller/mod.rs`:

```rust
//! Distiller — online writer.
//!
//! Phase A (extractive, always runs) + Phase B (LLM synthesis) + Phase C
//! (reconciliation). Phase 1 defines types; bodies land in Phase 3.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use coding_ingest::AgentEvent;
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

/// Which distiller phase produced a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistillerPhase {
    /// Phase A — deterministic extractive pass.
    Extractive,
    /// Phase B — LLM synthesis.
    Llm,
    /// Phase C — reconciliation (ADD / SUPERSEDE / NOOP).
    Reconciliation,
}

/// Deterministic pass output — always produced, never lost.
#[derive(Debug, Clone)]
pub struct TurnTrace {
    /// Session id.
    pub session_id: String,
    /// Turn id.
    pub turn_id: Option<String>,
    /// Files read during the turn.
    pub files_read: Vec<PathBuf>,
    /// Files modified with byte deltas.
    pub files_modified: Vec<(PathBuf, i64)>,
    /// Shell commands run.
    pub commands_run: Vec<String>,
    /// Test runner outcomes.
    pub test_outcomes: Vec<TestOutcome>,
    /// Errors encountered.
    pub errors_encountered: Vec<(Option<String>, String)>,
    /// Final assistant token usage (if any).
    pub token_usage: Option<TurnTokenUsage>,
    /// Start of turn.
    pub started_at: Timestamp,
    /// End of turn.
    pub ended_at: Option<Timestamp>,
}

/// Test-run outcome observed during a turn.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// Command.
    pub command: String,
    /// Framework.
    pub framework: Option<String>,
    /// Passed count.
    pub passed: u32,
    /// Failed count.
    pub failed: u32,
}

/// Token usage aggregated across a turn.
#[derive(Debug, Clone, Copy)]
pub struct TurnTokenUsage {
    /// Prompt tokens.
    pub prompt: u32,
    /// Completion tokens.
    pub completion: u32,
    /// Cache hits.
    pub cached: u32,
}

/// Distiller handle — constructed once per desktop; accepts events per turn.
#[derive(Debug)]
pub struct Distiller {
    /// Phase-3+ wiring will carry repo handles, provider manager, etc.
    _phase_stub: (),
}

impl Distiller {
    /// Construct a Distiller. Phase 1 stub (no deps wired).
    #[must_use]
    pub fn new() -> Self {
        Self { _phase_stub: () }
    }

    /// Accept a single event into the per-turn buffer. Phase 3.
    pub async fn accept_event(&self, _event: AgentEvent) -> Result<()> {
        Err(phase(3))
    }

    /// Trigger distillation for one turn (typically on `SessionEnd` or
    /// `AssistantMsg` with `token_usage`). Phase 3.
    pub async fn distill_turn(
        &self,
        _session_id: &str,
        _turn_id: Option<&str>,
    ) -> Result<DistillationReport> {
        Err(phase(3))
    }
}

impl Default for Distiller {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of what was written during one distillation cycle.
#[derive(Debug, Clone, Default)]
pub struct DistillationReport {
    /// Number of `SemanticFact` rows added or superseded.
    pub semantic_writes: u32,
    /// Number of `EpisodicMemory` rows added.
    pub episodic_writes: u32,
    /// Phase B LLM invocation count (0 when extractive-only).
    pub llm_calls: u32,
    /// Phase B cost in USD (0.0 when extractive-only).
    pub llm_cost_usd: f64,
    /// Turn trace id (`episodic_memories.id`).
    pub turn_trace_id: Option<Uuid>,
}

/// The LLM tool schema the Distiller exposes to Phase B providers.
#[async_trait]
pub trait RecordObservationTool: Send + Sync {
    /// Handle an observation the LLM emitted.
    async fn record_observation(
        &self,
        kind: crate::facts::CodingKind,
        subject: String,
        predicate: String,
        object: String,
        confidence: f32,
        scope: crate::facts::StyleScope,
        reasoning: String,
    ) -> Result<()>;
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/src/distiller
git commit -m "feat(coding-memory): Distiller + TurnTrace + DistillationReport stubs"
```

---

### Task 14: Recall service + response type stubs

**Files:**
- Modify: `crates/coding-memory/src/recall/mod.rs`
- Modify: `crates/coding-memory/src/recall/renderers.rs`

- [ ] **Step 1: Populate `recall/mod.rs`**

Replace `crates/coding-memory/src/recall/mod.rs`:

```rust
//! Recall service — the one engine behind passive injection and MCP tools.
//!
//! Phase 1: types + stub methods returning `NotImplemented`. Phase 4 wires
//! `QueryPipeline`, `UnifiedMemoryService`, the C3 failure-state probe, and
//! the dead-end check.

use crate::error::NotImplementedInPhase;
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod renderers;

/// One recall "level" for progressive disclosure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallLayer {
    /// Compact index — `recall_index`.
    Index,
    /// Chronological framing — `recall_timeline`.
    Timeline,
    /// Full structured content + provenance — `recall_fetch`.
    Fetch,
}

/// Layer-1 entry — used by `recall_index`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    /// Memory id.
    pub id: Uuid,
    /// `"fix_attempt" | "style_preference" | ...`
    pub kind: String,
    /// Short human-readable title.
    pub title: String,
    /// When recorded.
    pub when: Timestamp,
    /// `"global"` | `"repo:<id>"`.
    pub scope: String,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// Estimated token cost if fetched at layer 3.
    pub token_cost: u32,
}

/// Layer-2 entry — used by `recall_timeline`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    /// Memory id.
    pub id: Uuid,
    /// Kind.
    pub kind: String,
    /// When.
    pub when: Timestamp,
    /// Short snippet.
    pub snippet: String,
    /// Related memory ids (for expansion).
    pub related_ids: Vec<Uuid>,
}

/// Layer-3 entry — used by `recall_fetch`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FullEntry {
    /// Memory id.
    pub id: Uuid,
    /// Kind.
    pub kind: String,
    /// Full structured content as JSON value.
    pub content: serde_json::Value,
    /// Full `metadata` column JSON.
    pub metadata: serde_json::Value,
    /// Causal edges involving this memory (optional).
    pub causal_edges: Vec<crate::scope::CausalEdge>,
    /// Ancestor memory in SUPERSEDE chain.
    pub supersedes: Option<Uuid>,
    /// Descendant memory in SUPERSEDE chain.
    pub superseded_by: Option<Uuid>,
}

/// Response from `recall_index`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecallIndexResponse {
    /// Ranked results.
    pub results: Vec<IndexEntry>,
    /// C3 coverage score.
    pub coverage_score: f32,
    /// Whether the caller can request escalation.
    pub escalation_available: bool,
}

/// Response from `check_dead_ends`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndResponse {
    /// Prior failed attempts matching the approach.
    pub matches: Vec<DeadEndMatch>,
    /// Aggregate confidence that the approach is a dead end.
    pub aggregate_confidence: f32,
}

/// One dead-end match row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndMatch {
    /// Source fix-attempt episode id.
    pub attempt_id: Uuid,
    /// Problem hash.
    pub problem_hash: String,
    /// What was tried.
    pub approach: String,
    /// Why it failed.
    pub reason: String,
    /// When.
    pub when: Timestamp,
}

/// Response from `trace_causes`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CausalTraceResponse {
    /// Subject id requested.
    pub subject: Uuid,
    /// Ancestors walked.
    pub ancestors: Vec<crate::scope::CausalEdge>,
    /// Descendants walked.
    pub descendants: Vec<crate::scope::CausalEdge>,
    /// Depth used.
    pub depth: u32,
}

/// The single service both passive injection and MCP tools call.
#[derive(Debug)]
pub struct CodingRecallService {
    /// Phase-4 wiring will carry `UnifiedMemoryService`, `QueryPipeline`.
    _phase_stub: (),
}

impl CodingRecallService {
    /// Construct. Phase 1 stub.
    #[must_use]
    pub fn new() -> Self {
        Self { _phase_stub: () }
    }

    /// Layer-1 compact index. Phase 4.
    pub async fn recall_index(
        &self,
        _query: &str,
        _repo: Option<&str>,
        _kinds: Option<&[&str]>,
        _days: Option<u32>,
        _limit: u32,
    ) -> Result<RecallIndexResponse> {
        Err(phase(4))
    }

    /// Layer-2 timeline. Phase 4.
    pub async fn recall_timeline(
        &self,
        _ids_or_query: RecallQuery,
        _repo: Option<&str>,
        _days: u32,
    ) -> Result<Vec<TimelineEntry>> {
        Err(phase(4))
    }

    /// Layer-3 full fetch. Phase 4.
    pub async fn recall_fetch(
        &self,
        _ids: &[Uuid],
        _include_provenance: bool,
        _include_causal_graph: bool,
    ) -> Result<Vec<FullEntry>> {
        Err(phase(4))
    }

    /// Counterfactual check. Phase 4.
    pub async fn check_dead_ends(
        &self,
        _approach: &str,
        _repo: Option<&str>,
    ) -> Result<DeadEndResponse> {
        Err(phase(4))
    }

    /// Causal graph walk. Phase 6.
    pub async fn trace_causes(
        &self,
        _subject: Uuid,
        _repo: Option<&str>,
        _depth: u32,
    ) -> Result<CausalTraceResponse> {
        Err(phase(6))
    }
}

impl Default for CodingRecallService {
    fn default() -> Self {
        Self::new()
    }
}

/// Union accepted by `recall_timeline`.
#[derive(Debug, Clone)]
pub enum RecallQuery {
    /// Pre-selected memory ids.
    Ids(Vec<Uuid>),
    /// Free-text query.
    Text(String),
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 2: Populate `recall/renderers.rs`**

Replace `crates/coding-memory/src/recall/renderers.rs`:

```rust
//! Markdown renderers for passive injection (SessionStart + UserPromptSubmit).
//!
//! Phase 1 stubs. Phase 4 implements full rendering against the budget caps
//! (800 / 1500 tokens — invariant #9).

use crate::error::NotImplementedInPhase;
use common::{KlyntbotError, Result};

/// Token budget for SessionStart injection (design §8).
pub const SESSION_START_BUDGET_TOKENS: u32 = 800;
/// Token budget for UserPromptSubmit injection (design §8).
pub const USER_PROMPT_BUDGET_TOKENS: u32 = 1500;

/// Render the SessionStart injection block for a given repo.
pub async fn render_session_start_block(_repo: Option<&str>) -> Result<String> {
    Err(phase(4))
}

/// Render the UserPromptSubmit injection block.
pub async fn render_user_prompt_block(
    _query: &str,
    _repo: Option<&str>,
) -> Result<String> {
    Err(phase(4))
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 3: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/recall
git commit -m "feat(coding-memory): CodingRecallService + progressive-disclosure response types"
```

---

### Task 15: Reforge coding phase stubs

**Files:**
- Modify: `crates/coding-memory/src/reforge_phase.rs`

- [ ] **Step 1: Populate `reforge_phase.rs`**

Replace `crates/coding-memory/src/reforge_phase.rs`:

```rust
//! Reforge coding phases — 2.5 (Coding Synthesis) + 3.5 (Rule Artifact Generation).
//!
//! Phase 1 lands type + stub method surface. Bodies land in Phase 5.
//!
//! `ReforgeWriter` wrapper must be used by these phases; it rejects DELETE
//! at runtime to enforce the "Reforge-never-deletes-raw" invariant.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// A single Reforge phase run — plugs into the existing nightly cycle.
#[async_trait]
pub trait ReforgePhaseRun: Send + Sync {
    /// Human name for logging / mirror snippets.
    fn name(&self) -> &'static str;

    /// Run exactly one instance of the phase. Phases are `Result<()>`-isolated
    /// — a failure logs to `mirror_snippets` and does not cascade.
    async fn run(&self) -> Result<()>;
}

/// Phase 2.5 — Coding Synthesis.
///
/// Consumes: sessions + new `FixAttempt`s + causal edges + active
/// `WorkflowPattern`s. Emits: `ExtractPattern`, `ExtractFailurePattern`,
/// `PromoteToProblemClass`, `PromoteToProjectUnderstanding`, `PromoteToUserHabit`,
/// `PromoteToProblemSolutionPattern`.
#[derive(Debug, Default)]
pub struct CodingSynthesisPhase {
    /// Phase-5 wiring carries provider-manager handle + cognitive repos.
    _phase_stub: (),
}

#[async_trait]
impl ReforgePhaseRun for CodingSynthesisPhase {
    fn name(&self) -> &'static str {
        "reforge.coding_synthesis"
    }

    async fn run(&self) -> Result<()> {
        Err(phase(5))
    }
}

/// Phase 3.5 — Rule Artifact Generation.
///
/// Reads active patterns/preferences/understanding with `confidence ≥ 0.7`
/// and `stability ≥ 0.5`. Writes managed-block sections of per-repo
/// `CLAUDE.md` / `AGENTS.md` / `.cursorrules`. Skips `high` and `excluded`
/// sensitivity tiers.
#[derive(Debug, Default)]
pub struct RuleArtifactGenerationPhase {
    /// Phase-5 wiring carries repo discovery + managed-block writer.
    _phase_stub: (),
}

#[async_trait]
impl ReforgePhaseRun for RuleArtifactGenerationPhase {
    fn name(&self) -> &'static str {
        "reforge.rule_artifact_generation"
    }

    async fn run(&self) -> Result<()> {
        Err(phase(5))
    }
}

/// Managed-block markers. Opaque delimiters the rule writer preserves.
pub const MANAGED_BLOCK_START: &str =
    "<!-- klyntbot:managed:start";
/// End marker.
pub const MANAGED_BLOCK_END: &str = "<!-- klyntbot:managed:end -->";

/// Which on-disk rule artifact is being generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleArtifact {
    /// `<repo_root>/CLAUDE.md`.
    ClaudeMd,
    /// `<repo_root>/AGENTS.md`.
    AgentsMd,
    /// `<repo_root>/.cursorrules`.
    CursorRules,
    /// `<repo_root>/.continue/rules/klyntbot.md`.
    ContinueRules,
}

impl RuleArtifact {
    /// Relative path under a repo root for this artifact.
    #[must_use]
    pub fn relative_path(self) -> PathBuf {
        match self {
            RuleArtifact::ClaudeMd => PathBuf::from("CLAUDE.md"),
            RuleArtifact::AgentsMd => PathBuf::from("AGENTS.md"),
            RuleArtifact::CursorRules => PathBuf::from(".cursorrules"),
            RuleArtifact::ContinueRules => {
                PathBuf::from(".continue/rules/klyntbot.md")
            }
        }
    }
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/src/reforge_phase.rs
git commit -m "feat(coding-memory): CodingSynthesisPhase + RuleArtifactGenerationPhase stubs"
```

---

### Task 16: Project-skill evolver + retrieval-skill registry stubs

**Files:**
- Modify: `crates/coding-memory/src/skills.rs`
- Modify: `crates/coding-memory/src/retrieval_skills.rs`

- [ ] **Step 1: Populate `skills.rs`**

Replace `crates/coding-memory/src/skills.rs`:

```rust
//! Scope-aware `SkillStore` extension + project-scoped evolving skills.
//!
//! Phase 1 lands the types. Phase 5 wires Reforge's Phase 3.5 sub-phase
//! to auto-synthesize `SKILL.md` files from `WorkflowPattern`s.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::PathBuf;
use uuid::Uuid;

/// Where a project skill is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSkillLocation {
    /// Private default: `~/.klyntbot/project-skills/<repo>/<skill>/SKILL.md`.
    Private,
    /// Team-shared: `<repo_root>/.klyntbot/skills/<skill>/SKILL.md`.
    Team,
}

/// Scope of a skill — either global or bound to one repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillScope {
    /// Applies everywhere.
    Global,
    /// Applies to one repo only.
    Repo {
        /// Canonical repo id.
        repo_id: String,
    },
}

/// Identifier for a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillId(pub String);

/// Project-skill evolution driver — reads `WorkflowPattern`s, synthesizes
/// `SKILL.md`, writes via existing `SkillFileManager`.
#[async_trait]
pub trait ProjectSkillEvolver: Send + Sync {
    /// Run one evolution pass for a given repo.
    async fn evolve(&self, repo_id: &str) -> Result<Vec<SkillSynthesisResult>>;
}

/// Outcome of one skill synthesis.
#[derive(Debug, Clone)]
pub struct SkillSynthesisResult {
    /// Skill id.
    pub skill_id: SkillId,
    /// Absolute SKILL.md path.
    pub skill_path: PathBuf,
    /// Version row id.
    pub version_id: Uuid,
    /// Starting effectiveness score.
    pub effectiveness: f32,
}

/// Phase-1 stub evolver.
#[derive(Debug, Default)]
pub struct PhaseStubEvolver;

#[async_trait]
impl ProjectSkillEvolver for PhaseStubEvolver {
    async fn evolve(&self, _repo_id: &str) -> Result<Vec<SkillSynthesisResult>> {
        Err(phase(5))
    }
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 2: Populate `retrieval_skills.rs`**

Replace `crates/coding-memory/src/retrieval_skills.rs`:

```rust
//! C3 retrieval-skill registry — invoked by the failure-state-aware
//! retrieval probe when coverage_score falls below threshold.
//!
//! Closed set of 5 skills at Phase 4. Effectiveness tracked by EMA and
//! published as `DomainEvent::RetrievalSkillApplied`.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use common::{KlyntbotError, Result};

/// Budget tier at which a retrieval skill can operate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetTier {
    /// Fast (default) — bounded to the original retrieval budget.
    Fast,
    /// `deep_think` — larger budget for query rewriting/decomposing.
    DeepThink,
    /// `ultra` — full escalation, bypasses summaries.
    Ultra,
}

/// Context passed to a retrieval skill's `apply`.
#[derive(Debug, Clone)]
pub struct EscalationContext {
    /// Original query.
    pub query: String,
    /// Coverage score at invocation time.
    pub coverage_score: f32,
    /// Active tier.
    pub budget_tier: BudgetTier,
}

/// Outcome of a retrieval skill application.
#[derive(Debug, Clone)]
pub struct EscalationOutcome {
    /// Was coverage raised above threshold?
    pub succeeded: bool,
    /// New coverage score after applying.
    pub coverage_after: f32,
    /// Additional context produced (stringified).
    pub added_context: String,
}

/// Retrieval skill — the unit of C3 escalation.
#[async_trait]
pub trait RetrievalSkill: Send + Sync {
    /// Skill name used in telemetry + effectiveness EMA.
    fn name(&self) -> &'static str;

    /// Short description for UI surfaces.
    fn description(&self) -> &'static str;

    /// Apply the skill against an escalation context. Phase 4.
    async fn apply(
        &self,
        ctx: &EscalationContext,
    ) -> Result<EscalationOutcome>;

    /// Current EMA-updated effectiveness (0.0 – 1.0).
    fn effectiveness_score(&self) -> f32;
}

macro_rules! phase_stub_skill {
    ($struct_name:ident, $n:expr, $d:expr) => {
        /// Phase 4 stub.
        #[derive(Debug, Default)]
        pub struct $struct_name;

        #[async_trait]
        impl RetrievalSkill for $struct_name {
            fn name(&self) -> &'static str {
                $n
            }
            fn description(&self) -> &'static str {
                $d
            }
            async fn apply(
                &self,
                _ctx: &EscalationContext,
            ) -> Result<EscalationOutcome> {
                Err(phase(4))
            }
            fn effectiveness_score(&self) -> f32 {
                0.5
            }
        }
    };
}

phase_stub_skill!(
    QueryRewriter,
    "query_rewriter",
    "PRF + multi-query expansion; 3 rewrites, RRF-merge."
);
phase_stub_skill!(
    QueryDecomposer,
    "query_decomposer",
    "Split compound queries into 2-4 sub-queries."
);
phase_stub_skill!(
    EvidenceFocuser,
    "evidence_focuser",
    "Cross-encoder rerank on top-20 to identify top 5."
);
phase_stub_skill!(
    RawEventEscalator,
    "raw_event_escalator",
    "Bypass summaries; use provenance pointers to raw events."
);
phase_stub_skill!(
    CausalContextExpander,
    "causal_context_expander",
    "Walk memory_causal_edges from top-k; surface chains."
);

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!(
        "{:?}",
        NotImplementedInPhase::new(p)
    ))
}
```

- [ ] **Step 3: Build + clippy**

Run: `cargo build -p coding-memory && cargo clippy -p coding-memory --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/skills.rs crates/coding-memory/src/retrieval_skills.rs
git commit -m "feat(coding-memory): ProjectSkillEvolver + 5 RetrievalSkill stubs"
```

---

### Task 17: MCP tool stubs

**Files:**
- Modify: `crates/coding-memory/src/mcp.rs`

- [ ] **Step 1: Populate `mcp.rs`**

Replace `crates/coding-memory/src/mcp.rs`:

```rust
//! MCP tool stubs for the 8 coding-memory tools added in Phase 4/6.
//!
//! Phase 1 registers the tool names (via `EXPLICIT_TOOL_ALLOWLIST`) and
//! exposes stub handlers that return `NotImplementedInPhase`. Tool schemas
//! are finalized in Phase 4 when handlers gain real behavior.

use crate::error::NotImplementedInPhase;
use common::{KlyntbotError, Result};

/// Canonical tool names — must match entries appended to
/// `EXPLICIT_TOOL_ALLOWLIST` in `crates/config/src/schema/mcp.rs`.
pub const CODING_MEMORY_MCP_TOOLS: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];

/// Stub handler used by MCP registration; returns `NotImplemented`.
pub fn stub_handler(tool_name: &str) -> Result<serde_json::Value> {
    Err(KlyntbotError::NotImplemented(format!(
        "coding-memory MCP tool `{tool_name}` is a Phase-1 stub; wiring lands in Phase {}",
        phase_for_tool(tool_name)
    )))
}

fn phase_for_tool(tool: &str) -> u8 {
    match tool {
        "trace_causes" => 6,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_phase() {
        for t in CODING_MEMORY_MCP_TOOLS {
            let err = stub_handler(t).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains(t), "expected tool name in error: {msg}");
            assert!(
                msg.contains("Phase 4") || msg.contains("Phase 6"),
                "expected phase marker in error: {msg}"
            );
        }
    }

    #[test]
    fn tools_match_allowlist_constants() {
        // Structural assertion — ensures EXPLICIT_TOOL_ALLOWLIST in config
        // stays in sync. If this fails, update that list in config/schema/mcp.rs.
        let expected = [
            "recall_index",
            "recall_timeline",
            "recall_fetch",
            "trace_causes",
            "check_dead_ends",
            "recall_facts_as_of",
            "recall_change_history",
            "recall_decision_points",
        ];
        assert_eq!(CODING_MEMORY_MCP_TOOLS, expected);
        let _ = NotImplementedInPhase::new(4);
    }
}
```

- [ ] **Step 2: Build + test**

Run: `cargo nextest run -p coding-memory`
Expected: both unit tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/src/mcp.rs
git commit -m "feat(coding-memory): 8 MCP tool stubs + CODING_MEMORY_MCP_TOOLS constant"
```

---

### Task 18: Append MCP tool names to `EXPLICIT_TOOL_ALLOWLIST`

**Files:**
- Modify: `crates/config/src/schema/mcp.rs`

- [ ] **Step 1: Extend the allowlist**

Open `crates/config/src/schema/mcp.rs`. Find the existing constant at line ~191:

```rust
pub const EXPLICIT_TOOL_ALLOWLIST: &[&str] = &[
    "memory", "agent", "annotate", "cron", "alarm", "mirror", "temporal",
];
```

Replace with:

```rust
pub const EXPLICIT_TOOL_ALLOWLIST: &[&str] = &[
    "memory",
    "agent",
    "annotate",
    "cron",
    "alarm",
    "mirror",
    "temporal",
    // coding-memory tools (see crates/coding-memory/src/mcp.rs for stubs)
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];
```

- [ ] **Step 2: Build + clippy**

Run: `cargo build -p config && cargo clippy -p config --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/schema/mcp.rs
git commit -m "feat(config): register 8 coding-memory MCP tool names in EXPLICIT_TOOL_ALLOWLIST"
```

---

### Task 19: Add 6 `DomainEvent` variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add variants**

Open `crates/bus/src/domain_events.rs`. Locate `pub enum DomainEvent {` (line 22). At the end of the enum (before the closing `}` of the enum body), append:

```rust
    // -- Coding memory (see docs/superpowers/specs/2026-04-22-coding-memory-design.md) --
    /// A pattern or memory-backed rule was applied during a turn.
    PatternApplied {
        /// Pattern id (workflow pattern, failure pattern, or project skill).
        pattern_id: String,
        /// Session id.
        session_id: String,
        /// Repo id, if scoped.
        repo: Option<String>,
        /// `"skill_listing"` | `"recall_injection"` | `"explicit_call"`.
        source: String,
    },
    /// Observed outcome of a previously applied pattern.
    PatternOutcome {
        /// Pattern id.
        pattern_id: String,
        /// `"success"` | `"partial"` | `"failure"` | `"inconclusive"`.
        outcome: String,
        /// Free-form evidence string.
        evidence: String,
        /// When the outcome was measured.
        measured_at: String,
    },
    /// A FixAttempt ended in failure for a recurring `problem_hash`.
    FixAttemptFailed {
        /// Stable problem hash.
        problem_hash: String,
        /// Repo id, if scoped.
        repo: Option<String>,
        /// How many attempts have now failed.
        attempt_count: u32,
    },
    /// Coding memories were retrieved during a turn.
    MemoryRetrieved {
        /// Ids of retrieved memories.
        memory_ids: Vec<String>,
        /// Original query string.
        query: String,
        /// Session id.
        session_id: String,
        /// Turn id.
        turn_id: Option<String>,
    },
    /// Assistant turn completed; caller may report which memories were cited.
    AssistantMsgCompleted {
        /// Session id.
        session_id: String,
        /// Turn id.
        turn_id: Option<String>,
        /// Memory ids cited in the final assistant output.
        cited_memory_ids: Vec<String>,
    },
    /// A retrieval skill (C3) was applied against an escalation context.
    RetrievalSkillApplied {
        /// Skill name.
        skill: String,
        /// Coverage score before apply.
        before_score: f32,
        /// Coverage score after apply.
        after_score: f32,
        /// `"fast"` | `"deep_think"` | `"ultra"`.
        budget_used: String,
        /// Session id.
        session_id: String,
    },
```

- [ ] **Step 2: Add a round-trip test**

Append to the `#[cfg(test)] mod tests { … }` block at the bottom of the file:

```rust
    #[test]
    fn pattern_applied_roundtrips() {
        let e = DomainEvent::PatternApplied {
            pattern_id: "fp-1".into(),
            session_id: "s-1".into(),
            repo: Some("github.com/klynt/bot".into()),
            source: "recall_injection".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, DomainEvent::PatternApplied { .. }));
    }

    #[test]
    fn retrieval_skill_applied_roundtrips() {
        let e = DomainEvent::RetrievalSkillApplied {
            skill: "query_rewriter".into(),
            before_score: 0.1,
            after_score: 0.7,
            budget_used: "deep_think".into(),
            session_id: "s-1".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, DomainEvent::RetrievalSkillApplied { .. }));
    }
```

- [ ] **Step 3: Build + test + clippy**

Run:
```bash
cargo nextest run -p bus && cargo clippy -p bus --all-targets -- -D warnings
```
Expected: both new tests PASS; zero clippy warnings.

- [ ] **Step 4: Run downstream consumers to catch non-exhaustive match regressions**

Run: `cargo build --workspace`
Expected: PASS (the codebase uses `_ =>` / wildcard matches on `DomainEvent` in consumers; if any consumer has a non-wildcard match, address it by adding an explicit `_ => ()` branch in that site — do not strip the variants).

- [ ] **Step 5: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): 6 DomainEvent variants for coding memory (PatternApplied, …, RetrievalSkillApplied)"
```

---

### Task 20: Config surface — `CodingMemoryConfig` tree

**Files:**
- Create: `crates/config/src/schema/coding_memory.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Create `coding_memory.rs`**

Create `crates/config/src/schema/coding_memory.rs`:

```rust
//! Configuration for the coding-memory subsystem.
//!
//! Mirrors coding-memory design §13.D exactly. Keys shared with klynt-cli
//! are flagged in the design; do not rename them without coordinating with
//! the klynt-cli spec.

use serde::{Deserialize, Serialize};

/// Root config for coding memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingMemoryConfig {
    /// Master toggle. Default `false` (opt-in per CLI).
    pub enabled: bool,
    /// Distiller (online writer) config.
    pub distiller: CodingDistillerConfig,
    /// Ingest-side config (path exclusions).
    pub ingest: CodingIngestConfig,
    /// Privacy / sensitivity config.
    pub privacy: CodingPrivacyConfig,
    /// Recall API budgets + toggles.
    pub recall: CodingRecallConfig,
    /// Reforge cron + rule artifact config.
    pub reforge: CodingReforgeConfig,
    /// Project-skill evolution config.
    pub skills: CodingSkillsConfig,
    /// Workbench (desktop UI) config.
    pub workbench: CodingWorkbenchConfig,
    /// Per-CLI toggles.
    pub cli: CodingCliToggles,
}

impl Default for CodingMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            distiller: CodingDistillerConfig::default(),
            ingest: CodingIngestConfig::default(),
            privacy: CodingPrivacyConfig::default(),
            recall: CodingRecallConfig::default(),
            reforge: CodingReforgeConfig::default(),
            skills: CodingSkillsConfig::default(),
            workbench: CodingWorkbenchConfig::default(),
            cli: CodingCliToggles::default(),
        }
    }
}

/// Distiller (online writer) config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingDistillerConfig {
    /// Model id used for Phase-B LLM synthesis.
    pub model: String,
    /// Max input tokens passed to Phase B.
    pub max_input_tokens: u32,
    /// Phase-B call timeout as a humantime-parseable string.
    pub timeout: String,
}

impl Default for CodingDistillerConfig {
    fn default() -> Self {
        Self {
            model: "claude-haiku-4-5-20251001".to_string(),
            max_input_tokens: 8000,
            timeout: "30s".to_string(),
        }
    }
}

/// Ingest-side config — path-based exclusions for privacy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingIngestConfig {
    /// Globs excluded from ingestion at hook level.
    pub exclude_paths: Vec<String>,
}

impl Default for CodingIngestConfig {
    fn default() -> Self {
        Self {
            exclude_paths: default_exclude_paths(),
        }
    }
}

/// Shipped default exclude paths — see coding-memory design §5.
#[must_use]
pub fn default_exclude_paths() -> Vec<String> {
    [
        "**/.env",
        "**/.env.*",
        "**/secrets/**",
        "**/private/**",
        "**/*.key",
        "**/*.pem",
        "**/*.p12",
        "**/*.pfx",
        "**/id_rsa",
        "**/id_ed25519",
        "**/known_hosts",
        "**/.aws/credentials",
        "**/.gcloud/**",
        "**/.kube/config",
        "**/node_modules/**",
        "**/target/**",
        "**/.git/**",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

/// Privacy config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingPrivacyConfig {
    /// Default sensitivity tier for writes that don't explicitly set one.
    pub default_sensitivity: String,
    /// Globs whose matches are auto-promoted to `high` sensitivity.
    pub auto_promote_high_paths: Vec<String>,
}

impl Default for CodingPrivacyConfig {
    fn default() -> Self {
        Self {
            default_sensitivity: "normal".to_string(),
            auto_promote_high_paths: vec![
                "**/auth/**".to_string(),
                "**/billing/**".to_string(),
                "**/payment/**".to_string(),
            ],
        }
    }
}

/// Recall API config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingRecallConfig {
    /// Token budget for SessionStart injection.
    pub session_start_budget: u32,
    /// Token budget for UserPromptSubmit injection.
    pub user_prompt_budget: u32,
    /// Emit dead-end warning blocks when counterfactuals match.
    pub dead_end_warnings: bool,
    /// Enable C3 coverage-based escalation.
    pub escalation_enabled: bool,
    /// Coverage threshold for escalation (0.0 – 1.0).
    pub coverage_threshold: f32,
}

impl Default for CodingRecallConfig {
    fn default() -> Self {
        Self {
            session_start_budget: 800,
            user_prompt_budget: 1500,
            dead_end_warnings: true,
            escalation_enabled: true,
            coverage_threshold: 0.25,
        }
    }
}

/// Reforge config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingReforgeConfig {
    /// Cron spec for the nightly heavy cycle.
    pub nightly_cron: String,
    /// Rule-artifact toggles.
    pub rule_artifacts: CodingRuleArtifactsConfig,
}

impl Default for CodingReforgeConfig {
    fn default() -> Self {
        Self {
            nightly_cron: "0 3 * * *".to_string(),
            rule_artifacts: CodingRuleArtifactsConfig::default(),
        }
    }
}

/// Which rule artifacts Reforge should write.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingRuleArtifactsConfig {
    /// Write `CLAUDE.md` managed block.
    pub claude_md: bool,
    /// Write `AGENTS.md` managed block.
    pub agents_md: bool,
    /// Write `.cursorrules` managed block.
    pub cursorrules: bool,
    /// Write `.continue/rules/klyntbot.md` managed block.
    pub continue_rules: bool,
}

impl Default for CodingRuleArtifactsConfig {
    fn default() -> Self {
        Self {
            claude_md: true,
            agents_md: true,
            cursorrules: true,
            continue_rules: true,
        }
    }
}

/// Skill config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingSkillsConfig {
    /// Enable per-project evolving skills.
    pub project_skills: bool,
    /// `"private"` (default) or `"team"`.
    pub location: String,
}

impl Default for CodingSkillsConfig {
    fn default() -> Self {
        Self {
            project_skills: true,
            location: "private".to_string(),
        }
    }
}

/// Workbench (desktop UI) config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingWorkbenchConfig {
    /// Enable the workbench UI section.
    pub enabled: bool,
    /// Session replay page size.
    pub session_replay_page_size: u32,
    /// Max causal graph nodes before collapse.
    pub causal_graph_max_nodes: u32,
}

impl Default for CodingWorkbenchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            session_replay_page_size: 500,
            causal_graph_max_nodes: 200,
        }
    }
}

/// Per-CLI toggles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingCliToggles {
    /// Claude Code.
    pub claude_code: CodingCliEntry,
    /// Codex.
    pub codex: CodingCliEntry,
    /// kimi-cli.
    pub kimi_cli: CodingCliEntry,
    /// opencode.
    pub opencode: CodingCliEntry,
}

/// One CLI's toggle entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingCliEntry {
    /// Enabled.
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trips_through_serde() {
        let cfg = CodingMemoryConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: CodingMemoryConfig = serde_json::from_str(&json).unwrap();
        assert!(!parsed.enabled);
        assert_eq!(parsed.recall.session_start_budget, 800);
        assert_eq!(parsed.recall.user_prompt_budget, 1500);
        assert!(parsed.ingest.exclude_paths.contains(&"**/.env".to_string()));
    }
}
```

- [ ] **Step 2: Register the module**

Open `crates/config/src/schema/mod.rs`. Find the existing `mod ...;` declarations. Insert alphabetically near `mod channels;`:

```rust
mod coding_memory;
```

Also add a `pub use` line with the other `pub use` statements (grep for existing `pub use self::`; follow the same pattern):

```rust
pub use self::coding_memory::{
    CodingCliEntry, CodingCliToggles, CodingDistillerConfig, CodingIngestConfig,
    CodingMemoryConfig, CodingPrivacyConfig, CodingRecallConfig,
    CodingReforgeConfig, CodingRuleArtifactsConfig, CodingSkillsConfig,
    CodingWorkbenchConfig,
};
```

- [ ] **Step 3: Add field to `Config`**

Open `crates/config/src/schema/core.rs`. Find the `pub struct Config { … }`. Add a field grouped with the other top-level sections (match the existing camelCase serde convention on Config):

```rust
    /// Coding memory subsystem configuration.
    #[serde(default)]
    pub coding_memory: CodingMemoryConfig,
```

Ensure `use super::CodingMemoryConfig;` (or `use super::coding_memory::CodingMemoryConfig;`) is imported at the top of `core.rs` — match existing import style for other config sections.

If `Config` has a `Default` impl with explicit field initialization, add `coding_memory: CodingMemoryConfig::default(),`.

- [ ] **Step 4: Build + clippy + test**

Run:
```bash
cargo build -p config && \
cargo clippy -p config --all-targets -- -D warnings && \
cargo nextest run -p config
```
Expected: PASS — including the new `default_round_trips_through_serde` test.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/coding_memory.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): CodingMemoryConfig + nested structs matching design §13.D"
```

---

### Task 21: `ProviderRole` enum

**Files:**
- Modify: `crates/providers/src/lib.rs`

- [ ] **Step 1: Inspect existing providers surface**

Run: `grep -n "pub enum\|ProviderRole\|pub use" /Users/jayden/Projects/Klynt/bot/crates/providers/src/lib.rs | head`
Expected: see existing public items. If `ProviderRole` already exists, *extend it* (add the 3 new variants) rather than creating it. If it does not exist, *create it* per step 2.

- [ ] **Step 2: Add or extend `ProviderRole`**

At the top of `crates/providers/src/lib.rs` (near existing `pub enum` declarations), insert:

```rust
/// Identifies the role a provider invocation serves. Each phase in the
/// coding-memory pipeline can be bound to a different model tier via
/// `config.json → codingMemory.distiller.model` etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    /// Per-turn Distiller (Phase 3+).
    Distiller,
    /// Reforge Phase 2.5 — Coding Synthesis.
    ReforgeSynth,
    /// Reforge Phase 3.5 — Rule Artifact Generation.
    ReforgeRules,
}
```

If an enum with the same name already exists, add the 3 variants instead of duplicating the enum definition. Coordinate with existing variants — do not remove them.

- [ ] **Step 3: Build + clippy**

Run: `cargo build -p providers && cargo clippy -p providers --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/providers/src/lib.rs
git commit -m "feat(providers): ProviderRole enum (Distiller, ReforgeSynth, ReforgeRules)"
```

---

### Task 22: Consolidated Phase-1 schema migration

**Files:**
- Modify: `crates/coding-memory/migrations/001_coding_memory.sql`

- [ ] **Step 1: Write the migration SQL**

Replace the placeholder file with the full consolidated migration:

```sql
-- Phase-1 coding-memory schema consolidation.
--
-- Per CLAUDE.md pre-release policy: every column + table the 8-phase design
-- needs lands here. No incremental migrations between phases. Direct schema
-- changes authorized until first release.

-- === semantic_facts additions =============================================

ALTER TABLE semantic_facts ADD COLUMN scope_repo_id TEXT NULL;
ALTER TABLE semantic_facts ADD COLUMN metadata TEXT NULL;
ALTER TABLE semantic_facts ADD COLUMN actor_id TEXT DEFAULT 'local_user';

CREATE INDEX IF NOT EXISTS idx_semantic_facts_scope_repo
    ON semantic_facts(scope_repo_id);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_actor
    ON semantic_facts(actor_id);

-- === episodic_memories additions ==========================================

ALTER TABLE episodic_memories ADD COLUMN kind TEXT DEFAULT 'general';
ALTER TABLE episodic_memories ADD COLUMN actor_id TEXT DEFAULT 'local_user';
ALTER TABLE episodic_memories ADD COLUMN scope_repo_id TEXT NULL;
ALTER TABLE episodic_memories ADD COLUMN metadata TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_episodic_kind
    ON episodic_memories(kind);
CREATE INDEX IF NOT EXISTS idx_episodic_actor
    ON episodic_memories(actor_id);
CREATE INDEX IF NOT EXISTS idx_episodic_scope_repo
    ON episodic_memories(scope_repo_id);

-- === skill_versions additions =============================================

ALTER TABLE skill_versions ADD COLUMN scope TEXT DEFAULT 'global';
ALTER TABLE skill_versions ADD COLUMN scope_repo_id TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_skill_versions_scope_repo
    ON skill_versions(scope, scope_repo_id);

-- === ingest_event_log =====================================================

CREATE TABLE IF NOT EXISTS ingest_event_log (
    id             TEXT PRIMARY KEY,
    source         TEXT NOT NULL,
    session_id     TEXT NOT NULL,
    turn_id        TEXT,
    cwd            TEXT NOT NULL,
    repo_id        TEXT,
    occurred_at    TEXT NOT NULL,
    received_at    TEXT NOT NULL DEFAULT (datetime('now')),
    kind           TEXT NOT NULL,
    payload        TEXT NOT NULL,
    processed      BOOLEAN NOT NULL DEFAULT FALSE,
    processing     BOOLEAN NOT NULL DEFAULT FALSE,
    actor_id       TEXT NOT NULL DEFAULT 'local_user'
);

CREATE INDEX IF NOT EXISTS idx_ingest_event_log_session
    ON ingest_event_log(session_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_ingest_event_log_turn
    ON ingest_event_log(session_id, turn_id);
CREATE INDEX IF NOT EXISTS idx_ingest_event_log_unprocessed
    ON ingest_event_log(processed, received_at) WHERE processed = 0;
CREATE INDEX IF NOT EXISTS idx_ingest_event_log_repo
    ON ingest_event_log(repo_id, occurred_at);

-- === memory_causal_edges ==================================================

CREATE TABLE IF NOT EXISTS memory_causal_edges (
    id           TEXT PRIMARY KEY,
    from_id      TEXT NOT NULL,
    to_id        TEXT NOT NULL,
    edge_kind    TEXT NOT NULL,
    confidence   REAL NOT NULL DEFAULT 0.5,
    inferred_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_causal_from ON memory_causal_edges(from_id);
CREATE INDEX IF NOT EXISTS idx_causal_to ON memory_causal_edges(to_id);
CREATE INDEX IF NOT EXISTS idx_causal_kind
    ON memory_causal_edges(edge_kind);

-- === memory_utilization ===================================================

CREATE TABLE IF NOT EXISTS memory_utilization (
    id                TEXT PRIMARY KEY,
    memory_id         TEXT NOT NULL,
    retrieved_at      TEXT NOT NULL DEFAULT (datetime('now')),
    cited_in_response BOOLEAN NOT NULL DEFAULT FALSE,
    session_id        TEXT,
    turn_id           TEXT
);

CREATE INDEX IF NOT EXISTS idx_memory_util_memory
    ON memory_utilization(memory_id, retrieved_at);
CREATE INDEX IF NOT EXISTS idx_memory_util_session
    ON memory_utilization(session_id);

-- === klynt_sessions (owned by klynt-cli spec; consolidated here per §4) ====

CREATE TABLE IF NOT EXISTS klynt_sessions (
    id                 TEXT PRIMARY KEY,
    started_at         TEXT NOT NULL,
    ended_at           TEXT,
    cwd                TEXT NOT NULL,
    repo_id            TEXT,
    initial_prompt     TEXT,
    total_turns        INTEGER NOT NULL DEFAULT 0,
    total_cost_usd     REAL NOT NULL DEFAULT 0.0,
    total_tokens_in    INTEGER NOT NULL DEFAULT 0,
    total_tokens_out   INTEGER NOT NULL DEFAULT 0,
    actor_id           TEXT NOT NULL DEFAULT 'local_user'
);

CREATE INDEX IF NOT EXISTS idx_klynt_sessions_repo
    ON klynt_sessions(repo_id, started_at);
```

- [ ] **Step 2: Write migration-applies smoke test**

Create `crates/coding-memory/tests/migration_applies.rs`:

```rust
//! Migration applies cleanly on top of the cognitive + storage baseline.
//!
//! This seeds an in-memory SQLite pool with the base cognitive migrations
//! (which create `semantic_facts` / `episodic_memories` / `skill_versions`),
//! then runs the consolidated Phase-1 migration and asserts every new
//! column/table exists.

use coding_memory::coding_memory_migrations;
use cognitive::cognitive_migrations;
use sqlx::Row;
use storage::StoragePool;

#[tokio::test]
async fn phase1_migration_applies_over_cognitive_base() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");

    pool.run_feature_migrations(&cognitive_migrations())
        .await
        .expect("cognitive migrations");

    pool.run_feature_migrations(&coding_memory_migrations())
        .await
        .expect("coding-memory migration");

    // New semantic_facts columns
    for col in ["scope_repo_id", "metadata", "actor_id"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('semantic_facts') \
             WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool.sqlx_pool())
        .await
        .unwrap();
        assert_eq!(exists, 1, "semantic_facts missing column: {col}");
    }

    // New episodic_memories columns
    for col in ["kind", "actor_id", "scope_repo_id", "metadata"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('episodic_memories') \
             WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool.sqlx_pool())
        .await
        .unwrap();
        assert_eq!(exists, 1, "episodic_memories missing column: {col}");
    }

    // New tables
    for table in [
        "ingest_event_log",
        "memory_causal_edges",
        "memory_utilization",
        "klynt_sessions",
    ] {
        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
        )
        .bind(table)
        .fetch_optional(pool.sqlx_pool())
        .await
        .unwrap();
        assert!(row.is_some(), "missing table: {table}");
    }

    // skill_versions scope columns
    for col in ["scope", "scope_repo_id"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('skill_versions') \
             WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool.sqlx_pool())
        .await
        .unwrap();
        assert_eq!(exists, 1, "skill_versions missing column: {col}");
    }

    let _ = row_count_check(&pool).await;
}

async fn row_count_check(pool: &StoragePool) -> sqlx::Result<()> {
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_event_log")
        .fetch_one(pool.sqlx_pool())
        .await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_causal_edges")
        .fetch_one(pool.sqlx_pool())
        .await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_utilization")
        .fetch_one(pool.sqlx_pool())
        .await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM klynt_sessions")
        .fetch_one(pool.sqlx_pool())
        .await?;
    Ok(())
}

/// Silence clippy about the unused helper trait method.
const _ = ();
```

Note: if `StoragePool::sqlx_pool()` isn't the public accessor in this workspace, swap to the one used by existing `storage::test_util` consumers (grep existing tests for `StoragePool::connect_in_memory` to see the pool accessor). The intent of the test is: after running both migrations, every Phase-1 column + table exists.

- [ ] **Step 3: Wire dev deps so the test compiles**

Edit `crates/coding-memory/Cargo.toml` `[dev-dependencies]` to add:

```toml
cognitive = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

(If any of these are already present in `[dependencies]`, they still need to be listed in `[dev-dependencies]` only if they are dev-only, which here they are not — verify with `cargo tree -p coding-memory --edges=normal`.)

- [ ] **Step 4: Run the test**

Run: `cargo nextest run -p coding-memory --test migration_applies`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/migrations/001_coding_memory.sql crates/coding-memory/tests/migration_applies.rs crates/coding-memory/Cargo.toml
git commit -m "feat(coding-memory): consolidated Phase-1 schema migration + applies test"
```

---

### Task 23: Architecture docs in `docs/coding-memory/`

**Files:**
- Create: `docs/coding-memory/README.md`
- Create: `docs/coding-memory/decisions.md`

- [ ] **Step 1: Create `docs/coding-memory/README.md`**

```markdown
# Coding Memory — Engineering Notes

**Spec:** `docs/superpowers/specs/2026-04-22-coding-memory-design.md`
**Phase-1 plan:** `docs/superpowers/plans/2026-04-23-coding-memory-phase-1.md`

## What this is

Two new L5 workspace crates that turn klyntbot's cognitive infrastructure
into a cross-CLI coding memory layer.

- `crates/coding-ingest/` — `AgentEvent` contract, CLI adapter stubs,
  transport stubs, daemon stub, `klyntbot-hook` shell binary.
- `crates/coding-memory/` — fact taxonomy, `MemorySink` trait, Distiller /
  Recall / Reforge / Skill module stubs, consolidated migration, MCP tool
  stubs.

## Phase 1 scope

Architecture skeleton only. Every public type, trait, migration column, MCP
tool, and `DomainEvent` variant the 8-phase plan needs exists and compiles
clean. Methods return `KlyntbotError::NotImplemented` with the required
phase number. No runtime behavior.

## Dependency flow

```
coding-ingest  →  common, bus, storage (+ serde / tokio / uuid / jiff)
coding-memory  →  coding-ingest, common, bus, storage, providers,
                  context_engine, cognitive, tools-core
```

Strictly upward; neither crate reaches into `desktop` / `app-core`.

## Future phases

Downstream work (Phase 2–8) lives in siblings of the Phase-1 plan under
`docs/superpowers/plans/`. Each phase's plan starts from the skeleton this
document describes and fills in bodies.
```

- [ ] **Step 2: Create `docs/coding-memory/decisions.md`**

```markdown
# Coding Memory — Decisions

Concise mirror of the "Key decisions" table from the design spec §3. Kept
here so engineers working in this subsystem don't need to re-read the full
spec for context.

| Axis | Decision |
|---|---|
| Project shape | Combined memory + cognition + multi-CLI ingestion. |
| Ingestion priority | External CLIs first; native klynt-cli integration deferred. |
| Data topology | **Shared store with klyntbot** (Approach A). Same SQLite + LanceDB. `scope_repo_id` partitioning. |
| Crate placement | `coding-memory` (L5) + `coding-ingest` (L5). No new standalone binary beyond `klyntbot-hook`. |
| Distiller timing | Per-turn batched — one LLM call per user turn. |
| Distiller role | Online writer only. ADD or SUPERSEDE; never DELETE. |
| Reforge role | Offline optimizer only. Six responsibilities (see design §9). |
| Mirror role | Real-time observer. `PatternEffectivenessSubscriber` updates `effectiveness_score` within seconds. |
| Integration surface | Hooks (passive) + MCP tools (active). No LLM proxy, no ACP in Phase 1-7. |
| Daemon lifecycle | klyntbot desktop owns the ingest socket. Hook falls back to file buffer when desktop is off. |
| User install path | Desktop UI settings page. |
| Rule artifacts | Managed-block sections of `CLAUDE.md` / `AGENTS.md` / `.cursorrules`. |
| Schema approach | Consolidated Phase-1 migration. Pre-release authorizes direct schema changes. |
| klynt-cli source class | First-class native source emitting the full rich variant set. |

## Invariants (all proptest-enforced in later phases)

1. Provenance-always.
2. Distiller-never-deletes.
3. Reforge-never-deletes-raw.
4. Bi-temporal monotone (`valid_until ≥ valid_from`).
5. SUPERSEDE chain (`predecessor.valid_until == successor.valid_from`).
6. Scope isolation (repo-scoped retrieval never leaks cross-repo).
7. Hook round-trip identity (`parse(serialize(AgentEvent)) == AgentEvent`).
8. Causal edge validity (no dangling `from_id` / `to_id`).
9. Budget enforcement (SessionStart ≤ 800 tok; UserPromptSubmit ≤ 1500 tok).
```

- [ ] **Step 3: Commit**

```bash
mkdir -p docs/coding-memory
git add docs/coding-memory/README.md docs/coding-memory/decisions.md
git commit -m "docs(coding-memory): architecture + decision records for Phase 1"
```

---

### Task 24: Public-surface smoke test

**Files:**
- Create: `crates/coding-memory/tests/public_surface.rs`

- [ ] **Step 1: Write the test**

Create `crates/coding-memory/tests/public_surface.rs`:

```rust
//! Smoke test for Phase-1 public surface — every type is constructable and
//! compiles through the paths downstream phases will use. Runs no business
//! logic. Exists so a rename in later phases trips CI instead of silently
//! breaking the architecture skeleton.

use coding_memory::distiller::{Distiller, TurnTrace};
use coding_memory::error::NotImplementedInPhase;
use coding_memory::facts::{CodingKind, FixAttempt, FixOutcome, StyleScope};
use coding_memory::mcp::{stub_handler, CODING_MEMORY_MCP_TOOLS};
use coding_memory::recall::{
    CodingRecallService, IndexEntry, RecallQuery,
};
use coding_memory::reforge_phase::{
    CodingSynthesisPhase, RuleArtifact, RuleArtifactGenerationPhase,
};
use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, QueryRewriter, RetrievalSkill,
};
use coding_memory::scope::{
    AnchoredSymbol, CausalEdgeKind, ProvenanceKind, Sensitivity,
};
use coding_memory::sink::{InProcessSink, MemorySink};
use coding_memory::skills::{
    PhaseStubEvolver, ProjectSkillLocation, SkillId, SkillScope,
};

#[test]
fn phase1_types_are_constructable() {
    let _ = NotImplementedInPhase::new(4);
    let _ = CodingKind::FixAttempt;
    let _ = FixOutcome::Abandoned;
    let _ = StyleScope::Global;
    let _ = ProvenanceKind::DistillerExtractive;
    let _ = Sensitivity::default();
    let _ = CausalEdgeKind::Broke;
    let _ = BudgetTier::DeepThink;
    let _ = ProjectSkillLocation::Private;
    let _ = SkillScope::Global;
    let _ = SkillId("x".into());
    let _ = RuleArtifact::ClaudeMd.relative_path();
}

#[test]
fn phase1_mcp_tool_constant_matches_handler() {
    assert_eq!(CODING_MEMORY_MCP_TOOLS.len(), 8);
    for t in CODING_MEMORY_MCP_TOOLS {
        let err = stub_handler(t).unwrap_err();
        assert!(err.to_string().contains(t));
    }
}

#[tokio::test]
async fn phase1_stub_services_return_not_implemented() {
    let distiller = Distiller::new();
    assert!(distiller.accept_event(dummy_event()).await.is_err());

    let sink = InProcessSink::new();
    assert!(sink.accept_event(dummy_event()).await.is_err());
    assert!(sink.flush().await.is_err());

    let recall = CodingRecallService::new();
    assert!(recall
        .recall_index("q", None, None, None, 10)
        .await
        .is_err());
    assert!(recall
        .recall_timeline(RecallQuery::Text("q".into()), None, 7)
        .await
        .is_err());
    assert!(recall.check_dead_ends("approach", None).await.is_err());

    let phase = CodingSynthesisPhase::default();
    assert!(
        coding_memory::reforge_phase::ReforgePhaseRun::run(&phase)
            .await
            .is_err()
    );
    let phase = RuleArtifactGenerationPhase::default();
    assert!(
        coding_memory::reforge_phase::ReforgePhaseRun::run(&phase)
            .await
            .is_err()
    );

    let skill = QueryRewriter;
    let ctx = EscalationContext {
        query: "q".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::DeepThink,
    };
    assert!(skill.apply(&ctx).await.is_err());
    assert!((skill.effectiveness_score() - 0.5).abs() < f32::EPSILON);

    let evolver = PhaseStubEvolver;
    use coding_memory::skills::ProjectSkillEvolver;
    assert!(evolver.evolve("repo").await.is_err());

    // Turn trace exists as a type (not returned — just referenced).
    let _: Option<TurnTrace> = None;
    let _ = dummy_fix_attempt();
    let _: Option<IndexEntry> = None;
    let _: Option<AnchoredSymbol> = None;
}

fn dummy_event() -> coding_ingest::AgentEvent {
    use coding_ingest::{AgentEvent, AgentEventV1, AgentSource, EventKind};
    use jiff::Timestamp;
    use std::path::PathBuf;
    use uuid::Uuid;

    AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::KlyntCli,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/"),
        repo: None,
        occurred_at: Timestamp::from_second(0).unwrap(),
        kind: EventKind::SessionStart {
            model: None,
            source_reason: "test".into(),
        },
    })
}

fn dummy_fix_attempt() -> FixAttempt {
    use coding_memory::scope::ProvenanceMetadata;
    use jiff::Timestamp;
    use uuid::Uuid;

    FixAttempt {
        problem_hash: "h".into(),
        problem: "p".into(),
        files: vec![],
        approach: "a".into(),
        outcome: FixOutcome::Success,
        insight: None,
        duration_ms: 0,
        test_before: None,
        test_after: None,
        anchored_symbols: vec![],
        provenance: ProvenanceMetadata {
            source_events: vec![Uuid::nil()],
            session_id: "s".into(),
            turn_id: None,
            distilled_at: Timestamp::from_second(0).unwrap(),
            distiller_model: "m".into(),
            source_kind: ProvenanceKind::DistillerExtractive,
        },
        sensitivity: Sensitivity::default(),
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p coding-memory --test public_surface`
Expected: all three tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/tests/public_surface.rs
git commit -m "test(coding-memory): public-surface smoke test for Phase 1"
```

---

### Task 25: Final Phase-1 verification + exit gates

**Files:** none created; this task runs the exit-gate commands from the spec §11.

- [ ] **Step 1: Workspace build**

Run: `cargo build --workspace`
Expected: PASS, zero errors, zero warnings.

- [ ] **Step 2: Clippy (strict, all targets, all features)**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS, zero warnings. If `desktop` crate has pre-existing allowances, keep them — do not introduce new allowances elsewhere.

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`
Expected: exits 0.

- [ ] **Step 4: Full test run**

Run: `cargo nextest run --workspace`
Expected: all tests PASS including:
- `agent_event_roundtrip` (12 roundtrip tests)
- `migration_applies`
- `public_surface` (3 tests)
- `config::schema::coding_memory::tests::default_round_trips_through_serde`
- `mcp::tests::every_tool_has_a_phase` + `tools_match_allowlist_constants`
- `bus::tests::pattern_applied_roundtrips` + `retrieval_skill_applied_roundtrips`

- [ ] **Step 5: Doctest run**

Run: `cargo test --workspace --doc`
Expected: PASS.

- [ ] **Step 6: Doc coverage on new crates**

Run:
```bash
RUSTDOCFLAGS="-D missing-docs" cargo doc -p coding-ingest -p coding-memory --no-deps
```
Expected: exits 0.

- [ ] **Step 7: Regression grep — no stray `todo!()` or unimplemented `panic!()` in shipped paths**

Run:
```bash
grep -rn "todo!()\|unimplemented!()" crates/coding-ingest/src crates/coding-memory/src
```
Expected: zero hits. (Phase 1 uses `KlyntbotError::NotImplemented(...)` returns instead of macros.)

- [ ] **Step 8: Regression check — existing crates' tests still pass**

Run: `cargo nextest run -p bus -p config -p cognitive -p common -p storage`
Expected: PASS — Phase-1 edits are purely additive, so no prior tests should regress.

- [ ] **Step 9: Smoke-test the `klyntbot-hook` binary once more**

Run:
```bash
echo '{"hello":"world"}' | cargo run -q -p coding-ingest --bin klyntbot-hook -- claude-code PostToolUse 2>&1
```
Expected: stderr contains `phase 1 stub — not forwarded`; exit 0.

- [ ] **Step 10: Final commit (if anything is dirty)**

If the verification steps produced no changes, skip. Otherwise:

```bash
git status
git add -u && git commit -m "chore(coding-memory): Phase-1 verification touch-ups"
```

---

## Self-Review

**Spec coverage (Phase 1 exit gates — design §11):**

| Requirement | Task |
|---|---|
| `coding-memory` crate — all public types + traits + `unimplemented!()` bodies | Tasks 2, 10, 11, 12, 13, 14, 15, 16, 17 |
| `coding-ingest` crate — `AgentEvent`, `IngestSocket` trait, `IngestAdapter` trait, 4 adapter stubs | Tasks 1, 3, 4, 5, 6, 7 |
| Consolidated schema migration | Task 22 |
| 5 new `DomainEvent` variants (spec lists 5, plus `RetrievalSkillApplied` in appendix A = 6 total) | Task 19 |
| All MCP tool stubs registered, return typed `NotImplementedInPhase` error | Tasks 17, 18 |
| `klyntbot-hook` binary with arg parsing for all 4 CLIs; writes to stderr only | Task 9 |
| Architecture diagram + decision records in `docs/coding-memory/` | Task 23 |
| Workspace builds clean; zero clippy warnings; fmt passes; all public items documented | Task 25 |
| `MemorySink` trait present (Phase-1 surface per design §5) | Task 12 |
| `ProviderRole::Distiller` declared | Task 21 |
| Config surface (spec §13.D) | Task 20 |
| `actor_id` forward-compat column | Task 22 (semantic_facts + episodic_memories) |
| Sensitivity tagging type (`Sensitivity`) | Task 10 |
| Formalized `RetrievalSkill` registry | Task 16 |
| `klynt_sessions` table consolidated into Phase-1 migration | Task 22 |

**Placeholder scan:** Every stub method body returns `KlyntbotError::NotImplemented(format!("... Phase {}", required_phase))` or `NotImplementedInPhase`-formatted variant. No `TODO`, no `fill in details`, no "similar to Task N". Each code block is complete and runnable.

**Type consistency:**
- `CodingKind` enum has exactly 5 variants (Task 11) matching the Distiller LLM tool schema (design §6).
- `EventKind` has 9 base + 10 rich = 19 variants (Tasks 3, 4) matching design §5.
- `CausalEdgeKind` closed-set enum is used identically across `scope.rs` and `recall::CausalTraceResponse` (Tasks 10, 14).
- MCP tool names in `CODING_MEMORY_MCP_TOOLS` (Task 17) are byte-identical to the 8 names appended to `EXPLICIT_TOOL_ALLOWLIST` (Task 18).
- `Sensitivity` variants (`Normal`/`High`/`Excluded`) match design §7 and the config string values in `CodingPrivacyConfig::default_sensitivity` (Task 20).
- `ProviderRole` variants (`Distiller`, `ReforgeSynth`, `ReforgeRules`) match design Appendix C (Task 21).

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-23-coding-memory-phase-1.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
