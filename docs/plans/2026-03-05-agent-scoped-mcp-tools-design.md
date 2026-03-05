# Agent-Scoped MCP Tool Filtering

**Date:** 2026-03-05
**Status:** Approved

## Problem

All MCP tools bypass agent profile filtering unconditionally in `filter_tools_for_profile()` (`runtime.rs:L711`). Any tool with `mcp_` prefix passes through regardless of the agent profile's allowlist. This causes LLMs to confuse native tools with semantically similar MCP tools (e.g., native `task` vs `mcp_linear_create_issue`).

The root cause: the LLM sees both native and MCP tools side-by-side with no disambiguation, pattern-matches on semantics ("create a task" ≈ "create an issue"), and picks whichever seems most relevant from training data.

## Solution

Add `mcp_tools` field to agent profiles that controls which MCP servers each agent can access. Remove the unconditional `mcp_*` bypass.

## Design

### Agent Profile Schema Change

New `mcp_tools` field in `AGENT.md` YAML frontmatter:

```yaml
# agents/task/AGENT.md
name: task
tools: [task, area, project, okr, memory, grep, glob, read_file, list_dir]
mcp_tools: []  # No MCP tools — pure native task management

# agents/general/AGENT.md
name: general
tools: []  # All native tools
mcp_tools: ["*"]  # All MCP servers (orchestrator sees everything)

# agents/finance/AGENT.md
name: finance
tools: [finance, ask_user, memory]
mcp_tools: []

# agents/calendar/AGENT.md
name: calendar
tools: [calendar, ask_user, memory]
mcp_tools: []

# agents/automation/AGENT.md
name: automation
tools: [cron, ask_user, memory]
mcp_tools: []

# agents/communication/AGENT.md
name: communication
tools: [message, ask_user, memory]
mcp_tools: []  # Could add [slack, linear] later
```

### Semantics

| `mcp_tools` value | Behavior |
|---|---|
| not specified / absent | `[]` — no MCP tools (safe default, opt-in) |
| `[]` | No MCP tools |
| `["*"]` | All MCP servers' tools pass through |
| `["linear", "github"]` | Only tools from named MCP servers |

- **Default is opt-in** (empty = no MCP tools) for specialist agents
- **General agent uses `["*"]`** (opt-out) — sees all MCP servers for direct execution or delegation
- New MCP servers are automatically available to the general agent without config changes

### Code Changes

#### 1. `AgentProfile` parsing (`crates/agent/src/agent_profile/types.rs`)

- Add `mcp_tools: Vec<String>` field to `AgentProfile` struct
- Parse from YAML frontmatter, default to empty vec when absent
- Add method:
  ```rust
  pub fn allows_mcp_server(&self, server_name: &str) -> bool {
      self.mcp_tools.iter().any(|s| s == "*" || s == server_name)
  }
  ```

#### 2. `filter_tools_for_profile` (`crates/agent/src/agent_runtime/runtime.rs:~L700`)

Remove the unconditional `mcp_*` bypass. Replace with:

```rust
// For MCP tools, check against profile's mcp_tools allowlist
if tool_name.starts_with("mcp_") {
    // Extract server name: "mcp_{server}_{tool}" → "{server}"
    let server_name = tool_name
        .strip_prefix("mcp_")
        .and_then(|rest| rest.split('_').next())
        .unwrap_or("");
    return profile.allows_mcp_server(server_name);
}
```

#### 3. Update agent AGENT.md files (`agents/*/AGENT.md`)

- `general/AGENT.md`: add `mcp_tools: ["*"]`
- All other agents: either add `mcp_tools: []` explicitly or omit (default is empty)

#### 4. Intent classifier tool name filtering (`crates/agent/src/intent_pipeline/analysis.rs`)

Filter MCP tool names passed to the `CLASSIFICATION_PROMPT` based on the matched agent's `mcp_tools` scope. Currently all tool names including `mcp_linear_*` are passed, biasing the classifier.

### Data Flow

```
Message arrives
  → AgentManager::match_agent() → profile (e.g., "task")
  → profile.mcp_tools = []
  → filter_tools_for_profile():
      native tools: filtered by profile.tools (existing behavior, unchanged)
      mcp_* tools:  filtered by profile.mcp_tools (NEW)
        → extract server name from "mcp_{server}_{tool}"
        → profile.allows_mcp_server(server) → true/false
  → LLM sees only: [task, area, project, okr, ...] — no Linear tools
  → No confusion possible
```

### Edge Cases

- **User says "create a Linear issue" but matched to task agent**: The task agent won't have Linear tools. The existing MCP override at `analysis.rs:L651` already forces orchestration when MCP server names are mentioned — this routes to the general agent which has `mcp_tools: ["*"]`.
- **New MCP server added**: Automatically available to general agent. Specialist agents require explicit `mcp_tools` update.
- **MCP server name extraction**: Uses first segment after `mcp_` prefix. E.g., `mcp_linear_create_issue` → server = `linear`. This matches the naming convention in `sanitize::build_tool_name()`.

### What Does NOT Change

- `ToolRegistry` stays flat — no structural changes
- MCP tool naming convention (`mcp_{server}_{tool}`) unchanged
- MCP connection/discovery in `McpManager` unchanged
- General agent behavior unchanged — still sees everything
- Delegation system unchanged
- `ask_user` always-add behavior unchanged
