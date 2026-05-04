# Klynt Coding-in-Chat — Phase 4 Design

**Date:** 2026-05-03
**Status:** Draft (pre-implementation)
**Scope:** Single design — codex/klyntbot UX reconciliation. The other Phase 3+ tracks (Windows sandbox, IDE bridge via MCP, Computer Use × coding integration, snapshot dedup beyond ghost-commits, cross-CLI memory unification, test infra hardening) are separate brainstorms.
**Pre-release policy:** Per CLAUDE.md — schema changes are made directly, no migration scripts, dev DB wipe is the migration. No backwards-compat shims.
**Builds on:**
- [`2026-04-29-klynt-coding-in-chat-design.md`](./2026-04-29-klynt-coding-in-chat-design.md) — Phase 1–3+ canonical spec.
- [`2026-04-30-tool-layer-consolidation-design.md`](./2026-04-30-tool-layer-consolidation-design.md) — Phase 1 amendments.
- [`2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md`](../plans/2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md) — ghost-commits + process hardening + truncation crates.
**Reference architectures consulted:**
- `/Users/jayden/Projects/Klynt/opencode` — multi-provider Go-based coding agent. Borrowed: Parts union, typed pub/sub broker, summary-as-pivot compaction, sub-session cost roll-up, ctx-threaded session IDs.
- `/Users/jayden/Projects/Klynt/codex` — Rust app-server using JSON-RPC. Borrowed: AGENTS.md walking, item-level streaming events, ApprovalDecision::AcceptWithExecpolicyAmendment, ephemeral-thread flag, append-only rollout (deferred). Dropped: ChatGPT OAuth + account model, v1 legacy enum, OpenAI-specific rate-limit dashboard.

---

## 1. Problem statement

The desktop UI was ported from a codex desktop fork and now invokes 30 Tauri commands (`start_thread`, `send_user_message`, `turn_steer`, `respond_to_server_request`, `fork_thread`, `compact_thread`, `list_threads`, `thread_live_subscribe`, `account_*`, `codex_login*`, `list_workspace_files`, `file_read`, `file_write`, etc.) that have **no Rust backend implementation**. Frontend bypasses the type-safe `bindings.ts` layer with raw `invoke<any>(...)` strings, so today's `bindings_are_current` test never caught it.

Concurrently, the Phase 1–3 spec described coding-mode as a per-thread toggle (`CodingModePill`, slash commands inline in composer, approval cards in messages) on the **chat** surface (`useKlyntbotSurfaceProps`). The current UI replaces that with a workspace-scoped codex-style thread surface (`MainApp.tsx::useMainAppThreadCodexState`) — UX diverged from spec without spec being updated.

Result: the user-facing coding flow is broken end-to-end. Two chat-shaped UIs coexist in code; one works (general chat), one is dangling (coding).

---

## 2. Goal, scope, success criteria

### Goal

Make the codex-derived coding surface (Project picker → Code/Assistant tab → Workspace thread → Composer) work end-to-end. End-to-end means: open a workspace, type a coding task, watch the agent run bash/edit/write tools, approve gated actions inline, see diffs render, and have the work persist + recall on follow-up turns. The original klyntbot chat surface (general chat, task/finance/note creation) stays untouched.

### Scope — in

- Implement 24 new klyntbot-native Tauri commands (drop the 5 OpenAI-specific account/login ones).
- Migrate `chat_messages` to a typed Parts union content model. Rename to `session_messages`. Both surfaces use it.
- Introduce the `approval_request` event + `approval_respond` command channel; bridge to existing 3-layer engine.
- Borrow opencode's typed broker pattern for Tauri events; codex's item-level event granularity.
- Walk parent dirs for `AGENTS.md` at thread start; inject as synthetic user message (codex-borrowed). Add global `~/.klyntbot/AGENTS.md` extension.
- Replace dropped account/login UI with klyntbot-native provider/model selector backed by existing `~/.klyntbot/config.json`.
- New build-time test `no_raw_invoke_in_endpoints.rs` to catch raw `invoke<any>` strings.
- Ship `coding-orchestrator` skill (new built-in skill in `bot/skills/`).

### Scope — out (deferred to later phases)

- Windows sandbox.
- IDE bridge via MCP (separate spec).
- Snapshot content-addressed dedup beyond ghost-commits.
- Computer Use × coding integration.
- Cross-CLI memory unification deepening.
- ChatGPT OAuth, codex account model, rate-limit dashboards (dropped permanently).
- JSONL rollout audit log (revisit if SQLite proves insufficient).
- Per-thread git worktrees (worktree dropdown stays read-only/`unknown`).
- `service_tier: fast | flex` (klyntbot doesn't model service tiers yet).

### Success criteria (measurable, falsifiable)

1. **Cold-start E2E.** A user can: import an existing local repo → start a coding thread → ask agent to add a function → approve a `bash cargo test` → see green diff → close + reopen → click thread to resume → ask follow-up that references prior work → recall correctly cites the earlier turn. Recorded as a GIF via Chrome MCP.
2. **No klyntbot chat regressions.** Existing tasks/finance/notes flows pass their tests with no changes. `cargo nextest run --workspace` zero warnings + `bun run test` green.
3. **Zero clippy warnings**, zero unused imports, `bindings_are_current` + new `no_raw_invoke_outside_endpoints` test pass.
4. Phase 1–3 invariants K1–K11 still hold (proptests pass). New invariants K12, K13, K14 added (see §11).
5. All 24 new Tauri commands typed via `klynt_command`/`klynt_raw_command` macros. `bindings.ts` regenerated. No `invoke<any>` calls remain in `desktop-ui/src/api/endpoints/`.

---

## 3. Surface model — two surfaces, unified data

| Layer | Klyntbot chat surface | Coding surface |
|---|---|---|
| **UI codepath** | `useKlyntbotSurfaceProps` (`appView === "chat"`); slash commands; ask_user inline | Codex-derived: project picker, thread view, AGENTS.md panel, ApprovalCard, diff viewer |
| **Composer** | Casual chat / brainstorm / task creation / finance / notes | Coding-tuned with workspace-aware features |
| **Tauri commands** | `chat_send`, `chat_messages`, `chat_respond_interaction`, `list_workspaces`, all `task_*`/`finance_*`/etc. — unchanged | New klyntbot-native set: `coding_thread_*`, `coding_message_send`, `coding_turn_*`, `approval_*`, `workspace_meta_*`, `workspace_file*`, `workspace_files_list`, etc. |
| **Data tables** | `sessions` + `session_messages` | Same `sessions` + `session_messages`, distinguished by `channel = "coding"` + `workspace_id IS NOT NULL` |
| **Tool registry** | `task`, `finance`, `notes`, `okr`, `productivity`, `learning`, `agent`, `cron`, `temporal`, etc. | `bash`, `read`, `write`, `edit`, `apply_patch`, `glob`, `grep`, `web_fetch`, `ask_user`, `enter_plan_mode`, `notebook_edit`, `recall_*`. Gated by `Tool::allowed_channels()` returning `ChannelMask::CODING` |
| **Cognitive feed** | Mirror, recall, FSRS, distiller all read from unified tables — both surfaces benefit equally |

The two surfaces are isolated at the UI codepath but share one data model and one cognitive backbone.

---

## 4. Data model

### Entities

```rust
// crates/storage/src/sessions.rs (existing, extended)
struct Session {
    id: SessionId,
    channel: ChannelName,                    // "chat" | "coding" | "telegram" | …
    workspace_id: Option<WorkspaceId>,       // NEW — set for coding sessions
    parent_session_id: Option<SessionId>,    // NEW — sub-agent cost roll-up
    forked_from_id: Option<SessionId>,       // NEW — fork lineage
    summary_message_id: Option<MessageId>,   // NEW — compaction pivot
    title: Option<String>,
    ephemeral: bool,                         // NEW — sub-agent / unit-test sessions skip persistence
    cost_usd: f64,
    prompt_tokens: i64,
    completion_tokens: i64,
    created_at: Timestamp,
    updated_at: Timestamp,
    starred: bool,                           // existing — K11 invariant input
    archived_at: Option<Timestamp>,          // NEW — for coding_thread_archive
}

// crates/storage/src/messages.rs (replacing current chat_messages.content: String)
struct Message {
    id: MessageId,
    session_id: SessionId,
    role: MessageRole,                       // existing — User | Assistant | Tool | System
    parts: Vec<MessagePart>,                 // NEW — was `content: String`
    model: Option<ModelId>,
    turn_id: Option<TurnId>,                 // NEW — UI grouping label only
    created_at: Timestamp,
    finish_reason: Option<FinishReason>,
}

enum MessagePart {
    Text { text: String },
    ToolCall { call_id: String, name: String, args: serde_json::Value },
    ToolResult { call_id: String, output: ToolOutput, is_error: bool },
    Reasoning { text: String, redacted: bool },
    FileChange { path: PathBuf, before: Option<String>, after: String, diff_unified: String, applied: bool },
    CommandExecution { command: Vec<String>, cwd: PathBuf, exit_code: Option<i32>, stdout: String, stderr: String },
    Finish { reason: FinishReason },
}
```

### Schema changes (consolidated, in-place per pre-release policy)

- `sessions` gains 6 new columns: `workspace_id`, `parent_session_id`, `forked_from_id`, `summary_message_id`, `ephemeral`, `archived_at`.
- `chat_messages.content TEXT` → `session_messages.parts JSONB`. New columns: `turn_id`, `finish_reason`.
- Table rename `chat_messages` → `session_messages` (drop "chat" prefix; both surfaces now use it).
- All callers reading `content: String` are rewritten to use `Vec<MessagePart>` with helpers `Message::extract_text()`, `Message::extract_tool_results()`.
- Display helpers live in new `crates/storage/src/messages/render.rs`.

### Cross-surface implications

- Klyntbot chat: messages with single `Text` Part render unchanged. Tool calls in chat (e.g., `task` tool) now properly surface as `ToolCall` Parts — small UX win.
- Coding surface: gets rich Parts (FileChange renders as diff, CommandExecution as terminal output).
- Cognitive subsystems read via `Message::extract_text()` (joins all `Text` Parts) and `extract_tool_results()`.

---

## 5. Tauri command surface

24 new commands (groups A–H), 5 dropped, existing chat surface untouched. All new commands use `#[klynt_command]` or `#[klynt_raw_command]` and register in `klynt_collect_commands![…]` so `bindings.ts` regenerates.

### Group A — Thread lifecycle (8)

| Command | Replaces | Payload | Returns |
|---|---|---|---|
| `coding_thread_start` | `start_thread` | `{ workspaceId, model?, approvalPolicy?, ephemeral?: bool }` | `Thread` |
| `coding_thread_resume` | `resume_thread` | `{ threadId, includeItems?: bool }` | `Thread` |
| `coding_thread_read` | `read_thread` | `{ threadId, cursor?, limit? }` | `Thread { items: Vec<Message> }` |
| `coding_thread_list` | `list_threads` | `{ workspaceId, cursor?, limit?, sortKey? }` | `Vec<ThreadSummary>` |
| `coding_thread_fork` | `fork_thread` | `{ threadId, fromMessageId? }` | `Thread` (`forked_from_id` set) |
| `coding_thread_compact` | `compact_thread` | `{ threadId }` | `{ summaryMessageId }` (opencode-style pivot) |
| `coding_thread_archive` | `archive_thread` | `{ threadId }` | `{}` (sets `archived_at`) |
| `coding_thread_set_name` | `set_thread_name` | `{ threadId, name }` | `{}` |

### Group B — Live subscription (2)

| Command | Replaces | Payload |
|---|---|---|
| `coding_thread_subscribe` | `thread_live_subscribe` | `{ threadId }` → `{ subscriptionId }` |
| `coding_thread_unsubscribe` | `thread_live_unsubscribe` | `{ subscriptionId }` → `{}` |

### Group C — Turn lifecycle (3)

| Command | Replaces | Payload |
|---|---|---|
| `coding_message_send` | `send_user_message` | `{ threadId, text, model?, effort?, accessMode?, images?, appMentions?, collaborationMode? }` |
| `coding_turn_interrupt` | `turn_interrupt` | `{ threadId, turnId }` |
| `coding_turn_steer` | `turn_steer` | `{ threadId, turnId, text, expectedTurnId }` |

### Group D — Approval (1 command)

| Command | Replaces | Payload |
|---|---|---|
| `approval_respond` | `respond_to_server_request` + `remember_approval_rule` | `{ approvalId, decision: "accept" \| "decline" \| "accept_for_session" \| "accept_with_execpolicy_amendment", execpolicyAmendment? }` |

### Group E — Workspace metadata (2)

| Command | Replaces | Payload | Returns |
|---|---|---|---|
| `workspace_meta_read` | `file_read` | `{ scope: "workspace" \| "global", kind: "agents" \| "config", workspaceId? }` | `{ exists, content, truncated }` |
| `workspace_meta_write` | `file_write` | `{ scope, kind, content, workspaceId? }` | `{}` |

### Group F — Workspace files (3)

| Command | Replaces | Payload |
|---|---|---|
| `workspace_files_list` | `list_workspace_files` + `fuzzyFileSearch` | `{ workspaceId, query?, limit? }` → `Vec<FuzzyFileMatch>` |
| `workspace_file_read` | `read_workspace_file` | `{ workspaceId, path }` → `{ content, truncated, mime, encoding }` |
| `text_file_write` | `write_text_file` | `{ path, content }` → `{}` |

### Group G — Utilities (2)

| Command | Replaces | Notes |
|---|---|---|
| `image_data_url` | `read_image_as_data_url` | `{ path }` → `String` (data URL) |
| `app_icon_read` | `get_open_app_icon` | `{ appName }` → `String?` |

### Group H — Code review + MCP + metadata (3)

| Command | Replaces | Notes |
|---|---|---|
| `coding_review_start` | `start_review` | `{ threadId, target, delivery? }` |
| `coding_mcp_status` | `list_mcp_server_status` | `{ workspaceId, cursor?, limit? }` |
| `coding_thread_metadata_generate` | `generate_run_metadata` | `{ workspaceId, prompt }` → `{ title, worktreeName }` |

### Dropped (5) — replaced by klyntbot's multi-provider model

| Dropped | Replacement |
|---|---|
| `account_rate_limits` | Per-provider usage display in Settings |
| `account_read` | `providers_list` + `provider_status` |
| `codex_login` | Per-provider config form in Settings |
| `codex_login_cancel` | N/A (config forms are synchronous) |
| `get_codex_config_path` | N/A — klyntbot uses `~/.klyntbot/config.json` |

UI replacement: composer's `Default model` reroutes to `providers_list`. Settings's "Account" tab becomes "Providers".

### Type-safety enforcement

New `crates/desktop/tests/no_raw_invoke_in_endpoints.rs` greps `desktop-ui/src/api/endpoints/**/*.ts` for `invoke<any>` or `invoke(` without generic parameter; fails build if found.

---

## 6. Tauri event surface

Per-thread events flow through one ordered channel — `agent:thread_event#<subscriptionId>` — with a discriminated union payload. Approvals get their own channel (`agent:approval_request`). Cost updates get `agent:cost_update`.

### `ThreadEvent` discriminated union

```rust
// crates/desktop-shared/src/coding/events.rs (new)
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThreadEvent {
    TurnStarted   { thread_id, turn_id, model, started_at },
    ItemStarted   { thread_id, turn_id, item: Message },
    ItemDelta     { thread_id, turn_id, item_id, part_idx: usize, delta: PartDelta },
    ItemCompleted { thread_id, turn_id, item: Message },
    ToolCallStarted   { thread_id, turn_id, item_id, call_id, tool: String },
    ToolCallCompleted { thread_id, turn_id, call_id, success: bool, duration_ms: u64 },
    FileChanged       { thread_id, turn_id, path, change: FileChangeKind },
    CommandExecuted   { thread_id, turn_id, command: Vec<String>, exit_code: Option<i32> },
    ContextCompressed { thread_id, turn_id, before_tokens: u64, after_tokens: u64 },
    TurnCompleted     { thread_id, turn_id, finish_reason, completed_at, duration_ms: u64 },
    Heartbeat         { subscription_id, server_time },
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartDelta {
    Text { append: String },
    Reasoning { append: String, redacted: bool },
    ToolCallArgs { json_patch: serde_json::Value },
    CommandStdout { append: String },
    CommandStderr { append: String },
    FileChangeProgress { bytes_written: u64 },
}
```

`PartDelta` is typed (not raw string) so the frontend reducer is one match-on-kind that updates the correct Part field.

### Subscription lifecycle

```
[UI] ──coding_thread_subscribe(threadId)──▶ [AppCore]
       ◀─── { subscriptionId } ─────────
[AppCore] ──app.emit("agent:thread_event#<subscriptionId>", event)──▶ [UI]
                        …
[UI] ──coding_thread_unsubscribe(subscriptionId)──▶ [AppCore]
       ◀─── {} ───────
```

Lifetime: auto-closes on (a) explicit unsubscribe, (b) thread archived, (c) heartbeat ack missing 3× consecutively, (d) AppCore shutdown.

Replay on resubscribe: snapshot via `coding_thread_resume` + live deltas via `coding_thread_subscribe`. UI reducer handles "snapshot + diff" — no event-loss windows.

### `agent:approval_request` channel (cross-thread)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRequest {
    CommandExecution {
        approval_id, thread_id, turn_id,
        command: Vec<String>, cwd: PathBuf, reason: String,
        proposed_execpolicy_amendment: Option<ExecpolicyAmendment>,
        layer_decisions: LayerDecisions,
    },
    FileChange {
        approval_id, thread_id, turn_id,
        path: PathBuf, diff_unified: String, write_kind: WriteKind,
        layer_decisions: LayerDecisions,
    },
    UserInput {
        approval_id, thread_id, turn_id,
        prompt: String, questions: Vec<UserInputQuestion>,
    },
}
```

`LayerDecisions` audit trail — exposes "why am I being asked?" — disclosure surfaced in approval card.

### `agent:cost_update` channel

```rust
pub struct CostUpdate {
    pub thread_id: Option<ThreadId>,
    pub provider: ProviderId,
    pub prompt_tokens_delta: u64,
    pub completion_tokens_delta: u64,
    pub usd_delta: f64,
    pub thread_total_usd: Option<f64>,
    pub ceiling_breached: bool,
}
```

### Typed broker

```rust
// crates/bus/src/typed_broker.rs (new)
pub struct TypedBroker<E: Clone + Send + 'static> {
    sender: tokio::sync::broadcast::Sender<E>,
}
```

Per-domain instances live in `AppCore`:
```rust
struct AppCore {
    thread_events: TypedBroker<ThreadEvent>,
    approval_events: TypedBroker<ApprovalRequest>,
    cost_events: TypedBroker<CostUpdate>,
    // existing DomainEventBus continues for TaskCreated, MirrorAlerted, etc.
}
```

Adapter task per surface fans typed broker → Tauri `app.emit`. Compile-time type safety up to that boundary.

---

## 7. Thread start, AGENTS.md walking, initial context assembly

### `coding_thread_start` server-side lifecycle

```
1. Load Workspace from `workspaces` table
2. Resolve provider+model (chain below)
3. Resolve approval policy (chain below)
4. Resolve sandbox profile (platform + workspace.path)
5. Walk AGENTS.md from workspace.path up to filesystem root
6. Build initial system prompt via ContextEngine (existing, extended)
7. Insert synthetic user message containing AGENTS.md bundle
8. Activate skills relevant to workspace languages (existing SkillRouter)
9. Persist Session row + return Thread { id, cwd, model, approvalPolicy, sandbox, instructionSources }
```

No turn fires until the user sends a message.

### AGENTS.md walking

```rust
// crates/coding-agents/src/agents_md.rs (new)
pub fn walk_agents_md(start: &Path) -> Vec<AgentsMdSource> {
    let mut sources = Vec::new();
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("AGENTS.md");
        if candidate.exists() {
            if let Ok(contents) = fs::read_to_string(&candidate) {
                sources.push(AgentsMdSource { path: candidate, dir: cur.clone(), contents });
            }
        }
        match cur.parent() {
            Some(parent) => cur = parent.to_path_buf(),
            None => break,
        }
    }
    sources.reverse(); // outermost first → innermost wins on conflicts
    sources
}
```

### Synthetic user message format (codex-borrowed for prompt-cache compat)

```
For each AgentsMdSource:
  # AGENTS.md instructions for <dir>

  <INSTRUCTIONS>
  <file contents verbatim>
  </INSTRUCTIONS>

Joined with double newlines into one synthetic user message inserted as messages[0].
```

The bundle is one `Message { role: User, parts: [Text { text: <bundle> }], turn_id: None /* synthetic */ }`. Persists in `session_messages` so resumes don't re-walk.

**Global AGENTS.md**: `~/.klyntbot/AGENTS.md` is added as a synthetic top-level entry with `dir = "<global>"`.

**Coexistence with KLYNTBOT.md**:
- `KLYNTBOT.md` = persona/tone. Read by `SoulContextSource` for **all** sessions, both surfaces.
- `AGENTS.md` = technical/coding guidance. Read by new `WorkspaceAgentsSource` only for `channel = "coding"`.
- Both arrive in the prompt: `KLYNTBOT.md` in system prompt (high priority); `AGENTS.md` as synthetic user message at head of `messages[]`.

The AGENTS.md bundle is built once at thread start, persisted, **not** rebuilt every turn. The existing `Refresh AGENTS.md` UI button calls `workspace_meta_read` and updates the synthetic message in place.

### Provider/model resolution chain

Most-specific wins:
```
1. coding_message_send.model     // per-message
2. Session.model                 // per-thread
3. Workspace.settings.model      // per-workspace
4. config.coding.defaults.model  // per-channel (NEW config key)
5. config.agents.defaults.model  // global (existing)
```

Same chain for `effort` and `accessMode`. `service_tier` dropped.

### Approval policy resolution chain

```
1. coding_message_send.approval_policy_override  // (rare; future addition)
2. coding_thread_start.approval_policy
3. Workspace.settings.approval_policy
4. config.coding.defaults.approval_policy
5. ApprovalPolicy::AskOnRisky                    // hardcoded default
```

Codex enum mapping:

| Codex name | Klyntbot name | Behavior |
|---|---|---|
| `always` | `AskAlways` | Every gated action prompts; Mirror cache disabled |
| `unless-allow-listed` | `AskOnRisky` | L1+L2+L3 fire; reach UI only when undecided |
| `on-failure` | `AskOnFailure` | Skip gates pre-execution; ask before retry |
| `never` | `YoloMode` | All gates bypassed except K4 (privacy guard inviolable) |

K4 invariant — `excludePaths` cannot be bypassed even by `YoloMode`. The YoloMode codepath asserts `privacy_guard.check(path).is_safe()` even when other gates are off.

### Sandbox profile resolution

| Platform | Profile |
|---|---|
| macOS | Seatbelt with workspace path read+write, `/tmp` read+write, parent-dir read-only, network only via configured providers |
| Linux | bwrap + seccomp + landlock |
| Windows | **Deferred.** `Sandbox::Disabled` with UI banner |

Workspace path validation refuses thread start if path is `/`, `~`, `/etc`, `/usr`, `/Users`, or contains `~/.ssh`, `~/.aws`, `~/.gnupg`.

### `coding-orchestrator` skill (new)

`bot/skills/coding-orchestrator/SKILL.md`. Activation keywords: "code", "implement", "fix", "refactor", "test", "build", "compile", "review", file-extension matches. Body: instructs agent on bash/edit/write/apply_patch/recall_* tool usage, when to commit, when to ask, when to use plan-mode. Coding analog to existing task-management/automation/learning skills.

### `instructionSources` returned to UI

```typescript
{
  id: string;
  workspaceId: string;
  cwd: string;
  model: string;
  approvalPolicy: ApprovalPolicy;
  sandbox: SandboxKind;
  instructionSources: { path: string; bytes: number; isGlobal: boolean }[];
  // ... other Thread fields
}
```

---

## 8. Message send, turn execution, tool dispatch, approval integration

### `coding_message_send` lifecycle

```
1. Resolve provider/model/policy chain (§7)
2. Append user Message to session_messages
3. Allocate turn_id; emit ThreadEvent::TurnStarted
4. Spawn Tokio task → ExecutionRouter::route(session, params)
   - returns { execution_mode, cancellation_token } stored in ActiveStreams
5. Return { turnId, turnStartedAt } to UI synchronously
6. Tokio task continues; events stream through TypedBroker → Tauri channel
```

### Two execution modes (existing)

- **Direct mode** — single LLM call, no tools. Pure questions.
- **Reactive mode (ReAct)** — iteration loop in `crates/agent/src/execution/core.rs`:

```
for iteration in 0..max_iterations {
    if cancellation_token.cancelled() { break with FinishReason::Cancelled }

    // 1. Live context refresh
    let context_update_msgs = live_refresher.drain_at_boundary(token_budget);

    // 2. Mid-loop compression at 70%
    if compressor.should_compress(messages) {
        messages = compressor.compress(messages);
        emit ThreadEvent::ContextCompressed { ... };
    }

    // 3. Provider call (streaming)
    emit ThreadEvent::ItemStarted { item: placeholder_assistant() };
    let stream = provider.stream(messages, tools).await?;
    let assistant_msg = consume_stream(stream, |delta| {
        emit ThreadEvent::ItemDelta { part_idx, delta };
    }).await?;
    emit ThreadEvent::ItemCompleted { item: assistant_msg.clone() };

    // 4. Tool dispatch (parallel, fan-out limited to MAX_CONCURRENT_TOOLS = 10)
    if assistant_msg.has_tool_calls() {
        let tool_results = dispatch_tools_parallel(assistant_msg.tool_calls()).await?;
        messages.extend(tool_result_messages);
        continue;
    }

    // 5. Done
    emit ThreadEvent::TurnCompleted { finish_reason: Completed };
    break;
}

if iteration_exhausted {
    let final_msg = synthesize_final(messages).await?;
    emit ThreadEvent::TurnCompleted { finish_reason: ToolCallsExhausted };
}
```

### Tool dispatch

```rust
async fn dispatch_tools_parallel(calls: Vec<ToolCall>) -> Vec<ToolResult> {
    let semaphore = global_tool_semaphore.clone(); // MAX_CONCURRENT_TOOLS = 10

    let tasks = calls.into_iter().map(|call| {
        let sem = semaphore.clone();
        tokio::spawn(async move {
            let _permit = sem.acquire_owned().await?;

            // Approval gate (3-layer; runs BEFORE tool.execute)
            let approval = approval_engine.evaluate(&call, ctx).await?;

            match approval {
                Approval::AutoApprove(_) => { /* proceed */ }
                Approval::AutoDeny(reason) => return ToolResult::denied(reason),
                Approval::AskUser => {
                    let decision = await_user_decision(&call, ctx).await?;
                    match decision {
                        Decision::Accept | Decision::AcceptForSession => { /* proceed */ }
                        Decision::Decline | Decision::Cancel => return ToolResult::declined(),
                        Decision::AcceptWithExecpolicyAmendment(rule) => {
                            execpolicy_repo.persist(rule)?;
                            // proceed
                        }
                    }
                }
            }

            emit ThreadEvent::ToolCallStarted { call_id, tool };
            let started_at = Instant::now();

            // Snapshot pre-state for mutation tools
            if call.tool.mutates_files() {
                snapshot_repo.try_record_with_ghost(&call.target_path).await?;
            }

            let timeout = if call.tool.is_interactive() { INTERACTIVE_TOOL_TIMEOUT } else { default_tool_timeout };
            let result = timeout(timeout, tool.execute(call, ctx)).await??;

            let truncated = truncate_to(&result, MAX_TOOL_RESULT_LENGTH);

            emit ThreadEvent::ToolCallCompleted { call_id, success: true, duration_ms: started_at.elapsed().as_millis() as u64 };
            ToolResult { ... }
        })
    });

    futures::future::join_all(tasks).await
}
```

Per-tool concurrency safety via `Tool::is_concurrency_safe()`. Per-path locks via `klynt-core::path_locks` for tools that mutate files.

### Approval engine (3-layer, existing)

```rust
async fn evaluate(call: &ToolCall, ctx: &ApprovalContext) -> Approval {
    // K4 invariant — privacy guard inviolable
    if privacy_guard.is_blocked(&call.target_path()) {
        return Approval::AutoDeny(reason: "Path matches privacy excludePaths".into());
    }

    // YoloMode short-circuit (after K4)
    if ctx.policy == ApprovalPolicy::YoloMode {
        return Approval::AutoApprove(reason: "yolo".into());
    }

    // Layer 1 — declarative allow/deny
    if let Some(decision) = layer1_declarative.evaluate(call) { return decision; }

    // Layer 2 — Starlark hook
    if let Some(decision) = layer2_starlark.evaluate(call, ctx).await? { return decision; }

    // Layer 3 — Mirror-learned
    if ctx.mirror_learning_enabled {
        if let Some(decision) = layer3_mirror.evaluate(call, ctx).await? { return decision; }
    }

    Approval::AskUser
}
```

When `Approval::AskUser` fires, runtime emits `agent:approval_request` with `LayerDecisions` audit trail showing why this approval reached the UI.

### Cancellation + steer

- **Cancellation** (`coding_turn_interrupt`) — looks up `CancellationToken` in `ActiveStreams`, fires `cancel()`. Observed at iteration boundaries. In-flight tools run to timeout. Emits `TurnCompleted { Cancelled }`.
- **Steer** (`coding_turn_steer`) — appends to a new `SteerQueue` (Tokio mpsc, one per active turn, owned by `ActiveStreams`) that the existing `LiveContextRefresher` drains at the next iteration boundary. Steer message arrives as synthetic user message in next iteration. Distinct from cancel.

### Cost ceiling integration

After every successful provider call, fires `agent:cost_update`. If `session.cost_usd > config.cost_ceiling.per_thread_usd`:
- Publishes `MirrorSignal::CostThresholdCrossed`
- If `hard_stop`, fires `cancellation_token.cancel()` and emits `TurnCompleted { CostCeilingReached }`

### Snapshot recording

Snapshots fire **before** every mutation tool execution via existing `snapshot_repo.try_record_with_ghost`. `snapshot_id` stored on `Message::Part::FileChange` so `/sessions rewind` can restore.

### Tool result Parts

A tool call generates **one** `Message` with `role: Tool` whose `parts` contains:
- `MessagePart::ToolResult` (canonical result with output text + isError flag)
- Optionally `MessagePart::FileChange` (one per file modified)
- Optionally `MessagePart::CommandExecution` (one per shell command)

UI renders single tool-result message containing stack of typed parts. Frontend tool-result component dispatches on part kind.

### What deliberately NOT changed

- `MidLoopCompressor` thresholds (70%, last 8 messages)
- `MAX_CONCURRENT_TOOLS = 10`
- `MAX_TOOL_RESULT_LENGTH = 50_000`
- `INTERACTIVE_TOOL_TIMEOUT = 600s`
- 3-layer approval engine internals (`klynt-core/src/approval/`)
- Skill routing logic in `SkillRouter`
- `ContextEngine::build_system_prompt()` source assembly

---

## 9. UI components, frontend state

### File map

```
desktop-ui/src/
├── api/endpoints/
│   ├── thread.ts            REWRITE → coding_thread_*, coding_message_*, coding_turn_*
│   ├── files.ts             REWRITE → workspace_meta_*, workspace_file_*, workspace_files_list, image_data_url, app_icon_read, text_file_write
│   ├── approval.ts          NEW    → approval_respond
│   ├── providers.ts         NEW    → providers_list, provider_status
│   └── (existing chat.ts, tasks.ts, finance.ts, …)   UNTOUCHED
├── features/
│   ├── coding/              REWRITTEN
│   │   ├── components/
│   │   │   ├── ProjectPicker.tsx           // existing; rewire to add_workspace
│   │   │   ├── ThreadView.tsx
│   │   │   ├── ThreadComposer.tsx          // rebrand label, keep behavior
│   │   │   ├── ThreadItemList.tsx
│   │   │   ├── parts/
│   │   │   │   ├── TextPart.tsx
│   │   │   │   ├── ToolCallPart.tsx
│   │   │   │   ├── ToolResultPart.tsx
│   │   │   │   ├── FileChangePart.tsx       // diff view
│   │   │   │   ├── CommandExecutionPart.tsx // terminal panel
│   │   │   │   ├── ReasoningPart.tsx
│   │   │   │   └── FinishPart.tsx
│   │   │   ├── ApprovalCard.tsx             // renders ApprovalRequest with LayerDecisions
│   │   │   ├── AgentsMdPanel.tsx            // workspace_meta_read + Refresh
│   │   │   └── CostCeilingBanner.tsx        // existing; listens agent:cost_update
│   │   └── hooks/
│   │       ├── useThreadEvents.ts
│   │       ├── useApprovals.ts
│   │       ├── useCodingThread.ts
│   │       └── useApplyThreadEvent.ts
│   ├── chat/                UNTOUCHED
│   └── settings/
│       └── components/
│           └── ProvidersSubsection.tsx     // NEW — replaces "Account" tab
├── services/
│   ├── __mocks__/tauri-browser-shim.ts     EXTEND
│   └── events.ts                            EXTEND
└── bindings.ts                              REGENERATED
```

### Per-thread state

```typescript
// useCodingThread(threadId) returns:
{
  thread: Thread | null;
  items: Message[];
  turnState: TurnState;       // { kind: "idle" | "streaming" | "awaiting_approval" | "tool_executing" }
  pendingApprovals: ApprovalRequest[];
  cost: CostSnapshot;
}
```

### Reducer (single source of truth)

```typescript
// useApplyThreadEvent.ts
type State = { items: Message[]; turnState: TurnState };

export function applyThreadEvent(state: State, event: ThreadEvent): State {
  switch (event.kind) {
    case "turn_started":   return { ...state, turnState: { kind: "streaming", turnId: event.turn_id, model: event.model } };
    case "item_started":   return { ...state, items: [...state.items, event.item] };
    case "item_delta":     return { ...state, items: applyDeltaToItem(state.items, event) };
    case "item_completed": return { ...state, items: replaceItem(state.items, event.item) };
    case "tool_call_started":   return { ...state, turnState: { kind: "tool_executing", call_id: event.call_id, tool: event.tool } };
    case "tool_call_completed": return state;
    case "file_changed":   return state;
    case "command_executed": return state;
    case "context_compressed": return { ...state, items: applyCompressionMarker(state.items, event) };
    case "turn_completed": return { ...state, turnState: { kind: "idle", lastFinishReason: event.finish_reason } };
    case "heartbeat":      return state;
  }
}
```

Pure function. Exhaustive (TypeScript catches missing variants). Unit-tested with fixture event sequences.

### Subscription lifecycle (UI)

```
[mount thread] → coding_thread_subscribe(threadId)
                ↓ subscriptionId
                ↓ listen(`agent:thread_event#${subId}`)
                ↓ snapshot via coding_thread_resume({ includeItems: true })
[user types]   → coding_message_send(...)
                ↓ events stream; reducer mutates state
[user cancels] → coding_turn_interrupt(turnId)
                ↓ TurnCompleted { Cancelled } arrives
[unmount]      → unlisten() + coding_thread_unsubscribe(subId)
```

---

## 10. Migration & rollout

Pre-release policy: **wipe `data.db` and `lance/`, no scripts, no backup, no conversion.**

```bash
rm -f  ~/.klyntbot/data.db ~/.klyntbot/data.db-wal ~/.klyntbot/data.db-shm
rm -rf ~/.klyntbot/lance/
# Preserves: config.json, KLYNTBOT.md, AGENTS.md, sessions/, workspace/, plugins/, personas/
```

Existing storage code panics with content-hash mismatch when migration 1 is edited — that's the pre-release contract per CLAUDE.md. No special Phase 4 detection needed.

`scripts/reset-dev-data.sh` ships as convenience:
```bash
#!/bin/bash
set -e
HOME_DIR="${KLYNTBOT_HOME:-$HOME/.klyntbot-dev}"
echo "Wiping $HOME_DIR/data.db + lance/"
rm -f "$HOME_DIR/data.db" "$HOME_DIR/data.db-wal" "$HOME_DIR/data.db-shm"
rm -rf "$HOME_DIR/lance/"
echo "Done. Config + sessions/ + KLYNTBOT.md + AGENTS.md preserved."
```

Browser-only mode caveats documented in CLAUDE.md update: native folder picker (mocked by browser shim), `set_tray_*` 404s, `local_usage_snapshot` 404 — these are dev_server scope, not Phase 4 bugs.

---

## 11. Testing strategy + invariants

### Unit (in-memory)

- `useApplyThreadEvent` reducer — fixture sequences for every event kind, edge cases
- Each `parts/<Kind>Part.tsx` component — render fixture parts, snapshot-test
- `WorkspaceAgentsSource::walk_agents_md` — temp-dir fixtures with nested AGENTS.md
- `coding_thread_start` payload validation — invalid workspace IDs, deleted paths, privacy-blocked paths
- `approval_respond` decision branches — each routed correctly

### Integration (cross-crate, real I/O)

- Full `coding_message_send` → `ItemStarted` → `ItemDelta` → `ItemCompleted` → `TurnCompleted` with mock provider
- Tool dispatch parallelism — 3 concurrent `bash` calls, verify ordering; per-path locks for 2 calls hitting same file
- Approval flow round-trip — request emitted, frontend simulates response, decision propagates
- Snapshot-before-mutation — verify `try_record_with_ghost` fires before edit/write/apply_patch
- AGENTS.md walking + injection — workspace with nested AGENTS.md; verify only ancestor-chain loaded
- Cancellation — fire mid-tool, verify turn ends with `Cancelled`, no orphaned tool executions

### Property-based (proptest)

| # | Invariant | Phase |
|---|---|---|
| K1–K11 | Existing (must still pass). K11 (starred never pruned) extends to: archived sessions are also never pruned. One-line proptest update covers both flags. | 1–2 |
| **K12** | **Subscription event ordering monotonicity.** For any sequence of events emitted to a subscription, the reducer's resulting state is identical regardless of arrival timing within the same iteration. | **4** |
| **K13** | **Privacy guard inviolability under YoloMode.** For any path matching `excludePaths`, even with `policy = YoloMode`, the approval engine returns `AutoDeny`. Strengthens K4. | **4** |
| **K14** | **AGENTS.md walk determinism.** For any directory tree, `walk_agents_md(start)` returns the same ordered list across runs (filesystem ordering normalized). | **4** |

### E2E (browser-only, recorded as GIF)

1. **Coding scenario.** Import repo → start thread → ask agent for a function → approve `bash cargo test` → verify diff renders → close + reopen → resume thread → follow-up question → recall cites prior turn.
2. **No-regression for klyntbot chat.** Start chat thread → ask "create task: buy milk" → verify task tool fires → verify `chat_send`/`chat_messages` paths unchanged → verify `chat_respond_interaction` for ask_user still works.

### Frontend (Vitest)

- Existing `tauri.test.ts` `file_read` mocks updated to mock `workspace_meta_read`
- New `useThreadEvents.test.tsx` — fixture event streams, assert state shape after each
- New `ApprovalCard.test.tsx` — renders for each `LayerDecisions` shape; clicking decision button calls `approval_respond` with correct payload

### Build-time enforcement

- New `crates/desktop/tests/no_raw_invoke_in_endpoints.rs` — fails if `invoke<any>` or `invoke(` without `<T>` found in `desktop-ui/src/api/endpoints/`
- Existing `bindings_are_current` continues
- Existing `no_raw_tauri_command_outside_macros` continues
- Zero clippy warnings

---

## 12. Open questions for the writing-plans phase

1. **Tool registry channel routing audit** — confirm every existing tool's `allowed_channels()` correctly includes/excludes `Coding`.
2. **`coding-orchestrator` skill content** — needs draft body; not architectural but ships with Phase 4.
3. **Concrete `LayerDecisions` UI design** — disclosure widget shape, copy text. Frontend-design territory.
4. **`text_file_write` permissions** — does this command need approval gating, or only when called via the `write` tool? Default: only via tool; the bare command is for save-dialog flows that already had user consent.
5. **`coding_thread_metadata_generate` model choice** — uses cheapest model (e.g. Haiku); specify which provider/model in the plan.

These are plan-level concrete details, not spec-level architectural decisions.

---

## 13. Component diagram

```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│                  klyntbot desktop (single Tauri 2 process)                            │
│                                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │                        desktop-ui (React, in webview)                            │  │
│  │                                                                                  │  │
│  │  ┌──────────────────────────┐         ┌────────────────────────────────────┐    │  │
│  │  │  Chat surface            │         │  Coding surface                    │    │  │
│  │  │  (useKlyntbotSurfaceProps)│        │  (useCodingThread + useThreadEvents)│   │  │
│  │  │                          │         │                                    │    │  │
│  │  │  • Composer (klyntbot)    │        │  • ProjectPicker                   │    │  │
│  │  │  • Slash commands         │        │  • ThreadView                      │    │  │
│  │  │  • ask_user inline       │         │  • ThreadComposer                  │    │  │
│  │  │  • task/finance/notes    │         │  • parts/* renderers               │    │  │
│  │  │                          │         │  • ApprovalCard (LayerDecisions)   │    │  │
│  │  │                          │         │  • AgentsMdPanel                   │    │  │
│  │  │                          │         │  • CostCeilingBanner               │    │  │
│  │  └────────────┬─────────────┘         └────────────────┬───────────────────┘    │  │
│  └───────────────┼──────────────────────────────────────┼────────────────────────┘  │
│                  │ Tauri IPC + Tauri events             │                              │
│                  │ chat_send / chat_messages /          │ coding_thread_* /            │
│                  │ chat_respond_interaction             │ coding_message_send /         │
│                  │ agent:* (existing)                   │ approval_respond /            │
│                  │                                      │ workspace_meta_* /            │
│                  │                                      │ workspace_file_* /            │
│                  │                                      │ agent:thread_event#<sub_id>/  │
│                  │                                      │ agent:approval_request /      │
│                  │                                      │ agent:cost_update             │
│                  ▼                                      ▼                              │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │                          AppCore handlers (Rust)                                 │  │
│  │  • chat_*: existing handlers (unchanged)                                         │  │
│  │  • coding_*, workspace_*, approval_*: NEW (Phase 4)                              │  │
│  │  • emit_updates → app.emit("agent:*")                                            │  │
│  │  • TypedBroker<ThreadEvent>, TypedBroker<ApprovalRequest>, TypedBroker<CostUpdate>│  │
│  └────────────────────────────────────┬────────────────────────────────────────────┘  │
│                                       ▼                                              │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │                    AgentRuntime (existing, unchanged internals)                  │  │
│  │   IntentAnalyzer → ContextEngine → SkillRouter → execute_loop                    │  │
│  │   For channel="coding":                                                          │  │
│  │     • coding tool registry (bash, edit, write, apply_patch, recall_*, …)         │  │
│  │     • CodingRecallService injected into ContextEngine                            │  │
│  │     • coding-aware skills (paths-conditional + dynamic discovery)                │  │
│  │     • WorkspaceAgentsSource (NEW — walks AGENTS.md at thread start)              │  │
│  └─┬───────┬───────┬──────┬──────┬──────────────┬─────────────────────────────────┘  │
│    │       │       │      │      │              │                                     │
│  ┌─▼─┐  ┌─▼─┐  ┌─▼──┐ ┌─▼──┐ ┌─▼──┐  ┌────────▼───────────┐                          │
│  │exe│  │san│  │hook│ │skl-│ │tool│  │ klynt-protocol     │                          │
│  │pol│  │box│  │s   │ │load│ │kit │  │  (event/op types)  │                          │
│  └───┘  └───┘  └────┘ └────┘ └────┘  └─────────────────────┘                          │
│                                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │  Cognitive subsystems (existing — read unified session_messages + Parts)         │  │
│  │   Distiller / Mirror (gains LayerDecisions audit) / Reforge / FSRS / Recall     │  │
│  └─────────────────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 14. Out-of-scope (consolidated)

- Windows sandbox (Phase 5+)
- Per-thread git worktrees (Phase 5+)
- IDE bridge via MCP (separate spec)
- Computer Use × coding integration (separate brainstorm)
- Snapshot content-addressed dedup beyond ghost-commits (Phase 5+)
- ChatGPT OAuth, codex account API, rate-limit dashboard (dropped permanently)
- JSONL rollout audit log (revisit if needed)
- `service_tier: fast | flex` (deferred)
- Cross-CLI memory unification deepening (separate brainstorm)
- Per-channel MCP allowlists already in Phase 3 — Phase 4 honors, doesn't extend
