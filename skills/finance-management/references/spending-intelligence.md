---
name: spending-intelligence
description: Spending analytics workflows — anomaly detection, trend analysis, correlations, and subscription audits
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-14"
  source: official
  tags: "finance,spending,analytics,anomaly,trends"
  always: false
  triggers: ""
  agent: finance
---

## Spending Intelligence Workflows

Four workflow patterns for different analytical needs.

### Workflow 1: Proactive Health Check

Use when the user asks "anything unusual?" or wants a general spending checkup.

1. `analyze_spending_anomalies(lookback_months: 3, sensitivity: "medium")`
   - Flags categories with spending significantly above historical norms
   - Medium sensitivity balances signal vs noise

2. `analyze_spending_trends(months: 6, group_by: "category")`
   - Shows direction and velocity of spending changes per category
   - Identify accelerating trends before they become problems

Present findings together: "Your dining spending spiked 40% this month (anomaly), and it's been trending up 8% per month over 6 months (trend)."

### Workflow 2: Deep Dive Analysis

Use when the user asks "why is my spending increasing?" or wants to understand spending drivers.

1. `analyze_spending_trends(months: 6, group_by: "month")`
   - Overall spending trajectory month-by-month

2. `analyze_spending_trends(months: 6, group_by: "category")`
   - Which categories are driving the change

3. `analyze_category_correlation(months: 6)`
   - Which categories move together (e.g., dining + entertainment often correlate)
   - Helps identify lifestyle patterns, not just individual expenses

Narrative: connect the dots between correlated categories to tell a spending story.

### Workflow 3: Anomaly Investigation

Use when the user says "why did my spending spike?" or notices something unexpected.

1. `analyze_spending_anomalies(lookback_months: 3, sensitivity: "low")`
   - Low sensitivity catches even mild anomalies for thorough investigation

2. `tx_list(period: "monthly", category: <flagged_category>)`
   - Drill into the specific transactions causing the anomaly

3. `analyze_recurring_charges(lookback_months: 3)`
   - Check if a new subscription or recurring charge appeared

Present: specific transactions that caused the spike, whether they're one-time or recurring, and whether action is needed.

### Workflow 4: Subscription Audit

Use when the user asks about subscriptions, recurring charges, or wants to cut costs.

1. `analyze_recurring_charges(lookback_months: 3)`
   - Identifies all detected recurring/subscription charges
   - Shows frequency, amount, and total annual cost

2. `analyze_spending_anomalies(lookback_months: 3, sensitivity: "high")`
   - High sensitivity may catch new subscriptions that just started

Present as a clear list with annual costs. Highlight:
- Subscriptions the user may have forgotten about
- Price increases on existing subscriptions
- Total annual subscription burden

## Sensitivity Guide

| Level | Use when |
|-------|----------|
| `low` | Investigating a known issue — catch everything |
| `medium` | General checkup — balanced signal-to-noise |
| `high` | Quick scan — only flag major anomalies |

## Cross-References

- For budget impact, combine with `budget_status` to see which anomalies affect budget health
- For net worth impact, use `snapshot_record` after major spending changes
- For trend-based FIRE planning, feed spending trends into `fire_traditional` for more realistic projections
