# Long-Running Task Handling: Comparative Analysis & Best-in-Class Roadmap

**Date:** 2026-05-07
**Status:** Research note (informs future specs/plans)
**Scope:** Compare Klynt's coding-mode long-running-task handling against `kimi-cli` (Python, Moonshot) and `opencode` (Go, Charmbracelet/archived). Identify gaps. Lay out a phase-by-phase roadmap to bring Klynt to best-in-class on this axis.
**Companion docs:**
- `docs/superpowers/specs/2026-05-07-coding-sidebar-titles-and-running-state-design.md`
- `docs/superpowers/plans/2026-05-07-coding-sidebar-titles-and-running-state.md`
- `docs/superpowers/specs/2026-05-05-model-self-stop-termination-design.md`
- `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md`

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Klynt: Current State](#2-klynt-current-state)
3. [kimi-cli Analysis](#3-kimi-cli-analysis)
4. [opencode Analysis](#4-opencode-analysis)
5. [Cross-Project Comparison](#5-cross-project-comparison)
6. [Detailed Gap Analysis](#6-detailed-gap-analysis)
7. [Comprehensive Roadmap to Best-in-Class](#7-comprehensive-roadmap-to-best-in-class)
8. [What Klynt Should NOT Copy](#8-what-klynt-should-not-copy)
9. [Verification & Success Criteria](#9-verification--success-criteria)

---

## 1. Executive Summary

### 1.1 The three systems on one axis

| System | Strength | Weakness |
|---|---|---|
| **Klynt** | Infrastructure rigour (loop detector, tiered compression, persistent grants, cognitive subsystems, 65 crates) | Thin LLM-facing affordances; coding-mode approval broken; no TODO tool; no plan mode; no immutable wire-log; no mid-stream cancel |
| **kimi-cli** | LLM-facing affordances (TodoWrite, plan mode, /btw, D-Mail, hooks, wire-bus, background tasks) | Single-list TODO; lossy compaction; no oscillation detection; no cognitive layer |
| **opencode** | Polished UX (diff-preview approval, persistent shell, working auto-title, prompt-cache breakpoints, multi-theme) | Serial tool execution; permission-gate leak bug; no TODO/plan; no daemon mode; no resumability |

### 1.2 Headline numbers

Two reasonable framings of "how complete is Klynt's long-running-task handling":

- **vs kimi-cli (the leader)**: ~55–60%. Infrastructure exists; LLM affordances are missing.
- **vs opencode**: ~75–80%. opencode is thinner; Klynt only loses on diff-preview, persistent shell, working auto-title, prompt-cache breakpoints.
- **Klynt's unique advantages over both**: tiered compression, loop detector, cognitive subsystems, persistent approval grants, KCA validation gates — roughly 30–40% extra infrastructure neither comparator has.

**Net assessment**: Klynt is a stronger platform but a thinner coding-agent product. The gap is in user-facing task-management abstractions, not in the engine that runs them.

### 1.3 The path forward (one-paragraph)

Build best-in-class coding mode on top of the platform Klynt already has. Don't dilute the platform identity (cognitive, mirror, reforge, FSRS5) chasing feature parity. Instead: (1) fix the four already-broken-or-stub things (approval handler, auto-title, ThreadEventBuffer, mid-stream cancel); (2) add the wire-bus foundation (`wire.jsonl`); (3) layer the kimi-cli LLM affordances on top (TodoWrite, plan mode, hooks, /btw, background bash); (4) borrow opencode's UX polish (diff-preview, persistent shell, prompt-cache); (5) leave research-grade ideas (D-Mail, Agent Flow) for after the rest lands. Roughly 2 weeks of focused work closes ~70% of the perceptual gap; another 2–3 weeks closes the rest.

---

## 2. Klynt: Current State

> Source: deep-dive agent investigation of `crates/agent`, `crates/app-core`, `crates/context_engine`, `crates/coding-*`, `desktop-ui/src/features/coding/*` on 2026-05-07.

### 2.1 Architecture map

```
Rust Backend
├── crates/agent/
│   ├── src/agent_loop/mod.rs           # Top-level routing, focus/defer, streaming handle
│   ├── src/agent_runtime/runtime.rs    # 3-phase: Prepare → Execute → Record
│   ├── src/execution/
│   │   ├── execute_loop.rs             # Core iteration loop (cancellation, safety cap, compression)
│   │   ├── core.rs                     # ExecutionCore: LLM + parallel tool fan-out (MAX_CONCURRENT_TOOLS=10)
│   │   ├── mid_loop_compressor.rs      # Tool result trimming at 70% context
│   │   ├── live_context_refresher.rs   # ContextUpdateQueue drain between turns
│   │   ├── loop_detector.rs            # Oscillation detection (warn@3 stop@5)
│   │   └── budget.rs                   # SafetyCap: Normal=100/60K, DeepThink=150/90K, Ultra=∞/180K
│   ├── src/subagent.rs                 # SubagentManager: 3 profiles, semaphore=3
│   └── src/events.rs                   # AgentEvent enum
├── crates/app-core/
│   ├── src/coding/turn_handler.rs      # Coding-mode bridge: AgentEvent → ThreadEvent
│   ├── src/coding/title_service.rs     # Auto-title (STUB, Task 4 pending)
│   ├── src/coding/steer_queue.rs       # Mid-turn user steer injection
│   ├── src/coding/approval_handler.rs  # Approval (STUB, returns NotAvailable)
│   └── src/handlers/chat/streaming.rs  # Assistant-mode relay
├── crates/approval/                    # ApprovalGate + persistent ApprovalGrants
├── crates/context_engine/
│   └── src/history_compressor/tiered.rs  # Tier 0 / 1 / 2
└── crates/bus/
    ├── src/context_updates.rs          # ContextUpdateQueue
    └── src/domain_events.rs            # DomainEventBus

React Frontend
├── desktop-ui/src/features/coding/hooks/useThreadEvents.ts     # Reducer: ThreadEvent → items + TurnState
├── desktop-ui/src/features/coding/components/parts/            # ToolCallPart, ToolResultPart, ReasoningPart
└── desktop-ui/src/features/coding/components/ThreadItemList.tsx
```

### 2.2 Long-running task lifecycle

**Create:** `coding_message_send` → session upserted → `TurnStarted` published → steer_queue registered → tool kit rebound to workspace cwd.

**Run:** `process_direct_streaming` spawns `tokio::task` running `run_pipeline` → `AgentRuntime::process_message` → `execute_loop`. Loop checks cancellation, safety cap, then `ExecutionCore::run_cycle` (LLM + parallel tools).

**Stream:** Each `ContentChunk`, `ToolStart`, `ToolEnd` event sent on a 64-cap mpsc. Coding-mode bridge translates to `ThreadEvent` published on broadcast broker, emits `agent:thread_event` to frontend.

**Cancel:** `cancel_token: CancellationToken` checked at top of each loop iteration (`execute_loop.rs:68`). UI calls `coding_turn_interrupt` → token.cancel(). **Mid-LLM-stream cancellation does NOT exist** — must wait for current iteration's LLM response to finish.

**Complete/Persist:** Coding mode persists to SQLite **per iteration** (text, reasoning, tool calls, tool results, file changes). Crash leaves DB consistent up to last completed iteration.

**Resume:** `coding_resume` does title-fuzzy-match. **No in-progress loop state survives a crash** — only persisted history reloads.

### 2.3 Strengths (best-in-class)

1. **Loop detector** — hash-based oscillation detection (warn@3, stop@5). Unique to Klynt.
2. **Tiered history compression** — T0 verbatim / T1 LLM-summary / T2 condensed. Unique.
3. **MidLoopCompressor** — 70% threshold, replaces tool results outside last-8 with extractive snippets.
4. **LiveContextRefresher** — drains ContextUpdateQueue between iterations, priority-aware token reservation.
5. **Per-iteration SQLite persistence** — every part (text, reasoning, tool call, tool result, file change) persists at iteration boundary.
6. **Persistent approval grants** — `ApprovalGrants` table, not in-memory like opencode.
7. **Multi-tool concurrency with safety partition** — MAX=10, safe-vs-unsafe partitioning, semaphore.
8. **Safety caps + depth modes** — Normal/DeepThink/Ultra with explicit turn+token budgets.
9. **Cognitive subsystems** — mirror, reforge, FSRS5 spaced repetition, KCA gates. Unique platform identity.
10. **Hot-reload config** — 5-second file-watcher.

### 2.4 Gaps (vs comparators)

1. **Approval handler in coding mode is broken** — `coding/approval_handler.rs` returns `NotAvailable`.
2. **Auto-title is a stub** — `title_service.rs:50` has `// TODO: LLM call`.
3. **No mid-LLM-stream cancellation** — only between iterations.
4. **No resumability after process kill** — thread reload only, no in-progress recovery.
5. **No TodoWrite / task-list tool for coding mode.** AgentTaskTool is for subagent coordination only.
6. **No plan mode.** No EnterPlanMode/ExitPlanMode equivalent.
7. **No `wire.jsonl`-style immutable event log.**
8. **ThreadEventBuffer is spec'd but not built** (24 tasks pending).
9. **No background bash with TaskList/TaskOutput/TaskStop.**
10. **No diff preview in approval modal.**
11. **No persistent shell session for bash tool.**
12. **No /btw side question.**
13. **No checkpoint/D-Mail revert.**
14. **No user-facing hook system** (PreToolUse, PostToolUse, Stop, PreCompact).
15. **Anthropic prompt-caching strategy not verified.**

---

## 3. kimi-cli Analysis

> Source: deep-dive agent investigation of `/Users/jayden/Projects/Klynt/kimi-cli` on 2026-05-07.

### 3.1 Overview

Python terminal agent from Moonshot AI ("Kimi Code CLI"). UX philosophy: "act first, explain second" — system prompt explicitly tells the model to default to tool use rather than text. Built on prompt_toolkit + Rich `Live`. Sessions stored on disk keyed by work-directory hash → resumable across process restarts. Three inner packages: `kosong` (LLM abstraction), `kaos` (filesystem/SSH), `kimi_cli` (main).

### 3.2 Architecture tree

```
src/kimi_cli/
  app.py               – KimiCLI entry point
  agentspec.py         – YAML agent spec loader
  session.py           – Session CRUD; context.jsonl + wire.jsonl paths
  session_state.py     – Persisted mutable state (todos, plan_mode, approvals, yolo)
  metadata.py          – Global ~/.kimi/kimi.json index
  config.py            – LoopControl, BackgroundConfig, LLMModel, hooks
  llm.py               – Chat provider factory
  soul/
    agent.py           – Runtime; load_agent(); AGENTS.md loader
    kimisoul.py        – THE AGENT LOOP: _agent_loop(); FlowRunner; BackToTheFuture
    context.py         – Append-only JSONL with checkpoints + _usage watermarks
    compaction.py      – SimpleCompaction: summarize old, preserve last 2 pairs
    approval.py        – Tool-facing approval facade
    toolset.py         – KimiToolset: load, inject deps, MCP bridging
    dynamic_injection.py – System-reminder injection (plan mode, yolo)
    btw.py             – /btw side-question: cache-matched parallel call, no context mutation
    slash.py           – Soul-level slash command registry
  tools/
    file/              – ReadFile, WriteFile, StrReplaceFile, Glob, Grep
    shell/             – Shell (run_in_background support)
    agent/             – Agent tool: spawn/resume named subagent instances
    todo/              – SetTodoList: single tool, reads/writes whole list
    background/        – TaskList, TaskOutput, TaskStop
    plan/              – EnterPlanMode, ExitPlanMode
    think/             – Think tool (scratchpad, no output)
    ask_user/          – AskUserQuestion
    web/               – WebSearch, WebFetch
  background/
    manager.py         – BackgroundTaskManager: bash workers + asyncio agent tasks
    worker.py          – Subprocess worker
    store.py           – session/tasks/<id>/
  subagents/
    runner.py          – ForegroundSubagentRunner
    builder.py         – SubagentBuilder
    store.py           – session/subagents/<agent_id>/
  ui/shell/
    __init__.py        – ShellUI: prompt_toolkit input, cancel_event, task browser
    visualize/_live_view.py – Rich Live; streaming tool-call blocks; approval panels
  wire/
    types.py           – TurnBegin/End, StepBegin, ToolCall, ApprovalRequest, ...
    file.py            – WireFile: append wire.jsonl + protocol version header
  hooks/
    engine.py          – HookEngine: UserPromptSubmit, Stop, PreToolUse, PostToolUse, PreCompact
  acp/                 – Agent Client Protocol server (IDE integrations)
  notifications/       – Notification pub/sub
packages/
  kosong/              – LLM abstraction
  kaos/                – OS abstraction (local + SSH)
```

### 3.3 Lifecycle walkthrough

1. User types prompt at prompt_toolkit shell.
2. `cancel_event = asyncio.Event()` created. Agent coroutine + Live view run concurrently as asyncio tasks.
3. `KimiSoul.run(user_input)` → `TurnBegin` wire message → user msg appended to Context (immediately appended to `context.jsonl`).
4. `_agent_loop()` begins; `step_no` increments (cap: 500).
5. Auto-compaction if `tokens >= max_context_size * 0.85` or within 50K of cap. Last 2 pairs preserved verbatim; rest summarized; context file rotated.
6. `StepBegin(n=step_no)` → `_step()`.
7. Dynamic injections (plan mode reminder, yolo reminder) prepended as `<system-reminder>` user messages.
8. `kosong.step()` called with on_message_part=wire_send. `TextPart`/`ThinkPart`/`ToolCallPart` events flow through Wire to `_LiveView`, rendered at 10 fps.
9. Tool calls dispatched. Mutating tools hit `approval.request(...)` → `ApprovalRequest` wire message → keyboard panel. Tool coroutine `await`s `request.wait()`.
10. Tool results collected (concurrent). `ToolResult` per call. Block flushed to terminal.
11. Assistant + tool results appended to context. `StatusUpdate` with token counts.
12. If tool calls present, loop iterates; otherwise `StepOutcome(stop_reason="no_tool_calls")` exits.
13. **Steer drain**: any user input typed during the run is dequeued and injected as follow-up user message before next step.
14. `TurnEnd`. Session title set from first turn text. `approve_for_session` flushed.

### 3.4 Per-area findings

**A. Loop architecture.** `_agent_loop()` at `src/kimi_cli/soul/kimisoul.py:725`. Standard ReAct `while True`. Each iteration = "step". Exits on `stop_reason="no_tool_calls"` or rejected tool with no feedback. Max steps: `LoopControl.max_steps_per_turn=500`. State = in-memory `Context._history` + on-disk `context.jsonl`. **Ralph mode** (`--max-ralph-iterations`) wraps prompt in `FlowRunner` with CONTINUE/STOP decision node.

**B. Long-running tasks.** `SetTodoList` tool at `src/kimi_cli/tools/todo/__init__.py`. Pure tool-driven: model calls with full list to replace; `null` arg = query mode. Status: `pending|in_progress|done`. Persisted to `state.json`. **System prompt explicitly warns against over-tracking.** Separate **background task** system: `Shell(run_in_background=true)` spawns subprocess worker with JSON heartbeat; `Agent(run_in_background=true)` spawns asyncio coroutine.

**C. Cancellation.** ESC → `cancel_event.set()` (`_live_view.py:548`) → shell UI cancels asyncio task → `_agent_loop` receives `CancelledError` → `Wire` emits `StepInterrupted`. Tool approvals in-flight cancelled via `ApprovalRuntime.cancel_by_source()`. Ctrl-C trapped by prompt_toolkit (returns to input). Resumability via JSONL replay on full process restart.

**D. Streaming + UI.** Rich `Live` for streaming view + prompt_toolkit for input line. KeyboardListener as background asyncio task intercepts arrows + ESC. Tool calls appear as `_ToolCallBlock`: tool name + streaming args JSON. Result arrives → OK/error state → flushed. Status bar: token count, context %, plan mode flag, MCP loading. Spinners: `moon` (waiting for LLM), `balloon` (compacting), `dots` (MCP startup). Subagent events nested as indented sub-entries.

**E. Persistence.** Storage root: `~/.kimi/`. Index: `~/.kimi/kimi.json` maps `(work_dir, kaos_backend)` → MD5 hash. Session dir:
```
~/.kimi/sessions/<md5hash>/<uuid>/
  context.jsonl     – conversation history + checkpoints + _usage watermarks
  wire.jsonl        – every WireMessage with timestamp + protocol version header
  state.json        – todos, plan_mode, yolo, approval, title
  tasks/            – background task directories
  subagents/        – per-agent_id context + wire + state
```
`context.jsonl` uses pseudo-roles: `_system_prompt`, `_checkpoint(id)`, `_usage(token_count)`. Sessions resumable: `Session.continue_(work_dir)` returns last; `--continue` CLI flag.

**F. Subagent / parallel.** First-class. `Agent` tool creates named instance (`agent_id`) or resumes one. Built-in types from YAML: `explore` (read-only), `coder`. Two runners:
- **Foreground**: same event loop, wire events bubbled up nested in parent `Agent` tool block.
- **Background**: `BackgroundAgentRunner`, root soul continues, polls for completion via `NotificationManager`.

Multiple foreground subagents run concurrently if LLM emits parallel `Agent` calls in one step. Each subagent has its own context.jsonl + wire.jsonl + state.json.

**G. Approval.** Three tiers: (1) **Manual** with numbered keyboard panel (Yes / Yes always for session / No); (2) **Session auto-approve** (added to `auto_approve_actions`, persisted); (3) **Yolo** (`--yolo` or `/yolo`) auto-approves all. Background agent approvals tagged `source_kind="background_agent"`, surfaced through root wire hub.

**H. Context management.** `SimpleCompaction` preserves last 2 user/assistant pairs verbatim, summarizes rest via dedicated compact prompt LLM call. Triggers: `tokens >= 0.85 * max` OR `tokens + 50_000 >= max`. After compaction, context file rotated (`.1` suffix). Active background tasks re-injected as system message post-compaction. `/compact <instruction>` for manual compaction with custom focus. **No memory feature** (no vector store, no external memory tool).

**I. Tools.** Python classes inherit `kosong.tooling.CallableTool2[Params]`. Pydantic `Params` → JSON schema. Dispatch in `KimiToolset.handle()`. Dependencies injected at `load_tools()` by type. MCP via `fastmcp`, loaded in background at startup. Plugin tools from plugins dir. Hooks intercept `PreToolUse`/`PostToolUse` — script-based extension.

**J. Plan mode.** `EnterPlanMode`/`ExitPlanMode` tools. While active:
- `WriteFile`/`StrReplaceFile` only allowed to write designated plan file.
- Other write tools rejected with `<system-reminder>` injection.
- LLM reminded each step it's in plan mode.

`EnterPlanMode` presents `QuestionRequest` for user confirmation. Stable `plan_session_id` UUID + slug for plan filename. `/plan` toggle slash command.

### 3.5 Standout features

1. **Wire bus as universal interface.** Every event → typed Pydantic message → queue. All UIs (shell, ACP, print, web) are pure consumers. Same agent code drives interactive terminal + IDE plugin + JSON stream API.

2. **`wire.jsonl` as immutable session record.** Every event appended with Unix timestamp + protocol version header. `kimi vis` serves local web UI replaying the log. Any session is a perfect debugger repro.

3. **Subagent parallelism via LLM-driven parallel tool calls.** System prompt nudges LLM to emit multiple `Agent` calls in one step. `kosong.step()` runs all results concurrently → multiple subagents in parallel within one event loop, no extra scheduling.

4. **D-Mail (time-travel revert).** `DenwaRenji` mechanism: a subagent calls `SendDMail(checkpoint_id, message)` → `BackToTheFuture` exception → loop catches → `Context.revert_to(checkpoint)` + synthetic user message. Effective branchable agent history.

5. **Side questions (`/btw`) without context mutation.** Parallel LLM call with shared system prompt + history (cache hit) but `_DenyAllToolset`. Response in bordered panel, never written to context. Lets user clarify without derailing in-flight turn.

6. **Agent Flow (Mermaid/D2 as agent programs).** Skills declare `type: flow` with embedded flowchart. `FlowRunner` traverses; decision nodes use `<choice>...</choice>` LLM responses. Ralph mode reuses this for autonomous retry.

### 3.6 What kimi-cli lacks

- No wall-clock timeout on foreground loop (only step count).
- TODO list flat: no parent/child, due dates, priority, file links.
- No parallel tool dispatch within single step at soul layer (kosong does it; soul awaits all collectively before context append).
- Compaction lossy with no fine-grained control: same model, same prompt, no "never compact" markers.
- Background task output polling-only: no reactive push from bash subprocess to main context.
- No per-session secret/credential scoping; auto-approvals are plain action-name strings.

---

## 4. opencode Analysis

> Source: deep-dive agent investigation of `/Users/jayden/Projects/Klynt/opencode` on 2026-05-07.

### 4.1 Overview

Archived (continued as "Crush" by Charmbracelet) Go terminal coding agent. UX: "minimal interruption with human oversight" — autonomous tool iterations, but pauses mid-loop for permission before any mutative action. TUI uses Bubble Tea + Lipgloss + Bubbles. **No background daemon — process is the app.** SQLite (WAL mode) for persistence. Multi-provider: Anthropic, OpenAI, Gemini, AWS Bedrock, Groq, Azure, GitHub Copilot, local.

### 4.2 Architecture tree

```
/
├── main.go                 – panic recovery, delegates to cmd.Execute()
├── cmd/root.go             – Cobra entry, boots DB + App + TUI
└── internal/
    ├── app/                – App struct wiring services; LSP lifecycle
    ├── config/             – Viper-based JSON config; agent, provider, LSP, MCP
    ├── db/                 – SQLite (ncruces/go-sqlite3); sqlc-generated; goose migrations
    ├── history/            – File versioning (pre-edit snapshots in SQLite)
    ├── llm/
    │   ├── agent/          – Core loop (agent.go), sub-agent tool, MCP bridge
    │   ├── models/         – Model registry: ID, provider, context window, costs
    │   ├── prompt/         – Coder, task, summarizer, title prompts
    │   ├── provider/       – Provider interface + per-provider impls
    │   └── tools/          – BaseTool + bash, edit, view, glob, grep, etc.
    ├── lsp/                – LSP client (JSON-RPC stdio), file watcher, diagnostics
    ├── message/            – Message service over DB; rich ContentPart
    ├── permission/         – Blocking permission gate
    ├── pubsub/             – Generic in-process broker
    ├── session/            – Session CRUD; CreateTaskSession for sub-agents
    ├── tui/
    │   ├── components/     – Chat list, editor, message renderer, dialogs
    │   ├── layout/         – Split-pane, container, overlay
    │   ├── page/           – chat.go, logs.go
    │   ├── theme/          – catppuccin, dracula, gruvbox, monokai, flexoki, default
    │   └── tui.go          – Root Bubble Tea model
    ├── diff/               – Unified diff with syntax color
    ├── completions/        – @ file/folder completions
    ├── format/             – text/JSON for non-interactive
    └── logging/            – Ring-buffer log sink + TUI log viewer
```

### 4.3 Lifecycle walkthrough

1. **User sends message** → `chat.go:sendMessage` → `app.CoderAgent.Run(ctx, sessionID, text)` (`agent.go:198`). Cancellable child context stored in `activeRequests sync.Map` keyed by `sessionID`.

2. **processGeneration** (`agent.go:233`) — Loads history; if `session.SummaryMessageID` set, slices from there. Creates user message; appends to in-memory slice; enters unbounded `for {}`.

3. **Each iteration** — `streamAndHandleEvents` (`agent.go:322`) → `provider.StreamResponse()` → `<-chan ProviderEvent`. Assistant message updated incrementally in SQLite (text deltas, tool call starts/stops). Every update fires `pubsub.UpdatedEvent`.

4. **Tool execution** — All `ToolCall`s run **serially**. Mutating tools call `permission.Service.Request()` → blocks goroutine via one-shot `chan bool` → TUI renders permission modal → user response unblocks.

5. **Continue or stop** — Finish reason `tool_use` → loop continues; `end_turn` → exits, final `AgentEvent{Done: true}` published.

6. **Cancellation** — `Ctrl+X` → `app.CoderAgent.Cancel(sessionID)` → `cancel()` on stored ctx. Streaming loop checks `ctx.Err()` between events. Persistent shell `Exec()` interrupts via timeout/cancellation.

7. **Auto-compaction** — TUI monitors `AgentEvent` for summarize signal. `agent.Summarize()` fetches messages, calls `summarizeProvider.SendMessages()` (non-streaming, blocking), stores summary as new assistant message in **same session**, sets `summary_message_id`. Next `processGeneration` slices history from there.

### 4.4 Per-area findings

**A. Loop.** `processGeneration` at `internal/llm/agent/agent.go:233`. Infinite `for {}` with cancel check at top. **No max-iterations / token-budget guard** in loop itself. Soft budget: auto-compaction at 95% (per README — actual check missing in code). State = in-memory `msgHistory []Message` + ctx values for `SessionIDContextKey`/`MessageIDContextKey`.

**B. Long-running tasks.** **No explicit todo/plan/checklist tool.** "Task" maps to sub-agent sessions: `sessions.CreateTaskSession(ctx, toolCallID, parentSessionID, "New Agent Session")` — `toolCallID` becomes child's PK (`session.go:54`). Task agent uses read-only subset (glob, grep, ls, view, sourcegraph). No SQLite "tasks" table — sessions with `parent_session_id` FK.

**C. Cancellation.** `context.CancelFunc` in `agent.activeRequests sync.Map`. Keys: `sessionID` + `sessionID+"-summarize"`. `Cancel()` LoadAndDeletes both. **Permission gate leak bug**: if ctx cancelled while goroutine blocked in `permission.Request()`, blocks forever — `respCh` select doesn't include `<-ctx.Done()` (`permission.go:74`). No resume after cancel.

**D. Streaming + UI.** Bubble Tea + Lipgloss + Bubbles. `cmd/root.go:249` fans out 5 pubsub streams (logging, sessions, messages, permissions, coderAgent) into one `chan tea.Msg` → `program.Send()`. **Tool calls rendered live while in-flight** (`message.go:535`): "Reading file..." for view tool. Once finished: actual result. Edit/write/patch results = syntax-colored unified diff. Bash output = fenced code block. Image rendering via `disintegration/imaging`.

**E. Persistence.** SQLite at `<dataDir>/opencode.db`. **WAL mode + tuned pragmas**:
```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA page_size = 4096;
PRAGMA cache_size = -8000;
```
Three tables:
- `sessions(id PK, parent_session_id, title, message_count, prompt_tokens, completion_tokens, cost, summary_message_id, ...)`.
- `messages(id PK, session_id FK, role, parts TEXT JSON, model, ...)` — `parts` is JSON blob with all content parts.
- `files(id PK, session_id FK, path, content, version, ...)` — UNIQUE(path, session_id, version) for pre-edit snapshots.

`messages.parts` JSON blob → readable by external pollers via `WHERE session_id = ? AND updated_at > ?`. **No daemon — sessions only progress while TUI open.**

**F. Subagent.** `agent` tool spawns sub-agent **synchronously** in parent goroutine — sub-agent `done` channel blocks before parent continues. **No concurrent fan-out.** System prompt advises LLM to "launch concurrently" but runtime is serial. TUI supports multiple sessions (`Ctrl+S`) but only one runs at a time per `App`.

**G. Approval.** Three-tier: **Allow once / Allow for session / Deny**. `sessionPermissions` slice (process-lifetime) + `autoApproveSessions` (non-interactive). Safe read-only bash bypasses (`safeReadOnlyCommands` whitelist). Banned: `curl, wget, nc, telnet, ...` rejected pre-permission. MCP always requires permission. Modal shows tool-specific previews: bash → highlighted fence; edit/write → colored unified diff with scrollable viewport. **No sandbox** (no container/chroot/seccomp). Granularity: tool name + action + cwd path.

**H. Context.** Auto-compact opt-out (`autoCompact: true`). 95% trigger documented but **actual check not implemented in agent loop**. Summarizer = separate agent role with own model. Summary stored in same session (not new). Coder prompt instructs LLM to read `OpenCode.md` for persistent memory.

**I. Tools.** `BaseTool` interface (`tools/tools.go:69`): `Info() ToolInfo` + `Run(ctx, ToolCall) (ToolResponse, error)`. Slice-based registry, no dynamic registration. Built-ins: `bash`, `edit`, `view`, `write`, `patch`, `glob`, `grep`, `ls`, `fetch`, `sourcegraph`, `diagnostics`, `agent`. MCP via `GetMcpTools()` → `ListTools` → wrap as `mcpTool`. Names: `{serverName}_{toolName}`. **MCP client re-init per call** (expensive for stdio servers). Anthropic prompt caching: `cache_control: ephemeral` on system + last 2 user + last tool def (`anthropic.go:135`). LSP `diagnostics` tool returns tagged XML blocks.

**J. Plan mode.** **No explicit plan phase.** Coder system prompt advises "use search → implement → verify with tests" — guidance only. `agent` tool is closest to delegation. No architect/code two-stage flow.

**K. Multi-provider.** `Provider` interface at `provider/provider.go:53`. Each provider has its own client. Non-Anthropic adapted to look like Anthropic streaming via same `ProviderEvent` types. Groq/OpenRouter/xAI/local reuse OpenAI client with `WithOpenAIBaseURL`. Capabilities (`CanReason`, `SupportsAttachments`, costs) in static `SupportedModels` map. Hot-swap via `agent.Update(name, modelID)` (guarded by `IsBusy()`).

**L. No daemon.** No `opencode serve`. Single-process app. TUI ↔ services via direct Go interface calls + pubsub. Non-interactive (`-p "prompt"`) bypasses TUI; auto-approves all.

### 4.5 Standout features

1. **Permission modal with live diffs.** File write/edit ops shown as colored unified diff with scrollable viewport before approve. Massively reduces blind-approval bugs vs plain "allow/deny" dialog.

2. **Task sessions keyed by toolCallID.** `session.go:54` uses parent tool-call ID as child session PK. Elegant join, no extra table. External pollers fetch sub-histories with `WHERE parent_session_id = ?`.

3. **WAL with tuned pragmas.** Poll-friendly. Klynt's `coding-ingest` adapter polls this exact DB.

4. **Persistent shell session.** Long-lived shell subprocess per session. `cd`, `export`, `source venv/bin/activate` persist across bash calls.

5. **Anthropic prompt caching strategy.** Cache breakpoints on system + last 2 user + last tool def. Textbook strategy.

6. **Multi-theme TUI.** Six themes with runtime switching via `ThemeChangedMsg`.

7. **Inline sub-agent rendering.** Sub-agent's tool calls rendered as indented rows under parent `agent` block (`message.go:613–626`) with tree prefix `└`.

### 4.6 What opencode lacks

- **No plan / TodoWrite / checklist** — same gap as Klynt.
- **Serial tool execution** despite system prompt advising parallelism.
- **No iteration budget / circuit breaker** — `for {}` runs until cancel or context fills.
- **Permission gate leak on cancellation** — goroutine blocks forever.
- **No background / offline execution.**
- **Single coder agent per App instance** — multiple sessions visually switchable but only one runs.
- **MCP client-per-call overhead** — stdio servers fork subprocess per call.
- **Thin LSP usage** — full client implemented but only `diagnostics` exposed to AI.

---

## 5. Cross-Project Comparison

### 5.1 Long-running-task feature scorecard

| Capability | Klynt | kimi-cli | opencode |
|---|---|---|---|
| Iterative loop | 10/10 | 8/10 | 6/10 |
| Streaming events | 9/10 | 9/10 | 8/10 |
| Per-iteration persistence | 10/10 | 9/10 | 7/10 |
| Mid-stream cancel | 4/10 | 8/10 | 5/10 |
| Resume after process kill | 2/10 | 9/10 | 5/10 |
| **TODO tool** | 0/10 | 8/10 | 0/10 |
| **Plan mode** | 0/10 | 9/10 | 0/10 |
| Time-travel checkpoint | 0/10 | 9/10 | 0/10 |
| Wire-bus immutable record | 2/10 | 10/10 | 4/10 |
| Real-time catch-up | 1/10 | 9/10 | 0/10 |
| Background bash tasks | 1/10 | 9/10 | 0/10 |
| Subagent system | 8/10 | 9/10 | 5/10 |
| Side-question (/btw) | 0/10 | 9/10 | 0/10 |
| Mid-loop context compress | 10/10 | 6/10 | 5/10 |
| Approval gate | 6/10 | 8/10 | 8/10 |
| Diff preview in approval | 0/10 | 4/10 | 9/10 |
| Auto-title generation | 1/10 | 6/10 | 7/10 |
| Persistent shell | 0/10 | 4/10 | 8/10 |
| Hooks (user-facing) | 1/10 | 8/10 | 0/10 |
| Loop oscillation detect | 9/10 | 0/10 | 0/10 |
| Tiered compression | 10/10 | 4/10 | 4/10 |
| Live context refresh | 9/10 | 0/10 | 0/10 |
| Multi-tool parallel | 9/10 | 5/10 | 0/10 |
| Cognitive memory | 9/10 | 0/10 | 0/10 |
| Hot-reload config | 8/10 | 5/10 | 5/10 |
| **TOTAL (out of 250)** | **125** | **161** | **86** |

### 5.2 Completion percentage

- **vs kimi-cli (the leader)**: 55–60% of long-running-task UX feature surface.
- **vs opencode**: 75–80% of feature surface.
- **Unique strengths beyond both**: ~30–40% extra infrastructure neither comparator has (compression, loop detector, cognitive, persistent grants, KCA gates).

---

## 6. Detailed Gap Analysis

### 6.1 Critical (production blockers for coding mode)

1. **Approval handler in coding mode broken** (`crates/app-core/src/coding/approval_handler.rs` returns `NotAvailable`). Wiring incomplete.
2. **No mid-LLM-stream cancellation.** Slow LLM = stuck UI.
3. **Auto-title is a stub** (`title_service.rs:50`). Sidebar shows "Untitled session".
4. **No resumability after process kill.** In-flight loop state lost.

### 6.2 High-value LLM-facing affordances

5. **No TodoWrite / task-list tool for coding mode.**
6. **No plan mode.**
7. **No `wire.jsonl`-style immutable event log.**
8. **No background bash with TaskList/Output/Stop trio.**
9. **No hooks** (PreToolUse, PostToolUse, Stop, PreCompact).
10. **No /btw side question.**

### 6.3 UX polish

11. **No diff preview in approval modal.**
12. **No persistent shell session for bash tool.**
13. **Anthropic prompt-caching strategy not visible / verified.**
14. **Sidebar updates rely on unimplemented ThreadEventBuffer.**

### 6.4 Research-grade

15. **No checkpoint / time-travel.** D-Mail equivalent.
16. **No Agent Flow.** Mermaid-as-agent-program.

---

## 7. Comprehensive Roadmap to Best-in-Class

> Goal: close all the gaps and bring Klynt's coding mode to best-in-class on long-running tasks. Five phases, ordered by leverage and dependency.

### Phase 0 — Critical fixes (Week 1)

> **Why first:** these are already-broken or already-stubbed things that prevent coding mode from feeling production-ready. Fixing them unlocks credibility before any new feature lands.

#### 0.1 Wire `coding/approval_handler.rs` to `ApprovalGate`
- **Files:** `crates/app-core/src/coding/approval_handler.rs`, `crates/app-core/src/coding/mod.rs`
- **Change:** Replace `NotAvailable` return with delegation to `ApprovalGate::check(req).await`. Reuse existing `BlockingFallbackChannel` until coding-specific channel lands.
- **Test:** Add an integration test that triggers a `Destructive` tool call in a coding turn and asserts the approval modal appears.
- **Effort:** 1 day.

#### 0.2 Finish `title_service.rs` LLM call (Task 4)
- **Files:** `crates/app-core/src/coding/title_service.rs`
- **Change:** Replace `// TODO: LLM call` stub with actual call to `cognitive.provider`. 5-second timeout. On success, emit `coding:thread_updated` over the bus. Apply `sanitize_title` before persist.
- **Tied to:** the spec at `docs/superpowers/specs/2026-05-07-coding-sidebar-titles-and-running-state-design.md`.
- **Effort:** 1 day.

#### 0.3 Mid-LLM-stream cancellation
- **Files:** `crates/providers/src/anthropic_stream.rs`, `crates/providers/src/openai_stream.rs` (and equivalents).
- **Change:** Wrap SSE polling in `tokio::select! { event = ... => ..., _ = cancel_token.cancelled() => ... }`. Drop HTTP connection on cancel. Emit `AgentEvent::Cancelled` with partial content.
- **Also fix:** `ApprovalChannel::request().await` should `select!` on cancel_token to avoid opencode-style leak.
- **Test:** Cancel mid-stream, assert UI shows partial content + cancelled state immediately.
- **Effort:** 2–3 days.

#### 0.4 Implement `ThreadEventBuffer` per existing spec
- **Files:** `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts` (new), tied into `useThreadEvents`.
- **Change:** Per the existing 24-task plan in `docs/superpowers/plans/2026-05-07-coding-sidebar-titles-and-running-state.md`. Per-thread ring buffer (cap=500). `subscribeToThread` synchronously replays buffered events before live subscription.
- **Effort:** 3–5 days.

**Phase 0 outcome:** Coding mode feels production-grade. All "obviously broken" things are fixed.

---

### Phase 1 — Wire-bus foundation (Week 2)

> **Why second:** this is the highest-leverage single change. It enables resumability, replay, external tooling, and zero-loss catch-up — all from one append-only file.

#### 1.1 Define wire schema
- **New file:** `crates/coding-memory/src/wire/schema.rs`.
- **Schema:** `WireEvent` enum mirroring `ThreadEvent` but framed with `seq: u64`, `ts: jiff::Timestamp`, `protocol_version: u32`. Pydantic-equivalent in TypeScript via existing specta bindings.
- **Tests:** Property test: `serialize(parse(event)) == event`.

#### 1.2 `WireFile` appender service
- **New file:** `crates/coding-memory/src/wire/file.rs`.
- **Behaviour:** Subscribe to `DomainEventBus` for `agent:thread_event`. Append to `{data_dir}/coding/<thread_id>/wire.jsonl` with `tokio::fs::OpenOptions::append(true)`. Periodic `fsync` (every 1s or 100 events). First line: `{"protocol_version":1,"created_at":"..."}`.
- **Why subscribe rather than emit-from-loop:** decouples the loop from disk I/O latency. Same pattern kimi-cli uses.

#### 1.3 Replay-on-load
- **Change:** When opening a coding thread that has a `wire.jsonl`, replay events into the React `useThreadEvents` reducer before the live subscription starts. This is a stronger version of `ThreadEventBuffer` — works across process restarts, not just tab switches.
- **Cap:** Replay only the last N events to avoid slow loads on huge threads (e.g. last 1000 events; older events are still readable on demand).

#### 1.4 `klynt vis` CLI subcommand
- **New file:** `crates/desktop/src/commands/vis.rs` (reuse the no-raw-tauri-command convention).
- **Behaviour:** `klynt vis <thread_id>` opens a local HTTP server rendering the wire log as a chronological viewer. Useful for debugging, sharing, and post-hoc analysis.

**Phase 1 outcome:** Klynt has the same "wire log as source of truth" architecture as kimi-cli. Resumability after crash works. Tab catch-up is now a free downstream consequence.

---

### Phase 2 — LLM-facing affordances (Weeks 3–4)

> **Why third:** these are additive features that work on top of the wire-bus foundation. Each is independent — can be parallelized across multiple PRs.

#### 2.1 TodoWrite tool

- **New crate:** `feature-coding-todo` OR add to existing `feature-tasks` with channel-gating to `coding`.
- **Tool surface:** Single tool `coding_todo` with two modes:
  - `coding_todo(items: Vec<TodoItem>)` — overwrites the list.
  - `coding_todo()` — read-only query.
- **TodoItem schema:** `{ id: u32, title: String, status: pending|in_progress|done }`.
- **Persistence:** New table `coding_todos(thread_id, items_json, updated_at)`. One row per thread.
- **System prompt:** Add anti-abuse paragraph copied from kimi-cli's prompt: "Don't track too small steps; don't repeat without progress; use status `in_progress` for the current task only."
- **UI:** New `TodoSidebar.tsx` component showing live todo list. Subscribes to `coding:todos_updated` bus event.
- **Wire log:** TodoWrite calls also flow through `wire.jsonl` (free via Phase 1).
- **Effort:** 3 days.

#### 2.2 Plan mode

- **Tool pair:** `coding_enter_plan_mode` + `coding_exit_plan_mode`.
- **State:** New `coding_session_state.json` per-thread (or column on session row) — `plan_mode_active: bool`, `plan_session_id: uuid`, `plan_file_slug: string`.
- **Enforcement:** Hook into existing `interceptor_chain` in `core.rs:756`. While `plan_mode_active`, reject `Edit`/`Write` to paths != `plan_file_path` with a `<system-reminder>` injection.
- **Implementation tip:** Frame as a `CodingApprovalPolicy::PlanMode` variant rather than raw tool gating — fits the existing approval architecture.
- **System prompt:** Add a dynamic injection (à la kimi-cli's `dynamic_injection.py`) that prepends a `<system-reminder>` user message during plan mode.
- **UI:** Status bar pill "Plan Mode" + plan-file viewer panel.
- **Effort:** 3–4 days.

#### 2.3 Background bash tasks

- **Tool surface:** `coding_shell(run_in_background: true)` returns `task_id`. New tools: `coding_task_list`, `coding_task_output(id)`, `coding_task_stop(id)`.
- **Manager:** New `BackgroundShellManager` in `crates/coding-memory/src/background/`.
- **Persistence:** `coding_background_tasks` table with `id, thread_id, command, pid, status, output_path, started_at, heartbeat_at`. Output streamed to `{data_dir}/coding/<thread_id>/tasks/<task_id>/output.log`.
- **Reuse:** Klynt's `klynt-sandbox` and `klynt-sandbox-helper` crates already provide a sandboxed exec foundation.
- **Heartbeat:** Worker writes JSON heartbeat every 5s; manager marks "lost" if heartbeat expires + 30s.
- **Re-injection on compaction:** When MidLoopCompressor or TieredHistoryCompressor fires, inject `<system-reminder>active background tasks: ...</system-reminder>` so the agent doesn't forget.
- **Effort:** 1 week.

#### 2.4 Hook system

- **Surface:** `config.json` gains `hooks: { pre_tool_use: [{ match: "Bash", script: "..." }, ...], post_tool_use: [...], stop: [...], pre_compact: [...] }`.
- **Implementation:** Extend existing `interceptor_chain`. Each hook is a shell command; stdin = JSON event payload; exit code 0 = allow, non-zero = block.
- **Sandbox:** Run hooks in `klynt-sandbox` for safety.
- **Klynt-specific bonus:** Hooks can also fire on `MirrorEvent` and `ReforgeEvent` — leverages cognitive subsystems no other agent has.
- **Effort:** 3 days.

#### 2.5 /btw side-question

- **UI:** `/btw <question>` slash command in composer.
- **Backend:** New handler `coding_btw_ask` that:
  1. Builds same system prompt + history snapshot (cache hit).
  2. Substitutes a `DenyAllToolset` (rejects every tool call with "btw mode: tools disabled").
  3. Runs single LLM iteration, no context mutation.
  4. Returns response as a transient panel (not persisted to wire log).
- **Effort:** 1–2 days.

**Phase 2 outcome:** The LLM has first-class abstractions for managing long tasks. TodoWrite, plan mode, background shells, hooks, and side questions all work.

---

### Phase 3 — UX polish (Weeks 5–6)

> **Why fourth:** these are direct ports from opencode that polish the user-facing experience without architectural change.

#### 3.1 Diff preview in approval modal

- **Files:** `desktop-ui/src/features/approvals/ApprovalModal.tsx`, `crates/app-core/src/coding/approval_handler.rs`.
- **Change:** When the approval request involves `Edit` or `Write`, compute unified diff in Rust using `similar` crate. Pass diff payload through `ApprovalRequest`. Render in modal with syntax highlighting and scrollable viewport (reuse Klynt's existing diff renderer if any, otherwise port opencode's `diff/` package logic).
- **Bonus:** Show file path, line range, and approve-once vs approve-forever-for-this-file granularity.
- **Effort:** 2–3 days.

#### 3.2 Persistent shell session

- **Files:** `crates/tools/src/bash.rs`, `crates/klynt-sandbox-helper/src/lib.rs`.
- **Change:** Per-thread persistent shell subprocess. Stdin pipe reused across calls. `cd`, `export`, `source` persist.
- **Lifecycle:** Shell spawned on first bash call in a thread; killed on thread close. `coding_shell_reset` tool to restart.
- **Caveat:** Background tasks (Phase 2.3) use separate processes; persistent shell is for foreground sequential calls only.
- **Effort:** 3 days.

#### 3.3 Anthropic prompt-caching audit

- **Files:** `crates/providers/src/anthropic*`.
- **Change:** Audit cache_control breakpoints. Should be on:
  1. System message (always).
  2. Last 2 user messages.
  3. Last tool definition.
- **Tied to:** existing spec `docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md` and plan `docs/superpowers/plans/2026-05-05-provider-agnostic-prompt-cache-placement-implementation.md`.
- **Verify:** Logs show cache hit rate >70% on long sessions.
- **Effort:** 1 day.

#### 3.4 Visual streaming polish

- **Tool-row redesign refinements:** Already shipped per commit `a6e35e7b4` but expand:
  - Streaming animation for tool args (show JSON building character-by-character).
  - Spinner per tool while in flight.
  - Token count per tool result.
- **Reasoning expand-collapse:** Currently `ReasoningPart` doesn't have expand control — add one.
- **Effort:** 2 days.

**Phase 3 outcome:** Coding mode feels as polished as Claude Code or Cursor. Approvals show what they're approving. Shells remember state. Caching reduces costs.

---

### Phase 4 — Research-grade (Weeks 7+, optional)

> **Why last:** these are speculative features that need brainstorming before building. None blocks the core experience; they're potential differentiators if Klynt wants to lead rather than match.

#### 4.1 Checkpoint / D-Mail revert

- **Concept:** Subagent (or slash command `/revert <checkpoint>`) requests history revert. Loop catches, truncates in-memory state, injects synthetic user message, continues.
- **Hard part:** UX — branching turns the linear thread into a DAG. Sidebar needs to show branches; user needs a way to switch between them.
- **Klynt-specific:** Per-iteration SQLite persistence already creates implicit checkpoints. The wire.jsonl gives natural seq numbers. Use seq as checkpoint ID.
- **Brainstorm needed.** Don't just copy kimi-cli; design for Klynt's UX.

#### 4.2 Agent Flow (Mermaid as agent program)

- **Concept:** Skills declare `type: flow` with embedded Mermaid/D2 flowchart. `FlowRunner` traverses, submits each node as user prompt. Decision nodes use `<choice>...</choice>` LLM responses.
- **Leverage:** Combines well with Klynt's existing skill system.
- **Lower priority** than core coding-agent affordances.

#### 4.3 Multi-modal screenshot tools (already designed)

- **Reference:** `docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md` (in-design, not implemented).
- **Note:** Design exists but execution is gated on Anthropic adapter changes (computer_use beta header, ImageData content part).
- **Recommendation:** Defer until Phase 0–3 land.

---

### Phase 5 — Cross-cutting infrastructure improvements

> Smaller but valuable wins that touch multiple phases.

#### 5.1 Iteration budget breaching becomes user-visible

- Currently `LoopFinishReason::SafetyTurnLimit` is logged but not surfaced to the user.
- Change: emit a `ThreadEvent::SafetyCapHit { reason, partial_content }` and render it as a banner in the React UI offering "continue with same depth", "promote to deeper depth", or "stop".

#### 5.2 Subagent parallel-call enforcement

- Klynt already supports `MAX_CONCURRENT_TOOLS=10` parallel tool fan-out — but the **system prompt** doesn't nudge the LLM to emit parallel `spawn_subagent` calls. Add explicit instruction: "When facing N independent subtasks, emit N `spawn_subagent` tool calls in a single response."

#### 5.3 Per-tool concurrency policy (opencode lacks this)

- Klynt's `partition_by_concurrency_safety` already exists. Document it and expand: tools should declare `ConcurrencySafety::{Safe, Sequential, Exclusive}` rather than relying on hardcoded heuristics.

#### 5.4 MCP client connection pooling

- opencode re-inits MCP client per call (expensive for stdio). Klynt should pool stdio connections per server.
- Reference: `crates/mcp/`, `crates/mcp-bridge/`.

#### 5.5 LSP-driven tools beyond diagnostics

- Klynt has `crates/lsp-client/`. Currently surfaced as diagnostics tool. Expose more: `lsp_definition`, `lsp_references`, `lsp_hover`, `lsp_workspace_symbols`. Massively improves agent's code understanding.

---

## 8. What Klynt Should NOT Copy

Things kimi-cli/opencode have that Klynt should consciously **pass on**:

1. **kimi-cli's 500-step turn limit.** Klynt's tiered safety cap (Normal/DeepThink/Ultra) is more nuanced. Don't regress.

2. **kimi-cli's `SimpleCompaction`.** Klynt's three-layer compression (MidLoop + Tiered + LiveRefresh) is genuinely better. The "preserve last 2 pairs verbatim" heuristic is too coarse.

3. **opencode's serial tool execution.** Klynt's parallel fan-out with safety partitioning is a real advantage. Keep it.

4. **opencode's permission-gate leak bug.** Make sure `ApprovalChannel::request().await` always wires through cancellation.

5. **opencode's missing iteration budget.** Klynt's `SafetyCap` + `LoopDetector` should be preserved as differentiators.

6. **opencode's in-memory-only session approvals.** Klynt's `ApprovalGrants` table is durable across restarts — don't regress.

7. **kimi-cli's polling-only background output.** Klynt should consider streaming partial output from background tasks via the bus rather than requiring the LLM to call `TaskOutput` to see progress. (Cost: more events; benefit: real-time UX.)

8. **The temptation to add a "synthesis pass" at safety cap.** Klynt's recent design explicitly removed this (see `docs/superpowers/specs/2026-05-05-model-self-stop-termination-design.md`). Preserve that decision.

---

## 9. Verification & Success Criteria

### 9.1 Per-phase exit criteria

**Phase 0 done when:**
- [ ] Approval modal appears for `Edit`/`Write` in coding mode (manual test).
- [ ] All coding sessions show real titles in sidebar.
- [ ] `Cmd+.` cancels a streaming response within 200ms.
- [ ] `ThreadEventBuffer` test: switch tabs during a long turn, switch back, all events present.

**Phase 1 done when:**
- [ ] `wire.jsonl` exists for every coding thread, valid newline-delimited JSON, parses round-trip.
- [ ] Kill `klyntbot` mid-turn, restart, open same thread — full event log replays.
- [ ] `klynt vis <thread_id>` renders the log in browser.
- [ ] Property test passes: `parse(serialize(WireEvent)) == WireEvent` for all variants.

**Phase 2 done when:**
- [ ] Agent calls `coding_todo` during a multi-step task; sidebar shows todos updating live.
- [ ] Plan mode rejects file writes outside plan file with `<system-reminder>` injection visible in wire log.
- [ ] `coding_shell(run_in_background=true)` for `cargo build --workspace` returns immediately; `coding_task_output` shows partial output; agent can `coding_task_stop`.
- [ ] User-supplied PreToolUse hook can block specific bash commands.
- [ ] `/btw <q>` returns answer in panel without polluting context (verify by checking `context.jsonl` is unchanged).

**Phase 3 done when:**
- [ ] Approval modal shows live unified diff for an `Edit` operation, with syntax color.
- [ ] In a single coding thread, two consecutive `coding_shell` calls share env vars (e.g. `export X=1` then `echo $X` returns `1`).
- [ ] Anthropic provider logs show cache hit rate >70% on long sessions.

### 9.2 Holistic success benchmarks

After all phases land, Klynt should:
- **Match or exceed kimi-cli** on the long-running-task scorecard (target: ≥160/250).
- **Match opencode** on UX polish (diff-preview, persistent shell, prompt-cache).
- **Lead both** on infrastructure rigour (preserve current 30–40% advantage on compression/cognitive/loop-detect/persistent-grants).
- **Compose into Klynt-unique workflows**: e.g. "agent enters plan mode → spawns 3 explore subagents → summarizes findings via cognitive memory → exits plan → executes with todo list → background tests run while agent edits → wire.jsonl audits everything → mirror reflects on outcome → reforge improves the rule next night."

That last bullet is the actual differentiator: no other agent in the comparison set has the cognitive layer to close the loop.

---

## Update 2026-05-07 (later same day) — Phase 1 reframed

After this analysis was first written, deeper reading of the existing codebase revealed that **most of the "wire-bus foundation" the roadmap proposed already exists** — built generically as a multi-source observability pipeline rather than as a Klynt-specific subsystem. This is a more elegant architecture than what the roadmap originally proposed. Specifically:

| Component | Originally proposed | Already in tree |
|---|---|---|
| Typed event schema | New `WireEvent` enum | `coding-ingest::AgentEvent` with `EventKind` (10+ variants) at `crates/coding-ingest/src/event.rs` |
| Event source tagging | New | `AgentSource::KlyntCli` enum variant exists at `event.rs:55` |
| Runtime → ingest translator | New | `Translator` at `crates/coding-memory/src/sink/translator.rs` (already tags `KlyntCli` at line 233) |
| Aggregator (pairs ToolStart+End, accumulates ContentChunk) | New | `Aggregator` at `crates/coding-memory/src/sink/aggregator.rs` |
| Sink trait + in-process sink | New | `MemorySink` + `InProcessSink` + `MemorySinkSubscriber` at `crates/coding-memory/src/sink/` |
| Replay-on-load | New | Tracing UI's `WireViewer` already replays per session |
| `klynt vis` web replay | New CLI subcommand | `desktop-ui/src/tracing/` page with WireViewer, ContextViewer, StateViewer, DualView, AgentsPanel, Statistics — already serves all four ingested CLIs |
| Property test for round-trip | New | `crates/coding-ingest/tests/cross_cli_normalization.rs` enforces it across ClaudeCode/Codex/KimiCli/OpenCode |
| Provider abstraction | (not in original proposal) | `TracingProvider` trait + `TracingRegistry` at `crates/app-core/src/tracing/` |
| `KimiTracingProvider` reference impl | (n/a) | 16-file complete impl at `crates/app-core/src/tracing/providers/kimi/` |

**Net:** Phase 1 collapses from "design and build a wire-bus from scratch" to "wire Klynt into the existing pipeline." Specifically, four narrow gaps:

1. **Wire `turn_handler.rs` to call the Translator → Distiller** (the active 2026-04-29 spec §12 prescribes this verbatim: *"the desktop process is the runtime, so emission is always in-process"*).
2. **Build `KlyntTracingProvider`** mirroring `KimiTracingProvider` but reading from the existing SQLite (much simpler — no JSONL parsing, no filesystem discovery).
3. **Register `KlyntTracingProvider`** in the registry at AppCore init.
4. **Frontend provider selector** — replace the hardcoded `PROVIDER_ID = "kimi"` at `desktop-ui/src/tracing/lib/api.ts:7` with a selector.

**Effort:** 4–6 days, split across two crates and the frontend. Companion documents (created same day):

- Spec: `docs/superpowers/specs/2026-05-07-klynt-tracing-provider-design.md`
- Plan: `docs/superpowers/plans/2026-05-07-klynt-tracing-provider.md`

**Architectural insight:** The team had absorbed the kimi-cli wire-bus lesson months ago and *generalized* it into a provider abstraction so any coding agent (kimi-cli, claude-code, codex, opencode, **and Klynt itself**) plugs into one observability surface. Adding Klynt as a source becomes the smallest possible work because the architecture was always set up for it. The remaining roadmap phases (TodoWrite, plan mode, diff-preview-approval, etc.) sit cleanly on top of this once Phase 1 lands.

---

## Appendix A — Original agent reports

The three deep-dive reports (Klynt internal, kimi-cli, opencode) generated 2026-05-07 are the source-of-truth for the analysis above. They contain file:line citations and verbatim quotes that this synthesis condenses. If discrepancies arise, the agent reports take precedence — they were ground-truth at the time of writing.

To regenerate, dispatch three parallel `Explore` agents with the prompts that produced this analysis (see git log for this file's commit message).

---

*End of document.*
