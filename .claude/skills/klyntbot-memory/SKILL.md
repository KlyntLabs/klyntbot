---
name: klyntbot-memory
description: >
  Search past conversations and stored facts using Klyntbot's memory system.
  Use when the user asks "do you remember", references past conversations,
  or wants to find something they discussed before.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [memory]
    mcp_tools: []
---

Use the `klyntbot - memory` MCP tool for memory search.

## Actions

| Action | Key params | When to use |
|--------|-----------|-------------|
| `search_conversations` | query, limit | Search past chat history |
| `search_all` | query, limit | Search across everything (todos + conversations) |
| `status` | — | Check memory system health |

## When to Use

- User says "do you remember...", "we talked about...", "find that thing I mentioned..."
- User references something from a previous conversation
- User wants to find information across their data
