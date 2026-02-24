# Learning Loop + Hierarchical Sub-Agents Design

> Date: 2026-02-24
> Feature: Strategy persistence, specialized sub-agents, goal metrics, user satisfaction
> Status: Approved
> Pre-production: Breaking changes acceptable

---

## Overview

Close the feedback loops in klyntbot's adaptive orchestration system. Four changes:

1. **Strategy outcome persistence** — Write predicted vs actual strategy to `strategy_records` after every message. Unblocks the orchestrator's historical accuracy context.
2. **Specialized sub-agent profiles** — LLM-selected profiles (General, Research, Code, Analyst) with different tool sets and system prompts.
3. **Goal completion metrics** — Typed columns on `goals` table replacing JSON blobs. Computed in `PlanCompletionHandler`, exposed via `GoalTool`.
4. **User satisfaction via reactions** — Channel emoji reactions map to satisfaction scores, backfilled to `strategy_records`.

---

## 1. Strategy Outcome Persistence

### Pipeline Change

`AgentPipeline` gains a non-optional `StrategyRepo` field:

```rust
pub struct AgentPipeline {
    orchestrator: Arc<Orchestrator>,
    context_engine: ContextEngine,
    engine_dispatch: Arc<EngineDispatch>,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    strategy_repo: StrategyRepo,        // NEW: non-optional
    config: PipelineConfig,
}
```

### New Step 6 in `process_message()`

After cost tracking (Step 5), write a strategy record:

```rust
// Step 6: Record strategy outcome (best-effort)
let record = StrategyRecordRow {
    id: Uuid::new_v4(),
    timestamp: Utc::now(),
    request_id: Uuid::new_v4().to_string(),
    predicted_strategy: format!("{:?}", classification.strategy),
    actual_strategy: strategy_name.clone(),
    escalation_count: dispatch_result.escalation_count as i32,
    iterations_used: dispatch_result.iterations_used as i32,
    max_iterations: strategy_max_iters as i32,
    success: validation.is_valid,
    user_satisfaction: None,       // filled later by reaction handler
    response_time_ms: start.elapsed().as_millis() as i64,
    chat_id: Some(ctx.chat_id.clone()),
};
if let Err(e) = self.strategy_repo.create(&record).await {
    warn!("Pipeline: failed to record strategy: {}", e);
}
```

### `DispatchResult` Change

Add `iterations_used: u32` field. Each engine surfaces its iteration count in the result.

### `strategy_records` Schema Addition

```sql
ALTER TABLE strategy_records ADD COLUMN chat_id TEXT;
```

### Files Changed

| File | Change |
|------|--------|
| `crates/agent/src/pipeline.rs` | Add `strategy_repo` field, Step 6 write |
| `crates/agent/src/execution/dispatch.rs` | Add `iterations_used` to `DispatchResult` |
| `crates/agent/src/execution/react_plus.rs` | Surface iteration count in `ReactOutcome` |
| `crates/agent/src/execution/direct.rs` | Return `iterations_used: 1` |
| `crates/agent/src/agent_loop/builder.rs` | Pass `StrategyRepo` to pipeline constructor |
| `crates/storage/migrations/` | Add `chat_id` column to `strategy_records` |
| `crates/storage/src/rows/learning.rs` | Add `chat_id` to `StrategyRecordRow` |

---

## 2. Specialized Sub-Agent Profiles

### Profile Enum

```rust
pub enum SubagentProfile {
    General,    // filesystem + shell + web (default)
    Research,   // web + filesystem read-only, no shell
    Code,       // filesystem + shell, no web
    Analyst,    // filesystem read-only only (pure reasoning)
}
```

### SpawnTool Parameter

```json
{
  "task": "Research the latest Rust async patterns",
  "label": "async research",
  "profile": "research"
}
```

`profile` defaults to `"general"` if omitted.

### Tool Registration Per Profile

| Profile | Filesystem | Shell | Web Search | Web Fetch |
|---------|-----------|-------|-----------|-----------|
| General | read + write | yes | yes | yes |
| Research | read only | no | yes | yes |
| Code | read + write | yes | no | no |
| Analyst | read only | no | no | no |

### System Prompt Per Profile

- **General**: Current generic subagent prompt.
- **Research**: "You are a research specialist. Focus on finding and synthesizing information from web sources and files."
- **Code**: "You are a code specialist. Focus on reading, writing, and executing code to complete the task."
- **Analyst**: "You are an analyst. Reason about the information provided. You have read-only file access but no shell or web tools."

### Iteration Limits Per Profile

| Profile | Max Iterations |
|---------|---------------|
| General | 15 |
| Research | 10 |
| Code | 15 |
| Analyst | 5 |

### SpawnHandler Trait Change

```rust
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        profile: SubagentProfile,    // NEW
        origin_channel: String,
        origin_chat_id: String,
    ) -> String;
}
```

### Files Changed

| File | Change |
|------|--------|
| `crates/tools/src/spawn.rs` | Add `profile` param to `SpawnHandler` trait + `SpawnTool` |
| `crates/agent/src/subagent.rs` | `SubagentProfile` enum, profile-based tool registration + prompts + iteration limits |

---

## 3. Goal Completion Metrics

### Schema Change

Replace `metrics` and `metadata` JSON columns with typed columns:

```sql
-- Drop JSON blob columns
-- Add typed columns
ALTER TABLE goals ADD COLUMN plans_completed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE goals ADD COLUMN plans_failed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE goals ADD COLUMN avg_duration_ms INTEGER;
ALTER TABLE goals ADD COLUMN last_plan_at TEXT;
```

Remove `metrics` and `metadata` from `GoalRow`.

### GoalRepo Additions

```rust
/// Increment plans_completed and update avg_duration_ms.
pub async fn increment_completed(
    &self, id: Uuid, plan_duration_ms: i64,
) -> Result<(), StorageError>;

/// Increment plans_failed.
pub async fn increment_failed(&self, id: Uuid) -> Result<(), StorageError>;
```

`increment_completed` computes rolling average:
```
new_avg = ((old_avg * plans_completed) + new_duration) / (plans_completed + 1)
```

### PlanCompletionHandler Change

Replace JSON metadata manipulation with:

```rust
// On plan success:
let duration_ms = (plan.completed_at - plan.created_at).num_milliseconds();
goal_repo.increment_completed(goal_id, duration_ms).await;

// On plan failure:
goal_repo.increment_failed(goal_id).await;
```

### GoalTool Metrics Action

```
action: "metrics"
params: { "goal_id": "uuid" }
```

Response format:
```
Goal: "Ship v1.0"
Completion rate: 75% (3 of 4 plans)
Average plan duration: 2.4 hours
Last activity: 2026-02-24 10:30 UTC
Active plans: 1
```

`active_plans` computed via `PlanRepo` query (count plans with status `Executing` or `Approved` linked to goal).

### Files Changed

| File | Change |
|------|--------|
| `crates/storage/migrations/` | Add typed columns, drop JSON columns |
| `crates/storage/src/rows/goal.rs` | Update `GoalRow` struct |
| `crates/storage/src/repos/goal.rs` | Update queries, add `increment_completed()`, `increment_failed()` |
| `crates/agent/src/plan_completion_handler.rs` | Use typed columns |
| `crates/agent/src/goal_handler.rs` | Add `goal_metrics()` method |
| `crates/tools/src/goal.rs` | Add `metrics` action |

---

## 4. User Satisfaction via Channel Reactions

### Reaction Mapping

```
Positive (satisfaction = 1.0):  👍 ❤️ 🎉
Negative (satisfaction = 0.0):  👎 😕
```

### MessageKind Enum

```rust
pub enum MessageKind {
    Text,       // default
    Reaction,   // emoji reaction on a previous message
}

pub struct InboundMessage {
    pub channel: String,
    pub sender: String,
    pub chat_id: String,
    pub content: String,
    pub kind: MessageKind,  // NEW
}
```

### Data Flow

```
User reacts 👍 on bot message
  → Channel emits InboundMessage { kind: Reaction, content: "👍", chat_id }
  → AgentLoop receives reaction
  → Maps emoji to satisfaction score
  → Finds most recent strategy_record for this chat_id (within 5 minutes)
  → Calls strategy_repo.set_satisfaction_for_chat(chat_id, since, score)
  → No response sent (silent update)
```

### StrategyRepo Addition

```rust
/// Update user_satisfaction on the most recent record for a chat.
pub async fn set_satisfaction_for_chat(
    &self,
    chat_id: &str,
    since: DateTime<Utc>,
    satisfaction: f32,
) -> Result<bool, StorageError>;
```

Query: `UPDATE strategy_records SET user_satisfaction = ?1 WHERE chat_id = ?2 AND timestamp >= ?3 ORDER BY timestamp DESC LIMIT 1`

### Channel Changes

Telegram, Discord, and Slack add reaction event handlers that emit `InboundMessage { kind: Reaction }`. Channels without reaction support (Email, QQ) are unchanged.

### AgentLoop Change

In the message processing match, check `kind`:
- `MessageKind::Text` → existing `process_message()` flow
- `MessageKind::Reaction` → satisfaction handler (no LLM call)

### Files Changed

| File | Change |
|------|--------|
| `crates/bus/src/lib.rs` | Add `MessageKind` enum, `kind` field on `InboundMessage` |
| `crates/storage/src/repos/strategy.rs` | Add `set_satisfaction_for_chat()` |
| `crates/agent/src/agent_loop/` | Handle `MessageKind::Reaction` |
| `crates/channels/src/telegram.rs` | Emit reaction events |
| `crates/channels/src/discord.rs` | Emit reaction events |
| `crates/channels/src/slack.rs` | Emit reaction events |

---

## Testing Strategy

### Unit Tests

- Strategy persistence: mock `StrategyRepo`, verify `create()` called with correct fields after pipeline run
- SubagentProfile: verify tool registration per profile, iteration limits, prompt content
- Goal metrics: `increment_completed()` computes correct rolling average, `increment_failed()` increments
- Reaction mapping: emoji → satisfaction score mapping, unknown emoji ignored
- MessageKind: default is `Text`, reactions don't trigger LLM calls

### Integration Tests

- Full pipeline round-trip: message → classify → execute → strategy record written → verify record fields
- Subagent spawn with profile: verify restricted tool set (Research profile can't use shell)
- Goal metrics end-to-end: create goal → link plan → complete plan → verify metrics
- Reaction flow: simulate reaction InboundMessage → verify satisfaction updated on correct record

---

## Out of Scope

- Configurable profiles (config-driven profiles can be added later)
- Granular satisfaction scales (1-5 rating — binary is sufficient for now)
- Real-time strategy dashboards (query via LearningTool status action)
- Cross-subagent communication (subagents remain isolated)
- Hot-reload of profiles (restart required)
