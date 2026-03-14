---
name: memory
description: Store and recall important user information and conversation context
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "memory,fact,recall,store"
  always: false
  triggers: ""
  agent: general
---

Use the `memory` tool to manage persistent user knowledge.

## Actions

- `search_conversations` — search past conversations by keyword
- `search_all` — search across all memory sources
- `store` — save an important fact about the user

## When to Use

- User shares personal preferences, habits, or facts about themselves
- User asks "do you remember..." or references past conversations
- You need context from previous interactions to answer accurately

## Tips

- Store facts with clear category labels (e.g., preferences, projects, people)
- Search before storing to avoid duplicates
- Prioritize explicit user statements over inferred information
