//! Tests for Finance types with DomainEnum (AC 5.2).
//!
//! Verifies that all 10 Finance enums correctly use the DomainEnum derive macro
//! for from_str_loose (case-insensitive, alias-aware) and as_str (snake_case).
//!
//! Enums covered:
//!   AccountType, TransactionType, BudgetPeriod, BudgetMethod, JarType,
//!   AssetType, InvestmentTxType, GoalType, GoalStatus, LiabilityType
//!
//! Dev: implement these tests via TDD alongside the types in
//! `crates/feature-finance/src/types.rs`.

// NOTE: Adjust imports once feature-finance/src/types.rs is finalized.
//
// Expected imports:
//   use feature_finance::types::{
//       AccountType, TransactionType, BudgetPeriod, BudgetMethod, JarType,
//       AssetType, InvestmentTxType, GoalType, GoalStatus, LiabilityType,
//   };

// ============================================================
// 1. AccountType
// ============================================================

mod account_type {
    // use feature_finance::types::AccountType;

    #[test]
    fn test_as_str_cash() {
        // Verifies: AccountType::Cash.as_str() == "cash".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_bank() {
        // Verifies: AccountType::Bank.as_str() == "bank".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_ewallet() {
        // Verifies: AccountType::Ewallet.as_str() == "ewallet".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_crypto_wallet() {
        // Verifies: AccountType::CryptoWallet.as_str() == "crypto_wallet".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_brokerage() {
        // Verifies: AccountType::Brokerage.as_str() == "brokerage".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_e_wallet_alias() {
        // Verifies: AccountType::from_str_loose("e_wallet") == Some(AccountType::Ewallet).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_cryptowallet_alias() {
        // Verifies: AccountType::from_str_loose("cryptowallet") == Some(AccountType::CryptoWallet).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_case_insensitive() {
        // Verifies: AccountType::from_str_loose("CASH") == Some(AccountType::Cash),
        //           AccountType::from_str_loose("Bank") == Some(AccountType::Bank).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_unknown() {
        // Verifies: AccountType::from_str_loose("invalid") == None.
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 2. TransactionType
// ============================================================

mod transaction_type {
    // use feature_finance::types::TransactionType;

    #[test]
    fn test_as_str_income() {
        // Verifies: TransactionType::Income.as_str() == "income".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_expense() {
        // Verifies: TransactionType::Expense.as_str() == "expense".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_transfer() {
        // Verifies: TransactionType::Transfer.as_str() == "transfer".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_case_insensitive() {
        // Verifies: TransactionType::from_str_loose("INCOME") == Some(TransactionType::Income).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 3. BudgetPeriod
// ============================================================

mod budget_period {
    // use feature_finance::types::BudgetPeriod;

    #[test]
    fn test_as_str_monthly() {
        // Verifies: BudgetPeriod::Monthly.as_str() == "monthly".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_weekly() {
        // Verifies: BudgetPeriod::Weekly.as_str() == "weekly".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_yearly() {
        // Verifies: BudgetPeriod::Yearly.as_str() == "yearly".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_custom() {
        // Verifies: BudgetPeriod::Custom.as_str() == "custom".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_month_alias() {
        // Verifies: BudgetPeriod::from_str_loose("month") == Some(BudgetPeriod::Monthly).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_week_alias() {
        // Verifies: BudgetPeriod::from_str_loose("week") == Some(BudgetPeriod::Weekly).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_year_and_annual_aliases() {
        // Verifies: BudgetPeriod::from_str_loose("year") == Some(BudgetPeriod::Yearly),
        //           BudgetPeriod::from_str_loose("annual") == Some(BudgetPeriod::Yearly).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 4. BudgetMethod
// ============================================================

mod budget_method {
    // use feature_finance::types::BudgetMethod;

    #[test]
    fn test_as_str_standard() {
        // Verifies: BudgetMethod::Standard.as_str() == "standard".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_six_jar() {
        // Verifies: BudgetMethod::SixJar.as_str() == "six_jar".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_sixjar_alias() {
        // Verifies: BudgetMethod::from_str_loose("sixjar") == Some(BudgetMethod::SixJar).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_6jar_alias() {
        // Verifies: BudgetMethod::from_str_loose("6jar") == Some(BudgetMethod::SixJar).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 5. JarType
// ============================================================

mod jar_type {
    // use feature_finance::types::JarType;

    #[test]
    fn test_as_str_essentials() {
        // Verifies: JarType::Essentials.as_str() == "essentials".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_savings() {
        // Verifies: JarType::Savings.as_str() == "savings".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_investment() {
        // Verifies: JarType::Investment.as_str() == "investment".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_education() {
        // Verifies: JarType::Education.as_str() == "education".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_entertainment() {
        // Verifies: JarType::Entertainment.as_str() == "entertainment".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_charity() {
        // Verifies: JarType::Charity.as_str() == "charity".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_necessities_alias() {
        // Verifies: JarType::from_str_loose("necessities") == Some(JarType::Essentials).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_saving_alias() {
        // Verifies: JarType::from_str_loose("saving") == Some(JarType::Savings).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_play_alias() {
        // Verifies: JarType::from_str_loose("play") == Some(JarType::Entertainment).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_give_alias() {
        // Verifies: JarType::from_str_loose("give") == Some(JarType::Charity).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_investments_alias() {
        // Verifies: JarType::from_str_loose("investments") == Some(JarType::Investment).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 6. AssetType
// ============================================================

mod asset_type {
    // use feature_finance::types::AssetType;

    #[test]
    fn test_as_str_stock() {
        // Verifies: AssetType::Stock.as_str() == "stock".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_etf() {
        // Verifies: AssetType::Etf.as_str() == "etf".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_crypto() {
        // Verifies: AssetType::Crypto.as_str() == "crypto".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_real_estate() {
        // Verifies: AssetType::RealEstate.as_str() == "real_estate".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_bond() {
        // Verifies: AssetType::Bond.as_str() == "bond".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_exchange_rate() {
        // Verifies: AssetType::ExchangeRate.as_str() == "exchange_rate".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_stocks_alias() {
        // Verifies: AssetType::from_str_loose("stocks") == Some(AssetType::Stock).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_equity_alias() {
        // Verifies: AssetType::from_str_loose("equity") == Some(AssetType::Stock).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_cryptocurrency_alias() {
        // Verifies: AssetType::from_str_loose("cryptocurrency") == Some(AssetType::Crypto).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_property_alias() {
        // Verifies: AssetType::from_str_loose("property") == Some(AssetType::RealEstate).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_forex_alias() {
        // Verifies: AssetType::from_str_loose("forex") == Some(AssetType::ExchangeRate),
        //           AssetType::from_str_loose("fx") == Some(AssetType::ExchangeRate).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_bonds_alias() {
        // Verifies: AssetType::from_str_loose("bonds") == Some(AssetType::Bond),
        //           AssetType::from_str_loose("fixed_income") == Some(AssetType::Bond).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 7. InvestmentTxType
// ============================================================

mod investment_tx_type {
    // use feature_finance::types::InvestmentTxType;

    #[test]
    fn test_as_str_buy() {
        // Verifies: InvestmentTxType::Buy.as_str() == "buy".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_sell() {
        // Verifies: InvestmentTxType::Sell.as_str() == "sell".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_dividend() {
        // Verifies: InvestmentTxType::Dividend.as_str() == "dividend".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_rental_income() {
        // Verifies: InvestmentTxType::RentalIncome.as_str() == "rental_income".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_interest() {
        // Verifies: InvestmentTxType::Interest.as_str() == "interest".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_split() {
        // Verifies: InvestmentTxType::Split.as_str() == "split".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_purchase_alias() {
        // Verifies: InvestmentTxType::from_str_loose("purchase") == Some(InvestmentTxType::Buy).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_sale_alias() {
        // Verifies: InvestmentTxType::from_str_loose("sale") == Some(InvestmentTxType::Sell).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_rental_aliases() {
        // Verifies: InvestmentTxType::from_str_loose("rental") == Some(InvestmentTxType::RentalIncome),
        //           InvestmentTxType::from_str_loose("rent") == Some(InvestmentTxType::RentalIncome).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 8. GoalType
// ============================================================

mod goal_type {
    // use feature_finance::types::GoalType;

    #[test]
    fn test_as_str_savings() {
        // Verifies: GoalType::Savings.as_str() == "savings".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_purchase() {
        // Verifies: GoalType::Purchase.as_str() == "purchase".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_debt_payoff() {
        // Verifies: GoalType::DebtPayoff.as_str() == "debt_payoff".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_fire() {
        // Verifies: GoalType::Fire.as_str() == "fire".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_saving_alias() {
        // Verifies: GoalType::from_str_loose("saving") == Some(GoalType::Savings).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_emergency_fund_alias() {
        // Verifies: GoalType::from_str_loose("emergency_fund") == Some(GoalType::Savings).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_buy_alias() {
        // Verifies: GoalType::from_str_loose("buy") == Some(GoalType::Purchase).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_debt_aliases() {
        // Verifies: GoalType::from_str_loose("debt") == Some(GoalType::DebtPayoff),
        //           GoalType::from_str_loose("payoff") == Some(GoalType::DebtPayoff).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_financial_independence_alias() {
        // Verifies: GoalType::from_str_loose("financial_independence") == Some(GoalType::Fire).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 9. GoalStatus
// ============================================================

mod goal_status {
    // use feature_finance::types::GoalStatus;

    #[test]
    fn test_as_str_active() {
        // Verifies: GoalStatus::Active.as_str() == "active".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_achieved() {
        // Verifies: GoalStatus::Achieved.as_str() == "achieved".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_abandoned() {
        // Verifies: GoalStatus::Abandoned.as_str() == "abandoned".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_in_progress_alias() {
        // Verifies: GoalStatus::from_str_loose("in_progress") == Some(GoalStatus::Active).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_completed_alias() {
        // Verifies: GoalStatus::from_str_loose("completed") == Some(GoalStatus::Achieved),
        //           GoalStatus::from_str_loose("done") == Some(GoalStatus::Achieved).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_cancelled_alias() {
        // Verifies: GoalStatus::from_str_loose("cancelled") == Some(GoalStatus::Abandoned).
        // AC 5.2
        todo!()
    }
}

// ============================================================
// 10. LiabilityType
// ============================================================

mod liability_type {
    // use feature_finance::types::LiabilityType;

    #[test]
    fn test_as_str_mortgage() {
        // Verifies: LiabilityType::Mortgage.as_str() == "mortgage".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_credit_card() {
        // Verifies: LiabilityType::CreditCard.as_str() == "credit_card".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_personal_loan() {
        // Verifies: LiabilityType::PersonalLoan.as_str() == "personal_loan".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_student_loan() {
        // Verifies: LiabilityType::StudentLoan.as_str() == "student_loan".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_as_str_other() {
        // Verifies: LiabilityType::Other.as_str() == "other".
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_home_loan_alias() {
        // Verifies: LiabilityType::from_str_loose("home_loan") == Some(LiabilityType::Mortgage).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_creditcard_alias() {
        // Verifies: LiabilityType::from_str_loose("creditcard") == Some(LiabilityType::CreditCard),
        //           LiabilityType::from_str_loose("cc") == Some(LiabilityType::CreditCard).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_personal_alias() {
        // Verifies: LiabilityType::from_str_loose("personal") == Some(LiabilityType::PersonalLoan).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_student_aliases() {
        // Verifies: LiabilityType::from_str_loose("student") == Some(LiabilityType::StudentLoan),
        //           LiabilityType::from_str_loose("education_loan") == Some(LiabilityType::StudentLoan).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_case_insensitive() {
        // Verifies: LiabilityType::from_str_loose("MORTGAGE") == Some(LiabilityType::Mortgage).
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_from_str_loose_unknown() {
        // Verifies: LiabilityType::from_str_loose("invalid") == None.
        // AC 5.2
        todo!()
    }
}

// ============================================================
// Cross-cutting: Display trait delegates to as_str for all enums
// ============================================================

mod display_trait {
    #[test]
    fn test_all_enums_display_matches_as_str() {
        // Verifies: For every enum, format!("{}", variant) == variant.as_str().
        // This is a cross-cutting test covering all 10 enums.
        // AC 5.2
        todo!()
    }
}

// ============================================================
// Cross-cutting: FromStr trait delegates to from_str_loose
// ============================================================

mod from_str_trait {
    #[test]
    fn test_all_enums_from_str_delegates_to_from_str_loose() {
        // Verifies: For every enum, "canonical".parse::<EnumType>() is Ok.
        // AC 5.2
        todo!()
    }

    #[test]
    fn test_all_enums_from_str_unknown_returns_err() {
        // Verifies: For every enum, "nonsense".parse::<EnumType>() is Err.
        // AC 5.2
        todo!()
    }
}
