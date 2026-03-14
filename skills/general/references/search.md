---
name: search
description: Web search and information retrieval
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "search,web,find,lookup"
  always: false
  triggers: ""
  agent: general
---

Use `web_search` and `web_fetch` for real-time information retrieval.

## When to Use

- User asks factual questions you're unsure about
- Current events, prices, weather, news
- Verifying claims or looking up documentation
- Any question where your training data might be outdated

## Workflow

1. `web_search` with a concise query
2. Review results for relevance
3. Optionally `web_fetch` a specific URL for detailed content
4. Synthesize and present findings with sources

## Tips

- Keep search queries concise and specific
- Prefer authoritative sources
- Always cite your sources when presenting search results
