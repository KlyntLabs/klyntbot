---
name: summarize
description: Summarize or extract text/transcripts from URLs, podcasts, and local files
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  updated-on: "2026-03-13"
  source: official
  tags: "summarize,summary,digest,tldr"
  always: false
  triggers: ""
  agent: general
---

Summarize web pages, articles, and YouTube content using built-in tools.

## When to use

- "what's this link/video about?"
- "summarize this URL/article"
- "give me a TLDR of this page"

## How to summarize

1. Use the `web_fetch` tool to retrieve the page content
2. Summarize the returned text at the requested length
3. If the content is very long, produce a tight summary first, then ask which section to expand

## Length guidelines

- **short**: 2-3 sentences, key takeaway only
- **medium** (default): 1-2 paragraphs covering main points
- **long**: Detailed summary with section headings
- **extract-only**: Return the raw fetched text without summarizing

## Tips

- For YouTube links, `web_fetch` retrieves the page content including available transcript/description
- If the URL returns an error or paywall, inform the user and suggest alternatives
- When summarizing technical content, preserve code snippets and key terminology
