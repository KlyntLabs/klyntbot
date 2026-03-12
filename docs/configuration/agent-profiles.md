# Agent Profiles

## Overview

Agent profiles define how each specialized agent behaves within klyntbot. A profile controls tool access, MCP server access, trigger keywords for routing, delegation rules, iteration limits, and personality instructions. Profiles are defined as `AGENT.md` files with YAML frontmatter and are loaded at startup by the `AgentManager`.

Five built-in agents are compiled into the binary via `include_str!` from the `agents/` directory. Workspace-level overrides can be placed in a project's `agents/` folder.

## AGENT.md Format

Each agent profile is a Markdown file with YAML frontmatter:

```markdown
---
name: task
description: Task and project management specialist
tools: [task, area, project, okr, notes, ask_user]
mcp_tools: ["google-calendar"]
triggers: [todo, task, create a task, my tasks]
max_iterations: 12
can_delegate_to: [finance]
always_skills: [todo, daily-planner]
---

You are the task management agent. You help users create, organize,
and track tasks using the OKR+PARA framework.

## Behavior
- When creating tasks, follow the todo skill's workflow
- For daily planning, use the daily-planner skill
```

The frontmatter is parsed by `AgentProfile::parse()` into the `AgentProfile` struct. The Markdown body below the frontmatter becomes the `instructions` field, which is injected into the agent's system prompt.

## All Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `String` | (required) | Unique identifier for the agent |
| `description` | `String` | `""` | Human-readable description of the agent's role |
| `tools` | `Vec<String>` | `[]` | Allowed tool names. Empty means full access to all tools. |
| `mcp_tools` | `Vec<String>` | `[]` | Allowed MCP server names. Empty denies all. `["*"]` allows all. |
| `triggers` | `Vec<String>` | `[]` | Keywords/phrases that route messages to this agent |
| `max_iterations` | `u32` | `10` | Maximum ReAct loop iterations for this agent |
| `can_delegate_to` | `Vec<String>` | `[]` | Names of agents this agent can delegate work to |
| `always_skills` | `Vec<String>` | `[]` | Skill names that are always injected into the system prompt |

### Tool Access Details

When `tools` is empty, the agent has unrestricted access to all registered tools. When `tools` contains specific names, only those tools (plus `ask_user`, which is always included) are available. This is enforced by `allowed_tool_names()`, which returns `None` for full access or `Some(HashSet)` for filtered access.

## Skills

### Folder Structure

Each agent can have a `skills/` subfolder containing Markdown files:

```
agents/
  task/
    AGENT.md
    skills/
      todo.md
      daily-planner.md
      task-decompose.md
      weekly-review.md
      retrospective.md
      project-management.md
```

### Skill Format

Skills use the same YAML frontmatter + Markdown body pattern. Two formats are supported:

**Legacy format** (flat keys):

```markdown
---
name: todo
description: Task creation workflow
always: true
triggers: [create task, add todo]
---

Skill instructions here.
```

**Agent Skills spec format** (nested metadata):

```markdown
---
name: todo
description: Task creation with confidence scoring
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "task,todo,productivity"
  always: true
  triggers: "create task,add todo"
  agent: task
---

Skill instructions here.
```

### Skill Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Skill identifier |
| `description` | `String` | What the skill does |
| `license` | `Option<String>` | License identifier |
| `author` | `Option<String>` | Skill author |
| `version` | `Option<String>` | Skill version |
| `updated_on` | `Option<String>` | Last update date |
| `source` | `Option<String>` | Origin of the skill |
| `tags` | `Vec<String>` | Comma-separated tags (in metadata format) |
| `always` | `bool` | Whether the skill is always loaded into the system prompt |
| `triggers` | `Vec<String>` | Keywords that activate the skill for a given message |
| `agent` | `Option<String>` | Which agent this skill belongs to |

### Skill Activation

Skills are activated in two ways:

1. **Always-loaded skills** -- Skills with `always: true` or listed in the agent's `always_skills` field are injected into the system prompt for every request.

2. **Message-activated skills** -- On-demand skills whose `triggers` match the user's message. If a skill has no triggers, it falls back to matching on the skill name (with hyphen-to-space normalization). Always-loaded skills are excluded from message activation since they are already present.

## Built-in Profiles

| Agent | Tools | MCP Tools | Triggers (sample) | Max Iter | Delegates To | Always Skills |
|-------|-------|-----------|-------------------|----------|-------------|---------------|
| **general** | ask_user, memory, web_search, web_fetch, grep, glob, read_file, list_dir, spawn, learning | `["*"]` (all) | (none -- fallback agent) | 15 | task, finance, automation, communication | (none) |
| **task** | task, area, project, okr, notes, ask_user, memory, grep, glob, read_file, list_dir | google-calendar | todo, task, project, okr, plan my day, weekly review, note, calendar, schedule | 12 | finance | todo, daily-planner |
| **finance** | finance, ask_user, memory, web_search, web_fetch | (none) | finance, money, budget, spending, investment, net worth, crypto | 10 | task | budgeting |
| **automation** | cron, spawn, ask_user, memory | (none) | cron, schedule, reminder, remind me, recurring, automate | 10 | (none) | cron |
| **communication** | message, ask_user, memory | (none) | message, send, notify, tell, dm, broadcast | 10 | (none) | (none) |

### Built-in Skills

| Agent | Skills |
|-------|--------|
| general | memory, search, browser, summarize, skill-creator |
| task | todo, daily-planner, task-decompose, project-management, weekly-review, retrospective |
| finance | budgeting, spending-analysis |
| automation | cron |

## Agent Matching

`AgentManager::match_agent()` selects the best agent for a user message using trigger-based scoring:

1. Normalize the message: lowercase and replace hyphens with spaces.
2. For each agent with triggers, count matches:
   - Each matching trigger contributes its **word count** as a score (longer, more specific triggers rank higher).
   - Track both total score and number of hits.
3. Select the agent with the highest score. Ties are broken by hit count.
4. If no agent's triggers match, fall back to the **general** agent.

### Examples

- `"create a task for reviewing the budget"` -- Matches task agent (trigger `"create a task"` = 3 words).
- `"check my budget spending"` -- Matches finance agent (triggers `"budget"` + `"spending"`).
- `"remind me to buy groceries tomorrow"` -- Matches automation agent (trigger `"remind me"` = 2 words).
- `"hello, how are you?"` -- No triggers match, falls back to general.

### Normalization

Trigger matching normalizes both the message and triggers by lowercasing and replacing hyphens with spaces. This means the trigger `"weekly review"` will match messages containing `"weekly-review"` or `"weekly review"`.

## MCP Access Control

The `mcp_tools` field controls which MCP (Model Context Protocol) servers an agent can use:

| Value | Behavior |
|-------|----------|
| `[]` (empty) | Denies access to all MCP servers |
| `["*"]` | Grants access to all MCP servers |
| `["google-calendar", "linear"]` | Grants access only to the named servers |

This is checked by `AgentProfile::allows_mcp_server(server_name)`. The general agent has `["*"]` (full access), while most specialist agents have empty MCP tools or a targeted list (e.g., task agent has `["google-calendar"]`).

## Delegation

The `can_delegate_to` field lists agent names that this agent can hand off work to. Delegation enables multi-domain workflows:

- **general** can delegate to all four specialists (task, finance, automation, communication).
- **task** can delegate to finance (for budget context on financial tasks).
- **finance** can delegate to task (for creating follow-up tasks from financial analysis).
- **automation** and **communication** cannot delegate.

The general agent acts as an orchestrator: it decomposes multi-part requests, delegates each step to the appropriate specialist, chains context between delegations, and synthesizes a unified response.

### Workspace Overrides

Workspace agents are loaded from a project-level `agents/` directory via `AgentManager::load_workspace_agents()`. They override built-in agents by name, allowing per-project customization of agent behavior, tools, and skills. The same `AGENT.md` + `skills/` structure applies.
