//! Bidirectional Row ↔ Domain `From` impls (16 total).

use common::time::bridge::{chrono_date_to_jiff, chrono_to_jiff, jiff_date_to_chrono, jiff_to_chrono};
use storage::rows::finance::{
    FinanceAccountRow, FinanceBudgetRow, FinanceGoalRow, FinanceInvestmentRow,
    FinanceInvestmentTxRow, FinanceLiabilityRow, FinancePortfolioRow, FinanceTransactionRow,
};

use super::domain::*;

// ── FinanceAccount ──────────────────────────────────────────

impl From<FinanceAccountRow> for FinanceAccount {
    fn from(row: FinanceAccountRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            account_type: AccountType::from_str_loose(&row.account_type).unwrap_or_default(),
            currency: row.currency,
            balance: row.balance,
            institution: row.institution,
            notes: row.notes,
            is_archived: row.is_archived,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinanceAccount> for FinanceAccountRow {
    fn from(account: &FinanceAccount) -> Self {
        Self {
            id: account.id.clone(),
            name: account.name.clone(),
            account_type: account.account_type.as_str().to_string(),
            currency: account.currency.clone(),
            balance: account.balance,
            institution: account.institution.clone(),
            notes: account.notes.clone(),
            is_archived: account.is_archived,
            created_at: chrono_to_jiff(account.created_at).into(),
            updated_at: chrono_to_jiff(account.updated_at).into(),
            base_balance: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }
}

// ── FinanceTransaction ──────────────────────────────────────

impl From<FinanceTransactionRow> for FinanceTransaction {
    fn from(row: FinanceTransactionRow) -> Self {
        Self {
            id: row.id,
            account_id: row.account_id,
            tx_type: TransactionType::from_str_loose(&row.tx_type).unwrap_or_default(),
            amount: row.amount,
            currency: row.currency,
            category: row.category,
            subcategory: row.subcategory,
            counterparty: row.counterparty,
            notes: row.notes,
            tx_date: jiff_date_to_chrono(*row.tx_date),
            transfer_id: row.transfer_id,
            is_recurring: row.is_recurring,
            recurring_rule: row.recurring_rule,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinanceTransaction> for FinanceTransactionRow {
    fn from(tx: &FinanceTransaction) -> Self {
        Self {
            id: tx.id.clone(),
            account_id: tx.account_id.clone(),
            tx_type: tx.tx_type.as_str().to_string(),
            amount: tx.amount,
            currency: tx.currency.clone(),
            category: tx.category.clone(),
            subcategory: tx.subcategory.clone(),
            counterparty: tx.counterparty.clone(),
            notes: tx.notes.clone(),
            tx_date: chrono_date_to_jiff(tx.tx_date).into(),
            transfer_id: tx.transfer_id.clone(),
            is_recurring: tx.is_recurring,
            recurring_rule: tx.recurring_rule.clone(),
            created_at: chrono_to_jiff(tx.created_at).into(),
            updated_at: chrono_to_jiff(tx.updated_at).into(),
            base_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }
}

// ── FinanceBudget ────────────────────────────────────────────

impl From<FinanceBudgetRow> for FinanceBudget {
    fn from(row: FinanceBudgetRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            amount: row.amount,
            currency: row.currency,
            period: BudgetPeriod::from_str_loose(&row.period).unwrap_or_default(),
            category: row.category,
            method: BudgetMethod::from_str_loose(&row.method).unwrap_or_default(),
            jar_type: row.jar_type.as_deref().and_then(JarType::from_str_loose),
            start_date: jiff_date_to_chrono(*row.start_date),
            end_date: row.end_date.map(|d| jiff_date_to_chrono(*d)),
            is_active: row.is_active,
            alert_threshold: row.alert_threshold,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinanceBudget> for FinanceBudgetRow {
    fn from(budget: &FinanceBudget) -> Self {
        Self {
            id: budget.id.clone(),
            name: budget.name.clone(),
            amount: budget.amount,
            currency: budget.currency.clone(),
            period: budget.period.as_str().to_string(),
            category: budget.category.clone(),
            method: budget.method.as_str().to_string(),
            jar_type: budget.jar_type.map(|j| j.as_str().to_string()),
            start_date: chrono_date_to_jiff(budget.start_date).into(),
            end_date: budget.end_date.map(|d| chrono_date_to_jiff(d).into()),
            is_active: budget.is_active,
            alert_threshold: budget.alert_threshold,
            created_at: chrono_to_jiff(budget.created_at).into(),
            updated_at: chrono_to_jiff(budget.updated_at).into(),
            base_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }
}

// ── FinancePortfolio ─────────────────────────────────────────

impl From<FinancePortfolioRow> for FinancePortfolio {
    fn from(row: FinancePortfolioRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            currency: row.currency,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinancePortfolio> for FinancePortfolioRow {
    fn from(portfolio: &FinancePortfolio) -> Self {
        Self {
            id: portfolio.id.clone(),
            name: portfolio.name.clone(),
            description: portfolio.description.clone(),
            currency: portfolio.currency.clone(),
            created_at: chrono_to_jiff(portfolio.created_at).into(),
            updated_at: chrono_to_jiff(portfolio.updated_at).into(),
        }
    }
}

// ── FinanceInvestment ─────────────────────────────────────────

impl From<FinanceInvestmentRow> for FinanceInvestment {
    fn from(row: FinanceInvestmentRow) -> Self {
        Self {
            id: row.id,
            portfolio_id: row.portfolio_id,
            asset_type: AssetType::from_str_loose(&row.asset_type).unwrap_or_default(),
            symbol: row.symbol,
            name: row.name,
            quantity: row.quantity.parse::<f64>().unwrap_or(0.0),
            cost_basis: row.cost_basis,
            currency: row.currency,
            current_price: row.current_price,
            current_value: row.current_value,
            purchase_date: row.purchase_date.map(|d| jiff_date_to_chrono(*d)),
            asset_class: row.asset_class,
            notes: row.notes,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinanceInvestment> for FinanceInvestmentRow {
    fn from(inv: &FinanceInvestment) -> Self {
        Self {
            id: inv.id.clone(),
            portfolio_id: inv.portfolio_id.clone(),
            asset_type: inv.asset_type.as_str().to_string(),
            symbol: inv.symbol.clone(),
            name: inv.name.clone(),
            quantity: inv.quantity.to_string(),
            cost_basis: inv.cost_basis,
            currency: inv.currency.clone(),
            current_price: inv.current_price,
            current_value: inv.current_value,
            purchase_date: inv.purchase_date.map(|d| chrono_date_to_jiff(d).into()),
            asset_class: inv.asset_class.clone(),
            notes: inv.notes.clone(),
            created_at: chrono_to_jiff(inv.created_at).into(),
            updated_at: chrono_to_jiff(inv.updated_at).into(),
            market_currency: None,
            base_cost_basis: 0,
            base_current_value: 0,
            base_currency: "USD".to_string(),
            purchase_rate: 1.0,
            market_rate: 1.0,
        }
    }
}

// ── FinanceInvestmentTx ───────────────────────────────────────

impl From<FinanceInvestmentTxRow> for FinanceInvestmentTx {
    fn from(row: FinanceInvestmentTxRow) -> Self {
        Self {
            id: row.id,
            investment_id: row.investment_id,
            tx_type: InvestmentTxType::from_str_loose(&row.tx_type).unwrap_or_default(),
            quantity: row.quantity,
            price_per_unit: row.price_per_unit,
            total_amount: row.total_amount,
            currency: row.currency,
            fees: row.fees,
            tx_date: jiff_date_to_chrono(*row.tx_date),
            notes: row.notes,
            created_at: jiff_to_chrono(*row.created_at),
        }
    }
}

impl From<&FinanceInvestmentTx> for FinanceInvestmentTxRow {
    fn from(tx: &FinanceInvestmentTx) -> Self {
        Self {
            id: tx.id.clone(),
            investment_id: tx.investment_id.clone(),
            tx_type: tx.tx_type.as_str().to_string(),
            quantity: tx.quantity,
            price_per_unit: tx.price_per_unit,
            total_amount: tx.total_amount,
            currency: tx.currency.clone(),
            fees: tx.fees,
            tx_date: chrono_date_to_jiff(tx.tx_date).into(),
            notes: tx.notes.clone(),
            created_at: chrono_to_jiff(tx.created_at).into(),
            base_total_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }
}

// ── FinanceGoal ───────────────────────────────────────────────

impl From<FinanceGoalRow> for FinanceGoal {
    fn from(row: FinanceGoalRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            goal_type: GoalType::from_str_loose(&row.goal_type).unwrap_or_default(),
            target_amount: row.target_amount,
            current_amount: row.current_amount,
            currency: row.currency,
            status: GoalStatus::from_str_loose(&row.status).unwrap_or_default(),
            deadline: row.deadline.map(|d| jiff_date_to_chrono(*d)),
            monthly_contribution: row.monthly_contribution,
            expected_return_rate: row.expected_return_rate,
            inflation_rate: row.inflation_rate,
            notes: row.notes,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinanceGoal> for FinanceGoalRow {
    fn from(goal: &FinanceGoal) -> Self {
        Self {
            id: goal.id.clone(),
            name: goal.name.clone(),
            goal_type: goal.goal_type.as_str().to_string(),
            target_amount: goal.target_amount,
            current_amount: goal.current_amount,
            currency: goal.currency.clone(),
            status: goal.status.as_str().to_string(),
            deadline: goal.deadline.map(|d| chrono_date_to_jiff(d).into()),
            monthly_contribution: goal.monthly_contribution,
            expected_return_rate: goal.expected_return_rate,
            inflation_rate: goal.inflation_rate,
            notes: goal.notes.clone(),
            created_at: chrono_to_jiff(goal.created_at).into(),
            updated_at: chrono_to_jiff(goal.updated_at).into(),
            base_target_amount: 0,
            base_current_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }
}

// ── FinanceLiability ──────────────────────────────────────────

impl From<FinanceLiabilityRow> for FinanceLiability {
    fn from(row: FinanceLiabilityRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            liability_type: LiabilityType::from_str_loose(&row.liability_type).unwrap_or_default(),
            principal: row.principal,
            remaining: row.remaining,
            currency: row.currency,
            interest_rate: row.interest_rate,
            monthly_payment: row.monthly_payment,
            due_date: row.due_date.map(|d| jiff_date_to_chrono(*d)),
            notes: row.notes,
            created_at: jiff_to_chrono(*row.created_at),
            updated_at: jiff_to_chrono(*row.updated_at),
        }
    }
}

impl From<&FinanceLiability> for FinanceLiabilityRow {
    fn from(liability: &FinanceLiability) -> Self {
        Self {
            id: liability.id.clone(),
            name: liability.name.clone(),
            liability_type: liability.liability_type.as_str().to_string(),
            principal: liability.principal,
            remaining: liability.remaining,
            currency: liability.currency.clone(),
            interest_rate: liability.interest_rate,
            monthly_payment: liability.monthly_payment,
            due_date: liability.due_date.map(|d| chrono_date_to_jiff(d).into()),
            notes: liability.notes.clone(),
            created_at: chrono_to_jiff(liability.created_at).into(),
            updated_at: chrono_to_jiff(liability.updated_at).into(),
            base_principal: 0,
            base_remaining: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }
}
