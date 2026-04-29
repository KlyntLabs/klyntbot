# Klynt Coding-in-Chat — Design

**Date:** 2026-04-29
**Status:** Draft (pre-implementation)
**Scope:** Single design. Implementation plan will be derived via `writing-plans`.
**Pre-release policy:** Per CLAUDE.md — no user data to migrate, no backward-compat shims, no feature-flag gating. Schema changes consolidated into Phase 1.
**Supersedes:** [`docs/superpowers/specs/2026-04-23-klynt-cli-design.md`](./2026-04-23-klynt-cli-design.md). That design assumed a separate `klynt` TUI binary with desktop coordination via `~/.klyntbot/desktop.lock`. This design replaces the TUI binary with the existing desktop chat surface — coding becomes a first-class chat mode rather than a separate process.
**Companion spec:** [`docs/superpowers/specs/2026-04-22-coding-memory-design.md`](./2026-04-22-coding-memory-design.md) (amendments listed in §12).
**Related (already shipped):**
- [`docs/superpowers/specs/2026-04-26-klyntbot-chat-mvp-design.md`](./2026-04-26-klyntbot-chat-mvp-design.md) — initial chat MVP wiring.
- [`docs/superpowers/specs/2026-04-27-chat-surface-integration-design.md`](./2026-04-27-chat-surface-integration-design.md) — promoted klyntbot chat to render through the rich `Messages` + `Composer` surface.

---

## 1. Vision and non-goals

### Vision

**Klyntbot's coding capability lives inside the desktop chat.** A user opens the desktop app, opens a chat thread, flips it into "coding mode" (or starts a new thread already in coding mode), and from that point the chat is a full coding session: sandboxed shell, file edits, recall over prior coding turns, skill activation by file path, hook policy enforcement, approval gating. Every coding turn emits structured, klyntbot-native events that `Distiller`/`Mirror`/`Reforge` consume. There is **no separate binary, no TUI, no multi-process dance** — the desktop is the runtime, the chat thread is the session, the composer is the command surface.

The reason for existing remains the same as the superseded CLI spec: **be the richest data source klyntbot's cognitive subsystems will ever have.** What changes is the delivery surface — chat, not TUI — and everything that surface implies (React components, Tauri events, slash commands instead of subcommands).

### Goals

- One coding surface — the desktop chat — covers all use cases the CLI spec aimed at, except for unattended/headless use (out of scope for this spec; see §1 non-goals).
- Each chat thread is a coding session: thread = session, sidebar = session list, click = resume.
- The composer is the universal command surface: every former CLI subcommand becomes a slash command typed into `ComposerInput`.
- Coding-mode-aware tool registry (`bash`, `read`, `glob`, `grep`, `edit`, `write`, `apply_patch`, `web_fetch`, `ask_user`, `enter_plan_mode` / `exit_plan_mode`, `notebook_edit`) gated by a per-thread mode toggle, with auto-detection seed (open from a workspace/repo → coding mode by default).
- 3-layer approval model (declarative + Starlark + Mirror-learned) and OS-level sandbox (Seatbelt / Landlock) survive intact, with chat-inline approval cards instead of TUI dialogs.
- Skill system (`.klyntbot/skills/` + `~/.klyntbot/project-skills/<repo>/`) survives intact; skill management surfaces via slash commands and a Settings tab.
- Cognitive subsystems (Distiller, Mirror, Reforge) consume the same rich event vocabulary; channel name shifts from `coding_cli` to `coding`.
- Zero regression on existing klyntbot chat-channel functionality.

### Non-goals

- **No separate `klynt` binary.** Folded into the desktop binary. Power users invoke coding via the chat composer, not a shell command.
- **No TUI.** The React surface in `desktop-ui/src/features/` replaces ratatui+crossterm. No `klynt-tui` crate.
- **No Wire protocol observer attach.** Single process; nothing to attach to. Future IDE integration goes via MCP or a different mechanism, designed separately.
- **No multi-process coordination.** No `desktop.lock`, no heartbeat file, no `IngestSocketSink` fallback, no "is the daemon running" failure mode. The desktop process *is* the coding runtime.
- **No headless/unattended/CI mode.** If you want to run klyntbot's coding loop in CI, drive it via the existing MCP server (`klyntbot mcp serve --stdio`) or write a thin headless adapter — separate spec.
- **No standalone distribution.** Single installer (the desktop installer) ships everything.
- **No Windows sandbox.** macOS Seatbelt + Linux Landlock + bwrap. Phase 3+ if-and-when.
- **No rollout JSONL session recorder.** The existing `chat_messages` persistence is sufficient. Export-to-JSONL ships only if a concrete export use case appears (deferred).
- **No file-snapshot rewind.** Phase 2+ if-and-when.
- **No cross-ecosystem skill auto-discovery** (`~/.claude/skills/` etc.). Skills come from `.klyntbot/skills/` only; bringing in external skills requires explicit `/skills install`.

---

## 2. Architecture overview + component diagram

### Process model

Single Tauri 2 desktop process. The same process that hosts the existing chat, plugins page, settings, etc. Coding is a *capability layered onto* the existing AppCore + chat path, not a parallel runtime.

When the desktop starts, all coding infrastructure (tool registry entries, sandbox glue, approval engine, skill loader, hook engine) is wired into the existing `AgentRuntime`. The cost of having coding infrastructure loaded but unused is small: tools are eagerly registered, but the agent only sees the coding tool set when a session's `RoutingContext.channel == "coding"`.

### Component diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│              klyntbot desktop (single Tauri 2 process)                    │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │                desktop-ui (React, in webview)                     │    │
│  │  ┌────────────────────────────────────────────────────────────┐  │    │
│  │  │  Existing rich UI:                                         │  │    │
│  │  │   • Messages.tsx + MessageRows (message, tool, diff,       │  │    │
│  │  │     reasoning, userInput, review, explore, **approval**)   │  │    │
│  │  │   • Composer.tsx + ComposerSuggestionsPopover              │  │    │
│  │  │   • chatStreamStore + useChatSession + useAgentStream      │  │    │
│  │  └────────────────────────────────────────────────────────────┘  │    │
│  │  ┌────────────────────────────────────────────────────────────┐  │    │
│  │  │  NEW — features/coding/:                                   │  │    │
│  │  │   • useCodingMode hook (per-thread mode + auto-detect)     │  │    │
│  │  │   • useSlashCommands hook (dispatcher)                     │  │    │
│  │  │   • ApprovalCard, DiffPreview, RecallTrayCard rows         │  │    │
│  │  │   • CodingModePill in composer meta bar                    │  │    │
│  │  │   • CodingSettings page                                    │  │    │
│  │  └────────────────────────────────────────────────────────────┘  │    │
│  └────────────────────┬─────────────────────────────────────────────┘    │
│                       │ Tauri IPC: chat_send / chat_messages /            │
│                       │           chat_respond_interaction /              │
│                       │           coding_* (slash dispatcher endpoints)   │
│                       │ Tauri events: agent:* (existing) +                │
│                       │               6 new agent:* channels (see §10)    │
│                       ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │                  AppCore handlers (Rust)                          │    │
│  │  • chat_send: dispatches with channel = "coding" when mode=coding │    │
│  │  • coding_*: slash-command direct handlers (skills, status, ...)  │    │
│  │  • emit_updates → app.emit("agent:*")                             │    │
│  └────────────────────┬─────────────────────────────────────────────┘    │
│                       ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │                  AgentRuntime (existing)                          │    │
│  │   IntentAnalyzer → ContextEngine → SkillRouter → execute_loop     │    │
│  │   For channel="coding":                                           │    │
│  │     • coding tool registry (curated default; @power expand)       │    │
│  │     • CodingRecallService injected into ContextEngine             │    │
│  │     • coding-aware skills (paths-conditional + dynamic discovery) │    │
│  └─┬───────┬───────┬──────┬──────┬──────────────┬───────────────────┘    │
│    │       │       │      │      │              │                         │
│  ┌─▼─┐  ┌─▼─┐  ┌─▼──┐ ┌─▼──┐ ┌─▼──┐  ┌────────▼───────────┐              │
│  │exe│  │san│  │hook│ │skl-│ │tool│  │ klynt-protocol     │              │
│  │pol│  │box│  │s   │ │load│ │kit │  │  (event/op types,   │              │
│  └───┘  └───┘  └────┘ └────┘ └────┘  │  slim — no wire)    │              │
│   ▲ klynt-* infrastructure crates    └─────────────────────┘              │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  Cognitive subsystems (existing)                                  │    │
│  │   Distiller (consumes coding events into coding-memory)           │    │
│  │   Mirror (event-driven self-reflection; gains approval-history    │    │
│  │           and skill-effectiveness signal sources)                 │    │
│  │   Reforge (nightly synthesis; gains coding-rule-artifact phase)   │    │
│  └──────────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────────┘

         ▼ filesystem
   ┌──────────────────────────────────────┐
   │  ~/.klyntbot/                         │
   │    config.json (existing + codingMode)│
   │    data.db (existing; +coding columns)│
   │    lance/  (existing)                 │
   │    skills/  (existing convention)     │
   │    project-skills/<repo-id>/  (Reforge)│
   │    rules/*.rules  (Starlark approval) │
   │    hooks.toml  (hook config)          │
   └──────────────────────────────────────┘
```

### Data flow per coding turn

```
user types "now refactor the parser" in chat composer (mode = coding)
     ↓
ChatInput.onSend → chat.send({ content })
     ↓
invoke("chat_send", { content, sessionKey, context: { mode: "coding" } })
     ↓
AppCore::chat_send
     • looks up thread metadata; sets RoutingContext.channel = "coding"
     • forwards to AgentRuntime::process(...)
     ↓
AgentRuntime
     ├── IntentAnalyzer  → IntentClassified event
     ├── ContextEngine   → CodingRecallService injects recall snippets
     │       → RecallInjected event, DeadEndWarning if applicable
     ├── SkillRouter     → activate path-conditional skills based on cwd
     │       → SkillActivated events
     └── execute_loop()
            ├── ProviderRequest / ProviderResponse events
            ├── streaming AssistantMsg chunks → agent:content_chunk
            ├── tool calls:
            │     ├── PRIVACY GUARD (excludePaths)
            │     ├── 3-layer approval (declarative → Starlark → Mirror)
            │     │     ↳ if "ask": emit ApprovalRequested → frontend
            │     │       renders approval card; awaits user response
            │     ├── PreToolUse hook
            │     ├── sandbox launch (Seatbelt / Landlock)
            │     ├── tool execute → ToolStart / ToolEnd events
            │     │     ↳ for edit/apply_patch: FileEditWithSymbols event
            │     │       → frontend renders kind: "diff" row
            │     └── PostToolUse hook
            ├── parallel read-only tool dispatch (Tool::is_concurrency_safe)
            └── final AssistantMsg + token usage → agent:done
     ↓
chatStreamStore receives agent:* events; useChatSession + adapters push
into Messages props; React renders streaming text, tool rows, diff rows,
approval cards, recall tray.
     ↓
Distiller consumes the rich event stream into coding-memory tables.
Mirror consumes signals (approvals, skill activations, recall coverage).
```

### What changes vs. existing chat code path

The existing chat path (`crates/desktop/src/commands/chat.rs` → `AppCore::chat_send` → `AgentRuntime::process`) is **already** generic over channel. The coding path is *configured by* its `RoutingContext.channel` value — no new entry point in AppCore. The branching happens at:

1. **Tool registry construction** in agent runtime initialization: when `channel == "coding"`, the curated coding tool set is exposed.
2. **System prompt** assembly in `ContextEngine::build_system_prompt`: a coding-mode `ContextSource` injects coding system instructions when the channel matches.
3. **Recall injection** in ContextEngine: `CodingRecallService` is called only for the coding channel.
4. **Approval/sandbox/hooks** are tool-internal concerns: each coding-tool's `Tool::execute` consults `klynt-execpolicy` / `klynt-sandbox` / `klynt-hooks`. Non-coding channels never reach those code paths.

Result: only the React surface and the channel-routing constants are net-new entry points. Everything else is additive into existing extension seams.

### File system layout

```
~/.klyntbot/
  config.json                        # existing + new codingMode/permissions/skills/hooks keys
  data.db                            # existing (+ new columns on chat_sessions)
  lance/                             # existing
  skills/                            # existing convention; user-installed skills
  project-skills/<repo-id>/          # Reforge-synthesized
  rules/*.rules                      # Starlark approval rules (klynt-execpolicy)
  hooks.toml                         # hook configuration
```

No `sessions/<id>/` per-session directories. No `ingest.sock`. No `desktop.lock`. The single `data.db` plus the existing `chat_messages` / `chat_sessions` tables are sufficient.

---

## 3. Crate layout

### New crates added to `bot/crates/` (7 total)

Following klyntbot's flat-layout convention; dependency direction strictly upward.

| Crate | Layer | Purpose | Source |
|---|---|---|---|
| `klynt-protocol` | L0 | Event/Op/Submission types, `CodingTraceEvent` enum. **Slim** — wire types deleted. | Adapted from `codex-rs/protocol/` |
| `klynt-execpolicy` | L1 | Starlark prefix-rule approval engine; `~/.klyntbot/rules/*.rules` loader | Adapted from `codex-rs/execpolicy/` |
| `klynt-sandbox` | L1 | Seatbelt (.sbpl) policy gen for macOS, Landlock+bwrap for Linux; `SandboxPolicy` types | Adapted from `codex-rs/sandboxing/` |
| `klynt-sandbox-helper` | (binary) | Linux child-process helper that applies Landlock + seccomp | Adapted from `codex-rs/linux-sandbox/` |
| `klynt-hooks` | L2 | Hook engine: 13-event Claude-Code-compatible schema | Adapted from `codex-rs/hooks/`, retargeted to klyntbot's `AgentEvent` |
| `klynt-skill-loader` | L3 | `.klyntbot/skills/` + Reforge-path discovery, conditional activation | Fresh — extends `skill-system` |
| `klynt-core` | L4 | Coding-tool registry (`bash`, `read`, …), execpolicy/sandbox/hooks glue, slash-command dispatch handlers | Fresh — written against klyntbot crates |

**Deleted from the previous spec's plan:**
- `klynt-tui` — replaced by `desktop-ui/src/features/coding/`.
- `klynt` (binary) — folded into the existing `desktop` binary.
- `klynt-rollout` — superseded by existing `chat_messages` persistence; export-to-JSONL deferred.

### Code in `desktop` and `desktop-ui`

**Rust (`crates/desktop/`):**
- `commands/coding.rs` — Tauri commands for slash-command direct dispatch (`coding_skills_list`, `coding_skills_install`, `coding_status`, `coding_doctor`, `coding_resume`, …). Each is a thin adapter to an `AppCore` method.
- `commands/chat.rs` — gains a `mode` field in the `chat_send` payload (already accepts `context?`); when `mode == "coding"`, AppCore sets `RoutingContext.channel = "coding"`.
- Six new Tauri event channels (full list and payload shapes in §10): `agent:approval_requested`, `agent:approval_resolved`, `agent:file_edit_with_symbols`, `agent:recall_injected`, `agent:dead_end_warning_surfaced`, `agent:sandbox_policy_applied`. The frontend's `chatStreamStore` adds listeners; the React layer awaits user response on the approval pair and calls `chat_respond_approval` to resolve.

**Rust (`crates/app-core/`):**
- `coding/` module containing handlers used by both the Tauri commands and the future MCP coding-control surface.
- Routes `RoutingContext.channel == "coding"` to the coding tool registry inside `AgentRuntime` setup.

**React (`desktop-ui/src/features/coding/`):**
```
desktop-ui/src/features/coding/
├── components/
│   ├── ApprovalCard.tsx                  ← new ConversationItem variant
│   ├── ApprovalCard.test.tsx
│   ├── CodingModePill.tsx                ← composer meta-bar pill
│   ├── DiffPreview.tsx                   ← richer rendering for kind: "diff"
│   ├── RecallTrayCard.tsx                ← shown when RecallInjected fires
│   ├── DeadEndWarning.tsx                ← inline warning
│   └── CodingSettings.tsx                ← Settings tab
├── hooks/
│   ├── useCodingMode.ts                  ← per-thread mode + auto-detect
│   ├── useSlashCommands.ts               ← dispatcher
│   ├── useApprovalQueue.ts               ← subscribes to agent:approval_requested
│   └── useCodingRecallSnippets.ts        ← surfaces RecallInjected payloads
├── slash/
│   ├── registry.ts                       ← static catalog of all slash commands
│   ├── agentRouted.ts                    ← /plan /yolo /power /recall /...
│   └── direct.ts                         ← /skills /status /doctor /sessions /...
└── coding.css
```

`coding.css` is added to `src/styles/index.css`'s import chain.

### Dependency graph

```
                ┌────────────────────────────────────┐
                │  desktop (Tauri binary, L7)        │
                │  • commands/coding.rs              │
                │  • commands/chat.rs (extended)     │
                └──────────┬─────────────────────────┘
                           │
                           ▼
                ┌────────────────────────────────────┐
                │  app-core (L7)                     │
                │  • coding/ handlers                │
                └──────────┬─────────────────────────┘
                           │
                           ▼
                ┌────────────────────────────────────┐
                │  klynt-core (L4)                   │
                │  • coding tool implementations     │
                │  • slash-command direct handlers   │
                └──┬───┬───┬───┬───┬───┬─────────────┘
                   │   │   │   │   │   │
        ┌──────────┘   │   │   │   │   └──────────┐
        ▼              ▼   ▼   ▼   ▼              ▼
   ┌────────┐    ┌────────┐  ┌────────┐    ┌──────────┐
   │ klynt- │    │ klynt- │  │ klynt- │    │  klynt-  │
   │  hooks │    │ skill- │  │sandbox │    │execpolicy│
   │  (L2)  │    │ loader │  │  (L1)  │    │   (L1)   │
   └────────┘    │  (L3)  │  └────────┘    └──────────┘
                 └────────┘
                                ┌──────────────┐
                                │ klynt-       │
                                │ protocol     │
                                │ (L0; slim)   │
                                └──────────────┘
                            ▼ all depend on:
            ┌──────────────────────────────────────┐
            │  existing klyntbot crates (mostly    │
            │  unchanged — see §4 surgical edits)  │
            └──────────────────────────────────────┘
```

### Required surgical changes to existing crates

Same two as the superseded spec; both benefit all channels, not just coding:

1. **`tools-core::Tool` trait** — add `fn is_concurrency_safe(&self, args: &Value) -> bool { false }` (default false).
2. **`crates/agent/src/execution/core.rs`** — `execute_tool_calls` partitions by `is_concurrency_safe` (parallel for safe, sequential for unsafe). ~30 lines.

Plus:

3. **`crates/agent/src/events.rs`** — 18 new variants under `#[non_exhaustive]` (see §10). Three previously-listed variants from the superseded spec are deleted (Wire-related: nothing to plumb).
4. **`crates/common/src/types.rs`** — add `pub const CODING_CHANNEL: &str = "coding";` alongside the existing `SYSTEM_CHANNEL` / `CLI_CHANNEL` / `MCP_CHANNEL` literals. (Note: existing channel names like `"desktop"`, `"telegram"`, etc. are passed as raw strings to `ChannelName::new(...)` rather than constants; we lift `coding` to a constant because it's referenced in tool gating + Distiller filtering.)
5. **Chat channel match-arm audit** — every existing `match AgentEvent { ... }` outside the coding path must have a `_ =>` catch-all so additive variants don't break compilation.
6. **`chat_sessions` schema** — add columns per §11: `mode` (TEXT NOT NULL DEFAULT 'chat'), `cwd` (TEXT, nullable), `repo_id` (TEXT, nullable), `repo_branch` (TEXT, nullable), `tool_profile` (TEXT, nullable), `approval_mode` (TEXT NOT NULL DEFAULT 'default'), `total_cost_usd` (REAL NOT NULL DEFAULT 0), `total_tokens` (INTEGER NOT NULL DEFAULT 0), `starred` (BOOLEAN NOT NULL DEFAULT FALSE), `parent_session_id` (TEXT, nullable). One migration per pre-release policy.
7. **New Tauri command `chat_set_mode`** — accepts `{ sessionKey, mode }`, persists to the new `chat_sessions.mode` column, returns the updated row. Used by the `<CodingModePill>` mid-thread flip; new threads set the mode at creation via the existing `chat_send` payload.

---

## 4. Agent loop integration

### How coding plugs into the unified `execute_loop`

`crates/agent/src/execution/execute_loop.rs` is unchanged at the call-site level; the coding path uses the same entry point as every other channel:

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

The `event_tx` argument is a `tokio::sync::mpsc::Sender<AgentEvent>` — a **single-consumer** channel. It delivers events into the chat-streaming pipeline, which `app.emit("agent:*")`s to the React frontend. There is no fan-out at the `event_tx` seam.

For cognitive subsystems (Distiller, Mirror), klyntbot already has a separate broadcast bus: `DomainEventBus` (defined in `crates/bus/src/domain_events.rs`; uses `tokio::sync::broadcast` under the hood). The existing pattern is that subsystems publish to `DomainEventBus::publish(...)`; subscribers call `bus.subscribe()` and consume independently.

Coding events flow through both paths:

- **Path 1 — UI streaming**: every coding-relevant `AgentEvent` flows through `event_tx` → `app.emit("agent:*")` → `chatStreamStore` → React renders.
- **Path 2 — Cognitive ingest**: the same events (or a translated subset, per §10) are published to `DomainEventBus`. Two new subscribers spawn at AppCore init:
  - **Distiller subscriber** — consumes the rich event variants, writes to coding-memory tables. Filters on `RoutingContext.channel == CODING_CHANNEL`.
  - **Mirror signal subscriber(s)** — one per signal source (approval-history, skill-effectiveness, recall-coverage). Each consumes the relevant variant and feeds the existing Mirror engine (`MirrorEngine::start` in `crates/cognitive/src/mirror/engine.rs`).

The translator that converts runtime `agent::events::AgentEvent` into `coding-ingest::AgentEvent` (§10 Translator subsection) sits inside the Distiller subscriber. Mirror subscribers consume runtime events directly.

**Implementation detail — where the bus publish call sits:** AgentRuntime gains a `domain_event_bus: Arc<DomainEventBus>` constructor parameter (already an `Arc` in the existing codebase per CLAUDE.md "MirrorEngine::start takes Arc<DomainEventBus>"). The cleanest insertion point is a small helper that tees: every existing `event_tx.send(evt).await?` site is replaced with a single helper call that publishes to both. New helper in `crates/agent/src/execution/core.rs`:

```rust
async fn fan_out_event(
    event_tx: &Option<&mpsc::Sender<AgentEvent>>,
    domain_bus: &Arc<DomainEventBus>,
    evt: AgentEvent,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;   // UI path; ignore drops
    }
    domain_bus.publish(DomainEvent::Agent(evt));   // cognitive path
}
```

Existing call sites in `core.rs` and `execute_loop.rs` switch from `event_tx.send(...)` to `fan_out_event(event_tx.as_ref(), &domain_bus, ...)`. The change is mechanical — ~12 call sites — and lands as part of the surgical edits in §3. The bus channel is sized large enough to absorb burst (use existing `DomainEventBus::new(capacity)` with capacity tuned via `codingMemory.bus.capacity` already in coding-memory's config).

### Coding specialization at three pluggable seams

#### Seam 1 — `ToolRegistry` content

Coding tools are registered eagerly into the `ToolRegistry` at app start, but only **selected** when `channel == "coding"`. Two implementation options:

- **Option A (selected):** A single `ToolRegistry` containing every tool; `AgentRuntime` filters by channel at tool-list construction time using a per-tool `available_for_channel(channel: &str) -> bool` predicate.
- Option B: Two registries (chat / coding), wired by channel. Rejected because it duplicates infra.

Tools registered for the coding channel:

| Tool | Crate | Sandbox-aware | Approval-aware | Available in non-coding channels? |
|---|---|---|---|---|
| `bash` | `klynt-core::tools::bash` | yes | yes | no |
| `read` / `glob` / `grep` | `klynt-core::tools::fs` | yes (read-only) | rule-checked | no |
| `edit` / `write` / `apply_patch` | `klynt-core::tools::edit` | yes | yes | no |
| `web_fetch` | `klynt-core::tools::web` | n/a (network) | yes | no |
| `notebook_edit` | `klynt-core::tools::notebook` | yes | yes | no |
| `enter_plan_mode` / `exit_plan_mode` | `klynt-core::tools::mode` | n/a | n/a | no |
| `recall_*` (8 tools) | `coding-memory` | n/a | n/a | yes (configurable per channel; coding channel exposes them by default) |

Universal tools (also exposed in coding channel): `task` (subagent), curated klyntbot domain tools, MCP gateway.

#### Seam 2 — `RoutingContext` channel name

```rust
// crates/common/src/types.rs
pub const SYSTEM_CHANNEL: &str = "system";    // existing
pub const CLI_CHANNEL: &str = "cli";          // existing
pub const MCP_CHANNEL: &str = "mcp";          // existing
pub const CODING_CHANNEL: &str = "coding";    // NEW
// Other channel names like "telegram", "discord", "slack", "email", "desktop"
// are not lifted to constants today; they're passed as raw strings to
// ChannelName::new(...). Coding gets a constant because it's referenced in
// tool gating and Distiller filtering.
```

The `chat_send` Tauri command sets `RoutingContext.channel = ChannelName::new(CODING_CHANNEL)` when the thread's `mode == "coding"`, otherwise it uses the existing chat-channel literal (`"desktop"`). Tool implementations gate coding-specific behavior on this string.

#### Seam 3 — `agent::events::AgentEvent` enum extension

Same additive philosophy as the superseded spec: 18 new variants under `#[non_exhaustive]`; all match arms outside the coding path get `_ =>`. See §10 for the full list.

### Per-turn lifecycle

```
chat composer → chat.send(content)
   ↓
invoke("chat_send", { content, sessionKey, context: { mode: "coding" } })
   ↓
AppCore::chat_send {
   thread = lookup(sessionKey)
   channel = thread.mode == "coding"
       ? ChannelName::new(CODING_CHANNEL)   // common::types::CODING_CHANNEL = "coding"
       : ChannelName::new("desktop")        // existing chat channel literal
   ctx = RoutingContext { channel, repo_id, cwd, ... }
   runtime.process(content, ctx, Some(event_tx))
}
   ↓
runtime.process → IntentAnalyzer → ContextEngine → SkillRouter → execute_loop
   ↓
execute_loop {
   loop {
      budget gate / cancellation check
      provider.stream() → emit ContentChunk events
      tool calls:
         - privacy guard (always first)
         - 3-layer approval; ApprovalRequested fires for every gate evaluation
           (with requires_user_input: true|false). When true, also emit the
           agent:approval_requested Tauri event and await ApprovalResolved
           over a oneshot bound to request_id. When false, ApprovalResolved
           fires immediately with decided_by: auto_allow | auto_deny.
         - PreToolUse hook
         - sandbox launch (Seatbelt/Landlock)
         - tool execute (parallel for is_concurrency_safe; sequential otherwise)
         - PostToolUse hook
      mid_loop_compressor.maybe_compress()
      live_context_refresher.maybe_refresh()
   }
}
   ↓
events flow through two paths (see §4 above):
   • event_tx (mpsc, single consumer) → app.emit("agent:*") → chatStreamStore → React
   • DomainEventBus::publish (broadcast, multi-subscriber):
        ├── Distiller subscriber  → writes coding-memory tables
        └── Mirror signal subscribers → feed MirrorEngine
```

### Approval round-trip mechanism

When approval Layer 1/2/3 returns "ask," the agent loop **must wait** for a user decision before executing the tool. Mechanism (auto-allow / auto-deny paths short-circuit at step 0 below; only the "ask" path runs the full sequence):

0. (Always, regardless of decision) Emit `ApprovalRequested` runtime variant. If decision is auto-allow / auto-deny, immediately also emit `ApprovalResolved` with the appropriate `decided_by`, then proceed to PreToolUse hook. The Tauri `agent:approval_requested` event channel is **not** triggered for auto cases — only the runtime variant fires (for Distiller + Mirror).

1. (Ask path only) The `klynt-core::tools::approval` middleware allocates a `request_id`, registers a `oneshot::Sender<ApprovalDecision>` in a shared `DashMap<RequestId, oneshot::Sender>` keyed on the request_id, and emits `agent:approval_requested` Tauri event with `{ request_id, tool, args, sandbox_summary, layer, mirror_history }`.
2. The frontend renders an `ApprovalCard` row with Allow once / Allow always / Deny / Add rule buttons.
3. User clicks → `invoke("chat_respond_approval", { sessionKey, requestId, decision })`.
4. AppCore looks up the `oneshot::Sender` by `request_id`, sends the decision through it, and removes the entry.
5. Middleware wakes from its `select!`, applies the decision (and writes a Layer-1/2 rule if "Allow always" / "Add rule"), continues.

The middleware's await pattern is a three-way `select!` over (a) the user's response, (b) the session cancellation token, (c) the 600s timeout:

```rust
let (tx, rx) = oneshot::channel();
pending_approvals.insert(request_id, tx);
emit_approval_requested(...);

let decision = tokio::select! {
    user_response = rx => match user_response {
        Ok(d) => d,
        Err(_) => ApprovalDecision::deny_with("approval channel closed"),
    },
    _ = cancel_token.cancelled() => ApprovalDecision::cancelled(),
    _ = tokio::time::sleep(INTERACTIVE_TOOL_TIMEOUT) => ApprovalDecision::timed_out(),
};
pending_approvals.remove(&request_id);
emit_approval_resolved(request_id, decision);
```

Timeout: 600s (matches existing `INTERACTIVE_TOOL_TIMEOUT`). Cancellation: invoking `chat_cancel(sessionKey)` (existing API) cancels the session's `CancellationToken`, which the `select!` arm above observes; pending approvals resolve as `ApprovalDecision::cancelled()` and the tool returns `Err(ToolError::Cancelled)`. Closing the desktop window does **not** cancel the session — the agent loop continues running in the background and approvals stay pending until the user reopens the thread or invokes `chat_cancel` explicitly.

### Error handling matrix

| Failure | Behavior | Why |
|---|---|---|
| Tool call panic | Catch via `catch_unwind`; emit `ToolCall { ok: false }`; continue loop | One bad tool doesn't kill the turn |
| Provider 5xx | Retry per `ProviderManager` policy | Inherits klyntbot's retry config |
| Provider 4xx | Emit `Error`; abort turn | Unrecoverable |
| Sandbox launch failure | Fall back to unsandboxed with prominent UI banner + tightened approval | OS gaps shouldn't block work; user knows |
| Hook subprocess timeout | Fail open; log to `mirror_snippets` | Prevent flaky hooks from blocking sessions |
| Approval denied | Tool returns `Err(ToolError::Denied)`; agent continues | Standard ReAct error path |
| Approval card timeout | Treat as "deny once"; user can re-prompt | Predictable end-state |
| User closes desktop window mid-approval | Session keeps running; approvals stay pending | Window ≠ session — closing only hides the UI; agent loop is alive in the background |
| User invokes `chat_cancel(sessionKey)` | Pending approvals resolve as `Err(ToolError::Cancelled)`; loop aborts | Explicit cancel is the only way to kill an in-flight session |
| Recall query timeout (>5s) | Return partial results + marker | Don't stall the turn for memory |

---

## 5. The chat surface — React components and UX

### Mode toggle on the composer

`Composer.tsx` is unchanged except that `composerProps` (built by `useKlyntbotSurfaceProps`) gains a `modeOptions: [{ id: "chat", label: "Chat" }, { id: "coding", label: "Coding" }]` and `selectedMode` / `onSelectMode`. The composer's existing meta-bar slot (currently displaying the model and collaboration-mode pills) gains a new `<CodingModePill>` that swaps mode mid-thread.

Implementation: extend `useChatSession` to track and persist `thread.mode` to the backend (round-trip via a new `chat_set_mode` command writes to the `chat_sessions` table column added in §3).

**Mid-thread flip semantics:** Flipping the mode applies from the next user turn forward. Prior history (messages, tool rows, approval cards, diff rows) is unchanged. The next agent turn rebuilds the tool list (per the new mode's curated profile) and regenerates the system prompt before `execute_loop` runs. Approval rules persisted via "Allow always" / "Add rule" remain in effect regardless of mode (they're stored in `~/.klyntbot/config.json` and `~/.klyntbot/rules/`, not per-thread).

### Auto-detection of coding mode

When a new chat is created **from a workspace/repo context** (e.g., user clicked "New chat" while a workspace was selected, or opened a chat from a repo card), the new thread is seeded with `mode: "coding"` and `cwd: <repo path>`, `repo_id`, `repo_branch`. Otherwise mode defaults to `chat`.

Detection points:
- The Sidebar's `WorktreeSection.tsx` and `WorkspaceCard.tsx` already track repo context. When "New chat" originates from one of these, pass that context through to `MainApp.tsx`'s thread-creation handler.
- In any other case (top-level "New chat" button), default to `chat` mode; user can flip the pill at any time.

### Slash commands in the composer

The composer's autocomplete is implemented as: `ComposerSuggestionsPopover.tsx` (purely presentational; renders an `AutocompleteItem[]` list) + `useComposerAutocomplete.ts` (the trigger engine; accepts a configurable `triggers: AutocompleteTrigger[]` array, where each trigger declares its prefix character — e.g., `@`, `/`, `#` — and a source of items). Today the composer is constructed with triggers for skills/apps/prompts/files; the slash trigger is **not yet wired**.

Wiring (this spec):

1. Add a new `AutocompleteTrigger` entry with `trigger: "/"` and a source backed by `useSlashCommands().catalog`. The catalog returns slash-command items grouped by category (Mode, Skills, Status, Sessions, Permissions, Recall).
2. The trigger fires when `/` appears at the start of the input (or after whitespace/punctuation; existing `triggerPrefixRegex` in `useComposerAutocomplete.ts` enforces this).
3. Selecting an item from the popover inserts the command's text. Submitting (Enter) sends as usual.
4. Composer's `onSend` receives the raw input. `useChatSession.send` first asks `useSlashCommands().classify(input)`; based on the result, it either passes the message through to the agent (agent-routed) or invokes the direct dispatcher (direct) — see §9.

The `Composer.tsx` component itself does not change: triggers are passed in as configuration. The actual registration site is `useKlyntbotSurfaceProps` (which builds `composerProps`) — verified caller is `desktop-ui/src/features/app/components/MainApp.tsx:1740`. The hook returns an extended `composerProps.autocompleteTriggers` array (or equivalent prop name verified during writing-plans) that the composer threads into `useComposerAutocomplete`. The trigger is registered only when `mode == "coding"`; chat-mode threads keep their existing trigger set unchanged.

### Approval cards

Define a new `ConversationItem` variant:

```ts
| {
    id: string;
    kind: "approval";
    requestId: string;
    tool: string;
    args: Record<string, unknown>;
    cwd: string;
    sandboxSummary: string;
    layer: "privacy" | "declarative" | "starlark" | "mirror" | "default";
    layerReason: string;
    mirrorHistory?: { approvalCount: number; denialCount: number };
    status: "pending" | "approved-once" | "approved-always" | "denied" | "timed-out" | "cancelled";
    decidedAt?: string;
    decidedBy?: "user" | "auto_allow" | "auto_deny" | "timeout" | "cancelled";  // mirrors ApprovalResolved.decided_by
  };
```

`ApprovalCard.tsx` renders this row inline in the message stream. Keyboard shortcuts: `a` allow once, `s` allow always, `d` deny, `r` add rule (opens an inline Starlark editor). Pending cards are visually distinct (subtle pulse + "awaiting decision"); decided cards collapse to a one-line summary.

`MessageRows.tsx` gains a case for `kind: "approval"` rendering `<ApprovalCard {...} />`.

### Diff preview rows

The rich UI already has `kind: "diff"` rows from MessageRows. Today klyntbot doesn't emit data for them. With coding, **every `edit`/`write`/`apply_patch` tool call** produces a `FileEditWithSymbols` event. The adapter in `useKlyntbotSurfaceProps` maps the event to `kind: "diff"` with `{ path, hunks, anchoredSymbols, lspDiagnosticsDelta }`. The new `DiffPreview.tsx` is a small enhancement layer (syntax highlighting, expand/collapse, "view in editor" affordance) wrapped around the existing `kind: "diff"` row.

### Recall tray

When `RecallInjected` fires, the adapter pushes a `kind: "explore"` row (existing variant) augmented with klyntbot's `memory_ids`, `coverage_score`, `escalation_chain`. `RecallTrayCard.tsx` is a small component plugged into MessageRows' `kind: "explore"` case to render coverage + dead-end warning + click-to-expand citations.

### Status / cost / token affordances

The composer already supports `contextUsage` (token usage indicator). The adapter maps the cumulative `ProviderResponse` cost + token counts into `contextUsage`. A new pill in the composer meta bar shows total cost for the thread (`$0.034`) and the active sandbox status (`Sandbox: Seatbelt cwd-only` | `unsandboxed (warning)`).

### Settings page

`desktop-ui/src/features/settings/` gains a "Coding" section:

- **General**: default mode for new threads (`chat` | `coding`), auto-detect-from-workspace toggle.
- **Tools**: tool-profile selector (`minimal` / `curated` / `power`), per-thread override note.
- **Permissions**: declarative allow/deny/ask lists (textarea-edited; backed by `~/.klyntbot/config.json`), Mirror-learned approval toggle (default off).
- **Sandbox**: enforce on/off, warning when disabled, "test sandbox" button.
- **Skills**: list of installed skills with source + version + last-activated; install/update/uninstall buttons; install-from-URL field.
- **Hooks**: read-only display of `hooks.toml` plus an "open in editor" affordance (the file is user-managed).
- **Sessions**: retention controls (days, max disk, starred preservation).

Everything in this page maps 1-to-1 to a slash command (so power users never need to leave the composer; click users prefer the page).

### What stays unchanged in the existing chat infrastructure

- `chatStreamStore.ts`, `useChatSession`, `useChatThreads`, `useAgentStream` — unchanged except for the new event types they listen for (`agent:approval_requested`, `agent:approval_resolved`, `agent:file_edit_with_symbols`, `agent:recall_injected`, etc.). These are additive listeners.
- `Messages.tsx` — unchanged. New `ConversationItem` variants are handled inside `MessageRows.tsx`.
- `Composer.tsx` — unchanged structurally; new pills/props are additive.
- The non-coding (general chat) path renders identically.

---

## 6. Tool surface + curation model

### Tool inventory (the universe)

**Pool 1 — Coding tool kit (new, in `klynt-core::tools`):**

| Tool | Concurrency-safe | Sandbox required | Approval-aware |
|---|---|---|---|
| `bash` | no | yes | yes |
| `read` | yes | no | rule-checked |
| `glob` | yes | no | rule-checked |
| `grep` | yes | no | rule-checked |
| `edit` | no | yes | yes |
| `write` | no | yes | yes |
| `apply_patch` | no | yes | yes |
| `web_fetch` | no | n/a (network) | yes |
| `ask_user` | no | n/a | n/a |
| `enter_plan_mode` / `exit_plan_mode` | no | n/a | n/a |
| `notebook_edit` | no | yes | yes |

**Pool 2 — Recall tool kit** (from coding-memory): `recall_index`, `recall_timeline`, `recall_fetch`, `trace_causes`, `check_dead_ends`, `recall_facts_as_of`, `recall_change_history`, `recall_decision_points`. All read-only and concurrency-safe.

**Pool 3 — Klyntbot domain tools** (existing 15): `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `mirror`, `temporal`.

**Pool 4 — MCP gateway**: External MCP servers, surfaced as `mcp_<server>_<tool>`.

### Default curated set (coding mode)

24 eager tools when `mode == "coding"`:

```
Coding kit (12): bash, read, glob, grep, edit, write, apply_patch,
                 ask_user, enter_plan_mode, exit_plan_mode,
                 notebook_edit, web_fetch
Recall kit (8):  recall_index, recall_timeline, recall_fetch,
                 trace_causes, check_dead_ends,
                 recall_facts_as_of, recall_change_history,
                 recall_decision_points
Klyntbot lite (4): tasks, notes, memory, mirror
MCP gateway:     varies by user config
```

### Configuration shape

```json
{
  "coding": {
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
    }
  }
}
```

`@coding-kit`, `@recall-kit`, `@klyntbot-extra`, `@mcp-tools-over-threshold`, `@all` are aliases resolved by the loader.

### `/power on|off` slash command

Replaces the old `--power` flag and `/power` slash. Toggles the active profile **for the current thread**. Tool list rebuilt; system prompt regenerated for next iteration. Persists to the `chat_sessions.tool_profile` column. Emits `PowerModeToggled` event.

### Deferred-tool discovery (Phase 2)

Same `tool_search` tool as the superseded spec when the deferred list is non-empty. Phase 2+ reranking via Mirror's per-skill effectiveness scores.

### Tool concurrency model

Same as superseded spec: `Tool::is_concurrency_safe(args) -> bool` (default false; coding read tools override to true). Loop partitions safe tools for parallel dispatch (capped by existing `MAX_CONCURRENT_TOOLS = 10`); unsafe tools run sequentially.

### Tool result handling

Existing 50KB cap at `crates/agent/src/execution/core.rs:31` inherited unchanged. Phase 3+ adds Claude Code's content-replacement pattern for oversized results we want to preserve in full.

### Conflict detection

Naming hygiene unchanged: short verbs for the coding kit; `recall_` / `trace_` / `check_` prefixes for recall; `mcp_<server>_<tool>` for MCP. Conflict detection runs at registry build time; duplicates abort boot.

---

## 7. Approval + sandbox model (3-layer)

### Decision flow (unchanged from the superseded spec)

```
Tool call arrives
       │
       ▼
PRIVACY GUARD ─────── always first; non-bypassable; spec excludePaths
       │
       ▼
LAYER 1 — DECLARATIVE  (config.json allow/deny/ask globs)
       │
       ▼
LAYER 2 — STARLARK     (~/.klyntbot/rules/*.rules; prefix_rule, custom_rule)
       │
       ▼
LAYER 3 — MIRROR       (Phase 2; opt-in; auto-approve learned patterns)
       │
       ▼
MODE DEFAULT (default | plan | bypass)
```

Every gate evaluation — auto-allow, auto-deny, or "ask" — produces a paired `ApprovalRequested` + `ApprovalResolved`. The fields distinguish:
- `ApprovalRequested.requires_user_input == false` for auto-allow / auto-deny / privacy-blocked. The frontend ignores these (no `ApprovalCard` rendered); they're emitted purely for telemetry and so the translator has a uniform input.
- `ApprovalRequested.requires_user_input == true` only when the layers decide "ask." The frontend emits `agent:approval_requested` and renders the card; the agent loop awaits `ApprovalResolved`.
- `ApprovalResolved.decided_by` captures who/what produced the verdict: `user`, `auto_allow`, `auto_deny`, `timeout`, `cancelled`.

The translator (§10) collapses each `ApprovalRequested` + `ApprovalResolved` pair into one `ApprovalDecision` ingest event regardless of `requires_user_input`. This guarantees the translator has a uniform input shape for every approval evaluation, matches invariant K8 (approval round-trip identity), and means auto-decisions still flow into Mirror's approval-history signal source for Layer 3 learning.

### Approval modes

| Mode | When | Reads | Writes | Exec | Special |
|---|---|---|---|---|---|
| `default` | normal | per layers | per layers | per layers | layers run as designed |
| `plan` | `/plan` | allowed | denied | denied | research-only; agent told via system prompt |
| `bypass` | `/yolo` | allowed | allowed | allowed | requires `KLYNTBOT_ENABLE_YOLO=1` env |

The previous spec's `print` and `print --yolo` modes (for headless CI use) are out of scope; CI use goes through MCP separately.

### Layer 1 — Declarative rules

```json
{
  "coding": {
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
      "defaultIfNoMatch": "ask",
      "mirrorLearning": false
    }
  }
}
```

Matcher syntax: `Tool(glob)` with `globset` semantics. Bash globs against the full command-line; file tools against the resolved absolute path.

### Layer 2 — Starlark execpolicy

Loads `~/.klyntbot/rules/*.rules`:

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

In-thread "always allow for this session" appends in-memory rules via `append_session_allow_prefix(...)`.

### Layer 3 — Mirror-learned (Phase 2; opt-in)

After Layers 1 and 2 fall through:

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

`args_hash_for_relevance` ignores volatile fields. Cool-down per-repo: after the 5th approval, wait 24h before activating auto-approve. Single denial poisons cache for that key (until explicit clear via `/permissions clear-mirror <tool>`).

### Privacy guard

Coding-memory's `excludePaths` (`.env`, `secrets/**`, `*.key`, …) evaluated **before any layer**. Cannot be disabled by `/yolo`. Widen/narrow via `<repo>/.klyntbot/ignore.toml`.

### Sandbox enforcement

**macOS (Seatbelt):** Each `bash`/`edit`/`write` runs via `sandbox-exec` with a generated `.sbpl` policy. Default denies all, then allows process-fork, signal-self, file-read of cwd ancestors + common system paths, file-write only to cwd, network on (deny via permission rules).

**Linux (Landlock + bwrap):** `klynt-sandbox-helper` exec'd as a child; applies Landlock filesystem restrictions in-process; bwrap provides namespace sandbox.

**Windows:** Out of scope; macOS + Linux only at first ship.

### Sandbox failure handling

If sandbox unavailable: `klynt-core` does **not** silently run unsandboxed. Detects missing capability at startup (or first sandboxed launch), shows a banner in the chat header, tightens approval gate (every bash/exec/write defaults to `ask`), emits `SandboxPolicyApplied { fallback_unsandboxed: true }`. User can opt to run unsandboxed without tightening via `KLYNTBOT_ALLOW_UNSANDBOXED=1`.

### Hook interaction

`PreToolUse` hooks run **after** privacy guard + Layers 1-3 but **before** sandbox launch. Block return aborts the call. Hooks can add restrictions; cannot override a deny.

### `hooks.toml` schema

The hook engine reads `~/.klyntbot/hooks.toml`. Schema mirrors Claude Code's hook configuration (so existing user hooks work without translation):

```toml
# ~/.klyntbot/hooks.toml — Claude-Code-compatible schema

[[hook]]
event = "PreToolUse"
matcher = "Bash(*)"          # globset pattern; same syntax as Layer 1 permissions
command = "scripts/log-bash.sh"
timeout_ms = 5000
fail_open = true             # if hook subprocess errors or times out, continue (default true)

[[hook]]
event = "PostToolUse"
matcher = "Edit(./crates/**)"
command = "scripts/auto-format-rust.sh"
timeout_ms = 10000

[[hook]]
event = "Stop"
command = "scripts/notify-done.sh"
```

**The 13 hook events** (the `event = "..."` field):

| Event | Fires when |
|---|---|
| `SessionStart` | Coding session begins (chat thread enters coding mode for the first time, or new coding thread created). |
| `SessionEnd` | Session ends (`chat_cancel` invoked, or session reaches a quiescent terminal state). |
| `UserPromptSubmit` | User submits a message via composer (after slash classification, before agent loop starts). |
| `PreCompact` | Mid-loop compressor about to compact the message history. |
| `PostCompact` | Mid-loop compressor finished compacting. |
| `PreToolUse` | Tool call about to execute (after privacy guard + Layers 1-3 + approval but before sandbox launch). |
| `PostToolUse` | Tool call finished (success or failure). |
| `PreFileEdit` | A subset of `PreToolUse` for `edit` / `write` / `apply_patch`; gives early access to the diff before it's applied. |
| `PostFileEdit` | A subset of `PostToolUse` for the same tools; fires after the file is on disk. |
| `Notification` | Notification surfaced to the user (e.g., approval card opens). |
| `SubagentSpawn` | Agent calls the `task` tool to spawn a subagent. |
| `Stop` | Agent loop's terminal turn (final assistant message + done event). |
| `Error` | Unrecoverable error in the agent loop. |

**Hook output protocol:**
- The hook subprocess receives event metadata as JSON on `stdin`.
- Exit code 0 = OK, allow continuation. Exit code non-zero = error; behavior governed by `fail_open` (default true: log + continue; `fail_open = false`: abort the in-flight tool call with `Err(ToolError::HookFailed)`).
- For `PreToolUse` and `PreFileEdit`, the hook can additionally write structured JSON on `stdout`: `{ "block": true, "reason": "..." }` aborts the call regardless of exit code; `{ "modify_args": <new-args-object> }` rewrites the tool arguments before execution; absence of stdout JSON is equivalent to "allow."
- `timeout_ms` defaults to 30000 (30s); on timeout, behavior follows `fail_open`.

A blocking-bash example:

```bash
#!/usr/bin/env bash
# scripts/block-dangerous-bash.sh
input=$(cat)              # event JSON on stdin
cmd=$(echo "$input" | jq -r '.tool.args.command')
if echo "$cmd" | grep -qE '(rm -rf /|sudo|curl.*\| sh)'; then
  echo '{"block": true, "reason": "blocked dangerous command pattern"}'
fi
exit 0
```

### Chat-inline approval card (UI)

Shown in the message stream as a `kind: "approval"` row. Pending state visually distinct (subtle pulse + "awaiting decision"); decided state collapses to a one-line summary.

```
┌─ Approval needed ──────────────────────────────────────────┐
│  Tool: bash                                                │
│  Args: cargo test --workspace                              │
│  CWD:  /Users/jayden/Projects/Klynt/bot                    │
│  Sandbox: Seatbelt (cwd-only file writes)                  │
│  Layer: layer-2/starlark — no matching rule                │
│  Mirror history: 12 approvals, 0 denials in this repo      │
│                                                            │
│  [Allow once]  [Allow always]  [Deny]  [Add rule…]         │
└────────────────────────────────────────────────────────────┘
```

Keyboard: `a` allow once, `s` allow always, `d` deny, `r` add rule.
- "Allow always" appends to `coding.permissions.allow` (Layer 1).
- "Add rule" opens an inline Starlark editor (Layer 2); on save writes to `~/.klyntbot/rules/<auto-named>.rules`.
- "Deny" returns `Err(ToolError::Denied)`; agent continues.

Window-not-focused fallback: `ApprovalToasts.tsx` (already exists) renders a system notification toast; clicking opens the chat to the approval card.

---

## 8. Skill system

### Discovery (unchanged from the superseded spec)

```rust
const STATIC_PATHS: &[(&str, SkillSource)] = &[
    ("~/.klyntbot/skills",                              SkillSource::User),
    (".klyntbot/skills",                                SkillSource::Project),
    ("~/.klyntbot/project-skills/<sanitized-repo-id>",  SkillSource::ReforgePrivate),
    ("<repo_root>/.klyntbot/skills",                    SkillSource::ReforgeTeam),
];
```

Conflict resolution: Project > User. Reforge skills live in their own scoped namespace.

### SKILL.md frontmatter

Unchanged: Anthropic Agent Skills spec verbatim plus klyntbot-additive fields (`tags`, `sensitivity`, `references[].load`, `paths`).

### Conditional activation by `paths:`

When the agent calls a tool that touches a file, `SkillActivator` matches the path against every conditional skill's glob set. Matched skills move to the session's active set; their frontmatter summary injects into the system prompt next turn.

### Dynamic discovery on file touch

When a tool reads/edits a file deep in a repo, walk from its directory up to CWD checking each level for `.klyntbot/skills/` directories not in the index. Newly-found directories loaded on the spot. Gitignored directories skipped.

### Progressive loading

Frontmatter-only at discovery time; full body loaded via `skill_reference(skill_id, ref_name)` tool when needed.

### Skill management — slash commands

```
/skills list                       # list all installed skills with source + version
/skills info <name>                # show frontmatter, source, last-activated, references
/skills install <source>           # add a new skill
/skills update <name>              # re-fetch from origin if source supports it
/skills uninstall <name>           # remove from ~/.klyntbot/skills/
/skills validate <name>            # check SKILL.md syntax + allowed-tools
/skills toggle <name> --on|--off   # enable/disable without uninstalling
/skills reload                     # re-walk discovery without restart
```

Each is a **direct** slash command (§9): handled by a Tauri command that returns a result rendered as an inline system bubble. No agent involvement.

### `/skills install` source types

Phase 1:
- `/skills install ./local/path` — copies the directory.
- `/skills install ~/.claude/skills/foo` — manual bridge.
- `/skills install https://github.com/user/repo[/path]` — clones; validates SKILL.md.
- `/skills install https://gist.github.com/...` — same as github but for gists.

Install command shows SKILL.md content + allowed-tools + (optional) install-script preview, asks for confirmation (an inline `kind: "userInput"` row), then writes.

### Settings page parity

The Settings → Coding → Skills tab gives a browseable list with the same install / update / uninstall / toggle actions as a clickable surface. Anything you can type, you can also click. Anything you can click, you can also type.

### Configuration shape

```json
{
  "coding": {
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

## 9. Slash command system

### Why this is a new section

In the superseded TUI spec, slash commands were a TUI affordance (bottom-pane modal forms) and CLI subcommands were a separate surface (executable name + flags). This spec collapses both into one universal command surface: the chat composer.

### Two execution paths

| Path | When | Latency | Streaming | Side-effects |
|---|---|---|---|---|
| **Agent-routed** | command modifies the agent's behavior or asks the agent to do work | full agent loop (seconds) | yes — agent streams response | yes; subject to approvals |
| **Direct** | command queries or mutates state without an LLM call | sub-second | no — single result bubble | yes; not subject to approvals (the user's click is the consent) |

### Agent-routed commands

Translated to a system instruction prepended to the user message; the agent loop runs normally:

| Command | Effect |
|---|---|
| `/plan` | Sets approval mode = `plan` for this thread (writes/exec denied); agent told to research-only. |
| `/yolo` | Sets approval mode = `bypass` (requires env var). Pulses a banner in the header. |
| `/power on` / `/power off` | Toggles tool profile. Tool list rebuilt for next iteration. |
| `/recall <query>` | Forces a recall pass with the user's query as the seed; agent receives the snippets and can decide what to do with them. |
| `/dead-ends` | Asks Mirror for the current dead-end summary in this repo and surfaces it. |
| `/mirror` | Emits the agent's recent Mirror alerts inline. |

These commands typically produce a normal assistant response; they don't bypass the agent.

### Direct commands

Handled by a Tauri command + AppCore handler; result is rendered as a synthetic assistant bubble (or a `kind: "system"` variant if we add one):

| Command | Tauri command | Effect |
|---|---|---|
| `/skills list` | `coding_skills_list` | Returns installed skills as a list. |
| `/skills info <name>` | `coding_skills_info` | Returns skill frontmatter + last-activated. |
| `/skills install <src>` | `coding_skills_install` | Installs (with confirmation step). |
| `/skills update <name>` | `coding_skills_update` | Re-fetches from origin. |
| `/skills uninstall <name>` | `coding_skills_uninstall` | Deletes from local skills/. |
| `/skills toggle <name>` | `coding_skills_toggle` | Enables / disables without uninstalling. |
| `/status` | `coding_status` | Returns mode, profile, sandbox state, total cost, total tokens, active skills. |
| `/doctor` | `coding_doctor` | Runs diagnostic; returns checklist of green/red items. |
| `/sessions star` / `/sessions unstar` | `coding_sessions_star` | Marks current thread starred. |
| `/sessions export [--format md|json]` | `coding_sessions_export` | Writes export file; returns path. |
| `/resume <prefix>` | `coding_resume` | Switches the current view to the matching thread. |
| `/permissions clear-mirror <tool>` | `coding_permissions_clear_mirror` | Resets Mirror-learned cache for a tool. |

### Dispatcher

```ts
// desktop-ui/src/features/coding/hooks/useSlashCommands.ts
export function useSlashCommands() {
  return {
    catalog,                                 // for the autocomplete popover
    classify(input: string): "agent" | "direct" | null,
    async dispatch(input: string, sessionKey: string): DispatchResult,
  };
}
```

`catalog` is loaded from a static `slash/registry.ts` (with descriptions, arg hints, and category tags so the popover can group them).

**`classify` algorithm** (deterministic; locked by invariant K9):

1. Reject if the input doesn't start with `/`. Return `null` (the message goes to the agent unchanged).
2. Take the first whitespace-delimited token (e.g., `/skills` from `/skills install ./foo`). Strip the leading `/`.
3. Walk the catalog as a tree keyed on the first token; for the matched node, peek at the next whitespace-delimited token to traverse subcommands (e.g., `/skills` → `install`). Stop at the deepest match.
4. If the deepest match is a leaf with `path: "agent"`, return `"agent"`. If `path: "direct"`, return `"direct"`. If no match (e.g., `/sk` partial, or `/foo` unknown), return `null` and let the agent see the raw input (so the user can ask "what's `/foo`?" and the agent answers).
5. Tie-breaking: when both a direct subcommand and an agent-routed alias of the same name exist, direct wins (so `/skills` → catalog → direct, never accidentally hits the agent).

`dispatch`:

- For **agent-routed**: returns `{ kind: "passthrough", text: <transformed input> }` and `useChatSession.send` posts as a normal message. Transformation may prepend a system instruction (e.g., `/plan` becomes `[system: enter plan mode] <empty user message>`).
- For **direct**: invokes the Tauri command, captures the result, returns `{ kind: "render", item: ConversationItem }` which `useChatSession` appends to the session's `segments` (no backend round-trip; no agent loop).

### UX details

- **Autocomplete**: typing `/` at the start of the input (or after whitespace/punctuation; per `useComposerAutocomplete.ts::triggerPrefixRegex`) opens the suggestions popover with categories: Mode, Skills, Status, Sessions, Permissions, Recall.
- **Escape — sending a literal `/`**: slash-command interception only fires when `/` is the **first non-whitespace character** of the entire input. To ask the agent about a path like `/etc/passwd`, prefix with any non-slash character (`"What's in /etc/passwd?"`, with the leading `"`, works) or with whitespace. There is no backslash-escape; the heuristic is "first character must be `/` AND the input must match a known command prefix from the catalog." If the parser doesn't find a matching command, it falls through to the agent unchanged.
- **Confirmation for risky direct commands**: `/skills install` and `/permissions clear-mirror` render a `kind: "userInput"` row first ("Install skill <name> from <url>?") before executing.
- **Help**: `/help` lists all commands. `/help <cmd>` shows detailed help (sourced from the catalog metadata).
- **Discoverability**: a "Slash commands" link in the empty-state of new coding threads.

### Configuration shape

```json
{
  "coding": {
    "slashCommands": {
      "enabledCategories": ["mode", "skills", "status", "sessions", "permissions", "recall"],
      "showSuggestionsOnSlash": true
    }
  }
}
```

---

## 10. Event vocabulary: AgentEvent extensions

### Two enums, one event story

| Enum | Crate | Role |
|---|---|---|
| `agent::events::AgentEvent` | existing `agent` crate | Runtime streaming events; consumed by the Tauri event emitter, the Distiller, the Mirror signal sources |
| `coding-ingest::AgentEvent` | `coding-memory` (companion spec) | Cross-source normalized ingest events |

A `MemorySinkSubscriber` translator maps runtime events into ingest events as they flow.

### New variants on `agent::events::AgentEvent`

Same additive philosophy as the superseded spec, minus Wire-related variants. All under `#[non_exhaustive]`:

```rust
// Recall + skill telemetry
RecallInjected { memory_ids, coverage_score, escalation_chain, dead_end_warning, budget_used_tokens, budget_limit_tokens },
DeadEndWarningSurfaced { approach_summary, prior_attempt_id, confidence },
SkillActivationConsidered { skill_id, score, threshold, accepted, decision_reason },
SkillActivated { skill_id, source_path, trigger, injected_tokens },
SkillReferenceLoaded { skill_id, reference, tokens, load_kind },
ContextEngineDecision { included, excluded, total_tokens, budget_used_pct },

// Approval + sandbox
ApprovalRequested { request_id, tool, args_hash, layer, rule_matched, mirror_history, sandbox_summary, requires_user_input },
ApprovalResolved { request_id, decision, decision_reason, latency_ms, persisted_rule, decided_by /* user | auto_allow | auto_deny | timeout | cancelled */ },
SandboxPolicyApplied { tool, policy_summary, policy_hash, fallback_unsandboxed, fs_constraints, network_constraints },

// Tool + provider telemetry
ToolCallStreamChunk { tool, chunk_kind, bytes, truncated },
MCPSubcallTrace { server, tool, latency_ms, bytes_returned, error },
ProviderRequest { iteration, model, prompt_tokens, max_tokens, attempt },
ProviderResponse { latency_ms, usage, cost_usd, retries_used, finish_reason },
MidLoopCompressionTriggered { before_tokens, after_tokens, messages_condensed, regions },

// Coding-specific
FileEditWithSymbols { path, op, bytes, diff_full, anchored_symbols, lsp_diagnostics_delta },
TestRunDetailed { command, framework, passed_tests, failed_tests, newly_passing, newly_failing, coverage_delta, duration_ms },
PowerModeToggled { previous, current, eager_tool_count, deferred_tool_count },
TurnInterrupted { reason, partial_tools, iterations_completed },
```

**Phase-1 stub fields:** Two of these fields ride along but aren't fully populated in Phase 1, because their data sources require separate integrations:

- `FileEditWithSymbols.lsp_diagnostics_delta` — empty `Vec<LspDiagnostic>` in Phase 1. Phase 2+ adds an LSP client that runs against the modified file and emits the delta.
- `FileEditWithSymbols.anchored_symbols` — best-effort tree-sitter pass in Phase 1 (matches the existing `crates/agent`/coding-memory anchor behavior); LSP-grade resolution waits for Phase 2+.
- `TestRunDetailed.coverage_delta` — `None` in Phase 1 unless the test command emits coverage in a recognized format. Phase 2+ wires per-framework parsers.

The fields are present in the variant from day one so consumers (Distiller, Mirror) can subscribe to a stable shape.

**Removed from the superseded spec:** `KlyntSessionStart` / `KlyntSessionEnd` (subsumed by chat-thread lifecycle events; existing chat already emits `chat:thread_created`/`chat:thread_updated`), `MirrorAlertSurfaced` (already handled by existing notifications surface), `CostThresholdCrossed` (handled by `ProviderResponse` consumers; emit on threshold via Mirror signal instead).

### New variants on `coding-ingest::AgentEvent`

Same 10 net-new variants as the superseded spec; coordinated extension to coding-memory (see §12):

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

A subscriber on the runtime event stream translates and forwards. Aggregation patterns:

- `ContentChunk` accumulates into per-iteration `AssistantMsg` ingest event.
- `ToolStart` + `ToolEnd` pair into a single `ToolCall` ingest event with timing.
- `ApprovalRequested` + `ApprovalResolved` pair into a single `ApprovalDecision` ingest event.
- `FileEditWithSymbols` → `FileEditEnriched`.
- `RecallInjected` → `RecallInjected` (1:1).
- `ProviderResponse` → `ProviderCall`.
- Some events are runtime-only (UI consumes, no ingest equivalent): `IterationStart`, `ToolCallStreamChunk`, `PowerModeToggled`.

### Property tests (carries forward from superseded spec)

| # | Invariant |
|---|---|
| E1 | Every `FileEditWithSymbols` runtime event produces exactly one `FileEditEnriched` ingest event |
| E2 | Every `ProviderResponse` produces exactly one `ProviderCall` with matching `cost_usd` and `latency_ms` |
| E3 | `ContentChunk` stream of N chunks aggregates to exactly one `AssistantMsg` ingest event with concatenated text |
| E4 | `ToolStart` + `ToolEnd` for same tool produces exactly one `ToolCall` ingest event; orphan `ToolStart` → no ingest emit |
| E5 | Translator monotone: ingest emit count grows monotonically; never retroactively cancels prior emits |

### Extensibility patterns

`#[non_exhaustive]`, tuple variants wrapping struct types, `EventExtensions` typed escape hatch, versioned envelope — same as the superseded spec.

### Tauri-event channel additions

The existing `agent:*` event channel emits to the desktop-ui. Coding-specific channels that the React layer needs:

- `agent:approval_requested` — fires **only** when the corresponding `ApprovalRequested` runtime variant has `requires_user_input == true`. Frontend renders an `ApprovalCard`. Auto-allow / auto-deny evaluations skip this Tauri channel (they still hit `DomainEventBus` for telemetry).
- `agent:approval_resolved` — fires only for the auto-allow/deny case's resolutions that were paired with a `requires_user_input == true` request, OR explicitly when an awaiting card is resolved (user / timeout / cancelled). Frontend collapses the card to its decided state.
- `agent:file_edit_with_symbols` — frontend renders a `kind: "diff"` row.
- `agent:recall_injected` — frontend renders a recall-tray card.
- `agent:dead_end_warning_surfaced` — frontend renders an inline warning row.
- `agent:sandbox_policy_applied` — frontend updates the sandbox-status pill in the composer meta bar.

All additive. Existing chat listeners ignore them via the catch-all in chatStreamStore. The asymmetry — `ApprovalRequested` runtime events are emitted for every gate evaluation, but the `agent:approval_requested` Tauri channel is gated on `requires_user_input` — keeps the React store free of cards-that-never-need-rendering while still giving the Distiller and Mirror full-fidelity input via the runtime path.

---

## 11. Session model

### Chat thread = coding session

Each row in the existing `chat_sessions` table is a session. The original spec's `klynt_sessions` table is **not** added.

### Schema changes

`chat_sessions` gains:

```sql
ALTER TABLE chat_sessions ADD COLUMN mode TEXT NOT NULL DEFAULT 'chat';   -- 'chat' | 'coding'
ALTER TABLE chat_sessions ADD COLUMN cwd TEXT;
ALTER TABLE chat_sessions ADD COLUMN repo_id TEXT;
ALTER TABLE chat_sessions ADD COLUMN repo_branch TEXT;
ALTER TABLE chat_sessions ADD COLUMN tool_profile TEXT;                   -- 'minimal' | 'curated' | 'power'
ALTER TABLE chat_sessions ADD COLUMN approval_mode TEXT NOT NULL DEFAULT 'default';  -- 'default' | 'plan' | 'bypass'
ALTER TABLE chat_sessions ADD COLUMN total_cost_usd REAL NOT NULL DEFAULT 0;
ALTER TABLE chat_sessions ADD COLUMN total_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chat_sessions ADD COLUMN starred BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE chat_sessions ADD COLUMN parent_session_id TEXT;              -- if forked from another thread
```

Per CLAUDE.md pre-release policy, this is consolidated into a single migration; no incremental migration files.

**Type conventions:** Match the existing `chat_sessions` table's conventions (verified during writing-plans). SQLite has no native BOOLEAN — the `starred BOOLEAN` declaration above is conventional but stored as INTEGER (0 / 1) in practice. The `parent_session_id TEXT` column is **not** declared with a `REFERENCES chat_sessions(id)` foreign key in Phase 1; forks may outlive their parents (parent deleted manually) and there's no need for cascade behavior. Phase 2+ may add an FK with `ON DELETE SET NULL` if introspection patterns warrant it.

### Resume

"Resume" = "click thread in sidebar" (existing UX) **or** `/resume <prefix>` (slash command, fuzzy-matches against thread title).

The agent loop never replays prior tool calls. On thread switch, the chat surface re-fetches `chat_messages`; the agent receives the persisted history at the next user turn (existing behavior).

**Skill activation on resume:** Path-conditional skill activations are session-scoped and not persisted. On resume, the `SkillActivator` re-runs by replaying the file paths referenced in the persisted history (extracted from `kind: "tool"` and `kind: "diff"` rows). This is deterministic given the same persisted history (invariant K6 — see §14). Performance: scales with the number of unique paths × number of conditional skills; cached per-thread after first run.

### `cwd` updates

`chat_sessions.cwd` is set at thread creation (auto-detected from workspace context, or `null` for general chat threads). Bash `cd` calls inside the sandbox are **process-local** to that bash invocation — they do not update the persisted column, and they do not carry across tool calls (each `bash` invocation starts a new sandboxed process). To change the persisted cwd of a thread, the user invokes `/sessions cd <path>` (Phase 2 slash command) or recreates the thread from a different workspace context.

### Forking

A new slash command `/sessions fork` creates a new thread with `parent_session_id = <current>`, copies the message history up to the cursor (or up to a chosen user message — Phase 2), and switches to the new thread. Useful for experimenting without losing the original.

### Exporting

`/sessions export [--format md|json]` writes the thread to `~/.klyntbot/exports/<session-id>.<ext>` and returns the path inline. Phase 1 ships md + json; HTML is deferred.

### Retention

```json
{
  "coding": {
    "sessions": {
      "retentionDays": 90,
      "maxTotalDiskMb": 5000,
      "preserveStarred": true
    }
  }
}
```

A nightly cron job (registered via the existing `app-core/src/init/cron.rs`) prunes sessions older than `retentionDays` whose `starred = false`. Disk-budget pruning happens on-demand when the threshold is crossed.

### Multi-process safety

Out of scope: the desktop is the only process. SQLite WAL mode remains in place from the existing setup so concurrent reads (e.g., from the embedded MCP server) don't conflict with the desktop's writes.

### Snapshots / rewind

**Deferred to Phase 2.** When implemented, snapshots live in a side table or in the existing data dir; the rewind UI is a per-message "rewind to here" affordance in `MessageRows.tsx`.

---

## 12. Coordination with coding-memory spec

### Why this section exists

The coding-memory spec defined the original `klynt-cli` as a non-goal; its amendments listed in the superseded klynt-cli spec §14 still mostly apply. This section restates them for the chat-based architecture and identifies what shrinks.

### Spec amendments to `2026-04-22-coding-memory-design.md`

#### Category A — Additive event vocabulary (unchanged)

- §5 *AgentEvent (the core contract)* — add the 10 new variants from §10 of this spec.
- §5 *CLI adapter mappings* table — add row for "klyntbot desktop coding mode" with adapter type "Native (in-process emit)".
- §3 *Key decisions* table — add: "klyntbot desktop coding mode is a first-class source emitting the rich variant set".

#### Category B — Distiller behavior on rich variants (unchanged)

The table in the superseded spec §14 carries forward verbatim. `RecallInjected`, `ApprovalDecision`, `SandboxApplied`, `FileEditEnriched`, `TestRunEnriched`, `ProviderCall`, `CompressionApplied`, `MirrorAlert`, `SkillRoutingTrace`, `SkillActivated` — all extractive (no Phase B LLM pass).

#### Category C — Schema delta (shrunk)

The `klynt_sessions` table is **not** added. Instead, `chat_sessions` gains the columns listed in §11. Coding-memory spec §4 *Schema deltas* adds a row referencing this spec's §11.

#### Category D — Native source — chat coding mode (replaces "klynt-cli native source")

Coding-memory spec §5 gains:

> Klyntbot desktop chat in coding mode emits `AgentEvent` directly via in-process function calls to `Distiller::accept_event(...)`. There is no `MemorySink` trait abstraction needed in this spec (unlike the superseded klynt-cli design): the desktop process *is* the runtime, so emission is always in-process. External CLI ingest adapters remain as designed in coding-memory; they are out of scope for this spec.

#### Category E — Per-skill scope amendments

No change to coding-memory's existing per-skill scope amendments (`skill_versions.scope` + `scope_repo_id` columns and project-skill paths from coding-memory §9). The chat-based design uses these unchanged: `~/.klyntbot/project-skills/<repo-id>/` is still the Reforge-synthesized scope, and the same `scope` enum (`user`, `project`, `reforge_private`, `reforge_team`) governs visibility.

#### Category F — Configuration namespace coordination

- Coding-memory keeps `codingMemory.*`.
- This spec's keys live under `coding.*` (renamed from the superseded spec's `codingCli.*`).
- Klyntbot desktop chat consumes some coding-memory config:
  - `codingMemory.recall.sessionStartBudget` — used by recall injection.
  - `codingMemory.ingest.excludePaths` — used by the privacy guard.
  - `codingMemory.privacy.defaultSensitivity` — used by sensitivity tagging.
  - `codingMemory.distiller.model` — desktop respects this.
- Coding-memory spec §13.D — add comment on each: *"Used by klyntbot desktop coding mode — do not rename without coordinating both specs."*

### Cross-spec PR shape

```
docs(specs): amend coding-memory for klyntbot desktop coding-mode source

- §5 add 10 rich AgentEvent variants
- §5 add desktop coding-mode adapter row + Native source subsection
- §6 add rich-variant Distiller handling table
- §4 add chat_sessions column delta reference
- §13.D mark shared config keys
```

Single PR, both specs ship coherent.

### Implementation ordering

**Order A (recommended):** coding-memory implementation first, then this spec's Phase 1.

Same as the superseded spec's recommendation, for the same reason: ship a fully-working first release rather than partially stubbed.

### Shared invariants

Coding-memory spec's 9 invariants + this spec's 11 (next section) = **20 invariants**, down from 21 in the superseded spec (lost K12: Wire observer non-interference).

---

## 13. Phased buildout

### Phase 1 — Walking skeleton (target: 3 weeks)

**Goal:** A user opens a chat thread, flips coding mode, types "list the files in this repo and refactor the parser", and the agent runs sandboxed `bash`/`read`/`edit` calls with approval cards rendering inline. Recall injects from coding-memory. Skills activate by file path. Distiller writes coding-memory rows.

**Deliverables:**

- 7 new crates land in `bot/crates/` (`klynt-protocol`, `klynt-execpolicy`, `klynt-sandbox`, `klynt-sandbox-helper`, `klynt-hooks`, `klynt-skill-loader`, `klynt-core`).
- Codex adaptation: `scripts/adapt_codex_vendor.sh` adapts 5 Codex crates with full rename pass (drop `tui/` and `protocol/`'s wire types).
- All `agent::events::AgentEvent` extensions added under `#[non_exhaustive]`; chat-channel match arms audited for `_ =>` catch-all.
- `Tool::is_concurrency_safe()` added to `tools-core`.
- `crates/agent/src/execution/core.rs` extended with read-only-aware partitioning.
- Coding tool kit: `bash`, `read`, `glob`, `grep`, `edit`, `write`, `apply_patch`, `ask_user`, `enter_plan_mode`/`exit_plan_mode`, `notebook_edit`, `web_fetch`.
- Curated default tool profile (24 eager tools); `tool_search` registered as no-op stub.
- Three-layer approval architecture present (Layer 3 deferred to Phase 2).
- macOS Seatbelt sandbox live; Linux Landlock + bwrap live.
- klyntbot's `skill-system` extended with `paths:` conditional + dynamic discovery.
- React surface in `desktop-ui/src/features/coding/`: `ApprovalCard`, `CodingModePill`, `DiffPreview`, `RecallTrayCard`, `useCodingMode`, `useSlashCommands`, `useApprovalQueue`.
- Slash command catalog wired (agent-routed: `/plan`, `/yolo`, `/power`, `/recall`; direct: `/skills *`, `/status`, `/sessions star/unstar`, `/resume`, `/help`).
- Tauri commands `coding_*` for direct slash-command dispatch.
- Mode toggle on composer; auto-detection from workspace context.
- All 10 new `chat_sessions` columns (per §11): `mode`, `cwd`, `repo_id`, `repo_branch`, `tool_profile`, `approval_mode`, `total_cost_usd`, `total_tokens`, `starred`, `parent_session_id`. Single consolidated migration per pre-release policy.
- Hook engine reads `~/.klyntbot/hooks.toml`; PreToolUse + PostToolUse fire correctly.
- Distiller subscriber + Mirror signal subscriber spawned at AppCore init; consume coding-channel events.
- Settings → Coding page (read-only or edit-then-save for declarative permissions, sandbox toggle, skill list).
- 9 of 11 K-invariants under proptest (K1-K4, K6-K9; K5 via integration test, K10-K11 deferred to Phase 2) plus all 5 translator invariants (E1-E5). See §14.

**Stubs / deferred:**
- Mirror-learned approval (Layer 3): config flag accepted; layer skipped.
- Snapshot/rewind: not captured.
- `tool_search`: no-op stub.
- `recall_*` tools: depend on coding-memory implementation reaching its Phase 4 (Recall API). If coding-memory is behind, register stub tools that return empty results; full recall lights up when coding-memory ships.
- Sessions export: deferred to Phase 2.

**Exit gates:**
- `cargo build --workspace` clean.
- `cargo clippy --workspace --all-targets --all-features` zero warnings.
- `cargo fmt --all --check` passes.
- `cargo nextest run --workspace` green.
- `bun run lint && bun run typecheck && bun run test` green in `desktop-ui/`.
- All 5 translator property tests green.
- One end-to-end scenario: open chat → flip to coding → "list files and refactor X" → approval card appears, user approves → tools execute → diff row renders → final assistant message.

### Phase 2 — Polish + opt-in features (target: 2 weeks)

- Mirror-learned approval (Layer 3) lit up; opt-in via `mirrorLearning: true`.
- File snapshots → `/sessions rewind` slash command.
- `tool_search` becomes real.
- `/sessions export` ships (md + json).
- `/sessions fork` ships.
- Slash commands: `/dead-ends`, `/mirror`, `/permissions clear-mirror`.
- Per-thread cost ceiling + `CostThresholdCrossed` Mirror alert.
- Settings page: hooks display + skill install-from-URL field.
- Performance pass: chat-send → first-token < 800ms p95 in coding mode.

### Phase 3+ — Ecosystem & advanced

- MCP-contributed skills.
- Per-channel MCP allowlists.
- Skills.sh marketplace integration.
- IDE bridge via MCP `klyntbot mcp serve --stdio` extensions (separate spec).
- Multi-window per-repo coding (Phase 3+ via `lazy_window.rs`).
- Voice-driven coding via the existing `useDictationController.ts` (already works in composer; verify in coding mode).
- Snapshots: content-addressed dedup.
- Windows sandbox.

### Coordination with coding-memory

| Order | Recommendation |
|---|---|
| **A (recommended)** | coding-memory Phase 1-5 → this spec's Phase 1 |
| B alternative | this spec Phase 1 with stubbed `recall_*` tools → coding-memory |

### Quality gates (every phase)

| Gate | Check |
|---|---|
| Compilation | `cargo build --workspace` |
| Lint | `cargo clippy --workspace --all-targets --all-features` zero warnings |
| Format | `cargo fmt --all --check` |
| Tests | `cargo nextest run --workspace` |
| Frontend | `cd desktop-ui && bun run lint && bun run typecheck && bun run test` |
| Doc coverage | `cargo rustdoc -- -D missing-docs` on new public items |
| Translator invariants (E1-E5) | proptest |

---

## 14. Testing, invariants, benchmarks

### Philosophy

```
              ┌─────────────────────┐
              │   Benchmarks (5)    │  perf regressions
              ├─────────────────────┤
              │  Scenarios (~6)     │  end-to-end stories
              ├─────────────────────┤
              │  Property (~10)     │  invariants
              ├─────────────────────┤
              │  Integration (~25)  │  cross-crate, real I/O
              ├─────────────────────┤
              │  Unit (~120+)       │  per-module, in-memory
              └─────────────────────┘
```

In-memory SQLite for everything below scenarios. Real filesystems via `tempfile::TempDir`. Provider responses mocked via fixtures. React unit tests via Vitest with mocked `invoke`.

### The 11 architectural invariants (proptests)

Renumbered from the superseded spec (lost K12 Wire-related):

| # | Invariant | Phase |
|---|---|---|
| K1 | Translator round-trip determinism | 1 |
| K2 | Translator monotonicity (ingest count never retroactively decreases) | 1 |
| K3 | Approval gate composition (single decision; highest-priority match wins) | 1 |
| K4 | Privacy guard inviolability (`/yolo` cannot bypass excludePaths) | 1 |
| K5 | Sandbox-fallback safety | 1 |
| K6 | Skill discovery determinism | 1 |
| K7 | Mode-toggle event ordering (mode flip triggers tool registry rebuild before next iteration) | 1 |
| K8 | Approval round-trip identity (request_id ↔ resolved decision is 1:1) | 1 |
| K9 | Slash command classification stability (a given input string always classifies as the same path) | 1 |
| K10 | Mirror-learned cache poisoning (single denial → always ask) | 2 |
| K11 | Sessions retention monotonicity (a starred session is never pruned) | 2 |

Plus coding-memory's 9 invariants from its §3 = **20 total invariants** across both specs.

### Per-phase test breakdown

#### Phase 1 (~140 tests)

**Unit (~110):**
- `klynt-execpolicy` Starlark tests (~25)
- `klynt-sandbox` policy generation (~15)
- `klynt-hooks` marshalling (~10)
- `klynt-skill-loader` discovery + activation (~15)
- `klynt-core` per-tool tests (~20)
- React: `useCodingMode`, `useSlashCommands`, `useApprovalQueue`, `ApprovalCard`, `CodingModePill`, slash dispatcher (~25)

**Integration (~20):**
- Hook → broker → MemorySink round trip
- Sandbox launch on macOS + Linux
- Approval gate full-stack (privacy → declarative → starlark → mode default)
- Skill install + activation
- Mode-toggle round trip (frontend → backend → next iteration's tool list)
- Slash command direct dispatch (Tauri command + result render)
- Approval card → user-decision → tool resumes
- Multi-thread isolation (one thread's approvals don't leak into another)
- Sandbox failure → tightened approvals
- Hook subprocess timeout fail-open

**Property (~9):** K1, K2, K3, K4, K6, K7, K8, K9 from the §14 invariants table — every K-invariant marked Phase 1 — plus the 5 translator invariants E1-E5 from §10. K5 (sandbox-fallback safety) is exercised by integration rather than property test because its inputs are OS-state, not generated values.

**Scenario (~5):**
- Full coding session: open → coding mode → list files → refactor → tests pass.
- Approval flow: bash with no rule → ask → user approves → command runs.
- Skill activation: edit a file matching a `paths:` glob → skill activates next turn.
- Slash command: `/skills install ./fixtures/sample-skill` → install → `/skills list` shows it.
- Mode flip: thread starts in chat mode → user types `/coding` (or flips pill) → next message routes through coding tool registry.

#### Phase 2 (~50 tests added)

- Mirror-learned approval (~10 unit, K10 property, 1 scenario).
- Sessions rewind (~8 unit, K11 property, 1 scenario).
- `tool_search` (~6 unit).
- `/sessions export` (~5 unit, 1 integration).
- Slash commands `/dead-ends`, `/mirror`, `/permissions clear-mirror` (~10 unit).
- Cost-ceiling alert (~5 unit, 1 integration).

### Benchmark targets

```
bench_translator_throughput           > 50K events/sec sustained
bench_chat_send_to_first_token_p95    < 800ms in coding mode (warm cache)
bench_sandbox_launch_macos            < 50ms (cached policy hit)
bench_sandbox_launch_linux            < 80ms (Landlock + bwrap warm)
bench_skill_discovery_50_skills       < 30ms full walk + parse
bench_approval_gate_full_stack        < 1ms per call
```

Scenario benchmarks:
```
bench_typical_turn_4_tools            single turn with 4 tool calls; < 6s wall clock
bench_recall_injection_p95            recall_index 10 results; < 200ms
```

### Fixtures

```
tests/fixtures/coding/
  synthetic_session_simple.jsonl
  synthetic_session_error_recovery.jsonl
  synthetic_session_long.jsonl
  provider_responses/
    claude_simple_text.json
    claude_with_tools.json
    claude_with_thinking.json
  sandboxed_commands.json
  sample_skills/
    basic-skill/SKILL.md
    conditional-skill/SKILL.md
    invalid-skill/SKILL.md
  sample_repos/
    rust_workspace.tar
    python_project.tar
```

### Test isolation

- Each test gets unique `TempDir` for `~/.klyntbot/` overlay.
- `KLYNTBOT_HOME` env var honored (per CLAUDE.md).
- No test mutates user's actual `~/.klyntbot/`.
- Network access denied via `KLYNTBOT_TEST_NO_NETWORK=1`.
- Platform-gated tests: `#[cfg(target_os = "macos")]` for Seatbelt; `#[cfg(target_os = "linux")]` for Landlock.
- LLM provider via `_scripted_echo` (existing klyntbot pattern).

### React test harness

- Mock `invoke` per-test (existing pattern in `desktop-ui/__mocks__/`).
- Mock Tauri events via a synthetic event bus exposed by `chatStreamStore` for testing.
- Snapshot tests for `ApprovalCard`, `DiffPreview`, `RecallTrayCard`, `CodingModePill`.
- Interaction tests for the slash dispatcher: type → suggest → enter → render.

### Negative-path testing

- Try to write `~/.env` with privacy guard active → asserts `Blocked` even with `/yolo`.
- Try to install skill with malformed frontmatter → asserts install refuses.
- Sandbox unavailable → asserts approval tightening + banner.
- Slash command with unknown name → asserts inline error message, no agent call.
- Approval card timeout → tool errors out cleanly; agent continues.

### Coverage targets (informational)

| Crate | Target line coverage |
|---|---|
| `klynt-protocol` | 100% |
| `klynt-execpolicy` | 90% |
| `klynt-sandbox` | 70% |
| `klynt-hooks` | 85% |
| `klynt-skill-loader` | 90% |
| `klynt-core` | 85% |
| React `features/coding/` | 80% |

---

## Appendix A — Locked design decisions

| # | Axis | Decision |
|---|---|---|
| 1 | Surface | Klyntbot desktop chat — no separate binary, no TUI. |
| 2 | Process model | Single Tauri process. No multi-process coordination. |
| 3 | Coding mode | Per-thread toggle on the composer; auto-detect from workspace context; persisted in `chat_sessions.mode`. |
| 4 | Command surface | Composer slash commands. Two paths: agent-routed and direct. |
| 5 | Tool surface | Curated default of 24 tools when in coding mode; `/power on` expands. |
| 6 | Approval | Three layers (declarative + Starlark + Mirror-learned); chat-inline approval cards as `kind: "approval"` ConversationItem. |
| 7 | Sandbox | OS-level (Seatbelt / Landlock + bwrap); Windows deferred. |
| 8 | Skill system | `.klyntbot/skills/` only; `/skills install` for external sources; Settings tab parity. |
| 9 | Sessions | Chat thread = coding session; columns added to `chat_sessions`; no separate `klynt_sessions` table. |
| 10 | Wire | Deleted. Future IDE bridge designed separately. |
| 11 | Event richness | `agent::events::AgentEvent` gains 18 additive variants under `#[non_exhaustive]`; `coding-ingest::AgentEvent` gains 10. |
| 12 | Distribution | Bundled with desktop installer. |

---

## Appendix B — New crate inventory

```
bot/crates/
├── klynt-core            # Coding tool registry; sandbox/approval glue; slash dispatch
├── klynt-protocol        # Slim event/op types (no wire)
├── klynt-execpolicy      # Starlark prefix-rule approval engine (adapted)
├── klynt-sandbox         # Seatbelt + Landlock + bwrap policy construction (adapted)
├── klynt-sandbox-helper  # Linux child binary (renamed)
├── klynt-hooks           # Hook engine (adapted, retargeted)
└── klynt-skill-loader    # Multi-source skill discovery, conditional activation (fresh)
```

Plus surgical edits:
- `tools-core::Tool` — add `is_concurrency_safe(args) -> bool` (default `false`).
- `crates/agent/src/execution/core.rs` — read-only-aware partitioning.
- `crates/agent/src/events.rs` — 18 new variants under `#[non_exhaustive]`.
- `crates/common/src/types.rs` — add `pub const CODING_CHANNEL: &str = "coding";` alongside `SYSTEM_CHANNEL` / `CLI_CHANNEL` / `MCP_CHANNEL`.
- `crates/agent/src/agent_runtime/runtime.rs` — `AgentRuntime` constructor accepts `Arc<DomainEventBus>` for the cognitive-ingest path (per CLAUDE.md the bus is already passed as `Arc`).
- `chat_sessions` schema columns (mode, cwd, repo_id, repo_branch, tool_profile, approval_mode, total_cost_usd, total_tokens, starred, parent_session_id).
- Chat channel match-arm audit for `_ =>` catch-all.
- `desktop-ui/src/features/coding/` (React; new feature directory).
- `crates/desktop/src/commands/coding.rs` (Tauri commands for slash direct-dispatch).
- `crates/desktop/src/commands/chat.rs` extended with `mode` field on the `chat_send` payload, plus a new `chat_set_mode` command for mid-thread flips and a new `chat_respond_approval` command for approval-card resolution.
- `crates/app-core/src/coding/` (handlers).
- Composer slash trigger registration: pass an additional `AutocompleteTrigger { trigger: "/", source: <slash command catalog> }` into `useComposerAutocomplete` when `mode == "coding"`. No edits to `Composer.tsx` or `ComposerSuggestionsPopover.tsx` themselves.

---

## Appendix C — Shared invariants (20 total)

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

### From this spec §14 (11)

10. K1 — Translator round-trip determinism
11. K2 — Translator monotonicity
12. K3 — Approval gate composition
13. K4 — Privacy guard inviolability
14. K5 — Sandbox-fallback safety
15. K6 — Skill discovery determinism
16. K7 — Mode-toggle event ordering
17. K8 — Approval round-trip identity
18. K9 — Slash command classification stability
19. K10 — Mirror-learned cache poisoning (Phase 2)
20. K11 — Sessions retention monotonicity (Phase 2)

All enforced via `proptest!` in `tests/coding_in_chat_property.rs` and `tests/coding_memory_property.rs`.

---

## Appendix D — Configuration shape (full)

Added to `~/.klyntbot/config.json`:

```json
{
  "coding": {
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
      "allow": ["Read(*)", "Glob(*)", "Grep(*)", "Bash(git status*)"],
      "deny": ["Bash(rm -rf /*)", "Bash(sudo *)"],
      "ask": ["Bash(*)", "WebFetch(*)"],
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

    "sessions": {
      "retentionDays": 90,
      "maxTotalDiskMb": 5000,
      "preserveStarred": true
    },

    "slashCommands": {
      "enabledCategories": ["mode", "skills", "status", "sessions", "permissions", "recall"],
      "showSuggestionsOnSlash": true
    },

    "modeAutoDetect": {
      "fromWorkspaceContext": true,
      "defaultModeForNewThreads": "chat"
    },

    "sandbox": {
      "enforce": true,
      "allowUnsandboxedFallback": false
    }
  }
}
```

All flags default to safe values; user enables advanced features per-feature.

---

## Appendix E — Cross-spec amendment list

For the single-PR coordination per §12:

### `docs/superpowers/specs/2026-04-22-coding-memory-design.md` amendments

1. **§3** Add row to *Key decisions* table: "klyntbot desktop coding mode is a first-class source emitting the rich variant set; external CLIs emit a subset"
2. **§4** Add row to *Schema deltas* table: `chat_sessions` columns (per this spec §11) — `mode`, `cwd`, `repo_id`, `repo_branch`, `tool_profile`, `approval_mode`, `total_cost_usd`, `total_tokens`, `starred`, `parent_session_id`.
3. **§5** Add 10 new variants to `AgentEventV1::EventKind` (per this spec §10).
4. **§5** Add row to *CLI adapter mappings*: "klyntbot desktop coding mode" (Native, in-process emit, all rich events).
5. **§5** Add new subsection "Native source: klyntbot desktop coding mode" describing in-process emission (no `MemorySink` abstraction needed; single process).
6. **§6** Add subsection "Rich-variant handling" with the table from this spec §12 Category B.
7. **§13.D** Mark shared config keys with comment "Used by klyntbot desktop coding mode — do not rename without coordinating both specs."
8. **§13.G** Update the "klynt-cli native coding CLI" row: change to "Klyntbot desktop coding mode; spec at docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md; supersedes the previous klynt-cli plan."
9. **§13.H Amendment log** — add row: "2026-04-29 | Amendment 3: chat-based coding mode supersedes klynt-cli; rich variants flow into Distiller from desktop in-process".

### `docs/superpowers/specs/2026-04-23-klynt-cli-design.md` (the superseded spec)

Already marked superseded. No further edits required.

### This spec

Created. Amendments to this spec follow the same single-PR pattern when needed.

---

*End of design.*
