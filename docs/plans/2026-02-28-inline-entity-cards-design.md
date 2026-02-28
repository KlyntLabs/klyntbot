# Inline Entity Cards in Chat

**Date**: 2026-02-28
**Status**: Approved

## Problem

When klyntbot creates entities (tasks, projects, etc.) via chat, the response is plain markdown text with IDs. Users have no quick way to navigate to the created entity — they must manually go to the relevant page and find it.

## Solution

Render clickable entity cards inline in chat messages, appearing after the assistant's markdown content. Cards show key entity details and navigate to the entity's detail/list page on click.

## Supported Entity Types

- Task (todo)
- Project
- Goal (strategic)
- Finance (account, transaction, budget, financial goal)
- Cron job

## Backend: New `EntityCreated` Event

New variant in `AgentEvent` (`crates/agent/src/events.rs`):

```rust
EntityCreated {
    entity_type: String,        // "task", "project", "goal", "finance_account", etc.
    entity_id: String,
    title: String,
    subtitle: Option<String>,   // e.g. "Priority 1 · Due today"
    route: Option<String>,      // e.g. "/tasks/53d79f09" — None if no detail page
    icon_hint: String,          // "task", "project", "dollar", "target", "clock"
    metadata: HashMap<String, serde_json::Value>,
}
```

### Event Emission Points

Each tool emits `EntityCreated` after successful creation:

| Tool | Action | icon_hint |
|------|--------|-----------|
| TodoTool | `add` | `"task"` |
| ProjectTool | `create` | `"project"` |
| GoalTool | `create` | `"target"` |
| FinanceTool | account/tx/budget/goal creation | `"dollar"` |
| CronTool | `add` | `"clock"` |

Tools need access to an event emitter (`EventSender`). This is injected via the existing tool execution context.

## Frontend: EntityCard Component

### Data Flow

```
Tool executes
  → emits EntityCreated event via EventSender
  → WebSocket delivers to frontend
  → useAgent hook captures event, attaches to ChatMessage.entityCards[]
  → MessageBubble renders EntityCard components after markdown content
```

### Message Structure Change

```typescript
interface EntityCardData {
  entityType: string;
  entityId: string;
  title: string;
  subtitle?: string;
  route?: string;
  iconHint: string;
  metadata?: Record<string, unknown>;
}

interface ChatMessage {
  // ... existing fields
  entityCards?: EntityCardData[];  // NEW
}
```

### Card Layout

Position: After the full assistant message (below markdown text).

```
┌──────────────────────────────────────┐
│ [icon]  Title                    [→] │
│         Subtitle (dimmed)            │
└──────────────────────────────────────┘
```

- Left: Icon mapped from `iconHint`
- Center: Title (bold) + subtitle (dimmed, smaller)
- Right: Chevron indicating clickability
- Dark theme styling with subtle border
- Click navigates via React Router

### Navigation Rules

| Entity | Route | Target |
|--------|-------|--------|
| Task | `/tasks/:id` | Task detail page |
| Project | `/projects/:id` | Project detail page |
| Goal | `/plans` | Plans page (no goal detail page) |
| Finance | `/finance` | Finance list page |
| Cron | `/cron` | Cron list page |

## Scope Summary

### Rust Changes
1. Add `EntityCreated` variant to `AgentEvent` enum
2. Add `EntityCardData` struct
3. Inject `EventSender` into tool execution context (or pass through existing mechanism)
4. Emit events from 5 tools (todo, project, goal, finance, cron)

### Frontend Changes
1. Add `EntityCardData` type to `types.ts`
2. Handle `entityCreated` event in `useAgent.ts`
3. Attach entity cards to `ChatMessage`
4. Create `EntityCard` component
5. Render cards in `MessageBubble.tsx` after markdown content
6. Handle navigation on click
