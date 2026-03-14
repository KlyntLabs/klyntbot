---
name: klyntbot-agent
description: >
  Send natural language requests to Klyntbot's AI agent for complex multi-step tasks.
  Use when a request spans multiple tools or needs AI reasoning to complete.
  The agent automatically selects the right tools and executes multi-step workflows.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [agent]
    mcp_tools: ["*"]
    max_iterations: 15
---

Use the `klyntbot - agent` MCP tool to delegate complex requests to Klyntbot's AI pipeline.

## When to Use

- Request needs **multiple tools** (e.g., "check my budget then create a task for overspending")
- Request needs **AI reasoning** (e.g., "plan my week based on what's overdue")
- Request is **natural language** that doesn't map to a single tool action
- User wants a **conversational interaction** with Klyntbot

## How to Use

```json
{"query": "natural language request here"}
```

The agent will:
1. Route to the best skill (task-management, finance, automation, etc.)
2. Use multiple tools as needed
3. Return a synthesized response

## Examples

| User request | What happens |
|-------------|-------------|
| "Plan my day" | Agent routes to task-management, runs daily planner |
| "How much did I spend this week?" | Agent routes to finance, runs spending report |
| "Break down the API migration into subtasks" | Agent routes to task-management, runs decompose |
| "Check budget then create tasks for overspending" | Agent orchestrates across finance + tasks |

## When NOT to Use

If you know the exact tool and action, call it directly — it's faster:
- `tasks(action: "create", ...)` instead of `agent(query: "create a task...")`
- `finance(action: "report_spending")` instead of `agent(query: "spending report")`

Use the agent tool for **complex, multi-step, or ambiguous** requests only.
