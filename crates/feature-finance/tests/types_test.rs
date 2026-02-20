//! Tests for Finance types with DomainEnum (AC 5.2).
//!
//! Verifies that all 10 Finance enums correctly use the DomainEnum derive macro
//! for from_str_loose (case-insensitive, alias-aware) and as_str (snake_case).
//!
//! Enums covered:
//!   AccountType, TransactionType, BudgetPeriod, BudgetMethod, JarType,
//!   AssetType, InvestmentTxType, GoalType, GoalStatus, LiabilityType

// ============================================================
// 1. AccountType
// ============================================================

mod account_type {
    use feature_finance::types::AccountType;

    #[test]
    fn test_as_str_cash() {
        assert_eq!(AccountType::Cash.as_str(), "cash");
    }

    #[test]
    fn test_as_str_bank() {
        assert_eq!(AccountType::Bank.as_str(), "bank");
    }

    #[test]
    fn test_as_str_ewallet() {
        assert_eq!(AccountType::Ewallet.as_str(), "ewallet");
    }

    #[test]
    fn test_as_str_crypto_wallet() {
        assert_eq!(AccountType::CryptoWallet.as_str(), "crypto_wallet");
    }

    #[test]
    fn test_as_str_brokerage() {
        assert_eq!(AccountType::Brokerage.as_str(), "brokerage");
    }

    #[test]
    fn test_from_str_loose_e_wallet_alias() {
        assert_eq!(
            AccountType::from_str_loose("e_wallet"),
            Some(AccountType::Ewallet)
        );
    }

    #[test]
    fn test_from_str_loose_cryptowallet_alias() {
        assert_eq!(
            AccountType::from_str_loose("cryptowallet"),
            Some(AccountType::CryptoWallet)
        );
    }

    #[test]
    fn test_from_str_loose_case_insensitive() {
        assert_eq!(
            AccountType::from_str_loose("CASH"),
            Some(AccountType::Cash)
        );
        assert_eq!(
            AccountType::from_str_loose("Bank"),
            Some(AccountType::Bank)
        );
    }

    #[test]
    fn test_from_str_loose_unknown() {
        assert_eq!(AccountType::from_str_loose("invalid"), None);
    }
}

// ============================================================
// 2. TransactionType
// ============================================================

mod transaction_type {
    use feature_finance::types::TransactionType;

    #[test]
    fn test_as_str_income() {
        assert_eq!(TransactionType::Income.as_str(), "income");
    }

    #[test]
    fn test_as_str_expense() {
        assert_eq!(TransactionType::Expense.as_str(), "expense");
    }

    #[test]
    fn test_as_str_transfer() {
        assert_eq!(TransactionType::Transfer.as_str(), "transfer");
    }

    #[test]
    fn test_from_str_loose_case_insensitive() {
        assert_eq!(
            TransactionType::from_str_loose("INCOME"),
            Some(TransactionType::Income)
        );
    }
}

// ============================================================
// 3. BudgetPeriod
// ============================================================

mod budget_period {
    use feature_finance::types::BudgetPeriod;

    #[test]
    fn test_as_str_monthly() {
        assert_eq!(BudgetPeriod::Monthly.as_str(), "monthly");
    }

    #[test]
    fn test_as_str_weekly() {
        assert_eq!(BudgetPeriod::Weekly.as_str(), "weekly");
    }

    #[test]
    fn test_as_str_yearly() {
        assert_eq!(BudgetPeriod::Yearly.as_str(), "yearly");
    }

    #[test]
    fn test_as_str_custom() {
        assert_eq!(BudgetPeriod::Custom.as_str(), "custom");
    }

    #[test]
    fn test_from_str_loose_month_alias() {
        assert_eq!(
            BudgetPeriod::from_str_loose("month"),
            Some(BudgetPeriod::Monthly)
        );
    }

    #[test]
    fn test_from_str_loose_week_alias() {
        assert_eq!(
            BudgetPeriod::from_str_loose("week"),
            Some(BudgetPeriod::Weekly)
        );
    }

    #[test]
    fn test_from_str_loose_year_and_annual_aliases() {
        assert_eq!(
            BudgetPeriod::from_str_loose("year"),
            Some(BudgetPeriod::Yearly)
        );
        assert_eq!(
            BudgetPeriod::from_str_loose("annual"),
            Some(BudgetPeriod::Yearly)
        );
    }
}

// ============================================================
// 4. BudgetMethod
// ============================================================

mod budget_method {
    use feature_finance::types::BudgetMethod;

    #[test]
    fn test_as_str_standard() {
        assert_eq!(BudgetMethod::Standard.as_str(), "standard");
    }

    #[test]
    fn test_as_str_six_jar() {
        assert_eq!(BudgetMethod::SixJar.as_str(), "six_jar");
    }

    #[test]
    fn test_from_str_loose_sixjar_alias() {
        assert_eq!(
            BudgetMethod::from_str_loose("sixjar"),
            Some(BudgetMethod::SixJar)
        );
    }

    #[test]
    fn test_from_str_loose_6jar_alias() {
        assert_eq!(
            BudgetMethod::from_str_loose("6jar"),
            Some(BudgetMethod::SixJar)
        );
    }
}

// ============================================================
// 5. JarType
// ============================================================

mod jar_type {
    use feature_finance::types::JarType;

    #[test]
    fn test_as_str_essentials() {
        assert_eq!(JarType::Essentials.as_str(), "essentials");
    }

    #[test]
    fn test_as_str_savings() {
        assert_eq!(JarType::Savings.as_str(), "savings");
    }

    #[test]
    fn test_as_str_investment() {
        assert_eq!(JarType::Investment.as_str(), "investment");
    }

    #[test]
    fn test_as_str_education() {
        assert_eq!(JarType::Education.as_str(), "education");
    }

    #[test]
    fn test_as_str_entertainment() {
        assert_eq!(JarType::Entertainment.as_str(), "entertainment");
    }

    #[test]
    fn test_as_str_charity() {
        assert_eq!(JarType::Charity.as_str(), "charity");
    }

    #[test]
    fn test_from_str_loose_necessities_alias() {
        assert_eq!(
            JarType::from_str_loose("necessities"),
            Some(JarType::Essentials)
        );
    }

    #[test]
    fn test_from_str_loose_saving_alias() {
        assert_eq!(
            JarType::from_str_loose("saving"),
            Some(JarType::Savings)
        );
    }

    #[test]
    fn test_from_str_loose_play_alias() {
        assert_eq!(
            JarType::from_str_loose("play"),
            Some(JarType::Entertainment)
        );
    }

    #[test]
    fn test_from_str_loose_give_alias() {
        assert_eq!(
            JarType::from_str_loose("give"),
            Some(JarType::Charity)
        );
    }

    #[test]
    fn test_from_str_loose_investments_alias() {
        assert_eq!(
            JarType::from_str_loose("investments"),
            Some(JarType::Investment)
        );
    }
}

// ============================================================
// 6. AssetType
// ============================================================

mod asset_type {
    use feature_finance::types::AssetType;

    #[test]
    fn test_as_str_stock() {
        assert_eq!(AssetType::Stock.as_str(), "stock");
    }

    #[test]
    fn test_as_str_etf() {
        assert_eq!(AssetType::Etf.as_str(), "etf");
    }

    #[test]
    fn test_as_str_crypto() {
        assert_eq!(AssetType::Crypto.as_str(), "crypto");
    }

    #[test]
    fn test_as_str_real_estate() {
        assert_eq!(AssetType::RealEstate.as_str(), "real_estate");
    }

    #[test]
    fn test_as_str_bond() {
        assert_eq!(AssetType::Bond.as_str(), "bond");
    }

    #[test]
    fn test_as_str_exchange_rate() {
        assert_eq!(AssetType::ExchangeRate.as_str(), "exchange_rate");
    }

    #[test]
    fn test_from_str_loose_stocks_alias() {
        assert_eq!(
            AssetType::from_str_loose("stocks"),
            Some(AssetType::Stock)
        );
    }

    #[test]
    fn test_from_str_loose_equity_alias() {
        assert_eq!(
            AssetType::from_str_loose("equity"),
            Some(AssetType::Stock)
        );
    }

    #[test]
    fn test_from_str_loose_cryptocurrency_alias() {
        assert_eq!(
            AssetType::from_str_loose("cryptocurrency"),
            Some(AssetType::Crypto)
        );
    }

    #[test]
    fn test_from_str_loose_property_alias() {
        assert_eq!(
            AssetType::from_str_loose("property"),
            Some(AssetType::RealEstate)
        );
    }

    #[test]
    fn test_from_str_loose_forex_alias() {
        assert_eq!(
            AssetType::from_str_loose("forex"),
            Some(AssetType::ExchangeRate)
        );
        assert_eq!(
            AssetType::from_str_loose("fx"),
            Some(AssetType::ExchangeRate)
        );
    }

    #[test]
    fn test_from_str_loose_bonds_alias() {
        assert_eq!(
            AssetType::from_str_loose("bonds"),
            Some(AssetType::Bond)
        );
        assert_eq!(
            AssetType::from_str_loose("fixed_income"),
            Some(AssetType::Bond)
        );
    }
}

// ============================================================
// 7. InvestmentTxType
// ============================================================

mod investment_tx_type {
    use feature_finance::types::InvestmentTxType;

    #[test]
    fn test_as_str_buy() {
        assert_eq!(InvestmentTxType::Buy.as_str(), "buy");
    }

    #[test]
    fn test_as_str_sell() {
        assert_eq!(InvestmentTxType::Sell.as_str(), "sell");
    }

    #[test]
    fn test_as_str_dividend() {
        assert_eq!(InvestmentTxType::Dividend.as_str(), "dividend");
    }

    #[test]
    fn test_as_str_rental_income() {
        assert_eq!(InvestmentTxType::RentalIncome.as_str(), "rental_income");
    }

    #[test]
    fn test_as_str_interest() {
        assert_eq!(InvestmentTxType::Interest.as_str(), "interest");
    }

    #[test]
    fn test_as_str_split() {
        assert_eq!(InvestmentTxType::Split.as_str(), "split");
    }

    #[test]
    fn test_from_str_loose_purchase_alias() {
        assert_eq!(
            InvestmentTxType::from_str_loose("purchase"),
            Some(InvestmentTxType::Buy)
        );
    }

    #[test]
    fn test_from_str_loose_sale_alias() {
        assert_eq!(
            InvestmentTxType::from_str_loose("sale"),
            Some(InvestmentTxType::Sell)
        );
    }

    #[test]
    fn test_from_str_loose_rental_aliases() {
        assert_eq!(
            InvestmentTxType::from_str_loose("rental"),
            Some(InvestmentTxType::RentalIncome)
        );
        assert_eq!(
            InvestmentTxType::from_str_loose("rent"),
            Some(InvestmentTxType::RentalIncome)
        );
    }
}

// ============================================================
// 8. GoalType
// ============================================================

mod goal_type {
    use feature_finance::types::GoalType;

    #[test]
    fn test_as_str_savings() {
        assert_eq!(GoalType::Savings.as_str(), "savings");
    }

    #[test]
    fn test_as_str_purchase() {
        assert_eq!(GoalType::Purchase.as_str(), "purchase");
    }

    #[test]
    fn test_as_str_debt_payoff() {
        assert_eq!(GoalType::DebtPayoff.as_str(), "debt_payoff");
    }

    #[test]
    fn test_as_str_fire() {
        assert_eq!(GoalType::Fire.as_str(), "fire");
    }

    #[test]
    fn test_from_str_loose_saving_alias() {
        assert_eq!(
            GoalType::from_str_loose("saving"),
            Some(GoalType::Savings)
        );
    }

    #[test]
    fn test_from_str_loose_emergency_fund_alias() {
        assert_eq!(
            GoalType::from_str_loose("emergency_fund"),
            Some(GoalType::Savings)
        );
    }

    #[test]
    fn test_from_str_loose_buy_alias() {
        assert_eq!(
            GoalType::from_str_loose("buy"),
            Some(GoalType::Purchase)
        );
    }

    #[test]
    fn test_from_str_loose_debt_aliases() {
        assert_eq!(
            GoalType::from_str_loose("debt"),
            Some(GoalType::DebtPayoff)
        );
        assert_eq!(
            GoalType::from_str_loose("payoff"),
            Some(GoalType::DebtPayoff)
        );
    }

    #[test]
    fn test_from_str_loose_financial_independence_alias() {
        assert_eq!(
            GoalType::from_str_loose("financial_independence"),
            Some(GoalType::Fire)
        );
    }
}

// ============================================================
// 9. GoalStatus
// ============================================================

mod goal_status {
    use feature_finance::types::GoalStatus;

    #[test]
    fn test_as_str_active() {
        assert_eq!(GoalStatus::Active.as_str(), "active");
    }

    #[test]
    fn test_as_str_achieved() {
        assert_eq!(GoalStatus::Achieved.as_str(), "achieved");
    }

    #[test]
    fn test_as_str_abandoned() {
        assert_eq!(GoalStatus::Abandoned.as_str(), "abandoned");
    }

    #[test]
    fn test_from_str_loose_in_progress_alias() {
        assert_eq!(
            GoalStatus::from_str_loose("in_progress"),
            Some(GoalStatus::Active)
        );
    }

    #[test]
    fn test_from_str_loose_completed_alias() {
        assert_eq!(
            GoalStatus::from_str_loose("completed"),
            Some(GoalStatus::Achieved)
        );
        assert_eq!(
            GoalStatus::from_str_loose("done"),
            Some(GoalStatus::Achieved)
        );
    }

    #[test]
    fn test_from_str_loose_cancelled_alias() {
        assert_eq!(
            GoalStatus::from_str_loose("cancelled"),
            Some(GoalStatus::Abandoned)
        );
    }
}

// ============================================================
// 10. LiabilityType
// ============================================================

mod liability_type {
    use feature_finance::types::LiabilityType;

    #[test]
    fn test_as_str_mortgage() {
        assert_eq!(LiabilityType::Mortgage.as_str(), "mortgage");
    }

    #[test]
    fn test_as_str_credit_card() {
        assert_eq!(LiabilityType::CreditCard.as_str(), "credit_card");
    }

    #[test]
    fn test_as_str_personal_loan() {
        assert_eq!(LiabilityType::PersonalLoan.as_str(), "personal_loan");
    }

    #[test]
    fn test_as_str_student_loan() {
        assert_eq!(LiabilityType::StudentLoan.as_str(), "student_loan");
    }

    #[test]
    fn test_as_str_other() {
        assert_eq!(LiabilityType::Other.as_str(), "other");
    }

    #[test]
    fn test_from_str_loose_home_loan_alias() {
        assert_eq!(
            LiabilityType::from_str_loose("home_loan"),
            Some(LiabilityType::Mortgage)
        );
    }

    #[test]
    fn test_from_str_loose_creditcard_alias() {
        assert_eq!(
            LiabilityType::from_str_loose("creditcard"),
            Some(LiabilityType::CreditCard)
        );
        assert_eq!(
            LiabilityType::from_str_loose("cc"),
            Some(LiabilityType::CreditCard)
        );
    }

    #[test]
    fn test_from_str_loose_personal_alias() {
        assert_eq!(
            LiabilityType::from_str_loose("personal"),
            Some(LiabilityType::PersonalLoan)
        );
    }

    #[test]
    fn test_from_str_loose_student_aliases() {
        assert_eq!(
            LiabilityType::from_str_loose("student"),
            Some(LiabilityType::StudentLoan)
        );
        assert_eq!(
            LiabilityType::from_str_loose("education_loan"),
            Some(LiabilityType::StudentLoan)
        );
    }

    #[test]
    fn test_from_str_loose_case_insensitive() {
        assert_eq!(
            LiabilityType::from_str_loose("MORTGAGE"),
            Some(LiabilityType::Mortgage)
        );
    }

    #[test]
    fn test_from_str_loose_unknown() {
        assert_eq!(LiabilityType::from_str_loose("invalid"), None);
    }
}

// ============================================================
// Cross-cutting: Display trait delegates to as_str for all enums
// ============================================================

mod display_trait {
    use feature_finance::types::{
        AccountType, AssetType, BudgetMethod, BudgetPeriod, GoalStatus, GoalType,
        InvestmentTxType, JarType, LiabilityType, TransactionType,
    };

    #[test]
    fn test_all_enums_display_matches_as_str() {
        assert_eq!(
            format!("{}", AccountType::Cash),
            AccountType::Cash.as_str()
        );
        assert_eq!(
            format!("{}", TransactionType::Income),
            TransactionType::Income.as_str()
        );
        assert_eq!(
            format!("{}", BudgetPeriod::Monthly),
            BudgetPeriod::Monthly.as_str()
        );
        assert_eq!(
            format!("{}", BudgetMethod::Standard),
            BudgetMethod::Standard.as_str()
        );
        assert_eq!(
            format!("{}", JarType::Essentials),
            JarType::Essentials.as_str()
        );
        assert_eq!(
            format!("{}", AssetType::Stock),
            AssetType::Stock.as_str()
        );
        assert_eq!(
            format!("{}", InvestmentTxType::Buy),
            InvestmentTxType::Buy.as_str()
        );
        assert_eq!(
            format!("{}", GoalType::Savings),
            GoalType::Savings.as_str()
        );
        assert_eq!(
            format!("{}", GoalStatus::Active),
            GoalStatus::Active.as_str()
        );
        assert_eq!(
            format!("{}", LiabilityType::Mortgage),
            LiabilityType::Mortgage.as_str()
        );
    }
}

// ============================================================
// Cross-cutting: FromStr trait delegates to from_str_loose
// ============================================================

mod from_str_trait {
    use feature_finance::types::{
        AccountType, AssetType, BudgetMethod, BudgetPeriod, GoalStatus, GoalType,
        InvestmentTxType, JarType, LiabilityType, TransactionType,
    };
    use std::str::FromStr;

    #[test]
    fn test_all_enums_from_str_delegates_to_from_str_loose() {
        assert!(AccountType::from_str("cash").is_ok());
        assert!(TransactionType::from_str("income").is_ok());
        assert!(BudgetPeriod::from_str("monthly").is_ok());
        assert!(BudgetMethod::from_str("standard").is_ok());
        assert!(JarType::from_str("essentials").is_ok());
        assert!(AssetType::from_str("stock").is_ok());
        assert!(InvestmentTxType::from_str("buy").is_ok());
        assert!(GoalType::from_str("savings").is_ok());
        assert!(GoalStatus::from_str("active").is_ok());
        assert!(LiabilityType::from_str("mortgage").is_ok());
    }

    #[test]
    fn test_all_enums_from_str_unknown_returns_err() {
        assert!(AccountType::from_str("nonsense").is_err());
        assert!(TransactionType::from_str("nonsense").is_err());
        assert!(BudgetPeriod::from_str("nonsense").is_err());
        assert!(BudgetMethod::from_str("nonsense").is_err());
        assert!(JarType::from_str("nonsense").is_err());
        assert!(AssetType::from_str("nonsense").is_err());
        assert!(InvestmentTxType::from_str("nonsense").is_err());
        assert!(GoalType::from_str("nonsense").is_err());
        assert!(GoalStatus::from_str("nonsense").is_err());
        assert!(LiabilityType::from_str("nonsense").is_err());
    }
}
