//! FinanceTool — unified tool for all personal finance actions.
//!
//! Dispatches 37+ actions across 7 sub-modules. Each sub-module holds
//! a group of `handle_*` methods implemented on `FinanceTool`.

mod accounts;
mod budgets;
mod goals;
mod health;
mod investments;
mod reports;
mod settings;
mod transactions;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use common::{Result, ToolError};
use tools_core::{ConfigPersistence, ParamExtractor, RoutingContext, Tool};

/// Parse a `YYYY-MM-DD` date string into a `NaiveDate`.
pub(crate) fn parse_date(s: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ToolError::InvalidParams(format!("Invalid date format: {s}")).into())
}

use crate::handler::FinanceHandler;
use crate::price_service::PriceService;

/// Tool for managing personal finance: accounts, transactions, budgets,
/// investments, goals, liabilities, and financial reports.
// Fields are `pub(crate)` so sub-module implementations can access them directly.
pub struct FinanceTool {
    pub(crate) storage: storage::FinanceStorage,
    pub(crate) price_service: PriceService,
    pub(crate) finance_handler: Option<Arc<dyn FinanceHandler>>,
    pub(crate) default_currency: String,
    pub(crate) config_persistence: Option<Arc<dyn ConfigPersistence>>,
}

impl FinanceTool {
    /// Construct a new `FinanceTool` from a `FinanceStorage` aggregate.
    pub fn new(
        storage: storage::FinanceStorage,
        price_service: PriceService,
        default_currency: String,
    ) -> Self {
        Self {
            storage,
            price_service,
            finance_handler: None,
            default_currency,
            config_persistence: None,
        }
    }

    /// Attach an optional `FinanceHandler`. Returns `self` for builder-style chaining.
    pub fn with_finance_handler(mut self, handler: Arc<dyn FinanceHandler>) -> Self {
        self.finance_handler = Some(handler);
        self
    }

    /// Attach an optional `ConfigPersistence` for settings actions.
    pub fn with_config_persistence(mut self, cp: Arc<dyn ConfigPersistence>) -> Self {
        self.config_persistence = Some(cp);
        self
    }

    /// Convenience constructor: build a `FinanceTool` from a `StoragePool`.
    pub fn from_storage_pool(
        pool: &storage::StoragePool,
        default_currency: impl Into<String>,
    ) -> Self {
        Self::new(
            storage::FinanceStorage::from_pool(pool.inner()),
            crate::price_service::PriceService::new(15),
            default_currency.into(),
        )
    }
}

#[async_trait]
impl Tool for FinanceTool {
    fn name(&self) -> &str {
        "finance"
    }

    fn description(&self) -> &str {
        "Manage personal finances: accounts, transactions, budgets, investments, portfolios, \
         goals, liabilities, net worth, FIRE planning, spending reports, and settings. \
         Actions: account_add, account_list, account_update, account_delete, \
         tx_add, tx_list, tx_update, tx_delete, tx_search, tx_recurring_add, \
         budget_create, budget_list, budget_status, budget_update, budget_delete, \
         portfolio_create, portfolio_list, investment_add, investment_update, \
         investment_tx, investment_summary, price_fetch, price_refresh, \
         liability_add, liability_list, liability_update, net_worth, \
         goal_create, goal_list, goal_update, goal_fire, goal_whatif, \
         report_spending, report_income, report_trends, report_net_worth_history, \
         daily_review, settings_get, settings_update."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "account_add", "account_list", "account_update", "account_delete",
                        "tx_add", "tx_list", "tx_update", "tx_delete", "tx_search", "tx_recurring_add",
                        "budget_create", "budget_list", "budget_status", "budget_update", "budget_delete",
                        "portfolio_create", "portfolio_list",
                        "investment_add", "investment_update", "investment_tx", "investment_summary",
                        "price_fetch", "price_refresh",
                        "liability_add", "liability_list", "liability_update", "net_worth",
                        "goal_create", "goal_list", "goal_update", "goal_fire", "goal_whatif",
                        "report_spending", "report_income", "report_trends", "report_net_worth_history",
                        "daily_review",
                        "finance_health_check",
                        "settings_get", "settings_update"
                    ],
                    "description": "Finance action to perform"
                },
                "id": { "type": "string" },
                "name": { "type": "string" },
                "type": { "type": "string" },
                "currency": { "type": "string" },
                "balance": { "type": "integer" },
                "amount": { "type": "integer" },
                "account_id": { "type": "string" },
                "transfer_to_account_id": { "type": "string" },
                "category": { "type": "string" },
                "subcategory": { "type": "string" },
                "counterparty": { "type": "string" },
                "notes": { "type": "string" },
                "tx_date": { "type": "string" },
                "date_from": { "type": "string" },
                "date_to": { "type": "string" },
                "amount_min": { "type": "integer" },
                "amount_max": { "type": "integer" },
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "recurring_rule": { "type": "string" },
                "period": { "type": "string" },
                "method": { "type": "string" },
                "jar_type": { "type": "string" },
                "start_date": { "type": "string" },
                "end_date": { "type": "string" },
                "alert_threshold": { "type": "integer" },
                "portfolio_id": { "type": "string" },
                "description": { "type": "string" },
                "asset_type": { "type": "string" },
                "symbol": { "type": "string" },
                "quantity": { "type": "number" },
                "cost_basis": { "type": "integer" },
                "current_price": { "type": "integer" },
                "purchase_date": { "type": "string" },
                "tx_type": { "type": "string" },
                "price_per_unit": { "type": "integer" },
                "total_amount": { "type": "integer" },
                "fees": { "type": "integer" },
                "principal": { "type": "integer" },
                "remaining": { "type": "integer" },
                "interest_rate": { "type": "number" },
                "monthly_payment": { "type": "integer" },
                "due_date": { "type": "string" },
                "goal_type": { "type": "string" },
                "target_amount": { "type": "integer" },
                "current_amount": { "type": "integer" },
                "deadline": { "type": "string" },
                "monthly_contribution": { "type": "integer" },
                "expected_return_rate": { "type": "number" },
                "inflation_rate": { "type": "number" },
                "status": { "type": "string" },
                "annual_expenses": { "type": "integer" },
                "withdrawal_rate": { "type": "number" },
                "extra_monthly_savings": { "type": "integer" },
                "extra_return_rate": { "type": "number" },
                "metric": { "type": "string" },
                "interval": { "type": "string" },
                "target_currency": { "type": "string" },
                "institution": { "type": "string" },
                "is_archived": { "type": "boolean" },
                "proactivity_level": { "type": "string" },
                "confidence_threshold": { "type": "number" },
                "default_currency": { "type": "string" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "account_add" | "account_list" | "account_update" | "account_delete" => {
                self.handle_account(action, &p, ctx).await
            }

            "tx_add" | "tx_list" | "tx_update" | "tx_delete" | "tx_search" | "tx_recurring_add" => {
                self.handle_transaction(action, &p, ctx).await
            }

            "budget_create" | "budget_list" | "budget_status" | "budget_update"
            | "budget_delete" => self.handle_budget(action, &p, ctx).await,

            "portfolio_create" | "portfolio_list" | "investment_add" | "investment_update"
            | "investment_tx" | "investment_summary" | "price_fetch" | "price_refresh" => {
                self.handle_investment(action, &p, ctx).await
            }

            "goal_create" | "goal_list" | "goal_update" | "goal_fire" | "goal_whatif"
            | "liability_add" | "liability_list" | "liability_update" | "net_worth" => {
                self.handle_goal(action, &p, ctx).await
            }

            "report_spending"
            | "report_income"
            | "report_trends"
            | "report_net_worth_history"
            | "daily_review" => self.handle_report(action, &p, ctx).await,

            "finance_health_check" => self.finance_health_check(ctx).await,

            "settings_get" | "settings_update" => self.handle_settings(action, &p, ctx).await,

            _ => {
                Err(ToolError::InvalidParams(format!("Unknown finance action: {}", action)).into())
            }
        }
    }
}
