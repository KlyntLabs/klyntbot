---
name: summarize
description: Summarize or extract text/transcripts from URLs, podcasts, and local files
always: false
---

Fast CLI to summarize URLs, local files, and YouTube links.

## When to use

- "what's this link/video about?"
- "summarize this URL/article"
- "transcribe this YouTube/video"

## Quick start

```bash
summarize "https://example.com" --model google/gemini-3-flash-preview
summarize "/path/to/file.pdf" --model google/gemini-3-flash-preview
summarize "https://youtu.be/dQw4w9WgXcQ" --youtube auto
```

## YouTube transcript

```bash
summarize "https://youtu.be/dQw4w9WgXcQ" --youtube auto --extract-only
```

If transcript is huge, return a tight summary first, then ask which section to expand.

## Useful flags

- `--length short|medium|long|xl|xxl|<chars>`
- `--extract-only` (URLs only)
- `--youtube auto` (Apify fallback if `APIFY_API_TOKEN` set)
