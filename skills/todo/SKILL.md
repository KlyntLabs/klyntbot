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
3. **If low confidence** — create the task first, then use the `ask_user` tool to gather missing fields:
   - Use ask_user to present enrichment options interactively
   - Group related questions (description, priority, due date) into one call
   - Ask user how they want to proceed: Quick hint, Manual fill, Auto-enrich (YOLO), Brainstorm (Party), or Skip
4. **After creation** — always show task summary with confidence
5. **If confidence < 80%** — offer improvement suggestions

### Using ask_user for Enrichment

When a task has low confidence, use the `ask_user` tool to present enrichment options:

```json
{
  "tool": "ask_user",
  "args": {
    "title": "Improve Task Details",
    "questions": [
      {
        "id": "enrichment_mode",
        "title": "How to enrich",
        "text": "This task has low confidence. How would you like to improve it?",
        "answer_type": {
          "type": "single_select",
          "options": [
            {"value": "quick_hint", "label": "Quick hint", "description": "Give me context and I'll infer the rest"},
            {"value": "manual", "label": "I'll fill it", "description": "Ask me for each field"},
            {"value": "yolo", "label": "Auto-enrich", "description": "Infer from our conversation"},
            {"value": "brainstorm", "label": "Let's brainstorm", "description": "Ask me targeted questions"},
            {"value": "skip", "label": "Skip", "description": "Create as-is (low confidence)"}
          ]
        }
      }
    ]
  }
}
```

### Mode Activation

- If user selects "quick_hint" → ask for one-liner context, then update task
- If user selects "yolo" → activate todo-yolo skill behavior
- If user selects "brainstorm" → activate todo-party skill behavior
- If user selects "manual" → use ask_user to collect fields one-by-one
- If user selects "skip" → leave task as-is

### Enrichment Response Format

After creating a low-confidence task, the todo tool will suggest using ask_user.
When you see this suggestion, call ask_user with enrichment options:

```
Task created with low confidence. I'll now show you options to improve it.
```

Then use ask_user (see example above) to present the interactive options.
The user will select their preference via the tabbed UI, and you'll receive
a structured response like: `enrichment_mode: quick_hint`

### Post-Creation Summary

After creating a task, show the confidence score and offer improvement if < 80%.
For formatting (tables, single-item display, status indicators), follow the rules in **RESPONSE.md**.

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
