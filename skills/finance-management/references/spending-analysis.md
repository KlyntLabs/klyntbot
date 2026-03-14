---
name: spending-analysis
description: Analyze spending patterns, trends, and provide financial insights
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "finance,spending,analysis,report"
  always: false
  triggers: ""
  agent: finance
---

## When to use

- User asks about spending habits or trends
- Budget reviews and comparisons
- Financial health assessments

## Actions

- `report_spending` — spending breakdown by category for a period
- `report_trends` — spending trends over time (metric=spending)
- `finance_health_check` — comprehensive financial health assessment
- `net_worth` — current net worth calculation

## Analysis Workflow

1. Fetch spending data for the requested period
2. Compare against budgets if they exist
3. Identify top spending categories
4. Highlight anomalies (unusual spikes or drops)
5. Provide actionable recommendations

## Tips

- Always compare against previous periods for context
- Highlight both positive trends and areas of concern
- Suggest specific, actionable improvements
