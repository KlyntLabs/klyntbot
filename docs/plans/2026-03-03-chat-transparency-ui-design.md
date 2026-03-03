# Chat Transparency UI Design

**Date:** 2026-03-03
**Status:** Approved
**Approach:** Enrich existing AgentEvent stream (Approach A)

## Summary

Add a per-message transparency display to the `/chat` route that shows detailed token usage, tool calls, memory accesses, skills invoked, execution steps, cache usage, agents, memory writes, and learning events. Toggled via a chat header button. Data flows through the existing `AgentEvent` streaming relay and persists in `SessionMessage.metadata` for history replay.

## Design Decisions

- **Inline badge** with click-to-expand detail per assistant message (not sidebar panel)
- **Per-message** Klyntbot/Context boxes (not per-session)
- **Chat header toggle** persisted to localStorage
- **Approach A**: Enrich existing `AgentEvent` stream — minimal new infrastructure, real-time streaming, leverages existing relay and metadata persistence

## 1. New AgentEvent Variants

Add to `crates/agent/src/events.rs`:

| Event | Emitted From | Data |
|-------|-------------|------|
| `UsageReport` | `pipeline.rs` after `CostTracker::record()` | `Usage` + `estimated_cost_usd` + `model` + `response_time_ms` |
| `MemoryAccess` | `memory_tool.rs` on search/status | `action`, `query`, `results_count` |
| `SkillLoaded` | skill manager when skill content injected | `skill_name`, `trigger_reason` |
| `LearningEvent` | `LearningEventBus` subscriber | `event_type`, `detail` |
| `SubagentSpawned` | `subagent.rs` on spawn | `label`, `profile` |

Events already emitted but currently dropped in relay `_ => {}`:

| Event | Change |
|-------|--------|
| `IterationStart` | Relay as `agent:iteration_start` |
| `ConfidenceAssessed` | Relay as `agent:confidence_assessed` |
| `ClassificationComplete` | Already relayed — frontend now consumes it |
| `ExecutionStarted` | Already relayed — frontend now consumes it |

## 2. Backend Relay Changes (`chat.rs`)

### New accumulator

```rust
struct TransparencyData {
    usage: Option<UsageReportPayload>,
    classification: Option<ClassificationInfo>,
    execution: Option<ExecutionInfo>,
    tools: Vec<ToolInfo>,
    memory_accesses: Vec<MemoryAccessInfo>,
    skills: Vec<SkillInfo>,
    subagents: Vec<SubagentInfo>,
    learning: Vec<LearningInfo>,
    iterations: u32,
    timing: TimingInfo,
}
```

Accumulated alongside existing `segments` vec in the relay loop. On `AgentEvent::Done`, serialized into `metadata.transparency` alongside existing `metadata.segments`.

### New relay handlers

Replace `_ => {}` catch-all with explicit handlers:

- `IterationStart` → emit `agent:iteration_start` + increment `transparency.iterations`
- `ConfidenceAssessed` → emit `agent:confidence_assessed`
- `ClassificationComplete` → emit (already) + store in `transparency.classification`
- `ExecutionStarted` → emit (already) + store in `transparency.execution`
- `UsageReport` → emit `agent:usage_report` + store in `transparency.usage`
- `MemoryAccess` → emit `agent:memory_access` + push to `transparency.memory_accesses`
- `SkillLoaded` → emit `agent:skill_loaded` + push to `transparency.skills`
- `LearningEvent` → emit `agent:learning_event` + push to `transparency.learning`
- `SubagentSpawned` → emit `agent:subagent_spawned` + push to `transparency.subagents`

### New Tauri event payloads (`desktop-shared/src/events.rs`)

| Event Name | Payload Struct |
|-----------|----------------|
| `agent:usage_report` | `UsageReportPayload { prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens, estimated_cost_usd, model, response_time_ms }` |
| `agent:memory_access` | `MemoryAccessPayload { action, query, results_count }` |
| `agent:skill_loaded` | `SkillLoadedPayload { name, trigger }` |
| `agent:learning_event` | `LearningEventPayload { event_type, detail }` |
| `agent:subagent_spawned` | `SubagentSpawnedPayload { label, profile }` |
| `agent:iteration_start` | `IterationStartPayload { iteration, max_iterations }` |

## 3. Metadata Persistence Schema

Persisted in `SessionMessage.metadata` JSON (existing freeform field):

```json
{
  "segments": [...],
  "transparency": {
    "usage": {
      "promptTokens": 1247,
      "completionTokens": 823,
      "cacheReadTokens": 412,
      "cacheWriteTokens": 0
    },
    "cost": {
      "estimatedUsd": 0.0031,
      "model": "claude-sonnet-4-6"
    },
    "timing": {
      "totalMs": 2340,
      "classificationMs": 120,
      "contextAssemblyMs": 45
    },
    "tools": [
      { "name": "todo_search", "success": true, "durationMs": 89 }
    ],
    "memoryAccesses": [
      { "action": "search", "query": "work preferences", "resultsCount": 3 }
    ],
    "skills": [
      { "name": "todo", "trigger": "keyword:task" }
    ],
    "execution": {
      "engine": "reactive",
      "iterations": 2,
      "maxIterations": 10,
      "escalations": 0
    },
    "classification": {
      "strategy": "task_management",
      "confidence": 0.95,
      "source": "heuristic"
    },
    "subagents": [],
    "learning": [],
    "cache": {
      "readTokens": 412,
      "writeTokens": 0,
      "hitRate": 0.33
    }
  }
}
```

## 4. Frontend Architecture

### State management (`useAgentStream.ts`)

New state: `transparency: TransparencyData | null`

New event listeners accumulate data incrementally:
- `agent:classification_complete` → `transparency.classification`
- `agent:execution_started` → `transparency.execution`
- `agent:tool_end` → push to `transparency.tools`
- `agent:iteration_start` → `transparency.currentIteration`
- `agent:memory_access` → push to `transparency.memoryAccesses`
- `agent:skill_loaded` → push to `transparency.skills`
- `agent:usage_report` → `transparency.usage`
- `agent:learning_event` → push to `transparency.learning`
- `agent:subagent_spawned` → push to `transparency.subagents`

### Persistence on reload

`ChatMessageResponse` already carries `metadata` from `SessionMessage.metadata`. On `chat_messages` fetch, deserialize `metadata.transparency` into the same `TransparencyData` TypeScript type. Historical messages display transparency data identically to live-streamed messages.

### Global toggle

```ts
const [showTransparency, setShowTransparency] = useLocalStorage('chat:transparency', false);
```

Persisted to localStorage. Survives page refreshes.

## 5. UI Components

| Component | Purpose |
|-----------|---------|
| `TransparencyToggle` | Header icon button. Toggles `showTransparency` |
| `TokenBadge` | Compact `↑1.2k ↓0.8k · $0.003` right-aligned per assistant message |
| `TokenDetail` | Expandable breakdown: input/output/cache/model/cost/latency |
| `KlyntbotBox` | Collapsible box: memory accesses, session history count |
| `ContextBox` | Collapsible box: skills loaded with trigger reasons |
| `ExecutionDetail` | Section: engine/iterations, strategy/confidence, agents, memory writes, learning |

### Layout (transparency ON)

```
┌──────────────────────────────────────────────┐
│ [Assistant message content / segments]        │
│                                               │
│                     ↑1.2k ↓0.8k · $0.003     │  ← TokenBadge
│                                               │
│  ┌─ TokenDetail ───────────────────────────┐  │
│  │ Input: 1,247  Output: 823  Cache: 412   │  │
│  │ Model: claude-sonnet-4-6  Latency: 2.3s │  │
│  └─────────────────────────────────────────┘  │
│                                               │
│  ┌─ klyntbot ──────────────────────── ▼ ──┐  │
│  │ 📄 memory: work-preferences (3 hits)   │  │
│  │ 📄 session history (12 messages)       │  │
│  └────────────────────────────────────────┘  │
│  ┌─ Context ───────────────────────── ▼ ──┐  │
│  │ 📦 skill: todo (keyword: "task")       │  │
│  │ 📦 skill: daily-planning (always)      │  │
│  └────────────────────────────────────────┘  │
│                                               │
│  ┌─ Execution ─────────────────────── ▼ ──┐  │
│  │ 🔄 Engine: reactive (2/10 iterations)  │  │
│  │ 🧠 Strategy: task_management (95%)     │  │
│  │ 🤖 Agents: none                        │  │
│  │ 💾 Memory writes: none                 │  │
│  │ 📊 Learning: none                      │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

### Layout (transparency OFF)

No badge, no boxes. Identical to current UI.

### Streaming behavior

- TokenBadge: shows spinner until `agent:usage_report` arrives
- Klyntbot/Context boxes: populate incrementally as events arrive
- Execution section: shows live iteration count during reactive execution

### Styling

Existing design tokens only:
- `bg-surface-raised` for box backgrounds
- `text-muted` for labels, `text-secondary` for values
- `border-border` for separators
- `lucide-react` icons (Eye, ChevronDown, FileText, Package, Cpu, Brain, Database)
- Collapsible sections follow existing `SegmentedMessage` tool expand pattern

## 6. Files to Modify

### Backend (Rust)

| File | Change |
|------|--------|
| `crates/agent/src/events.rs` | Add `UsageReport`, `MemoryAccess`, `SkillLoaded`, `LearningEvent`, `SubagentSpawned` variants |
| `crates/agent/src/intent_pipeline/pipeline.rs` | Emit `UsageReport` after `record_usage()` |
| `crates/tools/src/memory_tool.rs` | Emit `MemoryAccess` events via `event_tx` |
| `crates/agent/src/subagent.rs` | Emit `SubagentSpawned` on spawn |
| `crates/desktop-shared/src/events.rs` | Add 6 new payload structs + event name constants |
| `crates/desktop/src/commands/chat.rs` | Replace `_ => {}` with full relay handlers, accumulate `TransparencyData`, persist to metadata |

### Frontend (TypeScript)

| File | Change |
|------|--------|
| `desktop-ui/src/lib/types.ts` | Add `TransparencyData` and all sub-types |
| `desktop-ui/src/hooks/useAgentStream.ts` | Add transparency state + 6 new event listeners |
| `desktop-ui/src/hooks/useChatSession.ts` | Pass transparency through to messages |
| `desktop-ui/src/components/chat/MessageList.tsx` | Render transparency components when toggled on |
| `desktop-ui/src/components/views/Chat.tsx` | Add `TransparencyToggle` to header, manage toggle state |
| `desktop-ui/src/components/chat/TokenBadge.tsx` | **New** — compact token/cost badge |
| `desktop-ui/src/components/chat/TokenDetail.tsx` | **New** — expandable token breakdown |
| `desktop-ui/src/components/chat/KlyntbotBox.tsx` | **New** — memory/files collapsible box |
| `desktop-ui/src/components/chat/ContextBox.tsx` | **New** — skills collapsible box |
| `desktop-ui/src/components/chat/ExecutionDetail.tsx` | **New** — agents/memory-writes/learning section |
| `desktop-ui/src/components/chat/TransparencyToggle.tsx` | **New** — header toggle button |
