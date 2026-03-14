---
name: klyntbot-agent
description: >
  Use when a request spans multiple tools, needs AI reasoning to complete, involves
  cross-domain orchestration, or is a natural language request that doesn't map to
  a single tool action. Also use when the user says "ask klyntbot", "have the agent",
  or wants a conversational multi-step workflow.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [agent]
    mcp_tools: ["*"]
    max_iterations: 15
    invokes: [klyntbot-tasks, klyntbot-finance, klyntbot-automation, klyntbot-notes, klyntbot-memory, klyntbot-okr, klyntbot-productivity, klyntbot-work-context]
---

## Quick Reference

| User request | What happens |
|-------------|-------------|
| "Plan my day" | Routes to task-management, runs daily planner |
| "How much did I spend this week?" | Routes to finance, runs spending report |
| "Break down the API migration" | Routes to task-management, runs decompose |
| "Check budget then create tasks for overspending" | Orchestrates across finance + tasks |
| "Summarize what I worked on and plan tomorrow" | Orchestrates across work-context + tasks |

## How to Use

```json
{"query": "natural language request here"}
```

The agent will:
1. Route to the best skill (task-management, finance, automation, etc.)
2. Use multiple tools as needed
3. Return a synthesized response

Use the `klyntbot - agent` MCP tool to delegate complex requests to Klyntbot's AI pipeline.

## When to Use

- Request needs **multiple tools** (e.g., "check my budget then create a task for overspending")
- Request needs **AI reasoning** (e.g., "plan my week based on what's overdue")
- Request is **natural language** that doesn't map to a single tool action
- User wants a **conversational interaction** with Klyntbot

## When NOT to Use

If you know the exact tool and action, call it directly -- it's faster:
- `tasks(action: "create", ...)` instead of `agent(query: "create a task...")`
- `finance(action: "report_spending")` instead of `agent(query: "spending report")`

Use the agent tool for **complex, multi-step, or ambiguous** requests only.

## Common Mistakes

1. **Using agent for simple single-tool actions** — If you know the exact tool and action, call it directly. Agent adds latency and cost for simple operations.
2. **Vague queries** — Be specific in the query. "Do stuff with tasks" is bad. "List overdue tasks in the Work area and suggest a plan for today" is good.
3. **Not checking the response** — The agent returns a synthesized response. Read it and relay the key information to the user rather than just saying "done".
4. **Chaining agent calls** — Don't call the agent multiple times for what should be a single request. Combine the intent into one query.

## Related Skills

- **klyntbot-tasks** — Direct task operations (faster for single actions)
- **klyntbot-finance** — Direct finance operations
- **klyntbot-automation** — Direct scheduling operations
- **klyntbot-notes** — Direct note operations
- **klyntbot-memory** — Direct memory search
- **klyntbot-okr** — Direct OKR operations
- **klyntbot-productivity** — Direct productivity tracking
- **klyntbot-work-context** — Direct work context operations
