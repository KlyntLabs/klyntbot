//! FinanceTool — unified tool for all personal finance actions.
//!
//! Dispatches 37+ actions across 7 sub-modules. Each sub-module holds
//! a group of `handle_*` methods implemented on `FinanceTool`.

mod accounts;
mod budgets;
mod goals;
mod investments;
mod reports;
mod settings;
mod transactions;

// tests.rs contains todo!()-based skeletons that will be enabled once
// the action handlers are implemented. Not compiled yet.
// #[cfg(test)]
// mod tests;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use super::{RoutingContext, Tool};
use crate::finance_handler::FinanceHandler;
use crate::params::ParamExtractor;
use crate::price_service::PriceService;
use common::{Result, ToolError};

/// Tool for managing personal finance: accounts, transactions, budgets,
/// investments, goals, liabilities, and financial reports.
// Fields are `pub(crate)` so sub-module implementations can access them directly.
// `dead_code` is suppressed for `timezone` which is plumbed through but not yet
// consumed by any action handler (reserved for date-localisation in reports).
#[allow(dead_code)]
pub struct FinanceTool {
    pub(crate) accounts: storage::FinanceAccountRepo,
    pub(crate) transactions: storage::FinanceTransactionRepo,
    pub(crate) budgets: storage::FinanceBudgetRepo,
    pub(crate) investments: storage::FinanceInvestmentRepo,
    pub(crate) goals: storage::FinanceGoalRepo,
    pub(crate) liabilities: storage::FinanceLiabilityRepo,
    pub(crate) price_service: PriceService,
    pub(crate) finance_handler: Option<Arc<dyn FinanceHandler>>,
    pub(crate) default_currency: String,
    pub(crate) timezone: String,
}

impl FinanceTool {
    /// Construct a new `FinanceTool` with all required repos and services.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        accounts: storage::FinanceAccountRepo,
        transactions: storage::FinanceTransactionRepo,
        budgets: storage::FinanceBudgetRepo,
        investments: storage::FinanceInvestmentRepo,
        goals: storage::FinanceGoalRepo,
        liabilities: storage::FinanceLiabilityRepo,
        price_service: PriceService,
        default_currency: String,
        timezone: String,
    ) -> Self {
        Self {
            accounts,
            transactions,
            budgets,
            investments,
            goals,
            liabilities,
            price_service,
            finance_handler: None,
            default_currency,
            timezone,
        }
    }

    /// Attach an optional `FinanceHandler` for proactive behaviors (daily review,
    /// budget alerts, price refresh). Returns `self` for builder-style chaining.
    pub fn with_finance_handler(mut self, handler: Arc<dyn FinanceHandler>) -> Self {
        self.finance_handler = Some(handler);
        self
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
                        "settings_get", "settings_update"
                    ],
                    "description": "Finance action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Resource ID (account, transaction, budget, investment, goal, liability, portfolio)"
                },
                "name": {
                    "type": "string",
                    "description": "Name for the new resource"
                },
                "type": {
                    "type": "string",
                    "description": "Account type (cash/bank/ewallet/crypto_wallet/brokerage/other), transaction type (income/expense/transfer), liability type (mortgage/credit_card/personal_loan/student_loan/other), or asset type (stock/etf/crypto/real_estate/bond/other)"
                },
                "currency": {
                    "type": "string",
                    "description": "ISO 4217 currency code (e.g. VND, USD, EUR)"
                },
                "balance": {
                    "type": "integer",
                    "description": "Account balance in smallest currency unit (e.g. cents)"
                },
                "amount": {
                    "type": "integer",
                    "description": "Transaction or budget amount in smallest currency unit"
                },
                "account_id": {
                    "type": "string",
                    "description": "Account ID for transactions"
                },
                "transfer_to_account_id": {
                    "type": "string",
                    "description": "Destination account for transfer transactions"
                },
                "category": {
                    "type": "string",
                    "description": "Transaction or budget category (e.g. food, transport)"
                },
                "subcategory": {
                    "type": "string",
                    "description": "Transaction subcategory"
                },
                "counterparty": {
                    "type": "string",
                    "description": "Transaction counterparty name"
                },
                "notes": {
                    "type": "string",
                    "description": "Free-text notes"
                },
                "tx_date": {
                    "type": "string",
                    "description": "Transaction date (YYYY-MM-DD)"
                },
                "date_from": {
                    "type": "string",
                    "description": "Start date filter (YYYY-MM-DD)"
                },
                "date_to": {
                    "type": "string",
                    "description": "End date filter (YYYY-MM-DD)"
                },
                "amount_min": {
                    "type": "integer",
                    "description": "Minimum amount filter"
                },
                "amount_max": {
                    "type": "integer",
                    "description": "Maximum amount filter"
                },
                "query": {
                    "type": "string",
                    "description": "Free-text search query"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default 20, max 100)"
                },
                "recurring_rule": {
                    "type": "string",
                    "description": "Cron expression for recurring transactions (e.g. '0 9 1 * *')"
                },
                "period": {
                    "type": "string",
                    "enum": ["monthly", "weekly", "yearly", "custom", "month", "week", "year", "last_30_days", "this_year"],
                    "description": "Budget period or report time period"
                },
                "method": {
                    "type": "string",
                    "enum": ["standard", "six_jar"],
                    "description": "Budget allocation method"
                },
                "jar_type": {
                    "type": "string",
                    "enum": ["essentials", "savings", "investment", "education", "entertainment", "charity"],
                    "description": "Six-Jar budget category (required when method=six_jar)"
                },
                "start_date": {
                    "type": "string",
                    "description": "Budget start date (YYYY-MM-DD)"
                },
                "end_date": {
                    "type": "string",
                    "description": "Budget end date (YYYY-MM-DD, optional)"
                },
                "alert_threshold": {
                    "type": "integer",
                    "description": "Budget alert threshold percentage (0-100, default 80)"
                },
                "portfolio_id": {
                    "type": "string",
                    "description": "Portfolio ID for investments"
                },
                "description": {
                    "type": "string",
                    "description": "Portfolio or goal description"
                },
                "asset_type": {
                    "type": "string",
                    "enum": ["stock", "etf", "crypto", "real_estate", "bond", "other"],
                    "description": "Asset type for investments"
                },
                "symbol": {
                    "type": "string",
                    "description": "Ticker symbol for stocks/ETFs/crypto (e.g. AAPL, bitcoin)"
                },
                "quantity": {
                    "type": "number",
                    "description": "Number of units held or traded"
                },
                "cost_basis": {
                    "type": "integer",
                    "description": "Total cost basis in smallest currency unit"
                },
                "current_price": {
                    "type": "integer",
                    "description": "Current price per unit in smallest currency unit"
                },
                "purchase_date": {
                    "type": "string",
                    "description": "Date of investment purchase (YYYY-MM-DD)"
                },
                "tx_type": {
                    "type": "string",
                    "enum": ["buy", "sell", "dividend", "rental_income", "interest", "split"],
                    "description": "Investment transaction type"
                },
                "price_per_unit": {
                    "type": "integer",
                    "description": "Price per unit for investment transactions"
                },
                "total_amount": {
                    "type": "integer",
                    "description": "Total transaction amount in smallest currency unit"
                },
                "fees": {
                    "type": "integer",
                    "description": "Transaction fees in smallest currency unit"
                },
                "principal": {
                    "type": "integer",
                    "description": "Original loan/liability principal"
                },
                "remaining": {
                    "type": "integer",
                    "description": "Remaining balance on liability"
                },
                "interest_rate": {
                    "type": "number",
                    "description": "Annual interest rate percentage (e.g. 18.0 for 18%)"
                },
                "monthly_payment": {
                    "type": "integer",
                    "description": "Monthly payment amount in smallest currency unit"
                },
                "due_date": {
                    "type": "string",
                    "description": "Liability due date (YYYY-MM-DD)"
                },
                "goal_type": {
                    "type": "string",
                    "enum": ["savings", "purchase", "debt_payoff", "fire", "custom"],
                    "description": "Financial goal type"
                },
                "target_amount": {
                    "type": "integer",
                    "description": "Goal target amount in smallest currency unit"
                },
                "current_amount": {
                    "type": "integer",
                    "description": "Current progress amount for the goal"
                },
                "deadline": {
                    "type": "string",
                    "description": "Goal deadline (YYYY-MM-DD)"
                },
                "monthly_contribution": {
                    "type": "integer",
                    "description": "Monthly savings contribution toward the goal"
                },
                "expected_return_rate": {
                    "type": "number",
                    "description": "Expected annual return rate (e.g. 7.0 for 7%)"
                },
                "inflation_rate": {
                    "type": "number",
                    "description": "Annual inflation rate assumption (e.g. 3.0 for 3%)"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "achieved", "abandoned"],
                    "description": "Goal status"
                },
                "annual_expenses": {
                    "type": "integer",
                    "description": "Annual expenses for FIRE calculation (smallest currency unit)"
                },
                "withdrawal_rate": {
                    "type": "number",
                    "description": "Safe withdrawal rate for FIRE (default 4.0 for 4%)"
                },
                "extra_monthly_savings": {
                    "type": "integer",
                    "description": "Hypothetical extra monthly savings for what-if analysis"
                },
                "extra_return_rate": {
                    "type": "number",
                    "description": "Hypothetical extra return rate for what-if analysis"
                },
                "metric": {
                    "type": "string",
                    "enum": ["spending", "income", "savings_rate"],
                    "description": "Trend metric to analyse"
                },
                "interval": {
                    "type": "string",
                    "enum": ["weekly", "monthly", "quarterly"],
                    "description": "Interval for net worth history"
                },
                "target_currency": {
                    "type": "string",
                    "description": "Target currency for net worth conversion (e.g. USD)"
                },
                "institution": {
                    "type": "string",
                    "description": "Financial institution name for accounts"
                },
                "is_archived": {
                    "type": "boolean",
                    "description": "Whether to include archived accounts"
                },
                "proactivity_level": {
                    "type": "string",
                    "enum": ["full", "moderate", "reactive"],
                    "description": "Proactivity level for autonomous finance actions"
                },
                "confidence_threshold": {
                    "type": "number",
                    "description": "Confidence threshold for automated actions (0.0-1.0)"
                },
                "default_currency": {
                    "type": "string",
                    "description": "New default currency setting"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            // ── Account actions ────────────────────────────────────────────
            "account_add" | "account_list" | "account_update" | "account_delete" => {
                self.handle_account(action, &p, ctx).await
            }

            // ── Transaction actions ────────────────────────────────────────
            "tx_add" | "tx_list" | "tx_update" | "tx_delete" | "tx_search" | "tx_recurring_add" => {
                self.handle_transaction(action, &p, ctx).await
            }

            // ── Budget actions ─────────────────────────────────────────────
            "budget_create" | "budget_list" | "budget_status" | "budget_update"
            | "budget_delete" => self.handle_budget(action, &p, ctx).await,

            // ── Investment & portfolio actions ─────────────────────────────
            "portfolio_create" | "portfolio_list" | "investment_add" | "investment_update"
            | "investment_tx" | "investment_summary" | "price_fetch" | "price_refresh" => {
                self.handle_investment(action, &p, ctx).await
            }

            // ── Goal, liability & net-worth actions ────────────────────────
            "goal_create" | "goal_list" | "goal_update" | "goal_fire" | "goal_whatif"
            | "liability_add" | "liability_list" | "liability_update" | "net_worth" => {
                self.handle_goal(action, &p, ctx).await
            }

            // ── Report actions ─────────────────────────────────────────────
            "report_spending"
            | "report_income"
            | "report_trends"
            | "report_net_worth_history"
            | "daily_review" => self.handle_report(action, &p, ctx).await,

            // ── Settings actions ───────────────────────────────────────────
            "settings_get" | "settings_update" => self.handle_settings(action, &p, ctx).await,

            _ => {
                Err(ToolError::InvalidParams(format!("Unknown finance action: {}", action)).into())
            }
        }
    }
}
