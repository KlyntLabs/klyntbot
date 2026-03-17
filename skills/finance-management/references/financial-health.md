---
name: financial-health
description: Comprehensive financial health report — net worth, spending analysis, budgets, goals, and FIRE progress
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-17"
  source: official
  tags: "finance,report,health,spending,budget,net-worth"
  always: false
  triggers: "financial health,money review,spending report,financial report,how's my money,financial summary"
  agent: finance
---

## When to Use

- User asks for a financial overview or health check
- Automated cron trigger (e.g., "Every month, generate my financial health report")

## Report: Financial Health

### Sections (execute in order)

1. **Net Worth Snapshot**
   - Tool: `finance net_worth`
   - Present: total net worth, breakdown by account type, change vs last period

2. **Spending Analysis**
   - Tool: `finance report_spending` for the period (default: last 30 days)
   - Present: total spend, top categories, comparison to budget

3. **Income Summary**
   - Tool: `finance report_income` for the period
   - Present: total income, sources, trend

4. **Budget Status**
   - Tool: `finance budget_status`
   - Present: each budget with used/remaining, over-budget alerts

5. **Goals Progress**
   - Tool: `finance goal_list`
   - Present: each goal with progress %, projected completion date

6. **FIRE Progress** (if user has FIRE goals configured)
   - Tool: `finance fire_status` if available
   - Present: FIRE number, current progress %, projected date

### Output Format
Structured markdown report with section headers.
Include specific dollar amounts and percentages.
End with 2-3 actionable recommendations based on the data.
Do NOT ask interactive questions — this is a passive report.
