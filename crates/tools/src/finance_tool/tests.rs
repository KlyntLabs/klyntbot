//! Skeleton tests for FinanceTool action dispatch.
//!
//! Tests cover all 37 finance actions: happy paths, validation errors,
//! not-found errors, and side effects (balance updates, budget impact, etc.).
//!
//! Test pattern:
//!   1. Create FinanceTool with repos (from test DB, or skip if unavailable)
//!   2. Call tool.execute(json!({"action": "..."}), &ctx).await
//!   3. Assert the response string or error type
//!
//! All tests call `todo!()` until FinanceTool is implemented.

#[cfg(test)]
mod tests {
    // use serde_json::json;
    // use crate::finance_tool::FinanceTool;
    // use crate::RoutingContext;
    // use storage::StoragePool;

    /// Create a FinanceTool with repos from the test database.
    /// Returns None if no database is available (allows tests to skip gracefully).
    #[allow(dead_code)]
    async fn make_finance_tool() -> Option<()> {
        // TODO: Replace `()` with `FinanceTool` once implemented.
        //
        // let url = std::env::var("DATABASE_URL")
        //     .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
        // match StoragePool::connect(&url).await {
        //     Ok(pool) => {
        //         let repos = storage::Repos::from_pool(&pool);
        //         Some(FinanceTool::new(
        //             repos.finance_accounts.clone(),
        //             repos.finance_transactions.clone(),
        //             repos.finance_budgets.clone(),
        //             repos.finance_investments.clone(),
        //             repos.finance_goals.clone(),
        //             repos.finance_liabilities.clone(),
        //             PriceService::new(15),
        //             "VND".to_string(),
        //             "UTC".to_string(),
        //         ))
        //     }
        //     Err(_) => None,
        // }
        None
    }

    #[allow(dead_code)]
    fn test_ctx() -> () {
        // TODO: Return RoutingContext::default() or similar once available.
    }

    // ==========================================================================
    // C.1 Account Actions
    // ==========================================================================

    // --- account_add ---

    /// AC 1.1: account_add with valid params creates account and returns confirmation.
    #[tokio::test]
    async fn account_add_happy_path() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tool.execute(json!({action: 'account_add', name: 'My Bank', \
            type: 'bank', currency: 'VND', balance: 1000000}), &ctx).await. \
            Assert: response is Ok(String). \
            Assert: response contains the account id and 'My Bank'. \
            Assert: response contains formatted balance '1,000,000'."
        )
    }

    /// AC 1.1 edge: invalid account type returns InvalidParams.
    #[tokio::test]
    async fn account_add_invalid_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tool.execute(json!({action: 'account_add', name: 'X', \
            type: 'checking', currency: 'VND', balance: 0}), &ctx).await. \
            Assert: returns Err with message containing 'Invalid account type: checking'. \
            Assert: error type is ToolError::InvalidParams."
        )
    }

    /// AC 1.1 edge: empty currency string returns InvalidParams.
    #[tokio::test]
    async fn account_add_empty_currency() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tool.execute(json!({action: 'account_add', name: 'X', \
            type: 'cash', currency: '', balance: 0}), &ctx).await. \
            Assert: Err with 'Currency must be a valid ISO 4217 code'."
        )
    }

    /// AC 1.1 edge: currency longer than 3 chars returns InvalidParams.
    #[tokio::test]
    async fn account_add_currency_too_long() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute with currency: 'USDC' (4 chars). \
            Assert: Err with 'Currency must be a valid ISO 4217 code'."
        )
    }

    /// AC 1.1 edge: empty name returns InvalidParams.
    #[tokio::test]
    async fn account_add_empty_name() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute with name: ''. \
            Assert: Err with 'Account name is required'."
        )
    }

    /// AC 1.1: negative balance is allowed (credit line / overdraft account).
    #[tokio::test]
    async fn account_add_negative_balance_allowed() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute with balance: -50000. \
            Assert: returns Ok (no error). Response confirms account created."
        )
    }

    // --- account_list ---

    /// AC 1.2 edge: no accounts → response says "No accounts found".
    #[tokio::test]
    async fn account_list_no_accounts_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: ensure no accounts in DB (isolated test). \
            Execute: action='account_list'. \
            Assert: response contains 'No accounts found'."
        )
    }

    /// AC 1.2: list with accounts groups by currency and shows totals.
    #[tokio::test]
    async fn account_list_with_accounts() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: 2 VND accounts + 1 USD account. \
            Execute: action='account_list'. \
            Assert: response contains 'VND' and 'USD' sections. \
            Assert: response contains the account names."
        )
    }

    // --- account_update / account_delete ---

    /// AC 1.3 edge: update with unknown id returns ExecutionFailed.
    #[tokio::test]
    async fn account_update_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='account_update', id='nonexistent'. \
            Assert: Err with 'Account nonexistent not found'. \
            Assert: error type is ToolError::ExecutionFailed."
        )
    }

    /// AC 1.3 edge: update with no fields to change returns InvalidParams.
    #[tokio::test]
    async fn account_update_no_fields() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='account_update', id='some-valid-id' (no other params). \
            Assert: Err with 'No fields to update'."
        )
    }

    /// AC 1.4 edge: delete with unknown id returns ExecutionFailed.
    #[tokio::test]
    async fn account_delete_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='account_delete', id='nonexistent'. \
            Assert: Err with 'Account nonexistent not found'."
        )
    }

    /// AC 1.4: delete account with transactions — response notes count of removed txns.
    #[tokio::test]
    async fn account_delete_cascades_transactions() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account with 3 transactions. \
            Execute: action='account_delete', id=that_account_id. \
            Assert: Ok response. \
            Assert: response contains '3 transactions removed'."
        )
    }

    // ==========================================================================
    // C.2 Transaction Actions
    // ==========================================================================

    // --- tx_add ---

    /// AC 2.1: expense transaction decrements account balance.
    #[tokio::test]
    async fn tx_add_expense_updates_balance() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account with balance=1000000. \
            Execute: tx_add(account_id, type='expense', amount=100000). \
            Assert: Ok response with new_balance=900000. \
            Assert: DB account balance is now 900000."
        )
    }

    /// AC 2.1: income transaction increments account balance.
    #[tokio::test]
    async fn tx_add_income_updates_balance() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account with balance=500000. \
            Execute: tx_add(type='income', amount=200000). \
            Assert: Ok response with new_balance=700000."
        )
    }

    /// AC 2.1 edge: tx_add with bad account_id returns ExecutionFailed.
    #[tokio::test]
    async fn tx_add_account_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_add(account_id='bad-id', type='expense', amount=100). \
            Assert: Err with 'Account bad-id not found'."
        )
    }

    /// AC 2.1 edge: transfer without transfer_to_account_id returns InvalidParams.
    #[tokio::test]
    async fn tx_add_transfer_missing_destination() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_add(type='transfer', account_id=valid_id, amount=100, \
            no transfer_to_account_id). \
            Assert: Err with 'transfer_to_account_id is required for transfer transactions'."
        )
    }

    /// AC 2.1 edge: transfer where source == destination returns InvalidParams.
    #[tokio::test]
    async fn tx_add_transfer_same_account() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_add(type='transfer', account_id=X, transfer_to_account_id=X). \
            Assert: Err with 'Cannot transfer to the same account'."
        )
    }

    /// AC 2.1 edge: transfer between different currencies returns ExecutionFailed.
    #[tokio::test]
    async fn tx_add_transfer_cross_currency_error() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account A (VND) + account B (USD). \
            Execute: tx_add(type='transfer', src=A, dst=B, amount=100000). \
            Assert: Err with 'Cross-currency transfers not yet supported'."
        )
    }

    /// AC 2.1 edge: unknown tx type returns InvalidParams.
    #[tokio::test]
    async fn tx_add_invalid_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_add(type='credit'). \
            Assert: Err with 'Invalid transaction type'."
        )
    }

    /// AC 2.1 edge: amount <= 0 returns InvalidParams.
    #[tokio::test]
    async fn tx_add_negative_amount() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_add(amount=-500). \
            Assert: Err with 'Amount must be positive'."
        )
    }

    /// AC 2.1 side effect: response includes budget_impact when category matches budget.
    #[tokio::test]
    async fn tx_add_with_budget_impact() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: active monthly food budget (5000000). \
            Add 3 expense txns of 1000000 each in food category. \
            Execute: tx_add(type='expense', category='food', amount=1000000). \
            Assert: response contains budget_impact section. \
            Assert: response shows spent/limit/percentage (4M/5M = 80%)."
        )
    }

    // --- tx_list ---

    /// AC 2.2 edge: no results → message.
    #[tokio::test]
    async fn tx_list_no_results_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_list with filters that match nothing. \
            Assert: response contains 'No transactions found matching your filters'."
        )
    }

    /// AC 2.2 edge: limit > 100 is capped.
    #[tokio::test]
    async fn tx_list_limit_exceeds_100_is_capped() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: 150 transactions. \
            Execute: tx_list(limit=200). \
            Assert: response contains at most 100 rows."
        )
    }

    // --- tx_update ---

    /// AC 2.3 edge: update nonexistent tx returns ExecutionFailed.
    #[tokio::test]
    async fn tx_update_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_update(id='bad-id'). \
            Assert: Err with 'Transaction bad-id not found'."
        )
    }

    /// AC 2.3 edge: update with no fields returns InvalidParams.
    #[tokio::test]
    async fn tx_update_no_fields() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_update(id=valid_id, no other params). \
            Assert: Err with 'No fields to update'."
        )
    }

    /// AC 2.3: changing tx amount adjusts the account balance by the delta.
    #[tokio::test]
    async fn tx_update_amount_adjusts_balance() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account balance=900000 after expense of 100000. \
            Execute: tx_update(id=tx_id, amount=150000). \
            Expected delta: 150000 - 100000 = 50000 extra subtracted. \
            Assert: account balance becomes 850000."
        )
    }

    // --- tx_delete ---

    /// AC 2.4 edge: delete nonexistent tx returns ExecutionFailed.
    #[tokio::test]
    async fn tx_delete_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_delete(id='bad-id'). \
            Assert: Err with 'Transaction bad-id not found'."
        )
    }

    /// AC 2.4: deleting an expense reverses the balance (adds back).
    #[tokio::test]
    async fn tx_delete_expense_reverses_balance() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account balance=900000 (had expense of 100000). \
            Execute: tx_delete(id=that_expense_tx_id). \
            Assert: account balance becomes 1000000 (reversed)."
        )
    }

    /// AC 2.4 edge: deleting a transfer tx deletes both paired rows and reverses both balances.
    #[tokio::test]
    async fn tx_delete_transfer_deletes_both_rows() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: 2 accounts (A=1M, B=500k). Transfer 200k from A to B. \
            Now: A=800k, B=700k. \
            Execute: tx_delete(id=one_of_the_transfer_tx_ids). \
            Assert: both transfer rows are deleted. \
            Assert: A balance = 1000000 (restored). B balance = 500000 (restored)."
        )
    }

    // --- tx_search ---

    /// AC 2.5 edge: no search criteria returns InvalidParams.
    #[tokio::test]
    async fn tx_search_no_criteria_error() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='tx_search' with no params. \
            Assert: Err with 'At least one search criterion is required'."
        )
    }

    // --- tx_recurring_add ---

    /// AC 2.6 edge: invalid cron expression returns InvalidParams.
    #[tokio::test]
    async fn tx_recurring_add_invalid_cron() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tx_recurring_add(recurring_rule='every monday'). \
            Assert: Err with 'Invalid recurring rule. Use cron format'."
        )
    }

    // ==========================================================================
    // C.3 Budget Actions
    // ==========================================================================

    /// AC 3.1: budget_create with valid params returns confirmation.
    #[tokio::test]
    async fn budget_create_happy_path() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_create(name='Monthly Food', amount=5000000, \
            currency='VND', period='monthly'). \
            Assert: Ok response containing the budget id and name."
        )
    }

    /// AC 3.1 edge: invalid period returns InvalidParams.
    #[tokio::test]
    async fn budget_create_invalid_period() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_create(period='daily'). \
            Assert: Err with 'Invalid period'."
        )
    }

    /// AC 3.1 edge: six_jar method without jar_type returns InvalidParams.
    #[tokio::test]
    async fn budget_create_six_jar_missing_jar_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_create(method='six_jar', no jar_type). \
            Assert: Err with 'jar_type is required for six_jar budgeting'."
        )
    }

    /// AC 3.1 edge: invalid jar_type returns InvalidParams.
    #[tokio::test]
    async fn budget_create_invalid_jar_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_create(method='six_jar', jar_type='health'). \
            Assert: Err with 'Invalid jar type'."
        )
    }

    /// AC 3.1 edge: negative amount returns InvalidParams.
    #[tokio::test]
    async fn budget_create_negative_amount() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_create(amount=-1000). \
            Assert: Err with 'Budget amount must be positive'."
        )
    }

    /// AC 3.1 edge: end_date before start_date returns InvalidParams.
    #[tokio::test]
    async fn budget_create_end_before_start() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_create(start_date='2026-03-01', end_date='2026-01-01'). \
            Assert: Err with 'End date cannot be before start date'."
        )
    }

    /// AC 3.2 edge: no active budgets → message.
    #[tokio::test]
    async fn budget_list_no_budgets_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no active budgets. \
            Execute: action='budget_list'. \
            Assert: response contains 'No active budgets. Create one with budget_create'."
        )
    }

    /// AC 3.3 edge: budget_status with unknown id returns ExecutionFailed.
    #[tokio::test]
    async fn budget_status_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_status(id='bad-id'). \
            Assert: Err with 'Budget bad-id not found'."
        )
    }

    /// AC 3.4 edge: budget_update with unknown id returns ExecutionFailed.
    #[tokio::test]
    async fn budget_update_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_update(id='bad-id', name='X'). \
            Assert: Err with 'Budget bad-id not found'."
        )
    }

    /// AC 3.5 edge: budget_delete with unknown id returns ExecutionFailed.
    #[tokio::test]
    async fn budget_delete_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: budget_delete(id='bad-id'). \
            Assert: Err with 'Budget bad-id not found'."
        )
    }

    // ==========================================================================
    // C.4 Investment Actions
    // ==========================================================================

    /// AC 4.1 edge: portfolio_create with empty name returns InvalidParams.
    #[tokio::test]
    async fn portfolio_create_empty_name() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: portfolio_create(name=''). \
            Assert: Err with 'Portfolio name is required'."
        )
    }

    /// AC 4.2 edge: no portfolios → message.
    #[tokio::test]
    async fn portfolio_list_no_portfolios_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no portfolios. \
            Execute: action='portfolio_list'. \
            Assert: response contains 'No portfolios found. Create one with portfolio_create'."
        )
    }

    /// AC 4.3 edge: invalid asset_type returns InvalidParams.
    #[tokio::test]
    async fn investment_add_invalid_asset_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: investment_add(asset_type='nft'). \
            Assert: Err with 'Invalid asset type'."
        )
    }

    /// AC 4.3 edge: stock without symbol returns InvalidParams.
    #[tokio::test]
    async fn investment_add_stock_without_symbol() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: investment_add(asset_type='stock', no symbol). \
            Assert: Err with 'Symbol is required for stock investments'."
        )
    }

    /// AC 4.3 edge: quantity <= 0 returns InvalidParams.
    #[tokio::test]
    async fn investment_add_negative_quantity() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: investment_add(quantity=-5). \
            Assert: Err with 'Quantity must be positive'."
        )
    }

    /// AC 4.3 edge: unknown portfolio_id returns ExecutionFailed.
    #[tokio::test]
    async fn investment_add_portfolio_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: investment_add(portfolio_id='bad-id'). \
            Assert: Err with 'Portfolio not found'."
        )
    }

    /// AC 4.4 edge: investment_update with unknown id returns ExecutionFailed.
    #[tokio::test]
    async fn investment_update_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: investment_update(id='bad-id', current_price=50000). \
            Assert: Err with 'Investment not found'."
        )
    }

    /// AC 4.4: when current_price set but not current_value, auto-compute value.
    #[tokio::test]
    async fn investment_update_price_auto_computes_value() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: investment with quantity=100. \
            Execute: investment_update(id=inv_id, current_price=50000). \
            Assert: Ok response. \
            Assert: DB investment.current_value == 50000 * 100 = 5000000."
        )
    }

    /// AC 4.5 buy: buying increases quantity and cost_basis.
    #[tokio::test]
    async fn investment_tx_buy_increases_quantity() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: investment with quantity=50, cost_basis=500000. \
            Execute: investment_tx(type='buy', quantity=10, total_amount=150000). \
            Assert: DB investment.quantity == 60. \
            Assert: DB investment.cost_basis == 650000."
        )
    }

    /// AC 4.5 sell: selling decreases quantity and reduces cost_basis proportionally.
    #[tokio::test]
    async fn investment_tx_sell_decreases_quantity() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: investment with quantity=100, cost_basis=1000000. \
            Execute: investment_tx(type='sell', quantity=50, total_amount=600000). \
            Assert: DB investment.quantity == 50. \
            Assert: DB investment.cost_basis == 500000 (proportional: 1M * (100-50)/100)."
        )
    }

    /// AC 4.5 edge: sell more than available quantity returns ExecutionFailed.
    #[tokio::test]
    async fn investment_tx_sell_exceeds_quantity() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: investment with quantity=10. \
            Execute: investment_tx(type='sell', quantity=20). \
            Assert: Err with 'Cannot sell more than current holding (10 available)'."
        )
    }

    /// AC 4.5 dividend: no quantity or cost_basis change, just records the tx.
    #[tokio::test]
    async fn investment_tx_dividend_no_quantity_change() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: investment with quantity=100, cost_basis=1000000. \
            Execute: investment_tx(type='dividend', total_amount=50000). \
            Assert: DB investment.quantity == 100 (unchanged). \
            Assert: DB investment.cost_basis == 1000000 (unchanged). \
            Assert: 1 investment_tx row created with type='dividend'."
        )
    }

    /// AC 4.6 edge: investment_summary with unknown portfolio_id returns ExecutionFailed.
    #[tokio::test]
    async fn investment_summary_portfolio_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: investment_summary(portfolio_id='bad-id'). \
            Assert: Err with 'Portfolio not found'."
        )
    }

    /// AC 4.7 edge: price_fetch for real_estate returns ExecutionFailed.
    #[tokio::test]
    async fn price_fetch_real_estate_error() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: price_fetch(symbol='property-123', asset_type='real_estate'). \
            Assert: Err with 'Price fetch is not available for real estate'."
        )
    }

    // ==========================================================================
    // C.5 Net Worth & Liability Actions
    // ==========================================================================

    /// AC 5.1 edge: invalid liability type returns InvalidParams.
    #[tokio::test]
    async fn liability_add_invalid_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: liability_add(type='auto_loan'). \
            Assert: Err with 'Invalid liability type'."
        )
    }

    /// AC 5.1 edge: non-positive principal returns InvalidParams.
    #[tokio::test]
    async fn liability_add_negative_principal() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: liability_add(principal=0, remaining=0). \
            Assert: Err with 'Amounts must be positive'."
        )
    }

    /// AC 5.1: remaining > principal is allowed (interest capitalized).
    #[tokio::test]
    async fn liability_add_remaining_exceeds_principal_allowed() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: liability_add(principal=100000, remaining=105000, type='credit_card'). \
            Assert: Ok response. No error."
        )
    }

    /// AC 5.2 edge: no liabilities → message.
    #[tokio::test]
    async fn liability_list_empty_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no liabilities. \
            Execute: action='liability_list'. \
            Assert: response contains 'No liabilities recorded'."
        )
    }

    /// AC 5.3 edge: update unknown liability returns ExecutionFailed.
    #[tokio::test]
    async fn liability_update_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: liability_update(id='bad-id', remaining=100). \
            Assert: Err with 'Liability not found'."
        )
    }

    /// AC 5.4 edge: no financial data → message.
    #[tokio::test]
    async fn net_worth_no_data_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no accounts, investments, or liabilities. \
            Execute: action='net_worth'. \
            Assert: response contains 'No financial data recorded yet'."
        )
    }

    /// AC 5.4: net worth with single currency — correct calculation.
    #[tokio::test]
    async fn net_worth_single_currency() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: account balance=15000000 VND + investment value=62000000 VND \
            + liability remaining=5000000 VND. \
            Execute: action='net_worth'. \
            Assert: response shows assets=77000000, liabilities=5000000, net_worth=72000000."
        )
    }

    /// AC 5.4 edge: multi-currency without target param — show per-currency totals.
    #[tokio::test]
    async fn net_worth_multicurrency_no_param() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: 1 VND account + 1 USD account. \
            Execute: net_worth with no currency param. \
            Assert: response shows VND and USD totals separately. \
            Assert: response does NOT attempt conversion."
        )
    }

    // ==========================================================================
    // C.6 Goal & FIRE Actions
    // ==========================================================================

    /// AC 6.1 edge: invalid goal_type returns InvalidParams.
    #[tokio::test]
    async fn goal_create_invalid_type() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: goal_create(goal_type='retire_early'). \
            Assert: Err(InvalidParams)."
        )
    }

    /// AC 6.1 edge: zero target_amount returns InvalidParams.
    #[tokio::test]
    async fn goal_create_negative_amount() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: goal_create(target_amount=0). \
            Assert: Err with 'Target amount must be positive'."
        )
    }

    /// AC 6.1 edge: past deadline returns InvalidParams.
    #[tokio::test]
    async fn goal_create_past_deadline() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: goal_create(deadline='2020-01-01'). \
            Assert: Err with 'Deadline must be in the future'."
        )
    }

    /// AC 6.2 edge: no goals → message.
    #[tokio::test]
    async fn goal_list_empty_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no goals. \
            Execute: action='goal_list'. \
            Assert: response contains 'No financial goals set'."
        )
    }

    /// AC 6.3 edge: update unknown goal returns ExecutionFailed.
    #[tokio::test]
    async fn goal_update_not_found() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: goal_update(id='bad-id'). \
            Assert: Err with 'Goal not found'."
        )
    }

    /// AC 6.3 edge: update with invalid status returns InvalidParams.
    #[tokio::test]
    async fn goal_update_invalid_status() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: goal_update(id=valid_id, status='paused'). \
            Assert: Err(InvalidParams) mentioning invalid status."
        )
    }

    /// AC 6.4 edge: no transaction history requires explicit annual_expenses.
    #[tokio::test]
    async fn goal_fire_no_data_requires_expenses_param() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no transaction history, no annual_expenses param. \
            Execute: action='goal_fire'. \
            Assert: Err with 'No transaction history. Please provide annual_expenses'."
        )
    }

    /// AC 6.4 edge: user is already at FIRE number.
    #[tokio::test]
    async fn goal_fire_already_at_fire_number() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: net worth >> FIRE number (e.g., annual_expenses=10M, net_worth=1B). \
            Execute: goal_fire(annual_expenses=10000000). \
            Assert: response contains 'Congratulations! Your net worth exceeds your FIRE number'."
        )
    }

    /// AC 6.4 edge: negative savings rate → message about not achievable.
    #[tokio::test]
    async fn goal_fire_negative_savings_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: monthly expenses > monthly income. \
            Execute: goal_fire(annual_expenses=...). \
            Assert: response contains 'FIRE is not achievable without increasing income'."
        )
    }

    /// AC 6.5 edge: no params returns InvalidParams.
    #[tokio::test]
    async fn goal_whatif_no_params_error() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='goal_whatif' with no scenario params. \
            Assert: Err with 'Provide at least one scenario parameter'."
        )
    }

    // ==========================================================================
    // C.7 Report Actions
    // ==========================================================================

    /// AC 7.1 edge: custom period without date params returns InvalidParams.
    #[tokio::test]
    async fn report_spending_custom_missing_dates() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: report_spending(period='custom', no date_from/date_to). \
            Assert: Err with 'date_from and date_to are required for custom period'."
        )
    }

    /// AC 7.1 edge: no expenses in period → message.
    #[tokio::test]
    async fn report_spending_no_expenses_message() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: no expense transactions in the current month. \
            Execute: report_spending(period='month'). \
            Assert: response contains 'No spending recorded for this period'."
        )
    }

    /// AC 7.1: monthly spending report returns table format.
    #[tokio::test]
    async fn report_spending_month_table_format() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: expenses in Feb 2026 across 3 categories. \
            Execute: report_spending(period='month'). \
            Assert: response contains '| Category |' table header. \
            Assert: response contains the expense categories."
        )
    }

    /// AC 7.3 edge: invalid metric returns InvalidParams.
    #[tokio::test]
    async fn report_trends_invalid_metric() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: report_trends(metric='balance'). \
            Assert: Err(InvalidParams)."
        )
    }

    /// AC 7.3 edge: insufficient data → note.
    #[tokio::test]
    async fn report_trends_insufficient_data() {
        todo!(
            "Implement after FinanceTool is created. \
            Setup: only 1 month of data. \
            Execute: report_trends(metric='spending'). \
            Assert: response contains 'Not enough data for trend analysis'."
        )
    }

    /// AC 7.4 edge: invalid interval returns InvalidParams.
    #[tokio::test]
    async fn report_net_worth_history_invalid_interval() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: report_net_worth_history(interval='hourly'). \
            Assert: Err(InvalidParams)."
        )
    }

    /// AC 7.4 edge: date_from > date_to returns InvalidParams.
    #[tokio::test]
    async fn report_net_worth_history_date_from_after_to() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: report_net_worth_history(date_from='2026-12-01', date_to='2026-01-01'). \
            Assert: Err(InvalidParams)."
        )
    }

    /// Dispatch: unknown action returns InvalidParams.
    #[tokio::test]
    async fn report_unknown_action_error() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: tool.execute(json!({action: 'explode'}), &ctx).await. \
            Assert: Err with 'Unknown finance action: explode'."
        )
    }

    // ==========================================================================
    // C.8 Settings Actions
    // ==========================================================================

    /// AC 8.1: settings_get returns formatted config.
    #[tokio::test]
    async fn settings_get_returns_formatted_config() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='settings_get'. \
            Assert: response contains 'Default currency:'. \
            Assert: response contains 'Proactivity level:'. \
            Assert: response contains 'Inflation rate:'."
        )
    }

    /// AC 8.2 edge: invalid proactivity_level returns InvalidParams.
    #[tokio::test]
    async fn settings_update_invalid_proactivity() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: settings_update(proactivity_level='always'). \
            Assert: Err with 'Invalid proactivity level'."
        )
    }

    /// AC 8.2 edge: confidence_threshold out of range returns InvalidParams.
    #[tokio::test]
    async fn settings_update_confidence_out_of_range() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: settings_update(confidence_threshold=1.5). \
            Assert: Err with 'Confidence threshold must be between 0.0 and 1.0'."
        )
    }

    /// AC 8.2 edge: no settings to update returns InvalidParams.
    #[tokio::test]
    async fn settings_update_no_params_error() {
        todo!(
            "Implement after FinanceTool is created. \
            Execute: action='settings_update' with no params. \
            Assert: Err with 'No settings to update'."
        )
    }
}
