//! FinanceTool — unified tool for all personal finance actions.
//!
//! Dispatches 37+ actions across 7 sub-modules. Each sub-module holds
//! a group of `handle_*` methods implemented on `FinanceTool`.

mod accounts;
mod allocations;
mod analyze_handlers;
mod budgets;
mod fire_handlers;
mod goals;
mod health;
mod investments;
mod reports;
mod settings;
mod snapshots;
mod transactions;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use bus::DomainEventBus;
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
    pub(crate) domain_bus: Option<Arc<DomainEventBus>>,
    pub(crate) rate_cache: Option<crate::rate_cache::RateCache>,
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
            domain_bus: None,
            rate_cache: None,
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

    /// Attach a domain event bus for cross-feature communication.
    pub fn with_domain_bus(mut self, bus: Arc<DomainEventBus>) -> Self {
        self.domain_bus = Some(bus);
        self
    }

    /// Attach a `RateCache` for two-layer exchange rate caching. Returns `self` for chaining.
    pub fn with_rate_cache(mut self, cache: crate::rate_cache::RateCache) -> Self {
        self.rate_cache = Some(cache);
        self
    }

    /// Convenience constructor: build a `FinanceTool` from a `StoragePool`.
    ///
    /// Creates a `RateCache` backed by the pool and wires it into the `PriceService`.
    pub fn from_storage_pool(
        pool: &storage::StoragePool,
        default_currency: impl Into<String>,
    ) -> Self {
        let exchange_rates = storage::repos::FinanceExchangeRateRepo::new(pool.inner().clone());
        let rate_cache = crate::rate_cache::RateCache::new(exchange_rates, 15);
        let price_service =
            crate::price_service::PriceService::with_rate_cache(15, rate_cache.clone());
        let mut tool = Self::new(
            storage::FinanceStorage::from_pool(pool.inner()),
            price_service,
            default_currency.into(),
        );
        tool.rate_cache = Some(rate_cache);
        tool
    }
}

#[async_trait]
impl Tool for FinanceTool {
    fn name(&self) -> &str {
        "finance"
    }

    fn description(&self) -> &str {
        "Manage personal finances: accounts, transactions, budgets, investments, portfolios, \
         goals, liabilities, net worth, FIRE planning, spending analytics, portfolio analytics, \
         allocation targets, snapshots, and settings. \
         Actions: account_add, account_list, account_update, account_delete, \
         tx_add, tx_list, tx_update, tx_delete, tx_search, tx_recurring_add, \
         budget_create, budget_list, budget_status, budget_update, budget_delete, \
         portfolio_create, portfolio_list, portfolio_delete, \
         investment_add, investment_update, investment_delete, \
         investment_tx, investment_summary, price_fetch, price_refresh, \
         portfolio_drift, portfolio_rebalance, portfolio_returns, portfolio_correlation, \
         liability_add, liability_list, liability_update, liability_delete, net_worth, \
         goal_create, goal_list, goal_update, goal_delete, goal_fire, goal_whatif, \
         report_spending, report_income, report_trends, report_net_worth_history, \
         daily_review, analyze_spending_anomalies, analyze_spending_trends, \
         analyze_recurring_charges, analyze_category_correlation, \
         fire_traditional, fire_coast, fire_lean, fire_fat, \
         fire_withdrawal_sim, fire_backtest, fire_sensitivity, \
         allocation_target_set, allocation_target_list, allocation_target_delete, \
         snapshot_record, snapshot_history, settings_get, settings_update."
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
                        "portfolio_create", "portfolio_list", "portfolio_delete",
                        "investment_add", "investment_update", "investment_delete", "investment_tx", "investment_summary",
                        "price_fetch", "price_refresh",
                        "portfolio_drift", "portfolio_rebalance", "portfolio_returns", "portfolio_correlation",
                        "liability_add", "liability_list", "liability_update", "liability_delete", "net_worth",
                        "goal_create", "goal_list", "goal_update", "goal_delete", "goal_fire", "goal_whatif",
                        "report_spending", "report_income", "report_trends", "report_net_worth_history",
                        "daily_review",
                        "finance_health_check",
                        "analyze_spending_anomalies", "analyze_spending_trends",
                        "analyze_recurring_charges", "analyze_category_correlation",
                        "fire_traditional", "fire_coast", "fire_lean", "fire_fat",
                        "fire_withdrawal_sim", "fire_backtest", "fire_sensitivity",
                        "allocation_target_set", "allocation_target_list", "allocation_target_delete",
                        "snapshot_record", "snapshot_history",
                        "settings_get", "settings_update"
                    ],
                    "description": "Finance action to perform"
                },
                "id": { "type": "string" },
                "name": { "type": "string" },
                "type": {
                    "type": "string",
                    "enum": ["bank", "cash", "ewallet", "crypto_wallet", "brokerage", "other"],
                    "description": "Account type. Use 'bank' for bank accounts, 'cash' for cash, 'ewallet' for e-wallets, 'crypto_wallet' for crypto wallets, 'brokerage' for investment/brokerage accounts, 'other' for anything else."
                },
                "currency": { "type": "string" },
                "balance": { "type": "integer" },
                "amount": {
                    "type": "integer",
                    "description": "Amount in smallest currency unit (e.g. cents for USD, dong for VND). $50.00 = 5000."
                },
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
                "goal_status": { "type": "string", "enum": ["active", "completed", "paused", "all"], "description": "Goal status filter (for goal_list)" },
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
                "default_currency": { "type": "string" },
                // Analytics params
                "current_portfolio": { "type": "integer" },
                "monthly_savings": { "type": "integer" },
                "expected_return": { "type": "number" },
                "annual_withdrawal": { "type": "integer" },
                "years": { "type": "integer" },
                "monte_carlo_runs": { "type": "integer" },
                "std_dev": { "type": "number" },
                "seed": { "type": "integer" },
                "strategy": { "type": "string" },
                "runs_per_point": { "type": "integer" },
                "current_age": { "type": "integer" },
                "target_age": { "type": "integer" },
                "annual_expenses_at_retirement": { "type": "integer" },
                "essential_expenses": { "type": "integer" },
                "desired_annual_spending": { "type": "integer" },
                "withdrawal_rates": { "type": "string" },
                "lookback_months": { "type": "integer" },
                "z_threshold": { "type": "number" },
                "window_months": { "type": "integer" },
                "min_occurrences": { "type": "integer" },
                "min_months": { "type": "integer" },
                "asset_class": { "type": "string" },
                "target_weight": { "type": "number" },
                "tolerance_band": { "type": "number" },
                "contribution": { "type": "integer" },
                "min_trade_amount": { "type": "integer" },
                "rebalance_strategy": { "type": "string" },
                "market_currency": { "type": "string", "description": "Currency the asset is quoted in on exchanges (e.g., USD for BTC). Defaults to purchase currency." }
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

            "portfolio_create"
            | "portfolio_list"
            | "portfolio_delete"
            | "investment_add"
            | "investment_update"
            | "investment_delete"
            | "investment_tx"
            | "investment_summary"
            | "price_fetch"
            | "price_refresh"
            | "portfolio_drift"
            | "portfolio_rebalance"
            | "portfolio_returns"
            | "portfolio_correlation" => self.handle_investment(action, &p, ctx).await,

            "goal_create" | "goal_list" | "goal_update" | "goal_delete" | "goal_fire"
            | "goal_whatif" | "liability_add" | "liability_list" | "liability_update"
            | "liability_delete" | "net_worth" => self.handle_goal(action, &p, ctx).await,

            "report_spending"
            | "report_income"
            | "report_trends"
            | "report_net_worth_history"
            | "daily_review" => self.handle_report(action, &p, ctx).await,

            "finance_health_check" => self.finance_health_check(ctx).await,

            // Spending analytics
            "analyze_spending_anomalies"
            | "analyze_spending_trends"
            | "analyze_recurring_charges"
            | "analyze_category_correlation" => self.handle_analyze(action, &p, ctx).await,

            // FIRE planning
            "fire_traditional"
            | "fire_coast"
            | "fire_lean"
            | "fire_fat"
            | "fire_withdrawal_sim"
            | "fire_backtest"
            | "fire_sensitivity" => self.handle_fire(action, &p, ctx).await,

            // Allocation targets
            "allocation_target_set" | "allocation_target_list" | "allocation_target_delete" => {
                self.handle_allocation(action, &p, ctx).await
            }

            // Net worth snapshots
            "snapshot_record" | "snapshot_history" => self.handle_snapshot(action, &p, ctx).await,

            "settings_get" | "settings_update" => self.handle_settings(action, &p, ctx).await,

            _ => {
                Err(ToolError::InvalidParams(format!("Unknown finance action: {}", action)).into())
            }
        }
    }
}
