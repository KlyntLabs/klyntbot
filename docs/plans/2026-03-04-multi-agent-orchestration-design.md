# Multi-Agent Orchestration Design

**Date**: 2026-03-04
**Status**: Approved
**Approach**: Wire Existing Delegation + General Agent as Orchestrator

## Problem Statement

The current agent system locks into a single agent per message. When a user sends a compound request like "check my transactions, then create a task for missing ones", the finance agent handles the financial queries but cannot create tasks — it lacks the `todo_add` tool and falls back to `ask_user`. The system cannot switch agents mid-conversation or decompose multi-intent messages.

### Root Causes

1. **DelegationTool is not wired**: `DelegationTool`, `DelegationHandler` trait, and `can_delegate_to` in all AGENT.md files exist but `AgentLoopBuilder` never registers the delegation tool and `DelegationHandler` has no implementation.
2. **No multi-intent decomposition**: Sequential language is detected but only increases iteration budget — the message is never split into sub-intents routed to different agents.
3. **Tool boundaries are hard**: Per-agent tool filtering means the finance agent literally cannot see task tools. This is correct by design but requires delegation to cross boundaries.

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Multi-intent handling | Intent decomposition via orchestrator | Structured, handles arbitrary multi-step requests |
| Orchestrator identity | General agent | Already exists as fallback, no new agent needed |
| Context sharing | Shared session history | Delegated agents see full conversation for coherent responses |
| Delegation types | Both orchestrator-driven and agent-initiated | Maximum flexibility — orchestrator decomposes upfront, agents can also delegate ad-hoc |

## Component 1: DelegationHandler Implementation

`AgentRuntime` implements the existing `DelegationHandler` trait to run mini agent executions for delegated agents.

### Flow

```
DelegationHandler::delegate(agent_name, query, ctx, depth)
  → AgentManager::get_agent(agent_name) → AgentProfile
  → Build delegated context:
     - Delegated agent's instructions + always_skills
     - Shared session history (full conversation so far)
     - Caller's tool results (delegated agent has context)
  → Filter tools to delegated agent's allowed_tool_names()
     + DelegationTool (if can_delegate_to non-empty AND depth < max)
  → Run ReactiveEngine with reduced budget:
     - max_iterations = min(profile.max_iterations, 8)
     - Same ExecutionCore, same providers
  → Return agent's response text as tool result to caller
  → Record delegation in InteractionLog
```

### Key Details

- Delegated execution shares the same `session_history` — delegated agent sees full conversation
- `DelegationTool` is dynamically constructed per-agent since `allowed_agents` varies per profile
- Depth incremented and passed through — at `depth >= max_depth`, `DelegationTool` excluded from delegated agent's tools
- `delegation_depth` field on `RoutingContext` tracks depth through call chain

## Component 2: Multi-Intent Detection & Orchestration

### Detection (IntentAnalyzer changes)

`IntentAnalysis` gains two new fields:

```rust
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub confidence: f64,
    pub needs_orchestration: bool,     // NEW
    pub sub_intents: Vec<String>,      // NEW (from LLM classifier)
    // ... existing fields ...
}
```

**Heuristic stage**: If sequential language detected (`first...then`, `if...then`, `check...and create`) AND message triggers match 2+ different agents → `needs_orchestration = true`.

**LLM classifier stage**: Classifier prompt updated to also output `needs_orchestration` and optionally `sub_intents` (decomposed queries). Additive change to existing JSON schema.

### Routing Override

In `AgentRuntime::process_message()`, after agent matching and intent analysis:

```rust
if analysis.needs_orchestration {
    profile = agent_manager.get_agent("general");
    // General agent's instructions include orchestration guidance
    // Increase max_iterations to accommodate multiple delegations
}
```

### General Agent Orchestration Instructions

The general agent's AGENT.md gets an orchestration section:

```markdown
## Orchestration

When handling multi-part requests:
1. Break down the request into discrete tasks
2. Use `delegate(agent, query)` for each part, in logical order
3. Pass relevant context from earlier steps to later delegations
4. Synthesize a unified response from all delegation results
```

The LLM handles decomposition naturally through its ReAct loop — no rigid planner. It calls `delegate()`, gets results, reasons about next steps, delegates again.

## Component 3: DelegationTool Registration & Wiring

### Dynamic Per-Message Construction

Instead of static global registration, construct `DelegationTool` dynamically in `AgentRuntime::process_message()` step 7 (tool filtering):

```rust
if !profile.can_delegate_to.is_empty() && delegation_depth < max_depth {
    let delegation_tool = DelegationTool::with_handler(self_ref.clone())
        .with_allowed_agents(profile.can_delegate_to.clone())
        .with_depth(delegation_depth, max_depth);
    filtered_tools.push(delegation_tool.to_definition());
}
```

### Self-Reference Pattern

`AgentRuntime` implements `DelegationHandler` and holds an `Arc<Self>` for the circular reference:

```rust
pub struct AgentRuntime {
    // ... existing fields ...
    self_ref: Option<Arc<dyn DelegationHandler>>,
}
```

Set after construction via a `set_self_ref()` method or two-phase init.

### Transparency Events

Each delegation emits:
- `DelegationStarted { from_agent, to_agent, query }`
- `DelegationCompleted { from_agent, to_agent, result_summary }`

These display in the desktop UI "Agents" panel so users see agent switching in real time.

## Component 4: Execution Tool Registry

The current `ExecutionCore` receives tools as a `&[Value]` slice of JSON definitions. To support dynamically-added `DelegationTool` execution (not just schema), we need the tool to be callable at runtime.

### Approach

Add `DelegationTool` to the per-execution `ToolRegistry` (or a side-channel):

```rust
// In AgentRuntime, before calling ExecutionRouter
let mut exec_tools = self.tool_registry.clone();
if let Some(delegation_tool) = delegation_tool {
    exec_tools.register(Arc::new(delegation_tool));
}
```

The `ToolRegistry` already supports dynamic registration via `register()`. The delegation tool is registered per-execution alongside the filtered tool definitions.

## Files to Modify

| File | Change |
|------|--------|
| `crates/agent/src/agent_runtime/runtime.rs` | Implement `DelegationHandler`, orchestration routing override, dynamic `DelegationTool` construction in step 7, per-execution tool registry |
| `crates/agent/src/intent_pipeline/analysis.rs` | Add `needs_orchestration` detection in heuristic + LLM classifier |
| `crates/agent/src/intent_pipeline/types.rs` | Add `needs_orchestration: bool` and `sub_intents: Vec<String>` to `IntentAnalysis` |
| `crates/agent/src/agent_loop/builder.rs` | Pass `Arc<AgentRuntime>` self-ref for delegation handler |
| `agents/general/AGENT.md` | Add orchestration instructions |
| `crates/tools/src/delegation.rs` | Ensure `DelegationTool` implements tool definition export, minor compatibility |

## Files Unchanged

- All existing tools, storage, providers, channels
- `ExecutionCore`, `ReactiveEngine` — reused as-is
- All other agent profiles — already have `can_delegate_to` configured
- `SubagentManager` — unchanged, remains for fire-and-forget background tasks

## Configuration

```json
{
  "orchestrator": {
    "maxDelegationDepth": 2,
    "delegatedMaxIterations": 8,
    "orchestratorMaxIterations": 15
  }
}
```

## Example Flow: "Check transactions, then create a task"

```
1. User: "Check my transactions, if none exist create a task to add them by tomorrow"
2. AgentManager matches: finance (triggers: "transaction")
3. IntentAnalyzer detects: sequential language + multi-agent triggers → needs_orchestration = true
4. Override: route to general agent as orchestrator
5. General agent (ReAct loop, max 15 iterations):
   a. Calls delegate("finance", "check what transactions the user has")
      → Finance agent runs (max 8 iterations)
      → Uses finance:tx_list, finance:account_list tools
      → Returns: "No transactions found. User has a Main Bank account."
   b. Receives delegation result
   c. Calls delegate("task", "Create a task: Add details for all transactions in account balance. Due: tomorrow")
      → Task agent runs (max 8 iterations)
      → Uses todo:add tool with enrichment
      → Returns: "Created task 'Add transaction details' due 2026-03-05, priority medium"
   d. Synthesizes: "I checked your transactions — none found yet. I've created a task 'Add transaction details' due tomorrow to help you get started."
6. User sees unified response with transparency showing: finance → task delegation chain
```
