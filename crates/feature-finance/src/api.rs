//! Tauri-facing finance API — CRUD operations backed by `FinanceStorage`.
//!
//! These functions encapsulate the business logic previously living in
//! `app-core::handlers::finance`.  They operate on `storage` native types so
//! the feature crate stays decoupled from `desktop_shared` command params.

use std::collections::HashMap;
use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};

use storage::rows::finance::*;
use storage::FinanceStorage;

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

pub async fn list_accounts(
    storage: &FinanceStorage,
) -> Result<Vec<FinanceAccountRow>, storage::StorageError> {
    storage.accounts.list(false).await
}

pub async fn create_account(
    storage: &FinanceStorage,
    row: &FinanceAccountRow,
) -> Result<FinanceAccountRow, storage::StorageError> {
    storage.accounts.add(row).await?;
    Ok(row.clone())
}

pub async fn update_account(
    storage: &FinanceStorage,
    patch: &FinanceAccountPatch,
) -> Result<FinanceAccountRow, storage::StorageError> {
    storage.accounts.update(patch).await
}

pub async fn delete_account(
    storage: &FinanceStorage,
    id: &str,
) -> Result<bool, storage::StorageError> {
    storage.accounts.delete(id).await
}

// ---------------------------------------------------------------------------
// Budgets
// ---------------------------------------------------------------------------

pub async fn budget_usage(
    storage: &FinanceStorage,
) -> Result<Vec<BudgetUsageRow>, storage::StorageError> {
    storage.budgets.all_budget_usage().await
}

pub async fn create_budget(
    storage: &FinanceStorage,
    row: &FinanceBudgetRow,
) -> Result<FinanceBudgetRow, storage::StorageError> {
    storage.budgets.add(row).await?;
    Ok(row.clone())
}

pub async fn update_budget(
    storage: &FinanceStorage,
    patch: &FinanceBudgetPatch,
) -> Result<FinanceBudgetRow, storage::StorageError> {
    storage.budgets.update(patch).await
}

pub async fn delete_budget(
    storage: &FinanceStorage,
    id: &str,
) -> Result<bool, storage::StorageError> {
    storage.budgets.delete(id).await
}

// ---------------------------------------------------------------------------
// Transactions
// ---------------------------------------------------------------------------

pub async fn list_transactions(
    storage: &FinanceStorage,
    filter: &FinanceTransactionFilter,
) -> Result<Vec<FinanceTransactionRow>, storage::StorageError> {
    storage.transactions.list(filter).await
}

/// Create a transaction, adjust the account balance, and emit relevant domain
/// events (TransactionRecorded, BudgetAlert).
pub async fn create_transaction(
    storage: &FinanceStorage,
    row: &FinanceTransactionRow,
    domain_bus: Option<&Arc<DomainEventBus>>,
) -> Result<FinanceTransactionRow, storage::StorageError> {
    storage.transactions.add(row).await?;

    // Adjust account balance
    let delta = match row.tx_type.as_str() {
        "income" => row.amount,
        "expense" => -row.amount,
        _ => 0,
    };
    if delta != 0 {
        storage
            .accounts
            .adjust_balance(&row.account_id, delta)
            .await?;
    }

    // Emit domain event for timeline tracking
    if let Some(bus) = domain_bus {
        bus.publish(DomainEvent::TransactionRecorded {
            category: row.category.clone().unwrap_or_default(),
            amount: row.amount as f64 / 100.0,
            is_over_budget: false,
        });
    }

    // Emit BudgetAlert if this expense crosses the budget limit for its category.
    if row.tx_type == "expense" {
        if let Some(ref category) = row.category {
            if let Ok(Some(budget)) = storage.budgets.get_by_category(category).await {
                if let Ok(usage) = storage.budgets.budget_usage(&budget.id).await {
                    let spent_after = usage.spent;
                    let this_tx_base = row.amount;
                    let spent_before = spent_after.saturating_sub(this_tx_base);
                    let limit = budget.amount;
                    if spent_after > limit && spent_before <= limit {
                        if let Some(bus) = domain_bus {
                            bus.publish(DomainEvent::BudgetAlert {
                                category: category.clone(),
                                spent: spent_after as f64 / 100.0,
                                limit: limit as f64 / 100.0,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(row.clone())
}

/// Delete a transaction and reverse the account balance adjustment.
pub async fn delete_transaction(
    storage: &FinanceStorage,
    id: &str,
) -> Result<Option<FinanceTransactionRow>, storage::StorageError> {
    let tx = storage.transactions.delete(id).await?;

    if let Some(ref tx) = tx {
        let delta = match tx.tx_type.as_str() {
            "income" => -tx.amount,
            "expense" => tx.amount,
            _ => 0,
        };
        if delta != 0 {
            storage
                .accounts
                .adjust_balance(&tx.account_id, delta)
                .await?;
        }
    }

    Ok(tx)
}

// ---------------------------------------------------------------------------
// Investments
// ---------------------------------------------------------------------------

pub async fn list_portfolios(
    storage: &FinanceStorage,
) -> Result<Vec<FinancePortfolioRow>, storage::StorageError> {
    storage.investments.list_portfolios().await
}

pub async fn portfolio_summaries(
    storage: &FinanceStorage,
    default_currency: &str,
) -> Result<Vec<PortfolioSummaryRow>, storage::StorageError> {
    let portfolios = storage.investments.list_portfolios().await?;
    let mut summaries = Vec::with_capacity(portfolios.len());
    for p in &portfolios {
        summaries.push(
            storage
                .investments
                .portfolio_summary(&p.id, default_currency)
                .await?,
        );
    }
    Ok(summaries)
}

pub async fn list_investments(
    storage: &FinanceStorage,
    filter: &FinanceInvestmentFilter,
) -> Result<Vec<FinanceInvestmentRow>, storage::StorageError> {
    storage.investments.list_investments(filter).await
}

pub async fn create_portfolio(
    storage: &FinanceStorage,
    row: &FinancePortfolioRow,
) -> Result<FinancePortfolioRow, storage::StorageError> {
    storage.investments.add_portfolio(row).await?;
    Ok(row.clone())
}

pub async fn create_investment(
    storage: &FinanceStorage,
    row: &FinanceInvestmentRow,
) -> Result<FinanceInvestmentRow, storage::StorageError> {
    storage.investments.add_investment(row).await?;
    Ok(row.clone())
}

pub async fn update_investment(
    storage: &FinanceStorage,
    patch: &FinanceInvestmentPatch,
) -> Result<FinanceInvestmentRow, storage::StorageError> {
    storage.investments.update_investment(patch).await
}

pub async fn upsert_allocation_target(
    storage: &FinanceStorage,
    portfolio_id: &str,
    asset_class: &str,
    target_weight: &str,
    tolerance_band: &str,
) -> Result<FinanceAllocationTargetRow, storage::StorageError> {
    storage
        .allocations
        .add(portfolio_id, asset_class, target_weight, tolerance_band)
        .await
}

pub async fn list_allocation_targets(
    storage: &FinanceStorage,
    portfolio_id: &str,
) -> Result<Vec<FinanceAllocationTargetRow>, storage::StorageError> {
    storage.allocations.list_by_portfolio(portfolio_id).await
}

pub async fn create_investment_tx(
    storage: &FinanceStorage,
    row: &FinanceInvestmentTxRow,
) -> Result<FinanceInvestmentTxRow, storage::StorageError> {
    storage.investments.add_investment_tx(row).await?;
    Ok(row.clone())
}

pub async fn list_investment_txs(
    storage: &FinanceStorage,
    investment_id: &str,
) -> Result<Vec<FinanceInvestmentTxRow>, storage::StorageError> {
    storage.investments.list_investment_txs(investment_id).await
}

// ---------------------------------------------------------------------------
// Goals
// ---------------------------------------------------------------------------

pub async fn list_goals(
    storage: &FinanceStorage,
) -> Result<Vec<FinanceGoalRow>, storage::StorageError> {
    storage.goals.list_all().await
}

pub async fn create_goal(
    storage: &FinanceStorage,
    row: &FinanceGoalRow,
) -> Result<FinanceGoalRow, storage::StorageError> {
    storage.goals.add(row).await?;
    Ok(row.clone())
}

pub async fn update_goal(
    storage: &FinanceStorage,
    patch: &FinanceGoalPatch,
    domain_bus: Option<&Arc<DomainEventBus>>,
) -> Result<FinanceGoalRow, storage::StorageError> {
    // Capture old current_amount before update (for delta).
    let old_current = if patch.current_amount.is_some() {
        storage
            .goals
            .get(&patch.id)
            .await
            .ok()
            .flatten()
            .map(|g| g.current_amount)
    } else {
        None
    };

    let row = storage.goals.update(patch).await?;

    if let Some(prev) = old_current {
        let delta = row.current_amount - prev;
        if delta != 0 {
            if let Some(bus) = domain_bus {
                bus.publish(DomainEvent::FinanceGoalProgress {
                    goal_id: row.id.clone(),
                    name: row.name.clone(),
                    current_amount: row.current_amount,
                    target_amount: row.target_amount,
                    delta,
                });
            }
        }
    }

    Ok(row)
}

pub async fn delete_goal(
    storage: &FinanceStorage,
    id: &str,
) -> Result<bool, storage::StorageError> {
    storage.goals.delete(id).await
}

// ---------------------------------------------------------------------------
// Liabilities
// ---------------------------------------------------------------------------

pub async fn list_liabilities(
    storage: &FinanceStorage,
) -> Result<Vec<FinanceLiabilityRow>, storage::StorageError> {
    storage.liabilities.list_all().await
}

pub async fn create_liability(
    storage: &FinanceStorage,
    row: &FinanceLiabilityRow,
) -> Result<FinanceLiabilityRow, storage::StorageError> {
    storage.liabilities.add(row).await?;
    Ok(row.clone())
}

pub async fn update_liability(
    storage: &FinanceStorage,
    patch: &FinanceLiabilityPatch,
) -> Result<FinanceLiabilityRow, storage::StorageError> {
    storage.liabilities.update(patch).await
}

pub async fn delete_liability(
    storage: &FinanceStorage,
    id: &str,
) -> Result<bool, storage::StorageError> {
    storage.liabilities.delete(id).await
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

/// Aggregate net worth across accounts, investments, and liabilities.
pub async fn net_worth(
    storage: &FinanceStorage,
) -> Result<Vec<(String, i64, i64, i64)>, storage::StorageError> {
    let (account_totals, investment_totals, liability_totals) = tokio::try_join!(
        storage.accounts.total_balance_by_currency(),
        storage.investments.total_value_by_currency(),
        storage.liabilities.total_remaining_by_currency(),
    )?;

    let mut by_currency: HashMap<String, (i64, i64, i64)> = HashMap::new();

    for (currency, total) in account_totals {
        by_currency.entry(currency).or_insert((0, 0, 0)).0 = total;
    }
    for (currency, total) in investment_totals {
        by_currency.entry(currency).or_insert((0, 0, 0)).1 = total;
    }
    for (currency, total) in liability_totals {
        by_currency.entry(currency).or_insert((0, 0, 0)).2 = total;
    }

    Ok(by_currency
        .into_iter()
        .map(|(currency, (accounts, investments, liabilities))| {
            (currency, accounts, investments, liabilities)
        })
        .collect())
}

/// Category breakdown for a transaction type over a date range.
pub async fn category_report(
    storage: &FinanceStorage,
    from: jiff::civil::Date,
    to: jiff::civil::Date,
    tx_type: &str,
    default_currency: &str,
) -> Result<Vec<(String, i64)>, storage::StorageError> {
    storage
        .transactions
        .sum_by_category(from, to, tx_type, default_currency)
        .await
}

/// Trend points by period.
pub async fn trend_report(
    storage: &FinanceStorage,
    tx_type: &str,
    periods: i32,
    default_currency: &str,
) -> Result<Vec<(String, i64)>, storage::StorageError> {
    storage
        .transactions
        .sum_by_period(tx_type, periods, default_currency)
        .await
}

/// Daily spending in a date range.
pub async fn daily_spending(
    storage: &FinanceStorage,
    from: jiff::civil::Date,
    to: jiff::civil::Date,
    default_currency: &str,
) -> Result<Vec<(String, i64, i32)>, storage::StorageError> {
    storage
        .transactions
        .daily_spending(from, to, default_currency)
        .await
}

/// Period summary (income + spending).
pub async fn period_summary(
    storage: &FinanceStorage,
    from: jiff::civil::Date,
    to: jiff::civil::Date,
    default_currency: &str,
) -> Result<(i64, i64), storage::StorageError> {
    let (income, spending) = tokio::try_join!(
        storage
            .transactions
            .sum_by_type_in_range("income", from, to, default_currency),
        storage
            .transactions
            .sum_by_type_in_range("expense", from, to, default_currency),
    )?;
    Ok((income, spending))
}

/// Monthly summary for the current and previous month.
pub async fn monthly_summary(
    storage: &FinanceStorage,
    default_currency: &str,
) -> Result<((String, i64, i64), (String, i64, i64)), storage::StorageError> {
    let now = jiff::Zoned::now();
    let current_month = now.strftime("%Y-%m").to_string();
    let prev_month = now
        .date()
        .with()
        .day(1)
        .build()
        .unwrap_or_else(|_| now.date())
        .checked_sub(jiff::Span::new().months(1))
        .unwrap_or_else(|_| now.date());
    let previous_month_label = prev_month.strftime("%Y-%m").to_string();

    let (income_rows, expense_rows) = tokio::try_join!(
        storage
            .transactions
            .sum_by_period("income", 3, default_currency),
        storage
            .transactions
            .sum_by_period("expense", 3, default_currency),
    )?;

    let income_map: HashMap<String, i64> = income_rows.into_iter().collect();
    let expense_map: HashMap<String, i64> = expense_rows.into_iter().collect();

    let current_income = *income_map.get(&current_month).unwrap_or(&0);
    let current_spending = *expense_map.get(&current_month).unwrap_or(&0);
    let previous_income = *income_map.get(&previous_month_label).unwrap_or(&0);
    let previous_spending = *expense_map.get(&previous_month_label).unwrap_or(&0);

    Ok((
        (current_month, current_income, current_spending),
        (previous_month_label, previous_income, previous_spending),
    ))
}
