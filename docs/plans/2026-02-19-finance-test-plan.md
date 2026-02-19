# Finance Module — Comprehensive Test Plan

**Date:** 2026-02-19
**Status:** Approved — TDD skeleton files created
**References:**
- Acceptance criteria: `docs/plans/2026-02-19-finance-acceptance-criteria.md`
- Architecture: `~/.claude/plans/concurrent-plotting-thimble.md`
- Original design: `docs/plans/2026-02-19-ai-first-personal-finance-design.md`

---

## Coverage Summary

| Layer | Tests | Files |
|-------|-------|-------|
| A. Storage repos (6) | 78 tests | `crates/storage/src/repos/tests/finance_*_tests.rs` |
| B. Domain types | 34 tests | `crates/tools/src/finance_types_tests.rs` |
| C. Tool actions (37) | 97 tests | `crates/tools/src/finance_tool/tests.rs` |
| D. PriceService | 11 tests + 1 `#[ignore]` | `crates/tools/src/price_service.rs` (inline) |
| E. FinanceHandler | 8 tests | `crates/agent/src/finance_adapter.rs` (inline) |
| F. Integration | 14 tests | `tests/finance_integration_tests.rs` |
| **Total** | **243 tests** | |

Every acceptance criterion in `2026-02-19-finance-acceptance-criteria.md` maps to at least one test. Edge cases are explicitly labeled.

---

## A. Storage Layer Tests

Location: `crates/storage/src/repos/tests/finance_*_tests.rs`
Pattern: `async fn test_finance_account_repo() -> Option<FinanceAccountRepo>` helper, graceful skip when no DB.
Naming: `{repo}_{operation}_{scenario}`

### A.1 FinanceAccountRepo (14 tests)

#### CRUD

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `account_add_and_get` | AC 1.1 happy path | Insert a `FinanceAccountRow` with valid fields | Row returned with same id, name, type, is_archived=false |
| `account_get_not_found_returns_none` | AC 1.3 edge: id not found | No rows inserted | `get("nonexistent")` returns `Ok(None)` |
| `account_update_name_and_balance` | AC 1.3 happy path | Insert row, then patch name + balance | Updated row returned, updated_at is newer |
| `account_update_not_found_returns_error` | AC 1.3 edge: id not found | No matching row | `update(patch)` returns `StorageError::NotFound` |
| `account_delete_existing` | AC 1.4 happy path | Insert row | `delete(id)` returns `true`, subsequent `get` returns `None` |
| `account_delete_not_found_returns_false` | AC 1.4 edge: id not found | No matching row | `delete("nonexistent")` returns `false` |

#### Filtering & Listing

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `account_list_excludes_archived_by_default` | AC 1.2 default behavior | 2 active + 1 archived account | `list(include_archived=false)` returns 2 accounts |
| `account_list_includes_archived_when_requested` | AC 1.2 include_archived | 2 active + 1 archived | `list(include_archived=true)` returns 3 accounts |
| `account_list_by_currency_filter` | AC 1.2 currency filter | 2 VND + 1 USD accounts | `list_by_currency("VND")` returns 2 accounts |
| `account_list_empty_returns_empty_vec` | AC 1.2 edge: no accounts | No rows | Returns empty vec, no error |

#### Aggregation

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `account_total_balance_by_currency` | Net worth / AC 5.4 | 2 VND accounts + 1 USD | Returns `[(VND, sum_vnd), (USD, sum_usd)]` |
| `account_total_balance_excludes_archived` | Net worth accuracy | 1 active + 1 archived, different amounts | Archived account NOT included in total |
| `account_adjust_balance_adds_delta` | AC 2.1 balance update | Account with balance=1000 | `adjust_balance(id, 500)` → balance becomes 1500 |
| `account_adjust_balance_allows_negative` | AC 2.1 overdraft | Account with balance=100 | `adjust_balance(id, -500)` → balance becomes -400, no error |

### A.2 FinanceTransactionRepo (20 tests)

#### CRUD

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `tx_add_and_get` | AC 2.1 happy path | Account + valid tx row | Row returned with generated id, correct account_id |
| `tx_get_not_found` | AC 2.3 edge: not found | No matching row | `get("bad-id")` returns `Ok(None)` |
| `tx_update_amount` | AC 2.3 happy path | Insert tx, patch amount | Updated row returned with new amount |
| `tx_delete_returns_deleted_row` | AC 2.4 happy path | Insert tx | `delete(id)` returns `Ok(Some(row))`, subsequent `get` returns `None` |
| `tx_delete_not_found_returns_none` | AC 2.4 edge: not found | No matching row | `delete("bad-id")` returns `Ok(None)` |

#### Filtering with QueryBuilder

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `tx_list_filter_by_account_id` | AC 2.2 filter | 3 txns for acct A, 2 for acct B | Filter by acct A → 3 results |
| `tx_list_filter_by_type_expense` | AC 2.2 type filter | Mix of income/expense | Filter by "expense" → only expenses returned |
| `tx_list_filter_by_date_range` | AC 2.2 date filter | Txns across 3 months | Filter Feb 1-28 → only Feb txns |
| `tx_list_filter_by_category` | AC 2.2 category filter | Txns with different categories | Filter by "food" → only food txns |
| `tx_list_limit_capped_at_100` | AC 2.2 limit | 150 transactions inserted | `list(limit=200)` returns 100 results |
| `tx_list_no_filter_returns_recent` | AC 2.2 default | 25 txns across 2 accounts | Returns 20 most recent (default limit) |

#### Transfer Operations

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `tx_get_by_transfer_id_returns_both_rows` | AC 2.1 transfer | Insert paired rows with same transfer_id | `get_by_transfer_id(tid)` returns exactly 2 rows |
| `tx_get_by_transfer_id_missing_returns_empty` | AC 2.4 transfer delete | No rows with that transfer_id | Returns empty vec |

#### Aggregation

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `tx_sum_by_category` | AC 7.1 spending report | 3 food txns + 2 transport | `sum_by_category(this_month, "expense")` returns correct category sums |
| `tx_sum_by_period_monthly` | AC 7.3 trends | 6 months of expenses | `sum_by_period("expense", 6, "month")` returns 6 period rows |
| `tx_category_history_returns_patterns` | Daily review / auto-categorize | 10 txns with known counterparty/category pairs | Returns counterparty→category mappings |

#### Search

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `tx_search_by_keyword_notes` | AC 2.5 keyword search | Txns with various notes | ILIKE "%vinmart%" matches txn with "Vinmart" in notes |
| `tx_search_by_amount_range` | AC 2.5 amount filter | Txns with amounts 100, 500, 1000 | `amount_min=200, amount_max=800` → only 500 |
| `tx_search_requires_at_least_one_criterion` | AC 2.5 edge | — | Call with no filters → `StorageError` or empty results |

### A.3 FinanceBudgetRepo (14 tests)

#### CRUD

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `budget_add_and_get` | AC 3.1 happy path | Valid budget row | Row returned with correct fields |
| `budget_update_name_and_amount` | AC 3.4 happy path | Insert + patch | Updated name and amount |
| `budget_delete_existing` | AC 3.5 happy path | Insert budget | `delete(id)` returns `true` |
| `budget_delete_not_found` | AC 3.5 edge | No row | Returns `false` |

#### Listing

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `budget_list_active_excludes_inactive` | AC 3.2 list | 2 active + 1 inactive | `list_active()` returns 2 |
| `budget_get_by_category` | AC 2.1 budget impact | Budget with category="food" | `get_by_category("food")` returns the budget |
| `budget_get_by_category_not_found` | AC 2.1 budget impact | No matching budget | Returns `Ok(None)` |

#### Budget Usage SQL Join

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `budget_usage_no_transactions` | AC 3.2 0% used | Budget with no matching txns | `spent = 0`, `percentage = 0` |
| `budget_usage_with_matching_transactions` | AC 3.2 usage calc | Budget for "food", 3 expense txns this period | `spent = sum of those txns` |
| `budget_usage_only_counts_expenses` | AC 3.2 tx_type filter | Food budget + food income txn + food expense | Only expense counted in `spent` |
| `budget_usage_only_counts_current_period` | AC 3.2 period filter | Budget, txns from last month + this month | Only this month's txns in `spent` |
| `budget_usage_category_null_sums_all` | AC 3.2 total budget | Budget with category=NULL | Sums ALL expense txns in period |
| `all_budget_usage_returns_all_active` | AC 3.2 list format | 3 active budgets | Returns 3 usage rows |
| `budget_usage_not_found_returns_error` | AC 3.3 edge: not found | No budget with that id | Returns `StorageError::NotFound` |

### A.4 FinanceInvestmentRepo (16 tests)

#### Portfolio CRUD

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `portfolio_add_and_get` | AC 4.1 happy path | Valid portfolio row | Row with correct id, name |
| `portfolio_list_returns_all` | AC 4.2 list | 3 portfolios | Returns 3 |
| `portfolio_delete` | AC 4.1 related | Insert + delete | Returns `true`, gone from list |

#### Investment CRUD

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `investment_add_and_get` | AC 4.3 happy path | Portfolio + investment row | Investment returned with portfolio_id |
| `investment_update_price` | AC 4.7 price_fetch | Insert investment | `update_price(id, price, value)` updates current_price + current_value |
| `investment_list_with_symbols_excludes_null_symbol` | AC 4.7 batch refresh | 2 with symbol + 1 real_estate (NULL symbol) | `list_with_symbols()` returns 2 |
| `investment_total_value_by_currency` | AC 5.4 net worth | 3 investments in VND | `total_value_by_currency()` returns correct sum |

#### Investment Transactions

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `investment_tx_add_buy` | AC 4.5 buy | Investment + buy tx row | Tx stored with correct fields |
| `investment_tx_list` | AC 4.5 list | 3 tx for one investment | `list_investment_txs(id)` returns 3 |

#### Portfolio Summary

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `portfolio_summary_sums_correct` | AC 4.2 summary | Portfolio with 3 investments | `portfolio_summary(pid)` returns correct totals |
| `portfolio_summary_empty_portfolio` | AC 4.2 edge: empty | Portfolio, no investments | Returns zeros |
| `portfolio_not_found_returns_error` | AC 4.6 edge | Bad portfolio_id | `get_portfolio("bad")` returns `None` |

#### Cascade Delete

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `portfolio_delete_cascades_investments` | AC 1.4 cascade pattern | Portfolio with 2 investments | Delete portfolio → investments also gone |
| `investment_delete_cascades_txs` | Cascade | Investment with 3 txs | Delete investment → txs also gone |

### A.5 FinanceGoalRepo (8 tests)

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `goal_add_and_get` | AC 6.1 happy path | Valid goal row | Row returned with status="active" |
| `goal_list_active` | AC 6.2 list | 2 active + 1 achieved | Returns 2 active |
| `goal_update_current_amount` | AC 6.3 update progress | Insert goal | Patch current_amount, verify updated |
| `goal_update_progress_method` | AC 6.3 | Insert goal | `update_progress(id, amount)` updates current_amount |
| `goal_update_status` | AC 6.3 status | Insert goal | Patch status to "achieved", verify |
| `goal_delete` | Related | Insert + delete | Returns `true` |
| `goal_get_not_found` | AC 6.3 edge | No row | Returns `Ok(None)` |
| `goal_update_not_found` | AC 6.3 edge | Bad id | Returns `StorageError::NotFound` |

### A.6 FinanceLiabilityRepo (6 tests)

| Test Name | Acceptance Criterion | Setup | Expected |
|-----------|---------------------|-------|----------|
| `liability_add_and_get` | AC 5.1 happy path | Valid liability row | Row returned with correct fields |
| `liability_list_all` | AC 5.2 list | 3 liabilities | Returns all 3 |
| `liability_update_remaining` | AC 5.3 update | Insert + patch remaining | Updated value returned |
| `liability_delete` | AC 5.3 | Insert + delete | Returns `true` |
| `liability_total_remaining_by_currency` | AC 5.4 net worth | 2 VND + 1 USD | Correct sums per currency |
| `liability_total_remaining_empty` | AC 5.4 edge | No liabilities | Returns empty vec |

---

## B. Domain Type Tests

Location: `crates/tools/src/finance_types_tests.rs`
Inline test module in `crates/tools/src/finance_types.rs`.

### B.1 Enum `from_str_loose` and `as_str` Round-Trips (11 enums)

| Test Name | Enum | Scenario |
|-----------|------|----------|
| `account_type_roundtrip_all_variants` | `AccountType` | All 6 variants: cash, bank, ewallet, crypto_wallet, brokerage, other |
| `transaction_type_roundtrip` | `TransactionType` | income, expense, transfer |
| `budget_period_roundtrip` | `BudgetPeriod` | monthly, weekly, yearly, custom |
| `budget_method_roundtrip` | `BudgetMethod` | standard, six_jar |
| `jar_type_roundtrip` | `JarType` | All 6 jars |
| `asset_type_roundtrip` | `AssetType` | stock, etf, crypto, real_estate, bond, other |
| `investment_tx_type_roundtrip` | `InvestmentTxType` | buy, sell, dividend, rental_income, interest, split |
| `goal_type_roundtrip` | `GoalType` | savings, purchase, debt_payoff, fire, custom |
| `goal_status_roundtrip` | `GoalStatus` | active, achieved, abandoned |
| `liability_type_roundtrip` | `LiabilityType` | mortgage, credit_card, personal_loan, student_loan, other |
| `enum_from_str_unknown_returns_none` | All enums | `from_str_loose("garbage")` returns `None` |

### B.2 From Implementations (8 domain structs — 11 round-trip tests)

| Test Name | Type | Scenario |
|-----------|------|----------|
| `finance_account_row_to_domain` | `FinanceAccount` | `FinanceAccountRow → FinanceAccount`: enum fields parsed correctly |
| `finance_account_domain_to_row` | `FinanceAccountRow` | `&FinanceAccount → FinanceAccountRow`: enum → string, timestamps preserved |
| `finance_account_roundtrip_preserves_data` | Both | Row → Domain → Row: all fields equal after double conversion |
| `finance_transaction_row_to_domain` | `FinanceTransaction` | transfer_id, tx_type, date all convert correctly |
| `finance_transaction_domain_to_row` | `FinanceTransactionRow` | TransactionType enum serializes to "income"/"expense"/"transfer" |
| `finance_budget_roundtrip` | `FinanceBudget` | method + jar_type Optional\<Enum\> round-trip |
| `finance_portfolio_roundtrip` | `FinancePortfolio` | name + currency preserved through Row → Domain → Row |
| `finance_investment_roundtrip` | `FinanceInvestment` | Optional current_price/current_value preserved |
| `finance_investment_tx_roundtrip` | `FinanceInvestmentTx` | tx_type enum + Optional quantity/price preserved |
| `finance_goal_roundtrip` | `FinanceGoal` | Status enum + Optional fields all preserved |
| `finance_liability_roundtrip` | `FinanceLiability` | interest_rate f64 Optional preserved |

### B.3 Filter Conversion

| Test Name | Filter Type | Scenario |
|-----------|-------------|----------|
| `tx_filter_converts_tx_type_enum` | `FinanceTransactionFilter` | `tx_type: Some(TransactionType::Expense)` → storage filter with `tx_type: Some("expense")` |
| `tx_filter_none_fields_remain_none` | `FinanceTransactionFilter` | Empty filter → all `None` storage filter fields |
| `tx_filter_limit_usize_to_i64` | `FinanceTransactionFilter` | `limit: Some(50)` → `i64: Some(50)` in storage |

---

## C. Tool Action Tests

Location: `crates/tools/src/finance_tool/tests.rs`

Pattern: `FinanceTool::new(...)` with mock repos (or real repos if DB available), dispatch via `execute(json!({"action": "..."}), &ctx)`.

### C.1 Account Actions (12 tests)

#### `account_add`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `account_add_happy_path` | AC 1.1 | Valid name/type/currency/balance | Response contains account id, name, formatted balance |
| `account_add_invalid_type` | AC 1.1 edge: invalid type | `type: "checking"` | `InvalidParams("Invalid account type: checking")` |
| `account_add_empty_currency` | AC 1.1 edge: empty currency | `currency: ""` | `InvalidParams("Currency must be a valid ISO 4217 code")` |
| `account_add_currency_too_long` | AC 1.1 edge: currency > 3 chars | `currency: "USDC"` | `InvalidParams("Currency must be a valid ISO 4217 code")` |
| `account_add_empty_name` | AC 1.1 edge: empty name | `name: ""` | `InvalidParams("Account name is required")` |
| `account_add_negative_balance_allowed` | AC 1.1 edge: overdraft | `balance: -50000` | Success (credit line account) |

#### `account_list`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `account_list_no_accounts_message` | AC 1.2 edge: empty | No accounts | Response contains "No accounts found" |
| `account_list_with_accounts` | AC 1.2 | 2 accounts different currencies | Grouped by currency with totals |

#### `account_update` / `account_delete`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `account_update_not_found` | AC 1.3 edge | Invalid id | `ExecutionFailed("Account ... not found")` |
| `account_update_no_fields` | AC 1.3 edge | id only, no other fields | `InvalidParams("No fields to update")` |
| `account_delete_not_found` | AC 1.4 edge | Invalid id | `ExecutionFailed("Account ... not found")` |
| `account_delete_cascades_transactions` | AC 1.4 | Account with 3 txns | Success, response notes "3 transactions removed" |

### C.2 Transaction Actions (18 tests)

#### `tx_add`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `tx_add_expense_updates_balance` | AC 2.1 | expense, amount=100000 | Account balance decremented, response shows new balance |
| `tx_add_income_updates_balance` | AC 2.1 | income, amount=500000 | Account balance incremented |
| `tx_add_account_not_found` | AC 2.1 edge | Bad account_id | `ExecutionFailed("Account ... not found")` |
| `tx_add_transfer_missing_destination` | AC 2.1 edge | type="transfer", no transfer_to_account_id | `InvalidParams("transfer_to_account_id is required")` |
| `tx_add_transfer_same_account` | AC 2.1 edge | type="transfer", same src+dst | `InvalidParams("Cannot transfer to the same account")` |
| `tx_add_transfer_cross_currency_error` | AC 2.1 edge | Src=VND, dst=USD | `ExecutionFailed("Cross-currency transfers not yet supported")` |
| `tx_add_invalid_type` | AC 2.1 edge | type="credit" | `InvalidParams("Invalid transaction type")` |
| `tx_add_negative_amount` | AC 2.1 edge | amount=-500 | `InvalidParams("Amount must be positive")` |
| `tx_add_with_budget_impact` | AC 2.1 side effect | Expense in category with active budget | Response includes budget_impact with spent/limit/% |

#### `tx_list`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `tx_list_no_results_message` | AC 2.2 edge | Filters that match nothing | "No transactions found matching your filters" |
| `tx_list_limit_exceeds_100_is_capped` | AC 2.2 edge | limit=200 | Returns at most 100 results |

#### `tx_update` / `tx_delete`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `tx_update_not_found` | AC 2.3 edge | Bad id | `ExecutionFailed("Transaction ... not found")` |
| `tx_update_no_fields` | AC 2.3 edge | id only | `InvalidParams("No fields to update")` |
| `tx_update_amount_adjusts_balance` | AC 2.3 | Change expense from 100 to 150 | Account balance adjusted by delta (-50 more) |
| `tx_delete_not_found` | AC 2.4 edge | Bad id | `ExecutionFailed("Transaction ... not found")` |
| `tx_delete_expense_reverses_balance` | AC 2.4 | Delete expense 100000 | Account balance += 100000 |
| `tx_delete_transfer_deletes_both_rows` | AC 2.4 edge: transfer | Transfer tx (has transfer_id) | Both paired rows deleted, both balances reversed |

#### `tx_search` / `tx_recurring_add`

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `tx_search_no_criteria_error` | AC 2.5 edge | No params | `InvalidParams("At least one search criterion is required")` |
| `tx_recurring_add_invalid_cron` | AC 2.6 edge | recurring_rule="every day" | `InvalidParams("Invalid recurring rule")` |

### C.3 Budget Actions (10 tests)

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `budget_create_happy_path` | AC 3.1 | Valid name/amount/currency/period | Response confirms budget created with id |
| `budget_create_invalid_period` | AC 3.1 edge | period="daily" | `InvalidParams("Invalid period")` |
| `budget_create_six_jar_missing_jar_type` | AC 3.1 edge | method="six_jar", no jar_type | `InvalidParams("jar_type is required")` |
| `budget_create_invalid_jar_type` | AC 3.1 edge | jar_type="health" | `InvalidParams("Invalid jar type")` |
| `budget_create_negative_amount` | AC 3.1 edge | amount=-1000 | `InvalidParams("Budget amount must be positive")` |
| `budget_create_end_before_start` | AC 3.1 edge | end_date < start_date | `InvalidParams("End date cannot be before start date")` |
| `budget_list_no_budgets_message` | AC 3.2 edge | No active budgets | "No active budgets. Create one with budget_create." |
| `budget_status_not_found` | AC 3.3 edge | Bad id | `ExecutionFailed("Budget ... not found")` |
| `budget_update_not_found` | AC 3.4 edge | Bad id | `ExecutionFailed("Budget ... not found")` |
| `budget_delete_not_found` | AC 3.5 edge | Bad id | `ExecutionFailed("Budget ... not found")` |

### C.4 Investment Actions (14 tests)

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `portfolio_create_empty_name` | AC 4.1 edge | name="" | `InvalidParams("Portfolio name is required")` |
| `portfolio_list_no_portfolios_message` | AC 4.2 edge | No portfolios | "No portfolios found." |
| `investment_add_invalid_asset_type` | AC 4.3 edge | asset_type="nft" | `InvalidParams("Invalid asset type")` |
| `investment_add_stock_without_symbol` | AC 4.3 edge | asset_type="stock", no symbol | `InvalidParams("Symbol is required for stock")` |
| `investment_add_negative_quantity` | AC 4.3 edge | quantity=-5 | `InvalidParams("Quantity must be positive")` |
| `investment_add_portfolio_not_found` | AC 4.3 edge | Bad portfolio_id | `ExecutionFailed("Portfolio not found")` |
| `investment_update_not_found` | AC 4.4 edge | Bad id | `ExecutionFailed("Investment not found")` |
| `investment_update_price_auto_computes_value` | AC 4.4 | current_price set, no current_value | current_value = current_price * quantity |
| `investment_tx_buy_increases_quantity` | AC 4.5 buy | type="buy", quantity=10 | investment.quantity += 10, cost_basis updated |
| `investment_tx_sell_decreases_quantity` | AC 4.5 sell | type="sell", quantity=5 | investment.quantity -= 5, cost_basis reduced proportionally |
| `investment_tx_sell_exceeds_quantity` | AC 4.5 edge: oversell | sell quantity > holdings | `ExecutionFailed("Cannot sell more than current holding")` |
| `investment_tx_dividend_no_quantity_change` | AC 4.5 dividend | type="dividend" | quantity/cost_basis unchanged, tx recorded |
| `investment_summary_portfolio_not_found` | AC 4.6 edge | Bad portfolio_id | `ExecutionFailed("Portfolio not found")` |
| `price_fetch_real_estate_error` | AC 4.7 edge | asset_type="real_estate" | `ExecutionFailed("Price fetch is not available for real estate")` |

### C.5 Net Worth & Liability Actions (8 tests)

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `liability_add_invalid_type` | AC 5.1 edge | type="auto_loan" | `InvalidParams("Invalid liability type")` |
| `liability_add_negative_principal` | AC 5.1 edge | principal=0 | `InvalidParams("Amounts must be positive")` |
| `liability_add_remaining_exceeds_principal_allowed` | AC 5.1 | remaining > principal | Success (interest capitalized) |
| `liability_list_empty_message` | AC 5.2 edge | No liabilities | "No liabilities recorded." |
| `liability_update_not_found` | AC 5.3 edge | Bad id | `ExecutionFailed("Liability not found")` |
| `net_worth_no_data_message` | AC 5.4 edge | No accounts/investments/liabilities | "No financial data recorded yet" |
| `net_worth_single_currency` | AC 5.4 | 2 accounts + 1 liability, all VND | Correct calculation: assets - liabilities |
| `net_worth_multicurrency_no_param` | AC 5.4 edge: multi-currency | VND + USD accounts | Shows separate totals per currency, no conversion |

### C.6 Goal & FIRE Actions (10 tests)

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `goal_create_invalid_type` | AC 6.1 edge | goal_type="retire_early" | `InvalidParams` |
| `goal_create_negative_amount` | AC 6.1 edge | target_amount=0 | `InvalidParams("Target amount must be positive")` |
| `goal_create_past_deadline` | AC 6.1 edge | deadline=2020-01-01 | `InvalidParams("Deadline must be in the future")` |
| `goal_list_empty_message` | AC 6.2 edge | No goals | "No financial goals set." |
| `goal_update_not_found` | AC 6.3 edge | Bad id | `ExecutionFailed("Goal not found")` |
| `goal_update_invalid_status` | AC 6.3 edge | status="paused" | `InvalidParams` |
| `goal_fire_no_data_requires_expenses_param` | AC 6.4 edge | No tx history | `InvalidParams("No transaction history. Please provide annual_expenses.")` |
| `goal_fire_already_at_fire_number` | AC 6.4 edge | Net worth > FIRE number | "Congratulations! Your net worth exceeds your FIRE number." |
| `goal_fire_negative_savings_message` | AC 6.4 edge | Expenses > income | Message explains FIRE not achievable at current rate |
| `goal_whatif_no_params_error` | AC 6.5 edge | No params | `InvalidParams("Provide at least one scenario parameter")` |

### C.7 Report Actions (8 tests)

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `report_spending_custom_missing_dates` | AC 7.1 edge | period="custom", no date_from | `InvalidParams("date_from and date_to are required")` |
| `report_spending_no_expenses_message` | AC 7.1 edge | No expense txns in period | "No spending recorded for this period." |
| `report_spending_month_table_format` | AC 7.1 | Expenses in Feb | Response contains `| Category |` table |
| `report_trends_invalid_metric` | AC 7.3 edge | metric="balance" | `InvalidParams` |
| `report_trends_insufficient_data` | AC 7.3 edge | < 2 months of data | Response notes "Not enough data for trend analysis" |
| `report_net_worth_history_invalid_interval` | AC 7.4 edge | interval="hourly" | `InvalidParams` |
| `report_net_worth_history_date_from_after_to` | AC 7.4 edge | date_from > date_to | `InvalidParams` |
| `report_unknown_action_error` | Dispatch | action="explode" | `InvalidParams("Unknown finance action: explode")` |

### C.8 Settings Actions (4 tests)

| Test Name | Criterion | Input | Expected |
|-----------|-----------|-------|----------|
| `settings_get_returns_formatted_config` | AC 8.1 | No params | Response contains "Default currency:", "Proactivity level:" etc. |
| `settings_update_invalid_proactivity` | AC 8.2 edge | proactivity_level="always" | `InvalidParams("Invalid proactivity level")` |
| `settings_update_confidence_out_of_range` | AC 8.2 edge | confidence_threshold=1.5 | `InvalidParams("Confidence threshold must be between 0.0 and 1.0")` |
| `settings_update_no_params_error` | AC 8.2 edge | No params | `InvalidParams("No settings to update")` |

---

## D. PriceService Tests

Location: `crates/tools/src/price_service.rs` — inline `#[cfg(test)] mod tests`

| Test Name | Criterion | Setup | Expected |
|-----------|-----------|-------|----------|
| `cache_hit_within_ttl_no_http` | AC 4.7 cache | Prime cache with stock:VIC | Second fetch returns same price, call count = 1 |
| `cache_miss_expired_ttl_triggers_http` | AC 4.7 cache | Prime with TTL=0ms | After TTL, refetch → call count = 2 |
| `cache_key_format_stock` | Cache key | — | Key = `"stock:VIC"` |
| `cache_key_format_crypto` | Cache key | — | Key = `"crypto:BTC"` |
| `provider_routing_stock_uses_yahoo` | AC 4.7 routing | Mock HTTP, stock | Yahoo Finance URL called |
| `provider_routing_crypto_uses_coingecko` | AC 4.7 routing | Mock HTTP, crypto | CoinGecko URL called |
| `provider_routing_exchange_rate` | AC 5.4 conversion | Asset type exchange | ExchangeRate API URL called |
| `api_failure_returns_execution_error` | AC 4.7 edge: API fail | HTTP 500 response | Returns error with message (no panic) |
| `symbol_not_found_returns_error` | AC 4.7 edge: not found | API returns empty | Returns `ExecutionFailed("Symbol ... not found on ...")` |
| `price_converted_to_smallest_unit` | AC 4.7 | Fetch price 123.45 USD | Returns `12345` (cents) |
| `concurrent_fetches_use_cache` ⚠️ `#[ignore]` | Cache thread safety | 3 concurrent fetches same key | Only 1 HTTP call made — **v1 DashMap does not coalesce in-flight requests; mark `#[ignore]` for future enhancement** |
| `real_estate_not_supported` | AC 4.7 edge | asset_type="real_estate" | Returns `ExecutionFailed("Price fetch is not available for real estate")` |

---

## E. FinanceHandler Tests

Location: `crates/agent/src/finance_adapter.rs` — inline `#[cfg(test)] mod tests`

| Test Name | Criterion | Setup | Expected |
|-----------|-----------|-------|----------|
| `daily_review_categorizes_uncategorized_txns` | Handler AC | Txns with no category, mock LLM returns categories | High-confidence categories applied via update |
| `daily_review_returns_formatted_summary` | Handler AC | Some txns today | Summary contains spending, budget status |
| `check_budgets_returns_alert_over_threshold` | AC check_budgets | Budget 90% spent (threshold=80) | Alert returned with budget_name, percentage |
| `check_budgets_no_alert_under_threshold` | AC check_budgets | Budget 70% spent (threshold=80) | Empty alerts vec |
| `check_budgets_returns_multiple_alerts` | AC check_budgets | 3 budgets over threshold | 3 alerts returned |
| `refresh_prices_updates_investments` | AC refresh_prices | 2 investments with symbols | `updated=2, failed=0` in summary |
| `refresh_prices_handles_api_failure_gracefully` | AC refresh_prices edge | Mock HTTP fails | `failed > 0`, no panic, other investments still updated |
| `proactivity_level_returns_config_value` | AC proactivity_level | Config with "moderate" | Returns `ProactivityLevel::Moderate` |

---

## F. Integration Tests

Location: `tests/finance_integration_tests.rs`

| Test Name | Criterion | Setup | Expected |
|-----------|-----------|-------|----------|
| `finance_tool_registered_in_tool_registry` | Agent wiring | Config with finance.enabled=true | ToolRegistry contains "finance" tool |
| `finance_tool_not_registered_when_disabled` | Agent wiring | finance.enabled=false | ToolRegistry does NOT contain "finance" |
| `finance_tool_dispatch_via_registry` | End-to-end dispatch | Call `registry.execute("finance", json!({action: "account_add", ...}))` | Returns success response |
| `account_add_then_list` | E2E accounts | Add account, then list | Account appears in list with correct data |
| `tx_add_updates_account_balance` | E2E tx | Add account (balance=0), add expense 100k | Account balance = -100k |
| `tx_add_with_budget_shows_impact` | E2E budget impact | Create budget for "food", add food expense | Response includes budget_impact |
| `transfer_creates_paired_rows` | E2E transfer | Two accounts, transfer 50k | Source -= 50k, dest += 50k, 2 rows with same transfer_id |
| `investment_buy_sell_cost_basis` | E2E investment | Buy 100 shares at 10k each (cost 1M). Sell 50 | cost_basis should be 500k after sell |
| `net_worth_calculation` | E2E net worth | Account 100k + investment 500k + liability 200k | Net worth = 400k |
| `budget_list_shows_current_usage` | E2E budget | Budget 500k for food, spend 250k on food | budget_list shows 50% used |
| `goal_fire_with_transaction_history` | E2E FIRE | 12 months of income/expense txns | FIRE calculation uses inferred annual_expenses |
| `settings_update_persists_to_disk` | E2E settings | `settings_update` with new currency | Config file on disk updated |
| `price_fetch_updates_investment` | E2E price | Investment with symbol="VIC" | After price_fetch, investment.current_value updated |
| `report_spending_shows_category_breakdown` | E2E report | Multiple expenses in different categories | Report shows each category with % |

---

## Test Infrastructure Notes

### Database Helper Pattern
All storage tests use the same graceful-skip pattern as `todo_repo_tests.rs`:
```rust
async fn test_finance_account_repo() -> Option<FinanceAccountRepo> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
    match StoragePool::connect(&url).await {
        Ok(pool) => Some(FinanceAccountRepo::new(pool.inner().clone())),
        Err(_) => None,
    }
}
```
Tests return early (not fail) when no DB is available. CI can set `DATABASE_URL` to run full suite.

### UUID Uniqueness
Test data uses `unique_id()` helper to avoid collisions across parallel test runs:
```rust
fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
}
```

### Tool Test Pattern
Finance tool tests use `FinanceTool::new(...)` with real repos (if DB available) or skip:
```rust
async fn make_finance_tool() -> Option<FinanceTool> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| ...);
    match StoragePool::connect(&url).await {
        Ok(pool) => Some(FinanceTool::new(/* repos from pool */)),
        Err(_) => None,
    }
}
```

### Cleanup Pattern
Tests that insert rows always clean up after themselves to avoid DB state leakage:
```rust
// Cleanup at end of test
let _ = repo.delete(&id).await;
```

---

## Skeleton Files Created

| File | Status | Tests |
|------|--------|-------|
| `crates/storage/src/repos/tests/finance_account_repo_tests.rs` | Skeleton | 14 |
| `crates/storage/src/repos/tests/finance_transaction_repo_tests.rs` | Skeleton | 20 |
| `crates/storage/src/repos/tests/finance_budget_repo_tests.rs` | Skeleton | 14 |
| `crates/storage/src/repos/tests/finance_investment_repo_tests.rs` | Skeleton | 16 |
| `crates/storage/src/repos/tests/finance_goal_repo_tests.rs` | Skeleton | 8 |
| `crates/storage/src/repos/tests/finance_liability_repo_tests.rs` | Skeleton | 6 |
| `crates/tools/src/finance_types_tests.rs` | Skeleton | 34 |
| `crates/tools/src/finance_tool/tests.rs` | Skeleton | 97 |
| `tests/finance_integration_tests.rs` | Skeleton | 14 |
