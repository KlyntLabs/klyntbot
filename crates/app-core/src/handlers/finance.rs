//! Finance handlers — read-only queries + mutations against FinanceStorage repos.

use std::collections::HashMap;

use chrono::Datelike;
use desktop_shared::commands::{
    CurrencyNetWorth, FinanceAccountCreateParams, FinanceAccountUpdateParams,
    FinanceBudgetCreateParams, FinanceBudgetUpdateParams, FinanceCategoryBreakdown,
    FinanceCategoryReportResponse, FinanceGoalCreateParams, FinanceGoalUpdateParams,
    FinanceInvestmentCreateParams, FinanceInvestmentUpdateParams, FinanceLiabilityCreateParams,
    FinanceLiabilityUpdateParams, FinanceNetWorthResponse, FinancePortfolioCreateParams,
    FinancePortfolioResponse, FinanceTransactionCreateParams, FinanceTransactionFilterParams,
    FinanceTrendPoint,
};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use futures_util::future::try_join_all;
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountPatch, FinanceAccountRow, FinanceBudgetPatch, FinanceBudgetRow,
    FinanceGoalPatch, FinanceGoalRow, FinanceInvestmentFilter, FinanceInvestmentPatch,
    FinanceInvestmentRow, FinanceLiabilityPatch, FinanceLiabilityRow, FinancePortfolioRow,
    FinanceTransactionFilter, FinanceTransactionRow,
};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, EntityUpdate, HandlerResult};

impl AppCore {
    // ── Helpers ──────────────────────────────────────────────────

    /// Read the configured default currency (e.g. "USD", "VND").
    async fn default_currency(&self) -> String {
        self.config.read().await.finance.default_currency.clone()
    }

    /// Build the entity-update vec common to all finance mutations.
    fn finance_updates(id: String) -> Vec<EntityUpdate> {
        vec![EntityUpdate {
            kind: EntityKind::Finance,
            id,
        }]
    }

    // ── Read-only queries ────────────────────────────────────────

    pub async fn finance_accounts(&self) -> Result<Vec<FinanceAccountRow>, ApiError> {
        self.repos
            .finance
            .accounts
            .list(false)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_transactions(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            limit,
            ..Default::default()
        };
        self.repos
            .finance
            .transactions
            .list(&filter)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_budget_usage(&self) -> Result<Vec<BudgetUsageRow>, ApiError> {
        self.repos
            .finance
            .budgets
            .all_budget_usage()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_portfolios(&self) -> Result<Vec<FinancePortfolioResponse>, ApiError> {
        let portfolios = self
            .repos
            .finance
            .investments
            .list_portfolios()
            .await
            .map_err(map_storage_err)?;

        let summaries = try_join_all(
            portfolios
                .iter()
                .map(|p| self.repos.finance.investments.portfolio_summary(&p.id)),
        )
        .await
        .map_err(map_storage_err)?;

        Ok(portfolios
            .iter()
            .zip(summaries)
            .map(|(p, summary)| FinancePortfolioResponse {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
                currency: p.currency.clone(),
                total_value: summary.total_current_value,
                total_cost_basis: summary.total_cost_basis,
                holding_count: summary.holding_count,
            })
            .collect())
    }

    pub async fn finance_investments(&self) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        self.repos
            .finance
            .investments
            .list_investments(&Default::default())
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_goals(&self) -> Result<Vec<FinanceGoalRow>, ApiError> {
        self.repos
            .finance
            .goals
            .list_all()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_liabilities(&self) -> Result<Vec<FinanceLiabilityRow>, ApiError> {
        self.repos
            .finance
            .liabilities
            .list_all()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_net_worth(&self) -> Result<FinanceNetWorthResponse, ApiError> {
        let (account_totals, investment_totals, liability_totals) = tokio::try_join!(
            self.repos.finance.accounts.total_balance_by_currency(),
            self.repos.finance.investments.total_value_by_currency(),
            self.repos.finance.liabilities.total_remaining_by_currency(),
        )
        .map_err(map_storage_err)?;

        let mut by_currency: HashMap<&str, CurrencyNetWorth> = HashMap::new();

        for (currency, total) in &account_totals {
            by_currency
                .entry(currency)
                .or_insert_with(|| CurrencyNetWorth::zero(currency.clone()))
                .accounts = *total;
        }
        for (currency, total) in &investment_totals {
            by_currency
                .entry(currency)
                .or_insert_with(|| CurrencyNetWorth::zero(currency.clone()))
                .investments = *total;
        }
        for (currency, total) in &liability_totals {
            by_currency
                .entry(currency)
                .or_insert_with(|| CurrencyNetWorth::zero(currency.clone()))
                .liabilities = *total;
        }

        let totals_by_currency: Vec<CurrencyNetWorth> = by_currency
            .into_values()
            .map(|mut c| {
                c.net = c.accounts + c.investments - c.liabilities;
                c
            })
            .collect();

        Ok(FinanceNetWorthResponse { totals_by_currency })
    }

    pub async fn finance_exchange_rates(&self) -> Result<HashMap<String, f64>, ApiError> {
        let config = self.config.read().await;
        Ok(config.finance.exchange_rates.clone().unwrap_or_default())
    }

    // ── Mutations ────────────────────────────────────────────────

    pub async fn finance_account_create(
        &self,
        params: FinanceAccountCreateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceAccountRow {
            id: id.clone(),
            name: params.name,
            account_type: params.account_type,
            currency,
            balance: params.balance.unwrap_or(0),
            institution: params.institution,
            notes: params.notes,
            is_archived: false,
            created_at: now,
            updated_at: now,
        };

        self.repos
            .finance
            .accounts
            .add(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_account_update(
        &self,
        params: FinanceAccountUpdateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let patch = FinanceAccountPatch {
            id: params.id.clone(),
            name: params.name,
            balance: params.balance,
            institution: params.institution,
            notes: params.notes,
            is_archived: params.is_archived,
        };
        let row = self
            .repos
            .finance
            .accounts
            .update(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_account_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos
            .finance
            .accounts
            .delete(&id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    pub async fn finance_transaction_create(
        &self,
        params: FinanceTransactionCreateParams,
    ) -> HandlerResult<FinanceTransactionRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let tx_date = params
            .tx_date
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| now.date_naive());

        let account = self
            .repos
            .finance
            .accounts
            .get_or_err(&params.account_id)
            .await
            .map_err(map_storage_err)?;
        let currency = params.currency.unwrap_or(account.currency.clone());

        let row = FinanceTransactionRow {
            id: id.clone(),
            account_id: params.account_id.clone(),
            tx_type: params.tx_type.clone(),
            amount: params.amount,
            currency,
            category: params.category,
            subcategory: params.subcategory,
            counterparty: params.counterparty,
            notes: params.notes,
            tx_date,
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
        };

        self.repos
            .finance
            .transactions
            .add(&row)
            .await
            .map_err(map_storage_err)?;

        // Adjust account balance
        let delta = match params.tx_type.as_str() {
            "income" => params.amount,
            "expense" => -params.amount,
            _ => 0,
        };
        if delta != 0 {
            self.repos
                .finance
                .accounts
                .adjust_balance(&params.account_id, delta)
                .await
                .map_err(map_storage_err)?;
        }

        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_transaction_delete(&self, id: String) -> HandlerResult<bool> {
        let tx = self
            .repos
            .finance
            .transactions
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        if let Some(tx) = tx {
            // Reverse the balance adjustment
            let delta = match tx.tx_type.as_str() {
                "income" => -tx.amount,
                "expense" => tx.amount,
                _ => 0,
            };
            if delta != 0 {
                self.repos
                    .finance
                    .accounts
                    .adjust_balance(&tx.account_id, delta)
                    .await
                    .map_err(map_storage_err)?;
            }
        }

        Ok((true, Self::finance_updates(id)))
    }

    pub async fn finance_budget_create(
        &self,
        params: FinanceBudgetCreateParams,
    ) -> HandlerResult<FinanceBudgetRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let start_date = params
            .start_date
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| now.date_naive());
        let end_date = params.end_date.and_then(|d| parse_naive_date(&d));
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceBudgetRow {
            id: id.clone(),
            name: params.name,
            amount: params.amount,
            currency,
            period: params.period,
            category: params.category,
            method: params.method.unwrap_or_else(|| "standard".to_string()),
            jar_type: None,
            start_date,
            end_date,
            is_active: true,
            alert_threshold: params.alert_threshold.unwrap_or(80),
            created_at: now,
            updated_at: now,
        };

        self.repos
            .finance
            .budgets
            .add(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_budget_update(
        &self,
        params: FinanceBudgetUpdateParams,
    ) -> HandlerResult<FinanceBudgetRow> {
        let patch = FinanceBudgetPatch {
            id: params.id.clone(),
            name: params.name,
            amount: params.amount,
            category: params.category,
            is_active: params.is_active,
        };
        let row = self
            .repos
            .finance
            .budgets
            .update(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_budget_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos
            .finance
            .budgets
            .delete(&id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    pub async fn finance_goal_create(
        &self,
        params: FinanceGoalCreateParams,
    ) -> HandlerResult<FinanceGoalRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let deadline = params.deadline.and_then(|d| parse_naive_date(&d));
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceGoalRow {
            id: id.clone(),
            name: params.name,
            goal_type: params.goal_type,
            target_amount: params.target_amount,
            current_amount: params.current_amount.unwrap_or(0),
            currency,
            status: "active".to_string(),
            deadline,
            monthly_contribution: params.monthly_contribution,
            expected_return_rate: None,
            inflation_rate: None,
            notes: params.notes,
            created_at: now,
            updated_at: now,
        };

        self.repos
            .finance
            .goals
            .add(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_goal_update(
        &self,
        params: FinanceGoalUpdateParams,
    ) -> HandlerResult<FinanceGoalRow> {
        let deadline = params
            .deadline
            .map(|opt| opt.and_then(|d| parse_naive_date(&d)));
        let patch = FinanceGoalPatch {
            id: params.id.clone(),
            current_amount: params.current_amount,
            target_amount: params.target_amount,
            monthly_contribution: params.monthly_contribution,
            deadline,
            status: params.status,
            ..Default::default()
        };
        let row = self
            .repos
            .finance
            .goals
            .update(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_goal_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos
            .finance
            .goals
            .delete(&id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    pub async fn finance_liability_create(
        &self,
        params: FinanceLiabilityCreateParams,
    ) -> HandlerResult<FinanceLiabilityRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let due_date = params.due_date.and_then(|d| parse_naive_date(&d));
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceLiabilityRow {
            id: id.clone(),
            name: params.name,
            liability_type: params.liability_type,
            principal: params.principal,
            remaining: params.remaining.unwrap_or(params.principal),
            currency,
            interest_rate: params.interest_rate,
            monthly_payment: params.monthly_payment,
            due_date,
            notes: params.notes,
            created_at: now,
            updated_at: now,
        };

        self.repos
            .finance
            .liabilities
            .add(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_liability_update(
        &self,
        params: FinanceLiabilityUpdateParams,
    ) -> HandlerResult<FinanceLiabilityRow> {
        let patch = FinanceLiabilityPatch {
            id: params.id.clone(),
            remaining: params.remaining,
            monthly_payment: params.monthly_payment,
            interest_rate: params.interest_rate,
            notes: params.notes,
        };
        let row = self
            .repos
            .finance
            .liabilities
            .update(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_liability_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos
            .finance
            .liabilities
            .delete(&id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    pub async fn finance_portfolio_create(
        &self,
        params: FinancePortfolioCreateParams,
    ) -> HandlerResult<FinancePortfolioRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };
        let row = FinancePortfolioRow {
            id: id.clone(),
            name: params.name,
            description: params.description,
            currency,
            created_at: now,
            updated_at: now,
        };
        self.repos
            .finance
            .investments
            .add_portfolio(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_investment_create(
        &self,
        params: FinanceInvestmentCreateParams,
    ) -> HandlerResult<FinanceInvestmentRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let purchase_date = params.purchase_date.and_then(|d| parse_naive_date(&d));
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceInvestmentRow {
            id: id.clone(),
            portfolio_id: params.portfolio_id,
            asset_type: params.asset_type,
            symbol: params.symbol,
            name: params.name.unwrap_or_default(),
            quantity: params.quantity,
            cost_basis: params.cost_basis,
            currency,
            current_price: None,
            current_value: None,
            purchase_date,
            notes: params.notes,
            created_at: now,
            updated_at: now,
        };
        self.repos
            .finance
            .investments
            .add_investment(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_investment_update(
        &self,
        params: FinanceInvestmentUpdateParams,
    ) -> HandlerResult<FinanceInvestmentRow> {
        let patch = FinanceInvestmentPatch {
            id: params.id.clone(),
            current_price: params.current_price,
            current_value: params.current_value,
            quantity: params.quantity,
            notes: params.notes,
            ..Default::default()
        };
        let row = self
            .repos
            .finance
            .investments
            .update_investment(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    // ── Upgraded queries ─────────────────────────────────────────

    pub async fn finance_transactions_filtered(
        &self,
        params: FinanceTransactionFilterParams,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            account_id: params.account_id,
            tx_type: params.tx_type,
            category: params.category,
            date_from: params.date_from.and_then(|d| parse_naive_date(&d)),
            date_to: params.date_to.and_then(|d| parse_naive_date(&d)),
            query: params.query,
            limit: params.limit,
            ..Default::default()
        };
        self.repos
            .finance
            .transactions
            .list(&filter)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_investments_filtered(
        &self,
        portfolio_id: Option<String>,
    ) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        let filter = FinanceInvestmentFilter {
            portfolio_id,
            ..Default::default()
        };
        self.repos
            .finance
            .investments
            .list_investments(&filter)
            .await
            .map_err(map_storage_err)
    }

    // ── Reports ──────────────────────────────────────────────────

    pub async fn finance_report_spending(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<FinanceCategoryReportResponse, ApiError> {
        self.finance_report_by_type(date_from, date_to, "expense")
            .await
    }

    pub async fn finance_report_income(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<FinanceCategoryReportResponse, ApiError> {
        self.finance_report_by_type(date_from, date_to, "income")
            .await
    }

    async fn finance_report_by_type(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
        tx_type: &str,
    ) -> Result<FinanceCategoryReportResponse, ApiError> {
        let now = chrono::Utc::now().date_naive();
        let from = date_from
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| now.with_day(1).unwrap_or(now));
        let to = date_to.and_then(|d| parse_naive_date(&d)).unwrap_or(now);

        let rows = self
            .repos
            .finance
            .transactions
            .sum_by_category(from, to, tx_type)
            .await
            .map_err(map_storage_err)?;

        let total: i64 = rows.iter().map(|(_, amt)| amt).sum();
        let breakdown = rows
            .into_iter()
            .map(|(category, amount)| FinanceCategoryBreakdown {
                category,
                amount,
                pct: if total > 0 {
                    (amount as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();

        Ok(FinanceCategoryReportResponse { total, breakdown })
    }

    pub async fn finance_report_trends(
        &self,
        metric: String,
        periods: Option<i64>,
    ) -> Result<Vec<FinanceTrendPoint>, ApiError> {
        let n = periods.unwrap_or(6).min(24);
        let tx_type = match metric.as_str() {
            "income" => "income",
            _ => "expense",
        };
        let rows = self
            .repos
            .finance
            .transactions
            .sum_by_period(tx_type, n as i32, "monthly")
            .await
            .map_err(map_storage_err)?;

        let points: Vec<FinanceTrendPoint> = rows
            .iter()
            .enumerate()
            .map(|(i, (period, value))| {
                let change_pct = if i > 0 {
                    let prev = rows[i - 1].1;
                    if prev > 0 {
                        Some(((value - prev) as f64 / prev as f64) * 100.0)
                    } else {
                        None
                    }
                } else {
                    None
                };
                FinanceTrendPoint {
                    period: period.clone(),
                    value: *value,
                    change_pct,
                }
            })
            .collect();

        Ok(points)
    }
}
