---
name: todo
description: Task management best practices and confidence scoring guidelines.
metadata: {"klyntbot":{"triggers":["todo","task","focus"]}}
---

# Todo Management

Use the `todo` tool to manage user tasks with AI-powered confidence scoring and enrichment.

## Confidence Scoring

Tasks are scored 0.0-1.0 based on:
- Title quality (25%): > 3 words
- Description (25%): > 10 chars
- Priority (15%): set to 1-5
- Due date (20%): concrete deadline
- Tags (15%): at least one tag

## When to Suggest Tasks

Automatically create tasks when the user mentions:
- "I need to..."
- "Remind me to..."
- "Don't forget..."
- Action items from meetings or conversations

Check existing tasks before creating to avoid duplicates.

## Chat Enrichment Flow

When a user asks to create a task in chat:

1. **Assess completeness** — would this task have confidence >= 50%?
2. **If high confidence** — create immediately with `todo.add`, show summary
3. **If low confidence** — present enrichment options before creating:
   - Quick hint, Manual fill, Auto-enrich (YOLO), Brainstorm (Party), or Skip
   - Accept natural language responses, not just numbers
4. **After creation** — always show task summary with confidence
5. **If confidence < 80%** — offer improvement suggestions

### Mode Activation

- If user provides extra context inline → treat as Quick Hint
- If user says "yolo" or "auto" → activate todo-yolo skill behavior
- If user says "brainstorm" or "help me think about it" → activate todo-party skill behavior
- If user provides structured fields → collect as Manual mode
- If user says "skip" or "just create it" → create as-is

### Enrichment Response Format

When presenting enrichment options for a low-confidence task:

```
I'll create that task, but it's pretty thin right now.

📋 "[title]" — Confidence: [X]%
   Missing: [list of missing fields]

How would you like to flesh it out?

1. **Quick hint** — give me a one-liner and I'll fill in the rest
2. **I'll fill it** — tell me the details (description, priority, due date, tags)
3. **Auto-enrich** — I'll infer everything from our conversation
4. **Let's brainstorm** — I'll ask you a few quick questions
5. **Skip** — create as-is (low confidence)

Just reply with a number or describe what you'd like.
```

### Post-Creation Summary

Always show after creating a task:

```
✅ Task Created

**[title]**
ID: [id]
Priority: P[n] ([label]) · Due: [date] · Tags: [tags]
Description: [description if present]

Confidence: [X]% [█████░░░░░]

Want to improve it, or focus on it now?
```

**If confidence < 80%**, proactively suggest improvements:

```
✅ Task Created

**[title]**
ID: [id]
Confidence: [X]% [██░░░░░░░░]

This task is pretty sparse. Missing: [list of missing fields].
Want me to help flesh it out? (I can auto-enrich or ask you questions)
```

### Confidence Visualization

Use Unicode bars for confidence display (no ANSI colors in chat):

- `Confidence: 90% ████████░░` (high — all good!)
- `Confidence: 65% ██████░░░░` (medium — could be better)
- `Confidence: 25% ██░░░░░░░░` (low — needs more detail)

For `todo show` responses with field breakdown:

```
Confidence: 65%
  ✅ Title quality (25%)
  ✅ Description (25%)
  ✅ Priority (15%)
  ⬜ Due date (0%) — not set
  ⬜ Tags (0%) — none

💡 Add a due date (+20%) and tags (+15%) to improve confidence.
```

### Important Rules

- Never create a task silently without showing the summary
- Always mention confidence score in the summary
- One question per message in Party/Manual modes
- Accept multi-field answers (don't ask what's already provided)
- Use `todo.update` for post-creation improvements (not delete + recreate)
- If user changes topic mid-enrichment, handle it, then optionally ask about the pending task

## Focus Mode

- Max 3 tasks focused simultaneously
- 18-hour deadline per focused task
- Auto-unfocus when expired
- Track focus_expired_count to detect postponing patterns
