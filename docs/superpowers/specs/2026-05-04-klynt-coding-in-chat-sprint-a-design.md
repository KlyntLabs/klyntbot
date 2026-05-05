# Klynt Coding-in-Chat — Sprint A: "Real Coding Mode" Polish

**Date:** 2026-05-04
**Status:** Draft (pre-implementation)
**Scope:** Five-track slice that closes the gap between today's working-skeleton coding mode (post-Phase 4) and a "feels like Codex Desktop / Claude Code Desktop" experience. NOT a full Phase 5 — picks the highest-leverage UX/UI items only.
**Pre-release policy:** Per CLAUDE.md — schema changes made directly, no migration scripts, dev DB wipe is the migration. No backwards-compat shims.
**Builds on:**
- [`2026-04-29-klynt-coding-in-chat-design.md`](./2026-04-29-klynt-coding-in-chat-design.md) — Phase 1–3+ canonical spec.
- [`2026-04-30-tool-layer-consolidation-design.md`](./2026-04-30-tool-layer-consolidation-design.md) — Phase 1 amendments.
- [`2026-05-03-klynt-coding-in-chat-phase4-design.md`](./2026-05-03-klynt-coding-in-chat-phase4-design.md) — Phase 4 codex/klyntbot UX reconciliation.
- [`2026-04-22-coding-memory-design.md`](./2026-04-22-coding-memory-design.md) — coding-memory layered phases (recall service, distiller, reforge phases 2.5/3.5).
**Reference benchmarks:**
- **Codex Desktop** — OpenAI's reference coding desktop. Strong points being matched: AGENTS.md transparency, subagent visibility, real review pass.
- **Claude Code Desktop** — Anthropic's reference. Strong points being matched: live recall surfacing prior turns, slash-command-driven review, polished progress UI.

---

## 1. Problem statement

Today's coding mode (post-Phase 4 commit `5985dfcd0`) is functionally end-to-end — approval flow unblocked, ThreadEvents stream over native IPC, ApprovalCard renders LayerDecisions disclosure, parts UI dispatches correctly. **But it feels like a demo, not a tool.**

Five gaps separate "demo-quality" from "Codex Desktop / Claude Code parity":

1. **Recall is fully stubbed.** All 8 `recall_*` tools (`recall_index`, `recall_timeline`, `recall_fetch`, `trace_causes`, `check_dead_ends`, `recall_facts_as_of`, `recall_change_history`, `recall_decision_points`) return `[recall stub: coding-memory not initialized]` from `crates/klynt-core/src/tools/recall_stubs.rs`. The agent has no memory of prior coding sessions — every conversation starts cold.
2. **`/review` returns canned strings.** `coding_review_start` in `crates/app-core/src/coding/review_handler.rs:46-53` returns `"Review of {target} — no issues found (stub)"`. There is no actual LLM-driven code review. Codex Desktop's `/review` slash command is a primary feature; ours is a mock.
3. **AGENTS.md is invisible.** Phase 4 added `WorkspaceAgentsSource` which walks the parent-dir chain, persists the bundle as a synthetic user message, and returns `instructionSources` to the UI in `coding_thread_start` — but the spec'd `AgentsMdPanel.tsx` was never built. Users have no way to see what context was loaded, no Refresh button, no edit affordance. Claude Code Desktop's "context loaded from CLAUDE.md" indicator is one of its most-praised UX details.
4. **Subagents run blind.** Phase 1's `SubagentManager` (`crates/agent/src/subagent.rs`) supports profile-bounded spawning (ReadOnly/ReadWrite/Full), semaphore-limited concurrency, cancel-by-id, and three-iteration limits. The infrastructure works. But: only one subagent event exists in `AgentEvent` (`SubagentSpawned { label, profile }`) — no `SubagentCompleted`, `SubagentProgress`, or `SubagentCancelled`. There is no UI surface at all in the coding thread view. When the agent spawns a subagent (which the agent CAN do today via `SpawnTool` registered at `crates/agent/src/agent_loop/builder.rs:632`), the user sees nothing.
5. **Realtime transport confusion.** Question raised: should the chat use WebSockets for "better UX"? Reality: native Tauri mode uses `app.emit` over OS-native IPC (Mach ports / named pipes / Unix sockets — sub-millisecond latency); browser-only dev mode uses Server-Sent Events over an axum HTTP server. Both are realtime. WebSocket would **add** overhead, not reduce it. This needs to be a documented architectural decision so it doesn't keep coming up.

Result: the platform CAN code, but the visible UX trails Codex Desktop / Claude Code Desktop on the four most-noticed surfaces (memory, review, context transparency, subagent visibility) and one architectural question keeps surfacing because it's never been documented.

---

## 2. Goal, scope, success criteria

### Goal

Match the user-visible quality of Codex Desktop and Claude Code Desktop on the five tracks above, in a single bounded sprint. Make the coding mode feel **production**, not **demo** — without expanding scope into Phase 5 platform-completion items (Windows sandbox, worktrees, snapshot dedup) or Phase 5 ecosystem items (marketplace, IDE bridge).

### Scope — in

| Track | What ships |
|---|---|
| **T1 — Live recall wiring** | Replace 8 `recall_*` stubs by registering `CodingMemoryToolset` after `ToolKitBuilder` in init order. Recall service `Phase 4` wiring becomes live. Mirror coding signals already wired (post-Phase 4) start producing real data. |
| **T2 — LLM-driven review** | Replace `coding_review_start` stub with a real review pass. Reuses agent loop with a review-specific system prompt + access to `bash`/`read`/`grep`/`recall_*`. Returns a structured `ReviewResult` with line-anchored issues. New slash command `/review [target]`. |
| **T3 — AgentsMdPanel** | New UI component showing the AGENTS.md bundle that was loaded at thread start. Refresh button calls existing `workspace_meta_*` commands; updates synthetic message in place. Inline source-list with byte counts + global/local origin. |
| **T4 — Subagent tray + lifecycle** | Add `SubagentCompleted`, `SubagentProgress`, `SubagentCancelled`, `SubagentSpawned` (extend existing) to `AgentEvent`. New `TypedBroker<SubagentEvent>` in AppCore. New Tauri channel `agent:subagent_event#<thread_id>`. New `SubagentTray.tsx` component shown inline in coding thread view with cancel buttons. New invariant K15. |
| **T5 — Realtime transport audit** | Documented architectural decision: stay with Tauri native events + dev-server SSE. No WebSocket layer. Add a benchmark crate that proves the latency claim. Settles the question permanently. |

### Scope — out (explicit non-goals for Sprint A)

- **File explorer / FileTree component** — Sprint B (Track B-1).
- **Live PTY panel** for streaming bash stdout — Sprint B (Track B-2). Today's `CommandExecutionPart` shows post-completion output only.
- **Diff staging / apply / revert UI** — Sprint B (Track B-3). `DiffPreview` renders but has no per-hunk staging.
- **Coding-memory reforge phases 2.5 / 3.5 bodies** — Sprint C (Track C-1). Phase 4 left these stubbed; coding signals don't yet feed nightly reforge synthesis. T1 lights up live recall, but full reforge integration is independent.
- **Coding-ingest Unix-socket transport** — Sprint C (Track C-2). Cross-CLI memory normalization works today via poll adapters; the hot path is a perf win, not a correctness gap.
- Windows sandbox (skipped per user direction — single-user macOS deployment).
- Skills.sh marketplace, IDE bridge via MCP, Computer Use × coding integration (Phase 5+ tracks).
- ChatGPT OAuth, codex account API, rate-limit dashboards (permanently dropped per Phase 4).

### Success criteria (measurable, falsifiable)

1. **Cold-start E2E "remembers prior turn".** A user opens the same workspace they used yesterday → asks "what did we change to fix the timezone bug?" → agent calls `recall_index` → cites at least one prior message turn from the previous coding session. Recorded as a GIF via Chrome MCP. (T1)
2. **Real review.** User runs `/review` after editing 3 files → review returns a `ReviewResult` with at least one structured `ReviewIssue` carrying a real `file`, `line`, and `description` (not a stub string). The review LLM call uses `bash`/`read`/`grep`/`recall_*` tools at least once during its iteration loop. (T2)
3. **AGENTS.md visible.** AgentsMdPanel renders all sources from `Thread.instructionSources` with paths + byte counts. Refresh button re-reads files via `workspace_meta_read` and updates the displayed list within 2s. (T3)
4. **Subagent visibility.** When the main agent spawns a subagent, a tray row appears in <100ms after `SubagentSpawned` fires; row updates on `SubagentProgress`; row resolves on `SubagentCompleted` or `SubagentCancelled`; cancel button fires `subagent_cancel` Tauri command and the subagent terminates within `INTERACTIVE_TOOL_TIMEOUT`. (T4)
5. **Realtime decision documented.** New file `docs/architecture/realtime-transport.md` exists, references the benchmark, and lands as part of this sprint. The Sprint A spec itself includes the analysis (§7). (T5)
6. **No regressions.** All Phase 1–4 invariants K1–K14 still pass. Two new invariants added: K15 (Subagent event ordering monotonicity), K16 (Recall stub shadowing — live registration replaces stubs by name without orphaning). `cargo nextest run --workspace` zero warnings + `bun run test` green. Zero clippy warnings.
7. **Codex Desktop / Claude Code parity check.** Side-by-side screenshot comparison of: (a) thread view with AGENTS.md panel, (b) approval card, (c) subagent tray, (d) review result. Sprint passes if a third party would not be able to identify which screenshot belongs to which app.

---

## 3. Track 1: Live recall wiring

### Current state

`crates/klynt-core/src/tools/recall_stubs.rs` defines 8 tools via the `recall_stub!` macro:

```rust
recall_stub!(RecallIndexTool,      "recall_index",      "Search coding-memory index");
recall_stub!(RecallTimelineTool,   "recall_timeline",   "Build chronological timeline from coding memory");
recall_stub!(RecallFetchTool,      "recall_fetch",      "Fetch full coding-memory entries by ID");
recall_stub!(TraceCausesTool,      "trace_causes",      "Trace causal graph from a memory entry");
recall_stub!(CheckDeadEndsTool,    "check_dead_ends",   "Check if an approach is a known dead end");
recall_stub!(RecallFactsAsOfTool,  "recall_facts_as_of", "Query facts as of a specific time");
recall_stub!(RecallChangeHistoryTool, "recall_change_history", "Recall change history for a file");
recall_stub!(RecallDecisionPointsTool,"recall_decision_points","List decision points in coding history");
```

Each returns `Ok("[recall stub: coding-memory not initialized]".into())`.

Meanwhile, `crates/coding-memory/src/mcp.rs` exposes the live implementation:

```rust
pub struct CodingMemoryToolset {
    svc: Arc<CodingRecallService>,
}

impl CodingMemoryToolset {
    pub fn new(svc: Arc<CodingRecallService>) -> Self { /* … */ }
    pub fn mcp_tools(&self) -> Vec<tools_core::DynTool> { /* 8 wrappers */ }
}
```

`CodingRecallService` is fully implemented (`crates/coding-memory/src/recall/mod.rs:245` total). The Phase 4 init code already constructs it (`app-core/src/init/mod.rs:1819` references `CodingAlertsQuery::new`), but the `mcp_tools()` are never registered into the agent's tool registry.

### Solution: register-after-shadow

Tool registries dedupe by name on registration. Register the live tools **after** the stubs in init order; the live tools shadow the stubs without modifying `recall_stubs.rs`. This preserves the stub schema for sub-agents that build their own registries (per the existing comment in `recall_stubs.rs:9` — "Registered in `ToolKitBuilder` so that sub-agents have the tool schema available even when the full `coding-memory` crate is not wired").

### Init wiring

In `crates/app-core/src/init/mod.rs`, after the `ToolKitBuilder::register_*` calls but before `core.agent.runtime().set_tool_kit(...)`:

```rust
// ── Sprint-A T1: shadow recall_* stubs with live CodingMemoryToolset ──
{
    let recall_svc = Arc::clone(&coding_recall_service);  // built earlier in init
    let toolset = coding_memory::CodingMemoryToolset::new(recall_svc);
    let mut registry_w = core.agent.runtime().tool_registry().write().await;
    for tool in toolset.mcp_tools() {
        registry_w.register_dyn(tool);  // overwrites stub by name
    }
    drop(registry_w);
    tracing::info!("Sprint-A T1: 8 recall_* stubs shadowed by live CodingMemoryToolset");
}
```

`register_dyn` already exists in `tools-core::ToolRegistry`. If overwrite-by-name is not the default, add `register_or_replace_dyn` (one-line addition to `tools-core/src/registry.rs`). Sub-agents continue to see stubs because their `ToolKitBuilder` runs without this shadowing pass — that's intentional: sub-agents shouldn't access the user's full coding memory by default.

### What lights up automatically

Once the live tools shadow:

- **`recall_index`** queries `RecallInvocationRepo` + the `CodingRecallService` index over `episodic_memories` + `semantic_memories` filtered by `repo_id` and the workspace path. Returns `RecallIndexResponse { entries: Vec<IndexEntry> }`.
- **`recall_timeline`** projects messages onto a chronological view of changed files / commands.
- **`check_dead_ends`** queries `dead_end_attempts` table for matching `problem_hash` (blake3-based; see `crates/coding-memory/src/problem_hash.rs`).
- **`trace_causes`** walks `memory_causal_edges` (Phase 6 wiring already exists).
- **`recall_facts_as_of`** uses `valid_from` / `valid_until` columns on facts (post-coding-memory Phase 7 schema).

The mirror signal sources (`CodingAlertsQuery`, pattern-effectiveness, stale-memory) registered at `init/mod.rs:572` start producing real `MirrorSignal::CodingAlertEmitted` events because they now have data to query.

### Frontend impact

`RecallTrayCard.tsx` already exists at `desktop-ui/src/features/coding/components/`. Today it would render the stub responses; post-T1 it renders real `IndexEntry` + `TimelineEntry` rows. Add one Vitest covering the new shape; the existing `RecallTrayCard.test.tsx` only tests skeleton rendering today.

### Telemetry

`RecallInvocationRepo` writes a row every time a `recall_*` tool fires. Sprint A wires a debug pane (Settings → Coding → Recall Stats) showing:
- Total invocations in last 7 days
- Top 5 recalled facts
- Mean latency

This makes recall *observable* — important for debugging "why did the agent forget?" complaints.

---

## 4. Track 2: LLM-driven review

### Current stub

`crates/app-core/src/coding/review_handler.rs:30-69`:

```rust
pub async fn coding_review_start(
    &self,
    thread_id: &str,
    target: Option<&str>,
    delivery: Option<&str>,
) -> Result<ReviewResult> {
    let session = self.repos.sessions.get_session(thread_id).await?;
    let _ = delivery;  // ignored
    let review_id = uuid::Uuid::new_v4().to_string();
    let summary = if let Some(t) = target {
        format!("Review of {t} — no issues found (stub)")
    } else { "Review of recent changes — no issues found (stub)".to_string() };
    Ok(ReviewResult { review_id, thread_id: session.key, summary, issues: vec![] })
}
```

### Replacement design

A review pass is a **scoped agent invocation** with:
1. A review-specific system prompt (declares the role + output JSON schema).
2. A constrained tool set (no `bash`, no `write`, no `edit` — read-only review).
3. A bounded iteration count (max 8).
4. A structured output schema enforced via the existing `IntentAnalyzer` JSON-mode flow.

This is intentionally **not a separate "review engine"** — that would duplicate the agent runtime. Instead it's a parameterized variant of `AgentRuntime::process` with a different `ExecutionParams`.

### Implementation

```rust
// crates/app-core/src/coding/review_handler.rs (replaces stub body)
pub async fn coding_review_start(
    &self,
    thread_id: &str,
    target: Option<&str>,
    delivery: Option<&str>,
) -> Result<ReviewResult> {
    let session = self.repos.sessions.get_session(thread_id).await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

    let review_id = Uuid::new_v4().to_string();
    let workspace_path = self.workspace_path_for_session(&session)?;

    // Build a review-scoped tool registry: read_only + recall_*; no mutating tools.
    let review_kit = self.tool_kit_for_review(&workspace_path)?;
    let review_registry = review_kit.build_read_only_registry();

    // System prompt — instructs LLM to emit ReviewResult JSON.
    let system_prompt = REVIEW_SYSTEM_PROMPT.replace(
        "{TARGET}",
        target.unwrap_or("recent changes in this thread"),
    );

    // Recent session messages → context.
    let recent_msgs = self.repos.session_messages
        .recent_for_session(&session.id, REVIEW_CONTEXT_TURN_LIMIT).await?;

    let params = ExecutionParams::review_pass()  // new constructor
        .with_max_iterations(REVIEW_MAX_ITER)
        .with_tool_timeout(REVIEW_TOOL_TIMEOUT)
        .with_response_format(ResponseFormat::JsonSchema(REVIEW_JSON_SCHEMA.clone()));

    let routing_ctx = RoutingContext::review_for_thread(&session, &workspace_path);

    let result = self.review_provider().chat_with_tools(
        system_prompt,
        recent_msgs,
        review_registry,
        params,
        routing_ctx,
    ).await?;

    let parsed: ReviewLlmOutput = serde_json::from_str(&result.content)?;

    let final_result = ReviewResult {
        review_id: review_id.clone(),
        thread_id: session.key.clone(),
        summary: parsed.summary,
        issues: parsed.issues.into_iter().map(Into::into).collect(),
    };

    self.repos.coding_reviews.insert(&final_result, &session.id).await?;
    self.emit_review_completed(&session.key, &final_result, delivery).await?;

    Ok(final_result)
}
```

### `REVIEW_SYSTEM_PROMPT` (verbatim)

```
You are a senior code reviewer. Your job is to review {TARGET} and produce a structured review.

You have access to read-only tools: read, list_dir, glob, grep, web_fetch, ask_user, recall_index, recall_timeline, check_dead_ends.

Process:
1. If reviewing recent changes, identify the changed files via the conversation history.
2. Read the changed files in full.
3. Optionally use `recall_*` to check for similar patterns or known dead-ends.
4. Identify concrete issues: bugs, security risks, style violations, missing tests, brittle patterns.
5. For each issue, cite file + line + a one-sentence description and (when actionable) a suggestion.
6. End with a one-paragraph summary.

Output ONLY the following JSON object — no commentary, no markdown fences:

{
  "summary": "<one paragraph>",
  "issues": [
    {
      "severity": "info" | "warning" | "error",
      "file": "<relative path>" | null,
      "line": <number> | null,
      "description": "<one sentence>",
      "suggestion": "<one sentence>" | null
    }
  ]
}

Severity guidance:
- "error":  bugs, data loss, security holes, broken APIs, race conditions
- "warning": brittle patterns, missing error handling, unclear ownership
- "info":   style nits, suggestions for improvement, optional enhancements

If you find no issues, return { "summary": "...", "issues": [] }. Do not invent issues to fill space.
```

### Provider/model resolution

Reuses the chain from Phase 4 §7:
1. `coding_review_start.model_override` (future addition)
2. `Workspace.settings.review_model` (new optional field)
3. `config.coding.review.defaults.model`
4. `config.coding.defaults.model`

Default review model: **Haiku-tier** (e.g., `claude-haiku-4-5-20251001`) — review is a high-frequency operation; cheap model is right. Configurable per workspace.

### Constants (new in `app-core/src/coding/review_handler.rs`)

```rust
const REVIEW_MAX_ITER: u32 = 8;
const REVIEW_CONTEXT_TURN_LIMIT: u32 = 20;
const REVIEW_TOOL_TIMEOUT: Duration = Duration::from_secs(60);
```

### Schema addition

New table `coding_reviews`:
```sql
CREATE TABLE coding_reviews (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL,
    summary       TEXT NOT NULL,
    issues_json   TEXT NOT NULL,         -- serde_json of Vec<ReviewIssue>
    target        TEXT,                  -- nullable
    delivery      TEXT,                  -- "inline" | "detached" | NULL
    created_at    TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);
CREATE INDEX idx_coding_reviews_session ON coding_reviews(session_id, created_at DESC);
```

Per pre-release policy, this is added to migration 1 (consolidated) — not a new migration file.

### Slash command surface

Add `/review` to the slash registry in `desktop-ui/src/features/coding/slash/registry.ts`:

```typescript
{ name: "review",
  classification: "agent-routed",   // dispatches via Tauri invoke
  description: "Run a code review on the current thread or a target file/diff",
  argsHint: "[target]",
  handler: async (args) => invoke<ReviewResult>("coding_review_start", {
    threadId: currentThreadId(),
    target: args ?? null,
    delivery: "inline",
  }),
}
```

### UI surfacing

New part variant: `MessagePart::ReviewResult { review_id, summary, issues }` rendered by a new `ReviewResultPart.tsx`. Issues group by severity; clicking a file+line opens the workspace file at that line via `workspace_file_read`.

### Why not a separate "review tool"?

We considered registering `coding_review_start` as a `Tool::Review` callable by the agent during normal turns. Rejected: review is a **user-initiated** operation that benefits from a different system prompt and tool set than normal coding. Mixing them risks the agent triggering a review mid-implementation and stalling on iteration. Slash command + API-only is cleaner.

---

## 5. Track 3: AgentsMdPanel

### Current state

- `crates/coding-agents-md/src/lib.rs` exposes `WorkspaceAgentsSource` which walks `AGENTS.md` from workspace root upward + optional global `~/.klyntbot/AGENTS.md`.
- `coding_thread_start` returns `Thread.instructionSources: { path, bytes, isGlobal }[]` per Phase 4 §7.
- `workspace_meta_read` Tauri command exists, takes `{ scope, kind, workspaceId? }`.
- **No `AgentsMdPanel.tsx` exists.** The `desktop-ui/src/features/coding/components/` directory has 14 components but none for AGENTS.md.

### Component design

```
AgentsMdPanel
├── header
│   "Loaded context"  [count chip]  [Refresh]
├── source list (collapsed by default)
│   • <span class="origin-pill">global</span> ~/.klyntbot/AGENTS.md  · 1.2 KB
│   • <span class="origin-pill">root</span>   bot/AGENTS.md         · 3.4 KB
│   • <span class="origin-pill">nested</span> bot/crates/coding/AGENTS.md · 870 B
├── (expanded) per-source preview: first 200 chars + "Open file" button
└── footer: "Last refreshed at hh:mm"
```

### File: `desktop-ui/src/features/coding/components/AgentsMdPanel.tsx`

```typescript
import { useState, useEffect } from "react";
import { invoke } from "@/api/client";
import type { Thread, AgentsMdSource } from "@/bindings";

type Props = {
  thread: Thread;
  workspaceId: string;
  onRefreshed?: () => void;
};

export function AgentsMdPanel({ thread, workspaceId, onRefreshed }: Props) {
  const [sources, setSources] = useState(thread.instructionSources);
  const [expanded, setExpanded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [lastRefreshedAt, setLastRefreshedAt] = useState<Date | null>(null);

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      const updated = await invoke<AgentsMdSource[]>("coding_thread_refresh_agents_md", {
        threadId: thread.id,
      });
      setSources(updated);
      setLastRefreshedAt(new Date());
      onRefreshed?.();
    } finally { setRefreshing(false); }
  };

  if (sources.length === 0) {
    return (
      <aside className="agents-md-panel agents-md-panel--empty">
        No AGENTS.md found in workspace ancestor chain.
      </aside>
    );
  }

  return (
    <aside className="agents-md-panel" aria-label="Loaded AGENTS.md context">
      <header>
        <button onClick={() => setExpanded(v => !v)} aria-expanded={expanded}>
          Loaded context <span className="count">{sources.length}</span>
        </button>
        <button onClick={handleRefresh} disabled={refreshing}>
          {refreshing ? "Refreshing…" : "Refresh"}
        </button>
      </header>
      {expanded && (
        <ol>
          {sources.map(src => (
            <li key={src.path}>
              <span className={`origin origin--${originKind(src)}`}>{originLabel(src)}</span>
              <code>{src.path}</code>
              <span className="bytes">{formatBytes(src.bytes)}</span>
            </li>
          ))}
        </ol>
      )}
      {lastRefreshedAt && (
        <footer>Last refreshed at {lastRefreshedAt.toLocaleTimeString()}</footer>
      )}
    </aside>
  );
}
```

### New Tauri command — `coding_thread_refresh_agents_md`

```rust
// crates/desktop/src/commands/coding_thread.rs (additional command)
#[klynt_command]
pub async fn coding_thread_refresh_agents_md(thread_id: String) -> Vec<AgentsMdSource> {
    core.coding_thread_refresh_agents_md(&thread_id).await
}
```

```rust
// crates/app-core/src/coding/thread_handler.rs (new method)
#[tracing::instrument(skip(self), err)]
pub async fn coding_thread_refresh_agents_md(&self, thread_id: &str) -> Result<Vec<AgentsMdSource>> {
    let session = self.repos.sessions.get_session(thread_id).await?;
    let workspace_id = session.workspace_id.ok_or_else(|| KlyntbotError::InvalidArgument(
        "thread is not workspace-scoped".into()))?;
    let workspace = self.repos.workspaces.get(&workspace_id).await?;
    let global_path = self.config.read().await.paths.klyntbot_home().join("AGENTS.md");

    let source = WorkspaceAgentsSource::new(workspace.path.clone())
        .with_global(global_path);
    let new_sources = source.walk();
    let bundle = source.build_bundle();

    // Update the synthetic message in place (don't append).
    if let Some(bundle_text) = bundle {
        self.repos.session_messages
            .update_synthetic_agents_md(&session.id, &bundle_text).await?;
    }

    Ok(new_sources)
}
```

### Mounting

In `desktop-ui/src/features/coding/components/ThreadView.tsx` (or equivalent — Phase 4 plan deferred this; check exact name):

```tsx
<aside className="thread-side">
  <AgentsMdPanel thread={thread} workspaceId={workspaceId} />
  <RecallTrayCard threadId={thread.id} />
  <SubagentTray threadId={thread.id} />            {/* T4 */}
</aside>
```

### CSS (BEM-ish, follows desktop-ui conventions)

Add `desktop-ui/src/styles/agents-md-panel.css`; import via `src/styles/index.css`. Uses existing tokens from `ds-tokens.css` (`--fs-xs` for source rows, `--fs-base` for header). No hardcoded font sizes per CLAUDE.md.

### Testing

- Unit: `AgentsMdPanel.test.tsx` — empty state, populated state, expanded state, refresh action
- Integration: refresh updates synthetic message; Vitest with mocked `invoke`
- Backend: `coding_thread_refresh_agents_md` — workspace lookup failure, global file missing, nested AGENTS.md update

---

## 6. Track 4: Subagent tray + lifecycle events

### Current state

`crates/agent/src/events.rs` has exactly **one** subagent event:
```rust
SubagentSpawned { label: String, profile: String },
```

`SubagentManager` (`crates/agent/src/subagent.rs`) has the full lifecycle but emits no events when subagents complete or are cancelled. The internal `handles` HashMap tracks them; from outside, you can call `cancel_subagent(agent_id)` but you can't observe completion.

### Lifecycle events to add

```rust
// crates/agent/src/events.rs (extending AgentEvent)
SubagentSpawned   { agent_id: String, label: String, profile: String, parent_session_id: String, spawned_at: Timestamp },
SubagentProgress  { agent_id: String, iteration: u32, last_tool: Option<String> },
SubagentCompleted { agent_id: String, success: bool, summary: String, tokens_used: u64, duration_ms: u64 },
SubagentCancelled { agent_id: String, reason: SubagentCancelReason, cancelled_at: Timestamp },
```

`SubagentSpawned` gains `agent_id`, `parent_session_id`, `spawned_at` — needed for UI correlation. Existing single-arm consumers (none today, per `grep`) update their match.

### `SubagentEvent` discriminated union (`desktop-shared`)

Mirror the `ThreadEvent` pattern from Phase 4:

```rust
// crates/desktop-shared/src/coding/subagent.rs (new)
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubagentEvent {
    Spawned   { agent_id, label, profile, parent_session_id, spawned_at },
    Progress  { agent_id, iteration: u32, last_tool: Option<String> },
    Completed { agent_id, success: bool, summary: String, tokens_used: u64, duration_ms: u64 },
    Cancelled { agent_id, reason: SubagentCancelReason, cancelled_at },
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SubagentCancelReason {
    UserRequested,
    Timeout,
    ParentCancelled,
    PolicyViolation,
}
```

### `TypedBroker<SubagentEvent>` in `AppCore`

Following Phase 4 §6 pattern:

```rust
struct AppCore {
    thread_events: TypedBroker<ThreadEvent>,
    approval_events: TypedBroker<ApprovalRequest>,
    cost_events: TypedBroker<CostUpdate>,
    subagent_events: TypedBroker<SubagentEvent>,  // NEW
    /* … */
}
```

### `SubagentManager` event emission

Modify `crates/agent/src/subagent.rs::run_subagent_task` (line 481) to:
1. Take a `subagent_event_tx: broadcast::Sender<SubagentEvent>` parameter (added to `SubagentConfig`).
2. Emit `Spawned` at start.
3. Emit `Progress` at each iteration boundary in the inner agent loop.
4. Emit `Completed { success: true }` on normal exit.
5. Emit `Completed { success: false }` on error.
6. Emit `Cancelled` when cancel_token observed.

`SubagentManagerBuilder::subagent_event_sender(tx)` is the new builder method. Wired in `init_agent` (`crates/app-core/src/init/agent.rs`).

### Tauri event bridge

Following Phase 4's adapter pattern, add to `crates/app-core/src/coding/subscription.rs`:

```rust
impl SubscriptionManager {
    pub fn fan_subagent_events_to_tauri(&self, app: tauri::AppHandle) {
        let mut rx = self.broker.subagent_events.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let parent = match &event {
                    SubagentEvent::Spawned { parent_session_id, .. } => parent_session_id.clone(),
                    SubagentEvent::Progress { agent_id, .. }
                    | SubagentEvent::Completed { agent_id, .. }
                    | SubagentEvent::Cancelled { agent_id, .. } => {
                        // lookup parent_session_id from agent_id registry
                        registry.parent_for(agent_id).await.unwrap_or_default()
                    }
                };
                let _ = app.emit(
                    &format!("agent:subagent_event#{}", parent),
                    &event,
                );
            }
        });
    }
}
```

### New Tauri commands

| Command | Payload | Returns |
|---|---|---|
| `subagent_list_active` | `{ threadId }` | `Vec<SubagentSummary>` |
| `subagent_cancel` | `{ agentId }` | `{}` |
| `subagent_inspect` | `{ agentId }` | `{ messages: Vec<Message>, tokensUsed, durationMs }` |

`SubagentSummary` is a thin DTO — id, label, profile, iteration, status, started_at, last_tool.

### Frontend — `SubagentTray.tsx`

```typescript
// desktop-ui/src/features/coding/components/SubagentTray.tsx
type Props = { threadId: string };

export function SubagentTray({ threadId }: Props) {
  const { activeSubagents, cancel } = useSubagents(threadId);

  if (activeSubagents.length === 0) return null;

  return (
    <aside className="subagent-tray" aria-label="Active subagents">
      <header>Subagents <span className="count">{activeSubagents.length}</span></header>
      <ol>
        {activeSubagents.map(sa => (
          <li key={sa.agentId} className={`subagent-row subagent-row--${sa.status}`}>
            <span className="profile">{sa.profile}</span>
            <span className="label">{sa.label}</span>
            <span className="iteration">iter {sa.iteration}</span>
            {sa.lastTool && <span className="last-tool">{sa.lastTool}</span>}
            <span className="duration">{formatDuration(sa.durationMs)}</span>
            {sa.status === "running" && (
              <button
                type="button"
                onClick={() => cancel(sa.agentId)}
                title="Cancel subagent"
              >Cancel</button>
            )}
          </li>
        ))}
      </ol>
    </aside>
  );
}
```

### `useSubagents` hook

```typescript
// desktop-ui/src/features/coding/hooks/useSubagents.ts
export function useSubagents(threadId: string) {
  const [activeSubagents, setActive] = useState<SubagentSummary[]>([]);

  useEffect(() => {
    invoke<SubagentSummary[]>("subagent_list_active", { threadId }).then(setActive);
    const unlisten = listen<SubagentEvent>(`agent:subagent_event#${threadId}`, e => {
      setActive(prev => applySubagentEvent(prev, e.payload));
    });
    return () => { unlisten.then(fn => fn()); };
  }, [threadId]);

  const cancel = useCallback(
    (agentId: string) => invoke("subagent_cancel", { agentId }),
    [],
  );

  return { activeSubagents, cancel };
}

function applySubagentEvent(prev: SubagentSummary[], e: SubagentEvent): SubagentSummary[] {
  switch (e.kind) {
    case "spawned":   return [...prev, summarize(e)];
    case "progress":  return prev.map(s => s.agentId === e.agent_id ? { ...s, iteration: e.iteration, lastTool: e.last_tool } : s);
    case "completed": return prev.filter(s => s.agentId !== e.agent_id);  // remove on completion
    case "cancelled": return prev.filter(s => s.agentId !== e.agent_id);
  }
}
```

### Visual model

Cribs from `desktop-ui/src/tracing/features/wire-viewer/timeline-view.tsx` — a pre-existing post-mortem subagent visualization. Reuse the row layout + icons; differences:
- Live (no scrubbing).
- Cancel button per row.
- No deep-dive (that's the tracing inspector's job).

### Invariant K15 — Subagent event ordering monotonicity

> For any `agent_id`, the sequence of received `SubagentEvent` values is a prefix of: `Spawned`, then zero or more `Progress`, then exactly one of `Completed`/`Cancelled`. No `Progress` arrives after a terminal event. No `Spawned` is repeated.

Tested via proptest in `crates/agent/tests/subagent_event_ordering.rs`. Generates random tokio task interleavings; asserts the property over the broker output.

---

## 7. Track 5: Realtime transport — documented architectural decision

### The question

> "Should the chat use WebSockets for better realtime UX?"

### The current state — measured

Two transports today:

| Mode | Transport | Path |
|---|---|---|
| **Tauri native** (production) | OS-native IPC via `app.emit()` | Mach ports (macOS) / Named pipes (Win) / Unix sockets (Linux) |
| **Browser dev** (`localhost:1420` against `:3456`) | Server-Sent Events over HTTP/1.1 keep-alive | axum routes `sse_handler`, `cognitive_sse_handler`, `insight_sse_handler`, `global_sse_handler` |

Both are realtime. Both are unidirectional (server → UI). User input goes via `invoke()` (Tauri command in native, HTTP POST in browser dev).

### Latency characterization (to be measured in Sprint A)

A new bench `crates/desktop/benches/event_transport_latency.rs` will measure event delivery from `app.emit()` call to UI handler entry, for:

1. Tauri native event (`app.emit` → `listen()`)
2. Dev-server SSE (axum SSE route → `EventSource.onmessage`)
3. *Hypothetical* WebSocket (axum WS route → `WebSocket.onmessage`)

Target measurements (to be confirmed by the bench):

| Transport | Expected p50 | Expected p99 |
|---|---|---|
| Tauri native IPC | < 200 µs | < 1 ms |
| Dev-server SSE | < 2 ms | < 10 ms (HTTP framing) |
| Hypothetical WS | < 2 ms | < 10 ms |

WebSocket is **not faster** than SSE for this workload (single-direction streaming) and is **3-5× slower** than native Tauri IPC. There is no UX-measurable benefit from switching.

### When WebSocket WOULD matter

Three scenarios — none apply to klyntbot today:

1. **Remote agent server.** If klyntbot ran on a remote server with browser clients — WebSocket would compete with SSE on equal footing. We don't ship that mode.
2. **Bidirectional realtime** (live cursors, collaborative editing, shared cursors). Coding chat is single-user single-thread.
3. **Frequent client→server pushes** (>10 Hz from UI to backend). User typing in the composer is debounced; tool approvals are user-pace.

### Decision

**Stay with the current architecture.** Document this in `docs/architecture/realtime-transport.md` (new file shipped with this sprint). No code change for T5 except:

1. The benchmark crate (proves the latency claim).
2. The architecture doc (settles the question).
3. A small refinement to dev-server SSE: switch from `axum::sse::KeepAlive::default()` to `KeepAlive::new().interval(Duration::from_secs(15))` to reduce the keep-alive frequency. This is a 1-line change.

### What this is NOT

- NOT a rejection of WebSocket forever. If we ship a remote agent (Phase 5+), revisit.
- NOT a claim that SSE is intrinsically better. The point is **the right transport for the workload** — and for a Tauri 2 desktop app with optional browser dev mode, native IPC + SSE is correct.

---

## 8. Data model & event surface additions

### Schema (consolidated, in-place per pre-release policy)

- `coding_reviews` table (per §4).
- No other table changes.

### `MessagePart` enum extension

```rust
// crates/storage/src/messages/parts.rs (extending Phase 4)
pub enum MessagePart {
    /* … existing variants … */
    ReviewResult { review_id: String, summary: String, issues: Vec<ReviewIssue> },  // NEW (T2)
}
```

### `AgentEvent` extension

Three new variants per §6.

### `SubagentEvent` discriminated union — new

In `desktop-shared/src/coding/subagent.rs` per §6.

---

## 9. Tauri command surface additions

| Command | Track | Payload | Returns |
|---|---|---|---|
| `coding_thread_refresh_agents_md` | T3 | `{ threadId }` | `Vec<AgentsMdSource>` |
| `subagent_list_active` | T4 | `{ threadId }` | `Vec<SubagentSummary>` |
| `subagent_cancel` | T4 | `{ agentId }` | `{}` |
| `subagent_inspect` | T4 | `{ agentId }` | `SubagentDetail` |
| `coding_recall_stats` | T1 | `{ workspaceId, days? }` | `RecallStats` |

5 new commands. All use `#[klynt_command]`. All registered in `klynt_collect_commands![…]`. `bindings.ts` regenerated.

`coding_review_start` already exists (Phase 4 Group H); only its handler body changes.

---

## 10. UI components & frontend state

### New components

```
desktop-ui/src/features/coding/components/
├── AgentsMdPanel.tsx           # T3
├── AgentsMdPanel.test.tsx
├── SubagentTray.tsx            # T4
├── SubagentTray.test.tsx
├── SubagentRow.tsx             # T4 — extracted for clean tests
├── parts/
│   └── ReviewResultPart.tsx    # T2
└── (existing components untouched)
```

### New hooks

```
desktop-ui/src/features/coding/hooks/
├── useSubagents.ts             # T4
├── useSubagents.test.ts
├── useAgentsMd.ts              # T3 — small wrapper over coding_thread_refresh_agents_md
└── useReview.ts                # T2 — wrapper over coding_review_start + listen for ReviewResultPart updates
```

### Settings additions

```
desktop-ui/src/features/settings/components/
└── CodingRecallStats.tsx       # T1 — debug pane for recall observability
```

### CSS

```
desktop-ui/src/styles/
├── agents-md-panel.css         # T3
├── subagent-tray.css           # T4
└── review-result-part.css      # T2
```

All `@import`-ed via `src/styles/index.css`. Uses existing `ds-tokens.css` (typography scale, spacing, color). No hardcoded sizes.

---

## 11. Migration & rollout

Pre-release policy: **wipe `data.db` and `lance/`, no scripts, no backup, no conversion.**

```bash
rm -f  ~/.klyntbot/data.db ~/.klyntbot/data.db-wal ~/.klyntbot/data.db-shm
rm -rf ~/.klyntbot/lance/
```

Migration 1 of `coding_review` feature gets a new `coding_reviews` table SQL block; the existing `FeatureMigration` version stays at 1 (Phase 4 plan ships the consolidated migration). If Phase 4's migration version is already finalized, bump to 2 with a single ADD-TABLE statement — but per the pre-release policy, in-place edit is fine.

`scripts/reset-dev-data.sh` (already shipping with Phase 4) covers the wipe.

---

## 12. Testing strategy + invariants

### Unit (in-memory)

- T1: registry shadowing — verify `recall_index` resolves to live tool, not stub, after init
- T2: review handler — mocked provider, fixture session, verify `ReviewResult` shape
- T3: `AgentsMdPanel` — empty / populated / expanded / refresh
- T3: `coding_thread_refresh_agents_md` — re-walks chain, updates synthetic message
- T4: `SubagentEvent` reducer — unit-test `applySubagentEvent` with fixture sequences
- T4: `SubagentTray` — empty list, populated list, cancel button click
- T4: `SubagentManager::run_subagent_task` — emit ordering with mock broker

### Integration (cross-crate, real I/O)

- T1: real `CodingRecallService` over in-memory SQLite, populate episodic_memories, query via `recall_index` tool, assert non-empty result
- T2: end-to-end `/review` — workspace with 3 changed files, mock provider returning fixture JSON, assert `ReviewResult` persisted in `coding_reviews` and emitted
- T4: spawn subagent via tool call → events flow through TypedBroker → fan to mock Tauri sink → reducer state matches expected sequence
- T4: cancel subagent → `Cancelled { reason: UserRequested }` emitted, subagent process exits

### Property-based (proptest)

| # | Invariant | Track |
|---|---|---|
| K15 | Subagent event ordering monotonicity (per §6) | T4 |
| K16 | Recall stub shadowing — registering the live `CodingMemoryToolset` after the stubs results in `registry.lookup("recall_index").type_id() == TypeId::of::<CodingMemoryMcpTool>()`. Stubs are unreachable from the main agent's registry post-init. | T1 |
| K17 | Review pass purity — a `coding_review_start` invocation never causes `MessagePart::FileChange`, `MessagePart::CommandExecution`, or any mutation to `coding_snapshots`, `episodic_memories`, or `semantic_memories`. (Review must be read-only.) | T2 |

### E2E (browser-only, recorded as GIF)

1. **Recall scenario.** Open same workspace twice (separate sessions). First session: ask agent to fix a bug; agent edits 2 files. Second session: ask "what did we change last time?" → assistant message includes a recall card citing the prior edit.
2. **Review scenario.** Open workspace, make 3 file edits, run `/review`. ReviewResultPart appears with at least one structured issue.
3. **AGENTS.md transparency scenario.** Open workspace with a nested AGENTS.md; AgentsMdPanel shows 2 sources (root + nested). Modify the nested AGENTS.md externally, click Refresh; panel updates.
4. **Subagent visibility scenario.** Ask a complex multi-file task. Watch SubagentTray populate as the agent spawns helpers. Cancel one mid-execution; verify it disappears from tray and the parent agent continues.

### Frontend (Vitest)

- `AgentsMdPanel.test.tsx` — 4 cases per §5
- `SubagentTray.test.tsx` — empty, populated, mid-cancel
- `useSubagents.test.ts` — fixture event sequences against reducer
- `ReviewResultPart.test.tsx` — issue grouping, severity colors, click-to-open behavior

### Build-time enforcement (existing)

- `bindings_are_current` continues
- `no_raw_tauri_command_outside_macros` continues
- `no_raw_invoke_in_endpoints` continues
- Zero clippy warnings

---

## 13. Component diagram (Sprint A additions only)

```
┌──────────────────────────────────────────────────────────────────────┐
│                  Coding thread view (existing + Sprint A)             │
│                                                                       │
│  ┌─────────────────┐  ┌─────────────────┐  ┌───────────────────────┐ │
│  │ AgentsMdPanel   │  │ RecallTrayCard  │  │ SubagentTray (T4)     │ │
│  │ (T3 — NEW)      │  │ (existing,      │  │ — NEW                  │ │
│  │                 │  │  populates      │  │                        │ │
│  │ Refresh ──┐     │  │  via T1)        │  │ row × N (live)        │ │
│  └───────────┼─────┘  └─────────────────┘  └───────────┬───────────┘ │
│              │                                          │             │
│              │ invoke()           listen()              │ listen()    │
│              ▼                  ◀─────                  ▼ ◀─────      │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │ Tauri command surface                                             │ │
│  │  coding_thread_refresh_agents_md (T3)                              │ │
│  │  subagent_list_active / subagent_cancel / subagent_inspect (T4)   │ │
│  │  coding_recall_stats (T1)                                          │ │
│  │  coding_review_start (T2 — re-implemented)                         │ │
│  └────────────────────────┬──────────────────────────────────────────┘ │
│                           │                                              │
│                           ▼                                              │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │ AppCore handlers (Rust)                                           │ │
│  │  coding/thread_handler.rs  + coding_thread_refresh_agents_md      │ │
│  │  coding/review_handler.rs  REWRITTEN body (T2)                    │ │
│  │  coding/subscription.rs    + fan_subagent_events_to_tauri (T4)    │ │
│  │  TypedBroker<SubagentEvent> (T4)                                   │ │
│  └────┬─────────────────────────────────────────────────────────────┘ │
│       │                                                                  │
│       ▼                                                                  │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │ Existing subsystems — wired LIVE                                  │ │
│  │  • coding_memory::CodingMemoryToolset    (T1: shadows recall_*)    │ │
│  │  • coding_memory::CodingRecallService    (Phase 4 wiring lit up)   │ │
│  │  • SubagentManager (events emitted to broker)            (T4)      │ │
│  │  • AgentRuntime (review pass via review-scoped params)   (T2)      │ │
│  │  • WorkspaceAgentsSource (re-walked on demand)           (T3)      │ │
│  └──────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 14. Open questions for the writing-plans phase

1. **`register_dyn` semantics** — confirm `tools_core::ToolRegistry::register_dyn` overwrites by name or whether we need a new `register_or_replace_dyn`. Code-read item, not a design decision.
2. **Review model default** — Haiku-tier is recommended; specify exact model ID (`claude-haiku-4-5-20251001`) once provider config schema is checked.
3. **Subagent lifecycle correlation** — should `agent_id` be a UUID (current) or a hash of `parent_session_id + label + spawned_at` (idempotent)? UUID is simpler; idempotent might help for retry semantics. Default UUID.
4. **AgentsMdPanel CSS placement** — verify position (right rail vs collapsed top bar). Frontend-design territory; `ui-ux-pro-max` skill consultation may help.
5. **`coding_recall_stats` vs adding to `coding_doctor`** — should recall stats be in Settings → Coding → Recall Stats, or in the existing doctor diagnostics? Default: separate component for visibility. Defer to user preference at plan time.
6. **`/review` slash command vs button** — composer-bar button or slash command? Default: both, slash command primary.
7. **Inline review delivery vs detached** — Phase 4 spec mentions `delivery: "inline" | "detached"`. Sprint A defaults to inline (rendered as `ReviewResultPart`). Detached UI shape is open.

These are plan-level concrete details, not spec-level architectural decisions.

---

## 15. Out-of-scope (consolidated)

### Deferred to Sprint B (UX completeness)
- File explorer / file tree component
- Live PTY panel for streaming bash stdout
- Diff staging / apply / revert affordance per hunk
- DeadEnd warning surfacing inline in thread

### Deferred to Sprint C (cognitive completion)
- Coding-memory reforge phases 2.5 + 3.5 bodies
- Coding-ingest Unix-socket transport (cross-CLI hot path)
- Causal-edge auto-detection refinements

### Deferred to Phase 5+ (platform / ecosystem)
- Windows sandbox
- Per-thread git worktrees
- Snapshot content-addressed dedup beyond ghost-commits
- IDE bridge via MCP (separate spec)
- Computer Use × coding integration (separate spec)
- Skills.sh marketplace
- MCP-contributed skills

### Permanently dropped
- ChatGPT OAuth, codex account API, OpenAI rate-limit dashboards
- WebSocket transport for the local desktop app (per §7 — current architecture is correct)

---

## 16. Effort estimate (rough)

| Track | Backend (rust-eng-days) | Frontend (ts-eng-days) | Test (eng-days) | Total |
|---|---|---|---|---|
| T1 — Live recall | 0.5 | 0.5 | 0.5 | 1.5 |
| T2 — LLM review | 1.5 | 1.0 | 1.0 | 3.5 |
| T3 — AgentsMdPanel | 0.5 | 1.0 | 0.5 | 2.0 |
| T4 — Subagent tray | 1.5 | 1.5 | 1.5 | 4.5 |
| T5 — Transport audit | 0.5 (bench) | 0 | 0.5 (bench harness) | 1.0 |
| **Total** | **4.5** | **4.0** | **4.0** | **12.5 eng-days** |

≈ 2.5 calendar weeks for one engineer working serial; ≈ 1 calendar week with subagent-driven parallel execution (T1/T2/T3/T4 are largely independent).

---

## 17. References

- Phase 4 spec — Tauri command surface, ThreadEvent/ApprovalRequest patterns
- Phase 1–3+ canonical spec — original recall_* tool design, Mirror signal sources
- Coding-memory design (`2026-04-22`) — `CodingRecallService`, `MemorySink`, fact taxonomy
- CLAUDE.md — pre-release migration policy, Tauri command macros, KCA validation gates
- `crates/agent/src/subagent.rs` — `SubagentManager` API surface
- `crates/coding-memory/src/mcp.rs` — `CodingMemoryToolset` API
- `crates/coding-agents-md/src/lib.rs` — `WorkspaceAgentsSource`, `walk_agents_md`
- `crates/desktop/src/dev_server/streaming.rs` — SSE handler reference
