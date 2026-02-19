//! Tests for `finance_types` domain types.
//!
//! Included by `finance_types.rs` via `#[path = "finance_types_tests.rs"] mod tests`.
//! All test functions are in the `tests` module so nextest discovers them via the
//! `tools` crate.

use super::*;
use chrono::{NaiveDate, Utc};

// ---------------------------------------------------------------------------
// B.1: Enum from_str_loose / as_str round-trips
// ---------------------------------------------------------------------------

#[test]
fn account_type_roundtrip_all_variants() {
    let cases = [
        (AccountType::Cash, "cash"),
        (AccountType::Bank, "bank"),
        (AccountType::Ewallet, "ewallet"),
        (AccountType::CryptoWallet, "crypto_wallet"),
        (AccountType::Brokerage, "brokerage"),
        (AccountType::Other, "other"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s, "as_str mismatch for {s}");
        assert_eq!(
            AccountType::from_str_loose(s),
            Some(variant),
            "from_str_loose({s}) failed"
        );
        // Case-insensitive
        assert_eq!(
            AccountType::from_str_loose(&s.to_uppercase()),
            Some(variant),
            "uppercase from_str_loose({}) failed",
            s.to_uppercase()
        );
        // Round-trip
        assert_eq!(
            AccountType::from_str_loose(variant.as_str()),
            Some(variant),
            "round-trip failed for {s}"
        );
        // Display matches as_str
        assert_eq!(variant.to_string(), s, "Display mismatch for {s}");
    }
}

#[test]
fn transaction_type_roundtrip() {
    let cases = [
        (TransactionType::Income, "income"),
        (TransactionType::Expense, "expense"),
        (TransactionType::Transfer, "transfer"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s);
        assert_eq!(TransactionType::from_str_loose(s), Some(variant));
        // Case-insensitive
        assert_eq!(
            TransactionType::from_str_loose(&s.to_uppercase()),
            Some(variant),
            "uppercase failed for {s}"
        );
        assert_eq!(variant.to_string(), s);
    }
}

#[test]
fn budget_period_roundtrip() {
    let cases = [
        (BudgetPeriod::Monthly, "monthly"),
        (BudgetPeriod::Weekly, "weekly"),
        (BudgetPeriod::Yearly, "yearly"),
        (BudgetPeriod::Custom, "custom"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s);
        assert_eq!(BudgetPeriod::from_str_loose(s), Some(variant));
        assert_eq!(variant.to_string(), s);
    }
}

#[test]
fn budget_method_roundtrip() {
    assert_eq!(BudgetMethod::Standard.as_str(), "standard");
    assert_eq!(BudgetMethod::SixJar.as_str(), "six_jar");
    assert_eq!(
        BudgetMethod::from_str_loose("six_jar"),
        Some(BudgetMethod::SixJar)
    );
    assert_eq!(
        BudgetMethod::from_str_loose("standard"),
        Some(BudgetMethod::Standard)
    );
    assert_eq!(BudgetMethod::SixJar.to_string(), "six_jar");
}

#[test]
fn jar_type_roundtrip() {
    let cases = [
        (JarType::Essentials, "essentials"),
        (JarType::Savings, "savings"),
        (JarType::Investment, "investment"),
        (JarType::Education, "education"),
        (JarType::Entertainment, "entertainment"),
        (JarType::Charity, "charity"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s);
        assert_eq!(JarType::from_str_loose(s), Some(variant));
        assert_eq!(variant.to_string(), s);
    }
}

#[test]
fn asset_type_roundtrip() {
    let cases = [
        (AssetType::Stock, "stock"),
        (AssetType::Etf, "etf"),
        (AssetType::Crypto, "crypto"),
        (AssetType::RealEstate, "real_estate"),
        (AssetType::Bond, "bond"),
        (AssetType::Other, "other"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s, "as_str mismatch for {s}");
        assert_eq!(
            AssetType::from_str_loose(s),
            Some(variant),
            "from_str_loose({s}) failed"
        );
        assert_eq!(variant.to_string(), s, "Display mismatch for {s}");
    }
    // snake_case with underscore
    assert_eq!(
        AssetType::from_str_loose("real_estate"),
        Some(AssetType::RealEstate)
    );
}

#[test]
fn investment_tx_type_roundtrip() {
    let cases = [
        (InvestmentTxType::Buy, "buy"),
        (InvestmentTxType::Sell, "sell"),
        (InvestmentTxType::Dividend, "dividend"),
        (InvestmentTxType::RentalIncome, "rental_income"),
        (InvestmentTxType::Interest, "interest"),
        (InvestmentTxType::Split, "split"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s, "as_str mismatch for {s}");
        assert_eq!(
            InvestmentTxType::from_str_loose(s),
            Some(variant),
            "from_str_loose({s}) failed"
        );
        assert_eq!(variant.to_string(), s);
    }
    assert_eq!(
        InvestmentTxType::from_str_loose("rental_income"),
        Some(InvestmentTxType::RentalIncome)
    );
}

#[test]
fn goal_type_roundtrip() {
    let cases = [
        (GoalType::Savings, "savings"),
        (GoalType::Purchase, "purchase"),
        (GoalType::DebtPayoff, "debt_payoff"),
        (GoalType::Fire, "fire"),
        (GoalType::Custom, "custom"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s);
        assert_eq!(GoalType::from_str_loose(s), Some(variant));
        assert_eq!(variant.to_string(), s);
    }
    assert_eq!(GoalType::from_str_loose("fire"), Some(GoalType::Fire));
    assert_eq!(
        GoalType::from_str_loose("debt_payoff"),
        Some(GoalType::DebtPayoff)
    );
}

#[test]
fn goal_status_roundtrip() {
    let cases = [
        (GoalStatus::Active, "active"),
        (GoalStatus::Achieved, "achieved"),
        (GoalStatus::Abandoned, "abandoned"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s);
        assert_eq!(GoalStatus::from_str_loose(s), Some(variant));
        assert_eq!(variant.to_string(), s);
    }
}

#[test]
fn liability_type_roundtrip() {
    let cases = [
        (LiabilityType::Mortgage, "mortgage"),
        (LiabilityType::CreditCard, "credit_card"),
        (LiabilityType::PersonalLoan, "personal_loan"),
        (LiabilityType::StudentLoan, "student_loan"),
        (LiabilityType::Other, "other"),
    ];
    for (variant, s) in cases {
        assert_eq!(variant.as_str(), s);
        assert_eq!(LiabilityType::from_str_loose(s), Some(variant));
        assert_eq!(variant.to_string(), s);
    }
    assert_eq!(
        LiabilityType::from_str_loose("credit_card"),
        Some(LiabilityType::CreditCard)
    );
    assert_eq!(
        LiabilityType::from_str_loose("personal_loan"),
        Some(LiabilityType::PersonalLoan)
    );
}

#[test]
fn enum_from_str_unknown_returns_none() {
    assert_eq!(AccountType::from_str_loose("garbage_value"), None);
    assert_eq!(AccountType::from_str_loose(""), None);
    assert_eq!(TransactionType::from_str_loose("credit"), None);
    assert_eq!(TransactionType::from_str_loose(""), None);
    assert_eq!(BudgetPeriod::from_str_loose("daily"), None);
    assert_eq!(BudgetMethod::from_str_loose("unknown"), None);
    assert_eq!(JarType::from_str_loose("health"), None);
    assert_eq!(AssetType::from_str_loose("nft"), None);
    assert_eq!(AssetType::from_str_loose(""), None);
    assert_eq!(InvestmentTxType::from_str_loose("refund"), None);
    assert_eq!(GoalType::from_str_loose("retire_early"), None);
    assert_eq!(GoalStatus::from_str_loose("paused"), None);
    assert_eq!(LiabilityType::from_str_loose("auto_loan"), None);
}

// ---------------------------------------------------------------------------
// B.2: From<Row> for Domain round-trips
// ---------------------------------------------------------------------------

#[test]
fn finance_account_row_to_domain() {
    let now = Utc::now();
    let row = storage::FinanceAccountRow {
        id: "acc001".to_string(),
        name: "VCB Savings".to_string(),
        account_type: "bank".to_string(),
        currency: "VND".to_string(),
        balance: 5_000_000,
        institution: Some("VCB".to_string()),
        notes: None,
        is_archived: false,
        created_at: now,
        updated_at: now,
    };
    let account = FinanceAccount::from(row.clone());
    assert_eq!(account.account_type, AccountType::Bank);
    assert_eq!(account.name, row.name);
    assert_eq!(account.is_archived, row.is_archived);
    assert_eq!(account.balance, row.balance);
}

#[test]
fn finance_account_domain_to_row() {
    let now = Utc::now();
    let account = FinanceAccount {
        id: "acc002".to_string(),
        name: "My Wallet".to_string(),
        account_type: AccountType::Ewallet,
        currency: "USD".to_string(),
        balance: 100,
        institution: None,
        notes: None,
        is_archived: false,
        created_at: now,
        updated_at: now,
    };
    let row = storage::FinanceAccountRow::from(&account);
    assert_eq!(row.account_type, "ewallet");
    assert_eq!(row.id, account.id);
}

#[test]
fn finance_account_roundtrip_preserves_data() {
    let now = Utc::now();
    let row = storage::FinanceAccountRow {
        id: "acc003".to_string(),
        name: "Brokerage Account".to_string(),
        account_type: "brokerage".to_string(),
        currency: "USD".to_string(),
        balance: 1_000_000,
        institution: Some("Vanguard".to_string()),
        notes: Some("Long-term".to_string()),
        is_archived: false,
        created_at: now,
        updated_at: now,
    };
    let domain = FinanceAccount::from(row.clone());
    let back_row = storage::FinanceAccountRow::from(&domain);
    assert_eq!(back_row.id, row.id);
    assert_eq!(back_row.account_type, row.account_type);
    assert_eq!(back_row.balance, row.balance);
    assert_eq!(back_row.institution, row.institution);
}

#[test]
fn finance_transaction_row_to_domain() {
    let now = Utc::now();
    let date = NaiveDate::from_ymd_opt(2026, 2, 19).unwrap();
    let row = storage::FinanceTransactionRow {
        id: "tx001".to_string(),
        account_id: "acc001".to_string(),
        tx_type: "transfer".to_string(),
        amount: 200_000,
        currency: "VND".to_string(),
        category: None,
        subcategory: None,
        counterparty: None,
        notes: None,
        tx_date: date,
        transfer_id: Some("uuid-abc".to_string()),
        is_recurring: false,
        recurring_rule: None,
        created_at: now,
        updated_at: now,
    };
    let tx = FinanceTransaction::from(row);
    assert_eq!(tx.tx_type, TransactionType::Transfer);
    assert_eq!(tx.transfer_id, Some("uuid-abc".to_string()));
}

#[test]
fn finance_transaction_domain_to_row() {
    let now = Utc::now();
    let date = NaiveDate::from_ymd_opt(2026, 2, 19).unwrap();
    let tx = FinanceTransaction {
        id: "tx002".to_string(),
        account_id: "acc001".to_string(),
        tx_type: TransactionType::Income,
        amount: 5_000_000,
        currency: "VND".to_string(),
        category: Some("salary".to_string()),
        subcategory: None,
        counterparty: None,
        notes: None,
        tx_date: date,
        transfer_id: None,
        is_recurring: false,
        recurring_rule: None,
        created_at: now,
        updated_at: now,
    };
    let row = storage::FinanceTransactionRow::from(&tx);
    assert_eq!(row.tx_type, "income");
}

#[test]
fn finance_budget_roundtrip() {
    let now = Utc::now();
    let start = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let row = storage::FinanceBudgetRow {
        id: "bud001".to_string(),
        name: "Monthly Food".to_string(),
        amount: 5_000_000,
        currency: "VND".to_string(),
        period: "monthly".to_string(),
        category: Some("food".to_string()),
        method: "six_jar".to_string(),
        jar_type: Some("essentials".to_string()),
        start_date: start,
        end_date: None,
        is_active: true,
        alert_threshold: 80,
        created_at: now,
        updated_at: now,
    };
    let domain = FinanceBudget::from(row.clone());
    assert_eq!(domain.method, BudgetMethod::SixJar);
    assert_eq!(domain.jar_type, Some(JarType::Essentials));

    let back_row = storage::FinanceBudgetRow::from(&domain);
    assert_eq!(back_row.jar_type, Some("essentials".to_string()));
    assert_eq!(back_row.method, "six_jar");
}

#[test]
fn finance_investment_roundtrip() {
    let now = Utc::now();
    let row = storage::FinanceInvestmentRow {
        id: "inv001".to_string(),
        portfolio_id: "pf001".to_string(),
        asset_type: "real_estate".to_string(),
        symbol: None,
        name: "Hanoi Apartment".to_string(),
        quantity: 1.0,
        cost_basis: 2_000_000_000,
        currency: "VND".to_string(),
        current_price: None,
        current_value: None,
        purchase_date: None,
        notes: None,
        created_at: now,
        updated_at: now,
    };
    let domain = FinanceInvestment::from(row.clone());
    assert_eq!(domain.current_price, None);
    assert_eq!(domain.current_value, None);
    assert_eq!(domain.asset_type, AssetType::RealEstate);

    let back_row = storage::FinanceInvestmentRow::from(&domain);
    assert_eq!(back_row.current_price, None);
    assert_eq!(back_row.asset_type, "real_estate");
}

#[test]
fn finance_goal_roundtrip() {
    let now = Utc::now();
    let deadline = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
    let row = storage::FinanceGoalRow {
        id: "goal001".to_string(),
        name: "Emergency Fund".to_string(),
        goal_type: "savings".to_string(),
        target_amount: 100_000_000,
        current_amount: 100_000_000,
        currency: "VND".to_string(),
        status: "achieved".to_string(),
        deadline: Some(deadline),
        monthly_contribution: None,
        expected_return_rate: None,
        inflation_rate: None,
        notes: None,
        created_at: now,
        updated_at: now,
    };
    let domain = FinanceGoal::from(row.clone());
    assert_eq!(domain.status, GoalStatus::Achieved);
    assert_eq!(domain.deadline, Some(deadline));

    let back_row = storage::FinanceGoalRow::from(&domain);
    assert_eq!(back_row.status, "achieved");
}

#[test]
fn finance_liability_roundtrip() {
    let now = Utc::now();
    let row = storage::FinanceLiabilityRow {
        id: "liab001".to_string(),
        name: "Credit Card Debt".to_string(),
        liability_type: "credit_card".to_string(),
        principal: 10_000_000,
        remaining: 5_000_000,
        currency: "VND".to_string(),
        interest_rate: Some(18.0),
        monthly_payment: None,
        due_date: None,
        notes: None,
        created_at: now,
        updated_at: now,
    };
    let domain = FinanceLiability::from(row.clone());
    assert_eq!(domain.liability_type, LiabilityType::CreditCard);
    assert_eq!(domain.interest_rate, Some(18.0));

    let back_row = storage::FinanceLiabilityRow::from(&domain);
    assert_eq!(back_row.interest_rate, Some(18.0));
    assert_eq!(back_row.liability_type, "credit_card");
}

#[test]
fn finance_portfolio_roundtrip() {
    let now = Utc::now();
    let row = storage::FinancePortfolioRow {
        id: "pf001".to_string(),
        name: "Vietnam Stocks".to_string(),
        description: None,
        currency: "VND".to_string(),
        created_at: now,
        updated_at: now,
    };
    let domain = FinancePortfolio::from(row.clone());
    assert_eq!(domain.name, row.name);
    assert_eq!(domain.currency, row.currency);

    let back_row = storage::FinancePortfolioRow::from(&domain);
    assert_eq!(back_row.id, row.id);
    assert_eq!(back_row.name, row.name);
}

#[test]
fn finance_investment_tx_roundtrip() {
    let now = Utc::now();
    let date = NaiveDate::from_ymd_opt(2026, 2, 19).unwrap();
    let row = storage::FinanceInvestmentTxRow {
        id: "itx001".to_string(),
        investment_id: "inv001".to_string(),
        tx_type: "dividend".to_string(),
        quantity: None,
        price_per_unit: None,
        total_amount: 50_000,
        currency: "VND".to_string(),
        fees: 0,
        tx_date: date,
        notes: None,
        created_at: now,
    };
    let domain = FinanceInvestmentTx::from(row.clone());
    assert_eq!(domain.tx_type, InvestmentTxType::Dividend);
    assert_eq!(domain.quantity, None);
    assert_eq!(domain.total_amount, 50_000);

    let back_row = storage::FinanceInvestmentTxRow::from(&domain);
    assert_eq!(back_row.tx_type, "dividend");
    assert_eq!(back_row.quantity, None);
}

// ---------------------------------------------------------------------------
// B.3: Filter Conversion
// ---------------------------------------------------------------------------

#[test]
fn tx_filter_converts_tx_type_enum() {
    let filter = FinanceTransactionFilter {
        tx_type: Some(TransactionType::Expense),
        ..Default::default()
    };
    let storage_filter = filter.to_storage_filter();
    assert_eq!(storage_filter.tx_type, Some("expense".to_string()));
}

#[test]
fn tx_filter_none_fields_remain_none() {
    let filter = FinanceTransactionFilter::default();
    let storage_filter = filter.to_storage_filter();
    assert_eq!(storage_filter.account_id, None);
    assert_eq!(storage_filter.category, None);
    assert_eq!(storage_filter.tx_type, None);
}

#[test]
fn tx_filter_limit_usize_to_i64() {
    let filter = FinanceTransactionFilter {
        limit: Some(50),
        ..Default::default()
    };
    let storage_filter = filter.to_storage_filter();
    assert_eq!(storage_filter.limit, Some(50i64));
}
