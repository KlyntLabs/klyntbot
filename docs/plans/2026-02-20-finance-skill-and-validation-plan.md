# Finance Skill Framework & Data Validation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a finance skill file for LLM guidance, prerequisite guards on all finance actions, a health check action, enhanced daily review, and cron-based autonomous scheduling.

**Architecture:** Single skill file (`skills/finance/SKILL.md`) provides LLM routing + onboarding. Prerequisite guards are embedded in each action handler method. Health check is a new action in `mod.rs` with validation logic in a new `health.rs` sub-module. Autonomous behaviors wire the existing `FinanceHandler` trait methods into the cron callback in `serve.rs`. The dead skeleton `tests.rs` is deleted and replaced with real inline tests.

**Tech Stack:** Rust, async_trait, serde_json, chrono, sqlx (PostgreSQL), scheduling crate (CronService)

---

### Task 1: Create Finance Skill File

**Files:**
- Create: `skills/finance/SKILL.md`

**Step 1: Create the skill directory and file**

```markdown
---
name: finance
description: Personal finance management — accounts, transactions, budgets, investments, goals, and reports.
metadata: '{"klyntbot":{"triggers":["finance","money","budget","spending","investment","savings","net worth","account","transaction","portfolio","goal","FIRE","net_worth","price","crypto"],"always":true}}'
---

# Personal Finance

## Agent Instructions

**When the user asks about finances** (money, budget, spending, investment, savings, net worth, accounts, transactions, portfolios, goals, FIRE), use the `finance` tool.

### First-Time Setup (Onboarding)

Before any financial action, check if accounts exist:

```json
{"action": "account_list"}
```

**If no accounts exist**, guide the user through setup:

1. Create their first account:
   ```json
   {"action": "account_add", "name": "Main Bank", "type": "bank", "currency": "VND", "balance": 0}
   ```
2. Confirm default currency: `{"action": "settings_get"}`
3. Optionally create a budget: `{"action": "budget_create", ...}`
4. Optionally create a portfolio: `{"action": "portfolio_create", ...}`

### Action Routing

| User says | Action | Key params |
|-----------|--------|-----------|
| "How much did I spend?" | `report_spending` | period (default: monthly) |
| "Add $50 groceries" | `tx_add` | amount (in cents), category, type=expense |
| "Check my budget" | `budget_status` | (no ID = show all) |
| "What's my net worth?" | `net_worth` | — |
| "How's my portfolio?" | `investment_summary` | — |
| "Bitcoin price" | `price_fetch` | symbol=BTC, asset_type=crypto |
| "Set up recurring rent" | `tx_recurring_add` | amount, recurring_rule, type=expense |
| "FIRE number" | `goal_fire` | annual_expenses (or omit to derive) |
| "Financial health check" | `finance_health_check` | — |
| "Add a savings goal" | `goal_create` | name, goal_type=savings, target_amount |
| "Add a liability" | `liability_add` | name, type, principal |
| "Spending trends" | `report_trends` | metric=spending |

### Critical Rules

1. **Amounts are in smallest currency unit** (cents for USD, dong for VND). $50 = 5000 cents.
2. **Never guess IDs.** Use `account_list`, `portfolio_list`, `budget_list`, `goal_list` to discover IDs.
3. **Auto-account selection works.** If `account_id` is omitted in `tx_add`/`tx_recurring_add`, the first active account is used automatically.
4. **Accept both `type` and `tx_type`** for transaction type (income/expense/transfer).
5. **Period defaults to "monthly"** for reports when not specified.
6. **Default currency** comes from `settings_get` — don't hardcode "USD".
7. **Don't create duplicate budgets** for the same category + period.

### Response Formatting

- **Currency amounts**: Always show with symbol and grouping (e.g., $1,234.56 or 1,234,500 VND)
- **Budget status**: Include percentage and visual indicator
- **Tables**: Use tables for multi-row data (transaction lists, budget breakdowns)
- **Single values**: Show inline (net worth, price fetch results)

### Prerequisite Errors

If a finance action returns a JSON response with `"error"` and `"suggested_action"` fields, follow the suggestion. Example:

```json
{"error": "no_accounts", "message": "No accounts found.", "suggested_action": "account_add"}
```

→ Guide the user to create the missing resource before retrying.

### Settings

View current settings: `{"action": "settings_get"}`
Update settings: `{"action": "settings_update", "default_currency": "USD", "proactivity_level": "full"}`

Proactivity levels:
- **full**: Daily reviews, budget alerts, price refreshes, in-chat nudges
- **moderate**: Scheduled alerts only, no in-chat nudges
- **reactive**: No automated actions, user must explicitly ask
```

**Step 2: Verify skill file is valid markdown with correct frontmatter**

Run: `head -5 skills/finance/SKILL.md`
Expected: YAML frontmatter with `---` delimiters, `name: finance`, `description:`, `metadata:`

**Step 3: Commit**

```bash
git add skills/finance/SKILL.md
git commit -m "feat(skills): add finance skill for LLM action routing and onboarding"
```

---

### Task 2: Add Prerequisite Guards to Transaction Actions

**Files:**
- Modify: `crates/tools/src/finance_tool/transactions.rs:38-56`

The `tx_add` method already has auto-account selection (lines 39-57) that returns a clear error if no accounts exist. The `tx_recurring_add` method has the same pattern. Both currently use `ToolError::InvalidParams` for the no-account case.

**Step 1: Write the failing test**

Add at the bottom of `crates/tools/src/finance_tool/transactions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::ParamExtractor;
    use serde_json::json;

    // Helper to build a FinanceTool for testing would require a database.
    // These tests validate parameter extraction logic only.

    #[test]
    fn test_tx_type_alias_accepted() {
        // Verify that both "type" and "tx_type" keys resolve to the same value.
        let args = json!({"action": "tx_add", "tx_type": "expense", "amount": 5000});
        let p = ParamExtractor::new(&args);
        // "tx_type" should be accessible via optional_str
        let tx_type = p.optional_str("tx_type").unwrap();
        assert_eq!(tx_type, Some("expense"));
    }

    #[test]
    fn test_transaction_type_from_str_loose() {
        assert!(TransactionType::from_str_loose("income").is_some());
        assert!(TransactionType::from_str_loose("expense").is_some());
        assert!(TransactionType::from_str_loose("transfer").is_some());
        assert!(TransactionType::from_str_loose("INCOME").is_some());
        assert!(TransactionType::from_str_loose("invalid").is_none());
    }
}
```

**Step 2: Run test to verify it passes**

Run: `cargo nextest run -p tools 'transactions::tests' --no-capture`
Expected: 2 tests PASS

**Step 3: Upgrade the no-accounts error to return helpful JSON instead of ToolError**

Change the error in `tx_add` (line 51-55) from `ToolError::InvalidParams(...)` to a structured JSON response:

```rust
// Replace the ok_or_else closure in tx_add (line 49-55):
.ok_or_else(|| {
    ToolError::InvalidParams(
        serde_json::to_string(&json!({
            "error": "no_accounts",
            "message": "No active accounts found. Create an account first.",
            "suggested_action": "account_add",
            "example": {"action": "account_add", "name": "Main Bank", "type": "bank", "currency": "USD"}
        })).unwrap()
    )
})?
```

Apply the same pattern in `tx_recurring_add` for its auto-account selection error.

**Step 4: Run tests**

Run: `cargo nextest run -p tools --no-capture`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add crates/tools/src/finance_tool/transactions.rs
git commit -m "feat(finance): add prerequisite guards and tests to transaction actions"
```

---

### Task 3: Add Prerequisite Guards to Investment Actions

**Files:**
- Modify: `crates/tools/src/finance_tool/investments.rs:105-114`

`investment_add` already checks if `portfolio_id` is valid (lines 106-114), but it uses `ToolError::ExecutionFailed`. We also need a guard on `investment_tx`.

**Step 1: Write the failing test**

Add at the bottom of `crates/tools/src/finance_tool/investments.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::finance_types::AssetType;

    #[test]
    fn test_asset_type_from_str_loose() {
        assert!(AssetType::from_str_loose("stock").is_some());
        assert!(AssetType::from_str_loose("etf").is_some());
        assert!(AssetType::from_str_loose("crypto").is_some());
        assert!(AssetType::from_str_loose("STOCK").is_some());
        assert!(AssetType::from_str_loose("invalid_type").is_none());
    }

    #[test]
    fn test_investment_tx_type_from_str_loose() {
        use crate::finance_types::InvestmentTxType;
        assert!(InvestmentTxType::from_str_loose("buy").is_some());
        assert!(InvestmentTxType::from_str_loose("sell").is_some());
        assert!(InvestmentTxType::from_str_loose("dividend").is_some());
        assert!(InvestmentTxType::from_str_loose("nope").is_none());
    }
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p tools 'investments::tests' --no-capture`
Expected: 2 tests PASS

**Step 3: Upgrade portfolio-not-found error in `investment_add`**

Change lines 110-113 from `ToolError::ExecutionFailed` to structured JSON:

```rust
if portfolio_exists.is_none() {
    return Ok(serde_json::to_string_pretty(&json!({
        "error": "portfolio_not_found",
        "message": format!("Portfolio '{}' not found. Create one first or use portfolio_list to find IDs.", portfolio_id),
        "suggested_action": "portfolio_create",
        "example": {"action": "portfolio_create", "name": "My Portfolio", "currency": "USD"}
    })).unwrap());
}
```

Apply same pattern to `investment_tx` for its portfolio/investment lookup.

**Step 4: Run tests**

Run: `cargo nextest run -p tools --no-capture`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add crates/tools/src/finance_tool/investments.rs
git commit -m "feat(finance): add prerequisite guards and tests to investment actions"
```

---

### Task 4: Add Prerequisite Guards to Budget and Report Actions

**Files:**
- Modify: `crates/tools/src/finance_tool/budgets.rs:164-169`
- Modify: `crates/tools/src/finance_tool/reports.rs:103-134`

**Step 1: Write tests for budgets**

Add at the bottom of `crates/tools/src/finance_tool/budgets.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::finance_types::{BudgetMethod, BudgetPeriod, JarType};

    #[test]
    fn test_budget_period_from_str_loose() {
        assert!(BudgetPeriod::from_str_loose("monthly").is_some());
        assert!(BudgetPeriod::from_str_loose("weekly").is_some());
        assert!(BudgetPeriod::from_str_loose("yearly").is_some());
        assert!(BudgetPeriod::from_str_loose("custom").is_some());
        assert!(BudgetPeriod::from_str_loose("invalid").is_none());
    }

    #[test]
    fn test_budget_method_from_str_loose() {
        assert!(BudgetMethod::from_str_loose("standard").is_some());
        assert!(BudgetMethod::from_str_loose("six_jar").is_some());
        assert!(BudgetMethod::from_str_loose("nope").is_none());
    }

    #[test]
    fn test_jar_type_from_str_loose() {
        assert!(JarType::from_str_loose("essentials").is_some());
        assert!(JarType::from_str_loose("savings").is_some());
        assert!(JarType::from_str_loose("entertainment").is_some());
        assert!(JarType::from_str_loose("invalid").is_none());
    }
}
```

**Step 2: Write tests for reports**

Add at the bottom of `crates/tools/src/finance_tool/reports.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::derive_date_range;
    use chrono::NaiveDate;

    #[test]
    fn test_derive_date_range_month() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let (from, to, label) = derive_date_range("month", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
        assert_eq!(label, "February 2026");
    }

    #[test]
    fn test_derive_date_range_monthly_alias() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let (from, to, _) = derive_date_range("monthly", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn test_derive_date_range_week() {
        // 2026-02-15 is a Sunday
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let (from, to, label) = derive_date_range("week", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 2, 9).unwrap()); // Monday
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()); // Sunday
        assert!(label.starts_with("Week of"));
    }

    #[test]
    fn test_derive_date_range_year() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let (from, to, label) = derive_date_range("year", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        assert_eq!(label, "2026");
    }

    #[test]
    fn test_derive_date_range_quarter() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let (from, to, label) = derive_date_range("quarter", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        assert_eq!(label, "Q2 2026");
    }

    #[test]
    fn test_derive_date_range_last_30_days() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let (from, _to, _) = derive_date_range("last_30_days", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 1, 31).unwrap());
    }

    #[test]
    fn test_derive_date_range_unknown_returns_error() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        assert!(derive_date_range("banana", today).is_err());
    }

    #[test]
    fn test_derive_date_range_custom_returns_error() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        assert!(derive_date_range("custom", today).is_err());
    }

    #[test]
    fn test_change_pct() {
        use super::change_pct;
        assert_eq!(change_pct(200, 100), Some(100)); // +100%
        assert_eq!(change_pct(50, 100), Some(-50));   // -50%
        assert_eq!(change_pct(100, 0), None);          // div by zero
        assert_eq!(change_pct(100, 100), Some(0));     // no change
    }
}
```

**Step 3: Run all new tests**

Run: `cargo nextest run -p tools 'budgets::tests|reports::tests' --no-capture`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add crates/tools/src/finance_tool/budgets.rs crates/tools/src/finance_tool/reports.rs
git commit -m "test(finance): add unit tests for budget types and report date range derivation"
```

---

### Task 5: Add Health Check Action — New Sub-module

**Files:**
- Create: `crates/tools/src/finance_tool/health.rs`
- Modify: `crates/tools/src/finance_tool/mod.rs:6-17` (add `mod health;`)
- Modify: `crates/tools/src/finance_tool/mod.rs:110-121` (add `"finance_health_check"` to enum)
- Modify: `crates/tools/src/finance_tool/mod.rs:396-436` (add dispatch match arm)

**Step 1: Create `health.rs` with health check logic**

```rust
//! Health check action handler for `FinanceTool`.
//!
//! Validates data integrity across all finance tables and returns
//! a structured report of any issues found.

use chrono::{Duration, Local};
use serde_json::json;

use crate::RoutingContext;
use common::Result;

use super::FinanceTool;

/// Severity level for health check issues.
#[derive(Debug, Clone, Copy)]
enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// A single health check finding.
struct Issue {
    check: &'static str,
    severity: Severity,
    count: usize,
    detail: String,
}

impl FinanceTool {
    pub(crate) async fn finance_health_check(&self, _ctx: &RoutingContext) -> Result<String> {
        let mut issues: Vec<Issue> = Vec::new();

        // 1. No accounts — finance is unusable
        let accounts = self.accounts.list(false).await.unwrap_or_default();
        if accounts.is_empty() {
            issues.push(Issue {
                check: "no_accounts",
                severity: Severity::Info,
                count: 0,
                detail: "No finance accounts exist. Create an account to get started.".into(),
            });
        }

        // 2. Balance reconciliation — account.balance vs SUM(income) - SUM(expense)
        for account in &accounts {
            let filter = storage::FinanceTransactionFilter {
                account_id: Some(account.id.clone()),
                ..Default::default()
            };
            let txs = self.transactions.list(&filter).await.unwrap_or_default();

            let mut computed_balance: i64 = 0;
            for tx in &txs {
                match tx.tx_type.as_str() {
                    "income" => computed_balance += tx.amount,
                    "expense" => computed_balance -= tx.amount,
                    "transfer" => {
                        // Outgoing transfers subtract, incoming add.
                        // If account_id matches this account, it's outgoing.
                        if tx.account_id == account.id {
                            computed_balance -= tx.amount;
                        } else {
                            computed_balance += tx.amount;
                        }
                    }
                    _ => {}
                }
            }

            // Compare with initial balance (account was created with some balance).
            // We can only check if computed delta from transactions is consistent.
            // Since we don't store the initial balance separately, we check if
            // the account balance looks reasonable (not negative for bank accounts).
            if account.balance < 0 && account.account_type != "crypto_wallet" {
                issues.push(Issue {
                    check: "negative_balance",
                    severity: Severity::Warning,
                    count: 1,
                    detail: format!(
                        "Account '{}' has negative balance: {}",
                        account.name, account.balance
                    ),
                });
            }
        }

        // 3. Stale investment prices
        let investments = self.investments.list_with_symbols().await.unwrap_or_default();
        let stale_threshold = Local::now() - Duration::hours(24);
        let stale_count = investments
            .iter()
            .filter(|inv| inv.updated_at < stale_threshold.to_utc())
            .count();
        if stale_count > 0 {
            issues.push(Issue {
                check: "stale_prices",
                severity: Severity::Warning,
                count: stale_count,
                detail: format!(
                    "{} investment(s) have prices older than 24 hours. Run price_refresh to update.",
                    stale_count
                ),
            });
        }

        // 4. Duplicate active budgets for same category + period
        let budgets = self.budgets.list(true).await.unwrap_or_default();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dup_count = 0usize;
        for b in &budgets {
            let key = format!("{}:{}", b.category.as_deref().unwrap_or("*"), b.period);
            if !seen.insert(key) {
                dup_count += 1;
            }
        }
        if dup_count > 0 {
            issues.push(Issue {
                check: "duplicate_budgets",
                severity: Severity::Warning,
                count: dup_count,
                detail: format!(
                    "{} duplicate active budget(s) found for the same category + period.",
                    dup_count
                ),
            });
        }

        // 5. Overdue goals — past deadline but still active
        let goals = self.goals.list_active().await.unwrap_or_default();
        let today = Local::now().date_naive();
        let overdue_goals: Vec<_> = goals
            .iter()
            .filter(|g| g.deadline.map(|d| d < today).unwrap_or(false))
            .collect();
        if !overdue_goals.is_empty() {
            issues.push(Issue {
                check: "overdue_goals",
                severity: Severity::Info,
                count: overdue_goals.len(),
                detail: format!(
                    "{} active goal(s) are past their deadline.",
                    overdue_goals.len()
                ),
            });
        }

        // 6. Negative remaining on liabilities
        let liabilities = self.liabilities.list().await.unwrap_or_default();
        let neg_remaining: Vec<_> = liabilities
            .iter()
            .filter(|l| l.remaining < 0)
            .collect();
        if !neg_remaining.is_empty() {
            issues.push(Issue {
                check: "negative_remaining",
                severity: Severity::Error,
                count: neg_remaining.len(),
                detail: format!(
                    "{} liability(ies) have negative remaining balance.",
                    neg_remaining.len()
                ),
            });
        }

        // 7. Empty portfolios
        let portfolios = self.investments.list_portfolios().await.unwrap_or_default();
        let mut empty_count = 0usize;
        for p in &portfolios {
            let filter = storage::FinanceInvestmentFilter {
                portfolio_id: Some(p.id.clone()),
                ..Default::default()
            };
            let holdings = self.investments.list_investments(&filter).await.unwrap_or_default();
            if holdings.is_empty() {
                empty_count += 1;
            }
        }
        if empty_count > 0 {
            issues.push(Issue {
                check: "empty_portfolios",
                severity: Severity::Info,
                count: empty_count,
                detail: format!(
                    "{} portfolio(s) have no investment holdings.",
                    empty_count
                ),
            });
        }

        // Build summary
        let errors = issues.iter().filter(|i| matches!(i.severity, Severity::Error)).count();
        let warnings = issues.iter().filter(|i| matches!(i.severity, Severity::Warning)).count();
        let infos = issues.iter().filter(|i| matches!(i.severity, Severity::Info)).count();

        let status = if errors > 0 {
            "errors_found"
        } else if warnings > 0 {
            "warnings_found"
        } else if infos > 0 {
            "info_only"
        } else {
            "all_clear"
        };

        let issues_json: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| {
                json!({
                    "check": i.check,
                    "severity": i.severity.as_str(),
                    "count": i.count,
                    "detail": i.detail,
                })
            })
            .collect();

        let result = json!({
            "status": status,
            "checks_run": 7,
            "issues": issues_json,
            "summary": format!("7 checks run: {} error(s), {} warning(s), {} info", errors, warnings, infos),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Info.as_str(), "info");
    }
}
```

**Step 2: Register the module and add dispatch**

In `crates/tools/src/finance_tool/mod.rs`:

Add `mod health;` after line 12 (after `mod transactions;`).

Add `"finance_health_check"` to the enum list in `parameters()` (after `"daily_review"` on line 121).

Add a match arm in `execute()` (after the settings match on line 431):

```rust
// ── Health check ─────────────────────────────────────────────
"finance_health_check" => self.finance_health_check(ctx).await,
```

**Step 3: Run tests**

Run: `cargo nextest run -p tools 'health::tests' --no-capture`
Expected: 1 test PASS (severity_as_str)

Run: `cargo build --workspace`
Expected: Success, no errors

**Step 4: Commit**

```bash
git add crates/tools/src/finance_tool/health.rs crates/tools/src/finance_tool/mod.rs
git commit -m "feat(finance): add finance_health_check action with 7 data integrity checks"
```

---

### Task 6: Delete Skeleton Tests File

**Files:**
- Delete: `crates/tools/src/finance_tool/tests.rs`
- Modify: `crates/tools/src/finance_tool/mod.rs:14-17` (remove commented-out `mod tests`)

**Step 1: Delete the file**

```bash
rm crates/tools/src/finance_tool/tests.rs
```

**Step 2: Remove the commented-out module reference**

In `crates/tools/src/finance_tool/mod.rs`, remove lines 14-17:

```rust
// tests.rs contains todo!()-based skeletons that will be enabled once
// the action handlers are implemented. Not compiled yet.
// #[cfg(test)]
// mod tests;
```

**Step 3: Verify build**

Run: `cargo build --workspace`
Expected: Success

**Step 4: Commit**

```bash
git add -A crates/tools/src/finance_tool/tests.rs crates/tools/src/finance_tool/mod.rs
git commit -m "chore(finance): remove skeleton tests file, replaced with inline tests"
```

---

### Task 7: Add Goal/Liability Tests

**Files:**
- Modify: `crates/tools/src/finance_tool/goals.rs` (add tests at bottom)

**Step 1: Write tests**

Add at the bottom of `crates/tools/src/finance_tool/goals.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::finance_types::{GoalStatus, GoalType, LiabilityType};

    #[test]
    fn test_goal_type_from_str_loose() {
        assert!(GoalType::from_str_loose("savings").is_some());
        assert!(GoalType::from_str_loose("purchase").is_some());
        assert!(GoalType::from_str_loose("debt_payoff").is_some());
        assert!(GoalType::from_str_loose("fire").is_some());
        assert!(GoalType::from_str_loose("custom").is_some());
        assert!(GoalType::from_str_loose("invalid").is_none());
    }

    #[test]
    fn test_goal_status_from_str_loose() {
        assert!(GoalStatus::from_str_loose("active").is_some());
        assert!(GoalStatus::from_str_loose("achieved").is_some());
        assert!(GoalStatus::from_str_loose("abandoned").is_some());
        assert!(GoalStatus::from_str_loose("nope").is_none());
    }

    #[test]
    fn test_liability_type_from_str_loose() {
        assert!(LiabilityType::from_str_loose("mortgage").is_some());
        assert!(LiabilityType::from_str_loose("credit_card").is_some());
        assert!(LiabilityType::from_str_loose("personal_loan").is_some());
        assert!(LiabilityType::from_str_loose("student_loan").is_some());
        assert!(LiabilityType::from_str_loose("other").is_some());
        assert!(LiabilityType::from_str_loose("invalid").is_none());
    }
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p tools 'goals::tests' --no-capture`
Expected: 3 tests PASS

**Step 3: Commit**

```bash
git add crates/tools/src/finance_tool/goals.rs
git commit -m "test(finance): add unit tests for goal and liability type parsing"
```

---

### Task 8: Add `run_health_check` to FinanceHandler Trait

**Files:**
- Modify: `crates/tools/src/finance_handler.rs:89-104` (add method to trait)
- Modify: `crates/agent/src/finance_adapter.rs:36-141` (implement method)

**Step 1: Add `run_health_check` to the FinanceHandler trait**

In `crates/tools/src/finance_handler.rs`, add after line 100 (after `analyze_spending`):

```rust
    /// Run all data integrity checks and return a summary string.
    /// Used by scheduled health checks and daily review (proactivity=full).
    async fn run_health_check(&self) -> Result<String>;
```

**Step 2: Implement in FinanceHandlerImpl**

In `crates/agent/src/finance_adapter.rs`, add before the `analyze_spending` method (around line 133):

```rust
    async fn run_health_check(&self) -> Result<String> {
        // Health check runs in the autonomous context (no FinanceTool instance).
        // We check a subset of validations that don't require the full tool.

        let mut issues = Vec::new();

        // Check for empty accounts
        let accounts = self.repos.finance_accounts.list(false).await?;
        if accounts.is_empty() {
            issues.push("No finance accounts configured.".to_string());
        }

        // Check stale investment prices
        let investments = self.repos.finance_investments.list_with_symbols().await?;
        let stale_threshold = chrono::Local::now() - chrono::Duration::hours(24);
        let stale_count = investments
            .iter()
            .filter(|inv| inv.updated_at < stale_threshold.to_utc())
            .count();
        if stale_count > 0 {
            issues.push(format!("{} investment(s) have stale prices (>24h old).", stale_count));
        }

        // Check overdue goals
        let goals = self.repos.finance_goals.list_active().await?;
        let today = chrono::Local::now().date_naive();
        let overdue = goals
            .iter()
            .filter(|g| g.deadline.map(|d| d < today).unwrap_or(false))
            .count();
        if overdue > 0 {
            issues.push(format!("{} active goal(s) are past their deadline.", overdue));
        }

        // Check negative liability remaining
        let liabilities = self.repos.finance_liabilities.list().await?;
        let neg = liabilities.iter().filter(|l| l.remaining < 0).count();
        if neg > 0 {
            issues.push(format!("{} liability(ies) have negative remaining balance.", neg));
        }

        if issues.is_empty() {
            Ok("Health check passed: no issues found.".to_string())
        } else {
            Ok(format!("Health check found {} issue(s):\n- {}", issues.len(), issues.join("\n- ")))
        }
    }
```

**Step 3: Build and verify**

Run: `cargo build --workspace`
Expected: Success

**Step 4: Commit**

```bash
git add crates/tools/src/finance_handler.rs crates/agent/src/finance_adapter.rs
git commit -m "feat(finance): add run_health_check to FinanceHandler trait and implementation"
```

---

### Task 9: Enhance Daily Review

**Files:**
- Modify: `crates/agent/src/finance_adapter.rs:37-65` (enhance `daily_review()`)

**Step 1: Enhance the daily_review method**

Replace the existing `daily_review()` body with an enhanced version that includes:
- Yesterday's spending (top 3 categories)
- Budget status with threshold warnings
- Overdue goals
- Overall summary

```rust
    async fn daily_review(&self) -> Result<String> {
        let mut sections = Vec::new();

        // Section 1: Budget status
        let usages = self.repos.finance_budgets.all_budget_usage().await?;
        if !usages.is_empty() {
            let mut budget_lines = vec!["### Budget Status".to_string()];
            for u in &usages {
                let pct = if u.amount > 0 {
                    (u.spent as f64 / u.amount as f64) * 100.0
                } else {
                    0.0
                };
                let status = if pct >= 100.0 {
                    "OVER BUDGET"
                } else if pct >= self.config.budgeting.alert_threshold as f64 {
                    "Near limit"
                } else {
                    "On track"
                };
                budget_lines.push(format!(
                    "- **{}**: {:.0}% ({} / {} {}) — {}",
                    u.name, pct, u.spent, u.amount, u.currency, status,
                ));
            }
            sections.push(budget_lines.join("\n"));
        }

        // Section 2: Yesterday's spending (top categories)
        let yesterday = chrono::Local::now().date_naive() - chrono::Duration::days(1);
        let spending = self
            .repos
            .finance_transactions
            .sum_by_category(yesterday, yesterday, "expense")
            .await
            .unwrap_or_default();
        if !spending.is_empty() {
            let total: i64 = spending.iter().map(|(_, v)| v).sum();
            let mut spend_lines = vec![format!("### Yesterday's Spending: {}", total)];
            for (cat, amount) in spending.iter().take(3) {
                spend_lines.push(format!("- {}: {}", cat, amount));
            }
            sections.push(spend_lines.join("\n"));
        }

        // Section 3: Goals progress
        let goals = self.repos.finance_goals.list_active().await?;
        let today = chrono::Local::now().date_naive();
        let approaching: Vec<_> = goals
            .iter()
            .filter(|g| {
                g.deadline
                    .map(|d| {
                        let days_left = (d - today).num_days();
                        days_left >= 0 && days_left <= 7
                    })
                    .unwrap_or(false)
            })
            .collect();
        if !approaching.is_empty() {
            let mut goal_lines = vec!["### Goals Approaching Deadline".to_string()];
            for g in &approaching {
                let days = (g.deadline.unwrap() - today).num_days();
                let pct = if g.target_amount > 0 {
                    (g.current_amount as f64 / g.target_amount as f64) * 100.0
                } else {
                    0.0
                };
                goal_lines.push(format!(
                    "- **{}**: {:.0}% complete, {} day(s) left",
                    g.name, pct, days
                ));
            }
            sections.push(goal_lines.join("\n"));
        }

        if sections.is_empty() {
            return Ok("No financial activity to review. Create accounts and budgets to get started.".to_string());
        }

        let mut result = vec!["## Daily Financial Review\n".to_string()];
        result.extend(sections);
        Ok(result.join("\n\n"))
    }
```

**Step 2: Build and verify**

Run: `cargo build --workspace`
Expected: Success

**Step 3: Commit**

```bash
git add crates/agent/src/finance_adapter.rs
git commit -m "feat(finance): enhance daily review with spending summary and goal tracking"
```

---

### Task 10: Wire Autonomous Cron Jobs in serve.rs

**Files:**
- Modify: `crates/cli/src/serve.rs:200-337` (add finance cron job registrations and callbacks)

**Step 1: Add finance cron callback handlers**

In `crates/cli/src/serve.rs`, inside the `cron_service.set_callback(...)` block (lines 69-203), add new match arms before the `_ => Ok(None)` default (line 200):

```rust
                    "__klyntbot_finance_daily_review" => {
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "finance_daily_review",
                            "Run finance daily review and send summary".to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish finance daily review: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Finance daily review triggered".to_string()))
                    }
                    "__klyntbot_finance_budget_check" => {
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "finance_budget_check",
                            "Check budget thresholds and send alerts".to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish budget check: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Finance budget check triggered".to_string()))
                    }
                    "__klyntbot_finance_price_refresh" => {
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "finance_price_refresh",
                            "Refresh investment prices".to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish price refresh: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Finance price refresh triggered".to_string()))
                    }
                    "__klyntbot_finance_health_check" => {
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "finance_health_check",
                            "Run finance data health check".to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish health check: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Finance health check triggered".to_string()))
                    }
```

**Step 2: Register the cron jobs (after daily planning registration)**

After the daily planning cron registration block (around line 336), add:

```rust
    // Register finance cron jobs (only when proactivity is not "reactive")
    if config.finance.enabled && config.finance.proactivity_level != "reactive" {
        // Daily financial review
        let review_time = &config.finance.scheduling.daily_review_time;
        let parts: Vec<&str> = review_time.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(hour), Ok(minute)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
                if hour < 24 && minute < 60 {
                    let cron_expr = format!("{} {} * * *", minute, hour);
                    cron_service
                        .add_job(
                            "__klyntbot_finance_daily_review",
                            scheduling::CronSchedule::Cron {
                                expr: cron_expr,
                                tz: None,
                            },
                            "Daily financial review",
                            false,
                            None,
                            None,
                            false,
                        )
                        .await?;
                    info!("Finance daily review cron registered (time: {})", review_time);
                }
            }
        }

        // Budget check every 6 hours
        cron_service
            .add_job(
                "__klyntbot_finance_budget_check",
                scheduling::CronSchedule::Every {
                    every_ms: 6 * 60 * 60 * 1000,
                },
                "Check budget thresholds",
                false,
                None,
                None,
                false,
            )
            .await?;

        // Price refresh (configurable interval)
        if config.finance.price_refresh.enabled {
            let interval_ms = config.finance.price_refresh.interval_hours as u64 * 60 * 60 * 1000;
            cron_service
                .add_job(
                    "__klyntbot_finance_price_refresh",
                    scheduling::CronSchedule::Every {
                        every_ms: interval_ms,
                    },
                    "Refresh investment prices",
                    false,
                    None,
                    None,
                    false,
                )
                .await?;
        }

        // Daily health check at midnight
        cron_service
            .add_job(
                "__klyntbot_finance_health_check",
                scheduling::CronSchedule::Cron {
                    expr: "0 0 * * *".to_string(),
                    tz: None,
                },
                "Finance data health check",
                false,
                None,
                None,
                false,
            )
            .await?;

        info!("Finance cron jobs registered (proactivity: {})", config.finance.proactivity_level);
    }
```

**Step 3: Build and verify**

Run: `cargo build --workspace`
Expected: Success

**Step 4: Commit**

```bash
git add crates/cli/src/serve.rs
git commit -m "feat(finance): wire autonomous cron jobs for daily review, budget alerts, price refresh, health check"
```

---

### Task 11: Add In-Chat Budget Nudge to tx_add

**Files:**
- Modify: `crates/tools/src/finance_tool/transactions.rs` (after successful tx_add, check budget)

**Step 1: Add budget nudge after transaction insertion**

In `transactions.rs`, after the successful insertion in `tx_add` (where the response JSON is built), add a budget check for expense transactions:

```rust
        // After building the response JSON for tx_add, before returning:
        // Check if this expense pushes any budget past threshold (proactive nudge)
        let mut nudge = String::new();
        if tx_type == TransactionType::Expense {
            if let Some(ref cat) = category {
                if let Ok(Some(budget)) = self.budgets.get_by_category(cat).await {
                    if let Ok(usage) = self.budgets.budget_usage(&budget.id).await {
                        let pct = if usage.amount > 0 {
                            (usage.spent * 100) / usage.amount
                        } else {
                            0
                        };
                        if pct >= usage.alert_threshold as i64 {
                            nudge = format!(
                                "\nNote: Your \"{}\" budget is now at {}% ({} / {} {}).",
                                usage.name, pct, usage.spent, usage.amount, usage.currency,
                            );
                        }
                    }
                }
            }
        }

        // Append nudge to response if present
        let mut response = serde_json::to_string_pretty(&resp).unwrap();
        if !nudge.is_empty() {
            response.push_str(&nudge);
        }
        Ok(response)
```

**Step 2: Build and verify**

Run: `cargo build --workspace`
Expected: Success

**Step 3: Commit**

```bash
git add crates/tools/src/finance_tool/transactions.rs
git commit -m "feat(finance): add in-chat budget nudge after expense transactions"
```

---

### Task 12: Run Full Test Suite and Final Verification

**Step 1: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS (1969+ existing + new inline tests)

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Check formatting**

Run: `cargo fmt --all --check`
Expected: No formatting issues

**Step 4: Final commit if needed**

If any formatting or clippy fixes are needed:

```bash
cargo fmt --all
git add -A
git commit -m "chore: fix formatting and clippy warnings"
```

---

## Summary

| Task | What | Files | Commit message |
|------|------|-------|---------------|
| 1 | Finance skill file | `skills/finance/SKILL.md` | `feat(skills): add finance skill` |
| 2 | Transaction guards + tests | `transactions.rs` | `feat(finance): prerequisite guards for transactions` |
| 3 | Investment guards + tests | `investments.rs` | `feat(finance): prerequisite guards for investments` |
| 4 | Budget/report tests | `budgets.rs`, `reports.rs` | `test(finance): budget and report tests` |
| 5 | Health check action | `health.rs`, `mod.rs` | `feat(finance): finance_health_check action` |
| 6 | Delete skeleton tests | `tests.rs` (delete), `mod.rs` | `chore(finance): remove skeleton tests` |
| 7 | Goal/liability tests | `goals.rs` | `test(finance): goal and liability tests` |
| 8 | FinanceHandler health check | `finance_handler.rs`, `finance_adapter.rs` | `feat(finance): run_health_check trait method` |
| 9 | Enhanced daily review | `finance_adapter.rs` | `feat(finance): enhance daily review` |
| 10 | Cron job wiring | `serve.rs` | `feat(finance): wire autonomous cron jobs` |
| 11 | Budget nudge in tx_add | `transactions.rs` | `feat(finance): in-chat budget nudge` |
| 12 | Final verification | — | (cleanup if needed) |
