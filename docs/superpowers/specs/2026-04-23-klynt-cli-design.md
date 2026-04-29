# Klynt CLI — Design

**Date:** 2026-04-23
**Status:** Superseded by [`docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`](./2026-04-29-klynt-coding-in-chat-design.md). The "coding capability in a separate `klynt` TUI binary" approach has been replaced with "coding capability inside the existing desktop chat surface." The architectural decisions here (3-layer approval, sandbox model, hook engine, skills, event vocabulary, coding-memory coordination) carry forward; the TUI/binary/Wire/multi-process-coordination chapters are obsolete. Body left intact for historical reference.
**Scope:** Single design. Implementation plan will be derived via `writing-plans`.
**Pre-release policy:** Per CLAUDE.md — no user data to migrate, no backward-compat shims, no feature-flag gating. Schema changes consolidated into Phase 1.
**Companion spec:** `docs/superpowers/specs/2026-04-22-coding-memory-design.md` (amendments listed in §14).

---

## 1. Vision and non-goals

### Vision

**Klynt-cli is klyntbot's native coding CLI.** It runs as an in-process binary inside the `bot/` workspace that shares klyntbot's `AgentRuntime`, `cognitive` store, `skill-system`, `providers`, `mcp` client, and (via the in-flight coding-memory spec) `coding-memory` crate. Its reason for existing is not to compete with Codex/Claude Code/Kimi/OpenCode as a standalone product but to be **the richest data source klyntbot's cognitive subsystems will ever have**. Every coding turn emits structured, klyntbot-native events that `Distiller`/`Mirror`/`Reforge` consume at higher fidelity than any external-CLI adapter ever could. It's excellent on its own for daily use — TUI, slash commands, sandboxed shell, MCP, portable skill ecosystem, diff-aware edits — but its competitive moat is *what klyntbot learns from it*. A user running klynt-cli for a month has a cognitive store that knows their code, their failure patterns, their style, and their project-evolved skills at a depth other tools categorically cannot reach.

### Goals

- One native coding CLI that serves as klyntbot's premier Distiller input, with ≥5x the structured event volume per turn that external-CLI adapters produce.
- In-process with klyntbot: no IPC tax, no serialization, no "is the daemon running" failure mode for core paths.
- Fully usable on a fresh machine without klyntbot desktop running — in-process Distiller handles ingestion; desktop hands off when alive.
- Excellent standalone UX: TUI on par with Codex/Claude Code; sandboxed shell; approval model that learns from Mirror; skill ecosystem rooted in `.klyntbot/skills/`.
- Reuse-first: Codex donates `execpolicy`/`sandboxing`/`hooks`/`rollout`/`protocol`/`tui` patterns (renamed to `klynt-*`); we write `klynt-core` fresh against klyntbot's crates.
- Zero regression on existing klyntbot chat-channel functionality.

### Non-goals

- Standalone product positioning, marketplace listing, public distribution before user is satisfied with internal use.
- Protocol compatibility with Kimi/Codex/Claude Code observers. Wire exists for klyntbot-internal use, not external interop.
- Running without klyntbot crates at all (i.e., klynt-cli as a dependency-free CLI). If you don't want klyntbot, use one of the four reference CLIs.
- Feature-parity with Claude Code's entire surface (vim mode, voice, coordinator mode, IDE bridge). We cherry-pick patterns that serve the harness-engineering thesis; we skip what doesn't.
- Full-fidelity IDE integration in Phase 1. Wire is observer-only in Phase 1; bidirectional control + ACP are Phase 2+ if-and-when.
- Cloud sync, multi-user, team features. Local-first only.
- Cross-ecosystem skill auto-discovery from `~/.claude/skills/` etc. Skills come from `.klyntbot/skills/` only; bringing in external skills requires explicit `klynt skills install`.

---

## 2. Architecture overview + component diagram

### Component diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         klynt (binary, single process)                    │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │                      klynt-tui (ratatui + crossterm)              │    │
│  │  • streaming markdown renderer • slash command palette            │    │
│  │  • bottom-pane modal forms     • file picker • status line        │    │
│  │  • Mirror live panel (Phase 2) • Cost tracker line                │    │
│  └────────────────────┬─────────────────────────────────────────────┘    │
│                       │ in-process channels                               │
│                       ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │                       klynt-core (orchestrator)                   │    │
│  │  • configures AgentRuntime with coding tools + event_tx           │    │
│  │  • owns the broadcast broker + subscribers                        │    │
│  │  • approval-flow controller (3 layers + privacy guard)            │    │
│  │  • sandbox glue • diff preview • MCP gateway                      │    │
│  │  • per-tool sandbox-aware bash, file ops, recall_*                │    │
│  └─┬───────┬───────┬───────┬───────┬──────┬─────────┬───────────────┘    │
│    │       │       │       │       │      │         │                    │
│  ┌─▼─┐  ┌─▼─┐  ┌──▼──┐  ┌─▼──┐ ┌─▼──┐ ┌─▼──┐  ┌──▼──┐                   │
│  │san│  │exe│  │hooks│  │roll│ │prot│ │ MCP│  │ tool│                   │
│  │box│  │pol│  │     │  │out │ │ocol│ │bridge│ │ kit│                   │
│  └───┘  └───┘  └─────┘  └────┘ └────┘ └────┘  └─────┘                   │
│   ▲ klynt-* infrastructure crates (adapted from Codex)                  │
│                                                                          │
│  ┌────────────────────▼─────────────────────────────────────────────┐    │
│  │       klyntbot crates linked in-process (cognitive pipeline)     │    │
│  │                                                                  │    │
│  │   agent::AgentRuntime                                            │    │
│  │     ├── IntentAnalyzer                                           │    │
│  │     ├── ContextEngine                                            │    │
│  │     ├── SkillRouter (extended w/ paths-conditional activation)   │    │
│  │     ├── execution::execute_loop (unified — current shape)        │    │
│  │     ├── MidLoopCompressor                                        │    │
│  │     └── CostTracker                                              │    │
│  │                                                                  │    │
│  │   skill-system  cognitive  context_engine  providers  mcp        │    │
│  │   bus  storage  config  notifications  autotuner                 │    │
│  │                                                                  │    │
│  │   coding-memory (from in-flight worktree spec)                   │    │
│  │     ├── CodingRecallService (read path)                          │    │
│  │     ├── Distiller (write path) ◄─────┐                           │    │
│  │     ├── coding-ingest::AgentEvent ◄──┼─ rich klynt-cli variants  │    │
│  │     └── memory_causal_edges, etc.    │                           │    │
│  └──────────────────────────────────────┼───────────────────────────┘    │
│                                         │                                 │
│  ┌──────────────────────────────────────▼───────────────────────────┐    │
│  │                  klynt-cli event fan-out broker                  │    │
│  │   tokio::sync::broadcast<agent::events::AgentEvent>              │    │
│  │   subscribers: [TUI, in-process Distiller OR ingest socket,      │    │
│  │                 klynt-rollout JSONL, hook engine, Wire (P2+)]    │    │
│  └─────┬────────────┬────────────┬───────────────┬─────────────────┘    │
└────────┼────────────┼────────────┼───────────────┼─────────────────────┘
         │            │            │               │
         ▼            ▼            ▼               ▼
   ┌─────────┐ ┌──────────────┐ ┌────────────┐ ┌────────────────────┐
   │ shell + │ │ ~/.klyntbot/ │ │ MCP servers│ │ LLM providers       │
   │ sandbox │ │  sessions/   │ │ (klyntbot- │ │ (via klyntbot's     │
   │ jail    │ │  ingest.sock │ │  mcp + ext)│ │  ProviderManager)   │
   │ (Seatbelt│ │  ingest-buff │ │            │ │                    │
   │ /Landlock│ │  project-skl │ │            │ │                    │
   │ /bwrap)  │ │  rule-artifs │ │            │ │                    │
   └─────────┘ └──────┬───────┘ └────────────┘ └────────────────────┘
                      │
                      ▼ (when desktop is running)
                ┌──────────────────────────────────────┐
                │  klyntbot desktop (Tauri 2)          │
                │  • owns Mirror, Reforge nightly      │
                │  • Coding Memory Workbench panels    │
                │  • full Distiller + rule artifacts   │
                │  • shares the same SQLite + LanceDB  │
                └──────────────────────────────────────┘
```

### Process model

Single Rust binary `klynt`. Links klyntbot's crates as normal Cargo deps; no IPC for the cognitive path. Klynt-cli's process *is* a klyntbot runtime — it just happens to wear a TUI hat instead of a desktop hat.

When klynt-cli starts:

1. Read `~/.klyntbot/config.json`.
2. Open `~/.klyntbot/data.db` (SQLite — multi-process safe via WAL mode).
3. Open `~/.klyntbot/lance/` (LanceDB) — lazy by default.
4. Construct an `AgentRuntime` with `ChannelName::new("coding_cli")` registered.
5. Check `~/.klyntbot/desktop.lock` (advisory file written by desktop on startup):
   - **Lock present, fresh (<60s heartbeat):** desktop is alive. Klynt-cli's `MemorySink` becomes `IngestSocketSink` writing to `~/.klyntbot/ingest.sock`. Desktop's Distiller, Mirror, Reforge own ingestion.
   - **Lock missing or stale:** desktop is off. Klynt-cli's `MemorySink` becomes `InProcessSink` invoking `coding-memory::Distiller` directly. We get fewer Mirror/Reforge benefits in the moment but never lose data.
6. Launch `klynt-tui` event loop; user prompts flow into `AgentRuntime::handle()`.

### Data flow per turn

```
user types prompt in TUI
     ↓
TUI → klynt-core::Session.submit(prompt)
     ↓
AgentRuntime::handle(channel="coding_cli", prompt, Some(event_tx))
     ├── IntentAnalyzer  → IntentClassified event ─────────┐
     ├── ContextEngine   → recall injection (CodingRecall) │
     │   ├── RecallInjected event ────────────────────────┤
     │   ├── DeadEndWarning event (if applicable) ────────┤
     │   └── ContextEngineDecision event ─────────────────┤
     ├── SkillRouter     → SkillActivationConsidered ─────┤   broadcast
     │                  → SkillActivated events ──────────┤   to all
     └── execute_loop()  → unified loop                    │   subscribers
            ├── ProviderRequest / ProviderResponse events ─┤   (TUI render,
            ├── streaming AssistantMsg chunks ─────────────┤   Distiller,
            ├── ToolCall events (with full args)           │   rollout
            ├── (if shell) sandbox check → ToolCall stream │   JSONL,
            ├── (if edit) FileEdit + tree-sitter symbols  ─┤   hooks,
            ├── parallel read-only tool dispatch ──────────┤   Wire P2+)
            ├── MidLoopCompressionTriggered (if needed) ──┤
            └── … final AssistantMsg + token usage ───────┘
```

### Desktop coordination model

Three states with clean transitions:

| State | klynt running | desktop running | Distiller location | Mirror/Reforge |
|---|---|---|---|---|
| Solo CLI | yes | no | klynt's process | not running |
| Coordinated | yes | yes | desktop's process (via socket) | desktop |
| Solo desktop | no | yes | desktop's process | desktop |
| Both off | no | no | nothing happening | nothing happening |

State transitions are observed via the `desktop.lock` file with 30-second heartbeats. If desktop dies mid-session, klynt-cli's MemorySink falls back to `InProcessSink` on the next event with a status-line warning. If desktop starts mid-session, klynt-cli switches to `IngestSocketSink` on the next event.

### File system layout

```
~/.klyntbot/
  config.json                        # shared with desktop
  data.db                            # shared SQLite (multi-process via WAL)
  lance/                             # shared LanceDB
  sessions/<session-id>/             # per-session directory (see §10)
    meta.json
    rollout.jsonl
    wire.sock                        # per-session observer socket
    wire.json                        # observer discovery sidecar
    snapshots/                       # Phase 2+ file snapshots for rewind
  ingest.sock                        # owned by desktop when alive
  ingest-buffer.jsonl                # fallback buffer (per coding-memory spec)
  desktop.lock                       # heartbeat file (PID + last-seen)
  project-skills/                    # Reforge-synthesized per-repo skills
  skills/                            # user-installed klyntbot skills
  rules/                             # Starlark approval rules (klynt-execpolicy)
  hooks.toml                         # third-party hooks (Phase 1 read; user-edited)
```

---

## 3. Crate layout + Codex adaptation map

### New crates added to `bot/crates/` (10 total)

Following klyntbot's flat-layout convention; dependency direction strictly upward:

| Crate | Layer | Purpose | Source |
|---|---|---|---|
| `klynt-protocol` | L0 | Event/Op/Submission types, `CodingTraceEvent` enum | Adapted from `codex-rs/protocol/` |
| `klynt-execpolicy` | L1 | Starlark prefix-rule approval engine; `~/.klyntbot/rules/*.rules` loader | Adapted from `codex-rs/execpolicy/` |
| `klynt-sandbox` | L1 | Seatbelt (.sbpl) policy gen for macOS, Landlock+bwrap for Linux; `SandboxPolicy` types | Adapted from `codex-rs/sandboxing/` |
| `klynt-sandbox-helper` | (binary) | Linux child-process helper that applies Landlock + seccomp | Adapted from `codex-rs/linux-sandbox/` |
| `klynt-hooks` | L2 | Hook engine: 13-event Claude-Code-compatible schema | Adapted from `codex-rs/hooks/`, retargeted to klyntbot's `AgentEvent` |
| `klynt-rollout` | L2 | JSONL session recorder writing to `~/.klyntbot/sessions/`; `klynt_sessions` index | Adapted from `codex-rs/rollout/` |
| `klynt-skill-loader` | L3 | `.klyntbot/skills/` + Reforge-path discovery, conditional activation | Fresh — extends `skill-system` |
| `klynt-core` | L7 | Coding-tool runtime registry, fan-out broker, `MemorySink` trait | Fresh — written against klyntbot crates |
| `klynt-tui` | L7 | ratatui+crossterm TUI: streaming markdown, slash commands, file picker, modal dialogs | Adapted from `codex-rs/tui/` |
| `klynt` | L8 | Binary entry point: clap dispatch, `--print`, `--plan`, `--yolo`, `--power`, `klynt skills install`, `klynt status` | Fresh |

### Codex adaptation rules

For each adapted crate:

- Module rename: `codex_*` → `klynt_*`
- Type rename: `CodexEvent` → `KlyntEvent`, etc.
- Path rewrites: `~/.codex/` → `~/.klyntbot/`
- Env var rewrites: `CODEX_API_KEY` → `KLYNT_API_KEY`
- License attribution preserved per Apache-2.0 (NOTICE file at workspace root + per-file `// Adapted from codex-rs/<crate>` comments)
- Upstream provenance pinned in `bot/crates/<klynt-crate>/VENDOR.md`
- Mechanical rename via `scripts/adapt_codex_vendor.sh` (uses ast-grep)

### Dependency graph

```
                 ┌───────────────────────────────────┐
                 │       klynt (binary, L8)          │
                 └───────────────┬───────────────────┘
                                 │
          ┌──────────────────────┼──────────────────────┐
          ▼                      ▼                      ▼
    ┌───────────┐         ┌──────────────┐       ┌─────────────┐
    │ klynt-tui │ ──────► │  klynt-core  │ ◄──── │klynt-skill- │
    │   (L7)    │         │    (L7)      │       │loader (L3)  │
    └───────────┘         └──────┬───────┘       └─────────────┘
                                 │
       ┌─────────────┬───────────┼────────────┬─────────────┬──────────┐
       ▼             ▼           ▼            ▼             ▼          ▼
   ┌────────┐   ┌────────┐  ┌────────┐  ┌─────────┐   ┌────────┐  ┌──────┐
   │ klynt- │   │ klynt- │  │ klynt- │  │  klynt- │   │ klynt- │  │klynt-│
   │  hooks │   │ rollout│  │sandbox │  │execpolicy│  │protocol│  │sandbox│
   │  (L2)  │   │  (L2)  │  │  (L1)  │  │   (L1)  │   │  (L0)  │  │helper│
   └───┬────┘   └────┬───┘  └────┬───┘  └────┬────┘   └───┬────┘  │(bin) │
       │            │           │            │            │       └──────┘
       └────────────┴───────────┴────────────┴────────────┘
                            │
                            ▼ all depend on:
                ┌──────────────────────────────────────┐
                │  klyntbot existing crates (unchanged │
                │  except for one extension to         │
                │  agent::events::AgentEvent + one to  │
                │  tools-core::Tool::is_concurrency_   │
                │  safe — see §4)                      │
                └──────────────────────────────────────┘
```

The `klynt-skill-loader` at L3 is the only crate that extends an existing klyntbot surface (`skill-system::SkillRouter`); all other crates are net-new and additive.

---

## 4. Agent loop: how klynt-cli plugs into the unified execute loop

### Current state of the agent crate (verified 2026-04-23)

`crates/agent/src/execution/execute_loop.rs:1` declares:

> *Unified execute loop — replaces DirectEngine, ReactiveEngine, and ExecutionRouter.*

There is no `ExecutionMode` enum and no `ExecutionRouter` trait. The unified `execute_loop()` is a free function:

```rust
pub async fn execute_loop(
    core: &ExecutionCore,
    mut messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    budget: &mut ExecutionBudget,
    ctx: &RoutingContext,
    event_tx: Option<mpsc::Sender<agent::events::AgentEvent>>,
) -> Result<ExecuteLoopResult>;
```

The `event_tx` parameter is documented in `events.rs:4` as *"allowing consumers (like the CLI) to display real-time progress"* — i.e., the seam for klynt-cli is already designed in.

### Klynt-cli's plug-in approach (zero-change-to-AgentRuntime)

Three things klynt-cli does at startup:

**1. Build an `AgentRuntime` with coding-shaped configuration:**
```rust
let runtime = AgentRuntime::new(provider, ...)
    .with_tool_registry(coding_tool_registry())     // curated kit (§5)
    .with_memory_service(coding_memory.unified_service())
    .with_context_update_queue(klynt_ctx_queue.clone())
    .with_user_situation(coding_situation_provider())
    .with_active_view(active_view_with_repo_context());
```

**2. Open an event channel + spawn the broker:**
```rust
let (event_tx, event_rx) = mpsc::channel::<agent::events::AgentEvent>(2048);
let broker = klynt_core::Broker::new(event_rx);
broker.spawn_subscribers([
    Box::new(klynt_tui::TuiSubscriber::new(...)),
    Box::new(klynt_rollout::JsonlSubscriber::new(...)),
    Box::new(klynt_core::MemorySinkSubscriber::new(memory_sink)),
    Box::new(klynt_hooks::HookSubscriber::new(hook_engine)),
    // Phase 2+: WireSubscriber
]);
```

**3. Run the user's input through `AgentRuntime::handle()`** with `Some(event_tx)`. The runtime's existing handle path calls `execute_loop()` internally; events flow into the broker.

### Coding specialization at three pluggable seams

#### Seam 1 — `ToolRegistry` content

Klynt-cli registers a coding-shaped tool set:

| Tool | Crate | Sandbox-aware | Approval-aware | Source |
|---|---|---|---|---|
| `bash` | `klynt-core::tools::bash` | yes (klynt-sandbox) | yes (klynt-execpolicy) | new |
| `read` / `glob` / `grep` | `klynt-core::tools::fs` | yes (read-only) | rule-checked | new |
| `edit` / `write` / `apply_patch` | `klynt-core::tools::edit` | yes | yes | new (apply_patch lifted from codex-apply-patch) |
| `task` (subagent) | klyntbot's existing `tools` | inherits | inherits | existing |
| `recall_*` (8 tools) | `coding-memory` | n/a | n/a | from in-flight worktree |
| `mcp_*` (gateway) | klyntbot's `mcp` | n/a | rule-checked | existing |
| Klyntbot domain (curated subset) | klyntbot existing | n/a | rule-checked | existing |

Each new tool consults `klynt-execpolicy::PolicyEngine::check`, wraps execution in `klynt-sandbox::Manager::run` when applicable, and emits both standard runtime events (`ToolStart`/`ToolEnd`) and rich klynt-only variants (`SandboxPolicyApplied`, `ApprovalEvaluated`, `ToolCallStreamChunk`).

#### Seam 2 — `RoutingContext` channel name

```rust
// in common/src/lib.rs
pub mod channels {
    pub const CODING_CLI: &str = "coding_cli";
    // existing: telegram, discord, slack, email, desktop
}
```

Klynt-cli constructs `RoutingContext { channel: ChannelName::new("coding_cli"), ... }`. Tool implementations gate coding-specific behavior on this string; `recall_*` tools boost relevance when channel matches.

#### Seam 3 — `agent::events::AgentEvent` enum extension

Add ~20 new variants (full list in §8). All additive under `#[non_exhaustive]`; chat-channel match arms must have `_ =>` catch-all (Phase 1 prerequisite audit).

### Required surgical changes

Two minimal, high-leverage changes to klyntbot crates:

1. **`tools-core::Tool` trait** — add `fn is_concurrency_safe(args: &Value) -> bool { false }` (default false).
2. **`crates/agent/src/execution/core.rs`** — extend `execute_tool_calls` to partition by `is_concurrency_safe` (parallel for safe, sequential for unsafe). ~30 lines.

Both edits benefit all channels, not just klynt-cli.

### Per-turn lifecycle

```
user types in TUI
   ↓
TUI → klynt-core::Session::submit(prompt)
   ↓
runtime.handle(channel="coding_cli", prompt, Some(event_tx)) {
   IntentAnalyzer → ContextEngine → SkillRouter → execute_loop()
}
   ↓
execute_loop(core, messages, tools, params, budget, ctx, Some(event_tx)) {
   loop {
      budget gate / cancellation check
      provider.stream() → emit ContentChunk events
      tool calls → run via core (parallel up to MAX_CONCURRENT_TOOLS=10
                   for is_concurrency_safe, sequential otherwise)
                   → emit ToolStart / ToolEnd events
      mid_loop_compressor.maybe_compress()
      live_context_refresher.maybe_refresh()
   }
}
   ↓
event_tx receives all events → broker fans out:
   ├── TuiSubscriber renders streaming text + tool indicators
   ├── JsonlSubscriber writes ~/.klyntbot/sessions/<id>/rollout.jsonl
   ├── MemorySinkSubscriber routes to in-process Distiller OR ingest socket
   ├── HookSubscriber matches PreToolUse/PostToolUse/etc. and runs subprocess hooks
   └── (Phase 2+) WireSubscriber forwards to attached observers
```

### Error handling matrix

| Failure | Behavior | Why |
|---|---|---|
| Tool call panic | Catch via `catch_unwind`; emit `ToolCall { ok: false, ... }`; continue loop | One bad tool doesn't kill the turn |
| Provider 5xx | Retry per `ProviderManager` policy | Inherits klyntbot's retry config |
| Provider 4xx | Emit `Error`; abort turn | Unrecoverable; user must intervene |
| Sandbox launch failure | Fall back to unsandboxed with prominent UI warning + tightened approval | OS gaps shouldn't block work; user knows |
| Hook subprocess timeout | Fail open; log to `mirror_snippets` | Prevent flaky hooks from blocking sessions |
| Approval denied | Tool returns `Err(ToolError::Denied)`; agent continues | Standard ReAct error path |
| MemorySink failure | Buffer to `~/.klyntbot/ingest-buffer.jsonl`; continue | Agent loop is sacred |
| Recall query timeout (>5s) | Return partial results + marker | Don't stall the turn for memory |

---

## 5. Tool surface + curation model

### Full tool inventory (the universe)

**Pool 1 — Coding tool kit (new, in `klynt-core::tools`):**

| Tool | Concurrency-safe | Sandbox required | Approval-aware |
|---|---|---|---|
| `bash` | no | yes | yes |
| `read` | yes | no | rule-checked |
| `glob` | yes | no | rule-checked |
| `grep` | yes | no | rule-checked |
| `edit` | no | yes | yes |
| `write` | no | yes | yes |
| `apply_patch` | no | yes | yes (lifted from codex-apply-patch) |
| `web_fetch` | no | n/a (network) | yes |
| `ask_user` | no | n/a | n/a |
| `enter_plan_mode` / `exit_plan_mode` | no | n/a | n/a |
| `notebook_edit` (Jupyter) | no | yes | yes |

**Pool 2 — Recall tool kit (from in-flight `coding-memory` worktree):**

| Tool | Phase |
|---|---|
| `recall_index` | 4 |
| `recall_timeline` | 4 |
| `recall_fetch` | 4 |
| `trace_causes` | 6 |
| `check_dead_ends` | 4 |
| `recall_facts_as_of` | 4 |
| `recall_change_history` | 4 |
| `recall_decision_points` | 4 |

All read-only and concurrency-safe.

**Pool 3 — Klyntbot domain tools:** existing 15 — `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `mirror`, `temporal`.

**Pool 4 — MCP gateway:** External MCP servers configured in `~/.klyntbot/config.json`, surfaced as `mcp_<server>_<tool>` per existing convention.

### Default curated set

Klynt-cli boots with **24 eager tools**:

```
Coding kit (12):
  bash, read, glob, grep, edit, write, apply_patch,
  ask_user, enter_plan_mode, exit_plan_mode,
  notebook_edit, web_fetch

Recall kit (8):
  recall_index, recall_timeline, recall_fetch,
  trace_causes, check_dead_ends,
  recall_facts_as_of, recall_change_history, recall_decision_points

Klyntbot lightweight (4):
  tasks, notes, memory, mirror

MCP gateway (varies by user config):
  mcp_<server>_<tool>
```

The 11 klyntbot tools NOT in the default (`project`, `area`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `temporal`) live in the deferred pool.

### Configuration shape

```json
{
  "codingCli": {
    "tools": {
      "defaultProfile": "curated",
      "profiles": {
        "minimal": { "include": ["bash", "read", "edit", "ask_user"] },
        "curated": {
          "include": ["@coding-kit", "@recall-kit", "tasks", "notes", "memory", "mirror"],
          "deferred": ["@klyntbot-extra", "@mcp-tools-over-threshold"]
        },
        "power": {
          "include": ["@all"],
          "deferred": []
        }
      },
      "deferredThreshold": 50,
      "alwaysEager": ["recall_index", "recall_timeline"]
    }
  }
}
```

`@coding-kit`, `@recall-kit`, `@klyntbot-extra`, `@mcp-tools-over-threshold`, `@all` are aliases resolved by the loader.

### `--power` and `/power on|off`

CLI flag changes the active profile. Slash command toggles mid-session. Tool list rebuilt; system prompt regenerated for next iteration. Emits `PowerModeToggled` event.

### Deferred-tool discovery (Phase 2)

When deferred list is non-empty, a `tool_search` tool is registered eagerly:

```rust
async fn tool_search(query: &str, max_results: usize) -> Vec<ToolSchema> {
    // semantic/fuzzy match against deferred pool
    // returns full schemas of top matches
}
```

Phase 2+ enhancement: results reranked by Mirror's per-skill effectiveness scores.

### Tool concurrency model

`Tool::is_concurrency_safe(args) -> bool` (added to `tools-core::Tool` per §4). Default returns `false`. Klynt-cli's read tools override to `true`. Execute loop's tool orchestration partitions accordingly:

```rust
let (safe, unsafe_) = tool_calls.partition(|t| {
    registry.get(&t.name).map_or(false, |tool| tool.is_concurrency_safe(&t.args))
});
let safe_results = futures::future::join_all(safe.into_iter().map(|tc| run_tool(tc))).await;
for tc in unsafe_ {
    let r = run_tool(tc).await;  // sequential
    results.push(r);
}
```

### Tool result handling

Existing 50KB cap at `crates/agent/src/execution/core.rs:31` inherited unchanged. Phase 3+ adds Claude Code's content-replacement pattern for oversized results we want to preserve in full.

### MCP integration

Klynt-cli uses klyntbot's existing `mcp::ToolRegistryBridge`. The bridge's `default_exposed_tools()` controls *external* MCP-server exposure; klynt-cli's `codingCli.tools.profiles.curated.include` is *internal* curation. Two separate lists, two audiences.

Phase 3+ candidate: `mcp.servers[].allowedChannels` for per-channel server gating.

### Tool naming hygiene

- Coding kit: short, lowercase verbs (`bash`, `read`, `edit`)
- Recall kit: prefixed (`recall_*`, `trace_*`, `check_*`)
- Klyntbot domain: kept as-is (no collisions with coding verbs)
- MCP gateway: prefixed `mcp_<server>_<tool>`

Conflict detection runs at registry build time; duplicate names abort boot.

---

## 6. Approval + sandbox model (3-layer)

### Decision flow

```
Tool call arrives
       │
       ▼
┌──────────────────────────────────────┐
│  PRIVACY GUARD (always first,        │  spec excludePaths — block
│  never bypassable)                    │  reads/writes of .env, secrets,
│                                       │  *.key, etc. Even --yolo cannot
└─────────────┬────────────────────────┘  override.
              ▼
┌──────────────────────────────────────┐
│  Layer 1 — DECLARATIVE RULES          │  allow / deny / ask globs in
│  config.json                          │  config.json. Match → done.
└─────────────┬────────────────────────┘
              ▼
┌──────────────────────────────────────┐
│  Layer 2 — STARLARK EXECPOLICY        │  Power-user rules in
│  ~/.klyntbot/rules/*.rules            │  ~/.klyntbot/rules/. Conditional
│                                       │  logic; prefix_rule, custom_rule.
└─────────────┬────────────────────────┘
              ▼
┌──────────────────────────────────────┐
│  Layer 3 — MIRROR-LEARNED             │  (Phase 2+ opt-in)
│  approval history per (tool, args     │  Frequently-approved → auto.
│  hash, repo)                          │  Any past denial → always-ask.
└─────────────┬────────────────────────┘
              ▼
┌──────────────────────────────────────┐
│  MODE DEFAULT                         │
│  plan | default | yolo | print        │
└──────────────────────────────────────┘
```

Each gate emits `ApprovalEvaluated` event capturing layer + reason.

### Approval modes

| Mode | When | Reads | Writes | Exec | Special |
|---|---|---|---|---|---|
| `default` | normal interactive | per layers | per layers | per layers | layers run as designed |
| `plan` | `--plan` or `/plan` | allowed | denied | denied | research-only; agent told via system prompt |
| `bypass` | `--yolo` or `/yolo` | allowed | allowed | allowed | requires `KLYNTBOT_ENABLE_YOLO=1` env |
| `print` | `klynt --print "..."` | allowed | denied | denied | defaults to `plan` mode |
| `print --yolo` | headless full-auto | allowed | allowed | allowed | for CI use |

Mode displayed prominently in TUI status line; can change via slash commands.

### Layer 1 — Declarative rules

```json
{
  "codingCli": {
    "permissions": {
      "allow": [
        "Read(*)", "Glob(*)", "Grep(*)",
        "Bash(git status*)", "Bash(git diff*)", "Bash(git log*)",
        "Bash(cargo build*)", "Bash(cargo nextest*)", "Bash(npm test*)", "Bash(bun *)",
        "Edit(./**)", "Write(./**)", "ApplyPatch(./**)",
        "RecallIndex(*)", "RecallTimeline(*)", "RecallFetch(*)", "TraceCauses(*)",
        "Tasks(*)", "Notes(*)", "Memory(*)", "Mirror(get*)"
      ],
      "deny": [
        "Bash(rm -rf /*)", "Bash(rm -rf ~*)", "Bash(sudo *)",
        "Bash(curl * | sh*)", "Bash(wget * | sh*)",
        "Write(/etc/**)", "Write(/usr/**)", "Write(~/.ssh/**)",
        "Edit(/etc/**)"
      ],
      "ask": [
        "Bash(*)", "WebFetch(*)", "Mcp(*)", "Edit(~/**)", "Write(~/**)"
      ],
      "defaultIfNoMatch": "ask"
    }
  }
}
```

Matcher syntax: `Tool(glob)`. `globset` semantics. For Bash, glob runs against the full command-line string. For file tools, against the resolved absolute path.

### Layer 2 — Starlark execpolicy

`klynt-execpolicy` (adapted from `codex-execpolicy`) loads `~/.klyntbot/rules/*.rules`:

```python
prefix_rule(["git", "status"], decision="allow")
prefix_rule(["git", "push"], decision="ask")

def check_git_push(args):
    if "main" in args or "master" in args:
        return forbid("never auto-push to main/master")
    return ask()

custom_rule(["git", "push"], handler=check_git_push)

prefix_rule(["cargo", "nextest"], decision="allow")
prefix_rule(["cargo", "fmt"], decision="allow")
prefix_rule(["cargo", "clippy"], decision="allow")
prefix_rule(["cargo", "build"], decision="allow")
```

In-session "always allow for this session" appends in-memory rules via `append_session_allow_prefix(...)`.

### Layer 3 — Mirror-learned approval (opt-in, Phase 2+)

Off by default. Enable via `codingCli.permissions.mirrorLearning: true`. After Layers 1 and 2 fall through:

```rust
let key = (tool_name, args_hash_for_relevance, repo_id);
let history = mirror.approval_history(&key);

if history.approval_count >= 5 && history.denial_count == 0 {
    return ApprovalDecision::Auto(reason: "mirror: 5+ prior approvals, no denials");
}
if history.denial_count >= 1 {
    return ApprovalDecision::Ask(reason: "mirror: prior denial — always confirm");
}
ApprovalDecision::FallThrough
```

`args_hash_for_relevance` ignores volatile fields (timestamps, IDs). Cool-down per-repo: after the 5th approval, wait 24h before activating auto-approve. Single denial poisons cache for that key (until explicit clear).

### Privacy guard

Coding-memory spec's `excludePaths` (`.env`, `secrets/**`, `*.key`, etc.) evaluated **before any layer**. Cannot be disabled by `--yolo`. Can be widened/narrowed via `<repo>/.klyntbot/ignore.toml` (per spec).

### Sandbox enforcement

**macOS (Seatbelt):** Each `bash`/`edit`/`write` runs via `sandbox-exec` with generated `.sbpl` policy. Default policy denies all, then allows process-fork, signal-self, file-read of cwd ancestors + common system paths, file-write only to cwd, network on (deny via permission rules).

**Linux (Landlock + bwrap):** `klynt-sandbox-helper` exec'd as a child; applies Landlock filesystem restrictions in-process; bwrap provides namespace sandbox.

**Windows:** Phase 3+. Initial release macOS + Linux only.

### Sandbox failure handling

If sandbox unavailable: `klynt-cli` does **not** silently run unsandboxed. Detects missing capability at startup, shows TUI banner, tightens approval gate (every bash/exec/write defaults to `ask`), emits `SandboxPolicyApplied { fallback_unsandboxed: true }`. User can opt to run unsandboxed without tightening via `KLYNTBOT_ALLOW_UNSANDBOXED=1`.

### Hook interaction

`PreToolUse` hooks run **after** privacy guard + Layers 1-3 but **before** sandbox launch. Block return aborts the call. Hooks can add restrictions; cannot override a deny.

### TUI presentation

```
┌─ Approval needed ──────────────────────────────────────────┐
│  Tool: bash                                                │
│  Args: cargo test --workspace                              │
│  CWD:  /Users/jayden/Projects/Klynt/bot                    │
│  Sandbox: Seatbelt (cwd-only file writes)                  │
│  Layer: layer-2/starlark — no matching rule                │
│                                                            │
│  Mirror history: 12 approvals, 0 denials in this repo      │
│  (Mirror-learning is OFF; enable to auto-approve patterns) │
│                                                            │
│  [a] Allow once   [s] Allow always   [d] Deny              │
│  [r] Add rule…    [m] Enable Mirror-learning               │
└────────────────────────────────────────────────────────────┘
```

`[s]` writes to Layer 1 (declarative) — appended to `allow` list.
`[r]` opens inline editor for a Starlark rule (Layer 2).

---

## 7. Skill system: single source, explicit install

### Direction

- **Discovery**: only `.klyntbot/skills/` (user-global + project-local) + Reforge-synthesized at `~/.klyntbot/project-skills/<repo-id>/`. We do **not** walk `~/.claude/skills/`, `~/.cursor/skills/`, `~/.codex/skills/`, or `~/.skills/` automatically.
- **External skills** enter klyntbot's world only via explicit `klynt skills install <source>` or manual copy/paste.
- **Skill upgrades** for user-installed skills via `klynt skills update`. For Reforge-synthesized skills, Reforge handles upgrades nightly.
- **Format** remains Anthropic Agent Skills spec — installs are mechanical copy operations; Reforge writes are portable to other tools.

### Discovery paths (final)

```rust
const STATIC_PATHS: &[(&str, SkillSource)] = &[
    ("~/.klyntbot/skills",                                SkillSource::User),
    (".klyntbot/skills",                                  SkillSource::Project),
    ("~/.klyntbot/project-skills/<sanitized-repo-id>",    SkillSource::ReforgePrivate),
    ("<repo_root>/.klyntbot/skills",                      SkillSource::ReforgeTeam),
];
```

Conflict resolution: `Project` > `User`. Reforge skills live in their own scoped namespace.

### SKILL.md frontmatter contract

Anthropic Agent Skills spec verbatim, with klyntbot-additive fields (`tags`, `sensitivity`, `references[].load`):

```yaml
---
name: "Add a new feature package crate"
description: "Scaffold a new crates/feature-* in the klyntbot workspace"
when_to_use: "User asks to add a new feature crate"
allowed-tools: [Bash, Edit, Write, Read, Tasks]
paths: ["crates/feature-*/**", "Cargo.toml"]
user-invocable: true
argument-hint: "<feature-name>"
arguments: [feature_name, layer]
model: "inherit"
effort: "medium"
context: "fork"
agent: "default"
hooks:
  Stop: []
version: "1.0"
disable-model-invocation: false
references:
  - { path: "schema.md", load: "always" }
  - { path: "examples.md", load: "on-demand" }
tags: ["coding", "scaffolding"]
sensitivity: "normal"
---

# Skill body — markdown.
```

### Conditional activation by `paths:`

Skills with `paths: [glob, ...]` activate only when matching files are touched. Implementation:

1. Boot: partition discovered skills into `unconditional_skills` and `conditional_skills` (keyed by glob).
2. Broker subscribes a `SkillActivator` to events. For every `FileEdit { path, .. }` and tool call referencing a path:
   - Activator runs path through every conditional skill's glob set.
   - Matched skills move to `dynamically_active_skills` (deduped).
   - `SkillActivated { skill_id, source_path, trigger: PathTouch }` fires.
3. Activated skills inject their frontmatter summary into the system prompt next turn.
4. Activation is session-scoped.

### Dynamic discovery on file touch

When a tool reads/edits a file deep in a repo, walk from its directory up to CWD checking each level for `.klyntbot/skills/` directories not in the index. Newly-found directories loaded on the spot. Gitignored directories skipped.

### Progressive loading (klyntbot's existing pattern, preserved)

- **Frontmatter-only at discovery time** — `name`, `description`, `when_to_use`, `paths`, `tags`, `allowed-tools` parsed; full body skipped.
- **Activation-time injection** — frontmatter flows into system prompt; body loads via `skill_reference(skill_id, ref_name)` tool.

### Skill management commands

```
klynt skills list                         # list all installed skills with source + version
klynt skills info <name>                  # show frontmatter, source, last-activated, references
klynt skills install <source>             # add a new skill (sources below)
klynt skills update <name>                # re-fetch from origin if source supports it
klynt skills uninstall <name>             # remove from ~/.klyntbot/skills/
klynt skills validate <name>              # check SKILL.md syntax + allowed-tools
klynt skills toggle <name> --on|--off     # enable/disable without uninstalling
klynt skills reload                       # re-walk discovery without restart
```

### `klynt skills install` source types

Phase 1:
- `klynt skills install ./local/path` — copies the directory
- `klynt skills install ~/.claude/skills/foo` — manual bridge from another ecosystem
- `klynt skills install https://github.com/user/repo[/path]` — clones; validates SKILL.md
- `klynt skills install https://gist.github.com/...` — same as github but for gists

Phase 3+:
- `klynt skills install skills-sh:<handle>` — registry resolution

Install command shows SKILL.md content + allowed-tools + (optional) install-script preview, asks for confirmation, then writes. Install-time confirmation; no per-activation prompt.

### Install metadata

`.klyntbot/skills/<name>/.install.json`:

```json
{
  "name": "code-review",
  "source": "https://github.com/example/skills",
  "source_type": "github",
  "installed_at": "2026-04-23T14:30:00Z",
  "installed_version": "abc1234",
  "last_updated": null,
  "managed_by": "klynt-cli",
  "auto_update": false
}
```

Reforge-synthesized skills get `managed_by: "reforge"`; `klynt skills` refuses to operate on them.

### Duplicate handling

1. **Project shadows User** (same name in both): Project wins; `/skills info <name>` shows both.
2. **Reforge collides with user**: User's Project or User skill wins; Reforge's preserved at scoped path.
3. **Install of duplicate name**: `install` refuses with clear message; `--force` overwrites with backup to `.skill-backups/`.

### Configuration shape

```json
{
  "codingCli": {
    "skills": {
      "enableConditionalActivation": true,
      "enableDynamicDiscovery": true,
      "maxActiveSkills": 30,
      "frontmatterTokenBudget": 2000,
      "alwaysActivate": ["code-review", "test-runner"],
      "neverActivate": ["legacy-foo"]
    }
  }
}
```

---

## 8. Event vocabulary: AgentEvent extensions

### Two enums, one event story

| Enum | Crate | Role |
|---|---|---|
| `agent::events::AgentEvent` | existing `agent` crate | Runtime streaming events for in-process consumers |
| `coding-ingest::AgentEvent` | in-flight coding-memory worktree | Cross-CLI normalized ingest events |

Klynt-cli extends both. A `MemorySinkSubscriber` translator maps runtime events into ingest events as they flow.

### New variants on `agent::events::AgentEvent`

All under `#[non_exhaustive]`; chat-channel match arms must have `_ =>` catch-all:

```rust
RecallInjected { memory_ids, coverage_score, escalation_chain, dead_end_warning, budget_used_tokens, budget_limit_tokens },
DeadEndWarningSurfaced { approach_summary, prior_attempt_id, confidence },
SkillActivationConsidered { skill_id, score, threshold, accepted, decision_reason },
SkillActivated { skill_id, source_path, trigger, injected_tokens },
SkillReferenceLoaded { skill_id, reference, tokens, load_kind },
ContextEngineDecision { included, excluded, total_tokens, budget_used_pct },
ApprovalEvaluated { tool, tool_args_hash, layer, rule_matched, mirror_history, decision, latency_ms },
SandboxPolicyApplied { tool, policy_summary, policy_hash, fallback_unsandboxed, fs_constraints, network_constraints },
ToolCallStreamChunk { tool, chunk_kind, bytes, truncated },
MCPSubcallTrace { server, tool, latency_ms, bytes_returned, error },
ProviderRequest { iteration, model, prompt_tokens, max_tokens, attempt },
ProviderResponse { latency_ms, usage, cost_usd, retries_used, finish_reason },
MidLoopCompressionTriggered { before_tokens, after_tokens, messages_condensed, regions },
MirrorAlertSurfaced { alert_id, severity, kind, action_taken },
CostThresholdCrossed { tier, accumulated_cost_usd, projected_cost_usd, ceiling_usd },
FileEditWithSymbols { path, op, bytes, diff_full, anchored_symbols, lsp_diagnostics_delta },
TestRunDetailed { command, framework, passed_tests, failed_tests, newly_passing, newly_failing, coverage_delta, duration_ms },
PowerModeToggled { previous, current, eager_tool_count, deferred_tool_count },
TurnInterrupted { reason, partial_tools, iterations_completed },
KlyntSessionStart { session_id, cwd, repo, active_profile, approval_mode, sandbox_status },
KlyntSessionEnd { session_id, reason, total_cost_usd, total_iterations, total_tool_calls },
```

### New variants on `coding-ingest::AgentEvent`

Coordinated extension to the coding-memory spec (see §14). 10 net-new variants; all additive:

```rust
SkillActivated { skill_id, source_path, trigger },
RecallInjected { memory_ids, coverage_score, dead_end_warning },
ApprovalDecision { tool, decision, layer },
SandboxApplied { tool, policy_summary, fallback_unsandboxed },
FileEditEnriched { path, op, anchored_symbols, lsp_diagnostics_delta },
TestRunEnriched { command, passed_tests, failed_tests, newly_failing },
ProviderCall { model, prompt_tokens, completion_tokens, cost_usd, latency_ms, retries },
CompressionApplied { before_tokens, after_tokens, messages_condensed },
MirrorAlert { alert_id, severity, kind },
SkillRoutingTrace { considered, chosen },
```

### Translator (runtime → ingest)

`klynt-core::MemorySinkSubscriber` consumes runtime `AgentEvent` from broker, emits `coding-ingest::AgentEvent` to MemorySink. Aggregation patterns:

- `ContentChunk` accumulates into per-iteration `AssistantMsg` ingest event
- `ToolStart` + `ToolEnd` pair into single `ToolCall` ingest event with timing
- `FileEditWithSymbols` → `FileEditEnriched`
- `RecallInjected` → `RecallInjected` (1:1)
- `ProviderResponse` → `ProviderCall`
- Some events are runtime-only (TUI consumes, no ingest equivalent): `IterationStart`, `ToolCallStreamChunk`, `PowerModeToggled`

### Property tests

| # | Invariant |
|---|---|
| E1 | Every `FileEditWithSymbols` runtime event produces exactly one `FileEditEnriched` ingest event |
| E2 | Every `ProviderResponse` produces exactly one `ProviderCall` with matching `cost_usd` and `latency_ms` |
| E3 | `ContentChunk` stream of N chunks aggregates to exactly one `AssistantMsg` ingest event with concatenated text |
| E4 | `ToolStart` + `ToolEnd` for same tool produces exactly one `ToolCall` ingest event; orphan `ToolStart` → no ingest emit |
| E5 | Translator monotone: ingest emit count grows monotonically; never retroactively cancels prior emits |

### Extensibility patterns

#### `#[non_exhaustive]` everywhere

Every event-related enum and struct gets `#[non_exhaustive]`. Forces external consumers to use catch-all match arms — adding variants becomes a non-breaking change.

#### Tuple variants wrapping struct types

```rust
#[non_exhaustive]
pub enum AgentEvent {
    ContentChunk(ContentChunkEvent),
    ProviderResponse(ProviderResponseEvent),
    // ... new variants additive
}

#[non_exhaustive]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderResponseEvent {
    pub latency_ms: u64,
    pub usage: providers::Usage,
    pub cost_usd: f64,
    pub retries_used: u32,
    pub finish_reason: FinishReason,
    pub extensions: EventExtensions,
}
```

Constructors use builder + `Default` for forward-compat.

#### `EventExtensions` — typed escape hatch

```rust
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EventExtensions {
    inner: BTreeMap<String, serde_json::Value>,
}

impl EventExtensions {
    pub fn set<T: serde::Serialize>(&mut self, key: &str, value: T) -> Result<()>;
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T>;
    pub fn keys(&self) -> impl Iterator<Item = &String>;
}
```

Pattern of use:
```rust
let mut response = ProviderResponseEvent::new(latency_ms, usage, cost_usd);
response.extensions.set("anthropic_cache_creation_tokens", cache_creation)?;
response.extensions.set("server_tier", "enterprise")?;
event_tx.send(AgentEvent::ProviderResponse(response)).await?;
```

Stable extensions promoted to typed fields in future PRs.

#### Versioned envelope

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "version", rename_all = "camelCase")]
pub enum VersionedAgentEvent {
    V1(AgentEventV1),
    // V2(AgentEventV2),    // future structural breaks
}
```

Used only for structural breaks; non-exhaustive + extensions absorb 95% of changes.

#### Translator pattern for consumer stability

Internal events evolve fast; external consumers (Distiller, Wire, rollout JSONL) all go through translators. Translators are the stability boundary.

### Documentation as code

`klynt events docs` — CLI subcommand that walks `agent::events::AgentEvent`, generates `docs/coding-cli/events.md` with per-variant reference. CI runs in `--check` mode; fails build if docs are stale.

`klynt events stats` — runtime broker telemetry showing per-variant emit count over the last session.

### Configuration shape (extensibility opt-ins)

```json
{
  "codingCli": {
    "events": {
      "captureExtensions": ["anthropic_cache_*", "lsp_*", "experimental_*"],
      "rateLimitVariants": {
        "ContentChunk": { "maxPerSecond": 200 },
        "ToolCallStreamChunk": { "maxPerSecond": 100 }
      },
      "warnOnUnknownVariants": true,
      "logBrokerStatsEverySeconds": 60
    }
  }
}
```

---

## 9. Wire protocol v0 (observer attach)

### Purpose

Wire exists for klyntbot-internal observer attach — not external IDE compatibility. Read-only observer attach, klyntbot-native event vocabulary, no protocol contortion to satisfy external standards. **In-process ingestion remains the primary data path**; Wire is the side door.

### Transport

JSON-RPC 2.0 over Unix domain socket. Per session:
- Socket: `~/.klyntbot/sessions/<session-id>/wire.sock` (mode 0600)
- Sidecar: `~/.klyntbot/sessions/<session-id>/wire.json` for observer discovery

`klynt list-sessions` walks `~/.klyntbot/sessions/*/wire.json` to show active sessions. Desktop's Workbench gets the same view.

### Frame format

Length-prefixed JSON: 4-byte LE length + JSON payload. JSON-RPC 2.0 envelopes.

### Phase 1 method set

**Client → Agent:**

| Method | Purpose |
|---|---|
| `initialize` | Handshake: protocol version, observer ID, optional event-filter |
| `replay` | Re-stream events from session start (or cursor) |
| `query_session` | Snapshot of current session state |
| `disconnect` | Clean shutdown signal from observer |

**Agent → Client (notifications):**

| Method | Purpose |
|---|---|
| `event` | A single `agent::events::AgentEvent` |
| `session_terminated` | klynt-cli is shutting down |

No `prompt`, `steer`, `cancel`, or `tools/call` in Phase 1 — those are Phase 2+ additions.

### `initialize` handshake

```json
// observer → klynt
{
  "jsonrpc": "2.0", "id": 1, "method": "initialize",
  "params": {
    "protocol_version": "0.1",
    "observer_name": "klyntbot-desktop-workbench",
    "observer_id": "uuid-...",
    "event_filter": {
      "include_kinds": ["ContentChunk", "ToolStart", "ToolEnd", "RecallInjected", "ApprovalEvaluated"],
      "exclude_kinds": ["ToolCallStreamChunk"]
    },
    "buffer_replay_since": "iteration_3"
  }
}

// klynt → observer
{
  "jsonrpc": "2.0", "id": 1,
  "result": {
    "protocol_version": "0.1",
    "session_id": "uuid-...",
    "session_meta": {
      "klynt_version": "0.1.1",
      "model": "claude-sonnet-4-7",
      "cwd": "/Users/jayden/Projects/Klynt/bot",
      "repo": { "id": "github.com/klyntbot/bot", "branch": "main" },
      "profile": "curated",
      "approval_mode": "default",
      "started_at": "2026-04-23T14:30:00Z"
    },
    "capabilities": {
      "supports_event_filter": true,
      "supports_replay": true,
      "supports_state_query": true,
      "supports_bidirectional_control": false
    }
  }
}
```

### Event delivery

```json
{
  "jsonrpc": "2.0", "method": "event",
  "params": {
    "version": "v1",
    "session_id": "uuid-...",
    "sequence": 247,
    "occurred_at": "2026-04-23T14:32:18.456Z",
    "kind": "ProviderResponse",
    "payload": {
      "latency_ms": 1234,
      "usage": { "prompt_tokens": 4500, "completion_tokens": 280 },
      "cost_usd": 0.0042,
      "retries_used": 0,
      "finish_reason": "Stop",
      "extensions": { "anthropic_cache_read_tokens": 1200 }
    }
  }
}
```

`sequence` lets observers detect missed events (gap → request `replay`).

### Multi-observer fan-out

`klynt-core::WireServer` subscribes once to broker, fans out to N observers concurrently. Per-observer `tokio::sync::mpsc::Sender` capacity 256. Slow observers receive `slow_observer_warning`; after three consecutive overflow events, force-disconnected with `dropped_due_to_lag`.

Slow observer cannot backpressure broker or stall agent loop.

### Replay + query_session

```json
// replay
{ "method": "replay", "params": { "since_sequence": 100 } }
// → klynt streams `event` notifications, then `replay_complete`

// query_session
{ "method": "query_session" }
// → returns current snapshot:
{
  "result": {
    "current_iteration": 5,
    "active_skills": ["code-review", "test-runner"],
    "active_profile": "curated",
    "approval_mode": "default",
    "cost_so_far_usd": 0.034,
    "tokens_used": 12450,
    "pending_approval": null,
    "cwd": "...",
    "last_event_sequence": 247
  }
}
```

### Security

Filesystem-permission-based: socket mode 0600; directory mode 0700; same-user only. No tokens, no TLS in Phase 1. Same threat model as `~/.ssh/agent.sock`.

Connections logged with observer_name + observer_id; viewable via `klynt status --wire-observers`.

### Versioning

`protocol_version` is SemVer minor.major. Klynt-cli accepts requests for any version ≤ its current. Major bumps mean structural breaks; klynt-cli responds with `error: { code: "version_unsupported" }`.

### Configuration

```json
{
  "codingCli": {
    "wire": {
      "enabled": true,
      "socketDirectory": "~/.klyntbot/sessions",
      "perObserverBufferCapacity": 256,
      "replayRingBufferSize": 1024,
      "maxObservers": 8,
      "slowObserverWarningThreshold": 3,
      "logObserverConnections": true,
      "defaultEventFilter": {
        "excludeKinds": ["ToolCallStreamChunk"]
      }
    }
  }
}
```

`enabled: false` skips socket creation entirely.

### Deferred (Phase 2+)

- Bidirectional control (`prompt`, `steer`, `cancel`)
- Tool registration from observers (`external_tools`)
- ACP v0.8.0 (Phase 3+ if IDE plugin appears)
- Cross-machine TCP transport with TLS
- OAuth/token-based auth
- Wire protocol schema validation library

---

## 10. Session rollout + resume

### Storage layout (per session)

```
~/.klyntbot/sessions/<session-id>/
├── meta.json                # session metadata
├── rollout.jsonl            # append-only event log
├── wire.sock                # observer socket (cleaned on exit)
├── wire.json                # discovery sidecar
└── snapshots/               # Phase 2+ file snapshots
    ├── 0001-pre-edit.json
    └── 0002-post-edit.json
```

### `meta.json`

```json
{
  "session_id": "01HXYZ-a1b2c3-...",
  "klynt_version": "0.1.1",
  "started_at": "2026-04-23T14:30:00Z",
  "ended_at": "2026-04-23T15:45:00Z",
  "ended_reason": "user_quit",
  "model": "claude-sonnet-4-7",
  "model_provider": "anthropic",
  "cwd": "/Users/jayden/Projects/Klynt/bot",
  "repo": {
    "id": "github.com/klyntbot/bot",
    "branch": "main",
    "commit": "0ada62b8a"
  },
  "profile": "curated",
  "approval_mode": "default",
  "active_skills_at_start": ["code-review"],
  "user_handle": "jayden"
}
```

### `rollout.jsonl` line types

```jsonc
// session_meta header
{ "type": "session_meta", "version": "v1", "payload": { ...meta.json... } }

// event lines
{ "type": "event", "version": "v1", "sequence": 1, "occurred_at": "...", "kind": "PipelineStarted", "payload": {} }

// compaction boundary
{ "type": "compact_boundary", "version": "v1", "sequence": 89, "before_tokens": 95000, "after_tokens": 38000, "messages_condensed": 47 }

// snapshot reference (Phase 2+)
{ "type": "snapshot_ref", "version": "v1", "sequence": 102, "snapshot_id": "0001-pre-edit", "files": ["src/foo.rs"] }

// user message marker
{ "type": "user_message", "version": "v1", "sequence": 110, "uuid": "msg-...", "text": "now refactor the parser", "parent_uuid": null }
```

### Writer task

Dedicated `tokio::spawn` task owns the file handle. Receives `RolloutLine` via `mpsc::channel(1024)`; sends are non-blocking on broker side.

```rust
pub struct RolloutRecorder {
    tx: mpsc::Sender<RolloutLine>,
}
```

Hot flushes after every event; durability over throughput.

### State DB integration

Reuses klyntbot's `~/.klyntbot/data.db` (no separate `state.db`). New table:

```sql
CREATE TABLE klynt_sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    ended_reason TEXT,
    cwd TEXT NOT NULL,
    repo_id TEXT,
    repo_branch TEXT,
    repo_commit TEXT,
    profile TEXT NOT NULL,
    approval_mode TEXT NOT NULL,
    model TEXT NOT NULL,
    model_provider TEXT NOT NULL,
    klynt_version TEXT NOT NULL,
    last_user_message_uuid TEXT,
    rollout_path TEXT NOT NULL,
    total_events INTEGER NOT NULL DEFAULT 0,
    total_cost_usd REAL NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    parent_session_id TEXT,
    starred BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_klynt_sessions_started_at ON klynt_sessions(started_at DESC);
CREATE INDEX idx_klynt_sessions_repo ON klynt_sessions(repo_id, started_at DESC);
```

WAL mode handles multi-process writes from klynt-cli + desktop. Updates at session start, on user messages, on session end. No per-event SQL writes.

### Resume mechanics

```
klynt list-sessions [--repo <id>] [--limit 20]
klynt resume                           # most recent
klynt resume <session-id-prefix>       # match by prefix
klynt resume --pick                    # interactive picker
klynt resume --inplace <id>            # rare; append to existing
```

`klynt resume <id>` flow:
1. Query `klynt_sessions` → get `rollout_path` + metadata
2. Open `rollout.jsonl`; replay via `RolloutReader::replay()`
3. Reconstruct conversation: walk events, accumulate `ContentChunk` into per-iteration assistant messages, honor `compact_boundary`
4. Render reconstructed conversation in TUI
5. Allocate new session-id (resumes always fork; `parent_session_id` points at original)
6. Restart broker + observer socket on new session-id
7. Wait for new user input

Resume does **not** re-run the agent loop on prior events — replays visible state only.

### Rewind (Phase 2+)

`snapshots/` directory holds file-content snapshots taken before any `Edit`/`Write`/`ApplyPatch`:

```rust
let snapshot_id = format!("{:04}-pre-edit", next_snapshot_num);
let snapshot = FileSnapshot { files, created_at, triggering_event_seq };
std::fs::write(session_dir.join("snapshots").join(format!("{}.json", snapshot_id)), serde_json::to_string(&snapshot)?)?;
broker.send(RolloutLine::SnapshotRef { snapshot_id, files, sequence });
```

Phase 2+ adds:
```
klynt rewind <session-id> --to <user-message-uuid>
```

Phase 1 captures snapshots; only exposes via Workbench panel (read-only). Active rewind ships in Phase 2+.

### Compaction interaction

When `MidLoopCompressor` runs, rollout subscriber captures both the original event (audit) and a synthetic `compact_boundary` line. On resume, replay logic honors the boundary: loads compacted summary instead of replaying every pre-boundary event.

### Retention policy

```json
{
  "codingCli": {
    "rollout": {
      "retentionDays": 90,
      "maxTotalDiskMb": 5000,
      "compactRolloutAfterDays": 30,
      "preserveStarred": true
    }
  }
}
```

Daily cleanup runs in desktop's nightly cron (or klynt-cli startup if desktop not running). Eviction: oldest non-starred first.

### CLI surface

```
klynt list-sessions [--repo <id>] [--limit N] [--starred] [--since <date>]
klynt resume                                 # most recent
klynt resume <id-prefix>
klynt resume --pick                          # interactive
klynt resume --inplace <id>                  # rare; append
klynt sessions star <id>
klynt sessions unstar <id>
klynt sessions delete <id>                   # explicit; prompts
klynt sessions export <id> [--format md|html|json]
klynt sessions clean [--dry-run]             # apply retention
```

### Multi-process safety

- WAL mode on `data.db` already handles concurrent klynt-cli + desktop writes
- `rollout.jsonl` owned by one klynt-cli process at a time; desktop reads but never writes
- `meta.json` written once at session start; post-mortem updated on graceful exit only
- `wire.sock` per-process; never shared

---

## 11. Distribution + boot model

### Distribution channels

| Channel | Audience | Phase |
|---|---|---|
| Bundled with klyntbot desktop installer | Primary path | 1 |
| `cargo install --git ...` | Power users / contributors | 1 |
| Standalone binary downloads (GitHub releases) | Users who want CLI without desktop | 2 |
| `brew install klyntbot/tap/klynt` | macOS power users | 3+ |

### Bundled-with-desktop (primary path)

Tauri 2 desktop installer drops `klynt` binary to `/usr/local/bin/klynt` (macOS) or `~/.local/bin/klynt` (Linux). Signed with same Developer ID as desktop.

Installer responsibilities:
1. Place `klynt` binary in PATH
2. Place `klynt-linux-sandbox` helper alongside (Linux)
3. Create `~/.klyntbot/` directory tree if absent
4. Seed default `config.json`
5. Seed default `~/.klyntbot/rules/` Starlark policy
6. Optionally drop `~/.klyntbot/skills/` examples
7. Register shell completions (bash, zsh, fish)
8. Print welcome message

Detects existing klyntbot installs and offers to migrate.

### `cargo install` path

```bash
cargo install --git https://github.com/klyntbot/bot --bin klynt
```

For development:
```bash
cd /Users/jayden/Projects/Klynt/bot
cargo build -p klynt --release
target/release/klynt
```

### Cold boot vs warm boot

**Cold boot — desktop NOT running:**
```
1. Process starts (~2ms)
2. Read config.json (~5ms)
3. Open data.db (SQLite WAL — 30-50ms first time)
4. Open lance/ (LanceDB — 100-200ms first time on cold cache)
5. Construct AgentRuntime + load skill catalog (~50ms)
6. Walk .klyntbot/skills/ (~10-30ms)
7. Start klynt-tui event loop (~5ms)
Total: ~200-300ms cold start
```

**Warm boot — desktop running:**
```
1. Process starts (~2ms)
2. Read config.json (~5ms — OS-cached)
3. Open data.db (already initialized — ~5ms)
4. Open lance/ (already mmap'd by desktop — ~10ms)
5. Construct AgentRuntime — skip Distiller setup — ~30ms
6. Connect to ingest.sock — ~3ms
7. Skill catalog from shared cache — ~10ms
Total: ~70-100ms warm start
```

### First-run setup

Interactive prompts when no `~/.klyntbot/config.json`:
- Provider choice (Anthropic / OpenAI / Local / OpenRouter / configure later)
- API key
- Default approval mode
- Sandbox enforcement (on/off)
- Coding memory integration (on/off)

If desktop is installed (bundled path), prompt is skipped — inherits desktop's config.

### Versioning + updates

| Channel | Cadence | Distribution |
|---|---|---|
| Stable | Every 4-6 weeks | Bundled with desktop release; brew upgrade |
| Nightly | Daily from main | `cargo install --git` or `klynt update --nightly` (Phase 2) |

`klynt --version` prints version, build date, git commit, klyntbot version, enabled features.

#### Auto-update

- Phase 1: no auto-update
- Phase 2: opt-in `klynt update`; signature verified; never runs during a session
- Phase 3: in-session update notifications

### Pre-flight checks (`klynt doctor`)

Comprehensive diagnostic output covering:
- System (OS, arch, sandbox availability)
- Configuration (config.json, data.db schema, lance, rules)
- Provider (API key + test request)
- klyntbot integration (desktop status, ingest socket)
- Skills (count, broken frontmatter)
- Sessions (count, retention health, orphan socks)
- MCP servers (connection status, tool count)
- Recommendations

Exits non-zero if any check fails. Useful in CI and as first-line diagnostic.

### `klynt status` (live state)

```
klynt CLI 0.1.1
─────────────────────────────────────────────────
Active sessions          : 2
Desktop                 : ✓ running
Ingest buffer           : 0 events queued
Distiller backlog       : 0 turns pending

Today
  Total turns           : 23
  Total cost (USD)      : $0.34
  Top model             : claude-sonnet-4-7
  Top repo              : github.com/klyntbot/bot

Skills
  Active in this dir    : 4
  Recently activated    : 7 in last hour

Mirror alerts (pending) : 1 (severity: medium)
```

### Boot configuration

```json
{
  "codingCli": {
    "boot": {
      "warmCacheTtlSeconds": 60,
      "skipFirstRunWizard": false,
      "preflightOnLaunch": false,
      "lazyLanceLoad": true,
      "deferSkillScan": false
    }
  }
}
```

### Migration story

| Scenario | Behavior |
|---|---|
| Desktop installed first, then CLI | CLI inherits desktop's config + cognitive store |
| CLI installed first, then desktop | Desktop reads CLI's existing config + DB |
| Conflicting versions | Loud warning; major mismatch refuses to start |

### Binary size

Target: < 30 MB stripped binary (release build, LTO, strip-symbols).

### Configuration shape (versioning)

```json
{
  "codingCli": {
    "version": {
      "channel": "stable",
      "autoUpdate": "off",
      "checkIntervalHours": 24
    }
  }
}
```

---

## 12. Phased buildout

### Guiding principles (carried from coding-memory spec §11)

| Principle | Rationale |
|---|---|
| Phases ≠ versions | Phases are dev milestones; user-visible versions stamp final states |
| Architecture skeleton first | Phase 1 lands every crate, trait, type, event variant |
| Production-quality every phase | Zero clippy warnings; full doc comments; property tests for invariants |
| Schema consolidated into Phase 1 | Pre-release authorizes direct schema changes |
| Provenance for every rollout entry | Every event in `rollout.jsonl` carries provenance metadata |
| Single source of truth for types | `agent::events::AgentEvent`, `coding-ingest::AgentEvent`, etc. defined once |
| Coordinated with coding-memory spec | Cross-spec contract changes go in one PR |

### Phase 1 — Walking skeleton (target: 3-4 weeks)

**Deliverables:**

- 10 new crates land in `bot/crates/`
- `scripts/adapt_codex_vendor.sh` adapts 6 Codex crates with full rename pass
- All `agent::events::AgentEvent` extensions added under `#[non_exhaustive]`; chat-channel match arms audited
- `Tool::is_concurrency_safe()` added to `tools-core`
- `crates/agent/src/execution/core.rs` extended with read-only-aware partitioning
- Coding tool kit: `bash`, `read`, `glob`, `grep`, `edit`, `write`, `apply_patch`, `ask_user`, `enter_plan_mode`/`exit_plan_mode`, `notebook_edit`, `web_fetch`
- Curated default tool profile (24 eager tools); `tool_search` registered as no-op stub
- Three-layer approval architecture present (Layer 3 deferred to Phase 2)
- macOS Seatbelt sandbox live; Linux Landlock + bwrap live
- klyntbot's `skill-system` extended with `paths:` conditional + dynamic discovery
- Skill management commands (local + github sources)
- `MemorySinkSubscriber` translator with 5 property tests
- `klynt-rollout` writer task; per-session directory layout; `klynt_sessions` table migration
- `klynt resume` (always forks); `klynt list-sessions`
- Wire v0 (observer-only, 6 methods)
- Distribution: bundled with desktop installer; `cargo install --git` works
- `klynt doctor` and `klynt status`
- First-run wizard
- Boot perf targets met

**Stubs / deferred:**
- Mirror-learned approval (Layer 3): config flag accepted; layer skipped
- File snapshots: captured to disk but no `klynt rewind`
- `klynt update`: prints "use desktop installer or cargo install"
- `tool_search`: no-op stub
- `recall_*` tools: depend on coding-memory worktree state

**Exit gates:**
- `cargo build --workspace` clean
- `cargo clippy --workspace --all-targets --all-features` zero warnings
- `cargo fmt --all --check` passes
- `cargo nextest run --workspace` green
- All 5 translator property tests green
- `klynt doctor` exits zero on fresh install
- Cold boot < 350ms p95, warm boot < 120ms p95 (relaxed Phase 1 targets)
- Two end-to-end scenario tests pass

### Phase 2 — Polish + opt-in features (target: 2-3 weeks)

- Mirror-learned approval (Layer 3) lit up; opt-in via `mirrorLearning: true`
- File snapshots → rewind
- `klynt update` opt-in self-update
- `tool_search` becomes real
- `klynt events stats` runtime tool
- `klynt sessions star/unstar/export/delete/clean` commands
- Slash commands: `/skills`, `/mirror`, `/workbench`, `/power`, `/dead-ends`, `/recall <query>`
- Wire v1: bidirectional control
- Workbench panel: klynt sessions list
- Standalone binary downloads
- `klynt skills update` (re-fetch from GitHub origin)
- Performance pass: cold < 250ms p95, warm < 100ms p95
- `VersionedAgentEvent::V1` envelope (only if needed)

### Phase 3+ — Ecosystem & advanced

- MCP-contributed skills
- ACP v0.8.0 server
- Skills.sh marketplace integration
- Per-channel MCP allowlists
- `brew tap klyntbot/tap`
- `klynt skills install` with auto-update from local paths
- Wire v2: external_tools registration, hooks-as-wire-subscriptions
- Per-extension capture rules in rollout config
- Snapshots: content-addressed dedup
- Canonical-form hash matching for skill duplicates
- In-session update notifications
- In-session live skill reload
- `klynt events docs --check` CI mode

### Phase 4+ — Speculative (not committed)

- Cross-machine TCP transport with TLS
- OAuth/token-based auth for Wire
- Klyntbot's own marketplace registry
- Voice mode integration
- First-party VS Code/Cursor extensions
- Windows sandbox

### Coordination with coding-memory spec

#### Order A (recommended): Coding-memory first, then klynt-cli

```
coding-memory Phase 1 (architecture skeleton)
  → coding-memory Phase 2 (Claude Code adapter + ingestion transport)
    → coding-memory Phase 3 (Distiller writes)
      → coding-memory Phase 4 (Recall API)
        → coding-memory Phase 5 (Reforge + Mirror)
          → klynt-cli Phase 1 (full power from day 1; recall_* tools work)
            → klynt-cli Phase 2 (Mirror-learned approval lights up)
              → klynt-cli Phase 3+
```

Rationale: klynt-cli's first release is fully working rather than partially stubbed.

#### Order B alternative: Klynt-cli first, coding-memory drains later

Workable but ships a degraded klynt-cli first release.

### Quality gates (every phase)

| Gate | Check |
|---|---|
| Compilation | `cargo build --workspace` |
| Lint | `cargo clippy --workspace --all-targets --all-features` zero warnings |
| Format | `cargo fmt --all --check` |
| Tests | `cargo nextest run --workspace` |
| Doc coverage | `cargo rustdoc -- -D missing-docs` on new public items |
| Translator invariants (E1-E5) | proptest |
| Boot perf | `klynt doctor --bench` shows targets met |
| Workbench panel live | Every phase shipping new data shape ships matching panel |

### Per-phase test deliverables

| Phase | Tests | Rough count |
|---|---|---|
| 1 | Adapter unit, hook→broker, sandbox, approval, Wire observer, rollout replay, skill install + activation | ~80 unit + 12 integration + 5 property + 2 scenario |
| 2 | Mirror-learned, rewind, Wire bidirectional, skill update, deferred-tool discovery | ~40 unit + 8 integration + 2 property + 2 scenario |
| 3+ | MCP-contributed skills, ACP, marketplace | ~30+ per chunk |

Phase 1+2 target: ~160 tests.

### "Done" definition for klynt-cli v0.1 (after Phase 2)

- 160+ tests green
- All translator property tests proved
- Boot perf targets met (cold < 250ms p95, warm < 100ms p95)
- Zero clippy warnings
- Full documentation
- User scenario: install desktop → open terminal → `klynt` → multi-hour coding session with sandboxed shell, recall surfacing prior work, Mirror-learned approval, project skills activating, Workbench showing real-time state, next-day resume continuing seamlessly

### Risk register

| Risk | Mitigation |
|---|---|
| Codex `tui` adapt is messier than expected | Phase 1 spike: 1-day investigation of `AppServerClient` trait; fall back to fresh ratatui TUI if too painful |
| Adding `Tool::is_concurrency_safe` breaks existing tests | Default impl returns `false` (safe); existing tools unaffected |
| Sandbox enforcement fails on user's OS | `klynt doctor` catches; tightened-approval fallback automatic; user sees banner |
| Coding-memory worktree lags klynt-cli | Order A explicitly puts coding-memory first |
| Schema migration conflicts | Single workspace, single migration story per CLAUDE.md |
| Wire v0 observers leak / DoS broker | Per-observer queue caps + slow-observer disconnect; property test asserts broker latency independent of observer count |
| Boot perf regresses with skill catalog growth | `lazyLanceLoad` + `deferSkillScan` flags; CI boot benchmark fails on regression |

---

## 13. Testing, invariants, benchmarks

### Philosophy

```
              ┌─────────────────────┐
              │   Benchmarks (5)    │  perf regressions
              ├─────────────────────┤
              │  Scenarios (~10)     │  end-to-end stories
              ├─────────────────────┤
              │  Property (~10)      │  invariants over generated inputs
              ├─────────────────────┤
              │  Integration (~30)   │  cross-crate, real I/O
              ├─────────────────────┤
              │  Unit (~150+)        │  per-module, in-memory
              └─────────────────────┘
```

In-memory SQLite for everything below scenarios. Real filesystems via `tempfile::TempDir`. Provider responses mocked via fixtures.

### The 12 architectural invariants (proptests)

| # | Invariant | Phase |
|---|---|---|
| K1 | Translator round-trip determinism | 1 |
| K2 | Translator monotonicity (ingest count never retroactively decreases) | 1 |
| K3 | Rollout replay fidelity | 1 |
| K4 | Sequence monotonicity per session | 1 |
| K5 | Approval gate composition (single decision; highest-priority match wins) | 1 |
| K6 | Privacy guard inviolability (--yolo cannot bypass excludePaths) | 1 |
| K7 | Sandbox-fallback safety | 1 |
| K8 | Skill discovery determinism | 1 |
| K9 | Boot warm vs cold cognitive store equivalence | 1 |
| K10 | Mirror-learned cache poisoning (single denial → always ask) | 2 |
| K11 | Rewind round-trip | 2 |
| K12 | Wire observer non-interference (latency independent of observer count) | 2 |

Plus the coding-memory spec's 9 invariants (carried from §3 of that spec). Total **21 invariants**.

### Per-phase test breakdown

#### Phase 1 (~99 tests)

**Unit (~80):** Per-tool tests; klynt-protocol serde; klynt-execpolicy Starlark; klynt-sandbox policies; klynt-hooks marshalling.

**Integration (~12):** Hook → broker → MemorySink round trip; sandbox launch on macOS + Linux; approval gate full-stack; Wire v0 observer attach; rollout writer + replay; skill install + activation; resume reconstructs prior conversation; multi-process SQLite; LanceDB cold-load timing; boot perf benchmark.

**Property (~5):** K1, K2, K3, K6, K8.

**Scenario (~2):** Full session in `bot/`; resume scenario.

#### Phase 2 (~50 tests added)

**Unit (~40):** Mirror-learned approval; rewind; tool_search; new slash commands; klynt update; standalone binary launch.

**Integration (~8):** Wire v1 bidirectional; rewind round-trip; auto-update verify + replace; standalone binary on fresh machine.

**Property (~2):** K10, K11.

**Scenario (~2):** Mirror-learned cycle; multi-day session.

### Benchmark targets

```
bench_klynt_cold_boot           p95 < 250ms (Phase 2)
bench_klynt_warm_boot           p95 < 100ms
bench_translator_throughput     > 50K events/sec sustained
bench_rollout_writer_throughput > 10K events/sec
bench_wire_fanout_8_observers   p95 < 200µs broker→all-observer
bench_sandbox_launch_macos      p95 < 50ms (cached policy hit)
bench_sandbox_launch_linux      p95 < 80ms (Landlock + bwrap warm)
bench_skill_discovery_50_skills < 30ms full walk + parse
bench_approval_gate_full_stack  < 1ms per call
```

Scenario benchmarks:
```
bench_typical_turn_4_tools      single turn with 4 tool calls; < 6s wall clock
bench_recall_injection_p95      recall_index 10 results; < 200ms
bench_session_replay_500_events replay 500-event rollout; < 300ms
```

### Fixtures

```
tests/fixtures/klynt_cli/
  synthetic_session_simple.jsonl
  synthetic_session_error_recovery.jsonl
  synthetic_session_long.jsonl
  provider_responses/
    claude_simple_text.json
    claude_with_tools.json
    claude_with_thinking.json
    openai_with_tools.json
    ollama_local.json
  sandboxed_commands.json
  sample_skills/
    basic-skill/SKILL.md
    conditional-skill/SKILL.md
    invalid-skill/SKILL.md
  sample_repos/
    rust_workspace.tar
    python_project.tar
  mock_mcp_servers/
    echo-server.sh
    slow-server.sh
```

### Test infrastructure

#### `klynt-test-harness` crate (in `tests/common/`)

```rust
pub fn ephemeral_storage() -> StoragePool;
pub fn mock_provider(scripted_responses: Vec<&str>) -> Arc<dyn Provider>;
pub fn capture_broker_events() -> (broadcast::Sender<AgentEvent>, EventCapture);
pub fn assert_invariant_K1<F>(input: Vec<AgentEvent>, translate: F) -> TestResult;
pub fn fake_repo(layout: &str) -> TempDir;
pub fn approval_decision_table(rules: &str, calls: &[(...)]) -> Vec<ApprovalDecision>;
```

#### Property test strategies

```rust
pub fn arb_agent_event() -> impl Strategy<Value = AgentEvent>;
pub fn arb_session_history(max_turns: usize) -> impl Strategy<Value = Vec<AgentEvent>>;
pub fn arb_approval_decision_sequence() -> impl Strategy<Value = Vec<(ToolCall, ApprovalAction)>>;
pub fn arb_filesystem_state(max_files: usize) -> impl Strategy<Value = TempDir>;
```

### Continuous integration

| Trigger | Runs |
|---|---|
| Every PR | compile + lint + fmt + unit + property (short fuzz) |
| Merge to main | + integration + scenario |
| Nightly | + benchmark deltas + property (long fuzz) |
| Phase-completion PR | + all phase tests + regression check |

CI matrix: macOS-latest + ubuntu-latest. Build time target: < 8 minutes for full PR suite.

### Test isolation

- Each test gets unique `TempDir` for `~/.klyntbot/` overlay
- `KLYNTBOT_HOME` env var honored (per CLAUDE.md)
- No test mutates user's actual `~/.klyntbot/`
- Network access denied via `KLYNTBOT_TEST_NO_NETWORK=1`
- Platform-gated tests: `#[cfg(target_os = "macos")]` for Seatbelt; `#[cfg(target_os = "linux")]` for Landlock
- LLM provider via `_scripted_echo` (existing klyntbot pattern)

### Negative-path testing

- Try to write `~/.env` with privacy guard active → asserts `Blocked` even with `--yolo`
- Try to install skill with malformed frontmatter → asserts install refuses
- Try to attach 100 Wire observers → asserts 9th overflows `maxObservers: 8`
- Try to resume session with missing rollout.jsonl → asserts clear error, not panic
- Try to start two klynt sessions writing to same SQLite → asserts WAL handles it
- Send malformed JSON-RPC frame to wire.sock → asserts connection dropped, process unaffected

### Coverage targets (informational)

| Crate | Target line coverage |
|---|---|
| `klynt-protocol` | 100% |
| `klynt-execpolicy` | 90% |
| `klynt-sandbox` | 70% |
| `klynt-hooks` | 85% |
| `klynt-rollout` | 90% |
| `klynt-tui` | 60% |
| `klynt-skill-loader` | 90% |
| `klynt-core` | 85% |
| `klynt` | 50% |

### Test naming conventions

- `test_` prefix for unit tests
- `integration_` prefix for integration tests
- `property_` prefix for proptests
- `scenario_` prefix for scenario tests
- `bench_` prefix for benchmarks

Filtering: `cargo nextest run -E 'test(integration_)'`.

---

## 14. Coordination with coding-memory spec

### Why this section exists

The coding-memory spec defined `klynt-cli` as a non-goal. This spec is the deferred follow-on. It lands changes the coding-memory spec didn't anticipate: extending `coding-ingest::AgentEvent`, surfacing klynt-rich variants to the Distiller, sharing `data.db` storage, evolving project-skill paths.

The coding-memory worktree hasn't been implemented yet — we can amend the coding-memory spec **before** it's coded, in one cross-file PR, so both specs ship a consistent contract.

### Spec amendments

#### Category A: Additive event vocabulary

Coding-memory spec amendments:
- §5 *AgentEvent (the core contract)* — add the 10 new variants from §8 of this spec
- §5 *CLI adapter mappings* table — add row for `klynt-cli` with adapter type "Native (in-process emit)"
- §3 *Key decisions* table — add: "klynt-cli is a first-class source emitting the rich variant set"
- §13.A *DomainEvent variants added* — no change

Purely additive. No rewrites.

#### Category B: Distiller behavior on rich variants

Coding-memory spec §6 gains "Rich-variant handling" subsection enumerating which klynt-rich variants go through Phase B (LLM) vs. extractive-only:

| Variant | Distiller handling |
|---|---|
| `RecallInjected` | Extractive — feeds Mirror's `PatternEffectivenessSubscriber` directly |
| `ApprovalDecision` | Extractive — captured for autotuner training |
| `SandboxApplied` | Extractive — captured but not surfaced to LLM |
| `FileEditEnriched` | Replaces existing `FileEdit` for klynt-cli sources; `anchored_symbols` flow into spec's C2 |
| `TestRunEnriched` | Replaces `TestRun` for klynt-cli; per-test failures improve `FailurePattern` extraction |
| `ProviderCall` | Extractive — captured for cost analysis |
| `CompressionApplied` | Extractive — captured for autotuner training |
| `MirrorAlert` | Extractive — surfaced to user via existing alert pipeline |
| `SkillRoutingTrace` | Extractive — feeds `RoutingMirrorSubscriber` directly |
| `SkillActivated` | Extractive — captured for skill-effectiveness telemetry |

#### Category C: Schema delta

Coding-memory spec §4 *Schema deltas* — add row:

| Target | Change | Owner |
|---|---|---|
| `klynt_sessions` | `CREATE TABLE` per §10 of klynt-cli spec | klynt-cli spec, consolidated into Phase 1 migration |

Single migration story per CLAUDE.md.

#### Category D: `IngestAdapter` for klynt-cli

Coding-memory spec §5 gains "Native source: klynt-cli" subsection:

> klynt-cli emits `AgentEvent` directly via in-process function calls to `Distiller::accept_event(...)` when desktop is not running, OR via `~/.klyntbot/ingest.sock` when desktop is alive. Choice governed by `MemorySink` trait (defined in this spec; implementation in `coding-memory` crate) with two impls: `InProcessSink` and `IngestSocketSink`. State transitions cause `MemorySink` to switch impls at next event boundary.

#### Category E: Per-skill scope amendments — already covered

The coding-memory spec §9 already adds `skill_versions.scope` + `scope_repo_id` columns and project-skill paths. **No spec amendment needed for E.**

#### Category F: Configuration namespace coordination

Both specs add config keys. Coding-memory under `codingMemory.*`; klynt-cli under `codingCli.*`. No collisions.

Klynt-cli consumes some coding-memory config:
- `codingMemory.recall.sessionStartBudget` — used by klynt-cli's recall injection
- `codingMemory.ingest.excludePaths` — used by klynt-cli's privacy guard
- `codingMemory.privacy.defaultSensitivity` — used by klynt-cli's sensitivity tagging
- `codingMemory.distiller.model` — klynt-cli respects this

Coding-memory spec §13.D — add comment on each: *"Used by klynt-cli — do not rename without coordinating both specs."*

### Cross-spec PR shape

**Commit 1: coding-memory amendments**
```
docs(specs): amend coding-memory for klynt-cli first-class source

- §5 add 10 klynt-rich AgentEvent variants
- §5 add klynt-cli adapter row + Native source subsection
- §6 add rich-variant Distiller handling table
- §4 add klynt_sessions table to consolidated migration list
- §13.D mark shared config keys

Coordinated with klynt-cli design at docs/superpowers/specs/2026-04-23-klynt-cli-design.md
```

**Commit 2: klynt-cli design**
```
docs(specs): add klynt-cli design doc

Single-binary klyntbot-first-party coding CLI: in-process with desktop
coordination, vendored Codex infrastructure, klynt-rich event vocabulary
into the cognitive store.
```

Both commits in same PR; amendments and new spec ship together.

### Implementation ordering

**Order A (recommended):** coding-memory implementation first, then klynt-cli.

Klynt-cli's first release is fully working rather than partially stubbed.

**Order B alternative:** klynt-cli first, coding-memory drains buffered events later. Workable but degraded first release.

### Shared invariants — both specs enforce

The coding-memory spec's 9 invariants from §3 + klynt-cli's 12 from §13 = **21 invariants** across both specs. All enforced by `proptest!`.

### Spec ownership

| Spec | Owner of changes | Types defined here |
|---|---|---|
| coding-memory | Cognitive subsystem layer | `AgentEvent` enum + variants, `Distiller`, `Reforge`, `Mirror`, `CodingRecallService`, `MemorySink` trait |
| klynt-cli | Coding CLI layer | `KlyntSession`, all the `klynt-*` crates, `WireServer`, `klynt-rollout`, `klynt-skill-loader`, `klynt-execpolicy`, `klynt-sandbox` |
| Shared | Both specs cite | `Channel`, `RoutingContext`, `ChannelName::CodingCli`, `Tool::is_concurrency_safe`, `klynt_sessions` table, the 21 invariants |

Cross-cutting changes go in coding-memory; CLI-shaped changes stay in klynt-cli's spec.

### Migration / compatibility considerations

Per CLAUDE.md ("Pre-release — no user data to migrate"):
- The 10 new `coding-ingest::AgentEvent` variants land in Phase 1 of coding-memory implementation
- The `klynt_sessions` table lands in same Phase 1 migration
- No backward-compatibility shims

After first release: proper versioned-migration discipline.

### Items NOT in this coordination

- klyntbot's existing chat channels (Telegram/Discord/Slack/Email/desktop) — unchanged
- MCP server's `default_exposed_tools()` — unchanged
- Existing `mirror_snippets` and `meta_rules` tables — unchanged
- `personas/` and `squads/` directories — out of scope

### One-page summary

> **Klynt-cli** is the in-process CLI that emits the richest events `AgentEvent` can carry. **Coding-memory** is the cognitive subsystem that consumes those events and gives klynt-cli memory. They share `~/.klyntbot/data.db`, the `AgentEvent` enum, the privacy guard, and 21 invariants. Klynt-cli implementation begins after coding-memory implementation completes Phase 5.

---

## Appendix A — Locked design decisions (the 9 axes)

| # | Axis | Decision |
|---|---|---|
| 1 | Identity | klyntbot's first-party coding CLI (tight coupling, max klyntbot power) |
| 2 | Process model | In-process same binary, with desktop coordination via shared ingest path |
| 3 | Agent loop | Shared cognitive pipeline (`AgentRuntime`); coding specialization at three pluggable seams (tool registry, RoutingContext channel, event subscribers) |
| 4 | Crate layout + Codex adaptation | Vendor selected Codex infra crates + lift TUI; rename all `codex-*` → `klynt-*` |
| 5 | Tool surface | Curated default (24 eager tools = coding kit + recall + tasks/notes/memory/mirror); `--power` opt-in for full surface |
| 6 | Approval philosophy | Three layers: declarative defaults + Starlark (power users) + Mirror-learned (opt-in Phase 2+) |
| 7 | Skill system | `.klyntbot/skills/` only; explicit `klynt skills install` for external sources; Reforge handles upgrades |
| 8 | Wire protocol | Minimal in Phase 1, klyntbot-native vocabulary, observer-only; in-process ingestion is primary |
| 9 | Event richness | Extend the spec's `AgentEventV1::EventKind` with klynt-cli-native variants; single vocabulary, klynt-cli emits a rich superset |

---

## Appendix B — New crate inventory

```
bot/crates/
├── klynt              # binary `klynt`
├── klynt-core         # CodingExecutionRouter, ToolRunner, hook bridge, sandbox glue
├── klynt-tui          # ratatui+crossterm TUI (adapted from Codex TUI)
├── klynt-protocol     # Event/Op/Submission types — adapted from codex-protocol
├── klynt-execpolicy   # Starlark prefix-rule approval engine — adapted
├── klynt-sandbox      # Seatbelt + Landlock + bwrap policy construction — adapted
├── klynt-hooks        # Hook engine — adapted, retargeted
├── klynt-rollout      # JSONL session recorder — retargeted
├── klynt-skill-loader # Multi-source skill discovery, conditional activation — fresh
└── klynt-linux-sandbox  # Linux child binary helper — renamed
```

Plus surgical changes:
- `tools-core::Tool` — add `is_concurrency_safe(args) -> bool` (default `false`)
- `crates/agent/src/execution/core.rs` — read-only-aware partitioning
- `crates/agent/src/events.rs` — ~20 new variants under `#[non_exhaustive]`
- `common::channels::CODING_CLI` constant
- Storage migration adding `klynt_sessions` table
- Chat channel match-arm audit for `_ =>` catch-all

---

## Appendix C — Shared invariants (21 total)

### From coding-memory spec §3 (9)

1. Provenance-always
2. Distiller-never-deletes
3. Reforge-never-deletes-raw
4. Bi-temporal monotone
5. SUPERSEDE chain
6. Scope isolation
7. Hook round-trip identity
8. Causal edge validity
9. Budget enforcement

### From klynt-cli §13 (12)

10. K1 — Translator round-trip determinism
11. K2 — Translator monotonicity
12. K3 — Rollout replay fidelity
13. K4 — Sequence monotonicity
14. K5 — Approval gate composition
15. K6 — Privacy guard inviolability
16. K7 — Sandbox-fallback safety
17. K8 — Skill discovery determinism
18. K9 — Boot warm vs cold equivalence
19. K10 — Mirror-learned cache poisoning (Phase 2)
20. K11 — Rewind round-trip (Phase 2)
21. K12 — Wire observer non-interference (Phase 2)

All enforced via `proptest!` in `tests/klynt_cli_property.rs` (klynt-cli's set) and `tests/coding_memory_property.rs` (coding-memory's set).

---

## Appendix D — Configuration shape (full)

Klynt-cli additions to `~/.klyntbot/config.json`:

```json
{
  "codingCli": {
    "tools": {
      "defaultProfile": "curated",
      "profiles": {
        "minimal": { "include": ["bash", "read", "edit", "ask_user"] },
        "curated": {
          "include": ["@coding-kit", "@recall-kit", "tasks", "notes", "memory", "mirror"],
          "deferred": ["@klyntbot-extra", "@mcp-tools-over-threshold"]
        },
        "power": { "include": ["@all"], "deferred": [] }
      },
      "deferredThreshold": 50,
      "alwaysEager": ["recall_index", "recall_timeline"]
    },

    "permissions": {
      "allow": ["Read(*)", "Glob(*)", "Grep(*)", "Bash(git status*)", ...],
      "deny": ["Bash(rm -rf /*)", "Bash(sudo *)", ...],
      "ask": ["Bash(*)", "WebFetch(*)", ...],
      "defaultIfNoMatch": "ask",
      "mirrorLearning": false
    },

    "skills": {
      "enableConditionalActivation": true,
      "enableDynamicDiscovery": true,
      "maxActiveSkills": 30,
      "frontmatterTokenBudget": 2000,
      "alwaysActivate": [],
      "neverActivate": []
    },

    "events": {
      "captureExtensions": ["anthropic_cache_*", "lsp_*", "experimental_*"],
      "rateLimitVariants": {
        "ContentChunk": { "maxPerSecond": 200 },
        "ToolCallStreamChunk": { "maxPerSecond": 100 }
      },
      "warnOnUnknownVariants": true,
      "logBrokerStatsEverySeconds": 60
    },

    "wire": {
      "enabled": true,
      "socketDirectory": "~/.klyntbot/sessions",
      "perObserverBufferCapacity": 256,
      "replayRingBufferSize": 1024,
      "maxObservers": 8,
      "slowObserverWarningThreshold": 3,
      "logObserverConnections": true,
      "defaultEventFilter": { "excludeKinds": ["ToolCallStreamChunk"] }
    },

    "rollout": {
      "enabled": true,
      "directory": "~/.klyntbot/sessions",
      "writerBufferCapacity": 1024,
      "flushOnEveryEvent": true,
      "retentionDays": 90,
      "maxTotalDiskMb": 5000,
      "compactRolloutAfterDays": 30,
      "preserveStarred": true,
      "snapshotsEnabled": false,
      "snapshotMaxFileSize": 1048576
    },

    "boot": {
      "warmCacheTtlSeconds": 60,
      "skipFirstRunWizard": false,
      "preflightOnLaunch": false,
      "lazyLanceLoad": true,
      "deferSkillScan": false
    },

    "version": {
      "channel": "stable",
      "autoUpdate": "off",
      "checkIntervalHours": 24
    }
  }
}
```

All flags default to safe values; user enables advanced features per-feature.

---

## Appendix E — Cross-spec amendment list

For the single-PR coordination per §14:

### `docs/superpowers/specs/2026-04-22-coding-memory-design.md` amendments

1. **§3** Add row to *Key decisions* table: "klynt-cli is a first-class source emitting the rich variant set; external CLIs emit a subset"
2. **§4** Add row to *Schema deltas* table: `klynt_sessions` table (CREATE TABLE per klynt-cli §10)
3. **§5** Add 10 new variants to `AgentEventV1::EventKind` (per klynt-cli §8)
4. **§5** Add row to *CLI adapter mappings*: klynt-cli (Native, in-process emit, all 19 events)
5. **§5** Add new subsection "Native source: klynt-cli" describing in-process emission via `MemorySink` trait
6. **§6** Add subsection "Rich-variant handling" with the table from klynt-cli §14 Category B
7. **§13.D** Mark shared config keys with comment "Used by klynt-cli — do not rename without coordinating both specs"
8. **§13.G** Update the "klynt-cli native coding CLI" row: change "Separate future project" status to "Linked spec at docs/superpowers/specs/2026-04-23-klynt-cli-design.md; implementation order: coding-memory first, then klynt-cli"
9. **§13.H Amendment log** — add row: "2026-04-23 | Amendment 2: klynt-cli first-class source coordination — rich variant set, native ingest path, klynt_sessions table, shared config keys, implementation ordering"

### `docs/superpowers/specs/2026-04-23-klynt-cli-design.md` (this file)

Created. Amendments to this spec follow the same single-PR pattern when needed.

---

*End of design.*
