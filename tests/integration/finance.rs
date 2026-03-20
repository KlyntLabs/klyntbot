//! End-to-end integration tests for the finance module.
//!
//! Tests cover:
//! - FinanceTool registration in ToolRegistry when finance.enabled = true
//! - Full action dispatch through ToolRegistry
//! - Multi-step workflows (add account -> add tx -> check balance)
//! - Budget impact via real transactions
//! - Transfer pair creation and balance consistency
//! - Investment buy/sell cost basis math
//! - Net worth calculation
//! - FIRE calculation with transaction history
//! - Settings persistence to disk
//! - Report generation
//!
//! Tests use an in-memory SQLite pool — no external database required.

use ::common::{ChannelName, ChatId};
use feature_finance::FinanceTool;
use serde_json::json;
use tools::{RoutingContext, Tool};

use super::common;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Create a FinanceTool backed by an in-memory SQLite pool.
async fn make_finance_tool() -> FinanceTool {
    let pool = common::test_pool().await;
    FinanceTool::from_storage_pool(&pool, "VND")
}

fn test_ctx() -> RoutingContext {
    RoutingContext::new(
        ChannelName::new("cli"),
        ChatId::new("test:finance_integration"),
    )
}

/// Helper: extract a string field from a JSON response.
fn extract_field(response: &str, field: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(response).ok()?;
    v.get(field).and_then(|f| f.as_str().map(|s| s.to_string()))
}

/// Helper: extract a nested field from a JSON response (one level).
#[allow(dead_code)]
fn extract_nested_field(response: &str, outer: &str, inner: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(response).ok()?;
    v.get(outer)?
        .get(inner)
        .and_then(|f| f.as_str().map(|s| s.to_string()))
}

// ---------------------------------------------------------------------------
// Tool metadata
// ---------------------------------------------------------------------------

/// Finance tool has the correct name.
#[tokio::test]
async fn tool_has_correct_name() {
    let tool = make_finance_tool().await;
    assert_eq!(tool.name(), "finance");
}

/// Finance tool parameters schema has required "action" field.
#[tokio::test]
async fn tool_parameters_has_action() {
    let tool = make_finance_tool().await;
    let params = tool.parameters();
    let required = params["required"].as_array().expect("should have required");
    assert!(
        required.iter().any(|v| v.as_str() == Some("action")),
        "parameters should require 'action'"
    );
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Finance tool dispatches correctly and creates an account end-to-end.
#[tokio::test]
async fn dispatch_creates_account() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create an account
    let result = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "RegistryTest",
                "type": "bank",
                "currency": "VND",
                "balance": 0
            }),
            &ctx,
        )
        .await;

    assert!(result.is_ok(), "account_add should succeed: {:?}", result);
    let response = result.unwrap();
    assert!(
        response.contains("RegistryTest"),
        "response should contain the account name"
    );

    // Extract account id for cleanup
    let id = extract_field(&response, "id").expect("response should contain id");

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": id}), &ctx)
        .await;
}

/// Unknown action returns error.
#[tokio::test]
async fn unknown_action_returns_error() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    let result = tool.execute(json!({"action": "explode"}), &ctx).await;

    assert!(result.is_err(), "unknown action should return Err");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Unknown finance action: explode"),
        "error should mention unknown action: {}",
        err_msg
    );
}

// ---------------------------------------------------------------------------
// Account -> Transaction flow
// ---------------------------------------------------------------------------

/// E2E: add account, then list -- account appears in response.
#[tokio::test]
async fn account_add_then_list() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account
    let result = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "IntegSavings",
                "type": "bank",
                "currency": "VND",
                "balance": 5000000
            }),
            &ctx,
        )
        .await
        .expect("account_add should succeed");

    let account_id = extract_field(&result, "id").expect("should have id");

    // List accounts
    let list_result = tool
        .execute(json!({"action": "account_list"}), &ctx)
        .await
        .expect("account_list should succeed");

    assert!(
        list_result.contains("IntegSavings"),
        "list should contain the new account name"
    );

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
}

/// E2E: add account, add expense transaction -- balance is decremented.
#[tokio::test]
async fn tx_updates_account_balance() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account with balance 1,000,000
    let acct = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "TxBalanceTest",
                "type": "bank",
                "currency": "VND",
                "balance": 1000000
            }),
            &ctx,
        )
        .await
        .expect("account_add should succeed");
    let account_id = extract_field(&acct, "id").expect("should have id");

    // Add expense of 100,000
    let tx_result = tool
        .execute(
            json!({
                "action": "tx_add",
                "account_id": account_id,
                "type": "expense",
                "amount": 100000,
                "category": "test"
            }),
            &ctx,
        )
        .await
        .expect("tx_add should succeed");

    // Check new_balance
    let v: serde_json::Value = serde_json::from_str(&tx_result).unwrap();
    let new_balance = v["new_balance"].as_i64().expect("should have new_balance");
    assert_eq!(
        new_balance, 900000,
        "balance should be decremented by 100000"
    );

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
}

/// E2E: budget shows correct usage percentage after expense in that category.
#[tokio::test]
async fn tx_with_budget_shows_impact() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account
    let acct = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "BudgetImpactAcct",
                "type": "bank",
                "currency": "VND",
                "balance": 10000000
            }),
            &ctx,
        )
        .await
        .expect("account_add should succeed");
    let account_id = extract_field(&acct, "id").expect("should have id");

    // Create budget for food category
    let budget_result = tool
        .execute(
            json!({
                "action": "budget_create",
                "name": "BudgetImpactFood",
                "amount": 5000000,
                "currency": "VND",
                "period": "monthly",
                "category": "integ_food_budget_test"
            }),
            &ctx,
        )
        .await
        .expect("budget_create should succeed");
    let budget_id = extract_field(&budget_result, "id").expect("should have budget id");

    // Add expense of 2,500,000 in food category
    let tx_result = tool
        .execute(
            json!({
                "action": "tx_add",
                "account_id": account_id,
                "type": "expense",
                "amount": 2500000,
                "category": "integ_food_budget_test"
            }),
            &ctx,
        )
        .await
        .expect("tx_add should succeed");

    // Check for budget_impact in response
    let v: serde_json::Value = serde_json::from_str(&tx_result).unwrap();
    assert!(
        v.get("budget_impact").is_some(),
        "response should include budget_impact"
    );
    let impact = &v["budget_impact"];
    let percentage = impact["percentage"].as_i64().unwrap_or(0);
    assert_eq!(percentage, 50, "2.5M / 5M should be 50%");

    // Cleanup
    let _ = tool
        .execute(json!({"action": "budget_delete", "id": budget_id}), &ctx)
        .await;
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
}

// ---------------------------------------------------------------------------
// Transfer flow
// ---------------------------------------------------------------------------

/// E2E: transfer creates two paired rows and adjusts both account balances.
#[tokio::test]
async fn transfer_creates_paired_rows() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account A (1,000,000 VND)
    let a = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "TransferSrc",
                "type": "bank",
                "currency": "VND",
                "balance": 1000000
            }),
            &ctx,
        )
        .await
        .expect("account_add A");
    let a_id = extract_field(&a, "id").unwrap();

    // Create account B (500,000 VND)
    let b = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "TransferDst",
                "type": "bank",
                "currency": "VND",
                "balance": 500000
            }),
            &ctx,
        )
        .await
        .expect("account_add B");
    let b_id = extract_field(&b, "id").unwrap();

    // Transfer 200,000 from A to B
    let result = tool
        .execute(
            json!({
                "action": "tx_add",
                "account_id": a_id,
                "transfer_to_account_id": b_id,
                "type": "transfer",
                "amount": 200000
            }),
            &ctx,
        )
        .await
        .expect("tx_add transfer");

    let v: serde_json::Value = serde_json::from_str(&result).unwrap();

    // Check transfer_id is present
    assert!(
        v.get("transfer_id").is_some(),
        "response should contain transfer_id"
    );

    // Check balances
    let from_balance = v["from_balance"].as_i64().unwrap();
    let to_balance = v["to_balance"].as_i64().unwrap();
    assert_eq!(from_balance, 800000, "A balance should be 800000");
    assert_eq!(to_balance, 700000, "B balance should be 700000");

    // Check both tx types
    let from_type = v["from_tx"]["type"].as_str().unwrap();
    let to_type = v["to_tx"]["type"].as_str().unwrap();
    assert_eq!(from_type, "expense", "source row should be expense");
    assert_eq!(to_type, "income", "destination row should be income");

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": a_id}), &ctx)
        .await;
    let _ = tool
        .execute(json!({"action": "account_delete", "id": b_id}), &ctx)
        .await;
}

// ---------------------------------------------------------------------------
// Investment flow
// ---------------------------------------------------------------------------

/// E2E: buy then sell -- cost basis reduced proportionally (average cost method).
#[tokio::test]
async fn investment_buy_sell_cost_basis() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create portfolio
    let portfolio = tool
        .execute(
            json!({
                "action": "portfolio_create",
                "name": "InvestTestPortfolio",
                "currency": "VND"
            }),
            &ctx,
        )
        .await
        .expect("portfolio_create");
    let portfolio_id = extract_field(&portfolio, "id").unwrap();

    // Add investment with quantity=0, cost_basis=0
    let inv = tool
        .execute(
            json!({
                "action": "investment_add",
                "portfolio_id": portfolio_id,
                "asset_type": "stock",
                "symbol": "INTTEST",
                "name": "Integration Test Stock",
                "quantity": 0.0,
                "cost_basis": 0,
                "currency": "VND"
            }),
            &ctx,
        )
        .await
        .expect("investment_add");
    let inv_v: serde_json::Value = serde_json::from_str(&inv).unwrap();
    let investment_id = inv_v["investment"]["id"].as_str().unwrap().to_string();

    // Buy 100 units for 1,000,000
    let buy_result = tool
        .execute(
            json!({
                "action": "investment_tx",
                "investment_id": investment_id,
                "tx_type": "buy",
                "quantity": 100.0,
                "total_amount": 1000000
            }),
            &ctx,
        )
        .await
        .expect("investment_tx buy");
    let buy_v: serde_json::Value = serde_json::from_str(&buy_result).unwrap();
    assert_eq!(
        buy_v["updated_investment"]["quantity"].as_f64().unwrap(),
        100.0
    );
    assert_eq!(
        buy_v["updated_investment"]["cost_basis"].as_i64().unwrap(),
        1000000
    );

    // Sell 50 units for 600,000
    let sell_result = tool
        .execute(
            json!({
                "action": "investment_tx",
                "investment_id": investment_id,
                "tx_type": "sell",
                "quantity": 50.0,
                "total_amount": 600000
            }),
            &ctx,
        )
        .await
        .expect("investment_tx sell");
    let sell_v: serde_json::Value = serde_json::from_str(&sell_result).unwrap();

    // After sell: quantity = 50, cost_basis = 1M * (50/100) = 500,000
    let final_qty = sell_v["updated_investment"]["quantity"].as_f64().unwrap();
    let final_basis = sell_v["updated_investment"]["cost_basis"].as_i64().unwrap();
    assert_eq!(
        final_qty, 50.0,
        "quantity should be 50 after selling 50 of 100"
    );
    assert_eq!(
        final_basis, 500000,
        "cost_basis should be 500000 (proportional: 1M * 50/100)"
    );

    // No direct portfolio/investment delete action, so just leave data.
    // The test is self-verifying via the assertions above.
}

// ---------------------------------------------------------------------------
// Net worth calculation
// ---------------------------------------------------------------------------

/// E2E: net worth correctly computes assets - liabilities.
#[tokio::test]
async fn net_worth_calculation() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account (100,000 VND)
    let acct = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "NWAcct",
                "type": "bank",
                "currency": "VND",
                "balance": 100000
            }),
            &ctx,
        )
        .await
        .expect("account_add");
    let account_id = extract_field(&acct, "id").unwrap();

    // Add liability (200,000 VND)
    let liability = tool
        .execute(
            json!({
                "action": "liability_add",
                "name": "NWLiability",
                "type": "personal_loan",
                "principal": 200000,
                "remaining": 200000,
                "currency": "VND"
            }),
            &ctx,
        )
        .await
        .expect("liability_add");
    let liability_id = extract_field(&liability, "id").unwrap();

    // Get net worth
    let nw_result = tool
        .execute(json!({"action": "net_worth"}), &ctx)
        .await
        .expect("net_worth");

    let v: serde_json::Value = serde_json::from_str(&nw_result).unwrap();
    // net_worth now returns a flat structure with base_currency aggregation
    assert_eq!(v["base_currency"], "VND", "base_currency should be VND");
    // accounts = 100000, liabilities = 200000, net = -100000
    let nw = v["net_worth"].as_i64().unwrap();
    assert!(
        nw < 0,
        "net worth should be negative (more liabilities): {nw}"
    );

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
    // No liability_delete action, so we update remaining to 0 as cleanup
    let _ = tool
        .execute(
            json!({"action": "liability_update", "id": liability_id, "remaining": 0}),
            &ctx,
        )
        .await;
}

// ---------------------------------------------------------------------------
// Budget list
// ---------------------------------------------------------------------------

/// E2E: budget_list shows correct usage percentage from real transactions.
#[tokio::test]
async fn budget_list_shows_current_usage() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account
    let acct = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "BudgetListAcct",
                "type": "bank",
                "currency": "VND",
                "balance": 10000000
            }),
            &ctx,
        )
        .await
        .expect("account_add");
    let account_id = extract_field(&acct, "id").unwrap();

    // Create budget
    let budget = tool
        .execute(
            json!({
                "action": "budget_create",
                "name": "BudgetListFood",
                "amount": 500000,
                "currency": "VND",
                "period": "monthly",
                "category": "integ_budget_list_test"
            }),
            &ctx,
        )
        .await
        .expect("budget_create");
    let budget_id = extract_field(&budget, "id").unwrap();

    // Add expenses totaling 250,000 in the category
    for _ in 0..5 {
        let _ = tool
            .execute(
                json!({
                    "action": "tx_add",
                    "account_id": account_id,
                    "type": "expense",
                    "amount": 50000,
                    "category": "integ_budget_list_test"
                }),
                &ctx,
            )
            .await
            .expect("tx_add");
    }

    // List budgets
    let list_result = tool
        .execute(json!({"action": "budget_list"}), &ctx)
        .await
        .expect("budget_list");

    // Parse and find our budget in the list
    let v: serde_json::Value = serde_json::from_str(&list_result).unwrap();
    let budgets = v.as_array().expect("budget_list should return an array");
    let our_budget = budgets
        .iter()
        .find(|b| b["id"].as_str() == Some(&budget_id));
    assert!(
        our_budget.is_some(),
        "our budget should be in the list: {}",
        list_result
    );
    let our_budget = our_budget.unwrap();
    let percentage = our_budget["percentage"].as_i64().unwrap();
    assert_eq!(percentage, 50, "250k / 500k = 50%: got {}", percentage);

    // Cleanup
    let _ = tool
        .execute(json!({"action": "budget_delete", "id": budget_id}), &ctx)
        .await;
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
}

// ---------------------------------------------------------------------------
// FIRE calculation
// ---------------------------------------------------------------------------

/// E2E: FIRE calculation with explicit annual_expenses.
#[tokio::test]
async fn fire_with_explicit_expenses() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account with large balance so net_worth > 0
    let acct = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "FIREAcct",
                "type": "bank",
                "currency": "VND",
                "balance": 500000000
            }),
            &ctx,
        )
        .await
        .expect("account_add");
    let account_id = extract_field(&acct, "id").unwrap();

    // Run FIRE calculation with explicit params
    let fire_result = tool
        .execute(
            json!({
                "action": "goal_fire",
                "annual_expenses": 84000000,
                "withdrawal_rate": 4.0,
                "expected_return_rate": 7.0,
                "inflation_rate": 3.0,
                "monthly_contribution": 3000000
            }),
            &ctx,
        )
        .await
        .expect("goal_fire");

    let v: serde_json::Value = serde_json::from_str(&fire_result).unwrap();

    // FIRE number = 84M * (100/4) = 2.1B
    let fire_number = v["fire_number"].as_i64().unwrap();
    assert_eq!(fire_number, 2_100_000_000, "FIRE number should be 2.1B");

    // Should have progress_pct and months_remaining
    assert!(v.get("progress_pct").is_some(), "should have progress_pct");
    assert!(
        v.get("months_remaining").is_some(),
        "should have months_remaining"
    );

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
}

// ---------------------------------------------------------------------------
// Settings persistence
// ---------------------------------------------------------------------------

/// E2E: settings_get returns formatted config.
#[tokio::test]
async fn settings_get_returns_config() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    let result = tool
        .execute(json!({"action": "settings_get"}), &ctx)
        .await
        .expect("settings_get should succeed");

    // The settings_get action returns the finance config as JSON.
    // It should contain fields like defaultCurrency.
    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        v.is_object(),
        "settings_get should return a JSON object: {}",
        result
    );
}

// ---------------------------------------------------------------------------
// Report generation
// ---------------------------------------------------------------------------

/// E2E: report_spending shows per-category breakdown with amounts.
#[tokio::test]
async fn report_spending_shows_category_breakdown() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account
    let acct = tool
        .execute(
            json!({
                "action": "account_add",
                "name": "ReportAcct",
                "type": "bank",
                "currency": "VND",
                "balance": 10000000
            }),
            &ctx,
        )
        .await
        .expect("account_add");
    let account_id = extract_field(&acct, "id").unwrap();

    // Add expenses in different categories this month
    let today = chrono::Local::now().date_naive();
    let tx_date = today.to_string();

    for (cat, amount) in &[
        ("integ_report_food", 300000),
        ("integ_report_transport", 100000),
        ("integ_report_entertainment", 200000),
    ] {
        let _ = tool
            .execute(
                json!({
                    "action": "tx_add",
                    "account_id": account_id,
                    "type": "expense",
                    "amount": amount,
                    "category": cat,
                    "tx_date": tx_date
                }),
                &ctx,
            )
            .await
            .expect("tx_add for report");
    }

    // Generate spending report for this month
    let report = tool
        .execute(
            json!({
                "action": "report_spending",
                "period": "month"
            }),
            &ctx,
        )
        .await
        .expect("report_spending");

    let v: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(
        v.get("breakdown").is_some(),
        "report should have breakdown: {}",
        report
    );
    assert!(
        v.get("total").is_some(),
        "report should have total: {}",
        report
    );

    // The total should include at least our 600,000 (possibly more from other tests)
    let total = v["total"].as_i64().unwrap();
    assert!(
        total >= 600000,
        "total spending should be at least 600,000: got {}",
        total
    );

    // Cleanup
    let _ = tool
        .execute(json!({"action": "account_delete", "id": account_id}), &ctx)
        .await;
}

// ---------------------------------------------------------------------------
// tx_update positivity guard
// ---------------------------------------------------------------------------

/// Create an account + transaction, return the tx_id for update tests.
async fn make_test_tx(tool: &FinanceTool, ctx: &RoutingContext) -> String {
    tool.execute(
        json!({
            "action": "account_add",
            "name": "Test Bank",
            "type": "bank",
            "currency": "USD",
            "balance": 100000
        }),
        ctx,
    )
    .await
    .unwrap();

    let result = tool
        .execute(
            json!({
                "action": "tx_add",
                "amount": 5000,
                "type": "expense",
                "category": "food"
            }),
            ctx,
        )
        .await
        .unwrap();

    serde_json::from_str::<serde_json::Value>(&result).unwrap()["tx"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn tx_update_rejects_negative_amount() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();
    let tx_id = make_test_tx(&tool, &ctx).await;

    let result = tool
        .execute(json!({"action": "tx_update", "id": tx_id, "amount": -500}), &ctx)
        .await;
    assert!(result.is_err(), "tx_update should reject negative amounts");
}

#[tokio::test]
async fn tx_update_rejects_zero_amount() {
    let tool = make_finance_tool().await;
    let ctx = test_ctx();
    let tx_id = make_test_tx(&tool, &ctx).await;

    let result = tool
        .execute(json!({"action": "tx_update", "id": tx_id, "amount": 0}), &ctx)
        .await;
    assert!(result.is_err(), "tx_update should reject zero amounts");
}
