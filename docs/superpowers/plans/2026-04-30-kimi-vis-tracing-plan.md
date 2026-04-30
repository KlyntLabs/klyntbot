# Kimi-vis–quality Agent Tracing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `Plugins → Coding Memory → Tracing` sub-tab that matches `kimi vis` quality for Kimi sessions, behind a `TracingProvider` abstraction so future Codex / Claude Code / opencode adapters drop in without touching shared UI.

**Architecture:** Live-read of `~/.kimi/sessions/` from a `KimiTracingProvider` implementing a generic `TracingProvider` trait held by `AppCore`. Provider-agnostic DTOs (`TraceEvent` tagged with `SemanticCategory`) flow through nine new Tauri commands to a new React tree under `desktop-ui/src/features/plugins/coding-memory/tracing/`. Per-category event-card components dispatch on `event.category`; provider-specific payload shape is read inside each card. No DB writes for tracing data.

**Tech Stack:** Rust (`tokio`, `async_trait`, `jiff`, `serde_json`, `md5`, `tracing`), React 19 (`react-virtuoso` already a dep), Vitest + RTL, Tauri 2, specta.

**Reference spec:** `docs/superpowers/specs/2026-04-30-kimi-vis-tracing-design.md`.

**Reading order if you're new to this codebase:**
- `CLAUDE.md` (root) — coding conventions, MSRV, Tauri command rules.
- `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs` — `WireRecord`, `parse_line`, `collect_events`.
- `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs` — `WorkdirIndex`, `hash_for`.
- `crates/desktop-shared/src/commands/coding_memory.rs` — DTO style with `specta::Type`.
- `crates/desktop/src/commands/coding_memory.rs` — Tauri adapter style with `#[klynt_command]`.
- `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx` — where the new sub-tab plugs in.
- `desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx` — existing virtualized viewer (a reference for the new one — do **not** edit).

**Conventions you must follow:**
- Every public method on an `AppCore` handler: `#[tracing::instrument(skip(self), err)]`. Tauri command shells in `crates/desktop/src/commands/` are NOT instrumented.
- Errors: `common::Result<T>` (alias for `Result<T, KlyntbotError>`).
- DTOs use `#[serde(rename_all = "camelCase")]` and derive `specta::Type`.
- Tauri commands: `#[klynt_command]` for the happy path; `#[klynt_raw_command]` only when the macro can't infer.
- Path aliases: `@/`, `@app/`, `@settings/`, `@threads/`, `@services/`, `@utils/`. **No relative `../../`.**
- CSS: BEM-ish; tokens from `ds-tokens.css` (`--fs-*`, never raw px font-sizes); `@import` new files into `desktop-ui/src/styles/index.css`.
- Tests: `cargo nextest run -p <crate>` for Rust; `bun run test` for FE. Doctests via `cargo test --workspace --doc`. Zero clippy warnings.
- After adding any Tauri command, run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`. Add new command paths to `klynt_collect_commands![…]` in `crates/desktop/src/specta_builder.rs`.

**Working tree:** This plan can be executed on `main` directly or in a worktree per `superpowers:using-git-worktrees`. Each task ends with a commit.

---

## Phase index

1. Test fixtures
2. Skeleton: module tree
3. Core types: `Scope`, `SemanticCategory`, `TraceEvent`
4. Core types: session/header DTOs
5. Core types: context, state, subagent, stats DTOs
6. `TracingProvider` trait
7. `TracingRegistry`
8. Kimi provider — categorize
9. Kimi provider — `WorkdirIndex::entries()` accessor
10. Kimi provider — discovery
11. Kimi provider — wire-file streaming + header stats + turn/step indexing
12. Kimi provider — context loader
13. Kimi provider — state loader
14. Kimi provider — subagent loader
15. Kimi provider — list-sessions cache
16. Kimi provider — import
17. Kimi provider — stats aggregation
18. Kimi provider — `TracingProvider` impl + integration test (includes `open_dir`)
19. AppCore wiring + handler methods
20. Desktop-shared DTOs (specta::Type derives on app-core types + re-export shim)
21. Tauri commands
22. Specta builder registration + bindings regen
23. Frontend: tracing module skeleton + CSS + provider chrome registry
24. Frontend: hooks (`useTracingSessions`, `useTracingSession`)
25. Frontend: TracingTab skeleton + plug into CodingMemoryPlugin
26. Frontend: SessionsToolbar
27. Frontend: SessionCard (vitest-tested)
28. Frontend: ProjectGroup
29. Frontend: SessionsListView (includes Import button + Imported toggle)
30. Frontend: DetailHeader
31. Frontend: ScopeSelector
32. Frontend: SessionDetailView shell with five tabs
33. Frontend: EventTypeChip
34. Frontend: Event cards (15 sub-tasks, one per category + barrel)
35. Frontend: WireEventsTab — list rendering + dispatch
36. Frontend: WireFilters — type chips + presets
37. Frontend: WireFilters — search + error nav + expand/collapse + completeness
38. Frontend: WireFilters — view modes (Chart, Events, Timeline, Decisions)
39. Frontend: TurnTree
40. Frontend: ToolCallDetail sidecar
41. Frontend: ContextMessagesTab
42. Frontend: StateTab
43. Frontend: DualTab
44. Frontend: AgentsTab
45. Frontend: StatisticsView
46. Frontend: Provider chip row
47. Final polish + manual smoke test

---

## Phase 1 — Test fixtures

The Kimi loader unit tests use small inline JSON. The integration test uses a real (but trimmed) session copied into the repo so behavior is reproducible without `~/.kimi/`.

### Task 1.1: Create the fixture directory tree

**Files:**
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/wire.jsonl`
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/context.jsonl`
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/state.json`
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/subagents/sub-aaa/meta.json`
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/subagents/sub-aaa/wire.jsonl`
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/subagents/sub-aaa/prompt.txt`
- Create: `crates/app-core/tests/fixtures/kimi/kimi.json`

- [ ] **Step 1: Create `kimi.json`**

```json
{
  "work_dirs": [
    {
      "path": "/tmp/kimi-fixture-repo",
      "kaos": "local",
      "last_session_id": "sess-fixture-001"
    }
  ]
}
```

Note: md5("/tmp/kimi-fixture-repo") = `5ac861883e2a4565a455cf1bc7fed83e`. The directory is named `abc123hash` for readability — tests will override the hash → cwd map directly rather than computing it. (We'll add a test-only constructor to `KimiTracingProvider` that takes an explicit `WorkdirIndex` so we don't rely on hash matching.)

- [ ] **Step 2: Create the trimmed `wire.jsonl`** with 1 turn, 2 steps, 1 tool call, 1 status update, 1 thinking, 1 text, 1 turn end. Each line is a complete JSON record.

```jsonl
{"type":"metadata","protocol_version":"1.9"}
{"timestamp":1777454398.5,"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"hello fixture"}]}}}
{"timestamp":1777454398.6,"message":{"type":"StepBegin","payload":{"n":1}}}
{"timestamp":1777454399.0,"message":{"type":"ContentPart","payload":{"type":"think","think":"thinking about the request","encrypted":null}}}
{"timestamp":1777454399.5,"message":{"type":"ToolCall","payload":{"type":"function","id":"tool_X","function":{"name":"ReadFile","arguments":"{\"path\":\"/tmp/x\"}"},"extras":null}}}
{"timestamp":1777454399.6,"message":{"type":"StatusUpdate","payload":{"context_usage":0.045,"context_tokens":11706,"max_context_tokens":262144,"token_usage":{"input_other":2490,"output":106,"input_cache_read":9216,"input_cache_creation":0},"message_id":"chatcmpl-fixture","plan_mode":false,"mcp_status":null}}}
{"timestamp":1777454400.0,"message":{"type":"ToolResult","payload":{"id":"tool_X","content":"ok content","is_error":false}}}
{"timestamp":1777454400.5,"message":{"type":"StepBegin","payload":{"n":2}}}
{"timestamp":1777454401.0,"message":{"type":"ContentPart","payload":{"type":"text","text":"all done"}}}
{"timestamp":1777454401.2,"message":{"type":"TurnEnd","payload":{}}}
```

- [ ] **Step 3: Create the trimmed `context.jsonl`**

```jsonl
{"role":"_system_prompt","content":"You are Kimi Code CLI."}
{"role":"user","content":"hello fixture"}
{"role":"assistant","content":"all done"}
{"role":"_usage","input_tokens":11706,"output_tokens":106}
{"role":"_checkpoint","at_message_index":3}
```

- [ ] **Step 4: Create `state.json`**

```json
{
  "version": 1,
  "approval": {"yolo": false, "afk": false, "auto_approve_actions": []},
  "additional_dirs": [],
  "custom_title": "fixture session",
  "title_generated": false,
  "title_generate_attempts": 0,
  "plan_mode": false,
  "plan_session_id": null,
  "plan_slug": null,
  "wire_mtime": null,
  "archived": false,
  "archived_at": null,
  "auto_archive_exempt": false,
  "todos": [
    {"title":"Done thing","status":"done"},
    {"title":"In-flight thing","status":"in_progress"}
  ]
}
```

- [ ] **Step 5: Create `subagents/sub-aaa/meta.json`**

```json
{
  "agent_id": "sub-aaa",
  "subagent_type": "coder",
  "status": "idle",
  "description": "Fixture subagent",
  "created_at": 1777454500.0,
  "updated_at": 1777454510.0,
  "last_task_id": null,
  "launch_spec": {"agent_id":"sub-aaa","subagent_type":"coder","model_override":null,"effective_model":null,"created_at":1777454500.0}
}
```

- [ ] **Step 6: Create `subagents/sub-aaa/wire.jsonl`**

```jsonl
{"type":"metadata","protocol_version":"1.9"}
{"timestamp":1777454500.5,"message":{"type":"TurnBegin","payload":{"user_input":"sub task"}}}
{"timestamp":1777454501.0,"message":{"type":"StepBegin","payload":{"n":1}}}
{"timestamp":1777454501.5,"message":{"type":"ContentPart","payload":{"type":"text","text":"sub done"}}}
{"timestamp":1777454501.6,"message":{"type":"TurnEnd","payload":{}}}
```

- [ ] **Step 7: Create `subagents/sub-aaa/prompt.txt`**

```
You are implementing a fixture task.
```

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/tests/fixtures/kimi/
git commit -m "test(app-core): add Kimi tracing fixture session"
```

---

## Phase 2 — Skeleton: module tree

### Task 2.1: Create `app-core/src/tracing/` module skeleton

**Files:**
- Create: `crates/app-core/src/tracing/mod.rs`
- Modify: `crates/app-core/src/lib.rs`

- [ ] **Step 1: Write `tracing/mod.rs`**

```rust
//! Cross-CLI agent-tracing module.
//!
//! v1 implements only the Kimi provider; the trait and DTOs are
//! deliberately CLI-agnostic so Codex/Claude Code/opencode adapters
//! drop in without UI changes.

pub mod provider;
pub mod providers;
pub mod registry;
pub mod types;

pub use provider::TracingProvider;
pub use registry::TracingRegistry;
pub use types::{
    ContextMessage, HeaderStats, KimiTodo, ProviderInfo, Scope, SemanticCategory,
    SessionDetail, SessionState, SessionSummary, StatsBundle, SubagentSummary,
    TraceEvent,
};
```

- [ ] **Step 2: Add `pub mod tracing;` to `crates/app-core/src/lib.rs`**

Open `crates/app-core/src/lib.rs`, add at top alongside other `pub mod` lines:

```rust
pub mod tracing;
```

- [ ] **Step 3: Commit (will not yet build — files referenced don't exist; we add stubs in next task)**

```bash
git add crates/app-core/src/tracing/mod.rs crates/app-core/src/lib.rs
git commit -m "feat(app-core): scaffold tracing module"
```

### Task 2.2: Create empty submodule files so it compiles

**Files:**
- Create: `crates/app-core/src/tracing/types.rs`
- Create: `crates/app-core/src/tracing/provider.rs`
- Create: `crates/app-core/src/tracing/registry.rs`
- Create: `crates/app-core/src/tracing/providers/mod.rs`

- [ ] **Step 1: Create empty `types.rs`** with one line so re-exports don't fail. We'll fill in next phase.

```rust
//! Provider-agnostic DTOs for the tracing UI.
```

The re-exports in `mod.rs` will fail until types are added. Temporarily comment them out:

In `tracing/mod.rs`, change to:

```rust
pub mod provider;
pub mod providers;
pub mod registry;
pub mod types;
```

(Drop the `pub use` lines for now; we add them back as types appear.)

- [ ] **Step 2: Create empty `provider.rs`**

```rust
//! TracingProvider trait — implemented per CLI.
```

- [ ] **Step 3: Create empty `registry.rs`**

```rust
//! TracingRegistry — provider-id → provider dispatch.
```

- [ ] **Step 4: Create `providers/mod.rs`**

```rust
//! Per-CLI TracingProvider implementations.

pub mod kimi;
```

- [ ] **Step 5: Create `providers/kimi/mod.rs` with empty placeholder**

```rust
//! Kimi-cli TracingProvider implementation.
```

- [ ] **Step 6: Verify it compiles**

```bash
cargo check -p app-core
```

Expected: success, no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/tracing/
git commit -m "feat(app-core): empty tracing submodule files"
```

---

## Phase 3 — Core types: Scope, SemanticCategory, TraceEvent

### Task 3.1: Add `Scope` enum

**Files:**
- Modify: `crates/app-core/src/tracing/types.rs`

- [ ] **Step 1: Add the type and a unit test in the same file**

Append to `types.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Selector for "main agent" vs a specific subagent within a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Scope {
    Main,
    Subagent { agent_id: String },
}

impl Scope {
    pub fn is_main(&self) -> bool {
        matches!(self, Scope::Main)
    }
    pub fn subagent_id(&self) -> Option<&str> {
        match self {
            Scope::Main => None,
            Scope::Subagent { agent_id } => Some(agent_id),
        }
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    #[test]
    fn main_serializes_with_tag() {
        let s = serde_json::to_string(&Scope::Main).unwrap();
        assert_eq!(s, r#"{"kind":"main"}"#);
    }
    #[test]
    fn subagent_serializes_with_camel_case_field() {
        let s = serde_json::to_string(&Scope::Subagent { agent_id: "a1".into() }).unwrap();
        assert_eq!(s, r#"{"kind":"subagent","agentId":"a1"}"#);
    }
}
```

- [ ] **Step 2: Run the tests**

```bash
cargo nextest run -p app-core -E 'test(scope_tests)'
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/types.rs
git commit -m "feat(app-core): add Scope enum for tracing"
```

### Task 3.2: Add `SemanticCategory` enum

**Files:**
- Modify: `crates/app-core/src/tracing/types.rs`

- [ ] **Step 1: Append the enum and tests**

```rust
/// Closed semantic category the UI dispatches per event card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticCategory {
    TurnBegin,
    TurnEnd,
    StepBegin,
    StepInterrupted,
    Thinking,
    AssistantText,
    UserInput,
    ToolCall,
    ToolCallStream,
    ToolResult,
    StatusUpdate,
    Subagent,
    CompactionBegin,
    CompactionEnd,
    Error,
    Other,
}

#[cfg(test)]
mod category_tests {
    use super::*;
    #[test]
    fn category_serializes_camel_case() {
        let s = serde_json::to_string(&SemanticCategory::ToolCall).unwrap();
        assert_eq!(s, r#""toolCall""#);
        let s = serde_json::to_string(&SemanticCategory::CompactionBegin).unwrap();
        assert_eq!(s, r#""compactionBegin""#);
    }
}
```

- [ ] **Step 2: Run**

```bash
cargo nextest run -p app-core -E 'test(category_tests)'
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/types.rs
git commit -m "feat(app-core): add SemanticCategory enum"
```

### Task 3.3: Add `TraceEvent` struct

**Files:**
- Modify: `crates/app-core/src/tracing/types.rs`

- [ ] **Step 1: Append the struct**

```rust
use jiff::Timestamp;

/// One event in a session's wire stream, in provider-agnostic form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEvent {
    /// Per-session-monotonic sequence (line index in wire.jsonl, skipping metadata).
    pub seq: u64,
    /// Provider id ("kimi", "claudeCode", …).
    pub provider_id: String,
    /// Verbatim raw kind string from the source ("TurnBegin", "ContentPart", …).
    pub raw_kind: String,
    /// Verbatim payload from the source.
    pub payload: serde_json::Value,
    pub occurred_at: Timestamp,
    pub category: SemanticCategory,
    pub turn_index: Option<u32>,
    pub step_index: Option<u32>,
    pub parent_subagent_id: Option<String>,
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p app-core
```

Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/types.rs
git commit -m "feat(app-core): add TraceEvent struct"
```

---

## Phase 4 — Core types: session/header DTOs

### Task 4.1: Add `HeaderStats`

**Files:**
- Modify: `crates/app-core/src/tracing/types.rs`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeaderStats {
    pub turn_count: u32,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub compaction_count: u32,
    pub agent_count: u32,
    pub total_duration_ms: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_hit_pct: f32,
    pub model: Option<String>,
}
```

- [ ] **Step 2: `cargo check -p app-core`** → success.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/types.rs
git commit -m "feat(app-core): add HeaderStats"
```

### Task 4.2: Add `SessionSummary`

- [ ] **Step 1: Append**

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub provider_id: String,
    pub source_dir: PathBuf,
    pub cwd: Option<PathBuf>,
    pub project_basename: Option<String>,
    pub custom_title: Option<String>,
    pub started_at: Timestamp,
    pub last_event_at: Timestamp,
    pub size_bytes: u64,
    pub turn_count: u32,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub subagent_count: u32,
    pub has_wire: bool,
    pub has_context: bool,
    pub imported: bool,
}
```

- [ ] **Step 2: `cargo check -p app-core`** → success.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-core): add SessionSummary"
```

### Task 4.3: Add `SessionDetail`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session_id: String,
    pub provider_id: String,
    pub scope: Scope,
    pub stats: HeaderStats,
    pub events: Vec<TraceEvent>,
    /// `true` when events were truncated due to file-size guardrail.
    pub truncated: bool,
    pub total_event_count: u64,
}
```

- [ ] **Step 2: `cargo check -p app-core`** → success.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-core): add SessionDetail"
```

---

## Phase 5 — Core types: context, state, subagent, stats DTOs

### Task 5.1: Add `ContextMessage`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMessage {
    pub index: u32,
    pub role: String,
    pub content: serde_json::Value,
}
```

- [ ] **Step 2: `cargo check`** → success.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-core): add ContextMessage"
```

### Task 5.2: Add `KimiTodo` + `SessionState`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiTodo {
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub custom_title: Option<String>,
    pub plan_mode: bool,
    pub archived: bool,
    pub todos: Vec<KimiTodo>,
    /// Verbatim parsed state.json for the State tab's "raw" view.
    pub raw: serde_json::Value,
}
```

- [ ] **Step 2: `cargo check`** → success.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-core): add SessionState + KimiTodo"
```

### Task 5.3: Add `SubagentSummary`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSummary {
    pub agent_id: String,
    pub subagent_type: String,
    pub status: String,
    pub description: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub event_count: u32,
}
```

- [ ] **Step 2: `cargo check`** → success.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-core): add SubagentSummary"
```

### Task 5.4: Add `StatsBundle`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTotals {
    pub project_basename: String,
    pub cwd: PathBuf,
    pub session_count: u32,
    pub turn_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsage {
    pub tool: String,
    pub call_count: u32,
    pub error_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorByTool {
    pub tool: String,
    pub error_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSeriesPoint {
    /// Day key in `YYYY-MM-DD`.
    pub day: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentTypeCount {
    pub subagent_type: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatsBundle {
    pub per_project: Vec<ProjectTotals>,
    pub tool_usage: Vec<ToolUsage>,
    pub errors_by_tool: Vec<ErrorByTool>,
    pub token_series: Vec<TokenSeriesPoint>,
    pub subagent_types: Vec<SubagentTypeCount>,
}
```

- [ ] **Step 2: `cargo check`** → success.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-core): add StatsBundle and friends"
```

### Task 5.5: Add `ProviderInfo`

- [ ] **Step 1: Append**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub session_count: u32,
}
```

- [ ] **Step 2: Restore the `pub use` block in `crates/app-core/src/tracing/mod.rs`**

```rust
pub use provider::TracingProvider;
pub use registry::TracingRegistry;
pub use types::{
    ContextMessage, ErrorByTool, HeaderStats, KimiTodo, ProjectTotals, ProviderInfo, Scope,
    SemanticCategory, SessionDetail, SessionState, SessionSummary, StatsBundle, SubagentSummary,
    SubagentTypeCount, TokenSeriesPoint, ToolUsage, TraceEvent,
};
```

The `TracingProvider` and `TracingRegistry` re-exports will fail until Phase 6/7. Comment them out for now:

```rust
// pub use provider::TracingProvider;
// pub use registry::TracingRegistry;
pub use types::{ /* … */ };
```

- [ ] **Step 3: `cargo check -p app-core`** → success.

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(app-core): add ProviderInfo and re-export tracing types"
```


---

## Phase 6 — `TracingProvider` trait

### Task 6.1: Define the trait

**Files:**
- Modify: `crates/app-core/src/tracing/provider.rs`
- Modify: `crates/app-core/Cargo.toml` (add `async-trait` if absent)

- [ ] **Step 1: Check whether `async-trait` is already a dep**

```bash
grep -n 'async-trait' crates/app-core/Cargo.toml
```

If absent, add under `[dependencies]`:

```toml
async-trait = "0.1"
```

- [ ] **Step 2: Write `provider.rs`**

```rust
use async_trait::async_trait;
use common::Result;
use std::path::{Path, PathBuf};

use super::types::{
    ContextMessage, Scope, SessionDetail, SessionState, SessionSummary, StatsBundle,
    SubagentSummary,
};

/// Per-CLI agent-tracing provider. v1 implements only Kimi.
#[async_trait]
pub trait TracingProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail>;
    async fn load_context(&self, session_id: &str, scope: Scope) -> Result<Vec<ContextMessage>>;
    async fn load_state(&self, session_id: &str) -> Result<SessionState>;
    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>>;
    async fn import_from_file(&self, path: &Path) -> Result<String>;
    async fn open_dir(&self, session_id: &str) -> Result<PathBuf>;
    async fn stats(&self) -> Result<StatsBundle>;
}
```

- [ ] **Step 3: Re-enable `pub use provider::TracingProvider;` in `tracing/mod.rs`**

- [ ] **Step 4: `cargo check -p app-core`** → success.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/Cargo.toml crates/app-core/src/tracing/provider.rs crates/app-core/src/tracing/mod.rs
git commit -m "feat(app-core): TracingProvider trait"
```

---

## Phase 7 — `TracingRegistry`

### Task 7.1: Implement `TracingRegistry`

**Files:**
- Modify: `crates/app-core/src/tracing/registry.rs`

- [ ] **Step 1: Write the registry**

```rust
use std::collections::HashMap;
use std::sync::Arc;

use common::{KlyntbotError, Result};

use super::provider::TracingProvider;
use super::types::ProviderInfo;

/// Holds all registered `TracingProvider`s by id.
#[derive(Default)]
pub struct TracingRegistry {
    providers: HashMap<&'static str, Arc<dyn TracingProvider>>,
}

impl TracingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: Arc<dyn TracingProvider>) {
        self.providers.insert(provider.id(), provider);
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn TracingProvider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| KlyntbotError::NotFound(format!("tracing provider: {id}")))
    }

    /// Cheap snapshot for the FE provider chip row. Calls each provider's
    /// `list_sessions` so the count badge is accurate at render time.
    pub async fn list_providers(&self) -> Result<Vec<ProviderInfo>> {
        let mut out = Vec::with_capacity(self.providers.len());
        for (id, p) in &self.providers {
            let sessions = p.list_sessions().await.unwrap_or_default();
            out.push(ProviderInfo {
                id: (*id).to_string(),
                display_name: p.display_name().to_string(),
                session_count: sessions.len() as u32,
            });
        }
        out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracing::types::*;
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};

    struct Stub;
    #[async_trait]
    impl TracingProvider for Stub {
        fn id(&self) -> &'static str { "stub" }
        fn display_name(&self) -> &'static str { "Stub" }
        async fn list_sessions(&self) -> Result<Vec<SessionSummary>> { Ok(vec![]) }
        async fn load_session(&self, _: &str, _: Scope) -> Result<SessionDetail> {
            unimplemented!()
        }
        async fn load_context(&self, _: &str, _: Scope) -> Result<Vec<ContextMessage>> { Ok(vec![]) }
        async fn load_state(&self, _: &str) -> Result<SessionState> { Ok(SessionState::default()) }
        async fn list_subagents(&self, _: &str) -> Result<Vec<SubagentSummary>> { Ok(vec![]) }
        async fn import_from_file(&self, _: &Path) -> Result<String> { Ok("x".into()) }
        async fn open_dir(&self, _: &str) -> Result<PathBuf> { Ok(PathBuf::from("/")) }
        async fn stats(&self) -> Result<StatsBundle> { Ok(StatsBundle::default()) }
    }

    #[tokio::test]
    async fn register_and_get() {
        let mut r = TracingRegistry::new();
        r.register(Arc::new(Stub));
        assert_eq!(r.get("stub").unwrap().id(), "stub");
        assert!(r.get("missing").is_err());
    }

    #[tokio::test]
    async fn list_providers_returns_all() {
        let mut r = TracingRegistry::new();
        r.register(Arc::new(Stub));
        let infos = r.list_providers().await.unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "stub");
    }
}
```

- [ ] **Step 2: Re-enable `pub use registry::TracingRegistry;` in `tracing/mod.rs`**

- [ ] **Step 3: Verify `KlyntbotError::NotFound` exists**

```bash
grep -n 'NotFound' crates/common/src/lib.rs | head -3
```

If `NotFound` doesn't exist, use `KlyntbotError::Other(format!("..."))` or whatever the closest variant is. Inspect `crates/common/src/lib.rs` for the canonical error variants and adjust the registry's `ok_or_else` accordingly.

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p app-core -E 'test(tracing::registry)'
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/tracing/registry.rs crates/app-core/src/tracing/mod.rs
git commit -m "feat(app-core): TracingRegistry"
```

---

## Phase 8 — Kimi provider — categorize

### Task 8.1: Submodule files

**Files:**
- Modify: `crates/app-core/src/tracing/providers/kimi/mod.rs`
- Create: `crates/app-core/src/tracing/providers/kimi/categorize.rs`

- [ ] **Step 1: Update `providers/kimi/mod.rs`**

```rust
//! Kimi-cli TracingProvider implementation.

pub mod categorize;
pub mod discovery;
pub mod loader;
pub mod context_loader;
pub mod state_loader;
pub mod subagent_loader;
pub mod cache;
pub mod import;
pub mod stats;
mod provider_impl;

pub use provider_impl::KimiTracingProvider;
```

These submodules will fail to compile until added — comment out all but `categorize` for now:

```rust
pub mod categorize;
// pub mod discovery;
// pub mod loader;
// pub mod context_loader;
// pub mod state_loader;
// pub mod subagent_loader;
// pub mod cache;
// pub mod import;
// pub mod stats;
// mod provider_impl;
//
// pub use provider_impl::KimiTracingProvider;
```

- [ ] **Step 2: `cargo check -p app-core`** → success.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/mod.rs
git commit -m "feat(app-core): kimi tracing submodule scaffolding"
```

### Task 8.2: Implement `categorize::kimi_kind_to_category` (TDD)

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/categorize.rs`

- [ ] **Step 1: Write the failing tests**

```rust
//! Map Kimi raw event kinds to provider-agnostic SemanticCategory.

use crate::tracing::types::SemanticCategory;
use serde_json::Value;

pub fn kimi_kind_to_category(raw_kind: &str, payload: &Value) -> SemanticCategory {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn turn_begin() {
        assert_eq!(
            kimi_kind_to_category("TurnBegin", &json!({})),
            SemanticCategory::TurnBegin
        );
    }

    #[test]
    fn turn_end() {
        assert_eq!(
            kimi_kind_to_category("TurnEnd", &json!({})),
            SemanticCategory::TurnEnd
        );
    }

    #[test]
    fn step_begin() {
        assert_eq!(
            kimi_kind_to_category("StepBegin", &json!({"n":1})),
            SemanticCategory::StepBegin
        );
    }

    #[test]
    fn step_interrupted() {
        assert_eq!(
            kimi_kind_to_category("StepInterrupted", &json!({})),
            SemanticCategory::StepInterrupted
        );
    }

    #[test]
    fn content_part_think_is_thinking() {
        assert_eq!(
            kimi_kind_to_category("ContentPart", &json!({"type":"think","think":"..."})),
            SemanticCategory::Thinking
        );
    }

    #[test]
    fn content_part_text_is_assistant_text() {
        assert_eq!(
            kimi_kind_to_category("ContentPart", &json!({"type":"text","text":"hi"})),
            SemanticCategory::AssistantText
        );
    }

    #[test]
    fn tool_call() {
        assert_eq!(
            kimi_kind_to_category("ToolCall", &json!({})),
            SemanticCategory::ToolCall
        );
    }

    #[test]
    fn tool_call_part_is_stream() {
        assert_eq!(
            kimi_kind_to_category("ToolCallPart", &json!({})),
            SemanticCategory::ToolCallStream
        );
    }

    #[test]
    fn tool_result_ok() {
        assert_eq!(
            kimi_kind_to_category("ToolResult", &json!({"is_error":false})),
            SemanticCategory::ToolResult
        );
        // Default-undefined is_error treated as success.
        assert_eq!(
            kimi_kind_to_category("ToolResult", &json!({})),
            SemanticCategory::ToolResult
        );
    }

    #[test]
    fn tool_result_error() {
        assert_eq!(
            kimi_kind_to_category("ToolResult", &json!({"is_error":true})),
            SemanticCategory::Error
        );
    }

    #[test]
    fn status_update() {
        assert_eq!(
            kimi_kind_to_category("StatusUpdate", &json!({})),
            SemanticCategory::StatusUpdate
        );
    }

    #[test]
    fn subagent_event() {
        assert_eq!(
            kimi_kind_to_category("SubagentEvent", &json!({})),
            SemanticCategory::Subagent
        );
    }

    #[test]
    fn compaction_begin_end() {
        assert_eq!(
            kimi_kind_to_category("CompactionBegin", &json!({})),
            SemanticCategory::CompactionBegin
        );
        assert_eq!(
            kimi_kind_to_category("CompactionEnd", &json!({})),
            SemanticCategory::CompactionEnd
        );
    }

    #[test]
    fn unknown_is_other() {
        assert_eq!(
            kimi_kind_to_category("BrandNew2027", &json!({})),
            SemanticCategory::Other
        );
        assert_eq!(
            kimi_kind_to_category("metadata", &json!({})),
            SemanticCategory::Other
        );
    }

    #[test]
    fn content_part_unknown_subtype_is_other() {
        assert_eq!(
            kimi_kind_to_category("ContentPart", &json!({"type":"image"})),
            SemanticCategory::Other
        );
    }
}
```

- [ ] **Step 2: Run — should fail with `todo!()`**

```bash
cargo nextest run -p app-core -E 'test(categorize::tests)'
```

Expected: panics from `todo!`.

- [ ] **Step 3: Implement**

Replace `todo!()` body:

```rust
pub fn kimi_kind_to_category(raw_kind: &str, payload: &Value) -> SemanticCategory {
    match raw_kind {
        "TurnBegin" => SemanticCategory::TurnBegin,
        "TurnEnd" => SemanticCategory::TurnEnd,
        "StepBegin" => SemanticCategory::StepBegin,
        "StepInterrupted" => SemanticCategory::StepInterrupted,
        "ToolCall" => SemanticCategory::ToolCall,
        "ToolCallPart" => SemanticCategory::ToolCallStream,
        "StatusUpdate" => SemanticCategory::StatusUpdate,
        "SubagentEvent" => SemanticCategory::Subagent,
        "CompactionBegin" => SemanticCategory::CompactionBegin,
        "CompactionEnd" => SemanticCategory::CompactionEnd,
        "ToolResult" => {
            if payload.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                SemanticCategory::Error
            } else {
                SemanticCategory::ToolResult
            }
        }
        "ContentPart" => match payload.get("type").and_then(Value::as_str) {
            Some("think") => SemanticCategory::Thinking,
            Some("text") => SemanticCategory::AssistantText,
            _ => SemanticCategory::Other,
        },
        _ => SemanticCategory::Other,
    }
}
```

- [ ] **Step 4: Run — all should pass**

```bash
cargo nextest run -p app-core -E 'test(categorize::tests)'
```

Expected: 14 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/categorize.rs
git commit -m "feat(app-core): Kimi categorize mapping"
```

---

## Phase 9 — Kimi `WorkdirIndex` extension

### Task 9.1: Add `entries()` accessor to existing `WorkdirIndex`

**Files:**
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs`

- [ ] **Step 1: Append a method**

Find the `impl WorkdirIndex { … }` block and add:

```rust
    /// Snapshot of `(hash, cwd)` entries. Cheap clone for callers that
    /// want to iterate without holding the read lock.
    pub async fn entries(&self) -> Vec<(String, std::path::PathBuf)> {
        self.map
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
```

- [ ] **Step 2: Add a test**

In the `#[cfg(test)] mod tests` block at file bottom, add:

```rust
    #[tokio::test]
    async fn entries_returns_loaded_pairs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kimi.json");
        std::fs::write(
            &path,
            r#"{"work_dirs":[
                {"path":"/tmp/a","kaos":"local","last_session_id":null},
                {"path":"/tmp/b","kaos":"local","last_session_id":null}
            ]}"#,
        )
        .unwrap();
        let idx = WorkdirIndex::new();
        idx.refresh(&path).await.unwrap();
        let mut e = idx.entries().await;
        e.sort();
        assert_eq!(e.len(), 2);
    }
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p coding-ingest -E 'test(entries_returns_loaded_pairs)'
```

Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/workdir.rs
git commit -m "feat(coding-ingest): WorkdirIndex::entries accessor"
```

---

## Phase 10 — Kimi discovery

### Task 10.1: Create `discovery.rs` (TDD)

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/discovery.rs`
- Modify: `crates/app-core/Cargo.toml` (ensure `tokio = { features = ["fs"] }`, `tempfile` for tests)

- [ ] **Step 1: Verify deps**

```bash
grep -A 1 '\[dependencies\]' crates/app-core/Cargo.toml | head -2
grep 'tempfile\|tokio' crates/app-core/Cargo.toml
```

If `tempfile` is not under `[dev-dependencies]`, add `tempfile = "3"`.

- [ ] **Step 2: Uncomment `pub mod discovery;` in `providers/kimi/mod.rs`**

- [ ] **Step 3: Write the failing test**

```rust
//! Discover Kimi sessions from `~/.kimi/sessions/<hash>/<uuid>/`
//! plus imported sessions from `{klyntbot_data}/coding_memory/imported_wire/<uuid>/`.

use common::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub source_dir: PathBuf,
    pub hash: Option<String>,
    pub imported: bool,
}

pub async fn discover_sessions(
    kimi_root: &Path,
    imported_root: &Path,
) -> Result<Vec<DiscoveredSession>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn finds_native_sessions() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("kimi/sessions");
        let session_dir = kimi_root.join("hash1/sessA");
        fs::create_dir_all(&session_dir).await.unwrap();
        fs::write(session_dir.join("wire.jsonl"), "").await.unwrap();
        let imported_root = tmp.path().join("imported");
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "sessA");
        assert_eq!(out[0].hash.as_deref(), Some("hash1"));
        assert!(!out[0].imported);
    }

    #[tokio::test]
    async fn finds_imported_sessions() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("kimi/sessions");
        fs::create_dir_all(&kimi_root).await.unwrap();
        let imported_root = tmp.path().join("imported");
        let imp = imported_root.join("imp-uuid-1");
        fs::create_dir_all(&imp).await.unwrap();
        fs::write(imp.join("wire.jsonl"), "").await.unwrap();
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "imp-uuid-1");
        assert!(out[0].imported);
        assert_eq!(out[0].hash, None);
    }

    #[tokio::test]
    async fn skips_dirs_without_wire() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("kimi/sessions");
        fs::create_dir_all(kimi_root.join("hash1/sessA")).await.unwrap();
        let imported_root = tmp.path().join("imported");
        fs::create_dir_all(&imported_root).await.unwrap();
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn missing_kimi_root_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("nope");
        let imported_root = tmp.path().join("imported");
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert!(out.is_empty());
    }
}
```

- [ ] **Step 4: Run — should fail with `todo!`**

```bash
cargo nextest run -p app-core -E 'test(discovery::tests)'
```

- [ ] **Step 5: Implement**

```rust
pub async fn discover_sessions(
    kimi_root: &Path,
    imported_root: &Path,
) -> Result<Vec<DiscoveredSession>> {
    let mut out = Vec::new();
    scan_native(kimi_root, &mut out).await?;
    scan_imported(imported_root, &mut out).await?;
    Ok(out)
}

async fn scan_native(root: &Path, out: &mut Vec<DiscoveredSession>) -> Result<()> {
    let mut hashes = match tokio::fs::read_dir(root).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", root.display()))),
    };
    while let Some(hash_entry) = hashes.next_entry().await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !hash_entry.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
        let hash = hash_entry.file_name().to_string_lossy().to_string();
        if hash.starts_with('.') { continue; }
        let mut sessions = match tokio::fs::read_dir(hash_entry.path()).await {
            Ok(d) => d,
            Err(_) => continue,
        };
        while let Some(s) = sessions.next_entry().await
            .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
        {
            if !s.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
            let session_dir = s.path();
            if tokio::fs::metadata(session_dir.join("wire.jsonl")).await.is_err() { continue; }
            out.push(DiscoveredSession {
                session_id: s.file_name().to_string_lossy().to_string(),
                source_dir: session_dir,
                hash: Some(hash.clone()),
                imported: false,
            });
        }
    }
    Ok(())
}

async fn scan_imported(root: &Path, out: &mut Vec<DiscoveredSession>) -> Result<()> {
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", root.display()))),
    };
    while let Some(s) = entries.next_entry().await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !s.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
        if tokio::fs::metadata(s.path().join("wire.jsonl")).await.is_err() { continue; }
        out.push(DiscoveredSession {
            session_id: s.file_name().to_string_lossy().to_string(),
            source_dir: s.path(),
            hash: None,
            imported: true,
        });
    }
    Ok(())
}
```

- [ ] **Step 6: Run — all should pass**

```bash
cargo nextest run -p app-core -E 'test(discovery::tests)'
```

Expected: 4 passed.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/Cargo.toml crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi session discovery"
```

---

## Phase 11 — Kimi wire-file streaming + header stats

### Task 11.1: Implement `loader::load_session_events`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/loader.rs`

- [ ] **Step 1: Uncomment `pub mod loader;` in `providers/kimi/mod.rs`**

- [ ] **Step 2: Write the failing test**

```rust
//! Stream a Kimi `wire.jsonl` into `(Vec<TraceEvent>, HeaderStats)`.

use crate::tracing::providers::kimi::categorize::kimi_kind_to_category;
use crate::tracing::types::{HeaderStats, SemanticCategory, TraceEvent};
use coding_ingest::adapters::kimi_cli::wire_file::{collect_events, parse_line, WireLine};
use common::Result;
use jiff::Timestamp;
use serde_json::Value;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

const MAX_EVENTS_DEFAULT: usize = 50_000;

pub struct LoadedSession {
    pub events: Vec<TraceEvent>,
    pub stats: HeaderStats,
    pub truncated: bool,
    pub total_event_count: u64,
}

pub async fn load_session_events(
    wire_path: &Path,
    provider_id: &str,
    parent_subagent_id: Option<String>,
    agent_count_for_main: u32,
) -> Result<LoadedSession> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_wire() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/wire.jsonl");
        p
    }

    #[tokio::test]
    async fn loads_fixture_session() {
        let res = load_session_events(&fixture_wire(), "kimi", None, 1).await.unwrap();
        // 1 turn, 2 steps, 1 tool call (paired), 0 errors, 0 compactions, 1 agent (passed in).
        assert_eq!(res.stats.turn_count, 1);
        assert_eq!(res.stats.step_count, 2);
        assert_eq!(res.stats.tool_call_count, 1);
        assert_eq!(res.stats.error_count, 0);
        assert_eq!(res.stats.compaction_count, 0);
        assert_eq!(res.stats.agent_count, 1);
        assert_eq!(res.stats.total_input_tokens, 2490 + 9216 + 0);
        assert_eq!(res.stats.total_output_tokens, 106);
        // cache_hit_pct = 9216 / (9216 + 2490 + 0) * 100 = 78.72…
        assert!((res.stats.cache_hit_pct - 78.7).abs() < 0.5);
        assert!(!res.truncated);
        // metadata line is skipped, and other 9 events become TraceEvents.
        assert_eq!(res.events.len(), 9);
        // First event is TurnBegin with category set.
        assert_eq!(res.events[0].raw_kind, "TurnBegin");
        assert_eq!(res.events[0].category, SemanticCategory::TurnBegin);
        assert_eq!(res.events[0].seq, 0);
    }

    #[tokio::test]
    async fn missing_file_returns_empty_with_zero_stats() {
        let p = std::path::PathBuf::from("/nonexistent/wire.jsonl");
        let res = load_session_events(&p, "kimi", None, 0).await.unwrap();
        assert_eq!(res.events.len(), 0);
        assert_eq!(res.stats.turn_count, 0);
    }
}
```

- [ ] **Step 3: Run — fails with `todo!`**

```bash
cargo nextest run -p app-core -E 'test(loader::tests)'
```

- [ ] **Step 4: Implement**

```rust
pub async fn load_session_events(
    wire_path: &Path,
    provider_id: &str,
    parent_subagent_id: Option<String>,
    agent_count_for_main: u32,
) -> Result<LoadedSession> {
    let file = match tokio::fs::File::open(wire_path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedSession {
                events: vec![],
                stats: HeaderStats::default(),
                truncated: false,
                total_event_count: 0,
            });
        }
        Err(e) => {
            return Err(common::KlyntbotError::Storage(format!(
                "open {}: {e}",
                wire_path.display()
            )));
        }
    };

    let mut events: Vec<TraceEvent> = Vec::new();
    let mut stats = HeaderStats::default();
    stats.agent_count = agent_count_for_main;
    let mut total: u64 = 0;
    let mut truncated = false;

    let mut turn_index: Option<u32> = None;
    let mut step_index: Option<u32> = None;
    let mut first_ts: Option<Timestamp> = None;
    let mut last_ts: Option<Timestamp> = None;
    let mut last_status_token_usage: Option<Value> = None;

    let mut reader = BufReader::new(file).lines();
    while let Some(line) = reader.next_line().await
        .map_err(|e| common::KlyntbotError::Storage(format!("read line: {e}")))?
    {
        if line.trim().is_empty() { continue; }
        let parsed = match parse_line(&line) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "skipping unparseable wire line");
                continue;
            }
        };
        match parsed {
            WireLine::Metadata(_) => continue,
            WireLine::Record(rec) => {
                total += 1;
                if total as usize > MAX_EVENTS_DEFAULT {
                    truncated = true;
                    continue;
                }
                let ts = unix_seconds_to_ts(rec.timestamp);
                first_ts.get_or_insert(ts);
                last_ts = Some(ts);
                let collected = collect_events(&rec.message);
                for ev in collected {
                    let raw_kind = ev.kind.clone();
                    let payload = ev.payload.clone();
                    let category = kimi_kind_to_category(&raw_kind, &payload);

                    match category {
                        SemanticCategory::TurnBegin => {
                            turn_index = Some(turn_index.map_or(0, |v| v + 1));
                            step_index = None;
                            stats.turn_count += 1;
                        }
                        SemanticCategory::StepBegin => {
                            step_index = Some(step_index.map_or(0, |v| v + 1));
                            stats.step_count += 1;
                        }
                        SemanticCategory::ToolCall => stats.tool_call_count += 1,
                        SemanticCategory::Error => stats.error_count += 1,
                        SemanticCategory::CompactionBegin => stats.compaction_count += 1,
                        SemanticCategory::StatusUpdate => {
                            if let Some(usage) = payload.get("token_usage") {
                                last_status_token_usage = Some(usage.clone());
                            }
                        }
                        _ => {}
                    }

                    events.push(TraceEvent {
                        seq: events.len() as u64,
                        provider_id: provider_id.to_string(),
                        raw_kind,
                        payload,
                        occurred_at: ts,
                        category,
                        turn_index,
                        step_index,
                        parent_subagent_id: parent_subagent_id.clone(),
                    });
                }
            }
        }
    }

    if let (Some(start), Some(end)) = (first_ts, last_ts) {
        stats.total_duration_ms =
            (end.as_millisecond() - start.as_millisecond()).max(0) as u64;
    }
    if let Some(usage) = last_status_token_usage {
        let other = usage.get("input_other").and_then(Value::as_u64).unwrap_or(0);
        let cache_read = usage.get("input_cache_read").and_then(Value::as_u64).unwrap_or(0);
        let cache_creation = usage.get("input_cache_creation").and_then(Value::as_u64).unwrap_or(0);
        let output = usage.get("output").and_then(Value::as_u64).unwrap_or(0);
        stats.total_input_tokens = other + cache_read + cache_creation;
        stats.total_output_tokens = output;
        let denom = (other + cache_read + cache_creation) as f32;
        stats.cache_hit_pct = if denom > 0.0 {
            (cache_read as f32) / denom * 100.0
        } else {
            0.0
        };
    }

    Ok(LoadedSession { events, stats, truncated, total_event_count: total })
}

fn unix_seconds_to_ts(s: f64) -> Timestamp {
    let secs = s.trunc() as i64;
    let nanos = ((s.fract() * 1_000_000_000.0) as i64).clamp(0, 999_999_999) as i32;
    Timestamp::new(secs, nanos).unwrap_or_else(|_| Timestamp::now())
}
```

- [ ] **Step 5: Add `coding-ingest` as a dep of `app-core` if not already**

```bash
grep '^coding-ingest' crates/app-core/Cargo.toml
```

If absent, add:

```toml
coding-ingest = { path = "../coding-ingest" }
```

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p app-core -E 'test(loader::tests)'
```

Expected: 2 passed.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/Cargo.toml crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi wire-file loader with header stats"
```

---

## Phase 12 — Kimi context loader

### Task 12.1: Implement `context_loader::load_context`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/context_loader.rs`

- [ ] **Step 1: Uncomment `pub mod context_loader;` in `providers/kimi/mod.rs`**

- [ ] **Step 2: Write the failing test**

```rust
use crate::tracing::types::ContextMessage;
use common::Result;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn load_context(path: &Path) -> Result<Vec<ContextMessage>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/context.jsonl");
        p
    }

    #[tokio::test]
    async fn loads_fixture() {
        let msgs = load_context(&fixture()).await.unwrap();
        assert_eq!(msgs.len(), 5);
        assert_eq!(msgs[0].role, "_system_prompt");
        assert_eq!(msgs[1].role, "user");
        assert_eq!(msgs[3].role, "_usage");
        assert_eq!(msgs[0].index, 0);
        assert_eq!(msgs[4].index, 4);
    }

    #[tokio::test]
    async fn missing_returns_empty() {
        let msgs = load_context(&PathBuf::from("/nope")).await.unwrap();
        assert!(msgs.is_empty());
    }
}
```

- [ ] **Step 3: Run** → fails with `todo!`.

- [ ] **Step 4: Implement**

```rust
pub async fn load_context(path: &Path) -> Result<Vec<ContextMessage>> {
    let file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("open {}: {e}", path.display()))),
    };
    let mut out = Vec::new();
    let mut reader = BufReader::new(file).lines();
    let mut idx: u32 = 0;
    while let Some(line) = reader.next_line().await
        .map_err(|e| common::KlyntbotError::Storage(format!("read line: {e}")))?
    {
        if line.trim().is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "skipping unparseable context line");
                continue;
            }
        };
        let role = v.get("role").and_then(|x| x.as_str()).unwrap_or("").to_string();
        out.push(ContextMessage { index: idx, role, content: v });
        idx += 1;
    }
    Ok(out)
}
```

- [ ] **Step 5: Run** → 2 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi context loader"
```

---

## Phase 13 — Kimi state loader

### Task 13.1: Implement `state_loader::load_state`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/state_loader.rs`

- [ ] **Step 1: Uncomment `pub mod state_loader;`**

- [ ] **Step 2: Write the failing test**

```rust
use crate::tracing::types::{KimiTodo, SessionState};
use common::Result;
use std::path::Path;

pub async fn load_state(path: &Path) -> Result<SessionState> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/state.json");
        p
    }

    #[tokio::test]
    async fn loads_fixture_state() {
        let s = load_state(&fixture()).await.unwrap();
        assert_eq!(s.custom_title.as_deref(), Some("fixture session"));
        assert!(!s.plan_mode);
        assert!(!s.archived);
        assert_eq!(s.todos.len(), 2);
        assert_eq!(s.todos[0].title, "Done thing");
        assert_eq!(s.todos[0].status, "done");
    }

    #[tokio::test]
    async fn missing_returns_default() {
        let s = load_state(&PathBuf::from("/nope")).await.unwrap();
        assert!(s.custom_title.is_none());
        assert!(s.todos.is_empty());
    }
}
```

- [ ] **Step 3: Run** → fails.

- [ ] **Step 4: Implement**

```rust
pub async fn load_state(path: &Path) -> Result<SessionState> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SessionState::default()),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", path.display()))),
    };
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| common::KlyntbotError::Storage(format!("parse state.json: {e}")))?;
    let custom_title = raw.get("custom_title").and_then(|v| v.as_str()).map(|s| s.to_string());
    let plan_mode = raw.get("plan_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    let archived = raw.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
    let todos = raw.get("todos").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|t| {
            let title = t.get("title")?.as_str()?.to_string();
            let status = t.get("status")?.as_str()?.to_string();
            Some(KimiTodo { title, status })
        }).collect())
        .unwrap_or_default();
    Ok(SessionState { custom_title, plan_mode, archived, todos, raw })
}
```

- [ ] **Step 5: Run** → 2 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi state loader"
```

---

## Phase 14 — Kimi subagent loader

### Task 14.1: Implement `subagent_loader::list_subagents`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/subagent_loader.rs`

- [ ] **Step 1: Uncomment `pub mod subagent_loader;`**

- [ ] **Step 2: Write failing test**

```rust
use crate::tracing::types::SubagentSummary;
use common::Result;
use jiff::Timestamp;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn list_subagents(session_dir: &Path) -> Result<Vec<SubagentSummary>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn session_dir() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001");
        p
    }

    #[tokio::test]
    async fn lists_fixture_subagent() {
        let v = list_subagents(&session_dir()).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].agent_id, "sub-aaa");
        assert_eq!(v[0].subagent_type, "coder");
        assert_eq!(v[0].description.as_deref(), Some("Fixture subagent"));
        // 4 wire records (excluding the metadata header).
        assert_eq!(v[0].event_count, 4);
    }

    #[tokio::test]
    async fn no_subagents_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let v = list_subagents(tmp.path()).await.unwrap();
        assert!(v.is_empty());
    }
}
```

- [ ] **Step 3: Run** → fails.

- [ ] **Step 4: Implement**

```rust
pub async fn list_subagents(session_dir: &Path) -> Result<Vec<SubagentSummary>> {
    let dir = session_dir.join("subagents");
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", dir.display()))),
    };
    let mut out = Vec::new();
    while let Some(s) = entries.next_entry().await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !s.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
        let agent_dir = s.path();
        let meta_path = agent_dir.join("meta.json");
        let bytes = match tokio::fs::read(&meta_path).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let agent_id = v.get("agent_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let subagent_type = v.get("subagent_type").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let description = v.get("description").and_then(|x| x.as_str()).map(|s| s.to_string());
        let created_at = secs_to_ts(v.get("created_at").and_then(|x| x.as_f64()).unwrap_or(0.0));
        let updated_at = secs_to_ts(v.get("updated_at").and_then(|x| x.as_f64()).unwrap_or(0.0));
        let event_count = count_wire_records(&agent_dir.join("wire.jsonl")).await;
        out.push(SubagentSummary {
            agent_id, subagent_type, status, description, created_at, updated_at, event_count,
        });
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(out)
}

fn secs_to_ts(s: f64) -> Timestamp {
    let secs = s.trunc() as i64;
    let nanos = ((s.fract() * 1_000_000_000.0) as i64).clamp(0, 999_999_999) as i32;
    Timestamp::new(secs, nanos).unwrap_or_else(|_| Timestamp::now())
}

async fn count_wire_records(path: &Path) -> u32 {
    let f = match tokio::fs::File::open(path).await { Ok(f) => f, Err(_) => return 0 };
    let mut reader = BufReader::new(f).lines();
    let mut n: u32 = 0;
    while let Ok(Some(line)) = reader.next_line().await {
        let t = line.trim();
        if t.is_empty() { continue; }
        // Skip metadata header.
        if t.contains("\"type\":\"metadata\"") || t.starts_with("{\"type\":\"metadata\"") { continue; }
        n += 1;
    }
    n
}
```

- [ ] **Step 5: Run** → 2 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi subagent loader"
```

---

## Phase 15 — Kimi list-sessions cache

### Task 15.1: Implement `cache::SessionCache`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/cache.rs`

- [ ] **Step 1: Uncomment `pub mod cache;`**

- [ ] **Step 2: Write the cache**

```rust
//! Per-(wire-path, mtime) memo for `list_sessions` summary rows.

use crate::tracing::types::SessionSummary;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub mtime: SystemTime,
    pub summary: SessionSummary,
}

#[derive(Default)]
pub struct SessionCache {
    map: RwLock<HashMap<PathBuf, CacheEntry>>,
}

impl SessionCache {
    pub fn new() -> Self { Self::default() }

    pub async fn get(&self, key: &PathBuf, mtime: SystemTime) -> Option<SessionSummary> {
        let g = self.map.read().await;
        g.get(key).filter(|e| e.mtime == mtime).map(|e| e.summary.clone())
    }

    pub async fn put(&self, key: PathBuf, mtime: SystemTime, summary: SessionSummary) {
        let mut g = self.map.write().await;
        g.insert(key, CacheEntry { mtime, summary });
    }

    pub async fn clear(&self) {
        self.map.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracing::types::*;
    use jiff::Timestamp;

    fn dummy(s: &str) -> SessionSummary {
        SessionSummary {
            session_id: s.into(),
            provider_id: "kimi".into(),
            source_dir: PathBuf::new(),
            cwd: None,
            project_basename: None,
            custom_title: None,
            started_at: Timestamp::UNIX_EPOCH,
            last_event_at: Timestamp::UNIX_EPOCH,
            size_bytes: 0,
            turn_count: 0,
            step_count: 0,
            tool_call_count: 0,
            error_count: 0,
            subagent_count: 0,
            has_wire: true,
            has_context: false,
            imported: false,
        }
    }

    #[tokio::test]
    async fn miss_then_hit_then_invalidate() {
        let c = SessionCache::new();
        let key = PathBuf::from("/x");
        let mtime = SystemTime::now();
        assert!(c.get(&key, mtime).await.is_none());
        c.put(key.clone(), mtime, dummy("a")).await;
        assert!(c.get(&key, mtime).await.is_some());
        // Different mtime treated as miss.
        let later = mtime + std::time::Duration::from_secs(1);
        assert!(c.get(&key, later).await.is_none());
    }
}
```

- [ ] **Step 3: Run** → 1 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi list-sessions cache"
```

---

## Phase 16 — Kimi import

### Task 16.1: Implement `import::import_from_file`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/import.rs`

- [ ] **Step 1: Uncomment `pub mod import;`**

- [ ] **Step 2: Write failing test**

```rust
//! Copy a foreign `wire.jsonl` (and optional sibling `context.jsonl`/`state.json`)
//! into `{klyntbot_data}/coding_memory/imported_wire/<new_uuid>/`.

use common::Result;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub async fn import_from_file(imported_root: &Path, source_wire: &Path) -> Result<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn copies_wire_and_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("foreign");
        fs::create_dir_all(&src).await.unwrap();
        fs::write(src.join("wire.jsonl"), "{}").await.unwrap();
        fs::write(src.join("context.jsonl"), "{}").await.unwrap();
        fs::write(src.join("state.json"), "{}").await.unwrap();
        let imported = tmp.path().join("imported");
        let new_id = import_from_file(&imported, &src.join("wire.jsonl")).await.unwrap();
        let target = imported.join(&new_id);
        assert!(target.join("wire.jsonl").exists());
        assert!(target.join("context.jsonl").exists());
        assert!(target.join("state.json").exists());
        assert!(target.join("meta.json").exists());
    }

    #[tokio::test]
    async fn missing_source_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let imp = tmp.path().join("imp");
        let result = import_from_file(&imp, &tmp.path().join("nope.jsonl")).await;
        assert!(result.is_err());
    }
}
```

- [ ] **Step 3: Run** → fails.

- [ ] **Step 4: Implement**

```rust
pub async fn import_from_file(imported_root: &Path, source_wire: &Path) -> Result<String> {
    if !tokio::fs::try_exists(source_wire).await
        .map_err(|e| common::KlyntbotError::Storage(format!("stat {}: {e}", source_wire.display())))?
    {
        return Err(common::KlyntbotError::Storage(format!(
            "source wire missing: {}",
            source_wire.display()
        )));
    }
    let new_id = Uuid::new_v4().to_string();
    let target_dir = imported_root.join(&new_id);
    tokio::fs::create_dir_all(&target_dir).await
        .map_err(|e| common::KlyntbotError::Storage(format!("mkdir {}: {e}", target_dir.display())))?;
    tokio::fs::copy(source_wire, target_dir.join("wire.jsonl")).await
        .map_err(|e| common::KlyntbotError::Storage(format!("copy wire: {e}")))?;

    let parent = source_wire.parent().unwrap_or(Path::new("."));
    for sibling in &["context.jsonl", "state.json"] {
        let sp = parent.join(sibling);
        if tokio::fs::try_exists(&sp).await.unwrap_or(false) {
            let _ = tokio::fs::copy(&sp, target_dir.join(sibling)).await;
        }
    }

    let meta = serde_json::json!({
        "imported_at": jiff::Timestamp::now().to_string(),
        "original_path": source_wire.display().to_string(),
    });
    tokio::fs::write(target_dir.join("meta.json"), serde_json::to_vec_pretty(&meta).unwrap())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("write meta: {e}")))?;
    Ok(new_id)
}
```

- [ ] **Step 5: Add `uuid` to Cargo.toml if missing** (it's already used elsewhere — verify).

```bash
grep '^uuid' crates/app-core/Cargo.toml
```

- [ ] **Step 6: Run** → 2 passed.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/ crates/app-core/Cargo.toml
git commit -m "feat(app-core): Kimi tracing import"
```

---

## Phase 17 — Kimi stats aggregation

### Task 17.1: Implement `stats::aggregate`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/stats.rs`

- [ ] **Step 1: Uncomment `pub mod stats;`**

- [ ] **Step 2: Implement (no test yet — exercised end-to-end in Phase 20)**

```rust
//! Cross-session aggregations for the Statistics tab.

use crate::tracing::providers::kimi::loader::load_session_events;
use crate::tracing::providers::kimi::subagent_loader::list_subagents;
use crate::tracing::types::*;
use common::Result;
use std::collections::HashMap;
use std::path::Path;

pub async fn aggregate(
    sessions: &[SessionSummary],
) -> Result<StatsBundle> {
    let mut per_project: HashMap<String, ProjectTotals> = HashMap::new();
    let mut tool_usage: HashMap<String, ToolUsage> = HashMap::new();
    let mut subagent_types: HashMap<String, u32> = HashMap::new();
    let mut token_series: HashMap<String, TokenSeriesPoint> = HashMap::new();

    for s in sessions {
        let key = s.project_basename.clone().unwrap_or_else(|| "(unknown)".to_string());
        let entry = per_project.entry(key.clone()).or_insert_with(|| ProjectTotals {
            project_basename: key,
            cwd: s.cwd.clone().unwrap_or_default(),
            session_count: 0, turn_count: 0, tool_call_count: 0, error_count: 0,
            total_input_tokens: 0, total_output_tokens: 0,
        });
        entry.session_count += 1;
        entry.turn_count += s.turn_count;
        entry.tool_call_count += s.tool_call_count;
        entry.error_count += s.error_count;

        // Per-day token spend keyed by `last_event_at` day.
        let day = s.last_event_at.to_string()[..10].to_string();
        let p = token_series.entry(day.clone()).or_insert(TokenSeriesPoint {
            day, input_tokens: 0, output_tokens: 0,
        });

        // Re-load each session's wire to get tool-name + token totals.
        // (Cheap because of `SessionCache`; this is the cold path.)
        let wire = s.source_dir.join("wire.jsonl");
        let loaded = load_session_events(&wire, "kimi", None, 0).await?;
        p.input_tokens += loaded.stats.total_input_tokens;
        p.output_tokens += loaded.stats.total_output_tokens;
        entry.total_input_tokens += loaded.stats.total_input_tokens;
        entry.total_output_tokens += loaded.stats.total_output_tokens;

        for ev in &loaded.events {
            if ev.category == SemanticCategory::ToolCall {
                let tool = ev.payload.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unknown)").to_string();
                let u = tool_usage.entry(tool.clone()).or_insert(ToolUsage {
                    tool, call_count: 0, error_count: 0,
                });
                u.call_count += 1;
            }
            if ev.category == SemanticCategory::Error {
                if let Some(tool) = associated_tool(&loaded.events, ev.seq) {
                    if let Some(u) = tool_usage.get_mut(&tool) {
                        u.error_count += 1;
                    }
                }
            }
        }

        // Subagent type counts.
        let subs = list_subagents(&s.source_dir).await.unwrap_or_default();
        for sub in subs {
            *subagent_types.entry(sub.subagent_type).or_insert(0) += 1;
        }
    }

    let mut errors_by_tool: Vec<ErrorByTool> = tool_usage.values()
        .filter(|u| u.error_count > 0)
        .map(|u| ErrorByTool { tool: u.tool.clone(), error_count: u.error_count })
        .collect();
    errors_by_tool.sort_by(|a, b| b.error_count.cmp(&a.error_count));

    let mut tool_usage: Vec<ToolUsage> = tool_usage.into_values().collect();
    tool_usage.sort_by(|a, b| b.call_count.cmp(&a.call_count));

    let mut per_project: Vec<ProjectTotals> = per_project.into_values().collect();
    per_project.sort_by(|a, b| b.session_count.cmp(&a.session_count));

    let mut token_series: Vec<TokenSeriesPoint> = token_series.into_values().collect();
    token_series.sort_by(|a, b| a.day.cmp(&b.day));

    let mut subagent_types: Vec<SubagentTypeCount> = subagent_types.into_iter()
        .map(|(t, c)| SubagentTypeCount { subagent_type: t, count: c })
        .collect();
    subagent_types.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(StatsBundle { per_project, tool_usage, errors_by_tool, token_series, subagent_types })
}

fn associated_tool(events: &[TraceEvent], seq: u64) -> Option<String> {
    // Look back for the matching ToolCall by payload.id.
    let result = events.iter().find(|e| e.seq == seq)?;
    let id = result.payload.get("id").and_then(|v| v.as_str())?;
    let call = events.iter().rev().find(|e| {
        e.category == SemanticCategory::ToolCall &&
        e.payload.get("id").and_then(|v| v.as_str()) == Some(id)
    })?;
    call.payload.get("function")?.get("name")?.as_str().map(|s| s.to_string())
}
```

- [ ] **Step 3: `cargo check -p app-core`** → success.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/
git commit -m "feat(app-core): Kimi stats aggregation"
```

---

## Phase 18 — Kimi `TracingProvider` impl

### Task 18.1: Implement the trait on `KimiTracingProvider`

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/provider_impl.rs`

- [ ] **Step 1: Uncomment `mod provider_impl;` and `pub use provider_impl::KimiTracingProvider;`**

- [ ] **Step 2: Write `provider_impl.rs`**

```rust
use async_trait::async_trait;
use coding_ingest::adapters::kimi_cli::workdir::WorkdirIndex;
use common::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::tracing::provider::TracingProvider;
use crate::tracing::providers::kimi::{
    cache::SessionCache,
    context_loader::load_context,
    discovery::discover_sessions,
    import::import_from_file,
    loader::load_session_events,
    stats::aggregate,
    state_loader::load_state,
    subagent_loader::list_subagents,
};
use crate::tracing::types::*;
use jiff::Timestamp;

pub struct KimiTracingProvider {
    pub kimi_root: PathBuf,
    pub kimi_json: PathBuf,
    pub imported_root: PathBuf,
    pub workdirs: Arc<WorkdirIndex>,
    cache: SessionCache,
}

impl KimiTracingProvider {
    pub fn new(
        home_dir: &Path,
        klyntbot_data: &Path,
        workdirs: Arc<WorkdirIndex>,
    ) -> Self {
        Self {
            kimi_root: home_dir.join(".kimi/sessions"),
            kimi_json: home_dir.join(".kimi/kimi.json"),
            imported_root: klyntbot_data.join("coding_memory/imported_wire"),
            workdirs,
            cache: SessionCache::new(),
        }
    }

    /// Test-only constructor used by integration tests with explicit paths.
    #[doc(hidden)]
    pub fn new_for_test(
        kimi_root: PathBuf,
        kimi_json: PathBuf,
        imported_root: PathBuf,
        workdirs: Arc<WorkdirIndex>,
    ) -> Self {
        Self { kimi_root, kimi_json, imported_root, workdirs, cache: SessionCache::new() }
    }

    async fn refresh_workdirs(&self) {
        let _ = self.workdirs.refresh(&self.kimi_json).await;
    }

    async fn resolve_session_dir(&self, session_id: &str) -> Result<PathBuf> {
        // Native session: <kimi_root>/<hash>/<session_id>/
        let mut entries = match tokio::fs::read_dir(&self.kimi_root).await {
            Ok(d) => d,
            Err(_) => {
                let imp = self.imported_root.join(session_id);
                if imp.is_dir() { return Ok(imp); }
                return Err(common::KlyntbotError::NotFound(format!("session {session_id}")));
            }
        };
        while let Some(h) = entries.next_entry().await
            .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
        {
            if !h.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
            let candidate = h.path().join(session_id);
            if candidate.is_dir() { return Ok(candidate); }
        }
        let imp = self.imported_root.join(session_id);
        if imp.is_dir() { return Ok(imp); }
        Err(common::KlyntbotError::NotFound(format!("session {session_id}")))
    }
}

#[async_trait]
impl TracingProvider for KimiTracingProvider {
    fn id(&self) -> &'static str { "kimi" }
    fn display_name(&self) -> &'static str { "Kimi" }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.refresh_workdirs().await;
        let discovered = discover_sessions(&self.kimi_root, &self.imported_root).await?;
        let entries = self.workdirs.entries().await;
        let hash_to_cwd: std::collections::HashMap<String, PathBuf> = entries.into_iter().collect();

        let mut out = Vec::with_capacity(discovered.len());
        for d in discovered {
            let wire = d.source_dir.join("wire.jsonl");
            let mtime = match tokio::fs::metadata(&wire).await {
                Ok(m) => m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            };
            if let Some(cached) = self.cache.get(&wire, mtime).await {
                out.push(cached);
                continue;
            }
            let loaded = load_session_events(&wire, "kimi", None, 0).await?;
            let subs = list_subagents(&d.source_dir).await.unwrap_or_default();
            let context_path = d.source_dir.join("context.jsonl");
            let has_context = tokio::fs::try_exists(&context_path).await.unwrap_or(false);
            let state_path = d.source_dir.join("state.json");
            let state = load_state(&state_path).await.unwrap_or_default();
            let custom_title = state.custom_title.clone().or_else(|| {
                loaded.events.iter().find(|e| e.category == SemanticCategory::TurnBegin)
                    .and_then(|e| extract_user_text(&e.payload))
            });
            let cwd = d.hash.as_ref().and_then(|h| hash_to_cwd.get(h).cloned());
            let project_basename = cwd.as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string());
            let context_size = tokio::fs::metadata(&context_path).await.map(|m| m.len()).unwrap_or(0);
            let size_bytes = tokio::fs::metadata(&wire).await.map(|m| m.len()).unwrap_or(0) + context_size;

            let summary = SessionSummary {
                session_id: d.session_id.clone(),
                provider_id: "kimi".into(),
                source_dir: d.source_dir.clone(),
                cwd,
                project_basename,
                custom_title,
                started_at: loaded.events.first().map(|e| e.occurred_at).unwrap_or(Timestamp::UNIX_EPOCH),
                last_event_at: loaded.events.last().map(|e| e.occurred_at).unwrap_or(Timestamp::UNIX_EPOCH),
                size_bytes,
                turn_count: loaded.stats.turn_count,
                step_count: loaded.stats.step_count,
                tool_call_count: loaded.stats.tool_call_count,
                error_count: loaded.stats.error_count,
                subagent_count: subs.len() as u32,
                has_wire: true,
                has_context,
                imported: d.imported,
            };
            self.cache.put(wire, mtime, summary.clone()).await;
            out.push(summary);
        }
        out.sort_by(|a, b| b.last_event_at.cmp(&a.last_event_at));
        Ok(out)
    }

    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail> {
        let dir = self.resolve_session_dir(session_id).await?;
        let (wire_path, parent) = match &scope {
            Scope::Main => (dir.join("wire.jsonl"), None),
            Scope::Subagent { agent_id } => (
                dir.join("subagents").join(agent_id).join("wire.jsonl"),
                Some(agent_id.clone()),
            ),
        };
        let agent_count = list_subagents(&dir).await.unwrap_or_default().len() as u32;
        let loaded = load_session_events(&wire_path, "kimi", parent, agent_count).await?;
        Ok(SessionDetail {
            session_id: session_id.to_string(),
            provider_id: "kimi".into(),
            scope,
            stats: loaded.stats,
            events: loaded.events,
            truncated: loaded.truncated,
            total_event_count: loaded.total_event_count,
        })
    }

    async fn load_context(&self, session_id: &str, scope: Scope) -> Result<Vec<ContextMessage>> {
        let dir = self.resolve_session_dir(session_id).await?;
        let path = match scope {
            Scope::Main => dir.join("context.jsonl"),
            Scope::Subagent { agent_id } => dir.join("subagents").join(&agent_id).join("context.jsonl"),
        };
        load_context(&path).await
    }

    async fn load_state(&self, session_id: &str) -> Result<SessionState> {
        let dir = self.resolve_session_dir(session_id).await?;
        load_state(&dir.join("state.json")).await
    }

    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>> {
        let dir = self.resolve_session_dir(session_id).await?;
        list_subagents(&dir).await
    }

    async fn import_from_file(&self, path: &Path) -> Result<String> {
        import_from_file(&self.imported_root, path).await
    }

    async fn open_dir(&self, session_id: &str) -> Result<PathBuf> {
        self.resolve_session_dir(session_id).await
    }

    async fn stats(&self) -> Result<StatsBundle> {
        let sessions = self.list_sessions().await?;
        aggregate(&sessions).await
    }
}

fn extract_user_text(payload: &serde_json::Value) -> Option<String> {
    if let Some(s) = payload.get("user_input").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    payload.get("user_input").and_then(|v| v.as_array())
        .and_then(|arr| arr.iter().find_map(|p| {
            if p.get("type")?.as_str()? == "text" {
                Some(p.get("text")?.as_str()?.to_string())
            } else { None }
        }))
}
```

- [ ] **Step 3: `cargo check -p app-core`** → success.

- [ ] **Step 4: Add an integration test exercising the trait against the fixture**

Create `crates/app-core/tests/kimi_tracing.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;

use app_core::tracing::{
    providers::kimi::KimiTracingProvider, Scope, TracingProvider,
};
use coding_ingest::adapters::kimi_cli::workdir::WorkdirIndex;

fn fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/kimi");
    let kimi_root = p.join("sessions");
    let kimi_json = p.join("kimi.json");
    let imported = tempfile::tempdir().unwrap().into_path();
    (kimi_root, kimi_json, imported)
}

#[tokio::test]
async fn kimi_provider_end_to_end() {
    let (kimi_root, kimi_json, imported) = fixture_paths();
    let widx = Arc::new(WorkdirIndex::new());
    widx.refresh(&kimi_json).await.unwrap();
    let provider = KimiTracingProvider::new_for_test(kimi_root, kimi_json, imported, widx);

    let sessions = provider.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    assert_eq!(s.session_id, "sess-fixture-001");
    assert_eq!(s.turn_count, 1);

    let detail = provider.load_session(&s.session_id, Scope::Main).await.unwrap();
    assert_eq!(detail.events.len(), 9);

    let ctx = provider.load_context(&s.session_id, Scope::Main).await.unwrap();
    assert_eq!(ctx.len(), 5);

    let state = provider.load_state(&s.session_id).await.unwrap();
    assert_eq!(state.todos.len(), 2);

    let subs = provider.list_subagents(&s.session_id).await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].agent_id, "sub-aaa");

    let sub_detail = provider.load_session(
        &s.session_id,
        Scope::Subagent { agent_id: "sub-aaa".into() },
    ).await.unwrap();
    assert_eq!(sub_detail.events.len(), 4);
}
```

- [ ] **Step 5: Run integration test**

```bash
cargo nextest run -p app-core -E 'test(kimi_provider_end_to_end)'
```

Expected: 1 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/ crates/app-core/tests/kimi_tracing.rs
git commit -m "feat(app-core): KimiTracingProvider trait impl + integration test"
```

---

## Phase 19 — AppCore wiring

### Task 19.1: Add `TracingRegistry` to `AppCore`

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/` (find the init module that runs during setup)

- [ ] **Step 1: Add a field to `AppCore`**

Locate the `AppCore` struct in `crates/app-core/src/state.rs` and add a field (alphabetical-ish placement among existing fields):

```rust
    pub(crate) tracing_registry: std::sync::Arc<crate::tracing::TracingRegistry>,
```

Update the `AppCore::new(...)` constructor (or builder) to take/build the registry. If the constructor uses a builder pattern, add a method `with_tracing_registry(...)`. Mirror the pattern of existing fields (e.g. how `mirror_facade` is wired).

- [ ] **Step 2: Add an accessor**

In the `impl AppCore { … }` block:

```rust
    pub fn tracing_registry(&self) -> &std::sync::Arc<crate::tracing::TracingRegistry> {
        &self.tracing_registry
    }
```

- [ ] **Step 3: Build the registry in init**

Find the file under `crates/app-core/src/init/` that constructs `AppCore`'s subsystems (likely `init/mod.rs` or `init/coding_memory.rs`). Add a function:

```rust
use std::sync::Arc;
use coding_ingest::adapters::kimi_cli::workdir::WorkdirIndex;
use crate::tracing::{TracingRegistry, providers::kimi::KimiTracingProvider};

pub async fn build_tracing_registry(
    home_dir: &std::path::Path,
    klyntbot_data: &std::path::Path,
) -> Arc<TracingRegistry> {
    let widx = Arc::new(WorkdirIndex::new());
    let _ = widx.refresh(&home_dir.join(".kimi/kimi.json")).await;
    let mut registry = TracingRegistry::new();
    registry.register(Arc::new(KimiTracingProvider::new(home_dir, klyntbot_data, widx)));
    Arc::new(registry)
}
```

Wire this into the AppCore construction site, passing the appropriate home/data paths (the `Config` struct already exposes `data_dir()` — see other init code for the home-dir convention; if no home is exposed, use `dirs::home_dir().expect("home dir")` from the `dirs` crate, which is already a workspace dep).

- [ ] **Step 4: Build**

```bash
cargo check -p app-core
```

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/
git commit -m "feat(app-core): wire TracingRegistry into AppCore"
```

### Task 19.2: Add `AppCore::tracing_*` handler methods

**Files:**
- Create: `crates/app-core/src/tracing_handlers.rs`
- Modify: `crates/app-core/src/lib.rs` (add `mod tracing_handlers;`)

- [ ] **Step 1: Write `tracing_handlers.rs`**

```rust
use crate::state::AppCore;
use crate::tracing::types::*;
use desktop_shared::errors::ApiError;
use std::path::PathBuf;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_list_providers(&self) -> Result<Vec<ProviderInfo>, ApiError> {
        self.tracing_registry.list_providers().await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_list_sessions(&self, provider_id: String)
        -> Result<Vec<SessionSummary>, ApiError>
    {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .list_sessions().await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_session(
        &self, provider_id: String, session_id: String, scope: Scope,
    ) -> Result<SessionDetail, ApiError> {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .load_session(&session_id, scope).await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_context(
        &self, provider_id: String, session_id: String, scope: Scope,
    ) -> Result<Vec<ContextMessage>, ApiError> {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .load_context(&session_id, scope).await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_state(
        &self, provider_id: String, session_id: String,
    ) -> Result<SessionState, ApiError> {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .load_state(&session_id).await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_list_subagents(
        &self, provider_id: String, session_id: String,
    ) -> Result<Vec<SubagentSummary>, ApiError> {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .list_subagents(&session_id).await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_import(
        &self, provider_id: String, file_path: String,
    ) -> Result<String, ApiError> {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .import_from_file(std::path::Path::new(&file_path)).await.map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_open_dir(
        &self, provider_id: String, session_id: String,
    ) -> Result<PathBuf, ApiError> {
        let path = self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .open_dir(&session_id).await.map_err(ApiError::from)?;
        // Open in Finder via standard `open` command.
        let _ = tokio::process::Command::new("open").arg(&path).spawn();
        Ok(path)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_stats(&self, provider_id: String) -> Result<StatsBundle, ApiError> {
        self.tracing_registry.get(&provider_id).map_err(ApiError::from)?
            .stats().await.map_err(ApiError::from)
    }
}
```

- [ ] **Step 2: Add `mod tracing_handlers;` to `lib.rs`**

- [ ] **Step 3: Build**

```bash
cargo check -p app-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/tracing_handlers.rs crates/app-core/src/lib.rs
git commit -m "feat(app-core): tracing_* handler methods on AppCore"
```

---

## Phase 20 — Desktop-shared DTOs

### Task 20.1: Mirror types with `specta::Type` derive

**Files:**
- Create: `crates/desktop-shared/src/commands/tracing.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`

The simplest path: re-export the `app-core` types as the wire shape. `specta::Type` requires the type be defined in a crate that depends on `specta` — so we need to either (a) add `specta` as a dep of `app-core` and derive there, or (b) define mirror types in `desktop-shared`.

**Decision: add `specta` derive to the `app-core` types directly** (cleaner, no duplication). This is consistent with how `desktop-shared` already exposes types — many existing DTOs derive specta directly.

- [ ] **Step 1: Add `specta` dep to `app-core`**

```bash
grep '^specta' crates/app-core/Cargo.toml
```

If absent, in `Cargo.toml`:

```toml
specta = { version = "2.0.0-rc", features = ["serde", "serde_json"] }
```

Match the version other crates use — check `crates/desktop-shared/Cargo.toml`.

- [ ] **Step 2: Add `specta::Type` derive to all tracing DTOs**

In `crates/app-core/src/tracing/types.rs`, add `specta::Type` to every struct/enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
```

Apply to: `Scope`, `SemanticCategory`, `TraceEvent`, `HeaderStats`, `SessionSummary`, `SessionDetail`, `ContextMessage`, `KimiTodo`, `SessionState`, `SubagentSummary`, `ProjectTotals`, `ToolUsage`, `ErrorByTool`, `TokenSeriesPoint`, `SubagentTypeCount`, `StatsBundle`, `ProviderInfo`.

For `serde_json::Value` fields, use the existing helper in desktop-shared:

```rust
#[specta(type = desktop_shared::specta_helpers::JsonValue)]
pub payload: serde_json::Value,
```

This requires `desktop-shared` as a dep of `app-core` (already true).

- [ ] **Step 3: Verify**

```bash
cargo check -p app-core
```

Expected: success.

- [ ] **Step 4: Create `desktop-shared/src/commands/tracing.rs` as a re-export shim**

```rust
//! Re-exports of `app-core` tracing DTOs for the FE bindings layer.

pub use app_core::tracing::types::{
    ContextMessage, ErrorByTool, HeaderStats, KimiTodo, ProjectTotals, ProviderInfo, Scope,
    SemanticCategory, SessionDetail, SessionState, SessionSummary, StatsBundle, SubagentSummary,
    SubagentTypeCount, TokenSeriesPoint, ToolUsage, TraceEvent,
};
```

Add to `crates/desktop-shared/src/commands/mod.rs`:

```rust
pub mod tracing;
```

- [ ] **Step 5: Build**

```bash
cargo check -p desktop-shared
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/Cargo.toml crates/app-core/src/tracing/types.rs crates/desktop-shared/
git commit -m "feat(desktop-shared): expose tracing DTOs with specta::Type"
```

---

## Phase 21 — Tauri commands

### Task 21.1: Write the command adapters

**Files:**
- Create: `crates/desktop/src/commands/tracing.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

- [ ] **Step 1: Write the file**

```rust
//! Tauri adapters for the tracing module.

use desktop_macros::klynt_command;
use desktop_shared::commands::tracing::*;
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn tracing_list_providers() -> Vec<ProviderInfo> {
    state.tracing_list_providers().await
}

#[klynt_command]
pub async fn tracing_list_sessions(provider_id: String) -> Vec<SessionSummary> {
    state.tracing_list_sessions(provider_id).await
}

#[klynt_command]
pub async fn tracing_load_session(
    provider_id: String,
    session_id: String,
    scope: Scope,
) -> SessionDetail {
    state.tracing_load_session(provider_id, session_id, scope).await
}

#[klynt_command]
pub async fn tracing_load_context(
    provider_id: String,
    session_id: String,
    scope: Scope,
) -> Vec<ContextMessage> {
    state.tracing_load_context(provider_id, session_id, scope).await
}

#[klynt_command]
pub async fn tracing_load_state(
    provider_id: String,
    session_id: String,
) -> SessionState {
    state.tracing_load_state(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_list_subagents(
    provider_id: String,
    session_id: String,
) -> Vec<SubagentSummary> {
    state.tracing_list_subagents(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_import(provider_id: String, file_path: String) -> String {
    state.tracing_import(provider_id, file_path).await
}

#[klynt_command]
pub async fn tracing_open_dir(provider_id: String, session_id: String) -> std::path::PathBuf {
    state.tracing_open_dir(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_stats(provider_id: String) -> StatsBundle {
    state.tracing_stats(provider_id).await
}
```

- [ ] **Step 2: Add `pub mod tracing;` to `crates/desktop/src/commands/mod.rs`**

- [ ] **Step 3: Build**

```bash
cargo check -p desktop
```

Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/commands/tracing.rs crates/desktop/src/commands/mod.rs
git commit -m "feat(desktop): tracing_* Tauri command adapters"
```

---

## Phase 22 — Specta builder registration + bindings regen

### Task 22.1: Register commands

**Files:**
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Add command paths to `klynt_collect_commands![…]`**

Find the existing macro invocation and add:

```rust
desktop_macros::klynt_collect_commands![
    // … existing entries …
    crate::commands::tracing::tracing_list_providers,
    crate::commands::tracing::tracing_list_sessions,
    crate::commands::tracing::tracing_load_session,
    crate::commands::tracing::tracing_load_context,
    crate::commands::tracing::tracing_load_state,
    crate::commands::tracing::tracing_list_subagents,
    crate::commands::tracing::tracing_import,
    crate::commands::tracing::tracing_open_dir,
    crate::commands::tracing::tracing_stats,
];
```

- [ ] **Step 2: Build**

```bash
cargo check -p desktop
```

- [ ] **Step 3: Regenerate bindings**

```bash
cargo tauri dev
```

Wait until you see Vite + Tauri start. Then `Ctrl+C`. Bindings should now exist in `desktop-ui/src/bindings.ts`.

```bash
grep 'tracingListSessions\|tracingLoadSession' desktop-ui/src/bindings.ts | head
```

Expected: matches showing the new commands.

- [ ] **Step 4: Run drift tests**

```bash
cargo nextest run -p desktop -E 'test(bindings_are_current) or test(registration_drift)'
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/specta_builder.rs desktop-ui/src/bindings.ts
git commit -m "feat(desktop): register tracing_* commands and regen bindings"
```

---

## Phase 23 — Frontend tracing module skeleton + CSS

### Task 23.1: Create the directory tree + types + CSS

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/types.ts`
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/providers.ts`
- Create: `desktop-ui/src/styles/tracing.css`
- Modify: `desktop-ui/src/styles/index.css`

- [ ] **Step 1: Write `types.ts`**

```ts
export type {
  ContextMessage,
  ErrorByTool,
  HeaderStats,
  KimiTodo,
  ProjectTotals,
  ProviderInfo,
  Scope,
  SemanticCategory,
  SessionDetail,
  SessionState,
  SessionSummary,
  StatsBundle,
  SubagentSummary,
  SubagentTypeCount,
  TokenSeriesPoint,
  ToolUsage,
  TraceEvent,
} from "@/bindings";

export type ViewMode = "chart" | "events" | "timeline" | "decisions";
export type DetailTab = "wireEvents" | "contextMessages" | "state" | "dual" | "agents";
```

- [ ] **Step 2: Write `providers.ts`**

```ts
import type { ProviderInfo } from "./types";

export interface ProviderChrome {
  id: string;
  displayName: string;
  /** CSS color value (variable or hex) for chip + header accents. */
  accent: string;
}

const REGISTRY: Record<string, ProviderChrome> = {
  kimi: { id: "kimi", displayName: "Kimi", accent: "var(--accent-violet, #7c5cff)" },
  // Future: claudeCode, codex, openCode, klyntCli.
};

export function chromeFor(providerId: string): ProviderChrome {
  return REGISTRY[providerId] ?? { id: providerId, displayName: providerId, accent: "var(--accent-blue)" };
}
```

- [ ] **Step 3: Write `tracing.css` (skeleton)**

```css
/* Tracing — Plugins → Coding Memory → Tracing.
 * Scaled BEM-ish names rooted at `.tracing-`.
 * All sizes use design tokens; never raw px font-sizes.
 */

.tracing-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  font-size: var(--fs-base);
}

.tracing-tabs {
  display: flex;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
}

.tracing-tab {
  background: transparent;
  border: 0;
  color: var(--text-secondary);
  cursor: pointer;
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--fs-base);
}

.tracing-tab--active {
  color: var(--text-primary);
  background: var(--bg-elevated);
}

.tracing-body {
  flex: 1 1 auto;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
```

(More rules added as components land in later phases.)

- [ ] **Step 4: Add `@import "./tracing.css";` to `desktop-ui/src/styles/index.css`**

Open `desktop-ui/src/styles/index.css` and append a line near the other `@import`s:

```css
@import "./tracing.css";
```

- [ ] **Step 5: Verify build**

```bash
cd desktop-ui && bun run typecheck
```

Expected: success.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ desktop-ui/src/styles/
git commit -m "feat(ui): tracing module types + CSS skeleton"
```

---

## Phase 24 — Frontend hooks

### Task 24.1: `useTracingSessions` hook

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/useTracingSessions.ts`

- [ ] **Step 1: Write**

```ts
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SessionSummary } from "./types";

interface State {
  sessions: SessionSummary[];
  loading: boolean;
  error: string | null;
}

export function useTracingSessions(providerId: string, refreshKey: number = 0): State {
  const [state, setState] = useState<State>({ sessions: [], loading: true, error: null });

  useEffect(() => {
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, error: null }));
    invoke<SessionSummary[]>("tracing_list_sessions", { providerId })
      .then((rows) => {
        if (cancelled) return;
        setState({ sessions: rows, loading: false, error: null });
      })
      .catch((err) => {
        if (cancelled) return;
        setState({ sessions: [], loading: false, error: String(err?.message ?? err) });
      });
    return () => { cancelled = true; };
  }, [providerId, refreshKey]);

  return state;
}
```

- [ ] **Step 2: Verify the invoke client export**

```bash
grep -n 'export\|invoke' desktop-ui/src/api/client.ts | head -10
```

If `@/api/client` exports something other than `invoke` directly, adjust the import. Should be a thin re-export of `@tauri-apps/api/core`.

- [ ] **Step 3: `bun run typecheck`** → success.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/useTracingSessions.ts
git commit -m "feat(ui): useTracingSessions hook"
```

### Task 24.2: `useTracingSession` hook

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/useTracingSession.ts`

- [ ] **Step 1: Write**

```ts
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { Scope, SessionDetail } from "./types";

interface State {
  detail: SessionDetail | null;
  loading: boolean;
  error: string | null;
}

export function useTracingSession(
  providerId: string,
  sessionId: string | null,
  scope: Scope,
  refreshKey: number = 0,
): State {
  const [state, setState] = useState<State>({ detail: null, loading: false, error: null });

  useEffect(() => {
    if (!sessionId) {
      setState({ detail: null, loading: false, error: null });
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, error: null }));
    invoke<SessionDetail>("tracing_load_session", { providerId, sessionId, scope })
      .then((d) => {
        if (cancelled) return;
        setState({ detail: d, loading: false, error: null });
      })
      .catch((err) => {
        if (cancelled) return;
        setState({ detail: null, loading: false, error: String(err?.message ?? err) });
      });
    return () => { cancelled = true; };
  }, [providerId, sessionId, JSON.stringify(scope), refreshKey]);

  return state;
}
```

- [ ] **Step 2: `bun run typecheck`** → success.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/useTracingSession.ts
git commit -m "feat(ui): useTracingSession hook"
```

---

## Phase 25 — Frontend: TracingTab + plug into CodingMemoryPlugin

### Task 25.1: Skeleton TracingTab

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/TracingTab.tsx`

- [ ] **Step 1: Write**

```tsx
import { useState } from "react";
import type { ProviderInfo } from "./types";
import { chromeFor } from "./providers";

type RootTab = "sessions" | "statistics";

interface ViewState {
  view: "list" | "detail";
  sessionId: string | null;
}

export function TracingTab() {
  const [providerId, setProviderId] = useState<string>("kimi");
  const [rootTab, setRootTab] = useState<RootTab>("sessions");
  const [viewState, setViewState] = useState<ViewState>({ view: "list", sessionId: null });

  // Placeholder body — phases 26-32 fill this in.
  return (
    <div className="tracing-root">
      <div className="tracing-tabs">
        <button
          type="button"
          className={"tracing-tab" + (rootTab === "sessions" ? " tracing-tab--active" : "")}
          onClick={() => setRootTab("sessions")}
        >
          Sessions
        </button>
        <button
          type="button"
          className={"tracing-tab" + (rootTab === "statistics" ? " tracing-tab--active" : "")}
          onClick={() => setRootTab("statistics")}
        >
          Statistics
        </button>
      </div>
      <div className="tracing-body">
        {/* Replaced by Phase 29 (SessionsListView) and Phase 32 (SessionDetailView). */}
        <div style={{ padding: "var(--space-3)", color: "var(--text-secondary)" }}>
          Tracing placeholder — provider <strong>{chromeFor(providerId).displayName}</strong> · tab <strong>{rootTab}</strong> · view <strong>{viewState.view}</strong>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: `bun run typecheck`** → success.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/TracingTab.tsx
git commit -m "feat(ui): TracingTab skeleton"
```

### Task 25.2: Plug into CodingMemoryPlugin

**Files:**
- Modify: `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx`

- [ ] **Step 1: Update the `secondaryTab` union and add the Tracing button + branch**

Locate the `useState<"sessions" | "reforge">("sessions")` line and change to:

```tsx
const [secondaryTab, setSecondaryTab] = useState<"sessions" | "tracing" | "reforge">("sessions");
```

In the `cm-plugin__secondary-tabs` block, add the Tracing button between Sessions and Reforge:

```tsx
<button
  type="button"
  className={"cm-plugin__sec-tab" + (secondaryTab === "tracing" ? " cm-plugin__sec-tab--active" : "")}
  onClick={() => setSecondaryTab("tracing")}
>
  Tracing
</button>
```

In the body conditional, replace the existing `secondaryTab === "sessions" ? … : <Reforge…/>` ternary with a switch:

```tsx
{secondaryTab === "sessions" ? (
  <>
    <aside className="cm-plugin__sidebar">
      <SessionList sessions={sessions} selectedId={selectedId} onSelect={setSelectedId} loading={loading} />
    </aside>
    <main className="cm-plugin__main">
      {selectedId
        ? <WireViewer sessionId={selectedId} refreshKey={refreshKey} />
        : <div className="cm-state cm-state--empty">Select a session to inspect.</div>}
    </main>
  </>
) : secondaryTab === "tracing" ? (
  <main className="cm-plugin__main">
    <TracingTab />
  </main>
) : (
  <main className="cm-plugin__main">
    <ReforgeCycleDiff />
  </main>
)}
```

Add the import:

```tsx
import { TracingTab } from "./tracing/TracingTab";
```

- [ ] **Step 2: `bun run typecheck`** → success.

- [ ] **Step 3: `bun run lint`** → success.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx
git commit -m "feat(ui): wire TracingTab into Coding Memory page"
```

---

## Phase 26 — Frontend: SessionsToolbar

### Task 26.1: SessionsToolbar component

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/SessionsToolbar.tsx`

- [ ] **Step 1: Write**

```tsx
import type { Dispatch, SetStateAction } from "react";

export type SortKey = "recent" | "created" | "size" | "errors";

interface Props {
  sessionCount: number;
  search: string;
  onSearchChange: (s: string) => void;
  sort: SortKey;
  onSortChange: (s: SortKey) => void;
  importedOnly: boolean;
  onImportedToggle: () => void;
  group: boolean;
  onGroupToggle: () => void;
  layout: "grid" | "list";
  onLayoutToggle: () => void;
  onImport: () => void;
}

export function SessionsToolbar(p: Props) {
  return (
    <div className="tracing-toolbar">
      <div className="tracing-toolbar__search">
        <input
          type="search"
          placeholder="Search sessions…"
          value={p.search}
          onChange={(e) => p.onSearchChange(e.target.value)}
        />
      </div>
      <select
        className="tracing-toolbar__sort"
        value={p.sort}
        onChange={(e) => p.onSortChange(e.target.value as SortKey)}
      >
        <option value="recent">Recent</option>
        <option value="created">Created</option>
        <option value="size">Size</option>
        <option value="errors">Errors</option>
      </select>
      <button
        type="button"
        className={"tracing-toolbar__btn" + (p.importedOnly ? " tracing-toolbar__btn--on" : "")}
        onClick={p.onImportedToggle}
      >Imported</button>
      <button
        type="button"
        className={"tracing-toolbar__btn" + (p.group ? " tracing-toolbar__btn--on" : "")}
        onClick={p.onGroupToggle}
      >Group</button>
      <button
        type="button"
        className="tracing-toolbar__btn"
        onClick={p.onLayoutToggle}
        title={p.layout === "grid" ? "Switch to list" : "Switch to grid"}
      >{p.layout === "grid" ? "List" : "Grid"}</button>
      <button type="button" className="tracing-toolbar__btn" onClick={p.onImport}>Import</button>
      <span className="tracing-toolbar__count">{p.sessionCount} sessions</span>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS to `tracing.css`**

```css
.tracing-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
}
.tracing-toolbar__search input {
  width: 240px;
  padding: var(--space-1) var(--space-2);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--fs-base);
}
.tracing-toolbar__sort {
  padding: var(--space-1) var(--space-2);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--fs-base);
}
.tracing-toolbar__btn {
  padding: var(--space-1) var(--space-2);
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  cursor: pointer;
  font-size: var(--fs-base);
}
.tracing-toolbar__btn--on {
  background: var(--bg-elevated);
  border-color: var(--border-strong);
}
.tracing-toolbar__count {
  margin-left: auto;
  color: var(--text-secondary);
  font-size: var(--fs-xs);
}
```

- [ ] **Step 3: `bun run typecheck`** → success.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/SessionsToolbar.tsx desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): SessionsToolbar"
```

---

## Phase 27 — Frontend: SessionCard

### Task 27.1: SessionCard with vitest

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/SessionCard.tsx`
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/SessionCard.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SessionCard } from "./SessionCard";
import type { SessionSummary } from "./types";

const sample: SessionSummary = {
  sessionId: "sess-fixture-001",
  providerId: "kimi",
  sourceDir: "/x",
  cwd: "/Users/me/proj",
  projectBasename: "proj",
  customTitle: "tạo cho tôi SUI counter",
  startedAt: "2026-04-29T00:00:00Z",
  lastEventAt: "2026-04-29T01:00:00Z",
  sizeBytes: 15_360,
  turnCount: 1,
  stepCount: 1,
  toolCallCount: 0,
  errorCount: 1,
  subagentCount: 0,
  hasWire: true,
  hasContext: true,
  imported: false,
};

describe("SessionCard", () => {
  it("renders id, title, counts, and error pill", () => {
    render(<SessionCard summary={sample} onClick={() => {}} />);
    expect(screen.getByText(/sess-fix/)).toBeInTheDocument();
    expect(screen.getByText(/tạo cho tôi SUI counter/)).toBeInTheDocument();
    expect(screen.getByText(/1 turn/)).toBeInTheDocument();
    expect(screen.getByText(/1 error/)).toBeInTheDocument();
  });

  it("renders 'Untitled Session' fallback when no title", () => {
    render(<SessionCard summary={{ ...sample, customTitle: null as any }} onClick={() => {}} />);
    expect(screen.getByText(/Untitled Session/)).toBeInTheDocument();
  });

  it("shows wire and context badges", () => {
    render(<SessionCard summary={sample} onClick={() => {}} />);
    expect(screen.getByText("wire")).toBeInTheDocument();
    expect(screen.getByText("context")).toBeInTheDocument();
  });

  it("hides error count when zero", () => {
    render(<SessionCard summary={{ ...sample, errorCount: 0 }} onClick={() => {}} />);
    expect(screen.queryByText(/error/)).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run — fails (no component)**

```bash
cd desktop-ui && bun run test SessionCard
```

- [ ] **Step 3: Implement**

```tsx
import type { SessionSummary } from "./types";
import { formatTime } from "@/shared/lib/dates";

interface Props {
  summary: SessionSummary;
  onClick: () => void;
  selected?: boolean;
}

export function SessionCard({ summary, onClick, selected }: Props) {
  const title = summary.customTitle?.trim() || "Untitled Session";
  const sizeKB = (summary.sizeBytes / 1024).toFixed(1);
  const durationMs = Date.parse(summary.lastEventAt) - Date.parse(summary.startedAt);
  const duration = formatDurationMs(durationMs);
  return (
    <button
      type="button"
      className={"tracing-card" + (selected ? " tracing-card--selected" : "")}
      onClick={onClick}
    >
      <div className="tracing-card__row">
        <span className="tracing-card__id">{summary.sessionId.slice(0, 8)}</span>
        <span className="tracing-card__time">{relativeTime(summary.lastEventAt)}</span>
      </div>
      <div className="tracing-card__title" title={title}>{title}</div>
      <div className="tracing-card__row">
        {summary.hasWire && <span className="tracing-badge tracing-badge--wire">wire</span>}
        {summary.hasContext && <span className="tracing-badge tracing-badge--context">context</span>}
        <span className="tracing-card__size">{sizeKB} KB</span>
      </div>
      <div className="tracing-card__counts">
        <span className="tracing-count tracing-count--turn">{summary.turnCount} turn</span>
        <span className="tracing-count-sep">·</span>
        <span className="tracing-count tracing-count--step">{summary.stepCount} step</span>
        <span className="tracing-count-sep">·</span>
        <span className="tracing-count tracing-count--tool">{summary.toolCallCount} tool</span>
      </div>
      <div className="tracing-card__row">
        <span className="tracing-card__duration">⏱ {duration}</span>
        {summary.errorCount > 0 && (
          <span className="tracing-card__error">ⓘ {summary.errorCount} error</span>
        )}
      </div>
    </button>
  );
}

function relativeTime(iso: string): string {
  const ms = Date.now() - Date.parse(iso);
  const days = ms / 86_400_000;
  if (days < 1) return "today";
  if (days < 2) return "1d ago";
  if (days < 30) return `${Math.floor(days)}d ago`;
  return new Date(iso).toLocaleDateString();
}

function formatDurationMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
```

If `formatTime` import fails, drop it — the helpers are inline.

- [ ] **Step 4: Add CSS to `tracing.css`**

```css
.tracing-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: var(--space-2) var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  text-align: left;
  cursor: pointer;
  color: var(--text-primary);
  font-size: var(--fs-base);
}
.tracing-card:hover { border-color: var(--border-strong); }
.tracing-card--selected { border-color: var(--accent-violet); }
.tracing-card__row { display: flex; align-items: center; gap: var(--space-2); }
.tracing-card__id { font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-secondary); }
.tracing-card__time { margin-left: auto; color: var(--text-secondary); font-size: var(--fs-xs); }
.tracing-card__title { font-size: var(--fs-base); white-space: nowrap; text-overflow: ellipsis; overflow: hidden; }
.tracing-badge { font-size: var(--fs-2xs); padding: 0 var(--space-1); border-radius: var(--radius-sm); }
.tracing-badge--wire { background: var(--accent-green-bg, #1a3a1a); color: var(--accent-green, #69d18a); }
.tracing-badge--context { background: var(--accent-green-bg, #1a3a1a); color: var(--accent-green, #69d18a); }
.tracing-card__size { margin-left: auto; color: var(--text-secondary); font-size: var(--fs-xs); }
.tracing-card__counts { display: flex; gap: var(--space-1); align-items: baseline; font-size: var(--fs-xs); }
.tracing-count--turn { color: var(--accent-blue, #6aaaff); }
.tracing-count--step { color: var(--accent-cyan, #5ad3d7); }
.tracing-count--tool { color: var(--accent-violet, #a07cff); }
.tracing-count-sep { color: var(--text-tertiary); }
.tracing-card__duration { color: var(--text-secondary); font-size: var(--fs-xs); }
.tracing-card__error { color: var(--accent-red, #ff6b6b); font-size: var(--fs-xs); }
```

- [ ] **Step 5: Run tests**

```bash
cd desktop-ui && bun run test SessionCard
```

Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/SessionCard.tsx desktop-ui/src/features/plugins/coding-memory/tracing/SessionCard.test.tsx desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): SessionCard"
```

---

## Phase 28 — Frontend: ProjectGroup

### Task 28.1: ProjectGroup component

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/ProjectGroup.tsx`

- [ ] **Step 1: Write**

```tsx
import { useState } from "react";
import type { ReactNode } from "react";

interface Props {
  basename: string;
  cwd: string;
  count: number;
  children: ReactNode;
  defaultOpen?: boolean;
}

export function ProjectGroup({ basename, cwd, count, children, defaultOpen = true }: Props) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="tracing-group">
      <button type="button" className="tracing-group__header" onClick={() => setOpen(!open)}>
        <span className="tracing-group__chevron" aria-hidden>{open ? "▾" : "▸"}</span>
        <span className="tracing-group__icon" aria-hidden>📁</span>
        <span className="tracing-group__name">{basename}</span>
        <span className="tracing-group__count">({count})</span>
        <span className="tracing-group__path">{cwd}</span>
      </button>
      {open && <div className="tracing-group__body">{children}</div>}
    </div>
  );
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-group { margin-bottom: var(--space-3); }
.tracing-group__header {
  display: flex; align-items: center; gap: var(--space-2);
  width: 100%; padding: var(--space-2) var(--space-3);
  background: transparent; border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md); color: var(--text-primary);
  cursor: pointer; font-size: var(--fs-base);
}
.tracing-group__chevron { width: 12px; color: var(--text-secondary); }
.tracing-group__count { color: var(--text-secondary); }
.tracing-group__path { margin-left: auto; color: var(--text-secondary); font-size: var(--fs-xs); }
.tracing-group__body {
  display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--space-2); padding: var(--space-2) 0 0 var(--space-4);
}
```

- [ ] **Step 3: `bun run typecheck`** → success.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ProjectGroup.tsx desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): ProjectGroup"
```

---

## Phase 29 — Frontend: SessionsListView

### Task 29.1: Wire the list view together

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/SessionsListView.tsx`
- Modify: `desktop-ui/src/features/plugins/coding-memory/tracing/TracingTab.tsx`

- [ ] **Step 1: Write `SessionsListView.tsx`**

```tsx
import { useMemo, useState } from "react";
import { invoke } from "@/api/client";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { SessionSummary } from "./types";
import { useTracingSessions } from "./useTracingSessions";
import { SessionsToolbar, type SortKey } from "./SessionsToolbar";
import { SessionCard } from "./SessionCard";
import { ProjectGroup } from "./ProjectGroup";

interface Props {
  providerId: string;
  onOpenSession: (sessionId: string) => void;
}

export function SessionsListView({ providerId, onOpenSession }: Props) {
  const [refreshKey, setRefreshKey] = useState(0);
  const { sessions, loading, error } = useTracingSessions(providerId, refreshKey);
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortKey>("recent");
  const [importedOnly, setImportedOnly] = useState(false);
  const [group, setGroup] = useState(true);
  const [layout, setLayout] = useState<"grid" | "list">("grid");

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return sessions.filter((s) => {
      if (importedOnly && !s.imported) return false;
      if (!q) return true;
      return (
        s.sessionId.toLowerCase().includes(q) ||
        (s.customTitle ?? "").toLowerCase().includes(q) ||
        (s.cwd ?? "").toLowerCase().includes(q)
      );
    });
  }, [sessions, search, importedOnly]);

  const sorted = useMemo(() => {
    const a = [...filtered];
    a.sort((x, y) => {
      switch (sort) {
        case "recent": return y.lastEventAt.localeCompare(x.lastEventAt);
        case "created": return y.startedAt.localeCompare(x.startedAt);
        case "size": return y.sizeBytes - x.sizeBytes;
        case "errors": return y.errorCount - x.errorCount;
      }
    });
    return a;
  }, [filtered, sort]);

  const grouped = useMemo(() => {
    const map = new Map<string, SessionSummary[]>();
    for (const s of sorted) {
      const key = s.projectBasename ?? "(unknown)";
      const list = map.get(key) ?? [];
      list.push(s);
      map.set(key, list);
    }
    return Array.from(map.entries());
  }, [sorted]);

  const totalSize = sorted.reduce((acc, s) => acc + s.sizeBytes, 0);
  const projectCount = grouped.length;

  const handleImport = async () => {
    const file = await openDialog({
      multiple: false,
      filters: [{ name: "wire.jsonl", extensions: ["jsonl"] }],
    });
    if (typeof file === "string") {
      await invoke("tracing_import", { providerId, filePath: file });
      setRefreshKey((k) => k + 1);
    }
  };

  if (loading) return <div className="tracing-state">Loading sessions…</div>;
  if (error) return <div className="tracing-state tracing-state--error">{error}</div>;

  return (
    <div className="tracing-list">
      <SessionsToolbar
        sessionCount={sorted.length}
        search={search} onSearchChange={setSearch}
        sort={sort} onSortChange={setSort}
        importedOnly={importedOnly} onImportedToggle={() => setImportedOnly((v) => !v)}
        group={group} onGroupToggle={() => setGroup((v) => !v)}
        layout={layout} onLayoutToggle={() => setLayout((l) => (l === "grid" ? "list" : "grid"))}
        onImport={handleImport}
      />
      <div className="tracing-list__body">
        {group ? (
          grouped.map(([basename, items]) => (
            <ProjectGroup
              key={basename}
              basename={basename}
              cwd={items[0]?.cwd ?? ""}
              count={items.length}
            >
              {items.map((s) => (
                <SessionCard key={s.sessionId} summary={s} onClick={() => onOpenSession(s.sessionId)} />
              ))}
            </ProjectGroup>
          ))
        ) : (
          <div className="tracing-list__flat">
            {sorted.map((s) => (
              <SessionCard key={s.sessionId} summary={s} onClick={() => onOpenSession(s.sessionId)} />
            ))}
          </div>
        )}
      </div>
      <div className="tracing-list__footer">
        {sorted.length} sessions · {projectCount} projects · {(totalSize / (1024 * 1024)).toFixed(1)} MB total
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-list { display: flex; flex-direction: column; flex: 1 1 auto; overflow: hidden; }
.tracing-list__body { flex: 1 1 auto; overflow-y: auto; padding: var(--space-3); }
.tracing-list__flat { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--space-2); }
.tracing-list__footer {
  padding: var(--space-2) var(--space-3);
  border-top: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: var(--fs-xs);
}
.tracing-state { padding: var(--space-4); color: var(--text-secondary); }
.tracing-state--error { color: var(--accent-red, #ff6b6b); }
```

- [ ] **Step 3: Wire into `TracingTab`**

Replace the placeholder body in `TracingTab.tsx`:

```tsx
{rootTab === "sessions" ? (
  viewState.view === "list" ? (
    <SessionsListView
      providerId={providerId}
      onOpenSession={(sessionId) => setViewState({ view: "detail", sessionId })}
    />
  ) : (
    <div style={{ padding: "var(--space-3)" }}>Detail view placeholder for {viewState.sessionId}</div>
  )
) : (
  <div style={{ padding: "var(--space-3)" }}>Statistics placeholder</div>
)}
```

Add the import at the top:

```tsx
import { SessionsListView } from "./SessionsListView";
```

- [ ] **Step 4: Verify the dialog plugin is installed**

```bash
grep '@tauri-apps/plugin-dialog' desktop-ui/package.json
```

If missing, run `cd desktop-ui && bun add @tauri-apps/plugin-dialog`. Also verify `crates/desktop/Cargo.toml` includes `tauri-plugin-dialog` and that it's registered in `lib.rs`. (Both should be present — confirm.)

- [ ] **Step 5: `bun run typecheck` + `bun run lint`** → success.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): SessionsListView with grouping, sort, search, import"
```

---

## Phase 30 — Frontend: DetailHeader

### Task 30.1: DetailHeader component

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/DetailHeader.tsx`

- [ ] **Step 1: Write**

```tsx
import type { HeaderStats } from "./types";

interface Props {
  providerDisplayName: string;
  sessionId: string;
  stats: HeaderStats;
  onBack: () => void;
  onOpenDir: () => void;
  onCopyDir: () => void;
  onDownload: () => void;
  onRefresh: () => void;
}

export function DetailHeader(p: Props) {
  return (
    <div className="tracing-detail-header">
      <div className="tracing-detail-header__top">
        <button type="button" className="tracing-detail-header__back" onClick={p.onBack}>←</button>
        <span className="tracing-detail-header__title">{p.providerDisplayName} Agent Tracing</span>
      </div>
      <div className="tracing-detail-header__stats">
        <span className="tracing-detail-header__id" title={p.sessionId}>{p.sessionId.slice(0, 16)}…</span>
        <span className="tracing-stat">{p.stats.turnCount} turns</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.stepCount} steps</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.toolCallCount} tool calls</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat tracing-stat--err">{p.stats.errorCount} errors</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.compactionCount} compaction</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.agentCount} agents</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{formatDurationMs(p.stats.totalDurationMs)}</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{(p.stats.totalInputTokens / 1000).toFixed(1)}k in / {(p.stats.totalOutputTokens / 1000).toFixed(1)}k out</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{p.stats.cacheHitPct.toFixed(0)}% cache</span>
        <div className="tracing-detail-header__actions">
          <button type="button" onClick={p.onOpenDir}>Open Dir</button>
          <button type="button" onClick={p.onCopyDir}>Copy DIR</button>
          <button type="button" onClick={p.onDownload}>Download</button>
          <button type="button" onClick={p.onRefresh}>Refresh</button>
        </div>
      </div>
    </div>
  );
}

function formatDurationMs(ms: number): string {
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-detail-header { padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--border-subtle); }
.tracing-detail-header__top { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-1); }
.tracing-detail-header__back {
  background: transparent; border: 0; color: var(--text-primary); cursor: pointer;
  font-size: var(--fs-lg); padding: 0 var(--space-1);
}
.tracing-detail-header__title { font-size: var(--fs-lg); font-weight: 500; }
.tracing-detail-header__stats { display: flex; align-items: center; gap: var(--space-1); flex-wrap: wrap; font-size: var(--fs-xs); color: var(--text-secondary); }
.tracing-detail-header__id { font-family: var(--font-mono); }
.tracing-stat--err { color: var(--accent-red, #ff6b6b); }
.tracing-detail-header__actions { margin-left: auto; display: flex; gap: var(--space-1); }
.tracing-detail-header__actions button {
  padding: var(--space-1) var(--space-2); background: transparent;
  border: 1px solid var(--border-subtle); border-radius: var(--radius-sm);
  color: var(--text-primary); cursor: pointer; font-size: var(--fs-xs);
}
```

- [ ] **Step 3: `bun run typecheck`** → success.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/DetailHeader.tsx desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): DetailHeader"
```

---

## Phase 31 — Frontend: ScopeSelector

### Task 31.1: ScopeSelector component

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/ScopeSelector.tsx`

- [ ] **Step 1: Write**

```tsx
import type { Scope, SubagentSummary } from "./types";

interface Props {
  scope: Scope;
  onScopeChange: (s: Scope) => void;
  subagents: SubagentSummary[];
}

export function ScopeSelector({ scope, onScopeChange, subagents }: Props) {
  const isMain = scope.kind === "main";
  return (
    <div className="tracing-scope">
      <span className="tracing-scope__label">SCOPE</span>
      <button
        type="button"
        className={"tracing-scope__chip" + (isMain ? " tracing-scope__chip--active" : "")}
        onClick={() => onScopeChange({ kind: "main" })}
      >Main Agent</button>
      {subagents.map((s) => {
        const active = scope.kind === "subagent" && scope.agentId === s.agentId;
        const color = stableColor(s.agentId);
        return (
          <button
            key={s.agentId}
            type="button"
            className={"tracing-scope__chip" + (active ? " tracing-scope__chip--active" : "")}
            style={{ borderLeft: `3px solid ${color}` }}
            onClick={() => onScopeChange({ kind: "subagent", agentId: s.agentId })}
            title={s.description ?? s.agentId}
          >{s.description ?? s.agentId}</button>
        );
      })}
    </div>
  );
}

function stableColor(id: string): string {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) % 360;
  return `hsl(${h}, 60%, 60%)`;
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-scope { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-3); flex-wrap: wrap; }
.tracing-scope__label { font-size: var(--fs-xs); color: var(--text-secondary); letter-spacing: 0.05em; }
.tracing-scope__chip {
  padding: var(--space-1) var(--space-2);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary); cursor: pointer; font-size: var(--fs-xs);
}
.tracing-scope__chip--active { border-color: var(--accent-violet); color: var(--text-primary); }
```

- [ ] **Step 3: `bun run typecheck`** → success.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ScopeSelector.tsx desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): ScopeSelector"
```

---

## Phase 32 — Frontend: SessionDetailView shell

### Task 32.1: SessionDetailView with five tabs

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/SessionDetailView.tsx`
- Modify: `desktop-ui/src/features/plugins/coding-memory/tracing/TracingTab.tsx`

- [ ] **Step 1: Write `SessionDetailView.tsx`**

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { DetailTab, Scope, SubagentSummary } from "./types";
import { useTracingSession } from "./useTracingSession";
import { chromeFor } from "./providers";
import { DetailHeader } from "./DetailHeader";
import { ScopeSelector } from "./ScopeSelector";

interface Props {
  providerId: string;
  sessionId: string;
  onBack: () => void;
}

export function SessionDetailView({ providerId, sessionId, onBack }: Props) {
  const [scope, setScope] = useState<Scope>({ kind: "main" });
  const [refreshKey, setRefreshKey] = useState(0);
  const [tab, setTab] = useState<DetailTab>("wireEvents");
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);

  const { detail, loading, error } = useTracingSession(providerId, sessionId, scope, refreshKey);

  useEffect(() => {
    let cancelled = false;
    invoke<SubagentSummary[]>("tracing_list_subagents", { providerId, sessionId })
      .then((s) => { if (!cancelled) setSubagents(s); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [providerId, sessionId, refreshKey]);

  const chrome = chromeFor(providerId);

  if (loading && !detail) return <div className="tracing-state">Loading session…</div>;
  if (error) return <div className="tracing-state tracing-state--error">{error}</div>;
  if (!detail) return null;

  return (
    <div className="tracing-detail">
      <DetailHeader
        providerDisplayName={chrome.displayName}
        sessionId={sessionId}
        stats={detail.stats}
        onBack={onBack}
        onOpenDir={() => invoke("tracing_open_dir", { providerId, sessionId })}
        onCopyDir={async () => {
          const path: string = await invoke("tracing_open_dir", { providerId, sessionId });
          await writeText(path);
        }}
        onDownload={() => { /* deferred — see "Out of scope for v1" */ }}
        onRefresh={() => setRefreshKey((k) => k + 1)}
      />
      <div className="tracing-tabs">
        {(["wireEvents", "contextMessages", "state", "dual", "agents"] as DetailTab[]).map((t) => (
          <button
            key={t}
            type="button"
            className={"tracing-tab" + (tab === t ? " tracing-tab--active" : "")}
            onClick={() => setTab(t)}
          >{labelFor(t)}</button>
        ))}
      </div>
      {tab === "wireEvents" && (
        <>
          <ScopeSelector scope={scope} onScopeChange={setScope} subagents={subagents} />
          {/* WireEventsTab — Phase 35+ */}
          <div className="tracing-state">Wire Events tab placeholder ({detail.events.length} events)</div>
        </>
      )}
      {tab === "contextMessages" && <div className="tracing-state">Context Messages placeholder</div>}
      {tab === "state" && <div className="tracing-state">State placeholder</div>}
      {tab === "dual" && <div className="tracing-state">Dual placeholder</div>}
      {tab === "agents" && <div className="tracing-state">Agents placeholder</div>}
    </div>
  );
}

function labelFor(t: DetailTab): string {
  switch (t) {
    case "wireEvents": return "Wire Events";
    case "contextMessages": return "Context Messages";
    case "state": return "State";
    case "dual": return "Dual";
    case "agents": return "Agents";
  }
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-detail { display: flex; flex-direction: column; flex: 1 1 auto; overflow: hidden; }
```

- [ ] **Step 3: Wire into `TracingTab.tsx`**

Replace the detail-view placeholder branch with:

```tsx
viewState.view === "detail" && viewState.sessionId ? (
  <SessionDetailView
    providerId={providerId}
    sessionId={viewState.sessionId}
    onBack={() => setViewState({ view: "list", sessionId: null })}
  />
) : (
  <SessionsListView … />
)
```

Add import:

```tsx
import { SessionDetailView } from "./SessionDetailView";
```

- [ ] **Step 4: Verify clipboard plugin**

```bash
grep '@tauri-apps/plugin-clipboard-manager' desktop-ui/package.json
```

If absent: `cd desktop-ui && bun add @tauri-apps/plugin-clipboard-manager`. Also verify `tauri-plugin-clipboard-manager` in `crates/desktop/Cargo.toml`.

- [ ] **Step 5: `bun run typecheck` + `bun run lint`** → success.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): SessionDetailView shell with five tabs"
```

---

## Phase 33 — Frontend: EventTypeChip

### Task 33.1: Color-coded chip per kimi-vis palette

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/EventTypeChip.tsx`

- [ ] **Step 1: Write**

```tsx
interface Props {
  rawKind: string;
  active?: boolean;
  onClick?: () => void;
}

const PALETTE: Record<string, string> = {
  TurnBegin: "var(--type-blue, #6aaaff)",
  TurnEnd: "var(--type-blue, #6aaaff)",
  StepBegin: "var(--type-green, #69d18a)",
  StepInterrupted: "var(--type-yellow, #d4a83a)",
  ContentPart: "var(--type-purple, #a07cff)",
  ThinkPart: "var(--type-purple, #a07cff)",
  TextPart: "var(--type-purple, #a07cff)",
  ToolCall: "var(--type-violet, #c060ff)",
  ToolCallPart: "var(--type-violet, #c060ff)",
  ToolResult: "var(--type-violet, #c060ff)",
  StatusUpdate: "var(--type-orange, #d68a3a)",
  SubagentEvent: "var(--type-blue, #6aaaff)",
  CompactionBegin: "var(--type-orange, #d68a3a)",
  CompactionEnd: "var(--type-orange, #d68a3a)",
  Error: "var(--accent-red, #ff6b6b)",
};

export function EventTypeChip({ rawKind, active, onClick }: Props) {
  const color = PALETTE[rawKind] ?? "var(--text-secondary)";
  return (
    <button
      type="button"
      className={"tracing-chip" + (active ? " tracing-chip--active" : "")}
      style={{ borderColor: color, color }}
      onClick={onClick}
    >{rawKind}</button>
  );
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-chip {
  padding: var(--space-0-5, 2px) var(--space-2);
  background: transparent; border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm); cursor: pointer; font-size: var(--fs-2xs);
  font-family: var(--font-mono);
}
.tracing-chip--active { background: rgba(255,255,255,0.06); }
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/EventTypeChip.tsx desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): EventTypeChip"
```

---

## Phase 34 — Frontend: Event cards (one task per category)

Each event card renders one `SemanticCategory` worth of `TraceEvent`. The cards live under `tracing/event-cards/`. The dispatcher in Phase 35 picks the right card from `event.category`.

The structure of every card is the same:

```tsx
interface Props { event: TraceEvent; expanded?: boolean; onToggle?: () => void; }
```

Each card renders the timestamp, the type chip, and category-specific content.

### Task 34.1: TurnBeginCard

**Files:** `tracing/event-cards/TurnBeginCard.tsx`

- [ ] **Step 1: Write**

```tsx
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";
import { formatTime } from "@/shared/lib/dates";

interface Props { event: TraceEvent; expanded: boolean; onToggle: () => void; }

export function TurnBeginCard({ event, expanded, onToggle }: Props) {
  const text = extractUserText(event.payload);
  return (
    <div className="tracing-evcard tracing-evcard--turn">
      <div className="tracing-evcard__row">
        <span className="tracing-evcard__time">{formatTime(event.occurredAt)}</span>
        <EventTypeChip rawKind={event.rawKind} />
        <button type="button" className="tracing-evcard__toggle" onClick={onToggle}>
          {expanded ? "▾" : "▸"}
        </button>
        <span className="tracing-evcard__preview">{text.slice(0, 200)}</span>
      </div>
      {expanded && <pre className="tracing-evcard__full">{text}</pre>}
    </div>
  );
}

function extractUserText(payload: any): string {
  if (typeof payload?.user_input === "string") return payload.user_input;
  if (Array.isArray(payload?.user_input)) {
    return payload.user_input.filter((p: any) => p.type === "text").map((p: any) => p.text).join("\n");
  }
  return "";
}
```

If `formatTime` from `@/shared/lib/dates` doesn't exist, inline `new Date(iso).toLocaleTimeString()`.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/event-cards/TurnBeginCard.tsx
git commit -m "feat(ui): TurnBeginCard"
```

### Task 34.2: TurnEndCard

**Files:** `tracing/event-cards/TurnEndCard.tsx`

- [ ] **Step 1: Write**

```tsx
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";
import { formatTime } from "@/shared/lib/dates";

interface Props { event: TraceEvent; }

export function TurnEndCard({ event }: Props) {
  return (
    <div className="tracing-evcard tracing-evcard--turn-end">
      <span className="tracing-evcard__time">{formatTime(event.occurredAt)}</span>
      <EventTypeChip rawKind={event.rawKind} />
      <hr className="tracing-evcard__rule" />
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git commit -am "feat(ui): TurnEndCard"
```

### Task 34.3: StepBeginCard

```tsx
import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; deltaMs?: number; }

export function StepBeginCard({ event, deltaMs }: Props) {
  const n = (event.payload as any)?.n ?? "?";
  return (
    <div className="tracing-evcard tracing-evcard--step">
      <span className="tracing-evcard__step-label">Step {n}</span>
      {deltaMs !== undefined && <span className="tracing-evcard__delta">+{formatDelta(deltaMs)}</span>}
    </div>
  );
}

function formatDelta(ms: number) {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
```

Commit: `feat(ui): StepBeginCard`.

### Task 34.4: StepInterruptedCard

```tsx
import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; }

export function StepInterruptedCard({ event }: Props) {
  const reason = (event.payload as any)?.reason ?? "interrupted";
  return <div className="tracing-evcard tracing-evcard--interrupted">{reason}</div>;
}
```

CSS: `.tracing-evcard--interrupted { background: rgba(255,107,107,0.1); padding: var(--space-1) var(--space-2); border-radius: var(--radius-sm); color: var(--accent-red); }`

Commit: `feat(ui): StepInterruptedCard`.

### Task 34.5: ThinkingCard

```tsx
import { useState } from "react";
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; }

export function ThinkingCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  const text = (event.payload as any)?.think ?? "";
  return (
    <div className="tracing-evcard tracing-evcard--think">
      <div className="tracing-evcard__row">
        <EventTypeChip rawKind="ThinkPart" />
        <button type="button" onClick={() => setOpen((v) => !v)} className="tracing-evcard__toggle">
          {open ? "▾" : "▸"} thinking…
        </button>
      </div>
      {open && <pre className="tracing-evcard__think">{text}</pre>}
    </div>
  );
}
```

CSS: `.tracing-evcard__think { color: var(--text-secondary); font-style: italic; padding: var(--space-1) 0 0 var(--space-3); }`

Commit: `feat(ui): ThinkingCard`.

### Task 34.6: AssistantTextCard

```tsx
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";
import { Markdown } from "@/features/messages/components/Markdown";

interface Props { event: TraceEvent; }

export function AssistantTextCard({ event }: Props) {
  const text = (event.payload as any)?.text ?? "";
  return (
    <div className="tracing-evcard tracing-evcard--text">
      <div className="tracing-evcard__row">
        <EventTypeChip rawKind="TextPart" />
      </div>
      <div className="tracing-evcard__md"><Markdown content={text} /></div>
    </div>
  );
}
```

If `Markdown` import path differs, run `grep -r 'export.*Markdown' desktop-ui/src/features/messages` to confirm.

Commit: `feat(ui): AssistantTextCard`.

### Task 34.7: ToolCallCard

```tsx
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; onSelect: () => void; selected: boolean; }

export function ToolCallCard({ event, onSelect, selected }: Props) {
  const fn = (event.payload as any)?.function;
  const name = fn?.name ?? "(tool)";
  const args = fn?.arguments ?? "";
  return (
    <button
      type="button"
      className={"tracing-evcard tracing-evcard--tool" + (selected ? " tracing-evcard--selected" : "")}
      onClick={onSelect}
    >
      <span className="tracing-evcard__tool-id">{(event.payload as any)?.id?.slice?.(0, 16)}</span>
      <EventTypeChip rawKind="ToolCall" />
      <span className="tracing-evcard__tool-name">{name}()</span>
      <span className="tracing-evcard__tool-args">{String(args).slice(0, 120)}</span>
      <span className="tracing-evcard__detail-link">detail</span>
    </button>
  );
}
```

Commit: `feat(ui): ToolCallCard`.

### Task 34.8: ToolCallStreamCard

```tsx
import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; }

export function ToolCallStreamCard({ event }: Props) {
  return <div className="tracing-evcard tracing-evcard--tool-stream">…streaming arguments…</div>;
}
```

CSS: `.tracing-evcard--tool-stream { color: var(--text-tertiary); font-size: var(--fs-2xs); padding: 0 var(--space-3); }`

Commit: `feat(ui): ToolCallStreamCard`.

### Task 34.9: ToolResultCard

```tsx
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; }

export function ToolResultCard({ event }: Props) {
  const id = (event.payload as any)?.id;
  const content = (event.payload as any)?.content;
  const preview = typeof content === "string" ? content : JSON.stringify(content);
  return (
    <div className="tracing-evcard tracing-evcard--tool-result">
      <span className="tracing-evcard__tool-id">{id?.slice?.(0, 16)}</span>
      <EventTypeChip rawKind="ToolResult" />
      <span className="tracing-evcard__preview">{preview.slice(0, 160)}</span>
    </div>
  );
}
```

Commit: `feat(ui): ToolResultCard`.

### Task 34.10: StatusUpdateCard

```tsx
import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; }

export function StatusUpdateCard({ event }: Props) {
  const p = event.payload as any;
  const ctxPct = ((p?.context_usage ?? 0) * 100).toFixed(1);
  const ctxTok = p?.context_tokens ?? 0;
  const maxCtx = p?.max_context_tokens ?? 0;
  const u = p?.token_usage ?? {};
  const inTok = (u.input_other ?? 0) + (u.input_cache_read ?? 0) + (u.input_cache_creation ?? 0);
  const cacheRead = u.input_cache_read ?? 0;
  const cachePct = inTok > 0 ? ((cacheRead / inTok) * 100).toFixed(0) : "0";
  return (
    <div className="tracing-evcard tracing-evcard--status">
      ctx: {ctxPct}% · {(ctxTok / 1000).toFixed(1)}k / {(maxCtx / 1000).toFixed(0)}k tokens · in: {(inTok / 1000).toFixed(1)}k ({cachePct}% cache) · out: {(u.output ?? 0) / 1000}k
    </div>
  );
}
```

CSS: `.tracing-evcard--status { color: var(--type-orange); font-size: var(--fs-2xs); padding: var(--space-1) var(--space-3); }`

Commit: `feat(ui): StatusUpdateCard`.

### Task 34.11: SubagentEventCard

```tsx
import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; onOpenSubagent?: (agentId: string) => void; }

export function SubagentEventCard({ event, onOpenSubagent }: Props) {
  const p = event.payload as any;
  const agentId = p?.agent_id;
  const wrapped = p?.event;
  const description = p?.subagent_type ?? agentId;
  return (
    <div className="tracing-evcard tracing-evcard--subagent">
      <button
        type="button"
        className="tracing-evcard__sub-link"
        onClick={() => agentId && onOpenSubagent?.(agentId)}
      >↳ {description}</button>
      <pre className="tracing-evcard__sub-payload">{JSON.stringify(wrapped, null, 2).slice(0, 240)}</pre>
    </div>
  );
}
```

Commit: `feat(ui): SubagentEventCard`.

### Task 34.12: CompactionMarkerCard

```tsx
import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; }

export function CompactionMarkerCard({ event }: Props) {
  const begin = event.rawKind === "CompactionBegin";
  return (
    <div className={"tracing-evcard tracing-evcard--compact " + (begin ? "tracing-evcard--compact-begin" : "tracing-evcard--compact-end")}>
      ⎯⎯⎯ {begin ? "Compaction begin" : "Compaction end"} ⎯⎯⎯
    </div>
  );
}
```

CSS: `.tracing-evcard--compact { text-align: center; color: var(--type-orange); font-size: var(--fs-xs); padding: var(--space-1); }`

Commit: `feat(ui): CompactionMarkerCard`.

### Task 34.13: ErrorCard

```tsx
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; }

export function ErrorCard({ event }: Props) {
  const p = event.payload as any;
  const content = typeof p?.content === "string" ? p.content : JSON.stringify(p?.content);
  return (
    <div className="tracing-evcard tracing-evcard--error">
      <EventTypeChip rawKind="Error" />
      <span className="tracing-evcard__error-msg">{content?.slice?.(0, 240)}</span>
    </div>
  );
}
```

CSS: `.tracing-evcard--error { background: rgba(255,107,107,0.08); padding: var(--space-2); border-radius: var(--radius-sm); display: flex; gap: var(--space-2); align-items: baseline; }`

Commit: `feat(ui): ErrorCard`.

### Task 34.14: OtherEventCard

```tsx
import { useState } from "react";
import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; }

export function OtherEventCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  return (
    <div className="tracing-evcard tracing-evcard--other">
      <div className="tracing-evcard__row">
        <EventTypeChip rawKind={event.rawKind} />
        <button type="button" onClick={() => setOpen((v) => !v)} className="tracing-evcard__toggle">
          {open ? "▾" : "▸"} {event.rawKind}
        </button>
      </div>
      {open && <pre className="tracing-evcard__raw">{JSON.stringify(event.payload, null, 2)}</pre>}
    </div>
  );
}
```

Commit: `feat(ui): OtherEventCard`.

### Task 34.15: Card index file

`tracing/event-cards/index.ts`:

```ts
export * from "./TurnBeginCard";
export * from "./TurnEndCard";
export * from "./StepBeginCard";
export * from "./StepInterruptedCard";
export * from "./ThinkingCard";
export * from "./AssistantTextCard";
export * from "./ToolCallCard";
export * from "./ToolCallStreamCard";
export * from "./ToolResultCard";
export * from "./StatusUpdateCard";
export * from "./SubagentEventCard";
export * from "./CompactionMarkerCard";
export * from "./ErrorCard";
export * from "./OtherEventCard";
```

Run `bun run typecheck` → success. Commit: `feat(ui): event-card barrel export`.

---

## Phase 35 — Frontend: WireEventsTab list rendering

### Task 35.1: Dispatch helper + virtualized list

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/WireEventsTab.tsx`

- [ ] **Step 1: Write**

```tsx
import { useCallback, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import type { Scope, SubagentSummary, TraceEvent } from "./types";
import {
  TurnBeginCard, TurnEndCard, StepBeginCard, StepInterruptedCard,
  ThinkingCard, AssistantTextCard, ToolCallCard, ToolCallStreamCard,
  ToolResultCard, StatusUpdateCard, SubagentEventCard, CompactionMarkerCard,
  ErrorCard, OtherEventCard,
} from "./event-cards";

interface Props {
  events: TraceEvent[];
  subagents: SubagentSummary[];
  onOpenSubagent: (agentId: string) => void;
}

export function WireEventsTab({ events, subagents, onOpenSubagent }: Props) {
  const [selectedEventSeq, setSelectedEventSeq] = useState<number | null>(null);
  const [expandedTurns, setExpandedTurns] = useState<Set<number>>(new Set());
  const ref = useRef<VirtuosoHandle>(null);

  const toggleExpand = useCallback((seq: number) => {
    setExpandedTurns((prev) => {
      const next = new Set(prev);
      next.has(seq) ? next.delete(seq) : next.add(seq);
      return next;
    });
  }, []);

  const renderCard = (event: TraceEvent, prev: TraceEvent | undefined) => {
    const deltaMs = prev
      ? Math.max(0, Date.parse(event.occurredAt) - Date.parse(prev.occurredAt))
      : 0;
    switch (event.category) {
      case "turnBegin":
        return <TurnBeginCard event={event} expanded={expandedTurns.has(event.seq)} onToggle={() => toggleExpand(event.seq)} />;
      case "turnEnd": return <TurnEndCard event={event} />;
      case "stepBegin": return <StepBeginCard event={event} deltaMs={deltaMs} />;
      case "stepInterrupted": return <StepInterruptedCard event={event} />;
      case "thinking": return <ThinkingCard event={event} />;
      case "assistantText": return <AssistantTextCard event={event} />;
      case "toolCall":
        return <ToolCallCard event={event} selected={selectedEventSeq === event.seq} onSelect={() => setSelectedEventSeq(event.seq === selectedEventSeq ? null : event.seq)} />;
      case "toolCallStream": return <ToolCallStreamCard event={event} />;
      case "toolResult": return <ToolResultCard event={event} />;
      case "statusUpdate": return <StatusUpdateCard event={event} />;
      case "subagent": return <SubagentEventCard event={event} onOpenSubagent={onOpenSubagent} />;
      case "compactionBegin":
      case "compactionEnd":
        return <CompactionMarkerCard event={event} />;
      case "error": return <ErrorCard event={event} />;
      default: return <OtherEventCard event={event} />;
    }
  };

  return (
    <div className="tracing-events">
      <Virtuoso
        ref={ref}
        data={events}
        itemContent={(i, event) => (
          <div className="tracing-events__row">
            {renderCard(event, i > 0 ? events[i - 1] : undefined)}
          </div>
        )}
      />
    </div>
  );
}
```

- [ ] **Step 2: Add CSS**

```css
.tracing-events { flex: 1 1 auto; overflow: hidden; height: 100%; }
.tracing-events__row { padding: var(--space-1) var(--space-3); }
.tracing-evcard { display: block; font-size: var(--fs-base); }
.tracing-evcard__row { display: flex; align-items: center; gap: var(--space-2); }
.tracing-evcard__time { color: var(--text-tertiary); font-size: var(--fs-2xs); font-family: var(--font-mono); }
.tracing-evcard__toggle { background: transparent; border: 0; color: var(--text-secondary); cursor: pointer; font-size: var(--fs-xs); }
.tracing-evcard__preview { color: var(--text-primary); flex: 1 1 auto; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-size: var(--fs-xs); }
.tracing-evcard__delta { color: var(--text-tertiary); font-size: var(--fs-2xs); margin-left: auto; }
```

- [ ] **Step 3: Wire into `SessionDetailView.tsx`**

Replace the placeholder:

```tsx
{tab === "wireEvents" && (
  <>
    <ScopeSelector scope={scope} onScopeChange={setScope} subagents={subagents} />
    <WireEventsTab
      events={detail.events}
      subagents={subagents}
      onOpenSubagent={(agentId) => setScope({ kind: "subagent", agentId })}
    />
  </>
)}
```

Add import:

```tsx
import { WireEventsTab } from "./WireEventsTab";
```

- [ ] **Step 4: `bun run typecheck` + `bun run lint`** → success.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): WireEventsTab with virtualized event-card dispatch"
```

---

## Phase 36 — Frontend: WireFilters — type chips + presets

### Task 36.1: WireFilters component (type chips + presets)

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/WireFilters.tsx`

- [ ] **Step 1: Write — type-chip + preset row only (search/expand/views in next phases)**

```tsx
import { useMemo } from "react";
import type { TraceEvent } from "./types";
import { EventTypeChip } from "./EventTypeChip";

export type FilterPreset = "all" | "errors" | "toolCalls" | "thinking" | "approvals" | "subagents";

interface Props {
  events: TraceEvent[];
  selectedKinds: Set<string>;
  onToggleKind: (kind: string) => void;
  preset: FilterPreset;
  onPresetChange: (p: FilterPreset) => void;
}

export function WireFilters(p: Props) {
  const allKinds = useMemo(() => {
    const s = new Set<string>();
    for (const e of p.events) s.add(e.rawKind);
    return [...s].sort();
  }, [p.events]);

  return (
    <div className="tracing-filters">
      <div className="tracing-filters__presets">
        {(["all", "errors", "toolCalls", "thinking", "approvals", "subagents"] as FilterPreset[]).map((k) => (
          <button
            key={k}
            type="button"
            className={"tracing-filters__preset" + (p.preset === k ? " tracing-filters__preset--active" : "")}
            onClick={() => p.onPresetChange(k)}
          >{labelFor(k)}</button>
        ))}
      </div>
      <div className="tracing-filters__chips">
        {allKinds.map((k) => (
          <EventTypeChip
            key={k}
            rawKind={k}
            active={p.selectedKinds.has(k)}
            onClick={() => p.onToggleKind(k)}
          />
        ))}
      </div>
    </div>
  );
}

function labelFor(p: FilterPreset): string {
  switch (p) {
    case "all": return "All Events";
    case "errors": return "Errors Only";
    case "toolCalls": return "Tool Calls";
    case "thinking": return "Thinking";
    case "approvals": return "Approvals";
    case "subagents": return "Sub-agents";
  }
}
```

- [ ] **Step 2: CSS**

```css
.tracing-filters { padding: var(--space-1) var(--space-3); border-bottom: 1px solid var(--border-subtle); }
.tracing-filters__presets { display: flex; gap: var(--space-1); flex-wrap: wrap; margin-bottom: var(--space-1); }
.tracing-filters__preset {
  padding: var(--space-1) var(--space-2); background: transparent;
  border: 1px solid var(--border-subtle); border-radius: var(--radius-sm);
  color: var(--text-secondary); cursor: pointer; font-size: var(--fs-xs);
}
.tracing-filters__preset--active { color: var(--text-primary); background: var(--bg-elevated); }
.tracing-filters__chips { display: flex; gap: var(--space-1); flex-wrap: wrap; }
```

- [ ] **Step 3: Apply preset → kinds in WireEventsTab**

In `WireEventsTab.tsx`, add filter state:

```tsx
const [preset, setPreset] = useState<FilterPreset>("all");
const [selectedKinds, setSelectedKinds] = useState<Set<string>>(new Set());

const toggleKind = (k: string) => setSelectedKinds((prev) => {
  const next = new Set(prev);
  next.has(k) ? next.delete(k) : next.add(k);
  return next;
});

const filteredByCategoryAndPreset = useMemo(() => {
  return events.filter((e) => {
    switch (preset) {
      case "errors": if (e.category !== "error") return false; break;
      case "toolCalls": if (!["toolCall", "toolResult", "toolCallStream"].includes(e.category)) return false; break;
      case "thinking": if (e.category !== "thinking") return false; break;
      case "approvals": if (!e.rawKind.toLowerCase().includes("approval")) return false; break;
      case "subagents": if (e.category !== "subagent") return false; break;
      case "all":
      default: break;
    }
    if (selectedKinds.size > 0 && !selectedKinds.has(e.rawKind)) return false;
    return true;
  });
}, [events, preset, selectedKinds]);
```

Replace `data={events}` in Virtuoso with `data={filteredByCategoryAndPreset}`.

Render `<WireFilters …/>` above the Virtuoso. Import:

```tsx
import { WireFilters, type FilterPreset } from "./WireFilters";
```

- [ ] **Step 4: `bun run typecheck`** → success.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): WireFilters preset + type-chip selection"
```

---

## Phase 37 — Frontend: WireFilters — search + error nav + expand/collapse + completeness

### Task 37.1: Add search and error navigation to WireFilters

**Files:**
- Modify: `desktop-ui/src/features/plugins/coding-memory/tracing/WireFilters.tsx`
- Modify: `desktop-ui/src/features/plugins/coding-memory/tracing/WireEventsTab.tsx`

- [ ] **Step 1: Extend `WireFilters` props**

```tsx
interface Props {
  events: TraceEvent[];
  selectedKinds: Set<string>;
  onToggleKind: (kind: string) => void;
  preset: FilterPreset;
  onPresetChange: (p: FilterPreset) => void;
  search: string;
  onSearchChange: (s: string) => void;
  errorCount: number;
  onPrevError: () => void;
  onNextError: () => void;
  totalCount: number;
  filteredCount: number;
  allExpanded: boolean;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  completenessPct: number;
}
```

Render an additional row above the chips:

```tsx
<div className="tracing-filters__row">
  <input
    type="search" placeholder="Search events… (/)"
    value={p.search} onChange={(e) => p.onSearchChange(e.target.value)}
    className="tracing-filters__search" data-wire-search
  />
  <span className="tracing-filters__err">
    {p.errorCount} errors
    <button type="button" onClick={p.onPrevError}>↑</button>
    <button type="button" onClick={p.onNextError}>↓</button>
  </span>
  <span className="tracing-filters__count">{p.filteredCount} / {p.totalCount}</span>
  <button type="button" onClick={p.allExpanded ? p.onCollapseAll : p.onExpandAll}>
    {p.allExpanded ? "Collapse" : "Expand"}
  </button>
  <span className="tracing-filters__completeness">{p.completenessPct.toFixed(0)}% complete</span>
</div>
```

- [ ] **Step 2: Wire state + handlers in `WireEventsTab.tsx`**

Add:

```tsx
const [search, setSearch] = useState("");
const [errorNavIdx, setErrorNavIdx] = useState(-1);

const errorEventSeqs = useMemo(
  () => events.filter((e) => e.category === "error").map((e) => e.seq),
  [events],
);

const completenessPct = events.length === 0 ? 100
  : 100 - (errorEventSeqs.length / events.length) * 100;

const matchSet = useMemo(() => {
  const q = search.trim().toLowerCase();
  if (!q) return new Set<number>();
  const out = new Set<number>();
  for (const e of events) {
    if (e.rawKind.toLowerCase().includes(q)) { out.add(e.seq); continue; }
    if (JSON.stringify(e.payload).toLowerCase().includes(q)) out.add(e.seq);
  }
  return out;
}, [events, search]);

const filtered = useMemo(() => {
  return filteredByCategoryAndPreset.filter((e) => !search || matchSet.has(e.seq));
}, [filteredByCategoryAndPreset, search, matchSet]);
```

Replace Virtuoso `data` with `filtered`.

Add error nav handlers:

```tsx
const navigateError = (dir: "next" | "prev") => {
  if (errorEventSeqs.length === 0) return;
  const next = dir === "next"
    ? (errorNavIdx + 1) % errorEventSeqs.length
    : (errorNavIdx - 1 + errorEventSeqs.length) % errorEventSeqs.length;
  setErrorNavIdx(next);
  const targetSeq = errorEventSeqs[next];
  const pos = filtered.findIndex((e) => e.seq === targetSeq);
  if (pos >= 0) ref.current?.scrollToIndex({ index: pos, align: "center", behavior: "smooth" });
};
```

Add expand/collapse all:

```tsx
const allExpanded = filtered.length > 0 && filtered.every((e) => expandedTurns.has(e.seq));
const expandAll = () => setExpandedTurns(new Set(filtered.map((e) => e.seq)));
const collapseAll = () => setExpandedTurns(new Set());
```

Add keyboard shortcut:

```tsx
useEffect(() => {
  const h = (e: KeyboardEvent) => {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (e.key === "/") {
      e.preventDefault();
      (document.querySelector("[data-wire-search]") as HTMLInputElement)?.focus();
    } else if (e.key === "e") {
      e.preventDefault();
      navigateError("next");
    }
  };
  window.addEventListener("keydown", h);
  return () => window.removeEventListener("keydown", h);
}, [errorEventSeqs, filtered]);
```

Pass all the new props into `<WireFilters …/>`.

- [ ] **Step 3: Add CSS**

```css
.tracing-filters__row { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-1); flex-wrap: wrap; }
.tracing-filters__search { flex: 1 1 240px; padding: var(--space-1) var(--space-2); background: var(--bg-input); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); color: var(--text-primary); font-size: var(--fs-xs); }
.tracing-filters__err { display: flex; align-items: center; gap: var(--space-1); color: var(--accent-red); font-size: var(--fs-xs); }
.tracing-filters__count { color: var(--text-secondary); font-size: var(--fs-xs); }
.tracing-filters__completeness { color: var(--accent-green); font-size: var(--fs-xs); }
```

- [ ] **Step 4: `bun run typecheck`** → success.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(ui): WireFilters search + error nav + expand/collapse + completeness"
```

---

## Phase 38 — Frontend: WireFilters — view modes (Chart/Events/Timeline/Decisions)

### Task 38.1: ViewMode tabs on WireFilters

- [ ] **Step 1: Extend WireFilters props**

```tsx
view: ViewMode;
onViewChange: (v: ViewMode) => void;
```

Render tabs:

```tsx
<div className="tracing-filters__views">
  {(["chart", "events", "timeline", "decisions"] as ViewMode[]).map((v) => (
    <button
      key={v}
      type="button"
      className={"tracing-filters__view" + (p.view === v ? " tracing-filters__view--active" : "")}
      onClick={() => p.onViewChange(v)}
    >{v}</button>
  ))}
</div>
```

- [ ] **Step 2: WireEventsTab tracks view**

```tsx
const [view, setView] = useState<ViewMode>("events");
```

Conditionally render:

```tsx
{view === "events" && <Virtuoso … />}
{view === "chart" && <EventsChart events={events} />}
{view === "timeline" && <EventsTimeline events={events} />}
{view === "decisions" && (
  <Virtuoso data={events.filter((e) => ["toolCall", "toolResult"].includes(e.category) || e.rawKind.toLowerCase().includes("approval"))} … />
)}
```

- [ ] **Step 3: Create `EventsChart.tsx` (events/sec sparkline)**

```tsx
import { useMemo } from "react";
import type { TraceEvent } from "./types";

export function EventsChart({ events }: { events: TraceEvent[] }) {
  const buckets = useMemo(() => bucketize(events, 60), [events]);
  const w = 800, h = 60;
  const max = Math.max(1, ...buckets.map((b) => b.count));
  const path = buckets.map((b, i) => {
    const x = (i / (buckets.length - 1 || 1)) * w;
    const y = h - (b.count / max) * (h - 4);
    return (i === 0 ? "M" : "L") + `${x.toFixed(1)} ${y.toFixed(1)}`;
  }).join(" ");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="tracing-chart">
      <path d={path} fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

function bucketize(events: TraceEvent[], n: number) {
  if (events.length === 0) return Array.from({ length: n }, () => ({ count: 0 }));
  const t0 = Date.parse(events[0].occurredAt);
  const t1 = Date.parse(events[events.length - 1].occurredAt);
  const span = Math.max(1, t1 - t0);
  const buckets = Array.from({ length: n }, () => ({ count: 0 }));
  for (const e of events) {
    const idx = Math.min(n - 1, Math.floor(((Date.parse(e.occurredAt) - t0) / span) * n));
    buckets[idx].count++;
  }
  return buckets;
}
```

CSS: `.tracing-chart { width: 100%; height: 60px; color: var(--accent-violet); padding: var(--space-2); }`

- [ ] **Step 4: Create `EventsTimeline.tsx`**

```tsx
import { useMemo } from "react";
import type { TraceEvent } from "./types";

const CATEGORY_COLOR: Record<string, string> = {
  thinking: "#a07cff", assistantText: "#6aaaff", toolCall: "#c060ff",
  toolResult: "#c060ff", error: "#ff6b6b", statusUpdate: "#d68a3a",
};

export function EventsTimeline({ events }: { events: TraceEvent[] }) {
  const lanes = useMemo(() => {
    const cats = Array.from(new Set(events.map((e) => e.category)));
    const t0 = events.length ? Date.parse(events[0].occurredAt) : 0;
    const span = events.length ? Date.parse(events[events.length - 1].occurredAt) - t0 : 1;
    return cats.map((c) => ({
      cat: c,
      points: events.filter((e) => e.category === c).map((e) => ({
        x: ((Date.parse(e.occurredAt) - t0) / Math.max(1, span)) * 100,
      })),
    }));
  }, [events]);

  return (
    <div className="tracing-timeline">
      {lanes.map((l) => (
        <div key={l.cat} className="tracing-timeline__lane">
          <span className="tracing-timeline__label">{l.cat}</span>
          <div className="tracing-timeline__track">
            {l.points.map((p, i) => (
              <span
                key={i}
                className="tracing-timeline__pt"
                style={{ left: `${p.x}%`, background: CATEGORY_COLOR[l.cat] ?? "#888" }}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
```

CSS:

```css
.tracing-timeline { padding: var(--space-2); display: flex; flex-direction: column; gap: var(--space-1); }
.tracing-timeline__lane { display: flex; gap: var(--space-2); align-items: center; }
.tracing-timeline__label { width: 120px; color: var(--text-secondary); font-size: var(--fs-2xs); }
.tracing-timeline__track { position: relative; flex: 1; height: 12px; background: var(--bg-elevated); border-radius: 6px; }
.tracing-timeline__pt { position: absolute; top: 2px; width: 2px; height: 8px; }
```

- [ ] **Step 5: `bun run typecheck`** → success.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/tracing/ desktop-ui/src/styles/tracing.css
git commit -m "feat(ui): WireFilters view modes (chart/events/timeline/decisions)"
```

---

## Phase 39 — Frontend: TurnTree

### Task 39.1: Left-side navigation tree

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/TurnTree.tsx`

- [ ] **Step 1: Write**

```tsx
import { useMemo, useState } from "react";
import type { TraceEvent } from "./types";

interface Props {
  events: TraceEvent[];
  onScrollToSeq: (seq: number) => void;
}

export function TurnTree({ events, onScrollToSeq }: Props) {
  const turns = useMemo(() => {
    const t: { seq: number; index: number; label: string; steps: { seq: number; index: number; toolCount: number }[] }[] = [];
    let cur: typeof t[number] | null = null;
    let curStep: { seq: number; index: number; toolCount: number } | null = null;
    for (const e of events) {
      if (e.category === "turnBegin") {
        if (cur) t.push(cur);
        const label = extractTitle(e.payload);
        cur = { seq: e.seq, index: e.turnIndex ?? 0, label, steps: [] };
        curStep = null;
      } else if (e.category === "stepBegin" && cur) {
        curStep = { seq: e.seq, index: e.stepIndex ?? 0, toolCount: 0 };
        cur.steps.push(curStep);
      } else if (e.category === "toolCall" && curStep) {
        curStep.toolCount++;
      }
    }
    if (cur) t.push(cur);
    return t;
  }, [events]);

  const [open, setOpen] = useState<Set<number>>(new Set([0]));
  const toggle = (idx: number) => setOpen((s) => { const n = new Set(s); n.has(idx) ? n.delete(idx) : n.add(idx); return n; });

  return (
    <div className="tracing-tree">
      <div className="tracing-tree__header">Navigation</div>
      {turns.map((t) => (
        <div key={t.seq} className="tracing-tree__turn">
          <button type="button" className="tracing-tree__turn-h" onClick={() => toggle(t.index)}>
            {open.has(t.index) ? "▾" : "▸"} Turn {t.index + 1}
          </button>
          {open.has(t.index) && (
            <div className="tracing-tree__steps">
              <div className="tracing-tree__label" onClick={() => onScrollToSeq(t.seq)}>{t.label.slice(0, 40)}</div>
              {t.steps.map((s) => (
                <button
                  key={s.seq}
                  type="button"
                  className="tracing-tree__step"
                  onClick={() => onScrollToSeq(s.seq)}
                >Step {s.index + 1}{s.toolCount > 0 ? ` (${s.toolCount} tool${s.toolCount === 1 ? "" : "s"})` : ""}</button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

function extractTitle(payload: any): string {
  if (typeof payload?.user_input === "string") return `"${payload.user_input}"`;
  if (Array.isArray(payload?.user_input)) {
    const t = payload.user_input.find((p: any) => p.type === "text")?.text;
    return t ? `"${t}"` : "Turn";
  }
  return "Turn";
}
```

- [ ] **Step 2: Wire into `WireEventsTab.tsx`**

Wrap the body in a flex row:

```tsx
<div className="tracing-events__split">
  <TurnTree events={events} onScrollToSeq={(seq) => {
    const pos = filtered.findIndex((e) => e.seq === seq);
    if (pos >= 0) ref.current?.scrollToIndex({ index: pos, align: "start" });
  }} />
  <div className="tracing-events__main">
    {/* existing Virtuoso/Chart/Timeline */}
  </div>
</div>
```

Import:

```tsx
import { TurnTree } from "./TurnTree";
```

- [ ] **Step 3: CSS**

```css
.tracing-events__split { display: flex; flex: 1 1 auto; overflow: hidden; }
.tracing-events__main { flex: 1 1 auto; overflow: hidden; }
.tracing-tree { width: 220px; flex-shrink: 0; border-right: 1px solid var(--border-subtle); overflow-y: auto; padding: var(--space-2); font-size: var(--fs-xs); }
.tracing-tree__header { color: var(--text-secondary); margin-bottom: var(--space-2); letter-spacing: 0.05em; }
.tracing-tree__turn-h { background: transparent; border: 0; color: var(--text-primary); cursor: pointer; padding: var(--space-1) 0; font-size: var(--fs-xs); }
.tracing-tree__steps { padding-left: var(--space-3); }
.tracing-tree__label { color: var(--text-tertiary); font-style: italic; padding: var(--space-1) 0; cursor: pointer; }
.tracing-tree__step { display: block; background: transparent; border: 0; color: var(--text-secondary); cursor: pointer; padding: 2px 0; font-size: var(--fs-2xs); text-align: left; }
.tracing-tree__step:hover { color: var(--text-primary); }
```

- [ ] **Step 4: `bun run typecheck`** → success.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(ui): TurnTree navigation"
```

---

## Phase 40 — Frontend: ToolCallDetail sidecar

### Task 40.1: ToolCallDetail component

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/event-cards/ToolCallDetail.tsx`

- [ ] **Step 1: Write**

```tsx
import { useMemo } from "react";
import type { TraceEvent } from "../types";

interface Props {
  selected: TraceEvent;
  allEvents: TraceEvent[];
  onClose: () => void;
}

export function ToolCallDetail({ selected, allEvents, onClose }: Props) {
  const callId = (selected.payload as any)?.id;
  const pair = useMemo(() => {
    const call = selected.category === "toolCall" ? selected : allEvents.find((e) => e.category === "toolCall" && (e.payload as any)?.id === callId);
    const result = selected.category === "toolResult" ? selected : allEvents.find((e) => e.category === "toolResult" && (e.payload as any)?.id === callId);
    return { call, result };
  }, [selected, allEvents, callId]);

  const fn = (pair.call?.payload as any)?.function;
  const name = fn?.name ?? "(tool)";
  const args = fn?.arguments ?? "";
  const content = (pair.result?.payload as any)?.content;
  const ok = !((pair.result?.payload as any)?.is_error);

  return (
    <aside className="tracing-detail-pane">
      <div className="tracing-detail-pane__hdr">
        <span>Tool: {name}</span>
        <button type="button" onClick={onClose}>×</button>
      </div>
      <section>
        <h4>Arguments</h4>
        <pre className="tracing-detail-pane__pre">{prettyJson(args)}</pre>
      </section>
      <section>
        <h4>Result {ok ? "" : "(error)"}</h4>
        <pre className="tracing-detail-pane__pre">{typeof content === "string" ? content : JSON.stringify(content, null, 2)}</pre>
      </section>
    </aside>
  );
}

function prettyJson(s: string): string {
  try { return JSON.stringify(JSON.parse(s), null, 2); } catch { return s; }
}
```

- [ ] **Step 2: CSS**

```css
.tracing-detail-pane {
  width: 360px; flex-shrink: 0; border-left: 1px solid var(--border-subtle);
  overflow-y: auto; padding: var(--space-2); background: var(--bg-elevated);
}
.tracing-detail-pane__hdr { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-2); }
.tracing-detail-pane__hdr button { background: transparent; border: 0; color: var(--text-secondary); cursor: pointer; font-size: var(--fs-md); }
.tracing-detail-pane__pre {
  font-family: var(--font-mono); font-size: var(--fs-2xs);
  background: var(--bg-base); padding: var(--space-2); border-radius: var(--radius-sm);
  white-space: pre-wrap; word-break: break-word;
}
```

- [ ] **Step 3: Wire into `WireEventsTab.tsx`**

In the events split, after `tracing-events__main`:

```tsx
{selectedEventSeq !== null && (() => {
  const ev = events.find((e) => e.seq === selectedEventSeq);
  return ev ? <ToolCallDetail selected={ev} allEvents={events} onClose={() => setSelectedEventSeq(null)} /> : null;
})()}
```

Import:

```tsx
import { ToolCallDetail } from "./event-cards/ToolCallDetail";
```

- [ ] **Step 4: `bun run typecheck`** → success.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(ui): ToolCallDetail sidecar"
```

---

## Phase 41 — Frontend: ContextMessagesTab

### Task 41.1: ContextMessagesTab

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/ContextMessagesTab.tsx`

- [ ] **Step 1: Write**

```tsx
import { useEffect, useMemo, useState } from "react";
import { Virtuoso } from "react-virtuoso";
import { invoke } from "@/api/client";
import type { ContextMessage, Scope } from "./types";

interface Props { providerId: string; sessionId: string; scope: Scope; }

export function ContextMessagesTab({ providerId, sessionId, scope }: Props) {
  const [msgs, setMsgs] = useState<ContextMessage[]>([]);
  const [search, setSearch] = useState("");

  useEffect(() => {
    let cancelled = false;
    invoke<ContextMessage[]>("tracing_load_context", { providerId, sessionId, scope })
      .then((m) => { if (!cancelled) setMsgs(m); });
    return () => { cancelled = true; };
  }, [providerId, sessionId, JSON.stringify(scope)]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return msgs;
    return msgs.filter((m) => JSON.stringify(m.content).toLowerCase().includes(q));
  }, [msgs, search]);

  return (
    <div className="tracing-context">
      <input
        type="search" placeholder="Search context…"
        value={search} onChange={(e) => setSearch(e.target.value)}
        className="tracing-context__search"
      />
      <div className="tracing-context__body">
        <Virtuoso
          data={filtered}
          itemContent={(_, m) => <ContextRow msg={m} />}
        />
      </div>
    </div>
  );
}

function ContextRow({ msg }: { msg: ContextMessage }) {
  const [open, setOpen] = useState(false);
  const role = msg.role.replace(/^_/, "");
  const text = pickText(msg.content);
  return (
    <div className="tracing-context__row">
      <button type="button" className="tracing-context__role" onClick={() => setOpen((v) => !v)}>
        {open ? "▾" : "▸"} <span className={`tracing-context__role-pill tracing-context__role-pill--${role}`}>{role}</span>
      </button>
      {!open && <span className="tracing-context__preview">{text.slice(0, 120)}</span>}
      {open && <pre className="tracing-context__full">{JSON.stringify(msg.content, null, 2)}</pre>}
    </div>
  );
}

function pickText(content: any): string {
  if (typeof content?.content === "string") return content.content;
  if (typeof content?.text === "string") return content.text;
  return JSON.stringify(content);
}
```

- [ ] **Step 2: CSS**

```css
.tracing-context { display: flex; flex-direction: column; flex: 1 1 auto; overflow: hidden; }
.tracing-context__search { margin: var(--space-2) var(--space-3); padding: var(--space-1) var(--space-2); background: var(--bg-input); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); color: var(--text-primary); font-size: var(--fs-base); }
.tracing-context__body { flex: 1 1 auto; overflow: hidden; }
.tracing-context__row { padding: var(--space-1) var(--space-3); border-bottom: 1px solid var(--border-subtle); }
.tracing-context__role { background: transparent; border: 0; color: var(--text-secondary); cursor: pointer; }
.tracing-context__role-pill { padding: 2px 6px; border-radius: var(--radius-sm); font-size: var(--fs-2xs); }
.tracing-context__role-pill--system_prompt { background: rgba(160,124,255,0.12); color: var(--accent-violet); }
.tracing-context__role-pill--user { background: rgba(106,170,255,0.12); color: var(--accent-blue); }
.tracing-context__role-pill--assistant { background: rgba(105,209,138,0.12); color: var(--accent-green); }
.tracing-context__role-pill--tool { background: rgba(192,96,255,0.12); color: var(--type-violet); }
.tracing-context__role-pill--checkpoint { background: rgba(214,138,58,0.12); color: var(--type-orange); }
.tracing-context__role-pill--usage { background: rgba(214,138,58,0.12); color: var(--type-orange); }
.tracing-context__preview { color: var(--text-secondary); margin-left: var(--space-2); font-size: var(--fs-xs); }
.tracing-context__full { font-family: var(--font-mono); font-size: var(--fs-2xs); padding: var(--space-2); background: var(--bg-base); border-radius: var(--radius-sm); white-space: pre-wrap; }
```

- [ ] **Step 3: Wire into `SessionDetailView.tsx`**

```tsx
{tab === "contextMessages" && (
  <ContextMessagesTab providerId={providerId} sessionId={sessionId} scope={scope} />
)}
```

Import:

```tsx
import { ContextMessagesTab } from "./ContextMessagesTab";
```

- [ ] **Step 4: `bun run typecheck`** → success.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(ui): ContextMessagesTab"
```

---

## Phase 42 — Frontend: StateTab

### Task 42.1: StateTab

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/StateTab.tsx`

- [ ] **Step 1: Write**

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SessionState } from "./types";

interface Props { providerId: string; sessionId: string; }

export function StateTab({ providerId, sessionId }: Props) {
  const [state, setState] = useState<SessionState | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<SessionState>("tracing_load_state", { providerId, sessionId })
      .then((s) => { if (!cancelled) setState(s); });
    return () => { cancelled = true; };
  }, [providerId, sessionId]);

  if (!state) return <div className="tracing-state">Loading…</div>;

  return (
    <div className="tracing-state-tab">
      <section>
        <h3>Session</h3>
        <dl className="tracing-state-tab__dl">
          <dt>Title</dt><dd>{state.customTitle ?? "—"}</dd>
          <dt>Plan mode</dt><dd>{state.planMode ? "yes" : "no"}</dd>
          <dt>Archived</dt><dd>{state.archived ? "yes" : "no"}</dd>
        </dl>
      </section>
      <section>
        <h3>Todos ({state.todos.length})</h3>
        <ul className="tracing-state-tab__todos">
          {state.todos.map((t, i) => (
            <li key={i}>
              <span className={`tracing-state-tab__pill tracing-state-tab__pill--${t.status}`}>{t.status}</span>
              {t.title}
            </li>
          ))}
        </ul>
      </section>
      <details>
        <summary>Raw state.json</summary>
        <pre className="tracing-state-tab__raw">{JSON.stringify(state.raw, null, 2)}</pre>
      </details>
    </div>
  );
}
```

- [ ] **Step 2: CSS**

```css
.tracing-state-tab { padding: var(--space-3); overflow-y: auto; flex: 1 1 auto; }
.tracing-state-tab section { margin-bottom: var(--space-3); }
.tracing-state-tab h3 { font-size: var(--fs-md); margin-bottom: var(--space-2); }
.tracing-state-tab__dl { display: grid; grid-template-columns: 120px 1fr; row-gap: var(--space-1); }
.tracing-state-tab__dl dt { color: var(--text-secondary); }
.tracing-state-tab__todos { list-style: none; padding: 0; display: flex; flex-direction: column; gap: var(--space-1); }
.tracing-state-tab__pill { padding: 2px 6px; border-radius: var(--radius-sm); margin-right: var(--space-2); font-size: var(--fs-2xs); }
.tracing-state-tab__pill--done { background: rgba(105,209,138,0.12); color: var(--accent-green); }
.tracing-state-tab__pill--in_progress { background: rgba(214,138,58,0.12); color: var(--type-orange); }
.tracing-state-tab__pill--cancelled { background: rgba(255,107,107,0.12); color: var(--accent-red); }
.tracing-state-tab__raw { font-family: var(--font-mono); font-size: var(--fs-2xs); padding: var(--space-2); background: var(--bg-base); border-radius: var(--radius-sm); white-space: pre-wrap; }
```

- [ ] **Step 3: Wire**

In `SessionDetailView.tsx`:

```tsx
{tab === "state" && <StateTab providerId={providerId} sessionId={sessionId} />}
```

Import: `import { StateTab } from "./StateTab";`

- [ ] **Step 4: `bun run typecheck`** → success.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(ui): StateTab"
```

---

## Phase 43 — Frontend: DualTab

### Task 43.1: DualTab — split view, scroll-locked

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/DualTab.tsx`

- [ ] **Step 1: Write**

```tsx
import type { Scope, SessionDetail } from "./types";
import { ContextMessagesTab } from "./ContextMessagesTab";
import { WireEventsTab } from "./WireEventsTab";

interface Props {
  providerId: string;
  sessionId: string;
  scope: Scope;
  detail: SessionDetail;
}

export function DualTab({ providerId, sessionId, scope, detail }: Props) {
  return (
    <div className="tracing-dual">
      <div className="tracing-dual__pane">
        <WireEventsTab events={detail.events} subagents={[]} onOpenSubagent={() => {}} />
      </div>
      <div className="tracing-dual__pane">
        <ContextMessagesTab providerId={providerId} sessionId={sessionId} scope={scope} />
      </div>
    </div>
  );
}
```

CSS: `.tracing-dual { display: grid; grid-template-columns: 1fr 1fr; gap: 0; flex: 1 1 auto; overflow: hidden; } .tracing-dual__pane { overflow: hidden; border-right: 1px solid var(--border-subtle); display: flex; flex-direction: column; }`

(Scroll-locking is a v1.5 polish — bumped because synchronization across two virtuoso lists by occurrence-time is non-trivial. Document this as a known v1 limitation.)

- [ ] **Step 2: Wire in `SessionDetailView.tsx`**

```tsx
{tab === "dual" && <DualTab providerId={providerId} sessionId={sessionId} scope={scope} detail={detail} />}
```

Import: `import { DualTab } from "./DualTab";`

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(ui): DualTab side-by-side wire+context (no scroll-lock yet)"
```

---

## Phase 44 — Frontend: AgentsTab

### Task 44.1: AgentsTab Gantt view

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/AgentsTab.tsx`

- [ ] **Step 1: Write**

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SubagentSummary } from "./types";

interface Props { providerId: string; sessionId: string; onOpen: (agentId: string) => void; }

export function AgentsTab({ providerId, sessionId, onOpen }: Props) {
  const [subs, setSubs] = useState<SubagentSummary[]>([]);
  useEffect(() => {
    let c = false;
    invoke<SubagentSummary[]>("tracing_list_subagents", { providerId, sessionId })
      .then((s) => { if (!c) setSubs(s); });
    return () => { c = true; };
  }, [providerId, sessionId]);

  if (subs.length === 0) return <div className="tracing-state">No subagents in this session.</div>;

  const t0 = Math.min(...subs.map((s) => Date.parse(s.createdAt)));
  const t1 = Math.max(...subs.map((s) => Date.parse(s.updatedAt)));
  const span = Math.max(1, t1 - t0);

  return (
    <div className="tracing-agents">
      {subs.map((s) => {
        const left = ((Date.parse(s.createdAt) - t0) / span) * 100;
        const width = ((Date.parse(s.updatedAt) - Date.parse(s.createdAt)) / span) * 100;
        return (
          <div key={s.agentId} className="tracing-agents__row">
            <span className="tracing-agents__label">{s.description ?? s.agentId}</span>
            <div className="tracing-agents__track">
              <button
                type="button"
                className="tracing-agents__bar"
                style={{ left: `${left}%`, width: `${Math.max(2, width)}%` }}
                onClick={() => onOpen(s.agentId)}
                title={`${s.subagentType} · ${s.eventCount} events`}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: CSS**

```css
.tracing-agents { padding: var(--space-3); display: flex; flex-direction: column; gap: var(--space-1); overflow-y: auto; flex: 1 1 auto; }
.tracing-agents__row { display: flex; align-items: center; gap: var(--space-2); }
.tracing-agents__label { width: 200px; color: var(--text-secondary); font-size: var(--fs-xs); }
.tracing-agents__track { position: relative; flex: 1; height: 14px; background: var(--bg-elevated); border-radius: var(--radius-sm); }
.tracing-agents__bar { position: absolute; top: 1px; bottom: 1px; background: var(--accent-violet); border: 0; border-radius: var(--radius-sm); cursor: pointer; }
```

- [ ] **Step 3: Wire**

```tsx
{tab === "agents" && (
  <AgentsTab providerId={providerId} sessionId={sessionId} onOpen={(agentId) => {
    setScope({ kind: "subagent", agentId });
    setTab("wireEvents");
  }} />
)}
```

Import: `import { AgentsTab } from "./AgentsTab";`

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(ui): AgentsTab gantt timeline"
```

---

## Phase 45 — Frontend: StatisticsView

### Task 45.1: StatisticsView wired to `tracing_stats`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/tracing/StatisticsView.tsx`

- [ ] **Step 1: Write**

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { StatsBundle } from "./types";

interface Props { providerId: string; }

export function StatisticsView({ providerId }: Props) {
  const [stats, setStats] = useState<StatsBundle | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let c = false;
    setLoading(true);
    invoke<StatsBundle>("tracing_stats", { providerId })
      .then((s) => { if (!c) { setStats(s); setLoading(false); } });
    return () => { c = true; };
  }, [providerId]);

  if (loading) return <div className="tracing-state">Loading stats…</div>;
  if (!stats) return null;

  return (
    <div className="tracing-stats">
      <section>
        <h3>Per project</h3>
        <table className="tracing-stats__table">
          <thead><tr><th>Project</th><th>Sessions</th><th>Turns</th><th>Tools</th><th>Errors</th><th>Tokens (in/out)</th></tr></thead>
          <tbody>
            {stats.perProject.map((p) => (
              <tr key={p.projectBasename}>
                <td>{p.projectBasename}</td>
                <td>{p.sessionCount}</td>
                <td>{p.turnCount}</td>
                <td>{p.toolCallCount}</td>
                <td>{p.errorCount}</td>
                <td>{(p.totalInputTokens / 1000).toFixed(1)}k / {(p.totalOutputTokens / 1000).toFixed(1)}k</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section>
        <h3>Tool usage (heatmap)</h3>
        <div className="tracing-stats__heatmap">
          {stats.toolUsage.map((t) => {
            const errPct = t.callCount > 0 ? (t.errorCount / t.callCount) : 0;
            const hue = (1 - errPct) * 120; // green → red
            return (
              <div key={t.tool} className="tracing-stats__heatcell" style={{ background: `hsl(${hue}, 60%, 30%)` }}>
                <strong>{t.tool}</strong>
                <span>{t.callCount}</span>
                {t.errorCount > 0 && <em>{t.errorCount} err</em>}
              </div>
            );
          })}
        </div>
      </section>

      <section>
        <h3>Errors by tool</h3>
        <table className="tracing-stats__table">
          <thead><tr><th>Tool</th><th>Errors</th></tr></thead>
          <tbody>
            {stats.errorsByTool.map((e) => (
              <tr key={e.tool}><td>{e.tool}</td><td>{e.errorCount}</td></tr>
            ))}
          </tbody>
        </table>
      </section>

      <section>
        <h3>Daily token spend</h3>
        <DailyChart points={stats.tokenSeries} />
      </section>

      <section>
        <h3>Subagent types</h3>
        <table className="tracing-stats__table">
          <thead><tr><th>Type</th><th>Count</th></tr></thead>
          <tbody>
            {stats.subagentTypes.map((s) => (
              <tr key={s.subagentType}><td>{s.subagentType}</td><td>{s.count}</td></tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}

function DailyChart({ points }: { points: import("./types").TokenSeriesPoint[] }) {
  if (points.length === 0) return <div className="tracing-state">No data.</div>;
  const w = 800, h = 120;
  const maxIn = Math.max(1, ...points.map((p) => p.inputTokens));
  const path = points.map((p, i) => {
    const x = (i / (points.length - 1 || 1)) * w;
    const y = h - (Number(p.inputTokens) / maxIn) * (h - 4);
    return (i === 0 ? "M" : "L") + `${x.toFixed(1)} ${y.toFixed(1)}`;
  }).join(" ");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="tracing-stats__chart">
      <path d={path} fill="none" stroke="var(--accent-violet)" strokeWidth="1.5" />
    </svg>
  );
}
```

- [ ] **Step 2: CSS**

```css
.tracing-stats { padding: var(--space-3); display: flex; flex-direction: column; gap: var(--space-3); overflow-y: auto; flex: 1 1 auto; }
.tracing-stats h3 { font-size: var(--fs-md); margin-bottom: var(--space-2); }
.tracing-stats__table { width: 100%; border-collapse: collapse; font-size: var(--fs-xs); }
.tracing-stats__table th, .tracing-stats__table td { padding: var(--space-1) var(--space-2); text-align: left; border-bottom: 1px solid var(--border-subtle); }
.tracing-stats__table th { color: var(--text-secondary); font-weight: 500; }
.tracing-stats__heatmap { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: var(--space-1); }
.tracing-stats__heatcell { padding: var(--space-2); border-radius: var(--radius-sm); display: flex; flex-direction: column; gap: 2px; color: white; font-size: var(--fs-xs); }
.tracing-stats__heatcell em { font-style: normal; color: var(--accent-red); }
.tracing-stats__chart { width: 100%; height: 120px; }
```

- [ ] **Step 3: Wire into `TracingTab.tsx`**

```tsx
{rootTab === "statistics" && <StatisticsView providerId={providerId} />}
```

Import: `import { StatisticsView } from "./StatisticsView";`

- [ ] **Step 4: `bun run typecheck` + `bun run lint`** → success.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(ui): StatisticsView with project totals, heatmap, charts"
```

---

## Phase 46 — Frontend: Provider chip row

### Task 46.1: Show registered providers + select

**Files:**
- Modify: `desktop-ui/src/features/plugins/coding-memory/tracing/TracingTab.tsx`

- [ ] **Step 1: Add a hook**

```tsx
const [providers, setProviders] = useState<ProviderInfo[]>([]);
useEffect(() => {
  invoke<ProviderInfo[]>("tracing_list_providers").then(setProviders).catch(() => {});
}, []);
```

Render above the rootTab buttons:

```tsx
<div className="tracing-providers">
  {providers.map((p) => (
    <button
      key={p.id}
      type="button"
      className={"tracing-provider" + (providerId === p.id ? " tracing-provider--active" : "")}
      onClick={() => setProviderId(p.id)}
    >{p.displayName} ({p.sessionCount})</button>
  ))}
</div>
```

CSS:

```css
.tracing-providers { display: flex; gap: var(--space-1); padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--border-subtle); }
.tracing-provider {
  padding: var(--space-1) var(--space-2);
  background: var(--bg-elevated); border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm); color: var(--text-primary); cursor: pointer; font-size: var(--fs-xs);
}
.tracing-provider--active { border-color: var(--accent-violet); }
```

- [ ] **Step 2: Commit**

```bash
git commit -am "feat(ui): provider chip row in TracingTab"
```

---

## Phase 47 — Final polish + smoke test

### Task 47.1: Match kimi-vis palette + typography

- [ ] **Step 1: Audit `tracing.css` for any hardcoded font-sizes; replace with `--fs-*`**

```bash
grep -n 'font-size: [0-9]' desktop-ui/src/styles/tracing.css
```

Replace any matches with the closest `--fs-*` token.

- [ ] **Step 2: Define palette variables**

In `desktop-ui/src/styles/ds-tokens.css`, append (if missing):

```css
:root {
  --type-blue: #6aaaff;
  --type-cyan: #5ad3d7;
  --type-purple: #a07cff;
  --type-violet: #c060ff;
  --type-green: #69d18a;
  --type-orange: #d68a3a;
  --type-yellow: #d4a83a;
  --accent-red: #ff6b6b;
}
```

- [ ] **Step 3: Commit**

```bash
git commit -am "chore(ui): tracing palette tokens + typography audit"
```

### Task 47.2: Manual smoke test

- [ ] **Step 1: Start the dev environment**

Terminal 1:

```bash
cd desktop-ui && bun run dev
```

Terminal 2:

```bash
cargo tauri dev
```

- [ ] **Step 2: Walk through each surface**

In the desktop app:
1. Open `Plugins → Coding Memory → Tracing`. Provider chip row shows "Kimi (N)".
2. Sessions list groups by project. Each card shows id / title / wire+context badges / size / counts / duration.
3. Sort dropdown changes order. Search filters. Group toggle flattens. Imported toggle empties the list when no imported sessions.
4. Click a session. Detail header shows correct counts. The five tabs all render. Wire Events tab shows a virtualized list of category-dispatched cards. Filter presets work. Search highlights matches. `e` cycles through error events. `/` focuses search.
5. Scope selector switches to a subagent and the event list updates.
6. Context Messages, State, Dual, Agents tabs all render real data.
7. Open Dir launches Finder. Copy DIR puts the path on the clipboard.
8. Statistics tab shows real aggregations.
9. Click Import, pick a `wire.jsonl`, the new session appears as imported.

If any step fails, file as a follow-up bug; do not block release on Dual scroll-lock (known v1.5 limitation per Phase 43).

- [ ] **Step 3: Run all tests**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cd desktop-ui && bun run typecheck && bun run lint && bun run test
```

All green.

- [ ] **Step 4: Final commit**

```bash
git commit --allow-empty -m "chore(tracing): v1 ready for review"
```

---

## Out of scope for v1 (deferred)

- **Live tail / file watcher.** Refresh button is the v1 mechanism.
- **Dual tab scroll-lock.** Side-by-side view ships, scroll synchronization deferred.
- **Live updates of session counts in provider chips.** Refresh-on-mount only.
- **Cross-CLI providers.** Trait + DTOs are CLI-agnostic; only Kimi implemented.
- **Persisted raw envelopes in SQLite.** Live-read only; Import button covers the durability use case.
- **Download wire.jsonl button.** Rendered in the header for chrome parity but the click handler is a no-op in v1; wiring `tauri-plugin-dialog`'s save dialog is a 10-line follow-up.

## Risks / known issues

- `cargo nextest` doesn't run doctests; remember `cargo test --workspace --doc` if anyone adds doctests to tracing modules.
- The `dirs::home_dir()` lookup in init relies on the `dirs` crate. If it's not already a workspace dep, switch to `home::home_dir()` or read from `Config`.
- `KlyntbotError::NotFound` may not exist in the codebase verbatim — check `crates/common/src/lib.rs` and adjust to the closest existing variant in Phase 7.
- `formatTime` import path in event cards is `@/shared/lib/dates` per CLAUDE.md; verify before relying on it.
