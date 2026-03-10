---
name: task-decompose
description: Break complex tasks or goals into structured subtask plans
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "task,decompose,break-down,subtask"
  always: false
  triggers: "break down,break this down,decompose,subtasks,plan out,how should i approach,workback"
  agent: task
---

## When to Use

- User says "break this down", "decompose", "plan out [goal]", "how should I approach [task]"
- User provides a complex goal with a deadline ("ship X by March 15")
- A task's title implies multiple steps ("set up the new deployment pipeline")

## Decomposition Strategies

Choose the strategy based on what the user provides:

### 1. Flat Decomposition (default)

**When**: Simple goal, no deadline, no implied ordering.

Input: "Plan a birthday party"
Output: 5–7 independent subtasks at the same level.

### 2. Sequential Decomposition

**When**: Goal has an implied pipeline or natural ordering.

Input: "Launch the new feature"
Output: Ordered chain — each step depends on the previous:
```
1. Research requirements and constraints
2. Design the solution
3. Implement core functionality
4. Write tests
5. Deploy to staging
6. Deploy to production
```

### 3. Hierarchical Decomposition

**When**: Complex multi-domain goal with distinct workstreams.

Input: "Renovate the kitchen"
Output: Tree structure with grouped subtasks:
```
Planning:
  - Set budget
  - Create timeline
  - Get permits

Demolition:
  - Clear cabinets
  - Remove counters

Installation:
  - Install cabinets
  - Install counters
  - Install backsplash
```

### 4. Workback Decomposition (deadline-first)

**When**: User provides a hard deadline. Most powerful strategy.

Input: "Ship feature X by March 15"
Output: Reverse-engineered timeline with calculated due dates:
```
Mar 14: Final QA and deploy       (1 day)
Mar 12: Integration testing        (2 days)
Mar 8:  Implementation complete    (4 days)
Mar 5:  Design approved            (3 days)
Mar 3:  Research and scoping       (2 days)
```

## Workflow

1. **Analyze the goal**: Identify the strategy (flat/sequential/hierarchical/workback)
2. **Gather context**: If the goal is vague, use `ask_user` to clarify scope and constraints
3. **Generate the plan**: Present the decomposed subtasks to the user
4. **Confirm**: Ask the user to approve, adjust, or regenerate
5. **Create tasks**: After approval, create each subtask via `todo add` with:
   - The parent task as context in the description
   - Appropriate due dates (especially for workback)
   - Sequential numbering in titles for ordered plans (e.g., "1/6: Research requirements")
   - All tasks in the same area and project as the parent goal

## Guidelines

- **Granularity**: Each subtask should be completable in 15min–2h. If a subtask would take longer, decompose it further.
- **Actionable titles**: Every subtask must start with a verb ("Research...", "Write...", "Send...", "Build...")
- **Duration estimates**: Include time estimates for each subtask when doing workback
- **Dependencies**: Note which tasks block others in the description (the system doesn't track dependencies formally, so make it explicit in text)
- **Don't over-decompose**: 5–10 subtasks is the sweet spot. More than 12 is usually too granular.

## Example Interaction

```
User: "I need to prepare for the product launch on April 1st"

Agent: I'll use workback decomposition since you have a deadline. Here's the plan:

Mar 28–31: Launch prep (4 days)
  - Mar 31: Go-live checklist and deploy
  - Mar 30: Final stakeholder review
  - Mar 28: Prepare launch comms and docs

Mar 21–27: Testing phase (7 days)
  - Mar 27: Bug fixes from beta feedback
  - Mar 24: Beta release to internal team
  - Mar 21: End-to-end testing

Mar 14–20: Build phase (7 days)
  - Mar 20: Feature freeze
  - Mar 17: Core features complete
  - Mar 14: Start implementation

Mar 10–13: Planning (4 days)
  - Mar 13: Design review and approval
  - Mar 10: Requirements finalization

That's 12 tasks across 22 days. Create these?
```
