# Action Space Redesign — Closing the Gap with Claude Code Patterns

**Date**: 2026-02-28
**Status**: Approved
**Scope**: 6 architectural improvements across 7 crates

## Background

A systematic comparison of Klyntbot's tool architecture against the lessons shared in Anthropic's blog post on Claude Code's action space design revealed 6 gaps. Since Klyntbot has not yet shipped to production, breaking changes are acceptable — the goal is long-term architectural quality.

## Summary of Changes

| # | Feature | Priority | Crates Affected |
|---|---------|----------|-----------------|
| 1 | Grep + Glob search tools | P0 | tools, agent |
| 2 | Subagent coordination (AgentTaskBoard) | P0 | storage, tools, agent |
| 3 | Platform-native elicitation | P1 | tools-core, channels, tools |
| 4 | Intent-based tool filtering | P1 | agent (intent_pipeline) |
| 5 | Separate agent-internal task system | P2 | storage, tools, agent |
| 6 | Mode-gated RAG + progressive disclosure | P2 | context_engine, agent |

---

## 1. Grep + Glob Search Tools

### Problem

The agent can read files and list directories, but cannot search file contents or find files by pattern. It must `list_dir` → `read_file` sequentially to find anything — extremely expensive in tokens and iterations. The Claude Code blog specifically highlights the shift from passive RAG to giving agents search tools.

### Design

Two new tools in `crates/tools/src/`:

#### GrepTool (`grep.rs`)

```json
{
  "name": "grep",
  "description": "Search file contents using regex patterns within a directory scope",
  "parameters": {
    "pattern": { "type": "string", "description": "Regex pattern to search for" },
    "path": { "type": "string", "description": "Directory to search in (default: workspace root)" },
    "glob": { "type": "string", "description": "File filter pattern, e.g. '*.rs'" },
    "max_results": { "type": "integer", "default": 20, "description": "Max matches to return" },
    "context_lines": { "type": "integer", "default": 0, "description": "Lines of context around each match" }
  },
  "required": ["pattern"]
}
```

- Returns: `filepath:line_number: matched_line` format
- Permission: `ReadOnly`
- Workspace restriction: same `allowed_dir` logic as `read_file`
- Implementation: Use the `grep` crate for pure-Rust regex search, with `walkdir` for directory traversal and `globset` for file filtering

#### GlobTool (`glob.rs`)

```json
{
  "name": "glob",
  "description": "Find files by pattern matching",
  "parameters": {
    "pattern": { "type": "string", "description": "Glob pattern like '**/*.rs' or 'src/**/*.ts'" },
    "path": { "type": "string", "description": "Root directory (default: workspace root)" }
  },
  "required": ["pattern"]
}
```

- Returns: list of matching file paths, sorted by modification time
- Permission: `ReadOnly`
- Implementation: Use the `glob` crate

#### Registration

Both tools register in `AgentLoopBuilder::build()` alongside filesystem tools. Available to all subagent profiles (read-only).

---

## 2. Subagent Coordination — AgentTaskBoard

### Problem

Subagents are fire-and-forget isolated workers with no shared state, no coordination, no cancellation, no progress reporting. The Claude Code blog's key evolution was from isolated workers to coordinating team members via the Task Tool.

### Design

Three components: SQLite table, subagent-facing tool, and SubagentManager overhaul.

#### 2a. `agent_tasks` Table (storage crate)

New migration:

```sql
CREATE TABLE agent_tasks (
    id              TEXT PRIMARY KEY,
    session_key     TEXT NOT NULL,
    description     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending','claimed','running','completed','failed')),
    owner_agent_id  TEXT,
    parent_task_id  TEXT REFERENCES agent_tasks(id),
    result          TEXT,
    error           TEXT,
    blocked_by      TEXT DEFAULT '[]',  -- JSON array of task IDs
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_agent_tasks_session ON agent_tasks(session_key);
CREATE INDEX idx_agent_tasks_status ON agent_tasks(status);
```

`AgentTaskRepo` in `storage` crate with methods:
- `create(session_key, description, blocked_by) -> AgentTask`
- `claim(task_id, agent_id) -> Result<AgentTask>` (fails if already claimed)
- `update_status(task_id, status, result/error)`
- `list_by_session(session_key) -> Vec<AgentTask>`
- `list_available(session_key) -> Vec<AgentTask>` (unclaimed + unblocked)
- `delete_by_session(session_key)` (cleanup)

#### 2b. `AgentTaskTool` (tools crate)

New tool available to subagents only (not in parent's registry):

| Action | Parameters | Description |
|--------|-----------|-------------|
| `list` | — | Show all tasks for this session with status |
| `claim` | `task_id` | Claim an unclaimed task |
| `update` | `task_id`, `status`, `result` | Update progress |
| `complete` | `task_id`, `result` | Mark done with final output |
| `fail` | `task_id`, `error` | Mark failed with error message |

Handler trait: `AgentTaskHandler` defined in tools crate, implemented in agent crate (same dependency inversion pattern as SpawnHandler).

#### 2c. SubagentManager Overhaul (agent crate)

**SpawnHandler trait extension:**

```rust
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(&self, task: String, label: Option<String>, profile: String,
                   origin_channel: String, origin_chat_id: String) -> String;
    async fn cancel(&self, agent_id: &str) -> Result<String>;
    async fn status(&self, session_key: &str) -> Result<String>;
}
```

**SpawnTool gets new actions**: `spawn` (existing behavior), `cancel` (new), `status` (new).

**Lifecycle changes:**
1. `spawn` action: creates `agent_tasks` rows → stores `CancellationToken` + `JoinHandle` in `HashMap<String, SubagentHandle>` → spawns tokio task
2. Subagent receives task IDs in its system prompt → calls `agent_task list` → claims tasks → works
3. `cancel` action: triggers `CancellationToken` → subagent's ReactiveEngine checks token between cycles
4. `status` action: reads `agent_tasks` table for the session → formats task board state

**SubagentHandle:**

```rust
struct SubagentHandle {
    cancel_token: CancellationToken,
    join_handle: JoinHandle<()>,
    label: String,
    profile: SubagentProfile,
    spawned_at: Instant,
}
```

---

## 3. Platform-Native Elicitation

### Problem

The `ask_user` tool works beautifully in CLI and Dashboard but degrades to plain text on all chat channels (Telegram, Discord, Slack). These platforms natively support buttons, select menus, and interactive components.

### Design

#### 3a. Channel Trait Extension

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    // ... existing methods ...

    /// Whether this channel supports structured interactions
    fn supports_interaction(&self) -> bool { false }

    /// Send a structured interaction and await response.
    /// Blocks until user responds or timeout (5 min).
    async fn send_interaction(
        &self,
        chat_id: &str,
        request: &InteractionRequest,
    ) -> Result<FormResponse> {
        Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            "Channel does not support interactions".into()
        )))
    }
}
```

#### 3b. Per-Channel Implementations

**Telegram** (`supports_interaction() → true`):
- `single_select`: `InlineKeyboardMarkup` with one button per option (rows of 2)
- `yes_no`: two buttons `[Yes]` `[No]`
- `multi_select`: toggle buttons with checkmark prefix + `[Submit]` button
- `free_text`: sends question as text, awaits next message via per-chat `PendingInteraction` state
- Response: captured via `callback_query` in existing long-poll loop
- Timeout: 5 minutes → `FormResponse::Cancelled`
- Cleanup: edit original message to show selected answer after response

**Discord** (`supports_interaction() → true`):
- Uses `ActionRow` + `Button` for yes/no and single_select
- Uses `StringSelectMenu` for multi_select with max 25 options
- Responds via interaction callback URL
- 5-minute timeout via component expiry

**Slack** (`supports_interaction() → true`):
- Block Kit: `section` for question text, `actions` with `static_select` / `button`
- Responds via Slack interaction payload URL
- 5-minute timeout

**WhatsApp, Email, QQ**: No change — `supports_interaction()` returns `false`, text fallback unchanged.

#### 3c. ask_user Tool Integration

Add `channel_ref: Option<Arc<dyn Channel>>` to `RoutingContext`. The `ask_user` execute flow becomes:

```
1. If interaction_tx is Some → CLI/Dashboard path (unchanged)
2. If interaction_tx is None AND channel_ref.supports_interaction():
   → call channel_ref.send_interaction(chat_id, &request)
   → format and return response
3. Else → text fallback (unchanged)
```

The `RoutingContext` is constructed in `process_message()` where the channel is already known. For bus-driven messages, look up the channel from `ChannelManager` by name and inject the `Arc<dyn Channel>`.

---

## 4. Intent-Based Tool Filtering

### Problem

Every Reactive/Planned mode message sees all 18+ tools regardless of relevance. A greeting gets finance tools. A task query gets browser tools. This wastes tokens and can confuse smaller models.

### Design

#### 4a. Tool Groups

Static mapping in `crates/agent/src/intent_pipeline/heuristics.rs`:

```rust
pub enum ToolGroup {
    None,              // Direct mode: greetings, acknowledgments
    TaskManagement,    // todo, goal, plan
    Search,            // grep, glob, read_file, list_dir, web_search, web_fetch, memory
    Calendar,          // calendar, todo
    Finance,           // finance
    Communication,     // message, ask_user
    Automation,        // cron, spawn
    Full,              // all tools
}

impl ToolGroup {
    pub fn tool_names(&self) -> &[&str] { /* ... */ }
}
```

#### 4b. IntentAnalysis Extension

```rust
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub confidence: f64,
    pub tool_groups: Vec<ToolGroup>,  // NEW
}
```

Heuristics populate `tool_groups` based on the same keyword patterns already used for mode classification. The LLM classifier adds `relevant_tools: Vec<String>` to its JSON output.

#### 4c. Pipeline Integration

In `IntentPipeline::process_message()`, after classification:

```rust
let filtered_tools = if tool_groups.contains(&ToolGroup::Full) {
    all_tool_definitions.clone()
} else {
    let allowed_names: HashSet<&str> = tool_groups.iter()
        .flat_map(|g| g.tool_names())
        .chain(["ask_user"].iter())  // always available
        .collect();
    all_tool_definitions.iter()
        .filter(|t| allowed_names.contains(t.name()))
        .cloned()
        .collect()
};
```

Safety: if an engine escalates, the escalated engine receives `Full` tools.

#### 4d. Tool Usage Analytics Table (P3)

```sql
CREATE TABLE tool_usage (
    id          TEXT PRIMARY KEY,
    tool_name   TEXT NOT NULL,
    action      TEXT,
    session_key TEXT,
    channel     TEXT,
    intent_category TEXT,
    success     BOOLEAN NOT NULL,
    duration_ms INTEGER,
    error_message TEXT,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

Populated in `ExecutionCore::run_cycle()` after each tool call. Used for future data-driven optimization.

---

## 5. Mode-Gated RAG

### Problem

`ConversationMemoryRetriever` runs on every message, injecting up to 5 retrieved conversation turns into context. For Direct-mode messages (greetings, simple questions), this wastes ~500-1000 tokens and may inject irrelevant context.

### Design

In `context_engine/src/assembler.rs`, gate memory retrieval on execution strategy:

```rust
let memory_entries = match strategy {
    ExecutionStrategy::DirectResponse | ExecutionStrategy::Clarification => {
        vec![] // Skip RAG for simple queries
    }
    _ => {
        self.memory_retriever.retrieve(message, budget.memory_budget).await
    }
};
```

The embedding write path (fire-and-forget) remains unchanged — every message is still embedded for future retrieval.

This is a ~5 line change with immediate token savings.

---

## 6. Progressive Disclosure — Skill Conventions

### Problem

Skills are either fully injected (`always: true`) or listed as a summary catalog. There's no mechanism for deeper exploration when the agent needs more detail.

### Design

Convention-based, no code changes:

1. Skills include a "Deep Dive" section with markdown links to detailed docs
2. The agent follows links with `read_file` when it needs more detail
3. Add a brief instruction to `IdentitySource` system prompt:

```
When skills reference additional documentation via markdown links,
use read_file to load that documentation if the current task requires
deeper knowledge.
```

Example skill structure:
```markdown
## Quick Reference
Basic calendar operations...

## Deep Dive
For advanced scheduling: [scheduling guide](docs/scheduling.md)
For CalDAV internals: [caldav reference](docs/caldav.md)
```

---

## Crate Impact Summary

| Crate | Changes |
|-------|---------|
| `storage` | New migration: `agent_tasks` + `tool_usage` tables. New `AgentTaskRepo`. |
| `tools-core` | Add `channel_ref` to `RoutingContext`. |
| `tools` | New: `GrepTool`, `GlobTool`, `AgentTaskTool`. Extend `SpawnTool` with cancel/status actions. Extend `SpawnHandler` trait. Update `ask_user` execute flow. |
| `channels` | Extend `Channel` trait with `supports_interaction()` + `send_interaction()`. Implement for Telegram, Discord, Slack. |
| `agent` | Overhaul `SubagentManager` (lifecycle, cancellation, task board). Tool filtering in pipeline. Wire `AgentTaskHandler`. Update `IdentitySource` prompt. |
| `context_engine` | Mode-gate memory retrieval (~5 lines). |
| `common` | No changes (InteractionRequest/FormResponse types already sufficient). |

## Dependencies (New Crate Dependencies)

- `tools`: add `glob`, `globset`, `walkdir`, `grep` crates
- No new crate creation — all changes fit existing crate boundaries

## Implementation Order

Since breaking changes are acceptable and this is pre-production:

1. **Grep + Glob** (P0, independent, small) — unblocks everything else
2. **AgentTaskBoard + SubagentManager** (P0, largest piece) — depends on storage migration
3. **Tool filtering** (P1, independent) — can parallelize with #2
4. **Platform-native elicitation** (P1, independent) — can parallelize with #2
5. **Mode-gated RAG** (P2, trivial) — can be done anytime
6. **Progressive disclosure conventions** (P2, no code) — documentation only
