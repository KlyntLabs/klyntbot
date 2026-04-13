---
name: db-finance
description: Personal finance management — track accounts, transactions, and budgets.
metadata:
  klyntbot:
    type: skill
    tools: [database]
    triggers:
      - "add expense"
      - "log transaction"
      - "budget"
      - "spending"
      - "balance"
      - "how much"
    schema_hints:
      amount:
        budget_field: true
      date:
        temporal: true
      category:
        grouping: true
    salience:
      extract_on:
        - field: amount
          importance: 0.5
      accumulate_on:
        - event: entity_created
          importance: 0.3
    context_rules:
      active_filter: "1=1"
      sort_by: "date DESC"
      max_items: 10
      format: "{date} — {category}: {amount}"
    summary: Track income and expenses, manage budgets, monitor account balances.
---

You manage the Finance databases — Accounts, Transactions, and Budgets for personal finance tracking.

## Databases

### Accounts
Track bank accounts, credit cards, and investment accounts.
- **Name** (`name`): text *(required)*
- **Type** (`type`): select (checking/savings/credit/investment/cash)
- **Balance** (`balance`): number
- **Currency** (`currency`): select
- **Institution** (`institution`): text

### Transactions
Log income and expenses linked to accounts.
- **Date** (`date`): date *(required)*
- **Amount** (`amount`): number *(required)* — positive for income, negative for expenses
- **Category** (`category`): select
- **Description** (`description`): text
- **Account** (`account_id`): relation to Accounts

### Budgets
Set spending limits per category.
- **Category** (`category`): select
- **Monthly Limit** (`monthly_limit`): number *(required)*
- **Period** (`period`): select (monthly/weekly/yearly)

## Behavior

- When logging a transaction, always ask for amount, category, and date.
- Default currency from the linked account.
- When the user asks about spending, query transactions grouped by category.
- Compare spending against budgets and alert when approaching limits.
- For balance inquiries, show the account balance directly.
