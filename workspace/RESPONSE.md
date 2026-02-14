# Response Formatting Rules

**MANDATORY**: These rules govern how you format ALL structured responses. You MUST follow them exactly.

## General Rules

- **NEVER use numbered lists with bullet sub-items for structured data.** Always use markdown tables instead.
- **Do NOT add preamble text** like "Here is your to-do list:" before tables. Jump straight to the stats line and table.
- Start list responses with a one-line stats summary (e.g., "📋 7 tasks · 1 done · 2 overdue")
- Keep conversational text short — let tables speak for themselves
- **NEVER end responses with filler phrases.** This includes ANY variation of: "Let me know if there's anything else!", "Just let me know!", "Feel free to ask!", "If you need to update or manage...", etc. These are strictly forbidden. Either end with a short actionable insight (e.g., "2 tasks are overdue") or end with the table itself.

## Table Patterns

Use tables for ANY list-like data: tasks, habits, expenses, goals. Adapt column headers to the domain but keep the table structure. Always include a stats summary line before tables.

### Todo List

```
📋 7 tasks · 1 done · 2 overdue

| # | Task | Priority | Status | Tag |
|---|------|----------|--------|-----|
| 1 | Write post | Medium | ☐ Todo | — |
| 2 | Upgrade server config | High | ☐ Todo | work |
| 3 | Write a story for Facebook | High | ✅ Done | — |
```

### Filtered View (e.g., "personal tasks")

```
🏷️ personal · 1 task

| # | Task | Priority | Status |
|---|------|----------|--------|
| 1 | Write a post for fb | High | ☐ Todo |
```

## Status Indicators

- `☐` Todo, `✅` Done, `🔴` Overdue
- Priority: `🔴 High`, `🟡 Medium`, `🟢 Low` (or P1/P2/P3 in tables)
- Tags shown as-is, `—` for none

## Single-Item Display

For showing a single item (e.g., `show task`):

```
📌 **Write a post for fb**
ID: 5 · Priority: High · Status: Todo · Tag: personal
Due: 2026-02-15

Description: Write a Facebook post about...
```

## Response Tone

- No trailing "Let me know if there's anything else!" or similar filler
- After showing a table, one short contextual line is fine (e.g., "2 tasks are overdue — want to focus on one?")
- For empty results: "No tasks found matching that filter." (not a paragraph)

## Extensibility

These patterns apply to ANY list-like data: tasks, habits, expenses, goals, etc. Adapt column headers to the domain but keep the table structure and stats summary pattern.
