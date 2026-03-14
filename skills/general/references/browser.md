---
name: browser
description: Browser automation for navigating web pages and performing real-world tasks
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "browser,web,navigate,scrape"
  always: false
  triggers: ""
  agent: general
---

Use the `browser` tool to navigate web pages and perform real-world tasks like booking tickets, shopping, and managing accounts.

## Core workflow

Always follow this sequence:
1. `navigate` to the target URL
2. `snapshot` to see all interactive elements as `@e1`, `@e2`, etc.
3. Interact using the `@e` references from the snapshot
4. `snapshot` again after navigation to refresh element references

## Action reference

| Action | When to use |
|---|---|
| `navigate` | Load a new URL |
| `snapshot` | Get current page elements (always do this before clicking) |
| `fill_form` | Fill multiple fields at once using label names |
| `login_flow` | Authenticate on a login page |
| `click` | Click a button or link by `@e` ref or label |
| `fill` | Fill a single input field |
| `type` | Type text character by character |
| `press` | Send a keyboard key (Enter, Tab, Escape) |
| `wait` | Wait for a page element or URL change |
| `get_text` | Extract text from an element |
| `screenshot` | Capture the current page state |
| `submit_and_confirm` | Click a submit/checkout button (always requires confirmation) |

## Write action confirmation

When you receive a `[CONFIRMATION_REQUIRED]` response from the browser tool:
1. Use `ask_user` to show the user what action is about to happen
2. Wait for their confirmation
3. If confirmed, call the same browser action again

## Tips
- `@e` references expire after navigation — always snapshot again after a page change
- Use `fill_form` instead of individual `fill` calls for multi-field forms
- Use `screenshot` to verify state after complex interactions
