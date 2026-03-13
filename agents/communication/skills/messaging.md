---
name: messaging
description: Send messages across channels with proper formatting
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-13"
  source: official
  tags: "message,send,broadcast,dm"
  always: false
  triggers: "send,message,dm,tell,broadcast,announce"
  agent: communication
---

Use the `message` tool to send messages to users across channels.

## Actions

- **Send a single message** — route to the correct channel (Telegram, Discord, Slack, Email)
- **Broadcast** — send the same message to multiple channels
- **Format per channel** — adapt formatting (Markdown for Telegram/Discord, blocks for Slack, HTML for Email)

## Channel formatting rules

| Channel   | Format         | Max length | Notes                          |
|-----------|----------------|------------|--------------------------------|
| Telegram  | MarkdownV2     | 4096 chars | Escape special chars           |
| Discord   | Markdown       | 2000 chars | Use embeds for rich content    |
| Slack     | Block Kit JSON | —          | Use sections + mrkdwn          |
| Email     | HTML           | —          | Include plain-text fallback    |

## When to use

- User says "send", "tell", "message", "dm", "ping", or "broadcast"
- User wants to notify someone or a group
- Cross-channel delivery is needed

## Tips

- Always confirm the target channel and recipient before sending
- For broadcasts, list which channels will receive the message and confirm
- If the target channel is ambiguous, ask the user to clarify
- Respect rate limits — batch multiple messages when possible
