//! Integration tests for /api/finance/* endpoints.
//!
//! Covers AC-14.x: Full CRUD for all 6 finance sub-resources:
//! accounts, transactions, budgets, investments, goals, liabilities.
//!
//! All monetary amounts are stored and transmitted as integer cents (i64).

mod common;

// ── Accounts ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_finance_accounts_empty_returns_array() {
    // AC-14.1: GET /api/finance/accounts returns []
    // TODO: implement
}

#[tokio::test]
async fn post_finance_account_creates_account() {
    // AC-14.1: POST /api/finance/accounts with required fields → 201
    // Required: name, accountType, currency, balance (cents)
    // TODO: implement
}

#[tokio::test]
async fn get_finance_account_by_id() {
    // AC-14.1: GET /api/finance/accounts/{id} returns FinanceAccountRow
    // TODO: implement
}

#[tokio::test]
async fn patch_finance_account_updates_fields() {
    // AC-14.1: PATCH /api/finance/accounts/{id}
    // TODO: implement
}

#[tokio::test]
async fn delete_finance_account_returns_204() {
    // AC-14.1: DELETE /api/finance/accounts/{id} → 204
    // TODO: implement
}

// ── Transactions ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_finance_transactions_empty_returns_array() {
    // AC-14.2: GET /api/finance/transactions returns []
    // TODO: implement
}

#[tokio::test]
async fn post_finance_transaction_creates_transaction() {
    // AC-14.2: POST /api/finance/transactions with required fields → 201
    // Required: accountId, txType (income|expense|transfer), amount (cents), currency, txDate
    // TODO: implement
}

#[tokio::test]
async fn get_finance_transaction_by_id() {
    // AC-14.2: GET /api/finance/transactions/{id}
    // TODO: implement
}

#[tokio::test]
async fn patch_finance_transaction() {
    // AC-14.2: PATCH /api/finance/transactions/{id}
    // TODO: implement
}

#[tokio::test]
async fn delete_finance_transaction_returns_204() {
    // AC-14.2: DELETE /api/finance/transactions/{id} → 204
    // TODO: implement
}

#[tokio::test]
async fn finance_amounts_are_integer_cents() {
    // AC-14.x, UX §5.3: All monetary amounts are i64 integer cents, not floats
    // Create a transaction with amount=1050 (=$10.50), verify JSON has "amount":1050
    // TODO: implement
}

// ── Budgets ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_finance_budgets_empty_returns_array() {
    // AC-14.3: GET /api/finance/budgets returns []
    // TODO: implement
}

#[tokio::test]
async fn post_finance_budget_creates_budget() {
    // AC-14.3: POST /api/finance/budgets → 201
    // TODO: implement
}

#[tokio::test]
async fn patch_finance_budget() {
    // AC-14.3: PATCH /api/finance/budgets/{id}
    // TODO: implement
}

#[tokio::test]
async fn delete_finance_budget_returns_204() {
    // AC-14.3: DELETE /api/finance/budgets/{id} → 204
    // TODO: implement
}

#[tokio::test]
async fn get_finance_budgets_usage() {
    // AC-14.3: GET /api/finance/budgets/usage returns Vec<BudgetUsageRow>
    // TODO: implement
}

// ── Investments ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_finance_investments_empty_returns_array() {
    // AC-14.4: GET /api/finance/investments returns []
    // TODO: implement
}

#[tokio::test]
async fn post_finance_investment() {
    // AC-14.4: POST /api/finance/investments → 201
    // TODO: implement
}

#[tokio::test]
async fn patch_finance_investment() {
    // AC-14.4: PATCH /api/finance/investments/{id}
    // TODO: implement
}

#[tokio::test]
async fn delete_finance_investment_returns_204() {
    // AC-14.4: DELETE /api/finance/investments/{id} → 204
    // TODO: implement
}

// ── Goals ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_finance_goals_empty_returns_array() {
    // AC-14.5: GET /api/finance/goals returns []
    // TODO: implement
}

#[tokio::test]
async fn post_finance_goal() {
    // AC-14.5: POST /api/finance/goals → 201
    // TODO: implement
}

#[tokio::test]
async fn patch_finance_goal() {
    // AC-14.5: PATCH /api/finance/goals/{id}
    // TODO: implement
}

#[tokio::test]
async fn delete_finance_goal_returns_204() {
    // AC-14.5: DELETE /api/finance/goals/{id} → 204
    // TODO: implement
}

// ── Liabilities ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_finance_liabilities_empty_returns_array() {
    // AC-14.6: GET /api/finance/liabilities returns []
    // TODO: implement
}

#[tokio::test]
async fn post_finance_liability() {
    // AC-14.6: POST /api/finance/liabilities → 201
    // TODO: implement
}

#[tokio::test]
async fn patch_finance_liability() {
    // AC-14.6: PATCH /api/finance/liabilities/{id}
    // TODO: implement
}

#[tokio::test]
async fn delete_finance_liability_returns_204() {
    // AC-14.6: DELETE /api/finance/liabilities/{id} → 204
    // TODO: implement
}

// ── camelCase check ───────────────────────────────────────────────────────────

#[tokio::test]
async fn finance_response_fields_are_camel_case() {
    // AC-1.8: All finance row responses use camelCase field names
    // Check: accountType, txType, txDate, createdAt, updatedAt
    // TODO: implement
}
