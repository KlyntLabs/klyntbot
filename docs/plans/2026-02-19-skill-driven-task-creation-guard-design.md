# Skill-Driven Task Creation Guard

**Date:** 2026-02-19
**Status:** Approved

## Problem

When a user types a vague task like "create for me a task: buy", klyntbot:
1. The LLM hallucates details — expands "buy" to "Buy groceries" with description "Weekly shopping", priority P3, due date tomorrow, tags "shopping"
2. The enrichment engine auto-applies estimated time (~60 min) without asking
3. The system prompt instruction "use ask_user FIRST" is ignored by the LLM

**Root cause:** The `todo` skill's detailed instructions are never injected into the system prompt (not marked `always: true`), so the LLM only sees a one-line summary and ignores the ask-first workflow.

## Design

Two-layer fix: stronger skill instructions + code-level guard.

### Layer 1: Always-Load the Todo Skill

**File:** `skills/todo/SKILL.md`

- Add `"always": true` to frontmatter metadata so the full skill content is injected into every system prompt
- Rewrite the skill to be more directive about ask-first workflow
- Remove the "create first, then ask" fallback — the default should be "ask first, then create"

**Before (summary only in prompt):**
```xml
<skill name="todo" available="true">
  <description>Task management best practices...</description>
</skill>
```

**After (full content in prompt):**
```
# Skill: todo

[Full workflow instructions including ask-first rules]
```

### Layer 2: Code Guard in TodoTool

**File:** `crates/tools/src/todo.rs` (in the `"add"` action)

Add a heuristic guard that detects when the LLM is hallucinating task details:

```
IF action == "add"
  AND title has <= 3 words
  AND 2+ optional fields are filled (description, priority, due_date, tags)
  AND no ask_user interaction happened in this conversation turn
THEN
  return soft rejection: "Task has unconfirmed details. Use ask_user to verify with the user first, or call todo add with only the title."
```

**Implementation:** Add a `confirmed` boolean parameter to the todo tool schema. The LLM must set `confirmed: true` when it has gathered details via `ask_user`. If `confirmed` is not set and the guard triggers, reject.

This is simpler than tracking ask_user state across the conversation — it's a single parameter the LLM must explicitly set.

### Layer 3: Skill-Based Mode Selection

Three built-in creation modes as skills:

| Skill | `always` | Guard | Behavior |
|-------|----------|-------|----------|
| `todo` (default) | true | ON | Ask-first via `ask_user`, then create with confirmed details |
| `todo-yolo` | false | OFF | Auto-enrich from conversation context, present for approval |
| `todo-party` | false | OFF | Interactive brainstorming, one question at a time |

Users configure the default mode in `config.json`:

```json
{
  "todo": {
    "creationMode": "ask-first"
  }
}
```

**Valid values:**
- `"ask-first"` (default) — uses `todo` skill, requires `ask_user` before creating vague tasks
- `"yolo"` — uses `todo-yolo` skill, auto-enriches from conversation context
- `"party"` — uses `todo-party` skill, interactive brainstorming

Users can also override per-session by saying "use todo-yolo mode" in chat.

When `creationMode` is `"yolo"` or `"party"`, the `confirmed` parameter guard in TodoTool is disabled.

### Enrichment Engine Changes

**File:** `crates/tools/src/todo.rs` lines 408-452

- Auto-enrichment still runs but only for fields the user didn't explicitly set
- Add the enrichment suggestions to the response message so the user can see what was auto-applied
- Respect the `autoApplyThreshold` from config (currently hardcoded at 0.7)

## Files to Modify

1. `skills/todo/SKILL.md` — add `always: true`, rewrite ask-first workflow
2. `crates/tools/src/todo.rs` — add `confirmed` parameter, guard logic in `"add"` action, read `creationMode` from config
3. `crates/agent/src/context.rs` — remove hardcoded ask_user instruction (now in skill)
4. `crates/config/src/lib.rs` (or relevant config struct) — add `todo.creationMode` field with serde default `"ask-first"`

## Files Unchanged

- `crates/agent/src/enrichment/` — enrichment engine stays the same
- `skills/todo-yolo/SKILL.md` — already describes auto-enrich behavior
- `skills/todo-party/SKILL.md` — already describes brainstorm behavior

## Success Criteria

1. `klyntbot chat "create task: buy"` → asks user for details before creating
2. `klyntbot chat "create task: buy milk from store, priority high, due tomorrow"` → creates immediately (enough detail provided)
3. User can switch to YOLO mode and get auto-enriched tasks without being asked
4. Enrichment suggestions are visible in the task creation response
