# Area Requirement for Task Creation

**Date:** 2026-03-03
**Status:** Approved
**Approach:** A — Skill + Context Source (minimal code changes)

## Problem

Tasks require `area_id` (NOT NULL FK in DB, `required_str("area_id")` in tool handler), but the LLM has no awareness of areas:

1. Todo skill (`skills/todo/SKILL.md`) never mentions `area_id`
2. No `AreaSource` context source — LLM doesn't know what areas exist
3. `ActionRepo::to_context_string()` omits area info from active task listing

Result: LLM creates tasks without `area_id` (tool error) or hallucinates area IDs.

## Design

### 1. AreaSource Context Source

**New file:** `crates/agent/src/context_sources/area.rs`

- Injects available areas (name + ID) into system prompt
- Queries `AreaRepo::list(Some("active"))` — areas only, no projects/OKRs
- Priority: 75 (above TodoSource at 70, so areas appear before tasks)
- TTL cache: 60 seconds (matches TodoSource)
- Registered in `builder.rs` alongside other sources

**Output format:**
```
Available areas:
- Personal (ID: area_abc)
- Work (ID: area_def)
- Health (ID: area_ghi)
```

Projects and OKRs are discovered lazily after area is confirmed (via `area show` or `project list`).

### 2. Todo Skill Update

**File:** `skills/todo/SKILL.md`

Add **Step 0: Determine area** before title assessment:

- If 1 active area: auto-assign, mention which area in response
- If multiple active areas: ask user via `ask_user` with `single_select` of available areas
- If user message specifies area (e.g. "add task to Work: fix bug"): use directly

Update `ask_user` example to include area selection question:
```json
{
  "id": "area",
  "title": "Area",
  "text": "Which area does this belong to?",
  "type": "single_select",
  "options": [{"value": "area_abc", "label": "Personal"}, {"value": "area_def", "label": "Work"}]
}
```

Update all `todo add` examples to include `area_id`.

Note that `key_result_id` (OKR linkage) is optional and can boost confidence score, discovered via `area show` → `project list` after area confirmation.

### 3. to_context_string() Enhancement

**File:** `crates/storage/src/repos/action_repo.rs` (line ~838)

JOIN area name into the active tasks query:
```sql
SELECT a.title, a.status, a.priority, a.focused_at, ar.name as area_name
FROM actions a JOIN areas ar ON a.area_id = ar.id
```

**New output format:**
```
Active tasks:
- [todo] P2 Buy milk (Personal)
- [doing] [FOCUSED] P1 Fix login bug (Work)
```

## Files Changed

| File | Change |
|------|--------|
| `crates/agent/src/context_sources/area.rs` | New — AreaSource context source |
| `crates/agent/src/context_sources/mod.rs` | Register AreaSource module |
| `crates/agent/src/agent_loop/builder.rs` | Wire AreaSource into context engine |
| `crates/storage/src/repos/action_repo.rs` | JOIN area name in to_context_string() |
| `skills/todo/SKILL.md` | Add area requirement, update examples |
