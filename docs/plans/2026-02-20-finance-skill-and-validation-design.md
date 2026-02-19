# Finance Skill Framework & Data Validation Design

**Date:** 2026-02-20
**Status:** Approved
**Approach:** Skill-First (Approach A)

## Context

The finance module (37 actions, 8 DB tables, 6 repos, ~8,400 lines) is functionally complete. All 1969+ tests pass. 36/37 actions tested in real chat. This design addresses three gaps:

1. **No skill file** — LLMs lack guidance on action routing, prerequisites, and formatting
2. **No data validation** — No health checks, no prerequisite guards, no onboarding detection
3. **Autonomous behaviors not wired** — `FinanceHandler` exists but isn't connected to cron

## 1. Finance Skill File

**File:** `skills/finance/SKILL.md`

**Metadata triggers:** finance, money, budget, spending, investment, savings, net worth, account, transaction, portfolio, goal, FIRE

### 1.1 Onboarding Flow

When user has no finance data (no accounts), guide through setup:

1. Create first account (ask: name, type, currency, initial balance)
2. Confirm default currency in settings
3. Optionally create a budget
4. Optionally create an investment portfolio

Detection: call `account_list` — if empty, trigger onboarding.

### 1.2 Action Routing Guide

Map natural language to actions:

| User says | Action | Key params |
|-----------|--------|-----------|
| "How much did I spend?" | `report_spending` | period=monthly |
| "Add $50 groceries" | `tx_add` | amount, category, tx_type=expense |
| "Check my budget" | `budget_status` | (no ID = show all) |
| "What's my net worth?" | `net_worth` | — |
| "How's my portfolio?" | `investment_summary` | — |
| "Bitcoin price" | `price_fetch` | symbol=BTC, asset_type=crypto |
| "Set up recurring rent" | `tx_recurring_add` | amount, rule, tx_type=expense |
| "FIRE number" | `goal_fire` | annual_expenses (or derive from history) |
| "Health check" | `finance_health_check` | — |

### 1.3 Prerequisite Chain

Before each action category, check dependencies:

- **Transactions** → require >= 1 account
- **Budgets** (status) → require >= 1 budget
- **Investments** → require >= 1 portfolio
- **Reports** (spending/income) → require >= 1 transaction
- **Goals/FIRE** → transaction history improves projections

### 1.4 Response Formatting

- Currency: symbol + commas (e.g., $1,234.56 or 1,234,500 VND)
- Budget status: progress bars `[████████░░] 80%`
- Tables for multi-row data
- Single values inline

### 1.5 LLM Guardrails

- Never guess account/portfolio/budget IDs — use auto-selection or list first
- Always include currency context
- Don't create duplicate budgets for same category+period
- Accept both `type` and `tx_type` for transaction type
- Use `period` default "monthly" for reports when not specified

## 2. Health Check Action

**Action name:** `finance_health_check`

### 2.1 Validation Checks

| # | Check | Description | Severity |
|---|-------|------------|----------|
| 1 | Orphan transactions | Transactions with account_id not in finance_accounts | Error |
| 2 | Balance reconciliation | Account balance != SUM(income) - SUM(expense) for that account | Warning |
| 3 | Stale prices | Investment prices older than configured TTL (default 24h) | Warning |
| 4 | No accounts | Finance module has no accounts (unusable) | Info |
| 5 | Duplicate budgets | Multiple active budgets for same category + period | Warning |
| 6 | Overdue goals | Goals past deadline still marked "active" | Info |
| 7 | Negative remaining | Liabilities with remaining < 0 | Error |
| 8 | Empty portfolios | Portfolios with zero investments | Info |

### 2.2 Output Format

```json
{
  "status": "warnings_found",
  "checks_run": 8,
  "issues": [
    { "check": "stale_prices", "severity": "warning", "count": 3, "detail": "3 investments have prices older than 24 hours" }
  ],
  "summary": "8 checks run: 0 errors, 1 warning, 0 info"
}
```

### 2.3 When It Runs

- Manually: `finance.finance_health_check`
- On startup: logged only (not sent to user)
- Daily review (proactivity=full): included in review output

## 3. Prerequisite Guards

Embedded in each action handler, not a separate layer.

### Pattern

```rust
// At the top of tx_add, before processing:
let accounts = self.accounts.list(false).await?;
if accounts.is_empty() {
    return Ok(json!({
        "error": "no_accounts",
        "message": "No accounts found. Create an account first.",
        "suggested_action": "account_add",
        "example": {"action": "account_add", "name": "Main Bank", "account_type": "bank", "currency": "USD"}
    }).to_string());
}
```

### Actions with guards

| Action | Guard | Suggested action |
|--------|-------|-----------------|
| `tx_add`, `tx_recurring_add` | >= 1 account | `account_add` |
| `investment_add`, `investment_tx` | >= 1 portfolio | `portfolio_create` |
| `budget_status` | >= 1 budget | `budget_create` |
| `report_spending`, `report_income` | >= 1 transaction | `tx_add` |

## 4. Autonomous Behaviors

### 4.1 Scheduled Jobs

| Job | Schedule | Handler method | Description |
|-----|----------|---------------|-------------|
| Daily review | Configurable (default 08:00) | `daily_review()` | Enhanced financial summary |
| Budget alerts | Every 6 hours | `check_budgets()` | Threshold breach notifications |
| Price refresh | Configurable (default every 4h) | `refresh_prices()` | Investment price updates |
| Health check | Daily (midnight) | `run_health_check()` | Data integrity validation |

### 4.2 Proactivity Levels

| Level | Scheduled jobs | In-chat nudges |
|-------|---------------|----------------|
| Full | All active | Yes (budget warnings in tx_add, stale price warnings in investment_summary) |
| Moderate | All active | No |
| Reactive | None | No |

### 4.3 In-Chat Contextual Nudges (proactivity=full)

**After tx_add:** If the relevant budget is now above alert threshold:
> Transaction added: $45.00 Groceries
> Note: Your "Food" budget is now at 87% ($870 of $1,000).

**In investment_summary:** If prices are stale:
> Portfolio "Main": $15,234.50
> Note: 3 holdings have prices > 24h old. Consider running price_refresh.

### 4.4 Enhanced Daily Review

Current: basic budget listing. Enhanced:

1. Yesterday's spending — total + top 3 categories
2. Budget status — any budgets approaching threshold
3. Investment highlights — biggest movers (if prices refreshed recently)
4. Upcoming — recurring transactions due in next 7 days
5. Goals progress — goals approaching deadline or recently achieved

## 5. Test Cleanup

### 5.1 Remove Skeleton

Delete `crates/tools/src/finance_tool/tests.rs` — 1,008 lines of `todo!()` placeholders that never compile.

### 5.2 Add Real Tests

Add `#[cfg(test)] mod tests` in relevant action modules:

| Module | Test focus |
|--------|-----------|
| `accounts.rs` | Account CRUD dispatch |
| `transactions.rs` | Prerequisite guard, auto-account selection, tx_type alias |
| `budgets.rs` | Budget status with no budgets, duplicate detection |
| `investments.rs` | Portfolio prerequisite, stale price warning |
| `goals.rs` | Remaining defaults to principal, FIRE calculation |
| `reports.rs` | Period defaults, date range parsing |
| `mod.rs` | Health check action dispatch, unknown action error |

### 5.3 Coverage

Existing: 1969+ tests (storage repos, price service, all other crates).
New: ~20-30 FinanceTool action-level tests for guards, defaults, and error paths.

## 6. Implementation Order

1. Finance skill file (no code changes, immediate LLM improvement)
2. Prerequisite guards (small changes in each action module)
3. Health check action (new action + handler method)
4. Test cleanup (delete skeleton, add real tests)
5. Autonomous wiring (cron integration, enhanced daily review)
6. In-chat nudges (proactivity-gated contextual warnings)

## 7. Files Affected

| File | Change type |
|------|------------|
| `skills/finance/SKILL.md` | New |
| `crates/tools/src/finance_tool/mod.rs` | Add health_check dispatch |
| `crates/tools/src/finance_tool/accounts.rs` | — (no guard needed, accounts are the root) |
| `crates/tools/src/finance_tool/transactions.rs` | Add prerequisite guard + budget nudge |
| `crates/tools/src/finance_tool/investments.rs` | Add prerequisite guard + stale price nudge |
| `crates/tools/src/finance_tool/budgets.rs` | Add empty-state guard |
| `crates/tools/src/finance_tool/goals.rs` | Add prerequisite info |
| `crates/tools/src/finance_tool/reports.rs` | Add empty-data guard |
| `crates/tools/src/finance_tool/tests.rs` | Delete (replace with inline tests) |
| `crates/tools/src/finance_handler.rs` | Add `run_health_check()` method to trait |
| `crates/agent/src/finance_adapter.rs` | Implement `run_health_check()`, enhance `daily_review()`, wire cron jobs |

## Non-Goals

- Cross-currency transfer implementation (deferred — not blocking any workflow)
- Auto-categorization ML (config exists but implementation deferred)
- 6-jar auto-allocation (config exists but implementation deferred)
- New database migrations (all validation is application-level)
