# Frontend Agent + Learning Integration Design

**Goal:** Make the Transparency panel's Agents and Learning sections display real data from the agent-driven architecture, showing which agent handled each message and what learning context was applied.

**Current state:** The frontend UI (TransparencyPanel, useAgentStream, desktop event handlers) is fully wired for `LearningEvent` and `SubagentSpawned` events, but the backend never emits them during normal message processing.

## Architecture

### Data flow

```
AgentRuntime.process_message()
  ├─ match_agent() → emit AgentSelected { name, description }
  ├─ context_engine.assemble() → query learning repos → emit LearningEvent summaries
  └─ (existing: ClassificationComplete, ExecutionStarted, ToolStart/End, etc.)
       ↓
desktop/commands/chat.rs  (event loop)
  ├─ AgentSelected → push to transparency.agent_selected + Tauri emit
  └─ LearningEvent → push to transparency.learning + Tauri emit
       ↓
useAgentStream.ts  (event listeners)
  ├─ agent:agent_selected → setTransparency({ agentSelected })
  └─ agent:learning_event → setTransparency({ learning: [...] })
       ↓
TransparencyPanel.tsx  (render)
  ├─ Agents: show selected agent + any delegated sub-agents
  └─ Learning: show profile facts, patterns, adaptations, confidence
```

### Changes by layer

**Backend (Rust):**
1. `crates/agent/src/events.rs` — Add `AgentSelected { name, description }` variant
2. `crates/agent/src/agent_runtime/runtime.rs` — Emit `AgentSelected` after agent matching; inject learning repos and emit `LearningEvent` summaries after context assembly
3. `crates/desktop-shared/src/events.rs` — Add `AGENT_AGENT_SELECTED` constant, `AgentSelectedPayload`, `TransparencyAgentSelected`, and `agent_selected` field on `TransparencyData`
4. `crates/desktop/src/commands/chat.rs` — Handle `AgentEvent::AgentSelected` in the event loop match block

**Frontend (TypeScript):**
5. `desktop-ui/src/lib/types.ts` — Add `agentSelected?: { name: string; description: string }` to `TransparencyData`
6. `desktop-ui/src/hooks/useAgentStream.ts` — Add `useEvent` listener for `agent:agent_selected`
7. `desktop-ui/src/components/chat/TransparencyPanel.tsx` — Redesign Agents section to show selected agent prominently + sub-agents; Learning section already renders correctly

### Learning event summaries

The `AgentRuntime` will query learning repos after context assembly and emit these events:

| event_type | detail example | Source |
|---|---|---|
| `user_profile` | `3 facts (projects, preferences, habits)` | `UserProfileRepo.list_above_confidence(0.5)` |
| `patterns` | `2 patterns (monday_tasks, morning_routine)` | `BehavioralPatternRepo.list_reliable(5)` |
| `adaptations` | `1 preference for task agent` | `AgentAdaptationRepo.list_by_agent(name)` |
| `confidence` | `threshold: 70%` | `ConfidenceEvaluator.threshold()` |

### Agents section UI

```
┌─ Agents ──────────────────┐
│ 🤖 task                   │  ← primary (AgentSelected)
│    Task management spec.   │
│ ─────────────────────────  │
│ (sub-agents if delegated)  │  ← existing SubagentSpawned
└───────────────────────────┘
```

When no agent is selected (should never happen), falls back to "none".
