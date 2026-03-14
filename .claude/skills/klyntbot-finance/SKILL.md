---
name: klyntbot-finance
description: >
  Track expenses, budgets, and financial goals using Klyntbot.
  Use when the user mentions budget, spending, money, accounts, transactions,
  investments, savings, net worth, portfolio, or FIRE.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [finance]
    mcp_tools: []
    max_iterations: 10
---

Use the `klyntbot - finance` MCP tool for financial management.

## Quick Reference

| User says | Action | Key params |
|-----------|--------|-----------|
| "add expense" | `tx_add` | amount (in cents!), category, type: "expense" |
| "check budget" | `budget_status` | budget_id (optional) |
| "spending report" | `report_spending` | period (default: "monthly") |
| "net worth" | `net_worth` | — |
| "list accounts" | `account_list` | — |
| "bitcoin price" | `price_fetch` | symbol: "BTC", asset_type: "crypto" |
| "FIRE number" | `goal_fire` | annual_expenses |

## Critical Rules

- **Amounts in smallest unit**: $50 = 5000 cents, 100k VND = 100000 dong
- **Never guess IDs** — use list actions first
- **Default currency** from `settings_get` — don't hardcode

For all actions and setup workflow, read `references/actions.md`.
