---
name: klyntbot-memory
description: >
  Use when the user asks "do you remember", references past conversations, wants
  to find something discussed before, says "we talked about", "find that thing
  I mentioned", "what did I say about", or needs to search their conversation history.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [memory]
    mcp_tools: []
    invokes: [klyntbot-notes]
---

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `search_conversations` | query, limit | Search past chat history |
| `search_all` | query, limit | Search across everything (todos + conversations) |
| `status` | -- | Check memory system health |

Use the `klyntbot - memory` MCP tool for memory search.

## When to Use

- User says "do you remember...", "we talked about...", "find that thing I mentioned..."
- User references something from a previous conversation
- User wants to find information across their data
- User asks "what did I say about X" or "when did we discuss Y"

## Common Mistakes

1. **Using too narrow a query** — Memory search is semantic. Use descriptive phrases, not single keywords. "budget discussion last week" is better than "budget".
2. **Not trying `search_all` when `search_conversations` returns nothing** — If conversation search comes up empty, try the broader `search_all` which includes tasks and other data.
3. **Presenting raw search results** — Summarize and contextualize the results for the user rather than dumping raw data.
4. **Assuming memory is empty** — Always try the search before telling the user you don't remember something.

## Related Skills

- **klyntbot-notes** — Save important findings from memory searches as notes for easy retrieval
- **klyntbot-tasks** — Search memory to find context for tasks
- **klyntbot-work-context** — Search past work contexts for project history
