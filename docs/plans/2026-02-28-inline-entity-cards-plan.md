# Inline Entity Cards Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Render clickable entity cards inline in chat messages when the agent creates tasks, projects, goals, finance entries, or cron jobs — enabling one-click navigation to the created entity.

**Architecture:** Add an `EntityCard` struct to `common` crate (Layer 0). Add an `entity_tx` channel to `RoutingContext` in `tools-core`. Each creation tool sends an `EntityCard` through the channel. `ExecutionCore` in the `agent` crate drains the channel after tool execution and emits `AgentEvent::EntityCreated` events over WebSocket. The frontend captures these events, attaches them to `ChatMessage`, and renders `EntityCard` components below the markdown content.

**Tech Stack:** Rust (tokio mpsc, serde), React (TypeScript), Lucide icons, React Router navigation, Framer Motion animation

---

### Task 1: Add `EntityCard` struct to `common` crate

**Files:**
- Create: `crates/common/src/entity_card.rs`
- Modify: `crates/common/src/lib.rs`

**Step 1: Create the `EntityCard` struct**

```rust
// crates/common/src/entity_card.rs
//! Entity card data emitted when tools create entities.

use serde::Serialize;
use std::collections::HashMap;

/// Card data for an entity created by a tool.
/// Sent through RoutingContext to the agent event stream.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityCard {
    pub entity_type: String,
    pub entity_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub route: Option<String>,
    pub icon_hint: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}
```

**Step 2: Register the module and re-export**

In `crates/common/src/lib.rs`, add:
```rust
pub mod entity_card;
pub use entity_card::EntityCard;
```

**Step 3: Run build to verify**

Run: `cargo build -p common`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add crates/common/src/entity_card.rs crates/common/src/lib.rs
git commit -m "feat(common): add EntityCard struct for inline entity cards"
```

---

### Task 2: Add `entity_tx` to `RoutingContext`

**Files:**
- Modify: `crates/tools-core/src/lib.rs:47-85` (RoutingContext)

**Step 1: Add the entity_tx field**

Add to `RoutingContext` struct (after `is_direct_mode`):
```rust
    /// Channel for tools to emit entity cards (e.g., after creating a task).
    /// The execution layer drains this and emits AgentEvent::EntityCreated.
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
```

**Step 2: Update `RoutingContext::new`**

Add `entity_tx: None` to the `new()` method body.

**Step 3: Update `RoutingContext::with_interaction`**

Add `entity_tx: None` to the `with_interaction()` method body.

**Step 4: Run build to verify**

Run: `cargo build -p tools-core`
Expected: compiles with no errors

**Step 5: Commit**

```bash
git add crates/tools-core/src/lib.rs
git commit -m "feat(tools-core): add entity_tx channel to RoutingContext"
```

---

### Task 3: Add `EntityCreated` variant to `AgentEvent`

**Files:**
- Modify: `crates/agent/src/events.rs`

**Step 1: Add the variant**

Add after the `PlanCompleted` variant:
```rust
    /// An entity was created by a tool (task, project, goal, etc.).
    EntityCreated(common::EntityCard),
```

**Step 2: Run build to verify**

Run: `cargo build -p agent`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "feat(agent): add EntityCreated event variant"
```

---

### Task 4: Wire entity channel in `ExecutionCore`

**Files:**
- Modify: `crates/agent/src/execution/core.rs` (tool execution loop, ~lines 260-335)

**Step 1: Create entity channel and inject into RoutingContext**

In the tool execution section (around line 260), before the `futures` Vec is constructed, create an mpsc channel and inject the sender into the routing context clone:

```rust
// Create entity card channel for this batch of tool calls
let (entity_tx, mut entity_rx) = tokio::sync::mpsc::channel::<common::EntityCard>(16);
```

Inside the `.map(|tc| { ... })` closure, when cloning `ctx`:
```rust
let mut ctx = routing_ctx.clone();
ctx.entity_tx = Some(entity_tx.clone());
```

Drop the sender after all futures are collected so the receiver knows when all tools are done:
```rust
drop(entity_tx);
```

**Step 2: Drain entity cards after tool execution and emit events**

After `let results = join_all(futures).await;` (around line 337), drain the entity channel and emit events:

```rust
// Emit EntityCreated events for any entities tools created
if let Some(ref tx) = event_tx {
    while let Ok(card) = entity_rx.try_recv() {
        let _ = tx.send(crate::events::AgentEvent::EntityCreated(card)).await;
    }
}
```

**Step 3: Run build to verify**

Run: `cargo build -p agent`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "feat(agent): wire entity card channel in ExecutionCore"
```

---

### Task 5: Emit entity cards from TodoTool

**Files:**
- Modify: `crates/feature-todo/src/tool/actions/add.rs` (handle_add, ~line 99-104)

**Step 1: Update `handle_add` signature to accept RoutingContext**

The `handle_add` method currently takes `&self, p: &ParamExtractor<'_>`. Add `ctx: &RoutingContext` parameter:

```rust
pub(crate) async fn handle_add(&self, p: &ParamExtractor<'_>, ctx: &RoutingContext) -> Result<String> {
```

**Step 2: Emit EntityCard after successful creation**

Before the final `Ok(format!(...))`, add:

```rust
// Emit entity card for inline display
if let Some(ref tx) = ctx.entity_tx {
    let subtitle = {
        let mut parts = Vec::new();
        if let Some(p) = created.priority {
            parts.push(format!("P{}", p));
        }
        if let Some(ref d) = created.due_date {
            parts.push(format!("Due {}", d.format("%b %d")));
        }
        if parts.is_empty() { None } else { Some(parts.join(" · ")) }
    };
    let _ = tx.send(common::EntityCard {
        entity_type: "task".to_string(),
        entity_id: created.id.clone(),
        title: created.title.clone(),
        subtitle,
        route: Some(format!("/tasks/{}", created.id)),
        icon_hint: "task".to_string(),
        metadata: std::collections::HashMap::new(),
    }).await;
}
```

**Step 3: Update the call site in TodoTool::execute**

Find where `handle_add` is called (in the tool's execute method) and pass `ctx`:

```rust
// Before: self.handle_add(&p).await
// After:
self.handle_add(&p, ctx).await
```

**Step 4: Run build to verify**

Run: `cargo build -p feature-todo`
Expected: compiles with no errors

**Step 5: Commit**

```bash
git add crates/feature-todo/src/tool/actions/add.rs
# Also add the file where execute() calls handle_add
git commit -m "feat(todo): emit entity card on task creation"
```

---

### Task 6: Emit entity cards from ProjectTool

**Files:**
- Modify: `crates/tools/src/project_tool.rs` (~lines 95-118)

**Step 1: Add entity card emission after project creation**

In the `"create"` match arm, after `let created: Project = created_row.into();`, add:

```rust
if let Some(ref tx) = ctx.entity_tx {
    let _ = tx.send(common::EntityCard {
        entity_type: "project".to_string(),
        entity_id: created.id.clone(),
        title: created.name.clone(),
        subtitle: None,
        route: Some(format!("/projects/{}", created.id)),
        icon_hint: "project".to_string(),
        metadata: std::collections::HashMap::new(),
    }).await;
}
```

Note: The execute method already receives `ctx` (as `_ctx`). Rename `_ctx` to `ctx`.

**Step 2: Run build to verify**

Run: `cargo build -p tools`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/tools/src/project_tool.rs
git commit -m "feat(project): emit entity card on project creation"
```

---

### Task 7: Emit entity cards from GoalTool

**Files:**
- Modify: `crates/tools/src/goal_tool.rs` (~lines 147-148)

**Step 1: Add entity card emission after goal creation**

After `let id = handler.create_goal(goal).await?;`, add:

```rust
if let Some(ref tx) = ctx.entity_tx {
    let _ = tx.send(common::EntityCard {
        entity_type: "goal".to_string(),
        entity_id: id.to_string(),
        title: title.to_string(),
        subtitle: Some(format!("Priority {}", priority)),
        route: Some("/plans".to_string()),
        icon_hint: "target".to_string(),
        metadata: std::collections::HashMap::new(),
    }).await;
}
```

Note: Check if execute receives `_ctx` and rename to `ctx`.

**Step 2: Run build to verify**

Run: `cargo build -p tools`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/tools/src/goal_tool.rs
git commit -m "feat(goal): emit entity card on goal creation"
```

---

### Task 8: Emit entity cards from CronTool

**Files:**
- Modify: `crates/tools/src/cron_tool.rs` (~lines 179-188)

**Step 1: Add entity card emission after cron job creation**

After `let job = handler.add_job(params).await?;`, add:

```rust
if let Some(ref tx) = ctx.entity_tx {
    let next_run = job.next_run_at_ms
        .map(format_timestamp_ms)
        .unwrap_or_else(|| "pending".to_string());
    let _ = tx.send(common::EntityCard {
        entity_type: "cron".to_string(),
        entity_id: job.id.clone(),
        title: job.name.clone(),
        subtitle: Some(format!("Next: {}", next_run)),
        route: Some("/cron".to_string()),
        icon_hint: "clock".to_string(),
        metadata: std::collections::HashMap::new(),
    }).await;
}
```

**Step 2: Run build to verify**

Run: `cargo build -p tools`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/tools/src/cron_tool.rs
git commit -m "feat(cron): emit entity card on cron job creation"
```

---

### Task 9: Emit entity cards from FinanceTool

**Files:**
- Modify: `crates/feature-finance/src/tool/accounts.rs` (~lines 82-93)

**Step 1: Update `account_add` to accept RoutingContext**

Add `ctx: &RoutingContext` parameter to `account_add`:

```rust
async fn account_add(&self, p: &ParamExtractor<'_>, ctx: &RoutingContext) -> Result<String> {
```

**Step 2: Add entity card emission after account creation**

After `let account = FinanceAccount::from(inserted);`, before the `json!` result construction:

```rust
if let Some(ref tx) = ctx.entity_tx {
    let _ = tx.send(common::EntityCard {
        entity_type: "finance_account".to_string(),
        entity_id: account.id.clone(),
        title: account.name.clone(),
        subtitle: Some(format!("{} · {}", account.account_type.as_str(), account.currency)),
        route: Some("/finance".to_string()),
        icon_hint: "dollar".to_string(),
        metadata: std::collections::HashMap::new(),
    }).await;
}
```

**Step 3: Update call site to pass ctx**

Find where `account_add` is called in the finance tool's execute method and pass `ctx`.

**Step 4: Run build to verify**

Run: `cargo build -p feature-finance`
Expected: compiles with no errors

**Step 5: Commit**

```bash
git add crates/feature-finance/src/tool/accounts.rs
# Add any other modified finance files
git commit -m "feat(finance): emit entity card on account creation"
```

---

### Task 10: Build and test all Rust changes

**Files:** None new — validation task

**Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: compiles with no errors, no warnings

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Run tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass

**Step 4: Fix any issues and commit**

If clippy or tests find issues, fix them and commit the fixes.

---

### Task 11: Add `EntityCardData` type and event handling in frontend

**Files:**
- Modify: `crates/dashboard/frontend/src/lib/types.ts`
- Modify: `crates/dashboard/frontend/src/lib/hooks/useAgent.ts`

**Step 1: Add TypeScript type**

In `types.ts`, after the `ChatMessage` interface (around line 449), add:

```typescript
/** Entity card data from an entityCreated WebSocket event. */
export interface EntityCardData {
  entityType: string;
  entityId: string;
  title: string;
  subtitle?: string;
  route?: string;
  iconHint: string;
  metadata?: Record<string, unknown>;
}
```

**Step 2: Add `entityCards` to `ChatMessage`**

```typescript
export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: Date;
  isStreaming?: boolean;
  toolCalls?: MessageToolCall[];
  entityCards?: EntityCardData[];  // NEW
}
```

**Step 3: Add event type**

After `ToolEndEvent`, add:

```typescript
export interface EntityCreatedEvent extends AgentEvent {
  type: 'entityCreated';
  entityType: string;
  entityId: string;
  title: string;
  subtitle?: string;
  route?: string;
  iconHint: string;
  metadata?: Record<string, unknown>;
}
```

**Step 4: Handle `entityCreated` event in useAgent.ts**

Add a ref to accumulate entity cards during streaming (next to `thinkingRef`):

```typescript
const entityCardsRef = useRef<EntityCardData[]>([]);
```

Add a case in the event switch (before `default:`):

```typescript
case 'entityCreated': {
  const card: EntityCardData = {
    entityType: event.entityType as string,
    entityId: event.entityId as string,
    title: event.title as string,
    subtitle: event.subtitle as string | undefined,
    route: event.route as string | undefined,
    iconHint: event.iconHint as string,
    metadata: event.metadata as Record<string, unknown> | undefined,
  };
  entityCardsRef.current = [...entityCardsRef.current, card];
  break;
}
```

**Step 5: Attach entity cards on `done` event**

In the `done` case, after capturing `toolCalls`, add:

```typescript
const entityCards = entityCardsRef.current.length > 0
  ? entityCardsRef.current
  : undefined;
```

Then include `entityCards` in both the `setMessages` update paths (the `prev.map(...)` and the `[...prev, {...}]` paths):

```typescript
// In the msg.id === id path:
? { ...msg, content: finalContent, isStreaming: false, toolCalls, entityCards }
// In the new message path:
{ id: generateId(), role: 'assistant', content: finalContent, timestamp: new Date(), isStreaming: false, toolCalls, entityCards }
```

Reset the ref at the end of `done`:

```typescript
entityCardsRef.current = [];
```

Also reset in `error` and `cancel` handlers:

```typescript
entityCardsRef.current = [];
```

**Step 6: Commit**

```bash
git add crates/dashboard/frontend/src/lib/types.ts crates/dashboard/frontend/src/lib/hooks/useAgent.ts
git commit -m "feat(dashboard): handle entityCreated events in useAgent"
```

---

### Task 12: Create `EntityCard` React component

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/components/EntityCard.tsx`

**Step 1: Create the component**

```tsx
import { useNavigate } from 'react-router';
import { motion } from 'motion/react';
import {
  CheckSquare,
  FolderKanban,
  Target,
  DollarSign,
  Clock,
  ChevronRight,
} from 'lucide-react';
import type { EntityCardData } from '../../../lib/types';

const ICON_MAP: Record<string, React.ComponentType<{ className?: string; strokeWidth?: number }>> = {
  task: CheckSquare,
  project: FolderKanban,
  target: Target,
  dollar: DollarSign,
  clock: Clock,
};

export function EntityCard({ card }: { card: EntityCardData }) {
  const navigate = useNavigate();
  const Icon = ICON_MAP[card.iconHint] ?? CheckSquare;

  const handleClick = () => {
    if (card.route) {
      navigate(card.route);
    }
  };

  return (
    <motion.button
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      onClick={handleClick}
      className="flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-left transition-colors cursor-pointer"
      style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        border: '1px solid var(--codex-border)',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.borderColor = 'var(--codex-accent)';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.borderColor = 'var(--codex-border)';
      }}
    >
      <div
        className="flex items-center justify-center w-8 h-8 rounded-md flex-shrink-0"
        style={{ backgroundColor: 'var(--codex-bg-tertiary)' }}
      >
        <Icon className="w-4 h-4" strokeWidth={1.5} style={{ color: 'var(--codex-accent)' }} />
      </div>
      <div className="flex-1 min-w-0">
        <div
          className="text-[13px] font-medium truncate"
          style={{ color: 'var(--codex-fg)' }}
        >
          {card.title}
        </div>
        {card.subtitle && (
          <div
            className="text-[11px] truncate mt-0.5"
            style={{ color: 'var(--codex-fg-subtle)' }}
          >
            {card.subtitle}
          </div>
        )}
      </div>
      <ChevronRight
        className="w-4 h-4 flex-shrink-0"
        strokeWidth={1.5}
        style={{ color: 'var(--codex-fg-subtle)' }}
      />
    </motion.button>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/components/EntityCard.tsx
git commit -m "feat(dashboard): add EntityCard component"
```

---

### Task 13: Render entity cards in `MessageBubble`

**Files:**
- Modify: `crates/dashboard/frontend/src/app/chat/components/MessageBubble.tsx`

**Step 1: Import the component**

Add import at top:
```typescript
import { EntityCard } from './EntityCard';
```

**Step 2: Render entity cards after markdown content**

In the assistant message section, after the streaming cursor `<span>` and before the timestamp `<div className="flex items-center gap-2 mt-2">`, add:

```tsx
{msg.entityCards && msg.entityCards.length > 0 && (
  <div className="flex flex-col gap-2 mt-3">
    {msg.entityCards.map((card) => (
      <EntityCard key={card.entityId} card={card} />
    ))}
  </div>
)}
```

**Step 3: Verify the build**

Run: `cd crates/dashboard/frontend && npm run build`
Expected: builds successfully with no errors

**Step 4: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/components/MessageBubble.tsx
git commit -m "feat(dashboard): render entity cards in MessageBubble"
```

---

### Task 14: Full integration build and manual test

**Files:** None new — validation task

**Step 1: Build entire workspace**

Run: `cargo build --workspace`
Expected: compiles with no errors

**Step 2: Run all tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass

**Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 4: Build frontend**

Run: `cd crates/dashboard/frontend && npm run build`
Expected: builds successfully

**Step 5: Manual smoke test**

Run `cargo run -- serve` and open the dashboard. Create a task via chat (e.g., "Create a task: Test entity cards with urgent priority"). Verify:
- Entity card appears below the assistant message
- Card shows task title and subtitle (priority/due date)
- Clicking the card navigates to `/tasks/:id`

**Step 6: Final commit if any fixes needed**

Fix any issues found during testing and commit.
