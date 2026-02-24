# Learning Tool Consolidation + Interactive Plan Approval — Design

## Overview

Two targeted improvements to klyntbot's agent pipeline:

1. **Learning tool consolidation**: Merge `learning_outcomes` into `strategy_records` so the learning tool shows real data from every chat interaction, not just plan executions.
2. **Interactive plan approval**: Plans are previewed and presented to the user for approve/revise/abandon before persisting, using the existing `ask_user` blocking mechanism.

---

## 1. Learning Tool — Consolidate to `strategy_records`

### Problem

`LearningHandlerImpl` reads from `learning_outcomes` (only written during plan step execution). Pipeline Step 6 writes to `strategy_records` on every chat message. Normal chat never populates `learning_outcomes`, so the learning tool always returns "No data yet."

### Solution

Stop writing to `learning_outcomes`. Add tool outcome columns to `strategy_records`. `LearningHandlerImpl` reads only `StrategyRepo`.

### Schema Change (migration 003)

```sql
ALTER TABLE strategy_records ADD COLUMN tool_name TEXT;
ALTER TABLE strategy_records ADD COLUMN tool_success INTEGER;
ALTER TABLE strategy_records ADD COLUMN tool_duration_ms INTEGER;
```

### Updated `strategy_records` Row

```
id, session_key, chat_id, predicted_strategy, actual_strategy,
escalation_count, iterations_used, response_time_ms,
tool_name, tool_success, tool_duration_ms,
user_satisfaction, created_at
```

### Data Flow

```
AgentPipeline::process_message()
  └─ Step 6: StrategyRepo::create()
       └─ Writes predicted/actual strategy, escalation, iterations,
          response time, tool_name, tool_success, tool_duration_ms, chat_id

LearningHandlerImpl
  └─ StrategyRepo (strategy_records only)
       └─ get_status() → record count, strategy accuracy, avg response time, satisfaction avg
       └─ analyze() → strategy distribution, escalation rate, per-tool success, satisfaction by strategy
       └─ history() → threshold changes over time
```

### Learning Tool Response Examples

**`status` action:**
```json
{
  "current_threshold": 0.7,
  "total_strategy_records": 47,
  "strategy_accuracy": 0.89,
  "avg_response_time_ms": 2340,
  "avg_satisfaction": 0.85,
  "suggested_threshold": 0.7,
  "per_tool": {"todo": {"calls": 12, "success_rate": 0.92}, "filesystem": {"calls": 8, "success_rate": 1.0}}
}
```

**`analyze` action:**
```json
{
  "strategy_distribution": {"DirectResponse": 28, "ToolAssisted": 15, "AutonomousTask": 4},
  "escalation_rate": 0.06,
  "satisfaction_by_strategy": {"DirectResponse": 0.92, "ToolAssisted": 0.80}
}
```

### Multiple Tool Calls Per Message

Pipeline Step 6 fires once per `process_message()`. If the ReAct+ engine called multiple tools, record the last tool (most likely the one that produced the answer) or leave `tool_name` null for multi-tool turns.

### Dead Code Removal

- Remove `OutcomeRecorder` (the writer to `learning_outcomes`)
- Remove `OutcomeStore` (the reader)
- Remove `OutcomeRepo` or leave as dead code (don't drop the `learning_outcomes` table — just stop writing)
- Remove `outcome_recorder` field from `AgentLoop`
- Update builder to stop constructing `OutcomeStore`/`OutcomeRecorder`

---

## 2. Interactive Plan Approval via `ask_user`

### Problem

`PlanTool::create` saves to DB immediately, then generates steps. User has no chance to review, reject, or revise before the plan is persisted.

### Solution

Generate steps as preview, present via `ask_user`, loop until approved or abandoned, only then persist.

### Flow

```
User: "Create a plan to refactor the auth module"
  |
  v
PlanTool::execute("create")
  |
  v
1. handler.preview_steps(title, description)
   -> LLM generates Vec<PlanStepDraft> without saving
  |
  v
2. Format plan summary + steps as readable text
  |
  v
3. ask_user with choices: [Approve / Revise / Abandon]
   - CLI: blocks on interaction_tx (real interactive prompt)
   - Channel: falls back to conversational text (existing behavior)
  |
  +---> Approve: handler.create_plan(title, description, steps) -> save to DB as Draft
  |
  +---> Revise: user gives feedback -> re-call preview_steps with feedback appended -> goto 3
  |
  +---> Abandon: return "Plan abandoned" -> nothing saved
```

### Component Changes

| Component | Change |
|-----------|--------|
| `PlanHandler` trait | Add `preview_steps(title, description) -> Result<Vec<PlanStepDraft>>` |
| `PlanHandlerImpl` | Implement `preview_steps` — same LLM call as `generate_steps` but returns without persisting |
| `PlanTool::execute("create")` | New flow: preview -> ask_user loop -> persist only on approve |

### Revision Loop

When user says "Revise", their feedback is appended to the description for the next `preview_steps` call. The LLM sees the original description + user feedback and generates improved steps. Max 3 revision rounds before auto-saving as Draft.

### Channel Fallback

For Telegram/Discord/Slack where `interaction_tx` is `None`: `ask_user` returns text instructions. The LLM presents the plan conversationally and waits for the user's next message. Multi-turn approval happens naturally through conversation.

### What Doesn't Change

- `PlanTool::execute("approve")` — still transitions Draft -> Approved
- `PlanTool::execute("execute")` — still transitions Approved -> Executing
- Plan state machine: Draft -> Approved -> Executing -> Completed/Failed/Abandoned

---

## Error Handling

### Learning Tool
- Empty `strategy_records` → "No data yet — strategy records accumulate as you chat"
- `tool_name`/`tool_success`/`tool_duration_ms` all nullable — multi-tool turns leave them null

### Plan Approval
- `preview_steps` LLM fails → return error asking user to try a more specific description
- LLM returns 0 steps → present with option to revise or abandon
- `ask_user` timeout → save as Draft with whatever steps exist
- Max 3 revisions → auto-save as Draft with note to review later
- `interaction_tx` is `None` → conversational fallback, no blocking

---

## Scope

- No new crates or traits
- Migration 003 adds 3 columns to `strategy_records`
- `LearningHandlerImpl` swaps `OutcomeStore` for `StrategyRepo`
- `PlanHandler` trait gains one method (`preview_steps`)
- `PlanTool::execute("create")` gains ask_user loop
- Dead code cleanup: `OutcomeRecorder`, `OutcomeStore`, `OutcomeRepo` references
